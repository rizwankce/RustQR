#[allow(dead_code)]
#[path = "../src/tools/mod.rs"]
mod tools;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct GlobalFixture<'a> {
    total_cases: usize,
    matched_cases: usize,
    reading_rate: f64,
    median_runtime_ms: f64,
    top_failure_signature: Option<&'a str>,
}

struct CategoryFixture<'a> {
    category: &'a str,
    total_cases: usize,
    matched_cases: usize,
    reading_rate: f64,
    median_runtime_ms: f64,
    top_failure_signature: Option<&'a str>,
}

#[test]
fn parse_benchdiff_defaults_and_help() {
    let args = vec![
        "--base".to_string(),
        "target/base.json".to_string(),
        "--candidate".to_string(),
        "target/candidate.json".to_string(),
    ];

    let parsed = tools::parse_benchdiff_args(&args).expect("parse should succeed");
    let tools::BenchdiffCommand::Run(parsed) = parsed else {
        panic!("expected run command");
    };

    assert_eq!(parsed.base_path, PathBuf::from("target/base.json"));
    assert_eq!(
        parsed.candidate_path,
        PathBuf::from("target/candidate.json")
    );
    assert_eq!(
        parsed.artifact_path,
        PathBuf::from(tools::DEFAULT_BENCHDIFF_ARTIFACT_PATH)
    );

    let help = vec!["--help".to_string()];
    let parsed_help = tools::parse_benchdiff_args(&help).expect("help should parse");
    assert_eq!(parsed_help, tools::BenchdiffCommand::Help);
}

#[test]
fn parse_benchdiff_rejects_missing_required_and_unknown_args() {
    let missing_candidate = vec!["--base".to_string(), "base.json".to_string()];
    let missing_candidate_err =
        tools::parse_benchdiff_args(&missing_candidate).expect_err("expected parse error");
    assert!(missing_candidate_err.contains("missing required --candidate"));

    let unknown = vec![
        "--base".to_string(),
        "base.json".to_string(),
        "--candidate".to_string(),
        "candidate.json".to_string(),
        "--wat".to_string(),
    ];
    let unknown_err = tools::parse_benchdiff_args(&unknown).expect_err("expected parse error");
    assert!(unknown_err.contains("unknown argument for benchdiff"));
}

#[test]
fn build_benchdiff_report_computes_deltas_and_highlights() {
    let temp = temp_dir("build_benchdiff");
    let base_path = temp.join("base.json");
    let candidate_path = temp.join("candidate.json");

    write_reading_rate_artifact(
        &base_path,
        GlobalFixture {
            total_cases: 100,
            matched_cases: 60,
            reading_rate: 0.60,
            median_runtime_ms: 500.0,
            top_failure_signature: Some("format-fail"),
        },
        &[
            CategoryFixture {
                category: "nominal",
                total_cases: 50,
                matched_cases: 30,
                reading_rate: 0.60,
                median_runtime_ms: 400.0,
                top_failure_signature: Some("format-fail"),
            },
            CategoryFixture {
                category: "rotations",
                total_cases: 20,
                matched_cases: 10,
                reading_rate: 0.50,
                median_runtime_ms: 600.0,
                top_failure_signature: Some("no-decode-yet"),
            },
        ],
    );

    write_reading_rate_artifact(
        &candidate_path,
        GlobalFixture {
            total_cases: 100,
            matched_cases: 70,
            reading_rate: 0.70,
            median_runtime_ms: 450.0,
            top_failure_signature: Some("no-decode-yet"),
        },
        &[
            CategoryFixture {
                category: "nominal",
                total_cases: 50,
                matched_cases: 38,
                reading_rate: 0.76,
                median_runtime_ms: 380.0,
                top_failure_signature: None,
            },
            CategoryFixture {
                category: "high_version",
                total_cases: 30,
                matched_cases: 15,
                reading_rate: 0.50,
                median_runtime_ms: 520.0,
                top_failure_signature: Some("payload-mismatch"),
            },
        ],
    );

    let args = tools::BenchdiffArgs {
        base_path: base_path.clone(),
        candidate_path: candidate_path.clone(),
        artifact_path: temp.join("diff.json"),
    };
    let report = tools::build_benchdiff_report(&args).expect("build benchdiff report");

    assert_eq!(report.base_artifact_path, base_path);
    assert_eq!(report.candidate_artifact_path, candidate_path);
    assert_eq!(report.global.total_cases_base, 100);
    assert_eq!(report.global.total_cases_candidate, 100);
    assert!((report.global.reading_rate.delta - 0.10).abs() < 1e-9);
    assert!((report.global.median_runtime_ms.delta + 50.0).abs() < 1e-9);
    assert_eq!(
        report.global.top_failure_signature.base.as_deref(),
        Some("format-fail")
    );
    assert_eq!(
        report.global.top_failure_signature.candidate.as_deref(),
        Some("no-decode-yet")
    );

    let nominal = report
        .categories
        .iter()
        .find(|summary| summary.category == "nominal")
        .expect("nominal category");
    assert!((nominal.reading_rate.delta - 0.16).abs() < 1e-9);
    assert!((nominal.median_runtime_ms.delta + 20.0).abs() < 1e-9);
    assert_eq!(nominal.matched_cases_base, 30);
    assert_eq!(nominal.matched_cases_candidate, 38);

    let added = report
        .categories
        .iter()
        .find(|summary| summary.category == "high_version")
        .expect("high_version category");
    assert_eq!(added.total_cases_base, 0);
    assert_eq!(added.total_cases_candidate, 30);
    assert!((added.reading_rate.delta - 0.50).abs() < 1e-9);

    let missing = report
        .categories
        .iter()
        .find(|summary| summary.category == "rotations")
        .expect("rotations category");
    assert_eq!(missing.total_cases_base, 20);
    assert_eq!(missing.total_cases_candidate, 0);
    assert!((missing.reading_rate.delta + 0.50).abs() < 1e-9);

    assert_eq!(report.top_improvements[0].category, "high_version");
    assert_eq!(report.top_regressions[0].category, "rotations");
    assert!(
        report
            .notes
            .contains(&"category added in candidate artifact: high_version".to_string())
    );
    assert!(
        report
            .notes
            .contains(&"category missing in candidate artifact: rotations".to_string())
    );
}

#[test]
fn build_benchdiff_report_rejects_duplicate_categories() {
    let temp = temp_dir("duplicate_categories");
    let base_path = temp.join("base.json");
    let candidate_path = temp.join("candidate.json");

    write_reading_rate_artifact(
        &base_path,
        GlobalFixture {
            total_cases: 5,
            matched_cases: 2,
            reading_rate: 0.40,
            median_runtime_ms: 100.0,
            top_failure_signature: None,
        },
        &[
            CategoryFixture {
                category: "nominal",
                total_cases: 5,
                matched_cases: 2,
                reading_rate: 0.40,
                median_runtime_ms: 100.0,
                top_failure_signature: None,
            },
            CategoryFixture {
                category: "nominal",
                total_cases: 5,
                matched_cases: 2,
                reading_rate: 0.40,
                median_runtime_ms: 100.0,
                top_failure_signature: None,
            },
        ],
    );

    write_reading_rate_artifact(
        &candidate_path,
        GlobalFixture {
            total_cases: 5,
            matched_cases: 3,
            reading_rate: 0.60,
            median_runtime_ms: 90.0,
            top_failure_signature: None,
        },
        &[CategoryFixture {
            category: "nominal",
            total_cases: 5,
            matched_cases: 3,
            reading_rate: 0.60,
            median_runtime_ms: 90.0,
            top_failure_signature: None,
        }],
    );

    let args = tools::BenchdiffArgs {
        base_path,
        candidate_path,
        artifact_path: temp.join("diff.json"),
    };
    let err = tools::build_benchdiff_report(&args).expect_err("expected duplicate category error");
    assert!(err.contains("duplicate category summary"));
}

#[test]
fn benchdiff_json_schema_is_machine_readable() {
    let temp = temp_dir("benchdiff_json");
    let base_path = temp.join("base.json");
    let candidate_path = temp.join("candidate.json");

    write_reading_rate_artifact(
        &base_path,
        GlobalFixture {
            total_cases: 10,
            matched_cases: 5,
            reading_rate: 0.50,
            median_runtime_ms: 300.0,
            top_failure_signature: Some("x"),
        },
        &[CategoryFixture {
            category: "nominal",
            total_cases: 10,
            matched_cases: 5,
            reading_rate: 0.50,
            median_runtime_ms: 300.0,
            top_failure_signature: Some("x"),
        }],
    );
    write_reading_rate_artifact(
        &candidate_path,
        GlobalFixture {
            total_cases: 10,
            matched_cases: 7,
            reading_rate: 0.70,
            median_runtime_ms: 250.0,
            top_failure_signature: Some("y"),
        },
        &[CategoryFixture {
            category: "nominal",
            total_cases: 10,
            matched_cases: 7,
            reading_rate: 0.70,
            median_runtime_ms: 250.0,
            top_failure_signature: Some("y"),
        }],
    );

    let report = tools::build_benchdiff_report(&tools::BenchdiffArgs {
        base_path,
        candidate_path,
        artifact_path: temp.join("diff.json"),
    })
    .expect("build benchdiff report");

    let json = tools::benchdiff_to_json(&report);
    assert!(json.contains("\"schema_version\":\"wp015-benchdiff-v1\""));
    assert!(json.contains(
        "\"global\":{\"total_cases\":{\"base\":10,\"candidate\":10,\"delta\":0},\"matched_cases\":{\"base\":5,\"candidate\":7,\"delta\":2},\"reading_rate\":{\"base\":0.500000,\"candidate\":0.700000,\"delta\":0.200000}"
    ));
    assert!(json.contains(
        "\"categories\":[{\"category\":\"nominal\",\"total_cases\":{\"base\":10,\"candidate\":10,\"delta\":0}"
    ));
    assert!(json.contains(
        "\"top_improvements\":[{\"category\":\"nominal\",\"reading_rate_delta\":0.200000,\"median_runtime_delta_ms\":-50.000000}]"
    ));
    assert!(json.contains("\"top_regressions\":[]"));
}

fn write_reading_rate_artifact(
    path: &Path,
    global: GlobalFixture<'_>,
    categories: &[CategoryFixture<'_>],
) {
    fn escape_json(raw: &str) -> String {
        let mut escaped = String::with_capacity(raw.len());
        for ch in raw.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                c if c <= '\u{1F}' => {
                    let _ = std::fmt::Write::write_fmt(
                        &mut escaped,
                        format_args!("\\u{:04X}", c as u32),
                    );
                }
                c => escaped.push(c),
            }
        }
        escaped
    }

    fn quoted(value: &str) -> String {
        format!("\"{}\"", escape_json(value))
    }

    fn optional_string(value: Option<&str>) -> String {
        match value {
            Some(v) => quoted(v),
            None => "null".to_string(),
        }
    }

    let categories_json = categories
        .iter()
        .map(|category| {
            format!(
                "{{\"category\":{},\"total_cases\":{},\"matched_cases\":{},\"reading_rate\":{:.6},\"median_runtime_ms\":{:.6},\"top_failure_signature\":{}}}",
                quoted(category.category),
                category.total_cases,
                category.matched_cases,
                category.reading_rate,
                category.median_runtime_ms,
                optional_string(category.top_failure_signature),
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let json = format!(
        concat!(
            "{{",
            "\"schema_version\":\"wp007-reading-rate-v1\",",
            "\"global\":{{",
            "\"total_cases\":{},",
            "\"matched_cases\":{},",
            "\"reading_rate\":{:.6},",
            "\"median_runtime_ms\":{:.6},",
            "\"top_failure_signature\":{}",
            "}},",
            "\"categories\":[{}]",
            "}}"
        ),
        global.total_cases,
        global.matched_cases,
        global.reading_rate,
        global.median_runtime_ms,
        optional_string(global.top_failure_signature),
        categories_json,
    );

    fs::write(path, json).expect("write reading-rate artifact");
}

fn temp_dir(suffix: &str) -> PathBuf {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "rustqr_wp015_{suffix}_{}_{}",
        std::process::id(),
        seed
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
