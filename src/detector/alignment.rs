/// Get alignment center positions for a QR version
pub fn get_alignment_positions(version: u8) -> Vec<(usize, usize)> {
    if version < 2 {
        return Vec::new();
    }

    let centers = match version {
        2 => vec![6, 18],
        3 => vec![6, 22],
        4 => vec![6, 26],
        5 => vec![6, 30],
        6 => vec![6, 34],
        _ => vec![6, 18], // Simplified for now
    };

    let mut positions = Vec::new();
    for &row in &centers {
        for &col in &centers {
            // Skip finder pattern positions (top-left, top-right, bottom-left corners)
            let last = *centers.last().unwrap_or(&6);
            let is_top_left = row == 6 && col == 6;
            let is_top_right = row == 6 && col == last;
            let is_bottom_left = row == last && col == 6;
            if is_top_left || is_top_right || is_bottom_left {
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
}
