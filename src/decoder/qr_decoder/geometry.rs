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
    let (matrix, _) = extract_qr_region_gray_with_transform_and_confidence(
        gray, width, height, transform, dimension,
    );
    matrix
}

pub(super) fn extract_qr_region_gray_with_transform_and_confidence(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> (BitMatrix, Vec<u8>) {
    extract_qr_region_gray_with_variant(gray, width, height, transform, dimension, 0.0, 0.0, 1.0)
}

pub(super) fn extract_qr_region_gray_with_transform_and_confidence_scaled(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
    sample_scale: f32,
) -> (BitMatrix, Vec<u8>) {
    extract_qr_region_gray_with_variant(
        gray,
        width,
        height,
        transform,
        dimension,
        0.0,
        0.0,
        sample_scale,
    )
}

pub(super) fn extract_qr_region_gray_with_radial_compensation(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> Option<(BitMatrix, Vec<u8>)> {
    let k1 = estimate_radial_k1(transform, dimension)?;
    Some(extract_qr_region_gray_with_variant(
        gray, width, height, transform, dimension, k1, 0.0, 1.0,
    ))
}

pub(super) fn extract_qr_region_gray_with_mesh_warp(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> (BitMatrix, Vec<u8>) {
    extract_qr_region_gray_with_variant(gray, width, height, transform, dimension, 0.0, 0.9, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn extract_qr_region_gray_with_variant(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
    radial_k1: f32,
    mesh_strength: f32,
    sample_scale: f32,
) -> (BitMatrix, Vec<u8>) {
    let mut samples: Vec<f32> = vec![255.0; dimension * dimension];
    let mut local_std_dev: Vec<f32> = vec![0.0; dimension * dimension];
    // A camera highlight can turn an entire sample footprint into white.  It
    // should not become an apparently high-confidence white module merely
    // because its samples agree with one another.
    let mut bright_saturation: Vec<f32> = vec![0.0; dimension * dimension];
    let center_module = Point::new(
        (dimension as f32 - 1.0) * 0.5,
        (dimension as f32 - 1.0) * 0.5,
    );
    let center_image = transform.transform(&center_module);
    for y in 0..dimension {
        for x in 0..dimension {
            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let mut img_point = transform.transform(&module_center);
            if radial_k1 != 0.0 {
                let ux = ((x as f32 + 0.5) / dimension as f32) - 0.5;
                let uy = ((y as f32 + 0.5) / dimension as f32) - 0.5;
                let r2 = ux * ux + uy * uy;
                let scale = 1.0 + radial_k1 * r2;
                img_point.x = center_image.x + (img_point.x - center_image.x) * scale;
                img_point.y = center_image.y + (img_point.y - center_image.y) * scale;
            }
            if mesh_strength != 0.0 {
                let ux = ((x as f32 + 0.5) / dimension as f32) - 0.5;
                let uy = ((y as f32 + 0.5) / dimension as f32) - 0.5;
                let dx = mesh_strength * ux * uy * 2.0;
                let dy = mesh_strength * (ux * ux - uy * uy) * 0.8;
                img_point.x += dx;
                img_point.y += dy;
            }
            let module_px = estimate_local_module_pixels(transform, x, y);
            let radius =
                ((adaptive_kernel_radius(module_px) as f32) * sample_scale).round() as usize;
            // At sub-pixel pitch, a single bilinear centre sample is safer
            // than smearing neighbouring modules into the decision.
            let radius = radius.clamp(0, 4);
            let sample_step = (0.35 / sample_scale.max(0.8)).clamp(0.2, 0.45);

            let mut sum = 0.0f32;
            let mut sum_sq = 0.0f32;
            let mut count = 0usize;
            let mut saturated = 0usize;
            for oy in -(radius as isize)..=(radius as isize) {
                for ox in -(radius as isize)..=(radius as isize) {
                    let sx = img_point.x + ox as f32 * sample_step;
                    let sy = img_point.y + oy as f32 * sample_step;
                    if let Some(v) = bilinear_sample(gray, width, height, sx, sy) {
                        sum += v;
                        sum_sq += v * v;
                        count += 1;
                        // 250 leaves a small guard band below the usual
                        // 8-bit white clip value while not penalising normal
                        // paper/background values.
                        if v >= 250.0 {
                            saturated += 1;
                        }
                    }
                }
            }

            let idx = y * dimension + x;
            let avg = if count > 0 { sum / count as f32 } else { 255.0 };
            let variance = if count > 1 {
                let c = count as f32;
                (sum_sq / c) - avg * avg
            } else {
                0.0
            };
            samples[idx] = avg;
            local_std_dev[idx] = variance.max(0.0).sqrt();
            bright_saturation[idx] = if count > 0 {
                saturated as f32 / count as f32
            } else {
                1.0
            };
        }
    }

    let mut result = BitMatrix::new(dimension, dimension);
    let mut confidence = vec![0u8; dimension * dimension];
    for y in 0..dimension {
        for x in 0..dimension {
            let idx = y * dimension + x;
            let local_t = local_threshold(&samples, dimension, x, y);
            let s = samples[idx];
            result.set(x, y, s < local_t);

            let margin = (s - local_t).abs();
            let var_penalty = (local_std_dev[idx] / 96.0).clamp(0.0, 1.0);
            // White-clipped samples are ambiguous under glare.  Keep their
            // sampled bit (the matrix decoder may still recover it), but make
            // the uncertainty available to confidence-guided RS recovery.
            let glare_penalty = bright_saturation[idx];
            let conf = ((margin / 64.0) * (1.0 - 0.45 * var_penalty) * (1.0 - glare_penalty))
                .clamp(0.0, 1.0);
            confidence[idx] = (conf * 255.0).round() as u8;
        }
    }

    (result, confidence)
}

fn estimate_radial_k1(transform: &PerspectiveTransform, dimension: usize) -> Option<f32> {
    if dimension < 21 {
        return None;
    }
    let c = estimate_local_module_pixels(transform, dimension / 2, dimension / 2);
    if c <= 0.0 {
        return None;
    }
    let c00 = estimate_local_module_pixels(transform, 1, 1);
    let c10 = estimate_local_module_pixels(transform, dimension.saturating_sub(2), 1);
    let c01 = estimate_local_module_pixels(transform, 1, dimension.saturating_sub(2));
    let c11 = estimate_local_module_pixels(
        transform,
        dimension.saturating_sub(2),
        dimension.saturating_sub(2),
    );
    let corner_avg = (c00 + c10 + c01 + c11) * 0.25;
    let ratio = (corner_avg - c) / c;
    if ratio.abs() < 0.12 {
        return None;
    }
    Some((ratio * 0.35).clamp(-0.18, 0.18))
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
    refine_transform_with_timing_and_alignment(
        binary,
        None,
        0,
        0,
        transform,
        version_num,
        dimension,
        module_size,
        top_left,
        top_right,
        bottom_left,
    )
}

/// Refine a finder-derived homography with a fixed number of alignment probes
/// and, when available, grayscale timing residuals.
///
/// The fit deliberately remains bounded: it inspects at most six alignment
/// centers and evaluates nine sub-module offsets for each observed center.
/// Every spec-relevant alignment center still contributes to ranking, so a
/// high-version candidate is not selected solely by its far-corner pattern.
#[allow(clippy::too_many_arguments)]
pub(super) fn refine_transform_with_timing_and_alignment(
    binary: &BitMatrix,
    gray: Option<&[u8]>,
    gray_width: usize,
    gray_height: usize,
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

    let centers = prioritized_alignment_centers(version_num, dimension);
    let original_score = refinement_quality(
        binary,
        gray,
        gray_width,
        gray_height,
        transform,
        dimension,
        version_num,
        module_size,
    );
    let mut best: Option<(PerspectiveTransform, f32)> = None;

    for (ax, ay) in centers {
        let align_src = Point::new(ax as f32 + 0.5, ay as f32 + 0.5);
        let predicted = transform.transform(&align_src);
        let Some(found) = find_alignment_center(binary, predicted, module_size) else {
            continue;
        };
        let Some(candidate) = best_refined_transform(
            binary,
            gray,
            gray_width,
            gray_height,
            dimension,
            version_num,
            top_left,
            top_right,
            bottom_left,
            align_src,
            found,
            module_size,
        ) else {
            continue;
        };
        let score = refinement_quality(
            binary,
            gray,
            gray_width,
            gray_height,
            &candidate,
            dimension,
            version_num,
            module_size,
        );
        match &best {
            Some((_, best_score)) if score <= *best_score => {}
            _ => best = Some((candidate, score)),
        }
    }

    let (best, score) = best?;
    // A refinement is only allowed to replace the finder homography when the
    // combined residual improves by a small margin. This prevents a noisy
    // alignment-like blob from adding recovery work without improving samples.
    (score > original_score + 0.002).then_some(best)
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

fn prioritized_alignment_centers(version: u8, dimension: usize) -> Vec<(usize, usize)> {
    const MAX_ALIGNMENT_PROBES: usize = 6;
    let mut centers = alignment_centers(version, dimension);
    // The far corner is normally most informative for perspective, but use a
    // deterministic spatial spread after it so high versions do not depend on
    // a single (possibly occluded) pattern.
    centers.sort_by(|left, right| {
        let left_distance = left.0 * left.0 + left.1 * left.1;
        let right_distance = right.0 * right.0 + right.1 * right.1;
        right_distance
            .cmp(&left_distance)
            .then_with(|| right.cmp(left))
    });
    centers.truncate(MAX_ALIGNMENT_PROBES);
    centers
}

fn find_alignment_center(binary: &BitMatrix, predicted: Point, module_size: f32) -> Option<Point> {
    if !predicted.x.is_finite() || !predicted.y.is_finite() {
        return None;
    }

    // Increased radius from 4.0*module_size to 6.0*module_size for better high-version detection
    let radius = (module_size * 6.0).max(6.0);
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

    // Relaxed threshold from 8 to 10 for high-version QR codes
    match best {
        Some((center, mismatch)) if mismatch <= 10 => Some(center),
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

fn bilinear_sample(gray: &[u8], width: usize, height: usize, x: f32, y: f32) -> Option<f32> {
    if x < 0.0 || y < 0.0 {
        return None;
    }
    if x > (width as f32 - 1.0) || y > (height as f32 - 1.0) {
        return None;
    }

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let p00 = gray[y0 * width + x0] as f32;
    let p10 = gray[y0 * width + x1] as f32;
    let p01 = gray[y1 * width + x0] as f32;
    let p11 = gray[y1 * width + x1] as f32;

    Some(p00 * w00 + p10 * w10 + p01 * w01 + p11 * w11)
}

fn estimate_local_module_pixels(transform: &PerspectiveTransform, x: usize, y: usize) -> f32 {
    let p = transform.transform(&Point::new(x as f32 + 0.5, y as f32 + 0.5));
    let px = transform.transform(&Point::new(x as f32 + 1.5, y as f32 + 0.5));
    let py = transform.transform(&Point::new(x as f32 + 0.5, y as f32 + 1.5));
    let sx = p.distance(&px);
    let sy = p.distance(&py);
    ((sx + sy) * 0.5).clamp(0.5, 8.0)
}

fn adaptive_kernel_radius(module_px: f32) -> usize {
    if module_px < 1.5 {
        0
    } else if module_px < 2.5 {
        1
    } else if module_px < 4.0 {
        2
    } else {
        3
    }
}

fn local_threshold(samples: &[f32], dimension: usize, x: usize, y: usize) -> f32 {
    let radius = 2usize;
    let min_x = x.saturating_sub(radius);
    let max_x = (x + radius).min(dimension - 1);
    let min_y = y.saturating_sub(radius);
    let max_y = (y + radius).min(dimension - 1);

    let mut sum = 0.0f32;
    let mut count = 0usize;
    for yy in min_y..=max_y {
        for xx in min_x..=max_x {
            sum += samples[yy * dimension + xx];
            count += 1;
        }
    }
    let mean = if count > 0 { sum / count as f32 } else { 127.0 };
    mean - 3.0
}

fn transform_quality(
    binary: &BitMatrix,
    transform: &PerspectiveTransform,
    dimension: usize,
    version_num: u8,
    module_size: f32,
) -> f32 {
    let mut score = 0.0f32;
    score += timing_quality(binary, transform, dimension) * 0.75;

    if version_num >= 2 {
        let centers = alignment_centers(version_num, dimension);
        let mut alignment_score = 0.0;
        let mut observed = 0usize;
        // Every spec-relevant alignment center contributes to the rank.  The
        // count is bounded by QR version 40 (at most 46 centers), unlike a
        // free-form pixel offset search.
        for (ax, ay) in centers {
            let p = transform.transform(&Point::new(ax as f32 + 0.5, ay as f32 + 0.5));
            if let Some(mm) = alignment_pattern_mismatch(binary, &p, module_size.max(1.0)) {
                alignment_score += 1.0 - (mm as f32 / 25.0).clamp(0.0, 1.0);
                observed += 1;
            }
        }
        if observed > 0 {
            score += (alignment_score / observed as f32) * 0.25;
        }
    } else {
        score += 0.25;
    }

    score
}

#[allow(clippy::too_many_arguments)] // Mirrors the transform refinement inputs.
fn refinement_quality(
    binary: &BitMatrix,
    gray: Option<&[u8]>,
    gray_width: usize,
    gray_height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
    version_num: u8,
    module_size: f32,
) -> f32 {
    let binary_quality = transform_quality(binary, transform, dimension, version_num, module_size);
    let gray_quality = gray
        .filter(|_| gray_width > 0 && gray_height > 0)
        .map(|values| timing_quality_gray(values, gray_width, gray_height, transform, dimension));
    match gray_quality {
        Some(timing) => binary_quality * 0.7 + timing * 0.3,
        None => binary_quality,
    }
}

fn timing_quality(binary: &BitMatrix, transform: &PerspectiveTransform, dimension: usize) -> f32 {
    let mut h_bits = Vec::new();
    for m in 8..=(dimension.saturating_sub(9)) {
        let p = transform.transform(&Point::new(m as f32 + 0.5, 6.5));
        let ix = p.x.round() as isize;
        let iy = p.y.round() as isize;
        if ix < 0 || iy < 0 || ix as usize >= binary.width() || iy as usize >= binary.height() {
            continue;
        }
        h_bits.push(binary.get(ix as usize, iy as usize));
    }

    let mut v_bits = Vec::new();
    for m in 8..=(dimension.saturating_sub(9)) {
        let p = transform.transform(&Point::new(6.5, m as f32 + 0.5));
        let ix = p.x.round() as isize;
        let iy = p.y.round() as isize;
        if ix < 0 || iy < 0 || ix as usize >= binary.width() || iy as usize >= binary.height() {
            continue;
        }
        v_bits.push(binary.get(ix as usize, iy as usize));
    }

    let h = alternation_ratio(&h_bits);
    let v = alternation_ratio(&v_bits);
    (h + v) * 0.5
}

fn alternation_ratio(bits: &[bool]) -> f32 {
    if bits.len() < 2 {
        return 0.0;
    }

    let mut transitions = 0usize;
    for i in 1..bits.len() {
        if bits[i] != bits[i - 1] {
            transitions += 1;
        }
    }

    transitions as f32 / (bits.len() - 1) as f32
}

fn timing_quality_gray(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> f32 {
    let h = score_timing_line_gray(gray, width, height, transform, dimension, true);
    let v = score_timing_line_gray(gray, width, height, transform, dimension, false);
    let transitions = dimension.saturating_sub(17).saturating_sub(1).max(1) as f32;
    ((h + v) / (2.0 * 255.0 * transitions)).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn best_refined_transform(
    binary: &BitMatrix,
    gray: Option<&[u8]>,
    gray_width: usize,
    gray_height: usize,
    dimension: usize,
    version_num: u8,
    top_left: &Point,
    top_right: &Point,
    bottom_left: &Point,
    align_src: Point,
    align_dst: Point,
    module_size: f32,
) -> Option<PerspectiveTransform> {
    let src = [
        Point::new(3.5, 3.5),
        Point::new(dimension as f32 - 3.5, 3.5),
        Point::new(3.5, dimension as f32 - 3.5),
        align_src,
    ];

    let mut best: Option<(PerspectiveTransform, f32)> = None;
    let step = module_size.max(1.0) * 0.35;
    for oy in [-1.0f32, 0.0, 1.0] {
        for ox in [-1.0f32, 0.0, 1.0] {
            let dst = [
                *top_left,
                *top_right,
                *bottom_left,
                Point::new(align_dst.x + ox * step, align_dst.y + oy * step),
            ];
            let Some(t) = PerspectiveTransform::from_points(&src, &dst) else {
                continue;
            };
            let s = refinement_quality(
                binary,
                gray,
                gray_width,
                gray_height,
                &t,
                dimension,
                version_num,
                module_size,
            );
            match &best {
                Some((_, bs)) if s <= *bs => {}
                _ => best = Some((t, s)),
            }
        }
    }

    best.map(|(t, _)| t)
}

#[allow(dead_code)]
pub(super) fn refine_transform_gray_timing(
    gray: &[u8],
    width: usize,
    height: usize,
    base_transform: &PerspectiveTransform,
    dimension: usize,
    module_size: f32,
) -> PerspectiveTransform {
    let step = module_size * 0.15;
    let mut best_transform = base_transform.clone();
    let mut best_score = score_timing_gray(gray, width, height, base_transform, dimension);

    let offsets = [
        (step, 0.0),
        (-step, 0.0),
        (0.0, step),
        (0.0, -step),
        (step, step),
        (-step, -step),
        (step, -step),
        (-step, step),
    ];

    for &(dx, dy) in &offsets {
        let shifted = match shift_transform(base_transform, dx, dy, dimension) {
            Some(t) => t,
            None => continue,
        };
        let score = score_timing_gray(gray, width, height, &shifted, dimension);
        if score > best_score {
            best_score = score;
            best_transform = shifted;
        }
    }

    best_transform
}

#[allow(dead_code)]
fn score_timing_gray(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
) -> f32 {
    let h_score = score_timing_line_gray(gray, width, height, transform, dimension, true);
    let v_score = score_timing_line_gray(gray, width, height, transform, dimension, false);
    h_score + v_score
}

#[allow(dead_code)]
fn score_timing_line_gray(
    gray: &[u8],
    width: usize,
    height: usize,
    transform: &PerspectiveTransform,
    dimension: usize,
    horizontal: bool,
) -> f32 {
    let start = 8;
    let end = dimension.saturating_sub(9);
    if end <= start {
        return 0.0;
    }

    let mut values = Vec::with_capacity(end - start + 1);
    for m in start..=end {
        let p = if horizontal {
            transform.transform(&Point::new(m as f32 + 0.5, 6.5))
        } else {
            transform.transform(&Point::new(6.5, m as f32 + 0.5))
        };
        if let Some(v) = bilinear_sample(gray, width, height, p.x, p.y) {
            values.push(v);
        }
    }

    if values.len() < 3 {
        return 0.0;
    }

    let mut score = 0.0f32;
    for i in 1..values.len() {
        let diff = (values[i] - values[i - 1]).abs();
        score += diff;
    }
    score
}

#[allow(dead_code)]
fn shift_transform(
    base: &PerspectiveTransform,
    dx: f32,
    dy: f32,
    dimension: usize,
) -> Option<PerspectiveTransform> {
    let src = [
        Point::new(3.5, 3.5),
        Point::new(dimension as f32 - 3.5, 3.5),
        Point::new(3.5, dimension as f32 - 3.5),
        Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5),
    ];

    let dst: Vec<Point> = src
        .iter()
        .map(|s| {
            let p = base.transform(s);
            Point::new(p.x + dx, p.y + dy)
        })
        .collect();

    PerspectiveTransform::from_points(&src, &[dst[0], dst[1], dst[2], dst[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_v7_geometry() -> (BitMatrix, Vec<u8>, PerspectiveTransform, [Point; 3]) {
        const DIMENSION: usize = 45;
        const SCALE: f32 = 4.0;
        const OFFSET: f32 = 16.0;
        const IMAGE: usize = 224;

        let src = [
            Point::new(3.5, 3.5),
            Point::new(DIMENSION as f32 - 3.5, 3.5),
            Point::new(3.5, DIMENSION as f32 - 3.5),
            Point::new(DIMENSION as f32 - 3.5, DIMENSION as f32 - 3.5),
        ];
        let point = |x: f32, y: f32| Point::new(OFFSET + x * SCALE, OFFSET + y * SCALE);
        let finders = [point(3.5, 3.5), point(41.5, 3.5), point(3.5, 41.5)];
        let actual = [finders[0], finders[1], finders[2], point(41.5, 41.5)];
        let base = PerspectiveTransform::from_points(
            &src,
            &[actual[0], actual[1], actual[2], point(43.0, 43.0)],
        )
        .unwrap();

        let mut binary = BitMatrix::new(IMAGE, IMAGE);
        let mut gray = vec![232u8; IMAGE * IMAGE];
        let mut set = |module_x: f32, module_y: f32, black: bool| {
            let x = (OFFSET + module_x * SCALE).round() as usize;
            let y = (OFFSET + module_y * SCALE).round() as usize;
            binary.set(x, y, black);
            gray[y * IMAGE + x] = if black { 18 } else { 232 };
        };

        for module in 8..=(DIMENSION - 9) {
            let black = module % 2 == 0;
            set(module as f32 + 0.5, 6.5, black);
            set(6.5, module as f32 + 0.5, black);
        }
        for (ax, ay) in alignment_centers(7, DIMENSION) {
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    let black = dx.abs() == 2 || dy.abs() == 2 || (dx == 0 && dy == 0);
                    set(
                        ax as f32 + 0.5 + dx as f32,
                        ay as f32 + 0.5 + dy as f32,
                        black,
                    );
                }
            }
        }
        (binary, gray, base, finders)
    }

    #[test]
    fn radial_estimate_is_none_for_uniform_scale() {
        let src = [
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(0.0, 20.0),
            Point::new(20.0, 20.0),
        ];
        let dst = src;
        let transform = PerspectiveTransform::from_points(&src, &dst).unwrap();
        assert!(estimate_radial_k1(&transform, 21).is_none());
    }

    #[test]
    fn confidence_extraction_returns_expected_shape() {
        let dim = 21usize;
        let gray = vec![128u8; 64 * 64];
        let src = [
            Point::new(3.5, 3.5),
            Point::new(dim as f32 - 3.5, 3.5),
            Point::new(3.5, dim as f32 - 3.5),
            Point::new(dim as f32 - 3.5, dim as f32 - 3.5),
        ];
        let dst = [
            Point::new(10.0, 10.0),
            Point::new(54.0, 10.0),
            Point::new(10.0, 54.0),
            Point::new(54.0, 54.0),
        ];
        let transform = PerspectiveTransform::from_points(&src, &dst).unwrap();
        let (matrix, conf) =
            extract_qr_region_gray_with_transform_and_confidence(&gray, 64, 64, &transform, dim);
        assert_eq!(matrix.width(), dim);
        assert_eq!(matrix.height(), dim);
        assert_eq!(conf.len(), dim * dim);
    }

    #[test]
    fn saturated_white_samples_are_not_confident() {
        let dim = 21usize;
        let gray = vec![255u8; 64 * 64];
        let src = [
            Point::new(3.5, 3.5),
            Point::new(dim as f32 - 3.5, 3.5),
            Point::new(3.5, dim as f32 - 3.5),
            Point::new(dim as f32 - 3.5, dim as f32 - 3.5),
        ];
        let dst = [
            Point::new(10.0, 10.0),
            Point::new(54.0, 10.0),
            Point::new(10.0, 54.0),
            Point::new(54.0, 54.0),
        ];
        let transform = PerspectiveTransform::from_points(&src, &dst).unwrap();
        let (_, confidence) =
            extract_qr_region_gray_with_transform_and_confidence(&gray, 64, 64, &transform, dim);

        assert!(confidence.iter().all(|&value| value == 0));
    }

    #[test]
    fn sampling_footprint_does_not_blur_sub_pixel_modules() {
        assert_eq!(adaptive_kernel_radius(0.9), 0);
        assert_eq!(adaptive_kernel_radius(1.4), 0);
        assert_eq!(adaptive_kernel_radius(1.5), 1);
    }

    #[test]
    fn bounded_refinement_improves_timing_and_alignment_residuals() {
        let (binary, gray, base, finders) = synthetic_v7_geometry();
        let before = refinement_quality(&binary, Some(&gray), 224, 224, &base, 45, 7, 4.0);
        let refined = refine_transform_with_timing_and_alignment(
            &binary,
            Some(&gray),
            224,
            224,
            &base,
            7,
            45,
            4.0,
            &finders[0],
            &finders[1],
            &finders[2],
        )
        .expect("synthetic alignment pattern should produce an improved fit");
        let after = refinement_quality(&binary, Some(&gray), 224, 224, &refined, 45, 7, 4.0);
        assert!(after > before + 0.002, "before={before}, after={after}");
    }

    #[test]
    fn refinement_probes_have_a_fixed_upper_bound() {
        let probes = prioritized_alignment_centers(40, 177);
        assert_eq!(probes.len(), 6);
        assert!(alignment_centers(40, 177).len() > probes.len());
    }
}
