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

/// Find connected black regions and return their bounding boxes
pub fn find_black_regions(matrix: &BitMatrix) -> Vec<(usize, usize, usize, usize)> {
    let width = matrix.width();
    let height = matrix.height();

    let mut labels = vec![0u32; width * height];
    let mut next_label = 1u32;
    let mut uf = UnionFind::new(width * height);

    // First pass: label components
    for y in 0..height {
        for x in 0..width {
            if !matrix.get(x, y) {
                continue;
            }

            let idx = y * width + x;
            let mut neighbor_labels = Vec::new();

            // Check left (4-connectivity)
            if x > 0 && matrix.get(x - 1, y) {
                neighbor_labels.push(labels[y * width + x - 1]);
            }
            // Check above (4-connectivity)
            if y > 0 && matrix.get(x, y - 1) {
                neighbor_labels.push(labels[(y - 1) * width + x]);
            }
            // Check upper-left diagonal (8-connectivity for finder patterns)
            if x > 0 && y > 0 && matrix.get(x - 1, y - 1) {
                neighbor_labels.push(labels[(y - 1) * width + x - 1]);
            }
            // Check upper-right diagonal (8-connectivity)
            if x + 1 < width && y > 0 && matrix.get(x + 1, y - 1) {
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

    bboxes.values().cloned().collect()
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
}
