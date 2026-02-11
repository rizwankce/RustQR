use image::{DynamicImage, imageops::FilterType, io::Reader as ImageReader};
use rust_qr::{DetectConfig, pipeline};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_DATASET_ROOT: &str = "benches/images/boofcv";
pub const DEFAULT_ARTIFACT_PATH: &str = "target/reading_rate_report.json";
pub const IMAGE_LOAD_FAILURE_SIGNATURE: &str = "image-load-fail";
pub const PAYLOAD_MISMATCH_SIGNATURE: &str = "payload-mismatch";
pub const MONITOR_SMOKE_PROFILE: &str = "monitor-smoke";
pub const NOMINAL_SMOKE_PROFILE: &str = "nominal-smoke";

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingRateCommand {
    Help,
    Run(ReadingRateArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingRateProfile {
    MonitorSmoke,
    NominalSmoke,
}

impl ReadingRateProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MonitorSmoke => MONITOR_SMOKE_PROFILE,
            Self::NominalSmoke => NOMINAL_SMOKE_PROFILE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingRateArgs {
    pub dataset_root: PathBuf,
    pub profile: Option<ReadingRateProfile>,
    pub artifact_path: PathBuf,
    pub limit: Option<usize>,
    pub max_working_dim: Option<usize>,
    pub emergency_cutoff_ms: Option<u64>,
}

impl Default for ReadingRateArgs {
    fn default() -> Self {
        Self {
            dataset_root: PathBuf::from(DEFAULT_DATASET_ROOT),
            profile: None,
            artifact_path: PathBuf::from(DEFAULT_ARTIFACT_PATH),
            limit: None,
            max_working_dim: None,
            emergency_cutoff_ms: None,
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

pub fn parse_reading_rate_profile(raw: &str) -> Result<ReadingRateProfile, String> {
    match raw {
        MONITOR_SMOKE_PROFILE => Ok(ReadingRateProfile::MonitorSmoke),
        NOMINAL_SMOKE_PROFILE => Ok(ReadingRateProfile::NominalSmoke),
        _ => Err(format!(
            "unknown --profile value: {raw}; expected one of: {MONITOR_SMOKE_PROFILE}, {NOMINAL_SMOKE_PROFILE}"
        )),
    }
}

pub fn reading_rate_profile_dataset_root(profile: ReadingRateProfile) -> PathBuf {
    match profile {
        ReadingRateProfile::MonitorSmoke => PathBuf::from("benches/images/boofcv/monitor"),
        ReadingRateProfile::NominalSmoke => PathBuf::from("benches/images/boofcv/nominal"),
    }
}

pub fn parse_reading_rate_args(args: &[String]) -> Result<ReadingRateCommand, String> {
    let mut parsed = ReadingRateArgs::default();
    let mut explicit_dataset_root: Option<PathBuf> = None;
    let mut selected_profile: Option<ReadingRateProfile> = None;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => return Ok(ReadingRateCommand::Help),
            "--dataset-root" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--dataset-root requires a value".to_string())?;
                explicit_dataset_root = Some(PathBuf::from(value));
            }
            "--profile" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--profile requires a value".to_string())?;
                selected_profile = Some(parse_reading_rate_profile(value)?);
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
            "--max-working-dim" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--max-working-dim requires a value".to_string())?;
                let parsed_dim = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --max-working-dim value: {value}"))?;
                parsed.max_working_dim = Some(parsed_dim);
            }
            "--emergency-cutoff-ms" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--emergency-cutoff-ms requires a value".to_string())?;
                let parsed_cutoff = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --emergency-cutoff-ms value: {value}"))?;
                parsed.emergency_cutoff_ms = Some(parsed_cutoff);
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

    if let Some(dataset_root) = explicit_dataset_root {
        parsed.dataset_root = dataset_root;
    } else if let Some(profile) = selected_profile {
        parsed.dataset_root = reading_rate_profile_dataset_root(profile);
    }
    parsed.profile = selected_profile;

    Ok(ReadingRateCommand::Run(parsed))
}

pub fn reading_rate_usage() -> &'static str {
    "usage: qrtool reading-rate [--profile monitor-smoke|nominal-smoke] [--dataset-root PATH] [--artifact PATH] [--limit N] [--max-working-dim N] [--emergency-cutoff-ms N]"
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
        let mut components = relative.components();
        if let Some(first) = components.next() {
            if components.next().is_some() {
                let first = first.as_os_str().to_string_lossy();
                if !first.is_empty() {
                    return first.to_string();
                }
            }
        }
    }

    if let Some(name) = dataset_root.file_name().and_then(|value| value.to_str()) {
        if !name.is_empty() {
            return name.to_string();
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

pub fn evaluate_cases(
    cases: Vec<LabelCase>,
    detect_config: &DetectConfig,
    decode_resize_max_dim: Option<usize>,
) -> Vec<CaseOutcome> {
    cases
        .into_iter()
        .map(|case| evaluate_case(case, detect_config, decode_resize_max_dim))
        .collect()
}

fn evaluate_case(
    case: LabelCase,
    detect_config: &DetectConfig,
    decode_resize_max_dim: Option<usize>,
) -> CaseOutcome {
    let case_start = Instant::now();
    let expected_payload = normalize_payload(&case.expected_payload);
    let annotation_label_mode = is_point_annotation_label(&case.expected_payload);

    let (image, width, height) = match load_rgb_image(&case.image_path, decode_resize_max_dim) {
        Ok(decoded) => decoded,
        Err(_) => {
            return CaseOutcome {
                category: case.category,
                label_path: case.label_path,
                image_path: case.image_path,
                expected_payload: case.expected_payload,
                matched: false,
                runtime_ms: elapsed_ms(case_start),
                failure_signature: Some(IMAGE_LOAD_FAILURE_SIGNATURE.to_string()),
            };
        }
    };

    let report = pipeline::detect_with_config(&image, width, height, detect_config);
    let matched = if annotation_label_mode {
        !report.codes.is_empty()
    } else {
        report
            .codes
            .iter()
            .any(|code| normalize_payload(&code.payload) == expected_payload)
    };

    let failure_signature = if matched {
        None
    } else if report.codes.is_empty() {
        report
            .failure_signature
            .or_else(|| Some("no-decode-yet".to_string()))
    } else {
        Some(PAYLOAD_MISMATCH_SIGNATURE.to_string())
    };

    CaseOutcome {
        category: case.category,
        label_path: case.label_path,
        image_path: case.image_path,
        expected_payload: case.expected_payload,
        matched,
        runtime_ms: elapsed_ms(case_start),
        failure_signature,
    }
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

pub fn build_reading_rate_report(args: &ReadingRateArgs) -> Result<ReadingRateReport, String> {
    let cases = discover_label_cases(&args.dataset_root, args.limit)
        .map_err(|err| format!("failed to discover label cases: {err}"))?;

    let detect_config = reading_rate_detect_config(args);
    let outcomes = evaluate_cases(cases, &detect_config, args.max_working_dim);
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

    let mut notes = vec![
        "reading-rate mode: real image decode + payload match evaluation".to_string(),
        "decode core is still scaffold quality; benchmark rate is expected to be low until real decoder lands".to_string(),
    ];
    if let Some(profile) = args.profile {
        notes.push(format!(
            "reading-rate profile={} resolved_dataset_root={}",
            profile.as_str(),
            args.dataset_root.display()
        ));
    }

    Ok(ReadingRateReport {
        dataset_root: args.dataset_root.clone(),
        generated_at_unix_ms,
        categories,
        global,
        kpi_gate,
        notes,
        cases: outcomes,
    })
}

fn reading_rate_detect_config(args: &ReadingRateArgs) -> DetectConfig {
    let mut config = DetectConfig::default();
    if let Some(max_working_dim) = args.max_working_dim {
        config.max_working_dim = max_working_dim;
    }
    if let Some(emergency_cutoff_ms) = args.emergency_cutoff_ms {
        config.emergency_cutoff_ms = emergency_cutoff_ms;
    }
    config
}

pub(crate) fn load_rgb_image(
    path: &Path,
    decode_resize_max_dim: Option<usize>,
) -> Result<(Vec<u8>, usize, usize), String> {
    let reader = ImageReader::open(path)
        .map_err(|err| format!("failed to open image {}: {err}", path.display()))?;
    let decoded = reader
        .decode()
        .map_err(|err| format!("failed to decode image {}: {err}", path.display()))?;
    let resized = maybe_resize_decoded_image(decoded, decode_resize_max_dim);
    let rgb = resized.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok((rgb.into_raw(), width as usize, height as usize))
}

fn maybe_resize_decoded_image(
    image: DynamicImage,
    decode_resize_max_dim: Option<usize>,
) -> DynamicImage {
    let Some(limit) = decode_resize_max_dim else {
        return image;
    };
    if limit == 0 {
        return image;
    }

    let width = image.width();
    let height = image.height();
    let source_max_dim = width.max(height) as usize;
    if source_max_dim <= limit {
        return image;
    }

    let scale = limit as f64 / source_max_dim as f64;
    let resized_width = ((width as f64) * scale).round().max(1.0) as u32;
    let resized_height = ((height as f64) * scale).round().max(1.0) as u32;
    image.resize_exact(resized_width, resized_height, FilterType::Triangle)
}

fn normalize_payload(payload: &str) -> String {
    payload.trim().replace("\r\n", "\n")
}

fn is_point_annotation_label(raw: &str) -> bool {
    let normalized = normalize_payload(raw);
    if normalized.is_empty() {
        return false;
    }
    if normalized
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("SETS"))
    {
        return true;
    }
    normalized
        .lines()
        .next()
        .map(|line| {
            line.to_ascii_lowercase()
                .contains("hand selected 2d points")
        })
        .unwrap_or(false)
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
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
