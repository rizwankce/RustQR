/// Finder pattern detection using 1:1:3:1:1 ratio scanning
use crate::models::{BitMatrix, Point};

pub struct FinderPattern {
    pub center: Point,
    pub module_size: f32,
}

impl FinderPattern {
    fn new(x: f32, y: f32, module_size: f32) -> Self {
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

        // Scan horizontally
        for y in 0..height {
            let row_candidates = Self::scan_horizontal(matrix, y, width);
            for c in row_candidates {
                candidates.push(c);
            }
        }

        // Merge nearby candidates
        Self::merge_candidates(candidates)
    }

    fn scan_horizontal(matrix: &BitMatrix, y: usize, width: usize) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        let mut run_start = 0usize;
        let mut current_run = 0usize;
        let mut run_lengths = [0usize; 5];
        let mut last_color = false;

        for x in 0..width {
            let is_black = matrix.get(x, y);

            if is_black != last_color {
                // Color changed - save current run
                if current_run < 5 {
                    run_lengths[current_run] = x - run_start;
                    current_run += 1;
                }
                run_start = x;
                last_color = is_black;

                // Check if we have 5 runs ending with black
                if current_run == 5 && is_black {
                    // Check 1:1:3:1:1 ratio
                    let total: f32 = run_lengths.iter().sum::<usize>() as f32;
                    let unit = total / 7.0;

                    let r1 = run_lengths[0] as f32 / unit;
                    let r2 = run_lengths[1] as f32 / unit;
                    let r3 = run_lengths[2] as f32 / unit;
                    let r4 = run_lengths[3] as f32 / unit;
                    let r5 = run_lengths[4] as f32 / unit;

                    // Allow 0.5 tolerance
                    if (r1 - 1.0).abs() <= 0.5
                        && (r2 - 1.0).abs() <= 0.5
                        && (r3 - 3.0).abs() <= 0.5
                        && (r4 - 1.0).abs() <= 0.5
                        && (r5 - 1.0).abs() <= 0.5
                    {
                        // Found pattern - center is in middle of center black region
                        let center_x = (x - run_lengths[4]) as f32
                            - run_lengths[3] as f32
                            - run_lengths[2] as f32 / 2.0;
                        candidates.push(FinderPattern {
                            center: Point::new(center_x, y as f32),
                            module_size: unit,
                        });
                    }

                    // Shift runs to check overlapping patterns
                    run_lengths[0] = run_lengths[2];
                    run_lengths[1] = run_lengths[3];
                    run_lengths[2] = run_lengths[4];
                    current_run = 3;
                }
            }
        }

        candidates
    }

    fn merge_candidates(candidates: Vec<FinderPattern>) -> Vec<FinderPattern> {
        let mut merged: Vec<FinderPattern> = Vec::new();
        const MERGE_DISTANCE: f32 = 5.0;

        for candidate in candidates {
            let mut found = false;
            for i in 0..merged.len() {
                let existing = &merged[i];
                let dx = candidate.center.x - existing.center.x;
                let dy = candidate.center.y - existing.center.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq < MERGE_DISTANCE * MERGE_DISTANCE {
                    // Average the centers
                    let new_x = (existing.center.x + candidate.center.x) / 2.0;
                    let new_y = (existing.center.y + candidate.center.y) / 2.0;
                    let new_module = (existing.module_size + candidate.module_size) / 2.0;
                    merged[i] = FinderPattern::new(new_x, new_y, new_module);
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
    fn test_simple_pattern() {
        // Create a simple 1:1:3:1:1 pattern
        let mut matrix = BitMatrix::new(30, 20);
        let y = 10;

        // Black(3), White(3), Black(9), White(3), Black(3)
        for x in 5..8 {
            matrix.set(x, y, true);
        } // Black 1
          // White 1 (x: 8-11) - already false
        for x in 11..20 {
            matrix.set(x, y, true);
        } // Black 2 (center)
          // White 2 (x: 20-23) - already false
        for x in 23..26 {
            matrix.set(x, y, true);
        } // Black 3

        let patterns = FinderDetector::detect(&matrix);

        assert!(!patterns.is_empty(), "Should find at least one pattern");
        assert!(
            patterns
                .iter()
                .any(|p| p.center.x > 10.0 && p.center.x < 20.0),
            "Pattern center should be around x=14"
        );
    }
}
