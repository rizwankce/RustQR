use crate::decoder::function_mask::alignment_pattern_positions;

/// Get alignment center positions for a QR version
pub fn get_alignment_positions(version: u8) -> Vec<(usize, usize)> {
    let centers = alignment_pattern_positions(version);
    if centers.is_empty() {
        return Vec::new();
    }
    let size = 17 + 4 * version as usize;
    let mut positions = Vec::new();
    for &row in &centers {
        for &col in &centers {
            let in_tl = row <= 8 && col <= 8;
            let in_tr = row >= size - 9 && col <= 8;
            let in_bl = row <= 8 && col >= size - 9;
            if in_tl || in_tr || in_bl {
                continue;
            }
            positions.push((row, col));
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_1_no_alignment() {
        let positions = get_alignment_positions(1);
        assert!(positions.is_empty());
    }

    #[test]
    fn test_version_2_has_alignment() {
        let positions = get_alignment_positions(2);
        assert!(!positions.is_empty());
        assert!(positions.contains(&(18, 18)));
    }

    #[test]
    fn test_version_7() {
        let positions = get_alignment_positions(7);
        // Version 7: centers [6, 22, 38], size=45
        // Should exclude corners overlapping finder patterns
        assert!(!positions.is_empty());
        // (6,6) excluded (TL finder), (6,38) excluded (BL), (38,6) excluded (TR)
        assert!(!positions.contains(&(6, 6)));
        assert!(!positions.contains(&(6, 38)));
        assert!(!positions.contains(&(38, 6)));
        // Valid positions should be present
        assert!(positions.contains(&(22, 22)));
        assert!(positions.contains(&(22, 6)));
        assert!(positions.contains(&(6, 22)));
        assert!(positions.contains(&(38, 22)));
        assert!(positions.contains(&(22, 38)));
        assert!(positions.contains(&(38, 38)));
    }

    #[test]
    fn test_version_10() {
        let positions = get_alignment_positions(10);
        // Version 10: centers [6, 28, 50], size=57
        assert!(!positions.is_empty());
        assert!(positions.contains(&(28, 28)));
        assert!(positions.contains(&(50, 50)));
    }

    #[test]
    fn test_version_14() {
        let positions = get_alignment_positions(14);
        // Version 14: centers [6, 26, 46, 66], size=73
        assert!(!positions.is_empty());
        assert!(positions.contains(&(26, 26)));
        assert!(positions.contains(&(46, 46)));
        assert!(positions.contains(&(66, 66)));
    }
}
