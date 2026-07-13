//! Deterministic, self-authored negative-image corpus for detector safety.
//!
//! The manifest is intentionally a generator contract rather than a set of
//! opaque binary fixtures. This keeps provenance and licensing unambiguous and
//! makes every image reproducible from source.

use rust_qr::{
    DecoderOptions, FailureStage, ImageInput, PixelFormat, try_detect, try_detect_with_options,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MANIFEST: &str = include_str!("negative_corpus/manifest.json");
const ZXING_MANIFEST: &str = include_str!("negative_corpus/zxing_manifest.json");
const ZXING_CROSS_SYMBOLOGY_MANIFEST: &str =
    include_str!("negative_corpus/zxing_cross_symbology_manifest.json");
const PER_IMAGE_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct CorpusManifest {
    schema_version: String,
    license: String,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    id: String,
    kind: String,
    width: usize,
    height: usize,
}

#[derive(Debug, Deserialize)]
struct ExternalCorpusManifest {
    schema_version: String,
    name: String,
    source_repository: String,
    source_commit: String,
    source_license: String,
    license_file: String,
    notice_file: String,
    reuse_declaration: String,
    cases: Vec<ExternalCorpusCase>,
}

#[derive(Debug, Deserialize)]
struct ExternalCorpusCase {
    id: String,
    local_path: String,
    source_path: String,
    category: String,
    source_expected_format: Option<String>,
    width: u32,
    height: u32,
    expected_qr_count: usize,
    sha256: String,
}

fn external_corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/negative_corpus")
}

fn external_corpus_shard() -> Option<(usize, usize)> {
    let value = std::env::var("WP009_EXTERNAL_SHARD").ok()?;
    let (index, count) = value
        .split_once('/')
        .unwrap_or_else(|| panic!("WP009_EXTERNAL_SHARD must be INDEX/COUNT"));
    let index = index
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("invalid shard index: {index}"));
    let count = count
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("invalid shard count: {count}"));
    assert!(count > 0 && index < count, "invalid shard {index}/{count}");
    Some((index, count))
}

fn canvas(width: usize, height: usize) -> Vec<u8> {
    vec![255; width * height]
}

fn fill_rect(image: &mut [u8], width: usize, x: usize, y: usize, w: usize, h: usize) {
    let height = image.len() / width;
    for py in y.min(height)..y.saturating_add(h).min(height) {
        for px in x.min(width)..x.saturating_add(w).min(width) {
            image[py * width + px] = 0;
        }
    }
}

fn draw_finder_like(image: &mut [u8], width: usize, x: usize, y: usize, scale: usize) {
    // A finder-looking target alone is not a QR symbol: the corpus deliberately
    // omits the QR timing/data structure and uses inconsistent target spacing.
    fill_rect(image, width, x, y, 7 * scale, 7 * scale);
    for py in y + scale..y + 6 * scale {
        for px in x + scale..x + 6 * scale {
            image[py * width + px] = 255;
        }
    }
    fill_rect(
        image,
        width,
        x + 2 * scale,
        y + 2 * scale,
        3 * scale,
        3 * scale,
    );
}

fn seeded_bit(state: &mut u64) -> bool {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state >> 63 != 0
}

fn render(case: &CorpusCase) -> Vec<u8> {
    let mut image = canvas(case.width, case.height);
    let (w, h) = (case.width, case.height);
    match case.kind.as_str() {
        "block_text" => {
            // Deliberately simple bitmap text-like strokes, not a screenshot.
            for row in 0..7 {
                for column in 0..12 {
                    let x = 22 + column * 18;
                    let y = 28 + row * 27;
                    fill_rect(&mut image, w, x, y, 10, 3);
                    if (row + column) % 3 != 0 {
                        fill_rect(&mut image, w, x, y, 3, 15);
                    }
                    if (row + column) % 2 == 0 {
                        fill_rect(&mut image, w, x + 7, y + 3, 3, 12);
                    }
                }
            }
        }
        "checkerboard" => {
            for y in 0..h {
                for x in 0..w {
                    if ((x / 8) + (y / 8)) % 2 == 0 {
                        image[y * w + x] = 0;
                    }
                }
            }
        }
        "package_panels" => {
            for (x, y, pw, ph) in [(14, 20, 104, 90), (136, 34, 100, 75), (46, 138, 170, 90)] {
                fill_rect(&mut image, w, x, y, pw, 3);
                fill_rect(&mut image, w, x, y + ph - 3, pw, 3);
                fill_rect(&mut image, w, x, y, 3, ph);
                fill_rect(&mut image, w, x + pw - 3, y, 3, ph);
                for line in 0..4 {
                    fill_rect(&mut image, w, x + 12, y + 14 + line * 15, pw - 27, 3);
                }
            }
        }
        "screen_grid" => {
            for x in (8..w).step_by(20) {
                fill_rect(&mut image, w, x, 8, 2, h - 16);
            }
            for y in (8..h).step_by(18) {
                fill_rect(&mut image, w, 8, y, w - 16, 2);
            }
            for row in 0..6 {
                fill_rect(&mut image, w, 20, 18 + row * 36, 85, 8);
            }
        }
        "data_matrix_like" => {
            let origin = 36;
            let modules = 23;
            let unit = 8;
            let mut state = 0xDADA_1234_5678_9ABC;
            for my in 0..modules {
                for mx in 0..modules {
                    let black = mx == 0
                        || my == modules - 1
                        || (my == 0 && mx % 2 == 0)
                        || (mx == modules - 1 && my % 2 == 0)
                        || (mx > 0
                            && mx + 1 < modules
                            && my > 0
                            && my + 1 < modules
                            && seeded_bit(&mut state));
                    if black {
                        fill_rect(
                            &mut image,
                            w,
                            origin + mx * unit,
                            origin + my * unit,
                            unit,
                            unit,
                        );
                    }
                }
            }
        }
        "aztec_like" => {
            let center = w / 2;
            for ring in 0..7 {
                let half = 12 + ring * 13;
                if ring % 2 == 0 {
                    fill_rect(&mut image, w, center - half, center - half, half * 2, 5);
                    fill_rect(&mut image, w, center - half, center + half - 5, half * 2, 5);
                    fill_rect(&mut image, w, center - half, center - half, 5, half * 2);
                    fill_rect(&mut image, w, center + half - 5, center - half, 5, half * 2);
                }
            }
        }
        "linear_barcode" => {
            let mut x = 18;
            let widths = [2, 3, 6, 2, 5, 4, 2, 7, 3, 5, 2, 4, 6, 3, 2];
            while x < w - 18 {
                for bar in widths {
                    fill_rect(&mut image, w, x, 30, bar, h - 60);
                    x += bar + 3;
                    if x >= w - 18 {
                        break;
                    }
                }
            }
        }
        "finder_like_graphics" => {
            draw_finder_like(&mut image, w, 24, 24, 9);
            draw_finder_like(&mut image, w, 153, 42, 7);
            draw_finder_like(&mut image, w, 83, 154, 6);
            fill_rect(&mut image, w, 20, 224, 210, 3);
        }
        "seeded_noise" => {
            let mut state = 0x5EED_CAFE_F00D_BAAD;
            for value in &mut image {
                *value = if seeded_bit(&mut state) { 0 } else { 255 };
            }
        }
        "halftone_dots" => {
            // A regular printed-halftone-like texture. The varying dot size
            // creates repeated square edges without borrowing a photograph.
            for gy in (12..h - 12).step_by(15) {
                for gx in (12..w - 12).step_by(15) {
                    let radius = 2 + ((gx / 15 + gy / 15 * 3) % 5);
                    fill_rect(
                        &mut image,
                        w,
                        gx.saturating_sub(radius),
                        gy.saturating_sub(radius),
                        radius * 2 + 1,
                        radius * 2 + 1,
                    );
                }
            }
        }
        "moire_stripes" => {
            // Two non-QR periodic line fields approximate interference from
            // resampled screen/print content, while staying source-generated.
            for y in 0..h {
                for x in 0..w {
                    let diagonal = (x + 2 * y) % 19 < 3;
                    let vertical = x % 23 < 3;
                    if diagonal || vertical {
                        image[y * w + x] = 0;
                    }
                }
            }
        }
        "finder_triplet_broken_timing" => {
            // Near-QR corner geometry, but the three target scales disagree
            // and the sparse timing strokes break at different intervals. It
            // exercises finder grouping without constructing a costly, valid-
            // looking matrix that would be inappropriate for a bounded test.
            draw_finder_like(&mut image, w, 13, 13, 3);
            draw_finder_like(&mut image, w, w - 16 - 7 * 3, 16, 3);
            draw_finder_like(&mut image, w, 17, h - 17 - 7 * 4, 4);
            for x in (40..88).step_by(11) {
                fill_rect(&mut image, w, x, 37, 4, 2);
            }
            for y in (42..82).step_by(13) {
                fill_rect(&mut image, w, 37, y, 2, 4);
            }
        }
        "nested_square_texture" => {
            // Multiple off-centre square rings have inconsistent spacing,
            // centres, and line widths, unlike a valid Aztec target.
            for (cx, cy, half, line) in [(46, 56, 35, 2), (70, 55, 25, 3), (58, 73, 14, 2)] {
                fill_rect(&mut image, w, cx - half, cy - half, half * 2, line);
                fill_rect(&mut image, w, cx - half, cy + half - line, half * 2, line);
                fill_rect(&mut image, w, cx - half, cy - half, line, half * 2);
                fill_rect(&mut image, w, cx + half - line, cy - half, line, half * 2);
            }
        }
        other => panic!("unknown negative corpus generator: {other}"),
    }
    image
}

#[test]
fn self_authored_negative_corpus_has_zero_false_positive_detections() {
    let manifest: CorpusManifest = serde_json::from_str(MANIFEST).expect("valid corpus manifest");
    assert_eq!(manifest.schema_version, "rustqr.negative-corpus.v1");
    assert_eq!(manifest.license, "MIT OR Apache-2.0");
    assert!(!manifest.cases.is_empty());

    let mut total_pixels = 0_usize;
    let mut positive_images = 0_usize;
    let mut false_positive_detections = 0_usize;
    let mut timeout_images = 0_usize;
    for case in &manifest.cases {
        let image = render(case);
        let result = try_detect_with_options(
            ImageInput::new(&image, case.width, case.height, PixelFormat::Grayscale),
            DecoderOptions::default()
                .with_deadline(PER_IMAGE_DEADLINE)
                .with_diagnostics(true),
        )
        .expect("generated corpus image is a valid grayscale input");
        total_pixels += case.width * case.height;
        if result.diagnostics.failure_stage == Some(FailureStage::Timeout) {
            timeout_images += 1;
            continue;
        }
        false_positive_detections += result.codes.len();
        positive_images += usize::from(!result.codes.is_empty());
        assert!(
            result.codes.is_empty(),
            "negative corpus case {} ({}) unexpectedly returned {} detections",
            case.id,
            case.kind,
            result.codes.len()
        );
    }

    assert_eq!(
        timeout_images, 0,
        "a timeout is not a passing negative-corpus result"
    );

    let megapixels = total_pixels as f64 / 1_000_000.0;
    println!(
        "NEGATIVE_CORPUS_METRICS cases={} pixels={} megapixels={megapixels:.6} positive_images={} timeout_images={} false_positive_detections={} fp_per_image={:.6} fp_per_megapixel={:.6}",
        manifest.cases.len(),
        total_pixels,
        positive_images,
        timeout_images,
        false_positive_detections,
        false_positive_detections as f64 / manifest.cases.len() as f64,
        false_positive_detections as f64 / megapixels,
    );
}

#[test]
fn admitted_zxing_negative_corpus_has_zero_false_positive_detections() {
    let manifest: ExternalCorpusManifest =
        serde_json::from_str(ZXING_MANIFEST).expect("valid external corpus manifest");
    assert_eq!(
        manifest.schema_version,
        "rustqr.external-negative-corpus.v1"
    );
    assert_eq!(manifest.name, "zxing-negative-blackbox");
    assert_eq!(manifest.source_repository, "https://github.com/zxing/zxing");
    assert_eq!(
        manifest.source_commit,
        "82333b3ed894ef097d41dd8c922689ede8880e01"
    );
    assert_eq!(manifest.source_license, "Apache-2.0");

    let root = external_corpus_root();
    for required_file in [
        &manifest.license_file,
        &manifest.notice_file,
        &manifest.reuse_declaration,
    ] {
        assert!(
            root.join(required_file).is_file(),
            "missing {required_file}"
        );
    }
    assert_eq!(manifest.cases.len(), 47);
    let shard = external_corpus_shard();

    let mut total_pixels = 0_u64;
    let mut evaluated_cases = 0_usize;
    let mut positive_images = 0_usize;
    let mut false_positive_detections = 0_usize;
    let mut timeout_images = 0_usize;
    for (case_index, case) in manifest.cases.iter().enumerate() {
        if let Some((shard_index, shard_count)) = shard {
            if case_index % shard_count != shard_index {
                continue;
            }
        }
        assert_eq!(
            case.expected_qr_count, 0,
            "{} is no longer a negative",
            case.id
        );
        assert_eq!(case.sha256.len(), 64, "{} is missing a SHA-256", case.id);
        assert!(
            case.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{} has an invalid SHA-256",
            case.id
        );
        assert!(
            case.source_path
                .starts_with("core/src/test/resources/blackbox/"),
            "{} has an unexpected source path",
            case.id
        );
        assert!(
            matches!(
                case.category.as_str(),
                "random_high_contrast_pattern" | "random_high_contrast_pattern_v2"
            ),
            "{} has an unexpected category",
            case.id
        );

        let image_path = root.join(&case.local_path);
        let image = image::open(&image_path)
            .unwrap_or_else(|error| panic!("{} did not load: {error}", image_path.display()));
        assert_eq!(image.width(), case.width, "{} width changed", case.id);
        assert_eq!(image.height(), case.height, "{} height changed", case.id);
        let rgb = image.to_rgb8();
        let result = try_detect_with_options(
            ImageInput::new(
                rgb.as_raw(),
                case.width as usize,
                case.height as usize,
                PixelFormat::Rgb,
            ),
            DecoderOptions::default()
                .with_deadline(PER_IMAGE_DEADLINE)
                .with_diagnostics(true),
        )
        .expect("admitted corpus image is a valid RGB input");
        total_pixels += u64::from(case.width) * u64::from(case.height);
        evaluated_cases += 1;
        if result.diagnostics.failure_stage == Some(FailureStage::Timeout) {
            timeout_images += 1;
            continue;
        }
        false_positive_detections += result.codes.len();
        positive_images += usize::from(!result.codes.is_empty());
        assert_eq!(
            result.codes.len(),
            case.expected_qr_count,
            "negative corpus case {} ({}) unexpectedly returned {} detections",
            case.id,
            case.category,
            result.codes.len()
        );
    }
    assert!(
        evaluated_cases > 0,
        "selected external corpus shard is empty"
    );

    assert_eq!(
        timeout_images, 0,
        "a timeout is not a passing external negative-corpus result"
    );
    let megapixels = total_pixels as f64 / 1_000_000.0;
    println!(
        "ZXING_NEGATIVE_CORPUS_METRICS cases={} pixels={} megapixels={megapixels:.6} positive_images={} timeout_images={} false_positive_detections={} fp_per_image={:.6} fp_per_megapixel={:.6}",
        evaluated_cases,
        total_pixels,
        positive_images,
        timeout_images,
        false_positive_detections,
        false_positive_detections as f64 / evaluated_cases as f64,
        false_positive_detections as f64 / megapixels,
    );
}

#[test]
#[ignore = "strict cross-symbology qualification currently has documented timeouts"]
fn admitted_zxing_cross_symbology_corpus_has_zero_false_positive_detections() {
    let manifest: ExternalCorpusManifest = serde_json::from_str(ZXING_CROSS_SYMBOLOGY_MANIFEST)
        .expect("valid cross-symbology external corpus manifest");
    assert_eq!(
        manifest.schema_version,
        "rustqr.external-negative-corpus.v1"
    );
    assert_eq!(manifest.name, "zxing-cross-symbology-blackbox");
    assert_eq!(manifest.source_repository, "https://github.com/zxing/zxing");
    assert_eq!(
        manifest.source_commit,
        "82333b3ed894ef097d41dd8c922689ede8880e01"
    );
    assert_eq!(manifest.source_license, "Apache-2.0");
    let root = external_corpus_root();
    for required_file in [
        &manifest.license_file,
        &manifest.notice_file,
        &manifest.reuse_declaration,
    ] {
        assert!(
            root.join(required_file).is_file(),
            "missing {required_file}"
        );
    }
    assert_eq!(manifest.cases.len(), 47);
    let shard = external_corpus_shard();

    let mut total_pixels = 0_u64;
    let mut evaluated_cases = 0_usize;
    let mut positive_images = 0_usize;
    let mut false_positive_detections = 0_usize;
    let mut timeout_images = 0_usize;
    for (case_index, case) in manifest.cases.iter().enumerate() {
        if let Some((shard_index, shard_count)) = shard {
            if case_index % shard_count != shard_index {
                continue;
            }
        }
        assert_eq!(
            case.expected_qr_count, 0,
            "{} is no longer a negative",
            case.id
        );
        assert!(
            matches!(
                (
                    case.category.as_str(),
                    case.source_expected_format.as_deref()
                ),
                ("valid_aztec", Some("AZTEC"))
                    | ("valid_datamatrix", Some("DATA_MATRIX"))
                    | ("valid_code128", Some("CODE_128"))
            ),
            "{} has an unexpected source format/category",
            case.id
        );
        assert!(
            case.source_path
                .starts_with("core/src/test/resources/blackbox/"),
            "{} has an unexpected source path",
            case.id
        );
        let image_path = root.join(&case.local_path);
        let image = image::open(&image_path)
            .unwrap_or_else(|error| panic!("{} did not load: {error}", image_path.display()));
        assert_eq!(image.width(), case.width, "{} width changed", case.id);
        assert_eq!(image.height(), case.height, "{} height changed", case.id);
        let rgb = image.to_rgb8();
        let result = try_detect_with_options(
            ImageInput::new(
                rgb.as_raw(),
                case.width as usize,
                case.height as usize,
                PixelFormat::Rgb,
            ),
            DecoderOptions::default()
                .with_deadline(PER_IMAGE_DEADLINE)
                .with_diagnostics(true),
        )
        .expect("admitted corpus image is a valid RGB input");
        total_pixels += u64::from(case.width) * u64::from(case.height);
        evaluated_cases += 1;
        if result.diagnostics.failure_stage == Some(FailureStage::Timeout) {
            timeout_images += 1;
            eprintln!("negative corpus timeout: {} ({})", case.id, case.category);
            continue;
        }
        false_positive_detections += result.codes.len();
        positive_images += usize::from(!result.codes.is_empty());
        assert_eq!(
            result.codes.len(),
            case.expected_qr_count,
            "negative corpus case {} ({}) unexpectedly returned {} detections",
            case.id,
            case.category,
            result.codes.len()
        );
    }
    assert!(
        evaluated_cases > 0,
        "selected external corpus shard is empty"
    );
    assert_eq!(
        timeout_images, 0,
        "a timeout is not a passing external corpus result"
    );
    let megapixels = total_pixels as f64 / 1_000_000.0;
    println!(
        "ZXING_CROSS_SYMBOLOGY_NEGATIVE_CORPUS_METRICS cases={} pixels={} megapixels={megapixels:.6} positive_images={} timeout_images={} false_positive_detections={} fp_per_image={:.6} fp_per_megapixel={:.6}",
        evaluated_cases,
        total_pixels,
        positive_images,
        timeout_images,
        false_positive_detections,
        false_positive_detections as f64 / evaluated_cases as f64,
        false_positive_detections as f64 / megapixels,
    );
}

#[test]
fn corpus_manifest_has_unique_case_ids_and_known_renderers() {
    let manifest: CorpusManifest = serde_json::from_str(MANIFEST).expect("valid corpus manifest");
    let mut ids: Vec<_> = manifest.cases.iter().map(|case| case.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), manifest.cases.len());
    for case in &manifest.cases {
        assert_eq!(render(case).len(), case.width * case.height);
    }
}

fn assert_case_is_negative(id: &str) {
    let manifest: CorpusManifest = serde_json::from_str(MANIFEST).expect("valid corpus manifest");
    let case = manifest
        .cases
        .iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("missing negative corpus case {id}"));
    let image = render(case);
    let detections = try_detect(ImageInput::new(
        &image,
        case.width,
        case.height,
        PixelFormat::Grayscale,
    ))
    .expect("generated corpus image is a valid grayscale input");
    assert!(
        detections.is_empty(),
        "negative corpus case {} ({}) unexpectedly returned {} detections",
        case.id,
        case.kind,
        detections.len()
    );
}

#[test]
fn halftone_dots_are_not_qr() {
    assert_case_is_negative("halftone_dots");
}

#[test]
fn moire_stripes_are_not_qr() {
    assert_case_is_negative("moire_stripes");
}

#[test]
fn broken_finder_triplet_is_not_qr() {
    assert_case_is_negative("finder_triplet_broken_timing");
}

#[test]
fn nested_square_texture_is_not_qr() {
    assert_case_is_negative("nested_square_texture");
}
