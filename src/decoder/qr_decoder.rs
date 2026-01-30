use crate::decoder::bitstream::BitstreamExtractor;
use crate::decoder::format::FormatInfo;
use crate::decoder::unmask::unmask;
use crate::decoder::version::VersionInfo;
/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, Point, QRCode, Version};
use crate::utils::geometry::PerspectiveTransform;

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

impl QrDecoder {
    /// Decode a QR code from a binary matrix and finder pattern locations
    pub fn decode(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<QRCode> {
        // Estimate module size from finder patterns
        let module_size = Self::estimate_module_size(top_left, top_right, bottom_left)?;

        // Calculate the bottom-right corner
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;

        // Determine QR code dimension (version)
        let dimension = Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;

        // Extract the QR code region with perspective correction
        let qr_matrix = Self::extract_qr_region(
            matrix,
            top_left,
            top_right,
            bottom_left,
            &bottom_right,
            dimension,
        )?;

        // Read format info
        let format_info = FormatInfo::extract(&qr_matrix)?;

        // Read version info (for v7+)
        let version = if dimension >= 45 {
            VersionInfo::extract(&qr_matrix).map(Version::Model2)
        } else {
            // Infer version from dimension
            let version_num = ((dimension - 17) / 4) as u8;
            if (1..=40).contains(&version_num) {
                Some(Version::Model2(version_num))
            } else {
                None
            }
        }?;

        // Unmask the QR code
        let mut unmasked = qr_matrix.clone();
        unmask(&mut unmasked, &format_info.mask_pattern);

        // Extract bitstream
        let bits = BitstreamExtractor::extract(&unmasked, dimension);

        // For now, return a basic QRCode structure
        // TODO: Actually decode the data from the bitstream
        let data = bits
            .iter()
            .map(|&b| if b { 1 } else { 0 })
            .collect::<Vec<u8>>();
        let content = format!(
            "QR v{:?} {:?} {:?}",
            version, format_info.ec_level, format_info.mask_pattern
        );

        Some(QRCode::new(
            data,
            content,
            version,
            format_info.ec_level,
            format_info.mask_pattern,
        ))
    }

    fn estimate_module_size(p1: &Point, p2: &Point, p3: &Point) -> Option<f32> {
        // Average distance between finder patterns / 7 modules
        let d12 = p1.distance(p2);
        let d13 = p1.distance(p3);
        let avg_dist = (d12 + d13) / 2.0;
        Some(avg_dist / 7.0)
    }

    fn calculate_bottom_right(
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<Point> {
        // In a perfect QR code, bottom_right = top_right + bottom_left - top_left
        let x = top_right.x + bottom_left.x - top_left.x;
        let y = top_right.y + bottom_left.y - top_left.y;
        Some(Point::new(x, y))
    }

    fn estimate_dimension(
        top_left: &Point,
        top_right: &Point,
        _bottom_right: &Point,
        module_size: f32,
    ) -> Option<usize> {
        // Calculate width in modules
        let width_pixels = top_left.distance(top_right);
        let width_modules = (width_pixels / module_size).round() as usize;

        // QR dimension = width + 7 (for the finder patterns at each end)
        // Actually, width should already include the full QR code
        // For version 1: 21 modules, version 2: 25, etc.
        // dimension = 17 + 4 * version

        // Infer version from measured width
        // version = (dimension - 17) / 4
        // dimension should be approximately width_modules + 7 (finder pattern width)
        let dimension = width_modules + 7;

        // Minimum valid dimension is 21 (version 1)
        if dimension < 21 {
            return None;
        }

        // Round to nearest valid dimension (must be 21, 25, 29, ... 177)
        let raw_version = (dimension - 17) / 4;
        let remainder = (dimension - 17) % 4;
        let version = if remainder <= 2 {
            raw_version as u8
        } else {
            (raw_version + 1) as u8
        };

        if (1..=40).contains(&version) {
            Some((17 + 4 * version) as usize)
        } else {
            None
        }
    }

    fn extract_qr_region(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<BitMatrix> {
        // Create perspective transform
        let src = [
            Point::new(3.5, 3.5), // Top-left finder center in module coordinates
            Point::new(dimension as f32 - 3.5, 3.5), // Top-right
            Point::new(3.5, dimension as f32 - 3.5), // Bottom-left
            Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5), // Bottom-right
        ];

        let dst = [*top_left, *top_right, *bottom_left, *bottom_right];

        let transform = PerspectiveTransform::from_points(&dst, &src)?;

        // Sample the QR code
        let mut result = BitMatrix::new(dimension, dimension);

        for y in 0..dimension {
            for x in 0..dimension {
                let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                let img_point = transform.transform(&module_center);

                // Sample the pixel (nearest neighbor for now)
                let img_x = img_point.x as usize;
                let img_y = img_point.y as usize;

                if img_x < matrix.width() && img_y < matrix.height() {
                    let is_black = matrix.get(img_x, img_y);
                    result.set(x, y, is_black);
                }
            }
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_basic() {
        // Create a simple test case with 3 finder patterns
        let matrix = BitMatrix::new(100, 100);
        let tl = Point::new(20.0, 20.0);
        let tr = Point::new(80.0, 20.0);
        let bl = Point::new(20.0, 80.0);

        // This will fail because there's no actual QR code in the matrix
        // but it tests the structure
        let result = QrDecoder::decode(&matrix, &tl, &tr, &bl);
        // Should return None because format extraction will fail
        assert!(result.is_none());
    }
}
