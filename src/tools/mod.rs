use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_DATASET_ROOT: &str = "benches/images/boofcv";
pub const DEFAULT_ARTIFACT_PATH: &str = "target/reading_rate_report.json";
pub const SCAFFOLD_FAILURE_SIGNATURE: &str = "scaffold-image-decode-unimplemented";

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingRateCommand {
    Help,
    Run(ReadingRateArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingRateArgs {
    pub dataset_root: PathBuf,
    pub artifact_path: PathBuf,
    pub limit: Option<usize>,
}

impl Default for ReadingRateArgs {
    fn default() -> Self {
        Self {
            dataset_root: PathBuf::from(DEFAULT_DATASET_ROOT),
            artifact_path: PathBuf::from(DEFAULT_ARTIFACT_PATH),
            limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelCase {
    pub category: String,
    pub label_path: PathBuf,
    pub image_path: PathBuf,
    pub expected_payload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseOutcome {
    pub category: String,
    pub label_path: PathBuf,
    pub image_path: PathBuf,
    pub expected_payload: String,
    pub matched: bool,
    pub runtime_ms: f64,
    pub failure_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategorySummary {
    pub category: String,
    pub total_cases: usize,
    pub matched_cases: usize,
    pub reading_rate: f64,
    pub median_runtime_ms: f64,
    pub top_failure_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalSummary {
    pub total_cases: usize,
    pub matched_cases: usize,
    pub reading_rate: f64,
    pub median_runtime_ms: f64,
    pub top_failure_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KpiGatePlaceholders {
    pub global_rate: f64,
    pub median_runtime_ms: f64,
    pub top_failure_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadingRateReport {
    pub dataset_root: PathBuf,
    pub generated_at_unix_ms: u128,
    pub categories: Vec<CategorySummary>,
    pub global: GlobalSummary,
    pub kpi_gate: KpiGatePlaceholders,
    pub notes: Vec<String>,
    pub cases: Vec<CaseOutcome>,
}

pub fn parse_reading_rate_args(args: &[String]) -> Result<ReadingRateCommand, String> {
    let mut parsed = ReadingRateArgs::default();

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => return Ok(ReadingRateCommand::Help),
            "--dataset-root" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--dataset-root requires a value".to_string())?;
                parsed.dataset_root = PathBuf::from(value);
            }
            "--artifact" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--artifact requires a value".to_string())?;
                parsed.artifact_path = PathBuf::from(value);
            }
            "--limit" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--limit requires a value".to_string())?;
                let parsed_limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --limit value: {value}"))?;
                parsed.limit = if parsed_limit == 0 {
                    None
                } else {
                    Some(parsed_limit)
                };
            }
            unknown => {
                return Err(format!(
                    "unknown argument for reading-rate: {unknown}\n{}",
                    reading_rate_usage()
                ));
            }
        }
        idx += 1;
    }

    Ok(ReadingRateCommand::Run(parsed))
}

pub fn reading_rate_usage() -> &'static str {
    "usage: qrtool reading-rate [--dataset-root PATH] [--artifact PATH] [--limit N]"
}

pub fn discover_label_cases(
    dataset_root: &Path,
    limit: Option<usize>,
) -> io::Result<Vec<LabelCase>> {
    let mut label_files = Vec::new();
    collect_label_files(dataset_root, &mut label_files)?;
    label_files.sort();

    let mut cases = Vec::new();
    for label_path in label_files {
        if let Some(max) = limit {
            if cases.len() >= max {
                break;
            }
        }

        let Some(image_path) = paired_image_path(&label_path) else {
            continue;
        };

        let expected_payload = fs::read_to_string(&label_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        cases.push(LabelCase {
            category: category_from_label_path(dataset_root, &label_path),
            label_path,
            image_path,
            expected_payload,
        });
    }

    Ok(cases)
}

pub fn category_from_label_path(dataset_root: &Path, label_path: &Path) -> String {
    if let Ok(relative) = label_path.strip_prefix(dataset_root) {
        if let Some(first) = relative.components().next() {
            let first = first.as_os_str().to_string_lossy();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }

    "uncategorized".to_string()
}

pub fn paired_image_path(label_path: &Path) -> Option<PathBuf> {
    for extension in IMAGE_EXTENSIONS {
        let candidate = label_path.with_extension(extension);
        if candidate.is_file() {
            return Some(candidate);
        }

        let uppercase = label_path.with_extension(extension.to_ascii_uppercase());
        if uppercase.is_file() {
            return Some(uppercase);
        }
    }

    None
}

pub fn scaffold_outcomes(cases: Vec<LabelCase>) -> Vec<CaseOutcome> {
    cases
        .into_iter()
        .map(|case| CaseOutcome {
            category: case.category,
            label_path: case.label_path,
            image_path: case.image_path,
            expected_payload: case.expected_payload,
            matched: false,
            runtime_ms: 0.0,
            failure_signature: Some(SCAFFOLD_FAILURE_SIGNATURE.to_string()),
        })
        .collect()
}

pub fn median_runtime(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    }
}

pub fn top_failure_signature<'a, I>(signatures: I) -> Option<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for signature in signatures {
        *counts.entry(signature).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .max_by(|(left_key, left_count), (right_key, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_key.cmp(left_key))
        })
        .map(|(value, _)| value.to_string())
}

pub fn summarize_outcomes(outcomes: &[CaseOutcome]) -> (Vec<CategorySummary>, GlobalSummary) {
    let mut grouped: BTreeMap<&str, Vec<&CaseOutcome>> = BTreeMap::new();
    for outcome in outcomes {
        grouped
            .entry(outcome.category.as_str())
            .or_default()
            .push(outcome);
    }

    let categories = grouped
        .into_iter()
        .map(|(category, rows)| {
            let total_cases = rows.len();
            let matched_cases = rows.iter().filter(|row| row.matched).count();
            let reading_rate = if total_cases == 0 {
                0.0
            } else {
                matched_cases as f64 / total_cases as f64
            };
            let runtimes: Vec<f64> = rows.iter().map(|row| row.runtime_ms).collect();
            let top_failure_signature = top_failure_signature(
                rows.iter()
                    .filter_map(|row| row.failure_signature.as_deref()),
            );

            CategorySummary {
                category: category.to_string(),
                total_cases,
                matched_cases,
                reading_rate,
                median_runtime_ms: median_runtime(&runtimes),
                top_failure_signature,
            }
        })
        .collect::<Vec<_>>();

    let total_cases = outcomes.len();
    let matched_cases = outcomes.iter().filter(|row| row.matched).count();
    let reading_rate = if total_cases == 0 {
        0.0
    } else {
        matched_cases as f64 / total_cases as f64
    };
    let all_runtimes: Vec<f64> = outcomes.iter().map(|row| row.runtime_ms).collect();

    let global = GlobalSummary {
        total_cases,
        matched_cases,
        reading_rate,
        median_runtime_ms: median_runtime(&all_runtimes),
        top_failure_signature: top_failure_signature(
            outcomes
                .iter()
                .filter_map(|row| row.failure_signature.as_deref()),
        ),
    };

    (categories, global)
}

pub fn build_scaffold_report(args: &ReadingRateArgs) -> Result<ReadingRateReport, String> {
    let cases = discover_label_cases(&args.dataset_root, args.limit)
        .map_err(|err| format!("failed to discover label cases: {err}"))?;

    let outcomes = scaffold_outcomes(cases);
    let (categories, global) = summarize_outcomes(&outcomes);

    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let kpi_gate = KpiGatePlaceholders {
        global_rate: global.reading_rate,
        median_runtime_ms: global.median_runtime_ms,
        top_failure_signature: global.top_failure_signature.clone(),
    };

    Ok(ReadingRateReport {
        dataset_root: args.dataset_root.clone(),
        generated_at_unix_ms,
        categories,
        global,
        kpi_gate,
        notes: vec![
            "scaffold mode: image decode execution is not wired yet".to_string(),
            format!("failure signature placeholder: {SCAFFOLD_FAILURE_SIGNATURE}"),
        ],
        cases: outcomes,
    })
}

pub fn render_console_summary(report: &ReadingRateReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "reading-rate dataset_root={} cases={} matched={} rate={:.4} median_runtime_ms={:.3} top_failure_signature={}",
        report.dataset_root.display(),
        report.global.total_cases,
        report.global.matched_cases,
        report.global.reading_rate,
        report.global.median_runtime_ms,
        report
            .global
            .top_failure_signature
            .as_deref()
            .unwrap_or("none")
    ));

    for category in &report.categories {
        lines.push(format!(
            "category={} cases={} matched={} rate={:.4} median_runtime_ms={:.3} top_failure_signature={}",
            category.category,
            category.total_cases,
            category.matched_cases,
            category.reading_rate,
            category.median_runtime_ms,
            category
                .top_failure_signature
                .as_deref()
                .unwrap_or("none")
        ));
    }

    for note in &report.notes {
        lines.push(format!("note={note}"));
    }

    lines.join("\n")
}

pub fn report_to_json(report: &ReadingRateReport) -> String {
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

    fn optional_string(value: &Option<String>) -> String {
        match value {
            Some(v) => quoted(v),
            None => "null".to_string(),
        }
    }

    fn format_path(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    let categories_json = report
        .categories
        .iter()
        .map(|category| {
            format!(
                "{{\"category\":{},\"total_cases\":{},\"matched_cases\":{},\"reading_rate\":{:.6},\"median_runtime_ms\":{:.6},\"top_failure_signature\":{}}}",
                quoted(&category.category),
                category.total_cases,
                category.matched_cases,
                category.reading_rate,
                category.median_runtime_ms,
                optional_string(&category.top_failure_signature),
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let notes_json = report
        .notes
        .iter()
        .map(|note| quoted(note))
        .collect::<Vec<_>>()
        .join(",");

    let cases_json = report
        .cases
        .iter()
        .map(|case| {
            format!(
                "{{\"category\":{},\"label_path\":{},\"image_path\":{},\"expected_payload\":{},\"matched\":{},\"runtime_ms\":{:.6},\"failure_signature\":{}}}",
                quoted(&case.category),
                quoted(&format_path(&case.label_path)),
                quoted(&format_path(&case.image_path)),
                quoted(&case.expected_payload),
                case.matched,
                case.runtime_ms,
                optional_string(&case.failure_signature),
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"schema_version\":\"wp007-reading-rate-v1\",",
            "\"generated_at_unix_ms\":{},",
            "\"dataset_root\":{},",
            "\"global\":{{",
            "\"total_cases\":{},",
            "\"matched_cases\":{},",
            "\"reading_rate\":{:.6},",
            "\"median_runtime_ms\":{:.6},",
            "\"top_failure_signature\":{}",
            "}},",
            "\"categories\":[{}],",
            "\"kpi_gate\":{{",
            "\"global_rate\":{{\"value\":{:.6},\"threshold_min\":null,\"pass\":null}},",
            "\"median_runtime_ms\":{{\"value\":{:.6},\"threshold_max\":null,\"pass\":null}},",
            "\"top_failure_signature\":{{\"value\":{},\"blocked_signatures\":[],\"pass\":null}}",
            "}},",
            "\"notes\":[{}],",
            "\"cases\":[{}]",
            "}}"
        ),
        report.generated_at_unix_ms,
        quoted(&format_path(&report.dataset_root)),
        report.global.total_cases,
        report.global.matched_cases,
        report.global.reading_rate,
        report.global.median_runtime_ms,
        optional_string(&report.global.top_failure_signature),
        categories_json,
        report.kpi_gate.global_rate,
        report.kpi_gate.median_runtime_ms,
        optional_string(&report.kpi_gate.top_failure_signature),
        notes_json,
        cases_json,
    )
}

pub fn write_report_json(artifact_path: &Path, json: &str) -> io::Result<()> {
    if let Some(parent) = artifact_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::write(artifact_path, json)
}

fn collect_label_files(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_label_files(&path, out)?;
            continue;
        }

        let is_txt = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("txt"))
            .unwrap_or(false);

        if !is_txt {
            continue;
        }

        let is_control_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('_'))
            .unwrap_or(false);

        if is_control_file {
            continue;
        }

        out.push(path);
    }

    Ok(())
}
