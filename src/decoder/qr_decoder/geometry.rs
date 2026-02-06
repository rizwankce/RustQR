use crate::decoder::function_mask::alignment_pattern_positions;
use crate::models::{BitMatrix, Point};
use crate::utils::geometry::PerspectiveTransform;

pub(super) fn calculate_bottom_right(
    top_left: &Point,
    top_right: &Point,
    bottom_left: &Point,
) -> Option<Point> {
    // In a perfect QR code, bottom_right = top_right + bottom_left - top_left
    let x = top_right.x + bottom_left.x - top_left.x;
    let y = top_right.y + bottom_left.y - top_left.y;
    Some(Point::new(x, y))
}

pub(super) fn estimate_dimension(
    top_left: &Point,
    top_right: &Point,
    _bottom_right: &Point,
    module_size: f32,
) -> Option<usize> {
    // Calculate width in modules
    let width_pixels = top_left.distance(top_right);
    let width_modules = (width_pixels / module_size).round() as i32;

    // Infer version from measured width.
    let dimension = width_modules + 7;

    if cfg!(debug_assertions) && crate::debug::debug_enabled() {
        eprintln!(
            "    DECODE: width_pixels={:.1}, width_modules={}, raw_dimension={}",
            width_pixels, width_modules, dimension
        );
    }

    if dimension < 21 {
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!("    DECODE: dimension {} < 21, FAIL", dimension);
        }
        return None;
    }

    // Round to nearest valid dimension (must be 21, 25, 29, ... 177)
    let raw_version = ((dimension - 17) as f32 / 4.0).round() as i32;
    let version = raw_version as u8;

    if cfg!(debug_assertions) && crate::debug::debug_enabled() {
        eprintln!(
            "    DECODE: raw_version={}, final_version={}",
            raw_version, version
        );
    }

    if (1..=40).contains(&version) {
        let final_dim = 17 + 4 * version as usize;
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!("    DECODE: final_dimension={}", final_dim);
        }
        Some(final_dim)
    } else {
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!("    DECODE: version {} out of range, FAIL", version);
        }
        None
    }
}

pub(super) fn version_candidates(estimated_version: i32) -> Vec<u8> {
    let mut candidates = Vec::new();
    for delta in -2..=2 {
        let v = estimated_version + delta;
        if (1..=40).contains(&v) {
            candidates.push(v as u8);
        }
    }
    candidates
}

pub(super) fn build_transform(
    top_left: &Point,
    top_right: &Point,
    bottom_left: &Point,
    bottom_right: &Point,
    dimension: usize,
) -> Option<PerspectiveTransform> {
    let src = [
        Point::new(3.5, 3.5),
        Point::new(dimension as f32 - 3.5, 3.5),
        Point::new(3.5, dimension as f32 - 3.5),
        Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5),
    ];
    let dst = [*top_left, *top_right, *bottom_left, *bottom_right];
    PerspectiveTransform::from_points(&src, &dst)
}

pub(super) fn extract_qr_region_with_transform(
    matrix: &BitMatrix,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> BitMatrix {
    let mut result = BitMatrix::new(dimension, dimension);

    for y in 0..dimension {
        for x in 0..dimension {
            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let img_point = transform.transform(&module_center);

            let img_x = img_point.x.round() as isize;
            let img_y = img_point.y.round() as isize;

            let mut black = 0;
            let mut total = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = img_x + dx;
                    let sy = img_y + dy;
                    if sx >= 0
                        && sy >= 0
                        && (sx as usize) < matrix.width()
                        && (sy as usize) < matrix.height()
                    {
                        total += 1;
                        if matrix.get(sx as usize, sy as usize) {
                            black += 1;
                        }
                    }
                }
            }
            if total > 0 {
                result.set(x, y, black * 2 >= total);
            }
        }
    }

    result
}

pub(super) fn extract_qr_region_gray_with_transform(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> BitMatrix {
    let mut samples: Vec<u8> = Vec::with_capacity(dimension * dimension);
    for y in 0..dimension {
        for x in 0..dimension {
            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let img_point = transform.transform(&module_center);
            let img_x = img_point.x.round() as isize;
            let img_y = img_point.y.round() as isize;

            let mut sum = 0u32;
            let mut count = 0u32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let sx = img_x + dx;
                    let sy = img_y + dy;
                    if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height {
                        let idx = sy as usize * width + sx as usize;
                        sum += gray[idx] as u32;
                        count += 1;
                    }
                }
            }
            let avg = if count > 0 {
                (sum / count) as u8
            } else {
                255u8
            };
            samples.push(avg);
        }
    }

    let mut result = BitMatrix::new(dimension, dimension);
    for y in 0..dimension {
        let row_start = y * dimension;
        let row_end = row_start + dimension;
        let mut row_sorted: Vec<u8> = samples[row_start..row_end].to_vec();
        row_sorted.sort_unstable();
        let row_threshold = row_sorted[row_sorted.len() / 2];

        for x in 0..dimension {
            let idx = y * dimension + x;
            result.set(x, y, samples[idx] < row_threshold);
        }
    }

    result
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refine_transform_with_alignment(
    binary: &BitMatrix,
    transform: &PerspectiveTransform,
    version_num: u8,
    dimension: usize,
    module_size: f32,
    top_left: &Point,
    top_right: &Point,
    bottom_left: &Point,
) -> Option<PerspectiveTransform> {
    if version_num < 2 || module_size < 1.0 {
        return None;
    }

    let centers = alignment_centers(version_num, dimension);
    let (ax, ay) = centers.iter().max_by_key(|(x, y)| x + y)?;
    let align_src = Point::new(*ax as f32 + 0.5, *ay as f32 + 0.5);
    let predicted = transform.transform(&align_src);
    let found = find_alignment_center(binary, predicted, module_size)?;

    let src = [
        Point::new(3.5, 3.5),
        Point::new(dimension as f32 - 3.5, 3.5),
        Point::new(3.5, dimension as f32 - 3.5),
        align_src,
    ];
    let dst = [*top_left, *top_right, *bottom_left, found];
    PerspectiveTransform::from_points(&src, &dst)
}

fn alignment_centers(version: u8, dimension: usize) -> Vec<(usize, usize)> {
    let positions = alignment_pattern_positions(version);
    if positions.is_empty() {
        return Vec::new();
    }

    let mut centers = Vec::new();
    for &cx in &positions {
        for &cy in &positions {
            let in_tl = cx <= 8 && cy <= 8;
            let in_tr = cx >= dimension - 9 && cy <= 8;
            let in_bl = cx <= 8 && cy >= dimension - 9;
            if in_tl || in_tr || in_bl {
                continue;
            }
            centers.push((cx, cy));
        }
    }
    centers
}

fn find_alignment_center(binary: &BitMatrix, predicted: Point, module_size: f32) -> Option<Point> {
    if !predicted.x.is_finite() || !predicted.y.is_finite() {
        return None;
    }

    let radius = (module_size * 4.0).max(4.0);
    let min_x = (predicted.x - radius).floor().max(0.0) as isize;
    let max_x = (predicted.x + radius)
        .ceil()
        .min((binary.width().saturating_sub(1)) as f32) as isize;
    let min_y = (predicted.y - radius).floor().max(0.0) as isize;
    let max_y = (predicted.y + radius)
        .ceil()
        .min((binary.height().saturating_sub(1)) as f32) as isize;

    let mut best: Option<(Point, usize)> = None;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let center = Point::new(x as f32, y as f32);
            let mismatch = match alignment_pattern_mismatch(binary, &center, module_size) {
                Some(v) => v,
                None => continue,
            };
            match best {
                Some((_, best_mismatch)) if mismatch >= best_mismatch => {}
                _ => best = Some((center, mismatch)),
            }
        }
    }

    match best {
        Some((center, mismatch)) if mismatch <= 8 => Some(center),
        _ => None,
    }
}

fn alignment_pattern_mismatch(
    binary: &BitMatrix,
    center: &Point,
    module_size: f32,
) -> Option<usize> {
    let mut mismatches = 0usize;
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            let expected_black = dx.abs() == 2 || dy.abs() == 2 || (dx == 0 && dy == 0);
            let sx = center.x + dx as f32 * module_size;
            let sy = center.y + dy as f32 * module_size;
            let ix = sx.round() as isize;
            let iy = sy.round() as isize;
            if ix < 0
                || iy < 0
                || (ix as usize) >= binary.width()
                || (iy as usize) >= binary.height()
            {
                return None;
            }
            let actual = binary.get(ix as usize, iy as usize);
            if actual != expected_black {
                mismatches += 1;
            }
        }
    }

    Some(mismatches)
}
