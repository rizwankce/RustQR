/// Finder pattern detection using 1:1:3:1:1 ratio scanning
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

        // Scan every row
        for y in 0..height {
            let row_candidates = Self::scan_row(matrix, y, width);
            candidates.extend(row_candidates);
        }

        Self::merge_candidates(candidates)
    }

    fn scan_row(matrix: &BitMatrix, y: usize, width: usize) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        let mut run_lengths: Vec<usize> = Vec::new();
        let mut run_colors: Vec<bool> = Vec::new();
        let mut run_start = 0usize;
        let mut current_color = matrix.get(0, y);

        for x in 1..width {
            let color = matrix.get(x, y);

            if color != current_color {
                // Save completed run
                let run_len = x - run_start;
                run_lengths.push(run_len);
                run_colors.push(current_color);

                run_start = x;
                current_color = color;

                // Check if we have enough runs for a pattern (need at least 5 ending in black)
                if run_colors.len() >= 5 {
                    // Check the last 5 runs
                    let end_idx = run_colors.len();
                    let colors = &run_colors[end_idx - 5..end_idx];
                    let lengths = &run_lengths[end_idx - 5..end_idx];

                    // Pattern should be: black-white-black-white-black
                    if colors[0] && !colors[1] && colors[2] && !colors[3] && colors[4] {
                        if let Some(pattern) = Self::check_pattern(lengths, x, y) {
                            candidates.push(pattern);
                        }
                    }
                }
            }
        }

        candidates
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
            for idx in 0..merged.len() {
                let existing = &merged[idx];
                let dx = candidate.center.x - existing.center.x;
                let dy = candidate.center.y - existing.center.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq < MERGE_DIST * MERGE_DIST {
                    let new_x = (existing.center.x + candidate.center.x) / 2.0;
                    let new_y = (existing.center.y + candidate.center.y) / 2.0;
                    let new_module = (existing.module_size + candidate.module_size) / 2.0;
                    merged[idx] = FinderPattern::new(new_x, new_y, new_module);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_line_pattern() {
        let mut matrix = BitMatrix::new(25, 10);
        let y = 5;
        let unit = 3;
        let x_start = 2;

        // Black(3) - White(3) - Black(9) - White(3) - Black(3)
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
}
