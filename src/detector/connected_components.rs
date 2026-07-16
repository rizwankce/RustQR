/// Connected Components for efficient QR finder pattern detection
/// Finds black regions and filters by size/shape to identify candidates
use crate::models::BitMatrix;

/// Union-Find data structure
pub struct UnionFind {
    parent: Vec<u32>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n as u32).collect(),
        }
    }

    pub fn find(&mut self, x: u32) -> u32 {
        if self.parent[x as usize] != x {
            self.parent[x as usize] = self.find(self.parent[x as usize]);
        }
        self.parent[x as usize]
    }

    pub fn union(&mut self, x: u32, y: u32) {
        let root_x = self.find(x);
        let root_y = self.find(y);
        if root_x != root_y {
            self.parent[root_x as usize] = root_y;
        }
    }
}

/// Find connected regions of `value` and return their bounding boxes in
/// deterministic raster order.
fn find_regions(matrix: &BitMatrix, value: bool) -> Vec<(usize, usize, usize, usize)> {
    let width = matrix.width();
    let height = matrix.height();

    let mut labels = vec![0u32; width * height];
    let mut next_label = 1u32;
    let mut uf = UnionFind::new(width * height);

    // First pass: label components
    for y in 0..height {
        for x in 0..width {
            if matrix.get(x, y) != value {
                continue;
            }

            let idx = y * width + x;
            let mut neighbor_labels = Vec::new();

            // Check left (4-connectivity)
            if x > 0 && matrix.get(x - 1, y) == value {
                neighbor_labels.push(labels[y * width + x - 1]);
            }
            // Check above (4-connectivity)
            if y > 0 && matrix.get(x, y - 1) == value {
                neighbor_labels.push(labels[(y - 1) * width + x]);
            }
            // Check upper-left diagonal (8-connectivity for finder patterns)
            if x > 0 && y > 0 && matrix.get(x - 1, y - 1) == value {
                neighbor_labels.push(labels[(y - 1) * width + x - 1]);
            }
            // Check upper-right diagonal (8-connectivity)
            if x + 1 < width && y > 0 && matrix.get(x + 1, y - 1) == value {
                neighbor_labels.push(labels[(y - 1) * width + x + 1]);
            }

            if neighbor_labels.is_empty() {
                labels[idx] = next_label;
                next_label += 1;
            } else {
                let min_label = *neighbor_labels.iter().min().unwrap();
                labels[idx] = min_label;
                for &l in &neighbor_labels {
                    if l != min_label {
                        uf.union(min_label, l);
                    }
                }
            }
        }
    }

    // Compute bounding boxes
    let mut bboxes: std::collections::HashMap<u32, (usize, usize, usize, usize)> =
        std::collections::HashMap::new();

    for y in 0..height {
        for x in 0..width {
            let label = labels[y * width + x];
            if label == 0 {
                continue;
            }
            let root = uf.find(label);

            let entry = bboxes.entry(root).or_insert((x, y, x, y));
            entry.0 = entry.0.min(x);
            entry.1 = entry.1.min(y);
            entry.2 = entry.2.max(x);
            entry.3 = entry.3.max(y);
        }
    }

    // Component discovery is geometric, but the intermediate map has an
    // intentionally randomized iteration order.  Callers merge nearby boxes
    // with order-sensitive logic, so expose a stable raster order rather than
    // letting a hash seed change proposal coverage between equivalent runs.
    let mut regions = bboxes.values().cloned().collect::<Vec<_>>();
    regions.sort_unstable_by_key(|&(min_x, min_y, max_x, max_y)| (min_y, min_x, max_y, max_x));
    regions
}

/// Find connected black regions and return their bounding boxes.
pub fn find_black_regions(matrix: &BitMatrix) -> Vec<(usize, usize, usize, usize)> {
    find_regions(matrix, true)
}

/// Find connected white regions and return their bounding boxes.
///
/// A QR finder has an enclosed white ring around its centre square.  Keeping
/// this as a sibling to the black-component helper lets the bounded detector
/// supplement inspect that independent structural cue without inverting or
/// copying the full image.
pub fn find_white_regions(matrix: &BitMatrix) -> Vec<(usize, usize, usize, usize)> {
    find_regions(matrix, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_black_regions() {
        let mut matrix = BitMatrix::new(10, 10);
        // Create 2x2 black square at (2,2)
        matrix.set(2, 2, true);
        matrix.set(3, 2, true);
        matrix.set(2, 3, true);
        matrix.set(3, 3, true);

        let regions = find_black_regions(&matrix);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], (2, 2, 3, 3));
    }

    #[test]
    fn regions_are_returned_in_raster_order() {
        let mut matrix = BitMatrix::new(12, 12);
        matrix.set(8, 2, true);
        matrix.set(2, 7, true);
        matrix.set(3, 7, true);

        assert_eq!(
            find_black_regions(&matrix),
            vec![(8, 2, 8, 2), (2, 7, 3, 7)]
        );
    }

    #[test]
    fn finds_enclosed_white_ring_separately_from_background() {
        let mut matrix = BitMatrix::new(9, 9);
        for y in 1..8 {
            for x in 1..8 {
                matrix.set(x, y, true);
            }
        }
        for y in 2..7 {
            for x in 2..7 {
                matrix.set(x, y, false);
            }
        }
        for y in 3..6 {
            for x in 3..6 {
                matrix.set(x, y, true);
            }
        }

        assert!(find_white_regions(&matrix).contains(&(2, 2, 6, 6)));
    }
}
