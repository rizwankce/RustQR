#[allow(dead_code)]
#[path = "../src/tools/mod.rs"]
mod tools;

use image::{Rgb, RgbImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parse_reading_rate_defaults() {
    let args = Vec::<String>::new();
    let parsed = tools::parse_reading_rate_args(&args).expect("parse should succeed");

    let tools::ReadingRateCommand::Run(parsed) = parsed else {
        panic!("expected run command");
    };

    assert_eq!(
        parsed.dataset_root,
        PathBuf::from(tools::DEFAULT_DATASET_ROOT)
    );
    assert_eq!(
        parsed.artifact_path,
        PathBuf::from(tools::DEFAULT_ARTIFACT_PATH)
    );
    assert_eq!(parsed.limit, None);
    assert_eq!(parsed.max_working_dim, None);
    assert_eq!(parsed.emergency_cutoff_ms, None);
}

#[test]
fn parse_reading_rate_overrides_and_help() {
    let args = vec![
        "--dataset-root".to_string(),
        "tmp/data".to_string(),
        "--artifact".to_string(),
        "tmp/report.json".to_string(),
        "--limit".to_string(),
        "5".to_string(),
        "--max-working-dim".to_string(),
        "640".to_string(),
        "--emergency-cutoff-ms".to_string(),
        "250".to_string(),
    ];

    let parsed = tools::parse_reading_rate_args(&args).expect("parse should succeed");
    let tools::ReadingRateCommand::Run(parsed) = parsed else {
        panic!("expected run command");
    };

    assert_eq!(parsed.dataset_root, PathBuf::from("tmp/data"));
    assert_eq!(parsed.artifact_path, PathBuf::from("tmp/report.json"));
    assert_eq!(parsed.limit, Some(5));
    assert_eq!(parsed.max_working_dim, Some(640));
    assert_eq!(parsed.emergency_cutoff_ms, Some(250));

    let help_args = vec!["--help".to_string()];
    let parsed_help = tools::parse_reading_rate_args(&help_args).expect("help should parse");
    assert_eq!(parsed_help, tools::ReadingRateCommand::Help);
}

#[test]
fn parse_reading_rate_rejects_invalid_args() {
    let bad_limit = vec!["--limit".to_string(), "abc".to_string()];
    let bad_limit_err =
        tools::parse_reading_rate_args(&bad_limit).expect_err("expected parse error");
    assert!(bad_limit_err.contains("invalid --limit value"));

    let bad_dim = vec!["--max-working-dim".to_string(), "abc".to_string()];
    let bad_dim_err = tools::parse_reading_rate_args(&bad_dim).expect_err("expected parse error");
    assert!(bad_dim_err.contains("invalid --max-working-dim value"));

    let bad_cutoff = vec!["--emergency-cutoff-ms".to_string(), "abc".to_string()];
    let bad_cutoff_err =
        tools::parse_reading_rate_args(&bad_cutoff).expect_err("expected parse error");
    assert!(bad_cutoff_err.contains("invalid --emergency-cutoff-ms value"));

    let unknown = vec!["--wat".to_string()];
    let unknown_err = tools::parse_reading_rate_args(&unknown).expect_err("expected parse error");
    assert!(unknown_err.contains("unknown argument"));
}

#[test]
fn reading_rate_usage_mentions_runtime_knobs() {
    let usage = tools::reading_rate_usage();
    assert!(usage.contains("--max-working-dim"));
    assert!(usage.contains("--emergency-cutoff-ms"));
}

#[test]
fn paired_image_and_category_helpers_work() {
    let temp = temp_dir("paired");

    let category_dir = temp.join("nominal");
    fs::create_dir_all(&category_dir).expect("create category");
    let label = category_dir.join("image001.txt");
    let image = category_dir.join("image001.jpg");

    fs::write(&label, "expected-payload\n").expect("write label");
    fs::write(&image, "fake").expect("write image");

    assert_eq!(tools::paired_image_path(&label), Some(image.clone()));
    assert_eq!(
        tools::category_from_label_path(&temp, &label),
        "nominal".to_string()
    );
}

#[test]
fn category_helper_uses_dataset_name_for_flat_roots() {
    let temp = temp_dir("flat_category");
    let label = temp.join("image001.txt");
    fs::write(&label, "payload").expect("write label");

    let category = tools::category_from_label_path(&temp, &label);
    let expected = temp
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temp dir name")
        .to_string();
    assert_eq!(category, expected);
}

#[test]
fn discover_label_cases_skips_control_files_and_unpaired_labels() {
    let temp = temp_dir("discover");

    let nominal_dir = temp.join("nominal");
    let glare_dir = temp.join("glare");
    fs::create_dir_all(&nominal_dir).expect("create nominal");
    fs::create_dir_all(&glare_dir).expect("create glare");

    fs::write(temp.join("_smoke.txt"), "nominal/image001.jpg").expect("write control file");

    let label_1 = nominal_dir.join("image001.txt");
    let image_1 = nominal_dir.join("image001.jpg");
    fs::write(&label_1, "payload-1").expect("write label 1");
    fs::write(&image_1, "fake").expect("write image 1");

    let label_2 = nominal_dir.join("image002.txt");
    fs::write(&label_2, "payload-2").expect("write label 2");

    let label_3 = glare_dir.join("image010.txt");
    let image_3 = glare_dir.join("image010.png");
    fs::write(&label_3, "payload-3").expect("write label 3");
    fs::write(&image_3, "fake").expect("write image 3");

    let discovered = tools::discover_label_cases(&temp, None).expect("discover cases");
    assert_eq!(discovered.len(), 2);
    assert!(
        discovered
            .iter()
            .all(|case| case.image_path.exists() && case.label_path.exists())
    );

    let limited = tools::discover_label_cases(&temp, Some(1)).expect("discover limited");
    assert_eq!(limited.len(), 1);
}

#[test]
fn summary_helpers_compute_rates_medians_and_top_failure() {
    let outcomes = vec![
        tools::CaseOutcome {
            category: "cat-a".to_string(),
            label_path: PathBuf::from("a/1.txt"),
            image_path: PathBuf::from("a/1.jpg"),
            expected_payload: "one".to_string(),
            matched: true,
            runtime_ms: 10.0,
            failure_signature: None,
        },
        tools::CaseOutcome {
            category: "cat-a".to_string(),
            label_path: PathBuf::from("a/2.txt"),
            image_path: PathBuf::from("a/2.jpg"),
            expected_payload: "two".to_string(),
            matched: false,
            runtime_ms: 30.0,
            failure_signature: Some("x".to_string()),
        },
        tools::CaseOutcome {
            category: "cat-b".to_string(),
            label_path: PathBuf::from("b/1.txt"),
            image_path: PathBuf::from("b/1.jpg"),
            expected_payload: "three".to_string(),
            matched: false,
            runtime_ms: 5.0,
            failure_signature: Some("y".to_string()),
        },
    ];

    let (categories, global) = tools::summarize_outcomes(&outcomes);
    assert_eq!(categories.len(), 2);

    let cat_a = categories
        .iter()
        .find(|summary| summary.category == "cat-a")
        .expect("cat-a summary");
    assert_eq!(cat_a.total_cases, 2);
    assert_eq!(cat_a.matched_cases, 1);
    assert!((cat_a.reading_rate - 0.5).abs() < 1e-9);
    assert!((cat_a.median_runtime_ms - 20.0).abs() < 1e-9);
    assert_eq!(cat_a.top_failure_signature.as_deref(), Some("x"));

    assert_eq!(global.total_cases, 3);
    assert_eq!(global.matched_cases, 1);
    assert!((global.reading_rate - (1.0 / 3.0)).abs() < 1e-9);
    assert!((global.median_runtime_ms - 10.0).abs() < 1e-9);
    assert_eq!(global.top_failure_signature.as_deref(), Some("x"));
}

#[test]
fn reading_rate_report_flags_image_decode_failures() {
    let temp = temp_dir("decode_fail");
    let label = temp.join("image001.txt");
    let image = temp.join("image001.jpg");
    fs::write(&label, "expected").expect("write label");
    fs::write(&image, "not-a-real-image").expect("write invalid image");

    let args = tools::ReadingRateArgs {
        dataset_root: temp.clone(),
        artifact_path: temp.join("artifact.json"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: None,
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert_eq!(report.global.total_cases, 1);
    assert_eq!(report.global.matched_cases, 0);
    assert_eq!(
        report.global.top_failure_signature.as_deref(),
        Some(tools::IMAGE_LOAD_FAILURE_SIGNATURE)
    );
    assert_eq!(report.cases.len(), 1);
    assert_eq!(
        report.cases[0].failure_signature.as_deref(),
        Some(tools::IMAGE_LOAD_FAILURE_SIGNATURE)
    );
}

#[test]
fn harness_loader_resizes_large_images_and_decodes_png_jpeg() {
    let temp = temp_dir("decode_resize");
    let png_path = temp.join("large.png");
    let jpg_path = temp.join("large.jpg");
    write_checkerboard_image(&png_path, 640, 320);
    write_checkerboard_image(&jpg_path, 300, 500);

    let (_, png_width, png_height) =
        tools::load_rgb_image(&png_path, Some(128)).expect("decode resized png");
    assert_eq!((png_width, png_height), (128, 64));

    let (_, jpg_width, jpg_height) =
        tools::load_rgb_image(&jpg_path, Some(100)).expect("decode resized jpeg");
    assert_eq!((jpg_width, jpg_height), (60, 100));

    let (_, original_width, original_height) =
        tools::load_rgb_image(&png_path, None).expect("decode original png");
    assert_eq!((original_width, original_height), (640, 320));
}

#[test]
fn reading_rate_report_applies_emergency_cutoff_override() {
    let temp = temp_dir("cutoff_override");
    let label = temp.join("image001.txt");
    let image = temp.join("image001.png");
    fs::write(&label, "expected").expect("write label");
    write_checkerboard_image(&image, 96, 96);

    let args = tools::ReadingRateArgs {
        dataset_root: temp.clone(),
        artifact_path: temp.join("artifact.json"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: Some(0),
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert_eq!(report.global.total_cases, 1);
    assert_eq!(report.cases.len(), 1);
    assert_eq!(
        report.cases[0].failure_signature.as_deref(),
        Some("emergency-cutoff")
    );
}

fn write_checkerboard_image(path: &Path, width: u32, height: u32) {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let value = if on { 224 } else { 32 };
            image.put_pixel(x, y, Rgb([value, value, value]));
        }
    }
    image.save(path).expect("save test image");
}

fn temp_dir(suffix: &str) -> PathBuf {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "rustqr_wp007_{suffix}_{}_{}",
        std::process::id(),
        seed
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
