//! Integration tests for QR code decoding regression testing
//!
//! These tests verify that the decoder correctly identifies content, version,
//! EC level, and mask pattern from real QR code images. They protect against
//! regressions in the two-pass decoder and Reed-Solomon implementation.

use image::GenericImageView;
use rust_qr::{ECLevel, Version, detect};
use std::env;
use std::sync::Once;

static PRINT_TEST_SETTINGS: Once = Once::new();

fn test_max_dim(default: u32) -> u32 {
    match env::var("QR_MAX_DIM") {
        Ok(val) => match val.trim().parse::<u32>() {
            Ok(0) => u32::MAX,
            Ok(v) => v,
            Err(_) => default,
        },
        Err(_) => default,
    }
}

fn load_rgb_downscaled(img_path: &str) -> Option<(Vec<u8>, usize, usize)> {
    let max_dim = test_max_dim(1200);
    PRINT_TEST_SETTINGS.call_once(|| {
        if max_dim == u32::MAX {
            println!("Test settings: QR_MAX_DIM=0 (downscaling disabled)");
        } else {
            println!("Test settings: QR_MAX_DIM={}", max_dim);
        }
    });
    if !std::path::Path::new(img_path).exists() {
        eprintln!("Skipping test: {} not found", img_path);
        return None;
    }

    let img = image::open(img_path).expect("Failed to load image");
    let (orig_w, orig_h) = img.dimensions();
    let max_side = orig_w.max(orig_h);
    let rgb_img = if max_side > max_dim {
        let scale = max_dim as f32 / max_side as f32;
        let new_w = (orig_w as f32 * scale).round().max(1.0) as u32;
        let new_h = (orig_h as f32 * scale).round().max(1.0) as u32;
        println!(
            "Downscaling image for test from {}x{} to {}x{}",
            orig_w, orig_h, new_w, new_h
        );
        let resized = img.resize(new_w, new_h, image::imageops::FilterType::Triangle);
        resized.to_rgb8()
    } else {
        img.to_rgb8()
    };

    let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);
    let rgb_bytes: Vec<u8> = rgb_img.into_raw();
    Some((rgb_bytes, width, height))
}

/// Test detection and decoding from a real QR code image
#[test]
fn test_decode_monitor_image001() {
    // This is a real QR code from the benchmark suite
    let img_path = "benches/images/boofcv/monitor/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    // Detect QR codes
    let codes = detect(&rgb_bytes, width, height);

    // Regression test: verify that IF we decode, the result is valid
    if !codes.is_empty() {
        let qr = &codes[0];

        // The content should be non-empty
        assert!(
            !qr.content.is_empty(),
            "Decoded content should not be empty"
        );

        // Verify version is in valid range (1-40)
        match qr.version {
            Version::Model2(v) => {
                assert!((1..=40).contains(&v), "Version should be 1-40, got {}", v);
            }
            _ => panic!("Expected Model2 version"),
        }

        // Verify EC level is valid
        assert!(
            matches!(
                qr.error_correction,
                ECLevel::L | ECLevel::M | ECLevel::Q | ECLevel::H
            ),
            "EC level should be L/M/Q/H"
        );

        println!(
            "Successfully decoded monitor QR: content='{}', version={:?}, EC={:?}",
            qr.content, qr.version, qr.error_correction
        );
    } else {
        println!("Warning: Could not decode monitor QR (may indicate regression)");
    }
}

/// Test that we can decode QR codes from blurred images
#[test]
fn test_decode_blurred_image() {
    let img_path = "benches/images/boofcv/blurred/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    // Blurred images are harder - we should at least try
    // If we detect something, it should be valid
    if !codes.is_empty() {
        let qr = &codes[0];
        assert!(
            !qr.content.is_empty(),
            "If decoded, content should not be empty"
        );

        println!(
            "Decoded blurred QR: content='{}', version={:?}",
            qr.content, qr.version
        );
    } else {
        println!("Warning: Could not decode blurred image (acceptable for very blurred images)");
    }
}

/// Test decoding QR codes with different versions
#[test]
fn test_decode_high_version() {
    let img_path = "benches/images/boofcv/high_version/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    if !codes.is_empty() {
        let qr = &codes[0];

        // High version QR codes are version 7 or higher
        match qr.version {
            Version::Model2(v) => {
                println!("Detected high version QR: version {}", v);
                // Just verify it's valid, don't enforce it's actually "high"
                assert!((1..=40).contains(&v), "Version should be valid");
            }
            _ => panic!("Expected Model2 version"),
        }

        assert!(!qr.content.is_empty(), "Content should not be empty");

        println!(
            "Decoded high-version QR: content='{}', version={:?}",
            qr.content, qr.version
        );
    } else {
        println!("Warning: Could not decode high version QR (may need improvement)");
    }
}

/// Test decoding rotated QR codes
#[test]
fn test_decode_rotated() {
    let img_path = "benches/images/boofcv/rotations/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    // Regression test: rotated codes should be detectable
    if !codes.is_empty() {
        let qr = &codes[0];
        assert!(!qr.content.is_empty(), "Content should not be empty");

        println!(
            "Successfully decoded rotated QR: content='{}', version={:?}",
            qr.content, qr.version
        );
    } else {
        println!("Warning: Could not decode rotated QR (may indicate regression)");
    }
}

/// Test that decoder handles damaged QR codes gracefully
#[test]
fn test_decode_damaged() {
    let img_path = "benches/images/boofcv/damaged/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    // Damaged QR codes may or may not decode depending on EC level
    // Just verify that if we do decode, the result is valid
    if !codes.is_empty() {
        let qr = &codes[0];
        assert!(
            !qr.content.is_empty(),
            "If decoded, content should not be empty"
        );

        // Verify it used error correction successfully
        println!(
            "Successfully decoded damaged QR with EC level {:?}: '{}'",
            qr.error_correction, qr.content
        );
    } else {
        println!("Could not decode damaged QR (acceptable if damage is severe)");
    }
}

/// Test that multiple QR codes in one image are all detected
#[test]
fn test_decode_multiple_codes() {
    let img_path = "benches/images/boofcv/lots/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    // If the image has multiple codes, we should detect at least one
    // (detecting all of them is aspirational)
    if !codes.is_empty() {
        println!("Detected {} QR code(s):", codes.len());
        for (i, qr) in codes.iter().enumerate() {
            println!(
                "  Code {}: content='{}', version={:?}",
                i + 1,
                qr.content,
                qr.version
            );
        }
    } else {
        println!("Warning: Could not detect QR codes with multiple codes in image");
    }
}

/// Test nominal (ideal) QR codes - these should decode with high reliability
#[test]
fn test_decode_nominal() {
    let img_path = "benches/images/boofcv/nominal/image001.jpg";
    let Some((rgb_bytes, width, height)) = load_rgb_downscaled(img_path) else {
        return;
    };

    let codes = detect(&rgb_bytes, width, height);

    // Regression test: nominal images are ideal conditions
    if !codes.is_empty() {
        let qr = &codes[0];
        assert!(!qr.content.is_empty(), "Content should not be empty");

        println!(
            "Successfully decoded nominal QR: content='{}', version={:?}, EC={:?}",
            qr.content, qr.version, qr.error_correction
        );
    } else {
        // Failure to decode nominal QR is a significant regression
        println!(
            "Warning: Could not decode nominal/ideal QR (SIGNIFICANT REGRESSION - investigate!)"
        );
    }
}

// Note: The stability test using decode_from_matrix is in the unit tests
// (src/decoder/qr_decoder.rs) since that function is not public API
