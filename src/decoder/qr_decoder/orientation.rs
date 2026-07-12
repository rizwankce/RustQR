use crate::decoder::version::VersionInfo;
use crate::detector::timing::read_timing_pattern;
use crate::models::{BitMatrix, Point};

#[allow(dead_code)]
pub(super) fn score_content(content: &str) -> i32 {
    if content.is_empty() {
        return -10_000;
    }
    if content.chars().all(|c| c.is_ascii_digit()) {
        return 1000 + content.len() as i32 * 10;
    }

    let mut digits = 0;
    let mut alnum = 0;
    let mut printable = 0;
    let mut total = 0;
    let mut non_ascii = 0;
    for ch in content.chars() {
        total += 1;
        if ch.is_ascii_digit() {
            digits += 1;
        }
        if ch.is_ascii_alphanumeric() {
            alnum += 1;
        }
        if ch.is_ascii_graphic() || ch == ' ' {
            printable += 1;
        }
        if !ch.is_ascii() {
            non_ascii += 1;
        }
    }
    let non_print = total - printable;
    let mut score = digits * 5 + alnum * 2 + printable - non_print * 5 - non_ascii * 10;
    if digits * 2 >= total {
        score += 50;
    }
    score
}

pub(super) fn rotate90(matrix: &BitMatrix) -> BitMatrix {
    let n = matrix.width();
    let mut out = BitMatrix::new(n, n);
    for y in 0..n {
        for x in 0..n {
            out.set(n - 1 - y, x, matrix.get(x, y));
        }
    }
    out
}

pub(super) fn rotate180(matrix: &BitMatrix) -> BitMatrix {
    let n = matrix.width();
    let mut out = BitMatrix::new(n, n);
    for y in 0..n {
        for x in 0..n {
            out.set(n - 1 - x, n - 1 - y, matrix.get(x, y));
        }
    }
    out
}

pub(super) fn rotate270(matrix: &BitMatrix) -> BitMatrix {
    let n = matrix.width();
    let mut out = BitMatrix::new(n, n);
    for y in 0..n {
        for x in 0..n {
            out.set(y, n - 1 - x, matrix.get(x, y));
        }
    }
    out
}

pub(super) fn flip_horizontal(matrix: &BitMatrix) -> BitMatrix {
    let n = matrix.width();
    let mut out = BitMatrix::new(n, n);
    for y in 0..n {
        for x in 0..n {
            out.set(n - 1 - x, y, matrix.get(x, y));
        }
    }
    out
}

pub(super) fn flip_vertical(matrix: &BitMatrix) -> BitMatrix {
    let n = matrix.width();
    let mut out = BitMatrix::new(n, n);
    for y in 0..n {
        for x in 0..n {
            out.set(x, n - 1 - y, matrix.get(x, y));
        }
    }
    out
}

pub(super) fn invert_matrix(matrix: &BitMatrix) -> BitMatrix {
    let mut out = BitMatrix::new(matrix.width(), matrix.height());
    for y in 0..matrix.height() {
        for x in 0..matrix.width() {
            out.set(x, y, !matrix.get(x, y));
        }
    }
    out
}

/// Check whether the matrix has finder patterns in the correct positions
/// for a properly oriented QR code: top-left (0,0), top-right (dim-7,0),
/// bottom-left (0,dim-7). Checks a small set of diagnostic cells at each
/// finder position (corners and center) to quickly determine orientation.
#[cfg(test)]
pub(super) fn has_finders_correct(matrix: &BitMatrix) -> bool {
    let dim = matrix.width();
    if dim < 21 || matrix.height() < 21 {
        return false;
    }

    let finder_checks: [(usize, usize, bool); 7] = [
        (0, 0, true),
        (6, 0, true),
        (0, 6, true),
        (6, 6, true),
        (3, 3, true),
        (1, 1, false),
        (2, 2, true),
    ];

    let origins = [(0, 0), (dim - 7, 0), (0, dim - 7)];

    let mut mismatches = 0;
    for &(ox, oy) in &origins {
        for &(dx, dy, expected) in &finder_checks {
            let x = ox + dx;
            let y = oy + dy;
            if x >= dim || y >= matrix.height() {
                return false;
            }
            if matrix.get(x, y) != expected {
                mismatches += 1;
            }
        }
    }

    mismatches <= 3
}

pub(super) fn candidate_orientations(matrix: &BitMatrix) -> Vec<BitMatrix> {
    let strict_tolerance = 3usize;
    let relaxed_tolerance = 7usize;
    let mut candidates = Vec::new();

    let r0 = matrix.clone();
    if has_finders_with_tolerance(&r0, strict_tolerance) {
        candidates.push(r0);
    }

    let r90 = rotate90(matrix);
    if has_finders_with_tolerance(&r90, strict_tolerance) {
        candidates.push(r90);
    }

    let r180 = rotate180(matrix);
    if has_finders_with_tolerance(&r180, strict_tolerance) {
        candidates.push(r180);
    }

    let r270 = rotate270(matrix);
    if has_finders_with_tolerance(&r270, strict_tolerance) {
        candidates.push(r270);
    }

    if !candidates.is_empty() {
        return candidates;
    }

    let fh = flip_horizontal(matrix);
    if has_finders_with_tolerance(&fh, relaxed_tolerance) {
        candidates.push(fh);
    }

    let fv = flip_vertical(matrix);
    if has_finders_with_tolerance(&fv, relaxed_tolerance) {
        candidates.push(fv);
    }

    let fhr90 = rotate90(&flip_horizontal(matrix));
    if has_finders_with_tolerance(&fhr90, relaxed_tolerance) {
        candidates.push(fhr90);
    }

    let fvr90 = rotate90(&flip_vertical(matrix));
    if has_finders_with_tolerance(&fvr90, relaxed_tolerance) {
        candidates.push(fvr90);
    }

    candidates
}

pub(super) fn candidate_orientations_relaxed(
    matrix: &BitMatrix,
    max_mismatches: usize,
) -> Vec<BitMatrix> {
    let mut candidates = Vec::new();
    let r0 = matrix.clone();
    if has_finders_with_tolerance(&r0, max_mismatches) {
        candidates.push(r0);
    }

    let r90 = rotate90(matrix);
    if has_finders_with_tolerance(&r90, max_mismatches) {
        candidates.push(r90);
    }

    let r180 = rotate180(matrix);
    if has_finders_with_tolerance(&r180, max_mismatches) {
        candidates.push(r180);
    }

    let r270 = rotate270(matrix);
    if has_finders_with_tolerance(&r270, max_mismatches) {
        candidates.push(r270);
    }

    let fh = flip_horizontal(matrix);
    if has_finders_with_tolerance(&fh, max_mismatches) {
        candidates.push(fh);
    }

    let fv = flip_vertical(matrix);
    if has_finders_with_tolerance(&fv, max_mismatches) {
        candidates.push(fv);
    }

    candidates
}

pub(super) fn has_finders_with_tolerance(matrix: &BitMatrix, max_mismatches: usize) -> bool {
    let dim = matrix.width();
    if dim < 21 || matrix.height() < 21 {
        return false;
    }

    let finder_checks: [(usize, usize, bool); 7] = [
        (0, 0, true),
        (6, 0, true),
        (0, 6, true),
        (6, 6, true),
        (3, 3, true),
        (1, 1, false),
        (2, 2, true),
    ];

    let origins = [(0, 0), (dim - 7, 0), (0, dim - 7)];

    let mut mismatches = 0usize;
    for &(ox, oy) in &origins {
        for &(dx, dy, expected) in &finder_checks {
            let x = ox + dx;
            let y = oy + dy;
            if x >= dim || y >= matrix.height() {
                return false;
            }
            if matrix.get(x, y) != expected {
                mismatches += 1;
            }
        }
    }

    mismatches <= max_mismatches
}

pub(super) fn validate_timing_patterns(matrix: &BitMatrix) -> bool {
    let dim = matrix.width();
    if dim < 21 || matrix.height() != dim {
        return false;
    }

    let horizontal = read_timing_pattern(
        matrix,
        &Point::new(8.0, 6.0),
        &Point::new((dim - 9) as f32, 6.0),
    );
    let vertical = read_timing_pattern(
        matrix,
        &Point::new(6.0, 8.0),
        &Point::new(6.0, (dim - 9) as f32),
    );

    let (Some(h_bits), Some(v_bits)) = (horizontal, vertical) else {
        return false;
    };

    alternation_ratio(&h_bits) >= 0.60 && alternation_ratio(&v_bits) >= 0.60
}

/// Validate the fixed Model 2 patterns that are independent of payload and
/// error-correction data. `max_mismatches_per_pattern` is zero on the normal
/// specification path and bounded only for image-recovery candidates.
pub(super) fn validate_structural_patterns(
    matrix: &BitMatrix,
    max_mismatches_per_pattern: usize,
) -> bool {
    let dim = matrix.width();
    if dim < 21 || matrix.height() != dim || !matrix.get(8, dim - 8) {
        return false;
    }

    for &(origin_x, origin_y) in &[(0, 0), (dim - 7, 0), (0, dim - 7)] {
        let mismatches = (0..7)
            .flat_map(|y| (0..7).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let expected = x == 0
                    || x == 6
                    || y == 0
                    || y == 6
                    || ((2..=4).contains(&x) && (2..=4).contains(&y));
                matrix.get(origin_x + x, origin_y + y) != expected
            })
            .count();
        if mismatches > max_mismatches_per_pattern {
            return false;
        }
    }

    let timing_mismatches = (8..(dim - 8))
        .filter(|&i| {
            let expected = i % 2 == 0;
            matrix.get(i, 6) != expected || matrix.get(6, i) != expected
        })
        .count();
    if timing_mismatches > max_mismatches_per_pattern {
        return false;
    }

    let centers =
        crate::decoder::function_mask::alignment_pattern_positions(((dim - 17) / 4) as u8);
    for &center_x in &centers {
        for &center_y in &centers {
            let overlaps_finder = (center_x <= 8 || center_x >= dim - 9) && center_y <= 8
                || center_x <= 8 && center_y >= dim - 9;
            if overlaps_finder {
                continue;
            }
            let mismatches = (0..5)
                .flat_map(|y| (0..5).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let expected = x == 0 || x == 4 || y == 0 || y == 4 || (x == 2 && y == 2);
                    matrix.get(center_x + x - 2, center_y + y - 2) != expected
                })
                .count();
            if mismatches > max_mismatches_per_pattern {
                return false;
            }
        }
    }

    true
}

/// Return a stable measure of disagreement with the fixed Model 2 patterns.
///
/// This is deliberately separate from [`validate_structural_patterns`]: the
/// latter is the acceptance gate, whereas recovery uses this score only to
/// decide which already-acceptable hypothesis deserves an attempt first.  A
/// damaged dark module is a hard structural failure and therefore ranks after
/// every candidate with a valid dark module.
pub(super) fn structural_mismatch_score(matrix: &BitMatrix) -> usize {
    let dim = matrix.width();
    if dim < 21 || matrix.height() != dim {
        return usize::MAX;
    }

    let mut mismatches = if matrix.get(8, dim - 8) { 0 } else { 1_000 };
    for &(origin_x, origin_y) in &[(0, 0), (dim - 7, 0), (0, dim - 7)] {
        mismatches += (0..7)
            .flat_map(|y| (0..7).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let expected = x == 0
                    || x == 6
                    || y == 0
                    || y == 6
                    || ((2..=4).contains(&x) && (2..=4).contains(&y));
                matrix.get(origin_x + x, origin_y + y) != expected
            })
            .count();
    }
    mismatches += (8..(dim - 8))
        .filter(|&i| {
            let expected = i % 2 == 0;
            matrix.get(i, 6) != expected || matrix.get(6, i) != expected
        })
        .count();

    let centers =
        crate::decoder::function_mask::alignment_pattern_positions(((dim - 17) / 4) as u8);
    for &center_x in &centers {
        for &center_y in &centers {
            let overlaps_finder = (center_x <= 8 || center_x >= dim - 9) && center_y <= 8
                || center_x <= 8 && center_y >= dim - 9;
            if overlaps_finder {
                continue;
            }
            mismatches += (0..5)
                .flat_map(|y| (0..5).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let expected = x == 0 || x == 4 || y == 0 || y == 4 || (x == 2 && y == 2);
                    matrix.get(center_x + x - 2, center_y + y - 2) != expected
                })
                .count();
        }
    }
    mismatches
}

pub(super) fn version_matches_candidate(matrix: &BitMatrix, version_num: u8) -> bool {
    if matrix.width() < 45 {
        return true;
    }

    match VersionInfo::extract(matrix) {
        Some(exact_version) => exact_version == version_num,
        None => true,
    }
}

pub(super) fn alternation_ratio(bits: &[bool]) -> f32 {
    if bits.len() < 2 {
        return 0.0;
    }

    let transitions = bits.windows(2).filter(|w| w[0] != w[1]).count();
    transitions as f32 / (bits.len() - 1) as f32
}

#[cfg(test)]
mod structural_tests {
    use super::*;

    fn structural_version_two_matrix() -> BitMatrix {
        let dim = 25;
        let mut matrix = BitMatrix::new(dim, dim);
        for &(origin_x, origin_y) in &[(0, 0), (dim - 7, 0), (0, dim - 7)] {
            for y in 0..7 {
                for x in 0..7 {
                    let black = x == 0
                        || x == 6
                        || y == 0
                        || y == 6
                        || ((2..=4).contains(&x) && (2..=4).contains(&y));
                    matrix.set(origin_x + x, origin_y + y, black);
                }
            }
        }
        for i in 8..(dim - 8) {
            let black = i % 2 == 0;
            matrix.set(i, 6, black);
            matrix.set(6, i, black);
        }
        matrix.set(8, dim - 8, true);
        for y in 0..5 {
            for x in 0..5 {
                let black = x == 0 || x == 4 || y == 0 || y == 4 || (x == 2 && y == 2);
                matrix.set(18 + x - 2, 18 + y - 2, black);
            }
        }
        matrix
    }

    #[test]
    fn structural_validation_rejects_function_pattern_damage() {
        let mut matrix = structural_version_two_matrix();
        assert!(validate_structural_patterns(&matrix, 0));

        matrix.set(8, 6, false);
        assert!(!validate_structural_patterns(&matrix, 0));
        assert!(validate_structural_patterns(&matrix, 1));

        matrix.set(0, 0, false);
        matrix.set(6, 0, false);
        assert!(!validate_structural_patterns(&matrix, 1));

        let mut dark_module_damaged = structural_version_two_matrix();
        dark_module_damaged.set(8, 17, false);
        assert!(!validate_structural_patterns(&dark_module_damaged, 3));
    }

    #[test]
    fn structural_score_ranks_less_damaged_matrix_first() {
        let clean = structural_version_two_matrix();
        let mut damaged = clean.clone();
        damaged.set(0, 0, false);
        damaged.set(6, 0, false);

        assert_eq!(structural_mismatch_score(&clean), 0);
        assert!(structural_mismatch_score(&clean) < structural_mismatch_score(&damaged));
    }
}
