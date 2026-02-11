use image::io::Reader as ImageReader;
use rust_qr::{DetectConfig, pipeline};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn decodes_real_payload_on_nominal_subset_without_synthetic_payloads() {
    let config = DetectConfig {
        proposal_ensemble_budget_ms: 10_000,
        hypothesis_and_refinement_budget_ms: 10_000,
        decode_budget_ms: 10_000,
        multi_qr_budget_ms: 10_000,
        emergency_cutoff_ms: 30_000,
        max_working_dim: 1024,
        max_decode_hypotheses: 16,
        ..DetectConfig::default()
    };

    let cases = first_label_cases(Path::new("benches/images/boofcv/nominal"), 12)
        .expect("collect nominal label cases");
    assert!(!cases.is_empty(), "no nominal cases found");

    let mut decoded_any = false;

    for (image_path, _expected_payload) in cases {
        let (image, width, height) = load_rgb_image(&image_path).expect("load nominal image");
        let report = pipeline::detect_with_config(&image, width, height, &config);

        for code in &report.codes {
            decoded_any = true;
            assert!(
                !code.payload.starts_with("wp004-hypothesis-"),
                "synthetic payload leaked into runtime decode path: {}",
                code.payload
            );
        }
    }

    assert!(decoded_any, "no payloads decoded from nominal subset");
}

#[test]
fn proposal_overrun_triggers_decode_reserve_lane() {
    let width = 2048usize;
    let height = 2048usize;
    let image = checkerboard_rgb(width, height, 8);

    let config = DetectConfig {
        proposal_ensemble_budget_ms: 1,
        hypothesis_and_refinement_budget_ms: 400,
        decode_budget_ms: 400,
        multi_qr_budget_ms: 400,
        emergency_cutoff_ms: 10_000,
        max_working_dim: 2048,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    let stage_labels = report
        .stage_timings
        .iter()
        .map(|timing| timing.stage)
        .collect::<Vec<_>>();

    assert!(
        stage_labels
            .iter()
            .any(|stage| *stage == "proposal_ensemble:over-budget"),
        "expected proposal over-budget stage, got: {stage_labels:?}"
    );
    assert!(
        stage_labels
            .iter()
            .any(|stage| *stage == "decode_engine:reserve"),
        "expected decode reserve stage, got: {stage_labels:?}"
    );
    assert!(
        stage_labels
            .iter()
            .any(|stage| *stage == "multi_qr_iteration:reserve"),
        "expected multi-QR reserve stage, got: {stage_labels:?}"
    );
}

fn first_label_cases(root: &Path, limit: usize) -> Result<Vec<(PathBuf, String)>, String> {
    let mut labels = fs::read_dir(root)
        .map_err(|err| format!("failed to read {}: {err}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to list {}: {err}", root.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("txt"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    labels.sort();

    let mut out = Vec::new();
    for label_path in labels.into_iter().take(limit) {
        let expected = fs::read_to_string(&label_path)
            .map_err(|err| format!("failed to read label {}: {err}", label_path.display()))?
            .trim()
            .to_string();

        let mut image_path = None;
        for ext in ["png", "jpg", "jpeg", "gif", "bmp"] {
            let candidate = label_path.with_extension(ext);
            if candidate.is_file() {
                image_path = Some(candidate);
                break;
            }
        }

        if let Some(image_path) = image_path {
            out.push((image_path, expected));
        }
    }

    Ok(out)
}

fn load_rgb_image(path: &Path) -> Result<(Vec<u8>, usize, usize), String> {
    let reader = ImageReader::open(path)
        .map_err(|err| format!("failed to open image {}: {err}", path.display()))?;
    let decoded = reader
        .decode()
        .map_err(|err| format!("failed to decode image {}: {err}", path.display()))?;
    let rgb = decoded.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok((rgb.into_raw(), width as usize, height as usize))
}

fn checkerboard_rgb(width: usize, height: usize, tile: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let on = ((x / tile) + (y / tile)) % 2 == 0;
            let v = if on { 232u8 } else { 24u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}
