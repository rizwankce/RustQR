/// Finder pattern detection using 1:1:3:1:1 ratio scanning with early termination optimizations
use crate::detector::connected_components::find_black_regions;
use crate::detector::pyramid::ImagePyramid;
use crate::models::{BitMatrix, Point};

pub struct FinderPattern {
    pub center: Point,
    pub module_size: f32,
}

impl FinderPattern {
    pub fn new(x: f32, y: f32, module_size: f32) -> Self {
        Self {
            center: Point::new(x, y),
            module_size,
        }
    }
}

pub struct FinderDetector;

impl FinderDetector {
    pub fn detect(matrix: &BitMatrix) -> Vec<FinderPattern> {
        let width = matrix.width();
        let height = matrix.height();
        let mut candidates = Vec::new();

        // Scan every row - edge detection provides the speedup
        let row_step = 1;
        for y in (0..height).step_by(row_step) {
            // Early termination 1: Skip rows with low variance (no edges)
            if !Self::has_significant_edges(matrix, y, width) {
                continue;
            }

            let row_candidates = Self::scan_row(matrix, y, width);
            candidates.extend(row_candidates);
        }

        Self::merge_candidates(candidates)
    }

    /// Detect finder patterns using parallel processing
    /// Processes rows in parallel for multi-core speedup
    pub fn detect_parallel(matrix: &BitMatrix) -> Vec<FinderPattern> {
        use rayon::prelude::*;

        let width = matrix.width();
        let height = matrix.height();

        // Collect candidates from all rows in parallel
        let all_candidates: Vec<Vec<FinderPattern>> = (0..height)
            .into_par_iter()
            .filter_map(|y| {
                // Early termination: Skip rows with low variance
                if !Self::has_significant_edges(matrix, y, width) {
                    return None;
                }

                let row_candidates = Self::scan_row(matrix, y, width);
                if row_candidates.is_empty() {
                    None
                } else {
                    Some(row_candidates)
                }
            })
            .collect();

        // Flatten all candidates
        let mut candidates = Vec::new();
        for row_candidates in all_candidates {
            candidates.extend(row_candidates);
        }

        Self::merge_candidates(candidates)
    }

    /// Detect finder patterns using multi-scale pyramid approach
    /// For large images, this is 3-5x faster than single-scale detection
    pub fn detect_with_pyramid(matrix: &BitMatrix) -> Vec<FinderPattern> {
        let width = matrix.width();
        let height = matrix.height();

        // For small images, use regular detection
        if width < 400 || height < 400 {
            return Self::detect(matrix);
        }

        // Create image pyramid
        let pyramid = ImagePyramid::new(matrix.clone());

        // Start with coarsest level for initial detection
        let (coarse_level, scale) = pyramid.coarsest_detection_level();
        let mut coarse_candidates = Vec::new();

        // Detect on coarse level
        let coarse_width = coarse_level.width();
        let coarse_height = coarse_level.height();

        for y in (0..coarse_height).step_by(1) {
            if !Self::has_significant_edges(coarse_level, y, coarse_width) {
                continue;
            }
            let row_candidates = Self::scan_row(coarse_level, y, coarse_width);
            coarse_candidates.extend(row_candidates);
        }

        // If no candidates found at coarse level, fall back to full detection
        if coarse_candidates.is_empty() {
            return Self::detect(matrix);
        }

        // Refine detection around coarse candidates at full resolution
        let mut refined_candidates = Vec::new();
        const WINDOW_SIZE: usize = 10; // Search window in original pixels

        for coarse_pattern in coarse_candidates {
            // Map coarse coordinates back to original
            let orig_x = (coarse_pattern.center.x * scale) as usize;
            let orig_y = (coarse_pattern.center.y * scale) as usize;

            // Get search window bounds
            let (min_x, min_y, max_x, max_y) = pyramid.get_search_window(
                coarse_pattern.center.x as usize,
                coarse_pattern.center.y as usize,
                scale,
                WINDOW_SIZE,
            );

            // Scan only the window area at full resolution
            for y in min_y..=max_y {
                if !Self::has_significant_edges(matrix, y, width) {
                    continue;
                }

                // Only check patterns near the coarse location
                let row_candidates = Self::scan_row_in_range(matrix, y, width, min_x, max_x);

                // Convert coarse module size to original scale for validation
                let expected_module = coarse_pattern.module_size * scale;

                for mut candidate in row_candidates {
                    // Validate module size matches expectation from coarse detection
                    let size_ratio = candidate.module_size / expected_module;
                    if size_ratio >= 0.5 && size_ratio <= 2.0 {
                        // Module size is consistent, keep this candidate
                        refined_candidates.push(candidate);
                    }
                }
            }
        }

        // If refinement found candidates, use them; otherwise fall back
        if !refined_candidates.is_empty() {
            Self::merge_candidates(refined_candidates)
        } else {
            // Fallback to full detection if refinement failed
            Self::detect(matrix)
        }
    }

    /// Check if row has enough edge transitions to potentially contain patterns
    fn has_significant_edges(matrix: &BitMatrix, y: usize, width: usize) -> bool {
        // Sample every 4th pixel to check for edges quickly
        let mut transitions = 0;
        let sample_step = 4;
        let mut prev_color = matrix.get(0, y);

        for x in (sample_step..width).step_by(sample_step) {
            let color = matrix.get(x, y);
            if color != prev_color {
                transitions += 1;
                prev_color = color;

                // Early termination: If we found enough transitions, row has edges
                if transitions >= 3 {
                    return true;
                }
            }
        }

        // Require at least 2 transitions for a row to be considered
        transitions >= 2
    }

    fn scan_row(matrix: &BitMatrix, y: usize, width: usize) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        let mut run_lengths: Vec<usize> = Vec::new();
        let mut run_colors: Vec<bool> = Vec::new();
        let mut run_start = 0usize;
        let mut current_color = matrix.get(0, y);

        // Early termination 2: Max patterns per row
        const MAX_PATTERNS_PER_ROW: usize = 5;

        for x in 1..width {
            let color = matrix.get(x, y);

            if color != current_color {
                // Save completed run
                let run_len = x - run_start;
                run_lengths.push(run_len);
                run_colors.push(current_color);

                run_start = x;
                current_color = color;

                // Check if we have enough runs for a pattern
                if run_colors.len() >= 5 {
                    let end_idx = run_colors.len();
                    let colors = &run_colors[end_idx - 5..end_idx];
                    let lengths = &run_lengths[end_idx - 5..end_idx];

                    // Pattern should be: black-white-black-white-black
                    if colors[0] && !colors[1] && colors[2] && !colors[3] && colors[4] {
                        // Early termination 3: Quick ratio check before full validation
                        if Self::quick_ratio_check(lengths) {
                            if let Some(pattern) = Self::check_pattern(lengths, x, y) {
                                candidates.push(pattern);

                                // Early termination 4: Stop after finding enough patterns
                                if candidates.len() >= MAX_PATTERNS_PER_ROW {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        candidates
    }

    fn scan_row_in_range(
        matrix: &BitMatrix,
        y: usize,
        width: usize,
        min_x: usize,
        max_x: usize,
    ) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        let mut run_lengths: Vec<usize> = Vec::new();
        let mut run_colors: Vec<bool> = Vec::new();

        // Clamp the range to valid row bounds
        let start_x = min_x.min(width - 1);
        let end_x = max_x.min(width - 1);

        // Need at least 2 pixels to detect runs
        if start_x >= end_x {
            return candidates;
        }

        let mut run_start = start_x;
        let mut current_color = matrix.get(start_x, y);

        // Early termination: Max patterns per row
        const MAX_PATTERNS_PER_ROW: usize = 5;

        for x in (start_x + 1)..=end_x {
            let color = matrix.get(x, y);

            if color != current_color {
                // Save completed run
                let run_len = x - run_start;
                run_lengths.push(run_len);
                run_colors.push(current_color);

                run_start = x;
                current_color = color;

                // Check if we have enough runs for a pattern
                if run_colors.len() >= 5 {
                    let end_idx = run_colors.len();
                    let colors = &run_colors[end_idx - 5..end_idx];
                    let lengths = &run_lengths[end_idx - 5..end_idx];

                    // Pattern should be: black-white-black-white-black
                    if colors[0] && !colors[1] && colors[2] && !colors[3] && colors[4] {
                        // Quick ratio check before full validation
                        if Self::quick_ratio_check(lengths) {
                            if let Some(pattern) = Self::check_pattern(lengths, x, y) {
                                candidates.push(pattern);

                                // Early termination: Stop after finding enough patterns
                                if candidates.len() >= MAX_PATTERNS_PER_ROW {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        candidates
    }

    /// Quick ratio validation - rough check before expensive floating-point math
    fn quick_ratio_check(lengths: &[usize]) -> bool {
        let b1 = lengths[0];
        let w1 = lengths[1];
        let b2 = lengths[2];
        let w2 = lengths[3];
        let b3 = lengths[4];

        let total = b1 + w1 + b2 + w2 + b3;

        // Quick checks using integer arithmetic:
        // 1. Center black should be roughly 3x the outer blacks
        // 2. Whites should be roughly equal to outer blacks
        // 3. Minimum size check (avoid detecting tiny noise)

        // Increased minimum: 7 modules at ~6 pixels each = 42 pixels
        // This filters out text/shadow noise while keeping real QR patterns
        if total < 42 {
            return false;
        }

        // Maximum size check: prevent detecting huge patterns that aren't QRs
        // Version 40 QR at 100px per module would be ~700px total
        if total > 800 {
            return false;
        }

        // Individual run checks: each run should be at least 4 pixels
        // Real QR modules are never just 1-2 pixels wide
        if b1 < 4 || w1 < 4 || b2 < 4 || w2 < 4 || b3 < 4 {
            return false;
        }

        // Check if center black is significantly larger than outer blacks
        // b2 should be roughly 2-4x larger than b1 and b3
        let b2_min = b1.min(b3);
        if b2 < b2_min * 2 || b2 > b2_min * 5 {
            return false;
        }

        // Check whites are roughly equal and similar to outer blacks
        let outer_avg = (b1 + b3 + w1 + w2) / 4;
        let w1_ok = w1 >= outer_avg / 2 && w1 <= outer_avg * 2;
        let w2_ok = w2 >= outer_avg / 2 && w2 <= outer_avg * 2;

        w1_ok && w2_ok
    }

    fn check_pattern(lengths: &[usize], end_x: usize, y: usize) -> Option<FinderPattern> {
        if lengths.len() != 5 {
            return None;
        }

        let b1 = lengths[0];
        let w1 = lengths[1];
        let b2 = lengths[2];
        let w2 = lengths[3];
        let b3 = lengths[4];

        let total = (b1 + w1 + b2 + w2 + b3) as f32;
        let unit = total / 7.0;

        // Check ratios with tolerance
        let r1 = b1 as f32 / unit;
        let r2 = w1 as f32 / unit;
        let r3 = b2 as f32 / unit;
        let r4 = w2 as f32 / unit;
        let r5 = b3 as f32 / unit;

        const TOL: f32 = 0.5;
        if (r1 - 1.0).abs() <= TOL
            && (r2 - 1.0).abs() <= TOL
            && (r3 - 3.0).abs() <= TOL
            && (r4 - 1.0).abs() <= TOL
            && (r5 - 1.0).abs() <= TOL
        {
            let center_x = (end_x as f32) - (b3 as f32) - (w2 as f32) - (b2 as f32 / 2.0);
            return Some(FinderPattern::new(center_x, y as f32, unit));
        }

        None
    }

    fn merge_candidates(candidates: Vec<FinderPattern>) -> Vec<FinderPattern> {
        let mut merged: Vec<FinderPattern> = Vec::new();
        const MERGE_DIST: f32 = 5.0;

        for candidate in candidates {
            let mut found = false;
            for existing in &mut merged {
                let dx = candidate.center.x - existing.center.x;
                let dy = candidate.center.y - existing.center.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq < MERGE_DIST * MERGE_DIST {
                    let new_x = (existing.center.x + candidate.center.x) / 2.0;
                    let new_y = (existing.center.y + candidate.center.y) / 2.0;
                    let new_module = (existing.module_size + candidate.module_size) / 2.0;
                    *existing = FinderPattern::new(new_x, new_y, new_module);
                    found = true;
                    break;
                }
            }

            if !found {
                merged.push(candidate);
            }
        }

        merged
    }

    /// Detect finder patterns using connected components approach
    /// O(k) where k = number of black regions instead of O(n²)
    pub fn detect_with_connected_components(matrix: &BitMatrix) -> Vec<FinderPattern> {
        let width = matrix.width();
        let height = matrix.height();
        let mut candidates = Vec::new();

        // Find all black regions
        let regions = find_black_regions(matrix);

        // For each region, check if it could be a finder pattern center
        for (min_x, min_y, max_x, max_y) in regions {
            let region_width = max_x - min_x + 1;
            let region_height = max_y - min_y + 1;

            // Skip very small or very large regions
            if region_width < 5 || region_height < 5 {
                continue;
            }
            if region_width > width / 4 || region_height > height / 4 {
                continue;
            }

            // Check aspect ratio (finder patterns are roughly square)
            let aspect_ratio = region_width as f32 / region_height as f32;
            if aspect_ratio < 0.5 || aspect_ratio > 2.0 {
                continue;
            }

            // Scan this region for finder patterns
            let search_min_x = min_x.saturating_sub(5);
            let search_max_x = (max_x + 5).min(width - 1);

            for y in min_y..=max_y {
                if !Self::has_significant_edges(matrix, y, width) {
                    continue;
                }
                let row_candidates =
                    Self::scan_row_in_range(matrix, y, width, search_min_x, search_max_x);
                candidates.extend(row_candidates);
            }
        }

        Self::merge_candidates(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_line_pattern() {
        let mut matrix = BitMatrix::new(50, 20);
        let y = 10;
        let unit = 6; // Minimum 6 pixels per run (new minimum is 4, using 6 for margin)
        let x_start = 5;

        // Black(6) - White(6) - Black(18) - White(6) - Black(6)
        // Total = 42 pixels, meets new minimum
        for x in x_start..x_start + unit {
            matrix.set(x, y, true);
        }
        // x_start+unit to x_start+2*unit is white (default)
        for x in x_start + 2 * unit..x_start + 5 * unit {
            matrix.set(x, y, true);
        }
        // x_start+5*unit to x_start+6*unit is white (default)
        for x in x_start + 6 * unit..x_start + 7 * unit {
            matrix.set(x, y, true);
        }

        let patterns = FinderDetector::detect(&matrix);

        assert!(!patterns.is_empty(), "Should detect the pattern");

        let expected_center = x_start as f32 + 3.5 * unit as f32;
        let found = patterns
            .iter()
            .any(|p| (p.center.x - expected_center).abs() < 3.0);
        assert!(
            found,
            "Should find pattern near x={}, got centers: {:?}",
            expected_center,
            patterns.iter().map(|p| p.center.x).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_quick_ratio_check() {
        // Valid pattern: 6-6-18-6-6 (unit = 6, total = 42)
        // Meets new minimum: each run >= 4, total >= 42
        let valid = vec![6, 6, 18, 6, 6];
        assert!(FinderDetector::quick_ratio_check(&valid));

        // Too small (individual runs < 4)
        let small = vec![2, 2, 6, 2, 2];
        assert!(!FinderDetector::quick_ratio_check(&small));

        // Too small total (< 42)
        let small_total = vec![4, 4, 12, 4, 4]; // total = 28
        assert!(!FinderDetector::quick_ratio_check(&small_total));

        // Bad ratios - center not 3x
        let bad_center = vec![6, 6, 10, 6, 6];
        assert!(!FinderDetector::quick_ratio_check(&bad_center));
    }
}
