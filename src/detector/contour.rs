use crate::detector::connected_components::{find_black_regions, find_white_regions};
use crate::detector::finder::FinderPattern;
use crate::models::BitMatrix;

pub struct ContourDetector;

impl ContourDetector {
    /// Detect finder-like square regions from connected components.
    ///
    /// This is a fallback detector family for cases where run-length scanning
    /// struggles (noncompliant/pathological/curved). It intentionally prefers
    /// higher precision over recall and is used with a bounded decode budget.
    pub fn detect(matrix: &BitMatrix) -> Vec<FinderPattern> {
        let regions = find_black_regions(matrix);
        let mut candidates = Vec::new();

        for (min_x, min_y, max_x, max_y) in regions {
            let w = max_x.saturating_sub(min_x) + 1;
            let h = max_y.saturating_sub(min_y) + 1;
            let area = w * h;
            // Lowered from 64 to 32 for better small QR detection
            if area < 32 {
                continue;
            }

            let aspect = w as f32 / h as f32;
            // Relaxed from 0.65-1.45 to 0.5-2.0 for noncompliant/pathological
            if !(0.50..=2.00).contains(&aspect) {
                continue;
            }

            let black = black_pixels_in_bbox(matrix, min_x, min_y, max_x, max_y);
            let fill_ratio = black as f32 / area as f32;
            // Relaxed fill ratio for damaged/partially obscured QR codes
            if !(0.12..=0.88).contains(&fill_ratio) {
                continue;
            }

            let module_size = (w.max(h) as f32 / 7.0).max(1.0);
            let cx = (min_x + max_x) as f32 * 0.5;
            let cy = (min_y + max_y) as f32 * 0.5;
            candidates.push(FinderPattern::new(cx, cy, module_size));
        }

        merge_nearby(candidates)
    }

    /// Detect candidate finder centres from enclosed white component rings.
    ///
    /// The white 5x5-module ring surrounding a QR finder's black 3x3 centre
    /// stays a separate component even when the outer black ring is joined to
    /// QR data.  This is deliberately only a structural hint: callers still
    /// require the normal horizontal/vertical/pitch evidence before it can
    /// enter the shared appended proposal frontier.
    pub fn detect_white_rings(matrix: &BitMatrix) -> Vec<FinderPattern> {
        let mut candidates = Vec::new();
        for (min_x, min_y, max_x, max_y) in find_white_regions(matrix) {
            // The background is also a white component. A ring must be
            // enclosed by black pixels, so it cannot touch the image edge.
            if min_x == 0
                || min_y == 0
                || max_x + 1 >= matrix.width()
                || max_y + 1 >= matrix.height()
            {
                continue;
            }
            let width = max_x.saturating_sub(min_x) + 1;
            let height = max_y.saturating_sub(min_y) + 1;
            let longest = width.max(height);
            let shortest = width.min(height);
            if shortest < 3 || longest > 256 {
                continue;
            }
            let aspect = width as f32 / height as f32;
            if !(0.70..=1.43).contains(&aspect) {
                continue;
            }

            let center_x = (min_x + max_x) / 2;
            let center_y = (min_y + max_y) / 2;
            // The centre must be the black 3x3 finder core, not a white hole
            // in unrelated text or packaging. Ratio evidence is an independent
            // second gate in the finder detector.
            if !matrix.get(center_x, center_y) {
                continue;
            }
            candidates.push(FinderPattern::new(
                center_x as f32,
                center_y as f32,
                (width as f32 + height as f32) / 10.0,
            ));
        }
        candidates
    }
}

fn black_pixels_in_bbox(
    matrix: &BitMatrix,
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
) -> usize {
    let mut black = 0usize;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if matrix.get(x, y) {
                black += 1;
            }
        }
    }
    black
}

fn merge_nearby(mut candidates: Vec<FinderPattern>) -> Vec<FinderPattern> {
    if candidates.len() < 2 {
        return candidates;
    }

    let mut merged: Vec<FinderPattern> = Vec::new();
    for cand in candidates.drain(..) {
        let mut found = false;
        for existing in &mut merged {
            let dist = existing.center.distance(&cand.center);
            let merge_dist = (existing.module_size + cand.module_size) * 2.5;
            if dist <= merge_dist {
                existing.center.x = (existing.center.x + cand.center.x) * 0.5;
                existing.center.y = (existing.center.y + cand.center.y) * 0.5;
                existing.module_size = (existing.module_size + cand.module_size) * 0.5;
                found = true;
                break;
            }
        }
        if !found {
            merged.push(cand);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_ring_detector_reports_enclosed_finder_core() {
        let mut matrix = BitMatrix::new(15, 15);
        for y in 2..13 {
            for x in 2..13 {
                matrix.set(x, y, true);
            }
        }
        for y in 4..11 {
            for x in 4..11 {
                matrix.set(x, y, false);
            }
        }
        for y in 6..9 {
            for x in 6..9 {
                matrix.set(x, y, true);
            }
        }

        let candidates = ContourDetector::detect_white_rings(&matrix);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].center,
            FinderPattern::new(7.0, 7.0, 1.4).center
        );
        assert!((candidates[0].module_size - 1.4).abs() < f32::EPSILON);
    }
}
