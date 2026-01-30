/// 2D point with floating point coordinates
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// X coordinate
    pub x: f32,
    /// Y coordinate
    pub y: f32,
}

impl Point {
    /// Create a new point
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Calculate distance to another point
    pub fn distance(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate squared distance (faster, no sqrt)
    pub fn distance_squared(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Translate point by (dx, dy)
    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// Integer point for grid coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PointI {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
}

impl PointI {
    /// Create a new integer point
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
