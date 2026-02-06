/// Finder pattern detection using 1:1:3:1:1 ratio scanning with early termination optimizations
use crate::detector::connected_components::find_black_regions;
use crate::detector::pyramid::ImagePyramid;
use crate::models::{BitMatrix, Point};

#[derive(Debug, Clone)]
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

        // Scan every column for vertically-oriented finder patterns (rotated QR codes)
        for x in 0..width {
            if !Self::has_significant_edges_column(matrix, x, height) {
                continue;
            }
            let col_candidates = Self::scan_column(matrix, x, height);
            candidates.extend(col_candidates);
        }

        Self::merge_candidates(candidates)
    }

    /// Detect finder patterns using parallel processing
    /// Processes rows and columns in parallel for multi-core speedup
    pub fn detect_parallel(matrix: &BitMatrix) -> Vec<FinderPattern> {
        use rayon::prelude::*;

        let width = matrix.width();
        let height = matrix.height();

        // Collect candidates from all rows in parallel
        let all_row_candidates: Vec<Vec<FinderPattern>> = (0..height)
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

        // Collect candidates from all columns in parallel
        let all_col_candidates: Vec<Vec<FinderPattern>> = (0..width)
            .into_par_iter()
            .filter_map(|x| {
                if !Self::has_significant_edges_column(matrix, x, height) {
                    return None;
                }

                let col_candidates = Self::scan_column(matrix, x, height);
                if col_candidates.is_empty() {
                    None
                } else {
                    Some(col_candidates)
                }
            })
            .collect();

        // Flatten all candidates
        let mut candidates = Vec::new();
        for row_candidates in all_row_candidates {
            candidates.extend(row_candidates);
        }
        for col_candidates in all_col_candidates {
            candidates.extend(col_candidates);
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

        // Also scan columns at coarse level for rotated QR codes
        for x in 0..coarse_width {
            if !Self::has_significant_edges_column(coarse_level, x, coarse_height) {
                continue;
            }
            let col_candidates = Self::scan_column(coarse_level, x, coarse_height);
            coarse_candidates.extend(col_candidates);
        }

        // If no candidates found at coarse level, fall back to full detection
        if coarse_candidates.is_empty() {
            return Self::detect(matrix);
        }

        // Refine detection around coarse candidates at full resolution
        let mut refined_candidates = Vec::new();
        const WINDOW_SIZE: usize = 10; // Search window in original pixels

        for coarse_pattern in coarse_candidates {
            // Get search window bounds
            let (min_x, min_y, max_x, max_y) = pyramid.get_search_window(
                coarse_pattern.center.x as usize,
                coarse_pattern.center.y as usize,
                scale,
                WINDOW_SIZE,
            );

            // Convert coarse module size to original scale for validation
            let expected_module = coarse_pattern.module_size * scale;

            // Scan rows in the window area at full resolution
            for y in min_y..=max_y {
                if !Self::has_significant_edges(matrix, y, width) {
                    continue;
                }

                let row_candidates = Self::scan_row_in_range(matrix, y, width, min_x, max_x);

                for candidate in row_candidates {
                    let size_ratio = candidate.module_size / expected_module;
                    if (0.5..=2.0).contains(&size_ratio) {
                        refined_candidates.push(candidate);
                    }
                }
            }

            // Scan columns in the window area at full resolution
            for x in min_x..=max_x {
                if !Self::has_significant_edges_column(matrix, x, height) {
                    continue;
                }

                let col_candidates = Self::scan_column_in_range(matrix, x, height, min_y, max_y);

                for candidate in col_candidates {
                    let size_ratio = candidate.module_size / expected_module;
                    if (0.5..=2.0).contains(&size_ratio) {
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
                            if let Some((center_x, _unit, total)) = Self::check_pattern(lengths, x)
                            {
                                if let Some((center_y, unit_v)) =
                                    Self::cross_check_vertical(matrix, center_x, y, total)
                                {
                                    if let Some((refined_x, unit_h)) = Self::cross_check_horizontal(
                                        matrix, center_x, center_y, total,
                                    ) {
                                        let module_size = (unit_h + unit_v) / 2.0;
                                        candidates.push(FinderPattern::new(
                                            refined_x,
                                            center_y,
                                            module_size,
                                        ));
                                    } else {
                                        candidates
                                            .push(FinderPattern::new(center_x, center_y, unit_v));
                                    }
                                }

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
                            if let Some((center_x, _unit, total)) = Self::check_pattern(lengths, x)
                            {
                                if let Some((center_y, unit_v)) =
                                    Self::cross_check_vertical(matrix, center_x, y, total)
                                {
                                    if let Some((refined_x, unit_h)) = Self::cross_check_horizontal(
                                        matrix, center_x, center_y, total,
                                    ) {
                                        let module_size = (unit_h + unit_v) / 2.0;
                                        candidates.push(FinderPattern::new(
                                            refined_x,
                                            center_y,
                                            module_size,
                                        ));
                                    } else {
                                        candidates
                                            .push(FinderPattern::new(center_x, center_y, unit_v));
                                    }
                                }

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
    /// Returns true if the pattern passes basic ratio checks
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

        // Minimum size: allow very small QR codes
        // 7 modules at ~1 pixel each = 7 pixels absolute minimum
        if total < 7 {
            if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                eprintln!("FINDER: Rejected - total {} < 7", total);
            }
            return false;
        }

        // Maximum size check: prevent detecting huge patterns that aren't QRs
        // Large QR codes in high-res images can have finder patterns up to ~1500px
        if total > 2000 {
            if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                eprintln!("FINDER: Rejected - total {} > 2000", total);
            }
            return false;
        }

        // Individual run checks: each run should be at least 2 pixels
        // Small QR codes in low-res images can have 2-3px modules
        if b1 < 2 || w1 < 2 || b2 < 2 || w2 < 2 || b3 < 2 {
            return false;
        }

        // Check if center black is significantly larger than outer blacks
        // b2 should be roughly 1.5-5x larger than b1 and b3 (relaxed for small patterns)
        let b2_min = b1.min(b3);
        if b2 < b2_min * 3 / 2 || b2 > b2_min * 5 {
            if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                eprintln!(
                    "FINDER: Rejected - b2 {} not 1.5-5x of min {} (ratio={:.1})",
                    b2,
                    b2_min,
                    b2 as f32 / b2_min as f32
                );
            }
            return false;
        }

        // Check whites are roughly equal and similar to outer blacks
        let outer_avg = (b1 + b3 + w1 + w2) / 4;
        let w1_ok = w1 >= outer_avg / 2 && w1 <= outer_avg * 2;
        let w2_ok = w2 >= outer_avg / 2 && w2 <= outer_avg * 2;

        if !w1_ok || !w2_ok {
            if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                eprintln!(
                    "FINDER: Rejected - whites not balanced: w1={}, w2={}, outer_avg={}",
                    w1, w2, outer_avg
                );
            }
            return false;
        }

        true
    }

    fn check_pattern(lengths: &[usize], end_x: usize) -> Option<(f32, f32, usize)> {
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
            return Some((center_x, unit, total as usize));
        }

        None
    }

    fn cross_check_vertical(
        matrix: &BitMatrix,
        center_x: f32,
        center_y: usize,
        total: usize,
    ) -> Option<(f32, f32)> {
        let x = center_x.round() as isize;
        if x < 0 || (x as usize) >= matrix.width() {
            return None;
        }

        let height = matrix.height() as isize;
        let mut counts = [0usize; 5];

        // Count up from center (black -> white -> black)
        let mut y = center_y as isize;
        while y >= 0 && matrix.get(x as usize, y as usize) {
            counts[2] += 1;
            y -= 1;
        }
        if y < 0 {
            return None;
        }
        while y >= 0 && !matrix.get(x as usize, y as usize) {
            counts[1] += 1;
            y -= 1;
        }
        if y < 0 {
            return None;
        }
        while y >= 0 && matrix.get(x as usize, y as usize) {
            counts[0] += 1;
            y -= 1;
        }

        // Count down from center (black -> white -> black)
        y = center_y as isize + 1;
        while y < height && matrix.get(x as usize, y as usize) {
            counts[2] += 1;
            y += 1;
        }
        while y < height && !matrix.get(x as usize, y as usize) {
            counts[3] += 1;
            y += 1;
        }
        while y < height && matrix.get(x as usize, y as usize) {
            counts[4] += 1;
            y += 1;
        }

        if counts.iter().any(|&c| c == 0) {
            return None;
        }

        let total_v: usize = counts.iter().sum();
        if total_v < 7 {
            return None;
        }
        if total > 0 {
            let diff = if total_v > total {
                total_v - total
            } else {
                total - total_v
            };
            if diff > total {
                return None;
            }
        }

        let unit = total_v as f32 / 7.0;
        let r1 = counts[0] as f32 / unit;
        let r2 = counts[1] as f32 / unit;
        let r3 = counts[2] as f32 / unit;
        let r4 = counts[3] as f32 / unit;
        let r5 = counts[4] as f32 / unit;

        const TOL: f32 = 0.7;
        if (r1 - 1.0).abs() > TOL
            || (r2 - 1.0).abs() > TOL
            || (r3 - 3.0).abs() > TOL
            || (r4 - 1.0).abs() > TOL
            || (r5 - 1.0).abs() > TOL
        {
            return None;
        }

        let center = y as f32 - counts[4] as f32 - counts[3] as f32 - (counts[2] as f32 / 2.0);
        Some((center, unit))
    }

    fn cross_check_horizontal(
        matrix: &BitMatrix,
        center_x: f32,
        center_y: f32,
        total: usize,
    ) -> Option<(f32, f32)> {
        let y = center_y.round() as isize;
        if y < 0 || (y as usize) >= matrix.height() {
            return None;
        }

        let width = matrix.width() as isize;
        let mut counts = [0usize; 5];

        // Count left from center (black -> white -> black)
        let mut x = center_x.round() as isize;
        while x >= 0 && matrix.get(x as usize, y as usize) {
            counts[2] += 1;
            x -= 1;
        }
        if x < 0 {
            return None;
        }
        while x >= 0 && !matrix.get(x as usize, y as usize) {
            counts[1] += 1;
            x -= 1;
        }
        if x < 0 {
            return None;
        }
        while x >= 0 && matrix.get(x as usize, y as usize) {
            counts[0] += 1;
            x -= 1;
        }

        // Count right from center (black -> white -> black)
        x = center_x.round() as isize + 1;
        while x < width && matrix.get(x as usize, y as usize) {
            counts[2] += 1;
            x += 1;
        }
        while x < width && !matrix.get(x as usize, y as usize) {
            counts[3] += 1;
            x += 1;
        }
        while x < width && matrix.get(x as usize, y as usize) {
            counts[4] += 1;
            x += 1;
        }

        if counts.iter().any(|&c| c == 0) {
            return None;
        }

        let total_h: usize = counts.iter().sum();
        if total_h < 7 {
            return None;
        }
        if total > 0 {
            let diff = if total_h > total {
                total_h - total
            } else {
                total - total_h
            };
            if diff > total {
                return None;
            }
        }

        let unit = total_h as f32 / 7.0;
        let r1 = counts[0] as f32 / unit;
        let r2 = counts[1] as f32 / unit;
        let r3 = counts[2] as f32 / unit;
        let r4 = counts[3] as f32 / unit;
        let r5 = counts[4] as f32 / unit;

        const TOL: f32 = 0.7;
        if (r1 - 1.0).abs() > TOL
            || (r2 - 1.0).abs() > TOL
            || (r3 - 3.0).abs() > TOL
            || (r4 - 1.0).abs() > TOL
            || (r5 - 1.0).abs() > TOL
        {
            return None;
        }

        let center = x as f32 - counts[4] as f32 - counts[3] as f32 - (counts[2] as f32 / 2.0);
        Some((center, unit))
    }

    /// Check if column has enough edge transitions to potentially contain patterns
    fn has_significant_edges_column(matrix: &BitMatrix, x: usize, height: usize) -> bool {
        if height == 0 {
            return false;
        }

        let mut transitions = 0;
        let sample_step = 4;
        let mut prev_color = matrix.get(x, 0);

        for y in (sample_step..height).step_by(sample_step) {
            let color = matrix.get(x, y);
            if color != prev_color {
                transitions += 1;
                prev_color = color;

                if transitions >= 3 {
                    return true;
                }
            }
        }

        transitions >= 2
    }

    fn scan_column(matrix: &BitMatrix, x: usize, height: usize) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        if height == 0 {
            return candidates;
        }

        let mut run_lengths: Vec<usize> = Vec::new();
        let mut run_colors: Vec<bool> = Vec::new();
        let mut run_start = 0usize;
        let mut current_color = matrix.get(x, 0);

        const MAX_PATTERNS_PER_COL: usize = 5;

        for y in 1..height {
            let color = matrix.get(x, y);

            if color != current_color {
                let run_len = y - run_start;
                run_lengths.push(run_len);
                run_colors.push(current_color);

                run_start = y;
                current_color = color;

                if run_colors.len() >= 5 {
                    let end_idx = run_colors.len();
                    let colors = &run_colors[end_idx - 5..end_idx];
                    let lengths = &run_lengths[end_idx - 5..end_idx];

                    // Pattern should be: black-white-black-white-black
                    if colors[0]
                        && !colors[1]
                        && colors[2]
                        && !colors[3]
                        && colors[4]
                        && Self::quick_ratio_check(lengths)
                    {
                        if let Some((center_y, _unit, total)) = Self::check_pattern(lengths, y) {
                            // Cross-check horizontally first (primary axis is vertical)
                            if let Some((center_x, unit_h)) =
                                Self::cross_check_horizontal(matrix, x as f32, center_y, total)
                            {
                                // Then refine vertically
                                if let Some((refined_y, unit_v)) = Self::cross_check_vertical(
                                    matrix,
                                    center_x,
                                    center_y.round() as usize,
                                    total,
                                ) {
                                    let module_size = (unit_h + unit_v) / 2.0;
                                    candidates.push(FinderPattern::new(
                                        center_x,
                                        refined_y,
                                        module_size,
                                    ));
                                } else {
                                    candidates.push(FinderPattern::new(center_x, center_y, unit_h));
                                }
                            }

                            if candidates.len() >= MAX_PATTERNS_PER_COL {
                                break;
                            }
                        }
                    }
                }
            }
        }

        candidates
    }

    fn scan_column_in_range(
        matrix: &BitMatrix,
        x: usize,
        height: usize,
        min_y: usize,
        max_y: usize,
    ) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        if height == 0 {
            return candidates;
        }

        let mut run_lengths: Vec<usize> = Vec::new();
        let mut run_colors: Vec<bool> = Vec::new();

        let start_y = min_y.min(height - 1);
        let end_y = max_y.min(height - 1);

        if start_y >= end_y {
            return candidates;
        }

        let mut run_start = start_y;
        let mut current_color = matrix.get(x, start_y);

        const MAX_PATTERNS_PER_COL: usize = 5;

        for y in (start_y + 1)..=end_y {
            let color = matrix.get(x, y);

            if color != current_color {
                let run_len = y - run_start;
                run_lengths.push(run_len);
                run_colors.push(current_color);

                run_start = y;
                current_color = color;

                if run_colors.len() >= 5 {
                    let end_idx = run_colors.len();
                    let colors = &run_colors[end_idx - 5..end_idx];
                    let lengths = &run_lengths[end_idx - 5..end_idx];

                    if colors[0]
                        && !colors[1]
                        && colors[2]
                        && !colors[3]
                        && colors[4]
                        && Self::quick_ratio_check(lengths)
                    {
                        if let Some((center_y, _unit, total)) = Self::check_pattern(lengths, y) {
                            if let Some((center_x, unit_h)) =
                                Self::cross_check_horizontal(matrix, x as f32, center_y, total)
                            {
                                if let Some((refined_y, unit_v)) = Self::cross_check_vertical(
                                    matrix,
                                    center_x,
                                    center_y.round() as usize,
                                    total,
                                ) {
                                    let module_size = (unit_h + unit_v) / 2.0;
                                    candidates.push(FinderPattern::new(
                                        center_x,
                                        refined_y,
                                        module_size,
                                    ));
                                } else {
                                    candidates.push(FinderPattern::new(center_x, center_y, unit_h));
                                }
                            }

                            if candidates.len() >= MAX_PATTERNS_PER_COL {
                                break;
                            }
                        }
                    }
                }
            }
        }

        candidates
    }

    fn merge_candidates(candidates: Vec<FinderPattern>) -> Vec<FinderPattern> {
        let mut merged: Vec<FinderPattern> = Vec::new();

        for candidate in candidates {
            let mut found = false;
            for existing in &mut merged {
                let dx = candidate.center.x - existing.center.x;
                let dy = candidate.center.y - existing.center.y;
                let dist_sq = dx * dx + dy * dy;
                let merge_dist = (existing.module_size + candidate.module_size) * 2.5;
                let merge_dist_sq = merge_dist * merge_dist;

                if dist_sq < merge_dist_sq {
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
            if !(0.5..=2.0).contains(&aspect_ratio) {
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
        let mut matrix = BitMatrix::new(50, 50);
        let u = 3;
        let start = 5;

        for my in 0..7 {
            for mx in 0..7 {
                let is_border = mx == 0 || mx == 6 || my == 0 || my == 6;
                let is_center = (2..=4).contains(&mx) && (2..=4).contains(&my);
                if is_border || is_center {
                    for y in start + my * u..start + (my + 1) * u {
                        for x in start + mx * u..start + (mx + 1) * u {
                            matrix.set(x, y, true);
                        }
                    }
                }
            }
        }

        let patterns = FinderDetector::detect(&matrix);
        assert!(!patterns.is_empty(), "Should detect pattern");

        let expected_center = start as f32 + 3.5 * u as f32;
        let found = patterns.iter().any(|p| {
            (p.center.x - expected_center).abs() < u as f32
                && (p.center.y - expected_center).abs() < u as f32
        });
        assert!(
            found,
            "Expected pattern near ({}, {}), found: {:?}",
            expected_center, expected_center, patterns
        );
    }

    #[test]
    fn test_quick_ratio_check() {
        let valid = vec![6, 6, 18, 6, 6];
        assert!(FinderDetector::quick_ratio_check(&valid));

        let bad_small_center = vec![2, 2, 2, 2, 2];
        assert!(!FinderDetector::quick_ratio_check(&bad_small_center));

        let bad_whites = vec![4, 1, 12, 8, 4];
        assert!(!FinderDetector::quick_ratio_check(&bad_whites));

        let bad_center = vec![6, 6, 6, 6, 6];
        assert!(!FinderDetector::quick_ratio_check(&bad_center));
    }

    #[test]
    fn test_detect_zero_height_matrix() {
        let matrix = BitMatrix::new(8, 0);
        let patterns = FinderDetector::detect(&matrix);
        assert!(patterns.is_empty());
    }
}
