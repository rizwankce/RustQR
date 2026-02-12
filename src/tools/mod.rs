use image::{DynamicImage, imageops::FilterType, io::Reader as ImageReader};
use rust_qr::{DetectConfig, pipeline};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_DATASET_ROOT: &str = "benches/images/boofcv";
pub const DEFAULT_ARTIFACT_PATH: &str = "target/reading_rate_report.json";
pub const DEFAULT_BENCHDIFF_ARTIFACT_PATH: &str = "target/benchdiff_report.json";
pub const IMAGE_LOAD_FAILURE_SIGNATURE: &str = "image-load-fail";
pub const PAYLOAD_MISMATCH_SIGNATURE: &str = "payload-mismatch";
pub const MONITOR_SMOKE_PROFILE: &str = "monitor-smoke";
pub const NOMINAL_SMOKE_PROFILE: &str = "nominal-smoke";
pub const PAYLOAD_VALIDATED_PROFILE: &str = "payload-validated";
pub const BOOFCV_ALL_PROFILE: &str = "boofcv-all";
pub const BOOFCV_BLURRED_PROFILE: &str = "boofcv-blurred";
pub const BOOFCV_BRIGHTNESS_PROFILE: &str = "boofcv-brightness";
pub const BOOFCV_BRIGHT_SPOTS_PROFILE: &str = "boofcv-bright-spots";
pub const BOOFCV_CLOSE_PROFILE: &str = "boofcv-close";
pub const BOOFCV_CURVED_PROFILE: &str = "boofcv-curved";
pub const BOOFCV_DAMAGED_PROFILE: &str = "boofcv-damaged";
pub const BOOFCV_GLARE_PROFILE: &str = "boofcv-glare";
pub const BOOFCV_HIGH_VERSION_PROFILE: &str = "boofcv-high-version";
pub const BOOFCV_LOTS_PROFILE: &str = "boofcv-lots";
pub const BOOFCV_MONITOR_PROFILE: &str = "boofcv-monitor";
pub const BOOFCV_NOMINAL_PROFILE: &str = "boofcv-nominal";
pub const BOOFCV_NONCOMPLIANT_PROFILE: &str = "boofcv-noncompliant";
pub const BOOFCV_PATHOLOGICAL_PROFILE: &str = "boofcv-pathological";
pub const BOOFCV_PERSPECTIVE_PROFILE: &str = "boofcv-perspective";
pub const BOOFCV_ROTATIONS_PROFILE: &str = "boofcv-rotations";
pub const BOOFCV_SHADOWS_PROFILE: &str = "boofcv-shadows";

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp"];
const MAX_BENCHDIFF_HIGHLIGHTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingRateCommand {
    Help,
    Run(ReadingRateArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingRateProfile {
    BoofcvAll,
    BoofcvBlurred,
    BoofcvBrightness,
    BoofcvBrightSpots,
    BoofcvClose,
    BoofcvCurved,
    BoofcvDamaged,
    BoofcvGlare,
    BoofcvHighVersion,
    BoofcvLots,
    BoofcvMonitor,
    BoofcvNominal,
    BoofcvNoncompliant,
    BoofcvPathological,
    BoofcvPerspective,
    BoofcvRotations,
    BoofcvShadows,
    MonitorSmoke,
    NominalSmoke,
    PayloadValidated,
}

impl ReadingRateProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BoofcvAll => BOOFCV_ALL_PROFILE,
            Self::BoofcvBlurred => BOOFCV_BLURRED_PROFILE,
            Self::BoofcvBrightness => BOOFCV_BRIGHTNESS_PROFILE,
            Self::BoofcvBrightSpots => BOOFCV_BRIGHT_SPOTS_PROFILE,
            Self::BoofcvClose => BOOFCV_CLOSE_PROFILE,
            Self::BoofcvCurved => BOOFCV_CURVED_PROFILE,
            Self::BoofcvDamaged => BOOFCV_DAMAGED_PROFILE,
            Self::BoofcvGlare => BOOFCV_GLARE_PROFILE,
            Self::BoofcvHighVersion => BOOFCV_HIGH_VERSION_PROFILE,
            Self::BoofcvLots => BOOFCV_LOTS_PROFILE,
            Self::BoofcvMonitor => BOOFCV_MONITOR_PROFILE,
            Self::BoofcvNominal => BOOFCV_NOMINAL_PROFILE,
            Self::BoofcvNoncompliant => BOOFCV_NONCOMPLIANT_PROFILE,
            Self::BoofcvPathological => BOOFCV_PATHOLOGICAL_PROFILE,
            Self::BoofcvPerspective => BOOFCV_PERSPECTIVE_PROFILE,
            Self::BoofcvRotations => BOOFCV_ROTATIONS_PROFILE,
            Self::BoofcvShadows => BOOFCV_SHADOWS_PROFILE,
            Self::MonitorSmoke => MONITOR_SMOKE_PROFILE,
            Self::NominalSmoke => NOMINAL_SMOKE_PROFILE,
            Self::PayloadValidated => PAYLOAD_VALIDATED_PROFILE,
        }
    }
}

#[cfg(test)]
#[allow(dead_code)] // Referenced by integration harness tests via #[path]-included module.
pub const ALL_READING_RATE_PROFILES: &[ReadingRateProfile] = &[
    ReadingRateProfile::BoofcvAll,
    ReadingRateProfile::BoofcvBlurred,
    ReadingRateProfile::BoofcvBrightness,
    ReadingRateProfile::BoofcvBrightSpots,
    ReadingRateProfile::BoofcvClose,
    ReadingRateProfile::BoofcvCurved,
    ReadingRateProfile::BoofcvDamaged,
    ReadingRateProfile::BoofcvGlare,
    ReadingRateProfile::BoofcvHighVersion,
    ReadingRateProfile::BoofcvLots,
    ReadingRateProfile::BoofcvMonitor,
    ReadingRateProfile::BoofcvNominal,
    ReadingRateProfile::BoofcvNoncompliant,
    ReadingRateProfile::BoofcvPathological,
    ReadingRateProfile::BoofcvPerspective,
    ReadingRateProfile::BoofcvRotations,
    ReadingRateProfile::BoofcvShadows,
    ReadingRateProfile::PayloadValidated,
];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchdiffCommand {
    Help,
    Run(BenchdiffArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchdiffArgs {
    pub base_path: PathBuf,
    pub candidate_path: PathBuf,
    pub artifact_path: PathBuf,
}

impl Default for BenchdiffArgs {
    fn default() -> Self {
        Self {
            base_path: PathBuf::new(),
            candidate_path: PathBuf::new(),
            artifact_path: PathBuf::from(DEFAULT_BENCHDIFF_ARTIFACT_PATH),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchDiffMetric {
    pub base: f64,
    pub candidate: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchDiffFailureSignature {
    pub base: Option<String>,
    pub candidate: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchDiffGlobalSummary {
    pub total_cases_base: usize,
    pub total_cases_candidate: usize,
    pub matched_cases_base: usize,
    pub matched_cases_candidate: usize,
    pub reading_rate: BenchDiffMetric,
    pub median_runtime_ms: BenchDiffMetric,
    pub top_failure_signature: BenchDiffFailureSignature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchDiffCategorySummary {
    pub category: String,
    pub total_cases_base: usize,
    pub total_cases_candidate: usize,
    pub matched_cases_base: usize,
    pub matched_cases_candidate: usize,
    pub reading_rate: BenchDiffMetric,
    pub median_runtime_ms: BenchDiffMetric,
    pub top_failure_signature: BenchDiffFailureSignature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchDiffHighlight {
    pub category: String,
    pub reading_rate_delta: f64,
    pub median_runtime_delta_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchDiffReport {
    pub generated_at_unix_ms: u128,
    pub base_artifact_path: PathBuf,
    pub candidate_artifact_path: PathBuf,
    pub global: BenchDiffGlobalSummary,
    pub categories: Vec<BenchDiffCategorySummary>,
    pub top_improvements: Vec<BenchDiffHighlight>,
    pub top_regressions: Vec<BenchDiffHighlight>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct ReadingRateArtifactInput {
    global: ArtifactGlobalInput,
    categories: Vec<ArtifactCategoryInput>,
}

#[derive(Debug, Clone)]
struct ArtifactGlobalInput {
    total_cases: usize,
    matched_cases: usize,
    reading_rate: f64,
    median_runtime_ms: f64,
    top_failure_signature: Option<String>,
}

#[derive(Debug, Clone)]
struct ArtifactCategoryInput {
    category: String,
    total_cases: usize,
    matched_cases: usize,
    reading_rate: f64,
    median_runtime_ms: f64,
    top_failure_signature: Option<String>,
}

#[derive(Debug, Clone)]
enum JsonValue {
    Null,
    Bool,
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

struct JsonParser<'a> {
    raw: &'a [u8],
    cursor: usize,
}

pub fn parse_reading_rate_profile(raw: &str) -> Result<ReadingRateProfile, String> {
    match raw {
        BOOFCV_ALL_PROFILE => Ok(ReadingRateProfile::BoofcvAll),
        BOOFCV_BLURRED_PROFILE => Ok(ReadingRateProfile::BoofcvBlurred),
        BOOFCV_BRIGHTNESS_PROFILE => Ok(ReadingRateProfile::BoofcvBrightness),
        BOOFCV_BRIGHT_SPOTS_PROFILE => Ok(ReadingRateProfile::BoofcvBrightSpots),
        BOOFCV_CLOSE_PROFILE => Ok(ReadingRateProfile::BoofcvClose),
        BOOFCV_CURVED_PROFILE => Ok(ReadingRateProfile::BoofcvCurved),
        BOOFCV_DAMAGED_PROFILE => Ok(ReadingRateProfile::BoofcvDamaged),
        BOOFCV_GLARE_PROFILE => Ok(ReadingRateProfile::BoofcvGlare),
        BOOFCV_HIGH_VERSION_PROFILE => Ok(ReadingRateProfile::BoofcvHighVersion),
        BOOFCV_LOTS_PROFILE => Ok(ReadingRateProfile::BoofcvLots),
        BOOFCV_MONITOR_PROFILE => Ok(ReadingRateProfile::BoofcvMonitor),
        BOOFCV_NOMINAL_PROFILE => Ok(ReadingRateProfile::BoofcvNominal),
        BOOFCV_NONCOMPLIANT_PROFILE => Ok(ReadingRateProfile::BoofcvNoncompliant),
        BOOFCV_PATHOLOGICAL_PROFILE => Ok(ReadingRateProfile::BoofcvPathological),
        BOOFCV_PERSPECTIVE_PROFILE => Ok(ReadingRateProfile::BoofcvPerspective),
        BOOFCV_ROTATIONS_PROFILE => Ok(ReadingRateProfile::BoofcvRotations),
        BOOFCV_SHADOWS_PROFILE => Ok(ReadingRateProfile::BoofcvShadows),
        // Backward compatible aliases.
        MONITOR_SMOKE_PROFILE => Ok(ReadingRateProfile::MonitorSmoke),
        NOMINAL_SMOKE_PROFILE => Ok(ReadingRateProfile::NominalSmoke),
        PAYLOAD_VALIDATED_PROFILE => Ok(ReadingRateProfile::PayloadValidated),
        _ => Err(format!(
            "unknown --profile value: {raw}; use a supported profile such as {BOOFCV_ALL_PROFILE}, {BOOFCV_ROTATIONS_PROFILE}, or {PAYLOAD_VALIDATED_PROFILE}"
        )),
    }
}

pub fn reading_rate_profile_dataset_root(profile: ReadingRateProfile) -> PathBuf {
    match profile {
        ReadingRateProfile::BoofcvAll => PathBuf::from("benches/images/boofcv"),
        ReadingRateProfile::BoofcvBlurred => PathBuf::from("benches/images/boofcv/blurred"),
        ReadingRateProfile::BoofcvBrightness => PathBuf::from("benches/images/boofcv/brightness"),
        ReadingRateProfile::BoofcvBrightSpots => {
            PathBuf::from("benches/images/boofcv/bright_spots")
        }
        ReadingRateProfile::BoofcvClose => PathBuf::from("benches/images/boofcv/close"),
        ReadingRateProfile::BoofcvCurved => PathBuf::from("benches/images/boofcv/curved"),
        ReadingRateProfile::BoofcvDamaged => PathBuf::from("benches/images/boofcv/damaged"),
        ReadingRateProfile::BoofcvGlare => PathBuf::from("benches/images/boofcv/glare"),
        ReadingRateProfile::BoofcvHighVersion => {
            PathBuf::from("benches/images/boofcv/high_version")
        }
        ReadingRateProfile::BoofcvLots => PathBuf::from("benches/images/boofcv/lots"),
        ReadingRateProfile::BoofcvMonitor => PathBuf::from("benches/images/boofcv/monitor"),
        ReadingRateProfile::BoofcvNominal => PathBuf::from("benches/images/boofcv/nominal"),
        ReadingRateProfile::BoofcvNoncompliant => {
            PathBuf::from("benches/images/boofcv/noncompliant")
        }
        ReadingRateProfile::BoofcvPathological => {
            PathBuf::from("benches/images/boofcv/pathological")
        }
        ReadingRateProfile::BoofcvPerspective => PathBuf::from("benches/images/boofcv/perspective"),
        ReadingRateProfile::BoofcvRotations => PathBuf::from("benches/images/boofcv/rotations"),
        ReadingRateProfile::BoofcvShadows => PathBuf::from("benches/images/boofcv/shadows"),
        // Backward compatible aliases.
        ReadingRateProfile::MonitorSmoke => PathBuf::from("benches/images/boofcv/monitor"),
        ReadingRateProfile::NominalSmoke => PathBuf::from("benches/images/boofcv/nominal"),
        ReadingRateProfile::PayloadValidated => PathBuf::from("benches/images/custom/decoding"),
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
    "usage: qrtool reading-rate [--profile boofcv-all|boofcv-<category>|payload-validated] [--dataset-root PATH] [--artifact PATH] [--limit N] [--max-working-dim N] [--emergency-cutoff-ms N]"
}

pub fn parse_benchdiff_args(args: &[String]) -> Result<BenchdiffCommand, String> {
    let mut parsed = BenchdiffArgs::default();

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--help" | "-h" => return Ok(BenchdiffCommand::Help),
            "--base" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--base requires a value".to_string())?;
                parsed.base_path = PathBuf::from(value);
            }
            "--candidate" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--candidate requires a value".to_string())?;
                parsed.candidate_path = PathBuf::from(value);
            }
            "--artifact" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| "--artifact requires a value".to_string())?;
                parsed.artifact_path = PathBuf::from(value);
            }
            unknown => {
                return Err(format!(
                    "unknown argument for benchdiff: {unknown}\n{}",
                    benchdiff_usage()
                ));
            }
        }
        idx += 1;
    }

    if parsed.base_path.as_os_str().is_empty() {
        return Err(format!("missing required --base\n{}", benchdiff_usage()));
    }
    if parsed.candidate_path.as_os_str().is_empty() {
        return Err(format!(
            "missing required --candidate\n{}",
            benchdiff_usage()
        ));
    }

    Ok(BenchdiffCommand::Run(parsed))
}

pub fn benchdiff_usage() -> &'static str {
    "usage: qrtool benchdiff --base PATH --candidate PATH [--artifact PATH]"
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

        let expected_payload = fs::read_to_string(&label_path)?.trim().to_string();

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
        "pipeline rebuild is in progress; compare strict payload and BoofCV annotation lanes separately".to_string(),
    ];
    let (kpi_lane, lane_semantics) = reading_rate_lane_semantics(args);
    notes.push(format!("kpi_lane={kpi_lane} semantics={lane_semantics}"));
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

fn reading_rate_lane_semantics(args: &ReadingRateArgs) -> (&'static str, &'static str) {
    match args.profile {
        Some(ReadingRateProfile::PayloadValidated) => ("payload-validated", "strict-payload-match"),
        Some(_) => ("boofcv", "annotation-any-decode"),
        None => {
            let normalized = args.dataset_root.to_string_lossy().replace('\\', "/");
            if normalized.ends_with("benches/images/custom/decoding") {
                ("payload-validated", "strict-payload-match")
            } else if normalized.contains("benches/images/boofcv") {
                ("boofcv", "annotation-any-decode")
            } else {
                ("custom", "dataset-dependent")
            }
        }
    }
}

pub fn build_benchdiff_report(args: &BenchdiffArgs) -> Result<BenchDiffReport, String> {
    let ReadingRateArtifactInput {
        global: base_global,
        categories: base_categories_raw,
    } = load_reading_rate_artifact(&args.base_path)?;
    let ReadingRateArtifactInput {
        global: candidate_global,
        categories: candidate_categories_raw,
    } = load_reading_rate_artifact(&args.candidate_path)?;

    let base_categories = categories_by_name(base_categories_raw, &args.base_path)?;
    let candidate_categories = categories_by_name(candidate_categories_raw, &args.candidate_path)?;

    let category_names: BTreeSet<String> = base_categories
        .keys()
        .chain(candidate_categories.keys())
        .cloned()
        .collect();

    let mut notes = Vec::new();
    let mut categories = Vec::with_capacity(category_names.len());
    for category in category_names {
        let base_row = base_categories.get(&category);
        let candidate_row = candidate_categories.get(&category);

        if base_row.is_none() {
            notes.push(format!("category added in candidate artifact: {category}"));
        }
        if candidate_row.is_none() {
            notes.push(format!(
                "category missing in candidate artifact: {category}"
            ));
        }

        categories.push(BenchDiffCategorySummary {
            category,
            total_cases_base: base_row.map(|row| row.total_cases).unwrap_or(0),
            total_cases_candidate: candidate_row.map(|row| row.total_cases).unwrap_or(0),
            matched_cases_base: base_row.map(|row| row.matched_cases).unwrap_or(0),
            matched_cases_candidate: candidate_row.map(|row| row.matched_cases).unwrap_or(0),
            reading_rate: bench_metric_delta(
                base_row.map(|row| row.reading_rate).unwrap_or(0.0),
                candidate_row.map(|row| row.reading_rate).unwrap_or(0.0),
            ),
            median_runtime_ms: bench_metric_delta(
                base_row.map(|row| row.median_runtime_ms).unwrap_or(0.0),
                candidate_row
                    .map(|row| row.median_runtime_ms)
                    .unwrap_or(0.0),
            ),
            top_failure_signature: BenchDiffFailureSignature {
                base: base_row.and_then(|row| row.top_failure_signature.clone()),
                candidate: candidate_row.and_then(|row| row.top_failure_signature.clone()),
            },
        });
    }

    let top_improvements = rank_benchdiff_highlights(&categories, true);
    let top_regressions = rank_benchdiff_highlights(&categories, false);

    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    Ok(BenchDiffReport {
        generated_at_unix_ms,
        base_artifact_path: args.base_path.clone(),
        candidate_artifact_path: args.candidate_path.clone(),
        global: BenchDiffGlobalSummary {
            total_cases_base: base_global.total_cases,
            total_cases_candidate: candidate_global.total_cases,
            matched_cases_base: base_global.matched_cases,
            matched_cases_candidate: candidate_global.matched_cases,
            reading_rate: bench_metric_delta(
                base_global.reading_rate,
                candidate_global.reading_rate,
            ),
            median_runtime_ms: bench_metric_delta(
                base_global.median_runtime_ms,
                candidate_global.median_runtime_ms,
            ),
            top_failure_signature: BenchDiffFailureSignature {
                base: base_global.top_failure_signature,
                candidate: candidate_global.top_failure_signature,
            },
        },
        categories,
        top_improvements,
        top_regressions,
        notes,
    })
}

fn load_reading_rate_artifact(path: &Path) -> Result<ReadingRateArtifactInput, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read artifact {}: {err}", path.display()))?;
    parse_reading_rate_artifact(&raw, path)
}

fn parse_reading_rate_artifact(
    raw: &str,
    artifact_path: &Path,
) -> Result<ReadingRateArtifactInput, String> {
    let root = JsonParser::new(raw).parse().map_err(|err| {
        format!(
            "failed to parse artifact {}: {err}",
            artifact_path.display()
        )
    })?;
    let root_object = as_object(&root, "artifact root", artifact_path)?;

    let global = root_object
        .get("global")
        .ok_or_else(|| format!("artifact {} missing field: global", artifact_path.display()))
        .and_then(|value| parse_artifact_global(value, artifact_path))?;

    let categories = match root_object.get("categories") {
        Some(value) => parse_artifact_categories(value, artifact_path)?,
        None => Vec::new(),
    };

    Ok(ReadingRateArtifactInput { global, categories })
}

fn parse_artifact_global(
    value: &JsonValue,
    artifact_path: &Path,
) -> Result<ArtifactGlobalInput, String> {
    let object = as_object(value, "global", artifact_path)?;
    Ok(ArtifactGlobalInput {
        total_cases: required_usize_field(object, "total_cases", "global", artifact_path)?,
        matched_cases: required_usize_field(object, "matched_cases", "global", artifact_path)?,
        reading_rate: required_number_field(object, "reading_rate", "global", artifact_path)?,
        median_runtime_ms: required_number_field(
            object,
            "median_runtime_ms",
            "global",
            artifact_path,
        )?,
        top_failure_signature: optional_string_field(
            object,
            "top_failure_signature",
            "global",
            artifact_path,
        )?,
    })
}

fn parse_artifact_categories(
    value: &JsonValue,
    artifact_path: &Path,
) -> Result<Vec<ArtifactCategoryInput>, String> {
    let rows = as_array(value, "categories", artifact_path)?;
    let mut categories = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        let context = format!("categories[{idx}]");
        let object = as_object(row, &context, artifact_path)?;
        categories.push(ArtifactCategoryInput {
            category: required_string_field(object, "category", &context, artifact_path)?,
            total_cases: required_usize_field(object, "total_cases", &context, artifact_path)?,
            matched_cases: required_usize_field(object, "matched_cases", &context, artifact_path)?,
            reading_rate: required_number_field(object, "reading_rate", &context, artifact_path)?,
            median_runtime_ms: required_number_field(
                object,
                "median_runtime_ms",
                &context,
                artifact_path,
            )?,
            top_failure_signature: optional_string_field(
                object,
                "top_failure_signature",
                &context,
                artifact_path,
            )?,
        });
    }
    Ok(categories)
}

fn as_object<'a>(
    value: &'a JsonValue,
    context: &str,
    artifact_path: &Path,
) -> Result<&'a BTreeMap<String, JsonValue>, String> {
    match value {
        JsonValue::Object(object) => Ok(object),
        other => Err(format!(
            "artifact {} expected object for {} but found {}",
            artifact_path.display(),
            context,
            json_type_name(other)
        )),
    }
}

fn as_array<'a>(
    value: &'a JsonValue,
    context: &str,
    artifact_path: &Path,
) -> Result<&'a [JsonValue], String> {
    match value {
        JsonValue::Array(items) => Ok(items),
        other => Err(format!(
            "artifact {} expected array for {} but found {}",
            artifact_path.display(),
            context,
            json_type_name(other)
        )),
    }
}

fn required_number_field(
    object: &BTreeMap<String, JsonValue>,
    field: &str,
    context: &str,
    artifact_path: &Path,
) -> Result<f64, String> {
    let value = object.get(field).ok_or_else(|| {
        format!(
            "artifact {} missing field {} in {}",
            artifact_path.display(),
            field,
            context
        )
    })?;
    match value {
        JsonValue::Number(number) => Ok(*number),
        other => Err(format!(
            "artifact {} expected numeric {} in {} but found {}",
            artifact_path.display(),
            field,
            context,
            json_type_name(other)
        )),
    }
}

fn required_usize_field(
    object: &BTreeMap<String, JsonValue>,
    field: &str,
    context: &str,
    artifact_path: &Path,
) -> Result<usize, String> {
    let value = required_number_field(object, field, context, artifact_path)?;
    if value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(format!(
            "artifact {} expected non-negative integer {} in {} but found {}",
            artifact_path.display(),
            field,
            context,
            value
        ));
    }
    Ok(value as usize)
}

fn required_string_field(
    object: &BTreeMap<String, JsonValue>,
    field: &str,
    context: &str,
    artifact_path: &Path,
) -> Result<String, String> {
    let value = object.get(field).ok_or_else(|| {
        format!(
            "artifact {} missing field {} in {}",
            artifact_path.display(),
            field,
            context
        )
    })?;
    match value {
        JsonValue::String(parsed) => Ok(parsed.clone()),
        other => Err(format!(
            "artifact {} expected string {} in {} but found {}",
            artifact_path.display(),
            field,
            context,
            json_type_name(other)
        )),
    }
}

fn optional_string_field(
    object: &BTreeMap<String, JsonValue>,
    field: &str,
    context: &str,
    artifact_path: &Path,
) -> Result<Option<String>, String> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };

    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(parsed) => Ok(Some(parsed.clone())),
        other => Err(format!(
            "artifact {} expected string|null {} in {} but found {}",
            artifact_path.display(),
            field,
            context,
            json_type_name(other)
        )),
    }
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

impl<'a> JsonParser<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            raw: raw.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.cursor != self.raw.len() {
            return Err(format!(
                "unexpected trailing characters at byte {}",
                self.cursor
            ));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b't') => self.parse_literal(b"true", JsonValue::Bool),
            Some(b'f') => self.parse_literal(b"false", JsonValue::Bool),
            Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
            Some(other) => Err(format!(
                "unexpected byte '{}' at position {}",
                other as char, self.cursor
            )),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        let mut object = BTreeMap::new();
        if self.try_consume_byte(b'}') {
            return Ok(JsonValue::Object(object));
        }

        loop {
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            if object.insert(key.clone(), value).is_some() {
                return Err(format!(
                    "duplicate object key {key:?} at byte {}",
                    self.cursor
                ));
            }

            self.skip_whitespace();
            if self.try_consume_byte(b',') {
                self.skip_whitespace();
                continue;
            }

            self.expect_byte(b'}')?;
            break;
        }

        Ok(JsonValue::Object(object))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        let mut array = Vec::new();
        if self.try_consume_byte(b']') {
            return Ok(JsonValue::Array(array));
        }

        loop {
            array.push(self.parse_value()?);
            self.skip_whitespace();
            if self.try_consume_byte(b',') {
                self.skip_whitespace();
                continue;
            }

            self.expect_byte(b']')?;
            break;
        }

        Ok(JsonValue::Array(array))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_byte(b'"')?;
        let mut parsed = String::new();
        while let Some(byte) = self.next_byte() {
            match byte {
                b'"' => return Ok(parsed),
                b'\\' => {
                    let escaped = self
                        .next_byte()
                        .ok_or_else(|| "unterminated escape sequence".to_string())?;
                    match escaped {
                        b'"' => parsed.push('"'),
                        b'\\' => parsed.push('\\'),
                        b'/' => parsed.push('/'),
                        b'b' => parsed.push('\u{0008}'),
                        b'f' => parsed.push('\u{000C}'),
                        b'n' => parsed.push('\n'),
                        b'r' => parsed.push('\r'),
                        b't' => parsed.push('\t'),
                        b'u' => {
                            let ch = self.parse_unicode_escape_char()?;
                            parsed.push(ch);
                        }
                        other => {
                            return Err(format!(
                                "unsupported escape sequence \\{} at byte {}",
                                other as char, self.cursor
                            ));
                        }
                    }
                }
                control if control < 0x20 => {
                    return Err(format!(
                        "control character in string at byte {}",
                        self.cursor.saturating_sub(1)
                    ));
                }
                ascii if ascii < 0x80 => parsed.push(ascii as char),
                non_ascii => {
                    let start = self.cursor.saturating_sub(1);
                    let Some(width) = utf8_sequence_width(non_ascii) else {
                        return Err(format!(
                            "invalid utf-8 leading byte 0x{non_ascii:02X} at byte {start}"
                        ));
                    };
                    let end = start + width;
                    let bytes = self.raw.get(start..end).ok_or_else(|| {
                        format!("unterminated utf-8 sequence starting at byte {start}")
                    })?;
                    let text = std::str::from_utf8(bytes).map_err(|err| {
                        format!("invalid utf-8 sequence at bytes {start}..{end}: {err}")
                    })?;
                    parsed.push_str(text);
                    self.cursor = end;
                }
            }
        }
        Err("unterminated string literal".to_string())
    }

    fn parse_unicode_escape_char(&mut self) -> Result<char, String> {
        let first = self.parse_hex_code_point()?;
        if (0xD800..=0xDBFF).contains(&first) {
            let slash = self.next_byte().ok_or_else(|| {
                format!(
                    "unterminated unicode surrogate pair after \\u{first:04X} at byte {}",
                    self.cursor
                )
            })?;
            if slash != b'\\' {
                return Err(format!(
                    "expected '\\' after high surrogate \\u{first:04X} at byte {}",
                    self.cursor.saturating_sub(1)
                ));
            }
            let u = self.next_byte().ok_or_else(|| {
                format!(
                    "unterminated unicode surrogate pair after \\u{first:04X} at byte {}",
                    self.cursor
                )
            })?;
            if u != b'u' {
                return Err(format!(
                    "expected 'u' after high surrogate \\u{first:04X} at byte {}",
                    self.cursor.saturating_sub(1)
                ));
            }

            let second = self.parse_hex_code_point()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(format!(
                    "invalid low surrogate \\u{second:04X} after high surrogate \\u{first:04X}"
                ));
            }
            let code_point = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(code_point).ok_or_else(|| {
                format!("invalid unicode code point from surrogate pair: U+{code_point:04X}")
            });
        }

        if (0xDC00..=0xDFFF).contains(&first) {
            return Err(format!(
                "unexpected low surrogate without preceding high surrogate: \\u{first:04X}"
            ));
        }

        char::from_u32(first).ok_or_else(|| {
            format!(
                "invalid unicode escape \\u{first:04X} at byte {}",
                self.cursor
            )
        })
    }

    fn parse_hex_code_point(&mut self) -> Result<u32, String> {
        let mut code_point = 0u32;
        for _ in 0..4 {
            let byte = self
                .next_byte()
                .ok_or_else(|| "unterminated unicode escape".to_string())?;
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => 10 + (byte - b'a'),
                b'A'..=b'F' => 10 + (byte - b'A'),
                _ => {
                    return Err(format!(
                        "invalid unicode escape digit '{}' at byte {}",
                        byte as char,
                        self.cursor.saturating_sub(1)
                    ));
                }
            };
            code_point = (code_point << 4) | digit as u32;
        }
        Ok(code_point)
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.cursor;

        if self.peek() == Some(b'-') {
            self.cursor += 1;
        }

        match self.peek() {
            Some(b'0') => {
                self.cursor += 1;
            }
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.cursor += 1;
                }
            }
            _ => return Err(format!("invalid number at byte {}", self.cursor)),
        }

        if self.peek() == Some(b'.') {
            self.cursor += 1;
            if !self.consume_digits() {
                return Err(format!("invalid fractional number at byte {}", self.cursor));
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            if !self.consume_digits() {
                return Err(format!(
                    "invalid scientific notation at byte {}",
                    self.cursor
                ));
            }
        }

        let literal = std::str::from_utf8(&self.raw[start..self.cursor])
            .map_err(|err| format!("invalid utf-8 in number literal: {err}"))?;
        let parsed = literal
            .parse::<f64>()
            .map_err(|err| format!("invalid number literal {literal:?}: {err}"))?;
        Ok(JsonValue::Number(parsed))
    }

    fn consume_digits(&mut self) -> bool {
        let start = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
        self.cursor > start
    }

    fn parse_literal(&mut self, literal: &[u8], value: JsonValue) -> Result<JsonValue, String> {
        if self
            .raw
            .get(self.cursor..)
            .map(|slice| slice.starts_with(literal))
            .unwrap_or(false)
        {
            self.cursor += literal.len();
            Ok(value)
        } else {
            let literal = std::str::from_utf8(literal).unwrap_or("literal");
            Err(format!("expected {literal} at byte {}", self.cursor))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.cursor += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), String> {
        match self.next_byte() {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => Err(format!(
                "expected byte '{}' at position {} but found '{}'",
                expected as char,
                self.cursor.saturating_sub(1),
                actual as char
            )),
            None => Err(format!(
                "expected byte '{}' at end of input",
                expected as char
            )),
        }
    }

    fn try_consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.raw.get(self.cursor).copied()
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.cursor += 1;
        Some(byte)
    }
}

fn utf8_sequence_width(first_byte: u8) -> Option<usize> {
    match first_byte {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

fn categories_by_name(
    categories: Vec<ArtifactCategoryInput>,
    artifact_path: &Path,
) -> Result<BTreeMap<String, ArtifactCategoryInput>, String> {
    let mut by_name = BTreeMap::new();
    for category in categories {
        let category_name = category.category.clone();
        if by_name.insert(category_name.clone(), category).is_some() {
            return Err(format!(
                "artifact {} contains duplicate category summary: {category_name}",
                artifact_path.display()
            ));
        }
    }
    Ok(by_name)
}

fn bench_metric_delta(base: f64, candidate: f64) -> BenchDiffMetric {
    BenchDiffMetric {
        base,
        candidate,
        delta: candidate - base,
    }
}

fn rank_benchdiff_highlights(
    categories: &[BenchDiffCategorySummary],
    improvements: bool,
) -> Vec<BenchDiffHighlight> {
    let mut highlights = categories
        .iter()
        .filter_map(|category| {
            let delta = category.reading_rate.delta;
            let selected = if improvements {
                delta > 0.0
            } else {
                delta < 0.0
            };
            if !selected {
                return None;
            }

            Some(BenchDiffHighlight {
                category: category.category.clone(),
                reading_rate_delta: delta,
                median_runtime_delta_ms: category.median_runtime_ms.delta,
            })
        })
        .collect::<Vec<_>>();

    highlights.sort_by(|left, right| {
        let primary = if improvements {
            right
                .reading_rate_delta
                .partial_cmp(&left.reading_rate_delta)
                .unwrap_or(Ordering::Equal)
        } else {
            left.reading_rate_delta
                .partial_cmp(&right.reading_rate_delta)
                .unwrap_or(Ordering::Equal)
        };

        primary.then_with(|| left.category.cmp(&right.category))
    });
    highlights.truncate(MAX_BENCHDIFF_HIGHLIGHTS);
    highlights
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

pub fn render_benchdiff_console_summary(report: &BenchDiffReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "benchdiff base_artifact={} candidate_artifact={} global_rate={:.4}->{:.4} delta={:+.4} global_median_runtime_ms={:.3}->{:.3} delta={:+.3} top_failure_signature={}=>{}",
        report.base_artifact_path.display(),
        report.candidate_artifact_path.display(),
        report.global.reading_rate.base,
        report.global.reading_rate.candidate,
        report.global.reading_rate.delta,
        report.global.median_runtime_ms.base,
        report.global.median_runtime_ms.candidate,
        report.global.median_runtime_ms.delta,
        report
            .global
            .top_failure_signature
            .base
            .as_deref()
            .unwrap_or("none"),
        report
            .global
            .top_failure_signature
            .candidate
            .as_deref()
            .unwrap_or("none"),
    ));

    for category in &report.categories {
        lines.push(format!(
            "category={} cases={}=>{} matched={}=>{} rate={:.4}->{:.4} delta={:+.4} median_runtime_ms={:.3}->{:.3} delta={:+.3} top_failure_signature={}=>{}",
            category.category,
            category.total_cases_base,
            category.total_cases_candidate,
            category.matched_cases_base,
            category.matched_cases_candidate,
            category.reading_rate.base,
            category.reading_rate.candidate,
            category.reading_rate.delta,
            category.median_runtime_ms.base,
            category.median_runtime_ms.candidate,
            category.median_runtime_ms.delta,
            category
                .top_failure_signature
                .base
                .as_deref()
                .unwrap_or("none"),
            category
                .top_failure_signature
                .candidate
                .as_deref()
                .unwrap_or("none"),
        ));
    }

    if report.top_improvements.is_empty() {
        lines.push("top_improvements=none".to_string());
    } else {
        for improvement in &report.top_improvements {
            lines.push(format!(
                "top_improvement category={} rate_delta={:+.4} median_runtime_delta_ms={:+.3}",
                improvement.category,
                improvement.reading_rate_delta,
                improvement.median_runtime_delta_ms
            ));
        }
    }

    if report.top_regressions.is_empty() {
        lines.push("top_regressions=none".to_string());
    } else {
        for regression in &report.top_regressions {
            lines.push(format!(
                "top_regression category={} rate_delta={:+.4} median_runtime_delta_ms={:+.3}",
                regression.category,
                regression.reading_rate_delta,
                regression.median_runtime_delta_ms
            ));
        }
    }

    for note in &report.notes {
        lines.push(format!("note={note}"));
    }

    lines.join("\n")
}

pub fn benchdiff_to_json(report: &BenchDiffReport) -> String {
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

    fn signed_delta(candidate: usize, base: usize) -> i128 {
        candidate as i128 - base as i128
    }

    let categories_json = report
        .categories
        .iter()
        .map(|category| {
            format!(
                concat!(
                    "{{",
                    "\"category\":{},",
                    "\"total_cases\":{{\"base\":{},\"candidate\":{},\"delta\":{}}},",
                    "\"matched_cases\":{{\"base\":{},\"candidate\":{},\"delta\":{}}},",
                    "\"reading_rate\":{{\"base\":{:.6},\"candidate\":{:.6},\"delta\":{:.6}}},",
                    "\"median_runtime_ms\":{{\"base\":{:.6},\"candidate\":{:.6},\"delta\":{:.6}}},",
                    "\"top_failure_signature\":{{\"base\":{},\"candidate\":{}}}",
                    "}}"
                ),
                quoted(&category.category),
                category.total_cases_base,
                category.total_cases_candidate,
                signed_delta(category.total_cases_candidate, category.total_cases_base),
                category.matched_cases_base,
                category.matched_cases_candidate,
                signed_delta(
                    category.matched_cases_candidate,
                    category.matched_cases_base
                ),
                category.reading_rate.base,
                category.reading_rate.candidate,
                category.reading_rate.delta,
                category.median_runtime_ms.base,
                category.median_runtime_ms.candidate,
                category.median_runtime_ms.delta,
                optional_string(&category.top_failure_signature.base),
                optional_string(&category.top_failure_signature.candidate),
            )
        })
        .collect::<Vec<_>>();

    let top_improvements_json = report
        .top_improvements
        .iter()
        .map(|improvement| {
            format!(
                "{{\"category\":{},\"reading_rate_delta\":{:.6},\"median_runtime_delta_ms\":{:.6}}}",
                quoted(&improvement.category),
                improvement.reading_rate_delta,
                improvement.median_runtime_delta_ms,
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let top_regressions_json = report
        .top_regressions
        .iter()
        .map(|regression| {
            format!(
                "{{\"category\":{},\"reading_rate_delta\":{:.6},\"median_runtime_delta_ms\":{:.6}}}",
                quoted(&regression.category),
                regression.reading_rate_delta,
                regression.median_runtime_delta_ms,
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

    format!(
        concat!(
            "{{",
            "\"schema_version\":\"wp015-benchdiff-v1\",",
            "\"generated_at_unix_ms\":{},",
            "\"base_artifact_path\":{},",
            "\"candidate_artifact_path\":{},",
            "\"global\":{{",
            "\"total_cases\":{{\"base\":{},\"candidate\":{},\"delta\":{}}},",
            "\"matched_cases\":{{\"base\":{},\"candidate\":{},\"delta\":{}}},",
            "\"reading_rate\":{{\"base\":{:.6},\"candidate\":{:.6},\"delta\":{:.6}}},",
            "\"median_runtime_ms\":{{\"base\":{:.6},\"candidate\":{:.6},\"delta\":{:.6}}},",
            "\"top_failure_signature\":{{\"base\":{},\"candidate\":{}}}",
            "}},",
            "\"categories\":[{}],",
            "\"top_improvements\":[{}],",
            "\"top_regressions\":[{}],",
            "\"notes\":[{}]",
            "}}"
        ),
        report.generated_at_unix_ms,
        quoted(&report.base_artifact_path.to_string_lossy()),
        quoted(&report.candidate_artifact_path.to_string_lossy()),
        report.global.total_cases_base,
        report.global.total_cases_candidate,
        signed_delta(
            report.global.total_cases_candidate,
            report.global.total_cases_base
        ),
        report.global.matched_cases_base,
        report.global.matched_cases_candidate,
        signed_delta(
            report.global.matched_cases_candidate,
            report.global.matched_cases_base
        ),
        report.global.reading_rate.base,
        report.global.reading_rate.candidate,
        report.global.reading_rate.delta,
        report.global.median_runtime_ms.base,
        report.global.median_runtime_ms.candidate,
        report.global.median_runtime_ms.delta,
        optional_string(&report.global.top_failure_signature.base),
        optional_string(&report.global.top_failure_signature.candidate),
        categories_json.join(","),
        top_improvements_json,
        top_regressions_json,
        notes_json,
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
