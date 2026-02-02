use crate::models::BitMatrix;

/// Function module mask for a specific QR version.
/// true = function module (not data), false = data module.
pub struct FunctionMask {
    mask: BitMatrix,
    version: u8,
}

impl FunctionMask {
    pub fn new(version: u8) -> Self {
        let size = 17 + 4 * version as usize;
        let mut mask = BitMatrix::new(size, size);

        // Finder patterns + separators (up to 9x9 areas, clipped to bounds)
        Self::mark_finder_area(&mut mask, 0, 0);
        Self::mark_finder_area(&mut mask, size - 7, 0);
        Self::mark_finder_area(&mut mask, 0, size - 7);

        // Timing patterns (row 6 and column 6)
        for i in 0..size {
            mask.set(6, i, true);
            mask.set(i, 6, true);
        }

        // Alignment patterns
        let align = alignment_pattern_positions(version);
        for &cx in &align {
            for &cy in &align {
                // Skip the three finder corners
                let in_tl = cx <= 8 && cy <= 8;
                let in_tr = cx >= size - 9 && cy <= 8;
                let in_bl = cx <= 8 && cy >= size - 9;
                if in_tl || in_tr || in_bl {
                    continue;
                }
                // 5x5 alignment pattern
                for dy in 0..5 {
                    for dx in 0..5 {
                        let x = cx.saturating_sub(2) + dx;
                        let y = cy.saturating_sub(2) + dy;
                        if x < size && y < size {
                            mask.set(x, y, true);
                        }
                    }
                }
            }
        }

        // Format info areas
        for i in 0..9 {
            if i != 6 {
                mask.set(8, i, true);
                mask.set(i, 8, true);
            }
        }
        for i in 0..8 {
            mask.set(size - 1 - i, 8, true);
            mask.set(8, size - 1 - i, true);
        }

        // Dark module
        mask.set(8, size - 8, true);

        // Version info (v7+)
        if version >= 7 {
            for dy in 0..6 {
                for dx in 0..3 {
                    mask.set(size - 11 + dx, dy, true);
                    mask.set(dx, size - 11 + dy, true);
                }
            }
        }

        Self { mask, version }
    }

    pub fn size(&self) -> usize {
        self.mask.width()
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn is_function(&self, x: usize, y: usize) -> bool {
        self.mask.get(x, y)
    }

    pub fn data_modules_count(&self) -> usize {
        let mut count = 0;
        let size = self.mask.width();
        for y in 0..size {
            for x in 0..size {
                if !self.mask.get(x, y) {
                    count += 1;
                }
            }
        }
        count
    }

    fn mark_finder_area(mask: &mut BitMatrix, x: usize, y: usize) {
        let size = mask.width();
        let start_x = x.saturating_sub(1);
        let start_y = y.saturating_sub(1);
        let end_x = (x + 7 + 1).min(size);
        let end_y = (y + 7 + 1).min(size);
        for yy in start_y..end_y {
            for xx in start_x..end_x {
                mask.set(xx, yy, true);
            }
        }
    }
}

/// Alignment pattern centers for a given version.
pub fn alignment_pattern_positions(version: u8) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let num_align = (version / 7) + 2;
    let size = 17 + 4 * version as usize;
    let step = if version == 32 {
        26
    } else {
        let numerator = version as usize * 4 + num_align as usize * 2 + 1;
        let denom = (num_align as usize * 2).saturating_sub(2);
        ((numerator + denom - 1) / denom) * 2
    };

    let mut positions = vec![0usize; num_align as usize];
    positions[0] = 6;
    let mut pos = size as isize - 7;
    for i in (1..num_align).rev() {
        positions[i as usize] = pos as usize;
        pos -= step as isize;
    }
    positions
}
