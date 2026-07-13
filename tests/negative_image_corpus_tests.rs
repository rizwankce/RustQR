//! Deterministic, self-authored negative-image corpus for detector safety.
//!
//! The manifest is intentionally a generator contract rather than a set of
//! opaque binary fixtures. This keeps provenance and licensing unambiguous and
//! makes every image reproducible from source.

use rust_qr::{ImageInput, PixelFormat, try_detect};
use serde::Deserialize;

const MANIFEST: &str = include_str!("negative_corpus/manifest.json");

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
    for case in &manifest.cases {
        let image = render(case);
        let detections = try_detect(ImageInput::new(
            &image,
            case.width,
            case.height,
            PixelFormat::Grayscale,
        ))
        .expect("generated corpus image is a valid grayscale input");
        total_pixels += case.width * case.height;
        false_positive_detections += detections.len();
        positive_images += usize::from(!detections.is_empty());
        assert!(
            detections.is_empty(),
            "negative corpus case {} ({}) unexpectedly returned {} detections",
            case.id,
            case.kind,
            detections.len()
        );
    }

    let megapixels = total_pixels as f64 / 1_000_000.0;
    println!(
        "NEGATIVE_CORPUS_METRICS cases={} pixels={} megapixels={megapixels:.6} positive_images={} false_positive_detections={} fp_per_image={:.6} fp_per_megapixel={:.6}",
        manifest.cases.len(),
        total_pixels,
        positive_images,
        false_positive_detections,
        false_positive_detections as f64 / manifest.cases.len() as f64,
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
