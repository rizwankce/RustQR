//! Strict, label-backed photographic regressions.
//!
//! The BoofCV point labels provide geometry and symbol counts, but not raw
//! payload labels.  This suite therefore asserts a known, stable payload only
//! for the monitor fixture and uses its checked-in geometry annotation for the
//! localization contract.  Known no-decode cases are intentionally excluded
//! rather than being represented by warning-only passing tests.

#![cfg(feature = "tools")]

use image::GenericImageView;
use rust_qr::{
    DecoderOptions, ECLevel, ImageInput, PixelFormat, Version, detect,
    tools::{Quadrilateral, parse_localization_labels, scale_quadrilaterals, score_localizations},
    try_detect_with_options,
};

const MONITOR_IMAGE: &str = "benches/images/boofcv/monitor/image001.jpg";
const MONITOR_LABEL: &str = "benches/images/boofcv/monitor/image001.txt";
const CLOSE_IMAGE: &str = "benches/images/boofcv/close/image002.jpg";
const CLOSE_LABEL: &str = "benches/images/boofcv/close/image002.txt";
const MAX_DIMENSION: u32 = 800;
const MINIMUM_IOU: f32 = 0.5;

struct LoadedRgb {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    source_width: usize,
    source_height: usize,
}

fn load_rgb_downscaled(path: &str) -> LoadedRgb {
    let image = image::open(path).expect("checked-in photographic fixture loads");
    let (source_width, source_height) = image.dimensions();
    let max_side = source_width.max(source_height);
    let image = if max_side > MAX_DIMENSION {
        let scale = MAX_DIMENSION as f32 / max_side as f32;
        let width = (source_width as f32 * scale).round().max(1.0) as u32;
        let height = (source_height as f32 * scale).round().max(1.0) as u32;
        image.resize(width, height, image::imageops::FilterType::Triangle)
    } else {
        image
    }
    .to_rgb8();
    LoadedRgb {
        width: image.width() as usize,
        height: image.height() as usize,
        pixels: image.into_raw(),
        source_width: source_width as usize,
        source_height: source_height as usize,
    }
}

#[test]
#[ignore = "slow real-image regression; run with cargo test --test decode_regression_tests -- --ignored"]
fn monitor_image001_decodes_payload_and_localizes_label() {
    let image = load_rgb_downscaled(MONITOR_IMAGE);
    assert_eq!((image.width, image.height), (658, 800));
    let labels = parse_localization_labels(MONITOR_LABEL).expect("monitor label is valid");
    let expected = scale_quadrilaterals(
        &labels.quadrilaterals,
        (image.source_width, image.source_height),
        (image.width, image.height),
    );
    assert_eq!(
        expected.len(),
        1,
        "monitor fixture has one annotated symbol"
    );

    let codes = detect(&image.pixels, image.width, image.height);
    assert_eq!(
        codes.len(),
        1,
        "monitor fixture must not miss or duplicate its symbol"
    );
    let code = &codes[0];
    assert_eq!(code.data, b"4376471154038");
    assert_eq!(code.content, "4376471154038");
    assert_eq!(code.version, Version::Model2(1));
    assert_eq!(code.error_correction, ECLevel::L);

    let predicted: Vec<Quadrilateral> = codes
        .iter()
        .map(|code| code.position.map(|point| [point.x, point.y]))
        .collect();
    let score = score_localizations(&expected, &predicted, MINIMUM_IOU);
    assert_eq!(
        score.true_positives, 1,
        "monitor symbol must localize at IoU >= {MINIMUM_IOU}"
    );
    assert_eq!(score.false_negatives, 0);
    assert_eq!(score.false_positives, 0);
    assert_eq!(score.duplicate_predictions, 0);
}

#[test]
#[ignore = "slow real-image regression; run with cargo test --test decode_regression_tests -- --ignored"]
fn close_image002_decodes_payload_and_localizes_label() {
    let image = load_rgb_downscaled(CLOSE_IMAGE);
    assert_eq!((image.width, image.height), (600, 800));
    let labels = parse_localization_labels(CLOSE_LABEL).expect("close label is valid");
    let expected = scale_quadrilaterals(
        &labels.quadrilaterals,
        (image.source_width, image.source_height),
        (image.width, image.height),
    );
    assert_eq!(expected.len(), 1, "close fixture has one annotated symbol");

    let codes = detect(&image.pixels, image.width, image.height);
    assert_eq!(
        codes.len(),
        1,
        "close fixture must not miss or duplicate its symbol"
    );
    let code = &codes[0];
    let expected_payload = "ABC 123456789 ".repeat(14) + "ABC 123456789";
    assert_eq!(code.data, expected_payload.as_bytes());
    assert_eq!(code.content, expected_payload);
    assert_eq!(code.version, Version::Model2(7));
    assert_eq!(code.error_correction, ECLevel::L);

    let predicted: Vec<Quadrilateral> = codes
        .iter()
        .map(|code| code.position.map(|point| [point.x, point.y]))
        .collect();
    let score = score_localizations(&expected, &predicted, MINIMUM_IOU);
    assert_eq!(
        score.true_positives, 1,
        "close symbol must localize at IoU >= {MINIMUM_IOU}"
    );
    assert_eq!(score.false_negatives, 0);
    assert_eq!(score.false_positives, 0);
    assert_eq!(score.duplicate_predictions, 0);
}

#[test]
#[ignore = "slow real-image regression; run with cargo test --test decode_regression_tests -- --ignored"]
fn monitor_image001_strict_path_needs_no_decoder_recovery() {
    let image = load_rgb_downscaled(MONITOR_IMAGE);
    let result = try_detect_with_options(
        ImageInput::new(&image.pixels, image.width, image.height, PixelFormat::Rgb),
        DecoderOptions::default().with_diagnostics(true),
    )
    .expect("validated RGB fixture is accepted");

    assert_eq!(result.codes.len(), 1, "monitor fixture must decode once");
    assert_eq!(result.codes[0].data, b"4376471154038");
    let telemetry = result
        .diagnostics
        .telemetry
        .expect("diagnostics were explicitly requested");
    assert_eq!(telemetry.recovery_mode_attempts, 0);
    assert_eq!(telemetry.rs_erasure_attempts, 0);
}

#[test]
#[ignore = "slow real-image regression; run with cargo test --test decode_regression_tests -- --ignored"]
fn close_image002_strict_path_needs_no_decoder_recovery() {
    let image = load_rgb_downscaled(CLOSE_IMAGE);
    let result = try_detect_with_options(
        ImageInput::new(&image.pixels, image.width, image.height, PixelFormat::Rgb),
        DecoderOptions::default().with_diagnostics(true),
    )
    .expect("validated RGB fixture is accepted");

    assert_eq!(result.codes.len(), 1, "close fixture must decode once");
    let telemetry = result
        .diagnostics
        .telemetry
        .expect("diagnostics were explicitly requested");
    assert_eq!(telemetry.recovery_mode_attempts, 0);
    assert_eq!(telemetry.rs_erasure_attempts, 0);
}

#[test]
fn photographic_acceptance_scope_lists_known_unresolved_cases() {
    // These labels prove the fixtures are usable for future strict gates. They
    // are deliberately not run through `detect` here: their current misses
    // must remain visible in the WP-007 record, not become warning-only passes.
    for (label, expected_symbols) in [
        ("benches/images/boofcv/blurred/image001.txt", 1),
        ("benches/images/boofcv/high_version/image001.txt", 1),
        ("benches/images/boofcv/rotations/image001.txt", 3),
        ("benches/images/boofcv/damaged/image001.txt", 1),
        ("benches/images/boofcv/lots/image001.txt", 60),
        ("benches/images/boofcv/nominal/image001.txt", 2),
    ] {
        let labels = parse_localization_labels(label).expect("photographic label is valid");
        assert_eq!(labels.quadrilaterals.len(), expected_symbols, "{label}");
    }
}
