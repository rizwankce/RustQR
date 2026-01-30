/// Timing pattern reading
/// Timing patterns run horizontally and vertically between finder patterns
use crate::models::{BitMatrix, Point};

/// Read timing pattern bits between two finder patterns
pub fn read_timing_pattern(matrix: &BitMatrix, start: &Point, end: &Point) -> Option<Vec<bool>> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance < 1.0 {
        return None;
    }

    let steps = distance as usize;
    let mut bits = Vec::with_capacity(steps);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (start.x + dx * t) as usize;
        let y = (start.y + dy * t) as usize;

        if x < matrix.width() && y < matrix.height() {
            bits.push(matrix.get(x, y));
        }
    }

    Some(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_pattern() {
        let mut matrix = BitMatrix::new(21, 21);

        // Create alternating pattern
        for x in 8..13 {
            matrix.set(x, 6, x % 2 == 0);
        }

        let start = Point::new(8.0, 6.0);
        let end = Point::new(13.0, 6.0);

        let bits = read_timing_pattern(&matrix, &start, &end);
        assert!(bits.is_some());
        assert_eq!(bits.unwrap().len(), 6);
    }
}
