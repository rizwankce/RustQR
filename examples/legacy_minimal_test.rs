use rust_qr::decoder::qr_decoder::QrDecoder;
/// Minimal test: Create a simple synthetic QR code and try to decode it
/// This helps isolate whether the decoder works at all
use rust_qr::models::{BitMatrix, Point, QRCode};

fn main() {
    println!("Minimal QR Decoder Test");
    println!("=======================\n");

    // Create a minimal 21x21 QR code (Version 1) with known pattern
    // This is a simple test pattern - a valid QR code has:
    // - Finder patterns at 3 corners
    // - Timing patterns
    // - Format info
    // - Data

    let mut matrix = BitMatrix::new(21, 21);

    // Draw finder patterns (7x7 black squares with white border and black center)
    // Top-left finder pattern at (0,0)
    draw_finder_pattern(&mut matrix, 0, 0);
    // Top-right finder pattern at (14,0)
    draw_finder_pattern(&mut matrix, 14, 0);
    // Bottom-left finder pattern at (0,14)
    draw_finder_pattern(&mut matrix, 0, 14);

    // The finder pattern centers are at:
    let top_left = Point::new(3.5, 3.5); // Center of 7x7 pattern at (0,0)
    let top_right = Point::new(17.5, 3.5); // Center of 7x7 pattern at (14,0)
    let bottom_left = Point::new(3.5, 17.5); // Center of 7x7 pattern at (0,14)

    println!("Created 21x21 test QR code with 3 finder patterns");
    println!("  Top-left: ({}, {})", top_left.x, top_left.y);
    println!("  Top-right: ({}, {})", top_right.x, top_right.y);
    println!("  Bottom-left: ({}, {})", bottom_left.x, bottom_left.y);

    // Try to decode
    println!("\nAttempting to decode...");
    match QrDecoder::decode(&matrix, &top_left, &top_right, &bottom_left, 1.0) {
        Some(qr) => {
            println!("✓ SUCCESS! Decoded QR code: {:?}", qr);
        }
        None => {
            println!("✗ FAILED - decode() returned None");
            println!("  This suggests the decoder has issues even with perfect synthetic input");
        }
    }
}

fn draw_finder_pattern(matrix: &mut BitMatrix, x: usize, y: usize) {
    // Finder pattern is 7x7:
    // Black border (7x7)
    // White ring (5x5 inside)
    // Black center (3x3 inside)

    for dy in 0..7 {
        for dx in 0..7 {
            let px = x + dx;
            let py = y + dy;

            // Check if this pixel should be black
            let is_black = if dx == 0 || dx == 6 || dy == 0 || dy == 6 {
                // Outer border - black
                true
            } else if dx >= 2 && dx <= 4 && dy >= 2 && dy <= 4 {
                // Center 3x3 - black
                true
            } else {
                // Middle ring - white
                false
            };

            matrix.set(px, py, is_black);
        }
    }
}
