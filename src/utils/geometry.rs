/// Geometry utilities for perspective transformations and calculations
use crate::models::Point;

/// Perspective transformation matrix (3x3)
pub struct PerspectiveTransform {
    a11: f32,
    a12: f32,
    a13: f32,
    a21: f32,
    a22: f32,
    a23: f32,
    a31: f32,
    a32: f32,
    a33: f32,
}

impl PerspectiveTransform {
    /// Create transform from 4 source points to 4 destination points
    pub fn from_points(src: &[Point; 4], dst: &[Point; 4]) -> Option<Self> {
        // Solve for perspective transformation
        // Using the direct linear transform (DLT) method

        let mut a = [[0.0f32; 8]; 8];
        let mut b = [0.0f32; 8];

        for i in 0..4 {
            let (sx, sy) = (src[i].x, src[i].y);
            let (dx, dy) = (dst[i].x, dst[i].y);

            let row = i * 2;
            a[row][0] = sx;
            a[row][1] = sy;
            a[row][2] = 1.0;
            a[row][3] = 0.0;
            a[row][4] = 0.0;
            a[row][5] = 0.0;
            a[row][6] = -dx * sx;
            a[row][7] = -dx * sy;
            b[row] = dx;

            a[row + 1][0] = 0.0;
            a[row + 1][1] = 0.0;
            a[row + 1][2] = 0.0;
            a[row + 1][3] = sx;
            a[row + 1][4] = sy;
            a[row + 1][5] = 1.0;
            a[row + 1][6] = -dy * sx;
            a[row + 1][7] = -dy * sy;
            b[row + 1] = dy;
        }

        // Solve using Gaussian elimination
        solve_linear_system(&a, &b).map(|solution| Self {
            a11: solution[0],
            a12: solution[1],
            a13: solution[2],
            a21: solution[3],
            a22: solution[4],
            a23: solution[5],
            a31: solution[6],
            a32: solution[7],
            a33: 1.0,
        })
    }

    /// Transform a point using this perspective matrix
    pub fn transform(&self, p: &Point) -> Point {
        let x = p.x;
        let y = p.y;

        let denominator = self.a31 * x + self.a32 * y + self.a33;
        if denominator.abs() < 1e-10 {
            return Point::new(0.0, 0.0);
        }

        let x_new = (self.a11 * x + self.a12 * y + self.a13) / denominator;
        let y_new = (self.a21 * x + self.a22 * y + self.a23) / denominator;

        Point::new(x_new, y_new)
    }
}

/// Solve 8x8 linear system using Gaussian elimination
#[allow(clippy::needless_range_loop)]
fn solve_linear_system(a: &[[f32; 8]; 8], b: &[f32; 8]) -> Option<[f32; 8]> {
    let mut a = *a;
    let mut b = *b;
    let n = 8;

    // Forward elimination
    for i in 0..n {
        // Find pivot
        let mut max_val = a[i][i].abs();
        let mut max_row = i;

        for k in (i + 1)..n {
            if a[k][i].abs() > max_val {
                max_val = a[k][i].abs();
                max_row = k;
            }
        }

        // Check for singular matrix
        if max_val < 1e-10 {
            return None;
        }

        // Swap rows
        if max_row != i {
            a.swap(i, max_row);
            b.swap(i, max_row);
        }

        // Eliminate column
        for k in (i + 1)..n {
            let factor = a[k][i] / a[i][i];
            b[k] -= factor * b[i];

            for j in i..n {
                a[k][j] -= factor * a[i][j];
            }
        }
    }

    // Back substitution
    let mut x = [0.0f32; 8];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for j in (i + 1)..n {
            sum -= a[i][j] * x[j];
        }

        if a[i][i].abs() < 1e-10 {
            return None;
        }

        x[i] = sum / a[i][i];
    }

    Some(x)
}

/// Calculate distance between two points
pub fn distance(p1: &Point, p2: &Point) -> f32 {
    let dx = p1.x - p2.x;
    let dy = p1.y - p2.y;
    (dx * dx + dy * dy).sqrt()
}

/// Calculate angle in radians between three points (p1-p2-p3)
pub fn angle(p1: &Point, p2: &Point, p3: &Point) -> f32 {
    let v1 = Point::new(p1.x - p2.x, p1.y - p2.y);
    let v2 = Point::new(p3.x - p2.x, p3.y - p2.y);

    let dot = v1.x * v2.x + v1.y * v2.y;
    let cross = v1.x * v2.y - v1.y * v2.x;

    cross.atan2(dot).abs()
}

/// Create perspective transform from multiple point correspondences using DLT with least squares
/// Returns None if fewer than 4 points or if the system is singular
pub fn perspective_from_points_multi(
    src_points: &[Point],
    dst_points: &[Point],
) -> Option<PerspectiveTransform> {
    let n = src_points.len().min(dst_points.len());
    if n < 4 {
        return None;
    }

    // Build over-determined system (2n equations, 8 unknowns)
    // Use least squares: (A^T A) x = A^T b
    let mut ata = [[0.0f32; 8]; 8];
    let mut atb = [0.0f32; 8];

    for i in 0..n {
        let (sx, sy) = (src_points[i].x, src_points[i].y);
        let (dx, dy) = (dst_points[i].x, dst_points[i].y);

        // x' equation: x' = (a*x + b*y + c) / (g*x + h*y + 1)
        // Linearized: a*x + b*y + c - g*x*x' - h*y*x' = x'
        ata[0][0] += sx * sx;
        ata[0][1] += sx * sy;
        ata[0][2] += sx;
        ata[0][6] -= sx * dx;
        ata[0][7] -= sy * dx;
        atb[0] += dx - sx;

        ata[1][0] += sx * sy;
        ata[1][1] += sy * sy;
        ata[1][2] += sy;
        ata[1][6] -= sx * dy;
        ata[1][7] -= sy * dy;
        atb[1] += dy - sy;

        ata[2][0] += sx * sx;
        ata[2][1] += sx * sy;
        ata[2][2] += sx;
        ata[2][6] -= sx * dx;
        ata[2][7] -= sy * dx;
        atb[2] += dx - sx;

        ata[3][0] += sx * sy;
        ata[3][1] += sy * sy;
        ata[3][2] += sy;
        ata[3][6] -= sx * dy;
        ata[3][7] -= sy * dy;
        atb[3] += dy - sy;
    }

    // Fill remaining equations (use first n equations twice for y')
    for i in 0..n.min(4) {
        let (sx, sy) = (src_points[i].x, src_points[i].y);
        let (_, dy) = (dst_points[i].x, dst_points[i].y);

        let row = 4 + i;
        ata[row][0] += sx * sx;
        ata[row][1] += sx * sy;
        ata[row][2] += sx;
        ata[row][6] -= sx * dy;
        ata[row][7] -= sy * dy;
        atb[row] += dy - sy;
    }

    // Solve normal equations: ATA * x = ATB
    // Use simple Gaussian elimination
    solve_normal_equations(&ata, &atb).map(|coeffs| PerspectiveTransform {
        a11: coeffs[0],
        a12: coeffs[1],
        a13: coeffs[2],
        a21: coeffs[3],
        a22: coeffs[4],
        a23: coeffs[5],
        a31: coeffs[6],
        a32: coeffs[7],
        a33: 1.0,
    })
}

fn solve_normal_equations(a: &[[f32; 8]; 8], b: &[f32; 8]) -> Option<[f32; 8]> {
    let mut a = *a;
    let mut b = *b;
    let n = 8;

    for i in 0..n {
        let mut max_val = a[i][i].abs();
        let mut max_row = i;
        for k in (i + 1)..n {
            if a[k][i].abs() > max_val {
                max_val = a[k][i].abs();
                max_row = k;
            }
        }

        if max_val < 1e-10 {
            return None;
        }

        if max_row != i {
            for j in 0..n {
                let temp = a[i][j];
                a[i][j] = a[max_row][j];
                a[max_row][j] = temp;
            }
            let temp = b[i];
            b[i] = b[max_row];
            b[max_row] = temp;
        }

        for k in (i + 1)..n {
            let factor = a[k][i] / a[i][i];
            for j in i..n {
                a[k][j] -= factor * a[i][j];
            }
            b[k] -= factor * b[i];
        }
    }

    let mut x = [0.0f32; 8];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for j in (i + 1)..n {
            sum -= a[i][j] * x[j];
        }
        if a[i][i].abs() < 1e-10 {
            return None;
        }
        x[i] = sum / a[i][i];
    }

    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perspective_transform() {
        let src = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];

        let dst = [
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(50.0, 50.0),
            Point::new(0.0, 50.0),
        ];

        let transform = PerspectiveTransform::from_points(&src, &dst);
        assert!(transform.is_some());

        let t = transform.unwrap();
        let p = t.transform(&Point::new(50.0, 50.0));
        assert!(p.x > 20.0 && p.x < 30.0); // Should be approximately in the middle
    }

    #[test]
    fn test_distance() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        assert!((distance(&p1, &p2) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_angle() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(1.0, 0.0);
        let p3 = Point::new(1.0, 1.0);

        let a = angle(&p1, &p2, &p3);
        assert!((a - std::f32::consts::PI / 2.0).abs() < 0.001);
    }
}
