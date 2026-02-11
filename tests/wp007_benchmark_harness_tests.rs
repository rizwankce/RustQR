#[allow(dead_code)]
#[path = "../src/tools/mod.rs"]
mod tools;

use std::fs;
use std::path::PathBuf;
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
    ];

    let parsed = tools::parse_reading_rate_args(&args).expect("parse should succeed");
    let tools::ReadingRateCommand::Run(parsed) = parsed else {
        panic!("expected run command");
    };

    assert_eq!(parsed.dataset_root, PathBuf::from("tmp/data"));
    assert_eq!(parsed.artifact_path, PathBuf::from("tmp/report.json"));
    assert_eq!(parsed.limit, Some(5));

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

    let unknown = vec!["--wat".to_string()];
    let unknown_err = tools::parse_reading_rate_args(&unknown).expect_err("expected parse error");
    assert!(unknown_err.contains("unknown argument"));
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
