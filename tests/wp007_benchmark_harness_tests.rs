#[allow(dead_code)]
#[path = "../src/tools/mod.rs"]
mod tools;

use image::{Rgb, RgbImage};
use std::fs;
use std::io;
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
    assert_eq!(parsed.profile, None);
    assert_eq!(parsed.limit, None);
    assert_eq!(parsed.max_working_dim, None);
    assert_eq!(parsed.emergency_cutoff_ms, None);
    assert_eq!(parsed.gate_global_rate_min, None);
    assert_eq!(parsed.gate_rotations_rate_min, None);
    assert_eq!(parsed.gate_high_version_rate_min, None);
    assert_eq!(parsed.gate_median_runtime_ms_max, None);
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
        "--gate-global-rate-min".to_string(),
        "0.9".to_string(),
        "--gate-rotations-rate-min".to_string(),
        "0.7".to_string(),
        "--gate-high-version-rate-min".to_string(),
        "0.4".to_string(),
        "--gate-median-runtime-ms-max".to_string(),
        "1000".to_string(),
    ];

    let parsed = tools::parse_reading_rate_args(&args).expect("parse should succeed");
    let tools::ReadingRateCommand::Run(parsed) = parsed else {
        panic!("expected run command");
    };

    assert_eq!(parsed.dataset_root, PathBuf::from("tmp/data"));
    assert_eq!(parsed.profile, None);
    assert_eq!(parsed.artifact_path, PathBuf::from("tmp/report.json"));
    assert_eq!(parsed.limit, Some(5));
    assert_eq!(parsed.max_working_dim, Some(640));
    assert_eq!(parsed.emergency_cutoff_ms, Some(250));
    assert_eq!(parsed.gate_global_rate_min, Some(0.9));
    assert_eq!(parsed.gate_rotations_rate_min, Some(0.7));
    assert_eq!(parsed.gate_high_version_rate_min, Some(0.4));
    assert_eq!(parsed.gate_median_runtime_ms_max, Some(1000.0));

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

    let bad_global_gate = vec!["--gate-global-rate-min".to_string(), "1.1".to_string()];
    let bad_global_gate_err =
        tools::parse_reading_rate_args(&bad_global_gate).expect_err("expected parse error");
    assert!(bad_global_gate_err.contains("invalid --gate-global-rate-min value"));

    let bad_rotations_gate = vec!["--gate-rotations-rate-min".to_string(), "abc".to_string()];
    let bad_rotations_gate_err =
        tools::parse_reading_rate_args(&bad_rotations_gate).expect_err("expected parse error");
    assert!(bad_rotations_gate_err.contains("invalid --gate-rotations-rate-min value"));

    let bad_high_version_gate = vec![
        "--gate-high-version-rate-min".to_string(),
        "-0.1".to_string(),
    ];
    let bad_high_version_gate_err =
        tools::parse_reading_rate_args(&bad_high_version_gate).expect_err("expected parse error");
    assert!(bad_high_version_gate_err.contains("invalid --gate-high-version-rate-min value"));

    let bad_runtime_gate = vec!["--gate-median-runtime-ms-max".to_string(), "-1".to_string()];
    let bad_runtime_gate_err =
        tools::parse_reading_rate_args(&bad_runtime_gate).expect_err("expected parse error");
    assert!(bad_runtime_gate_err.contains("invalid --gate-median-runtime-ms-max value"));

    let bad_profile = vec!["--profile".to_string(), "bad-smoke".to_string()];
    let bad_profile_err =
        tools::parse_reading_rate_args(&bad_profile).expect_err("expected parse error");
    assert!(bad_profile_err.contains("unknown --profile value"));

    let unknown = vec!["--wat".to_string()];
    let unknown_err = tools::parse_reading_rate_args(&unknown).expect_err("expected parse error");
    assert!(unknown_err.contains("unknown argument"));
}

#[test]
fn reading_rate_usage_mentions_runtime_knobs() {
    let usage = tools::reading_rate_usage();
    assert!(usage.contains("--profile"));
    assert!(usage.contains("boofcv-all"));
    assert!(usage.contains("payload-validated"));
    assert!(usage.contains("--max-working-dim"));
    assert!(usage.contains("--emergency-cutoff-ms"));
    assert!(usage.contains("--gate-global-rate-min"));
    assert!(usage.contains("--gate-rotations-rate-min"));
    assert!(usage.contains("--gate-high-version-rate-min"));
    assert!(usage.contains("--gate-median-runtime-ms-max"));
}

#[test]
fn parse_reading_rate_profile_resolves_dataset_root() {
    let boofcv_all_args = vec![
        "--profile".to_string(),
        tools::BOOFCV_ALL_PROFILE.to_string(),
    ];
    let parsed_boofcv_all =
        tools::parse_reading_rate_args(&boofcv_all_args).expect("boofcv-all profile should parse");
    let tools::ReadingRateCommand::Run(parsed_boofcv_all) = parsed_boofcv_all else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_boofcv_all.profile,
        Some(tools::ReadingRateProfile::BoofcvAll)
    );
    assert_eq!(
        parsed_boofcv_all.dataset_root,
        PathBuf::from("benches/images/boofcv")
    );

    let rotations_args = vec![
        "--profile".to_string(),
        tools::BOOFCV_ROTATIONS_PROFILE.to_string(),
    ];
    let parsed_rotations =
        tools::parse_reading_rate_args(&rotations_args).expect("rotations profile should parse");
    let tools::ReadingRateCommand::Run(parsed_rotations) = parsed_rotations else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_rotations.profile,
        Some(tools::ReadingRateProfile::BoofcvRotations)
    );
    assert_eq!(
        parsed_rotations.dataset_root,
        PathBuf::from("benches/images/boofcv/rotations")
    );

    let monitor_alias_args = vec![
        "--profile".to_string(),
        tools::MONITOR_SMOKE_PROFILE.to_string(),
    ];
    let parsed_monitor_alias = tools::parse_reading_rate_args(&monitor_alias_args)
        .expect("legacy monitor alias profile should parse");
    let tools::ReadingRateCommand::Run(parsed_monitor_alias) = parsed_monitor_alias else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_monitor_alias.profile,
        Some(tools::ReadingRateProfile::MonitorSmoke)
    );
    assert_eq!(
        parsed_monitor_alias.dataset_root,
        PathBuf::from("benches/images/boofcv/monitor")
    );

    let payload_args = vec![
        "--profile".to_string(),
        tools::PAYLOAD_VALIDATED_PROFILE.to_string(),
    ];
    let parsed_payload =
        tools::parse_reading_rate_args(&payload_args).expect("payload profile should parse");
    let tools::ReadingRateCommand::Run(parsed_payload) = parsed_payload else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_payload.profile,
        Some(tools::ReadingRateProfile::PayloadValidated)
    );
    assert_eq!(
        parsed_payload.dataset_root,
        PathBuf::from("benches/images/custom/decoding")
    );
}

#[test]
fn all_profiles_map_to_existing_roots() {
    for profile in tools::ALL_READING_RATE_PROFILES {
        let dataset_root = tools::reading_rate_profile_dataset_root(*profile);
        assert!(
            dataset_root.is_dir(),
            "missing dataset root for profile {} at {}",
            profile.as_str(),
            dataset_root.display()
        );
    }
}

#[test]
fn parse_reading_rate_dataset_root_overrides_profile() {
    let profile_then_dataset = vec![
        "--profile".to_string(),
        tools::MONITOR_SMOKE_PROFILE.to_string(),
        "--dataset-root".to_string(),
        "tmp/custom".to_string(),
    ];
    let parsed_profile_then_dataset = tools::parse_reading_rate_args(&profile_then_dataset)
        .expect("profile then dataset parse should succeed");
    let tools::ReadingRateCommand::Run(parsed_profile_then_dataset) = parsed_profile_then_dataset
    else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_profile_then_dataset.profile,
        Some(tools::ReadingRateProfile::MonitorSmoke)
    );
    assert_eq!(
        parsed_profile_then_dataset.dataset_root,
        PathBuf::from("tmp/custom")
    );

    let dataset_then_profile = vec![
        "--dataset-root".to_string(),
        "tmp/explicit".to_string(),
        "--profile".to_string(),
        tools::NOMINAL_SMOKE_PROFILE.to_string(),
    ];
    let parsed_dataset_then_profile = tools::parse_reading_rate_args(&dataset_then_profile)
        .expect("dataset then profile parse should succeed");
    let tools::ReadingRateCommand::Run(parsed_dataset_then_profile) = parsed_dataset_then_profile
    else {
        panic!("expected run command");
    };
    assert_eq!(
        parsed_dataset_then_profile.profile,
        Some(tools::ReadingRateProfile::NominalSmoke)
    );
    assert_eq!(
        parsed_dataset_then_profile.dataset_root,
        PathBuf::from("tmp/explicit")
    );
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
fn discover_label_cases_propagates_label_read_errors() {
    let temp = temp_dir("discover_label_read_error");
    let category_dir = temp.join("nominal");
    fs::create_dir_all(&category_dir).expect("create category");

    let label = category_dir.join("image001.txt");
    let image = category_dir.join("image001.jpg");
    fs::write(&image, "fake").expect("write image");
    fs::write(&label, [0xFF]).expect("write invalid utf8 label");

    let err = tools::discover_label_cases(&temp, None).expect_err("expected read error");
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
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
            pipeline_runtime_ms: 1.0,
            failure_signature: None,
        },
        tools::CaseOutcome {
            category: "cat-a".to_string(),
            label_path: PathBuf::from("a/2.txt"),
            image_path: PathBuf::from("a/2.jpg"),
            expected_payload: "two".to_string(),
            matched: false,
            runtime_ms: 30.0,
            pipeline_runtime_ms: 7.0,
            failure_signature: Some("x".to_string()),
        },
        tools::CaseOutcome {
            category: "cat-b".to_string(),
            label_path: PathBuf::from("b/1.txt"),
            image_path: PathBuf::from("b/1.jpg"),
            expected_payload: "three".to_string(),
            matched: false,
            runtime_ms: 5.0,
            pipeline_runtime_ms: 2.0,
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
    assert!((cat_a.median_pipeline_runtime_ms - 4.0).abs() < 1e-9);
    assert_eq!(cat_a.top_failure_signature.as_deref(), Some("x"));

    assert_eq!(global.total_cases, 3);
    assert_eq!(global.matched_cases, 1);
    assert!((global.reading_rate - (1.0 / 3.0)).abs() < 1e-9);
    assert!((global.median_runtime_ms - 10.0).abs() < 1e-9);
    assert!((global.median_pipeline_runtime_ms - 2.0).abs() < 1e-9);
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
        profile: None,
        artifact_path: temp.join("artifact.json"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: None,
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
fn reading_rate_runtime_gate_uses_pipeline_runtime_metric() {
    let temp = temp_dir("pipeline_runtime_gate");
    let label = temp.join("image001.txt");
    let image = temp.join("image001.jpg");
    fs::write(&label, "expected").expect("write label");
    fs::write(&image, "not-a-real-image").expect("write invalid image");

    let args = tools::ReadingRateArgs {
        dataset_root: temp,
        profile: None,
        artifact_path: PathBuf::from("unused"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: Some(0.0),
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert_eq!(report.kpi_gate.pass, Some(true));
    assert!(report.kpi_gate.failures.is_empty());
    assert_eq!(report.global.median_pipeline_runtime_ms, 0.0);
    assert_eq!(report.kpi_gate.median_pipeline_runtime_ms.value, 0.0);
    assert!(
        report.global.median_runtime_ms >= report.global.median_pipeline_runtime_ms,
        "end-to-end runtime should include pipeline runtime"
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
        profile: None,
        artifact_path: temp.join("artifact.json"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: Some(0),
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: None,
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert_eq!(report.global.total_cases, 1);
    assert_eq!(report.cases.len(), 1);
    assert_eq!(
        report.cases[0].failure_signature.as_deref(),
        Some("emergency-cutoff")
    );
}

#[test]
fn reading_rate_report_includes_profile_metadata_note() {
    let temp = temp_dir("profile_metadata");
    let label = temp.join("image001.txt");
    let image = temp.join("image001.jpg");
    fs::write(&label, "expected").expect("write label");
    fs::write(&image, "not-a-real-image").expect("write invalid image");

    let args = tools::ReadingRateArgs {
        dataset_root: temp.clone(),
        profile: Some(tools::ReadingRateProfile::MonitorSmoke),
        artifact_path: temp.join("artifact.json"),
        limit: Some(1),
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: None,
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert!(report.notes.iter().any(|note| {
        note.contains("profile=monitor-smoke")
            && note.contains(&format!("resolved_dataset_root={}", temp.display()))
    }));
}

#[test]
fn reading_rate_report_includes_kpi_lane_semantics_note() {
    let temp = temp_dir("kpi_lane_note");
    let label = temp.join("image001.txt");
    let image = temp.join("image001.jpg");
    fs::write(&label, "expected").expect("write label");
    fs::write(&image, "not-a-real-image").expect("write invalid image");

    let boofcv_args = tools::ReadingRateArgs {
        dataset_root: temp.clone(),
        profile: Some(tools::ReadingRateProfile::MonitorSmoke),
        artifact_path: temp.join("boofcv.json"),
        limit: Some(1),
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: None,
    };
    let boofcv_report =
        tools::build_reading_rate_report(&boofcv_args).expect("build boofcv report");
    assert!(
        boofcv_report
            .notes
            .iter()
            .any(|note| note == "kpi_lane=boofcv semantics=annotation-any-decode")
    );

    let strict_args = tools::ReadingRateArgs {
        dataset_root: temp,
        profile: Some(tools::ReadingRateProfile::PayloadValidated),
        artifact_path: PathBuf::from("unused"),
        limit: Some(1),
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: None,
        gate_rotations_rate_min: None,
        gate_high_version_rate_min: None,
        gate_median_runtime_ms_max: None,
    };
    let strict_report =
        tools::build_reading_rate_report(&strict_args).expect("build strict report");
    assert!(
        strict_report
            .notes
            .iter()
            .any(|note| note == "kpi_lane=payload-validated semantics=strict-payload-match")
    );
}

#[test]
fn payload_validated_profile_has_strict_labels() {
    let root =
        tools::reading_rate_profile_dataset_root(tools::ReadingRateProfile::PayloadValidated);
    let cases = tools::discover_label_cases(&root, Some(8)).expect("discover payload cases");
    assert!(!cases.is_empty());
    assert!(
        cases
            .iter()
            .all(|case| !case.expected_payload.trim().is_empty())
    );
    assert!(
        cases
            .iter()
            .all(|case| !case.expected_payload.contains("hand selected 2D points"))
    );
    assert!(cases.iter().all(|case| {
        !case
            .expected_payload
            .lines()
            .any(|line| line.trim() == "SETS")
    }));
}

#[test]
fn reading_rate_report_evaluates_kpi_gate_failures() {
    let temp = temp_dir("kpi_gate_failures");
    let rotations_dir = temp.join("rotations");
    let high_version_dir = temp.join("high_version");
    fs::create_dir_all(&rotations_dir).expect("create rotations dir");
    fs::create_dir_all(&high_version_dir).expect("create high_version dir");

    let rot_label = rotations_dir.join("image001.txt");
    let rot_image = rotations_dir.join("image001.jpg");
    fs::write(&rot_label, "expected-rot").expect("write rotations label");
    fs::write(&rot_image, "invalid-image-bytes").expect("write rotations image");

    let hv_label = high_version_dir.join("image001.txt");
    let hv_image = high_version_dir.join("image001.jpg");
    fs::write(&hv_label, "expected-hv").expect("write high_version label");
    fs::write(&hv_image, "invalid-image-bytes").expect("write high_version image");

    let args = tools::ReadingRateArgs {
        dataset_root: temp,
        profile: None,
        artifact_path: PathBuf::from("unused"),
        limit: None,
        max_working_dim: None,
        emergency_cutoff_ms: None,
        gate_global_rate_min: Some(0.5),
        gate_rotations_rate_min: Some(0.2),
        gate_high_version_rate_min: Some(0.2),
        gate_median_runtime_ms_max: Some(10_000.0),
    };
    let report = tools::build_reading_rate_report(&args).expect("build report");

    assert_eq!(report.kpi_gate.pass, Some(false));
    assert!(
        report
            .kpi_gate
            .failures
            .iter()
            .any(|failure| failure.contains("global_rate"))
    );
    assert!(
        report
            .kpi_gate
            .failures
            .iter()
            .any(|failure| failure.contains("rotations_rate"))
    );
    assert!(
        report
            .kpi_gate
            .failures
            .iter()
            .any(|failure| failure.contains("high_version_rate"))
    );

    let json = tools::report_to_json(&report);
    assert!(json.contains("\"rotations_rate\":{\"value\":"));
    assert!(json.contains("\"high_version_rate\":{\"value\":"));
    assert!(json.contains("\"pass\":false"));
    assert!(json.contains("\"failures\":["));
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
