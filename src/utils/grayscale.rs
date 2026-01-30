/// Convert RGB image to grayscale using standard luminance formula
/// Y = 0.299*R + 0.587*G + 0.114*B
///
/// Uses fast integer arithmetic: Y = (76*R + 150*G + 29*B) >> 8
pub fn rgb_to_grayscale(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = Vec::with_capacity(pixel_count);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;
            let r = rgb[idx] as u32;
            let g = rgb[idx + 1] as u32;
            let b = rgb[idx + 2] as u32;

            // Fast integer approximation: (76*R + 150*G + 29*B) / 256
            let luminance = (76 * r + 150 * g + 29 * b) >> 8;
            gray.push(luminance.min(255) as u8);
        }
    }

    gray
}

/// Convert RGBA image to grayscale (ignores alpha channel)
pub fn rgba_to_grayscale(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut gray = Vec::with_capacity(pixel_count);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let r = rgba[idx] as u32;
            let g = rgba[idx + 1] as u32;
            let b = rgba[idx + 2] as u32;

            let luminance = (76 * r + 150 * g + 29 * b) >> 8;
            gray.push(luminance.min(255) as u8);
        }
    }

    gray
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_grayscale() {
        // Pure white (approximate due to integer arithmetic)
        let white = vec![255, 255, 255];
        let gray = rgb_to_grayscale(&white, 1, 1);
        assert!(gray[0] >= 254); // 255 * 255 / 256 ≈ 254

        // Pure black
        let black = vec![0, 0, 0];
        let gray = rgb_to_grayscale(&black, 1, 1);
        assert_eq!(gray[0], 0);

        // Pure red (should be darker than white)
        let red = vec![255, 0, 0];
        let gray = rgb_to_grayscale(&red, 1, 1);
        assert!(gray[0] < 255);
        assert!(gray[0] > 0);

        // Pure green (should be brighter than red due to human eye sensitivity)
        let green = vec![0, 255, 0];
        let gray = rgb_to_grayscale(&green, 1, 1);
        assert!(gray[0] > 100);

        // 2x2 image
        let img = vec![
            255, 0, 0, // red
            0, 255, 0, // green
            0, 0, 255, // blue
            255, 255, 255, // white
        ];
        let gray = rgb_to_grayscale(&img, 2, 2);
        assert_eq!(gray.len(), 4);
    }

    #[test]
    fn test_rgba_to_grayscale() {
        let rgba = vec![255, 128, 64, 255]; // RGBA with alpha
        let gray = rgba_to_grayscale(&rgba, 1, 1);
        assert_eq!(gray.len(), 1);
    }
}
