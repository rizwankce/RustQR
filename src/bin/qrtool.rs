use clap::{Parser, Subcommand};
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
use rust_qr::models::{BitMatrix, Point};
use rust_qr::tools::{
    audit_brightness_vs_otsu, bench_limit_from_env, binarize, binary_stats, dataset_fingerprint,
    dataset_iter, dataset_root_from_env, detect_qr, evaluate_finder_and_grouping, grayscale_stats,
    label_fingerprint, load_rgb, load_rgb_with_geometry, parse_localization_labels,
    parse_payload_label, scale_quadrilaterals, score_localizations, score_payloads, smoke_from_env,
    to_grayscale,
};
use rust_qr::utils::geometry::PerspectiveTransform;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "qrtool", version, about = "RustQR CLI tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run QR detection on a single image
    Detect {
        #[arg(long)]
        image: PathBuf,
    },
    /// Print grayscale/binary stats and finder patterns for an image
    DebugDetect {
        #[arg(long)]
        image: PathBuf,
    },
    /// Try decoding using hand-labeled corner points
    DebugDecode {
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        points: Option<PathBuf>,
    },
    /// Compute reading rate on a dataset
    ReadingRate {
        /// Dataset root (default: QR_DATASET_ROOT or benches/images/boofcv)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Max images per category (default: QR_BENCH_LIMIT; 0 means all)
        #[arg(long)]
        limit: Option<usize>,
        /// Use smoke subset (default also enabled by QR_SMOKE)
        #[arg(long)]
        smoke: bool,
        /// Write machine-readable benchmark JSON artifact.
        #[arg(long, value_name = "PATH")]
        artifact_json: Option<PathBuf>,
        /// Suppress per-image logs for non-interactive runs (CI/scripts).
        #[arg(long)]
        non_interactive: bool,
        /// Emit progress every N labeled images (0 disables periodic progress).
        #[arg(long, default_value_t = 0)]
        progress_every: usize,
        /// Optional category to run (e.g. lots, rotations, high_version).
        #[arg(long)]
        category: Option<String>,
        /// Mark images exceeding this end-to-end deadline as timed out (0 disables).
        #[arg(long, default_value_t = 0)]
        timeout_ms: u64,
        /// Treat same-stem .txt labels as exact payloads instead of quadrilaterals.
        #[arg(long)]
        payload_validated: bool,
    },
    /// Measure finder-proposal and grouping stages against localization labels.
    ProposalEval {
        /// Dataset root (default: QR_DATASET_ROOT or benches/images/boofcv)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Max images total (default: QR_BENCH_LIMIT; 0 means all)
        #[arg(long)]
        limit: Option<usize>,
        /// Use the checked-in smoke subset.
        #[arg(long)]
        smoke: bool,
        /// Optional category to run (e.g. nominal, lots, rotations).
        #[arg(long)]
        category: Option<String>,
        /// Write a machine-readable JSON evidence artifact.
        #[arg(long, value_name = "PATH")]
        artifact_json: Option<PathBuf>,
    },
    /// Compare fresh brightness-route and strict-Otsu dense observations.
    DenseRouteAudit {
        #[arg(long)]
        image: PathBuf,
        /// Cooperative deadline shared with the public reading-rate evaluator.
        #[arg(long, default_value_t = 10_000)]
        timeout_ms: u64,
        /// Write a machine-readable route/geometry comparison.
        #[arg(long, value_name = "PATH")]
        output: PathBuf,
    },
    /// Iterate a dataset and run detection once per image
    DatasetBench {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        smoke: bool,
    },
    /// Export branch-neutral WP-005 predictions; does not score labels.
    PredictionExport {
        /// Dataset root (default: QR_DATASET_ROOT or benches/images/boofcv)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Max images per selected category (default: QR_BENCH_LIMIT; 0 means all)
        #[arg(long)]
        limit: Option<usize>,
        /// Optional category to export (e.g. nominal, rotations, lots).
        #[arg(long)]
        category: Option<String>,
        /// Cooperative request deadline in milliseconds (0 disables).
        #[arg(long, default_value_t = 0)]
        timeout_ms: u64,
        /// Write rustqr.wp005.prediction-stream.v1 JSON.
        #[arg(long, value_name = "PATH")]
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Detect { image } => detect_cmd(&image),
        Command::DebugDetect { image } => debug_detect_cmd(&image),
        Command::DebugDecode { image, points } => debug_decode_cmd(&image, points.as_deref()),
        Command::ReadingRate {
            root,
            limit,
            smoke,
            artifact_json,
            non_interactive,
            progress_every,
            category,
            timeout_ms,
            payload_validated,
        } => reading_rate_cmd(ReadingRateOptions {
            root,
            limit,
            smoke,
            artifact_json,
            non_interactive,
            progress_every,
            category,
            timeout_ms,
            payload_validated,
        }),
        Command::ProposalEval {
            root,
            limit,
            smoke,
            category,
            artifact_json,
        } => proposal_eval_cmd(
            root,
            limit,
            smoke,
            category.as_deref(),
            artifact_json.as_deref(),
        ),
        Command::DenseRouteAudit {
            image,
            timeout_ms,
            output,
        } => dense_route_audit_cmd(&image, timeout_ms, &output),
        Command::DatasetBench { root, limit, smoke } => dataset_bench_cmd(root, limit, smoke),
        Command::PredictionExport {
            root,
            limit,
            category,
            timeout_ms,
            output,
        } => prediction_export_cmd(root, limit, category.as_deref(), timeout_ms, &output),
    }
}

#[derive(Default)]
struct ProposalEvalStats {
    images: usize,
    expected_symbols: usize,
    finder_hits: usize,
    proposal_multiplicity: [usize; 4],
    grouping_hits: usize,
    finder_eligible_without_group: usize,
    spurious_proposals: usize,
    contained_proposals: usize,
    spurious_groups: usize,
    contained_groups: usize,
    duplicate_contained_groups: usize,
    raw_candidates: usize,
    proposals_after_nms: usize,
    roi_windows_considered: usize,
    roi_rows_considered: usize,
    roi_columns_considered: usize,
    roi_raw_candidates: usize,
    contour_raw_candidates: usize,
    contour_appended_proposals: usize,
    proposal_ms: Vec<f64>,
    grouping_ms: Vec<f64>,
}

impl ProposalEvalStats {
    fn add(&mut self, evaluation: rust_qr::tools::FinderGroupingEvaluation) {
        self.images += 1;
        self.expected_symbols += evaluation.expected_symbols;
        self.finder_hits += evaluation.finder_hits;
        for (total, value) in self
            .proposal_multiplicity
            .iter_mut()
            .zip(evaluation.proposal_multiplicity)
        {
            *total += value;
        }
        self.grouping_hits += evaluation.grouping_hits;
        self.finder_eligible_without_group += evaluation.finder_eligible_without_group;
        self.spurious_proposals += evaluation.spurious_proposals;
        self.contained_proposals += evaluation.contained_proposals;
        self.spurious_groups += evaluation.spurious_groups;
        self.contained_groups += evaluation.contained_groups;
        self.duplicate_contained_groups += evaluation.duplicate_contained_groups;
        self.raw_candidates += evaluation.scan_telemetry.raw_candidates;
        self.proposals_after_nms += evaluation.scan_telemetry.proposals_after_nms;
        self.roi_windows_considered += evaluation.scan_telemetry.roi_windows_considered;
        self.roi_rows_considered += evaluation.scan_telemetry.roi_rows_considered;
        self.roi_columns_considered += evaluation.scan_telemetry.roi_columns_considered;
        self.roi_raw_candidates += evaluation.scan_telemetry.roi_raw_candidates;
        self.contour_raw_candidates += evaluation.scan_telemetry.contour_raw_candidates;
        self.contour_appended_proposals += evaluation.scan_telemetry.contour_appended_proposals;
        self.proposal_ms.push(evaluation.proposal_latency_ms);
        self.grouping_ms.push(evaluation.grouping_latency_ms);
    }
}

fn proposal_eval_cmd(
    root: Option<PathBuf>,
    limit: Option<usize>,
    smoke: bool,
    category: Option<&str>,
    artifact_json: Option<&Path>,
) {
    let root = root.unwrap_or_else(dataset_root_from_env);
    let limit = limit.or_else(bench_limit_from_env);
    let smoke = smoke || smoke_from_env();
    let evaluation_root = category.map_or_else(|| root.clone(), |name| root.join(name));
    if !evaluation_root.exists() {
        eprintln!("Dataset root not found: {}", evaluation_root.display());
        std::process::exit(2);
    }

    let mut stats = ProposalEvalStats::default();
    for image_path in dataset_iter(&evaluation_root, limit, smoke) {
        let label_path = image_path.with_extension("txt");
        if !label_path.exists() {
            continue;
        }
        let labels = parse_localization_labels(&label_path).unwrap_or_else(|error| {
            eprintln!("invalid label {}: {error}", label_path.display());
            std::process::exit(2);
        });
        let loaded = load_rgb_with_geometry(&image_path).unwrap_or_else(|error| {
            eprintln!("failed to load {}: {error}", image_path.display());
            std::process::exit(2);
        });
        let expected = scale_quadrilaterals(
            &labels.quadrilaterals,
            (loaded.source_width, loaded.source_height),
            (loaded.width, loaded.height),
        );
        let gray = to_grayscale(&loaded.pixels, loaded.width, loaded.height);
        let binary = binarize(&gray, loaded.width, loaded.height);
        stats.add(evaluate_finder_and_grouping(&binary, &expected));
    }

    if stats.images == 0 {
        eprintln!(
            "No labeled images found under {}",
            evaluation_root.display()
        );
        std::process::exit(2);
    }
    let finder_recall = metric_ratio(stats.finder_hits, stats.expected_symbols);
    let grouping_recall = metric_ratio(stats.grouping_hits, stats.expected_symbols);
    let proposal_summary = latency_summary(&stats.proposal_ms);
    let grouping_summary = latency_summary(&stats.grouping_ms);
    println!("Finder/grouping stage evaluation");
    println!("Dataset: {}", evaluation_root.display());
    println!(
        "Images: {} | annotated symbols: {}",
        stats.images, stats.expected_symbols
    );
    println!(
        "Finder recall: {}/{} ({:.2}%) | grouping recall: {}/{} ({:.2}%)",
        stats.finder_hits,
        stats.expected_symbols,
        finder_recall * 100.0,
        stats.grouping_hits,
        stats.expected_symbols,
        grouping_recall * 100.0,
    );
    println!(
        "Proposal centres per symbol (0/1/2/3+): {}/{}/{}/{} | finder-eligible without group: {}",
        stats.proposal_multiplicity[0],
        stats.proposal_multiplicity[1],
        stats.proposal_multiplicity[2],
        stats.proposal_multiplicity[3],
        stats.finder_eligible_without_group,
    );
    println!(
        "Contained/spurious proposals: {}/{} | contained/duplicate/spurious groups: {}/{}/{}",
        stats.contained_proposals,
        stats.spurious_proposals,
        stats.contained_groups,
        stats.duplicate_contained_groups,
        stats.spurious_groups,
    );
    println!(
        "Raw/NMS proposals: {}/{}",
        stats.raw_candidates, stats.proposals_after_nms,
    );
    println!(
        "ROI recovery windows/rows/columns/raw: {}/{}/{}/{}",
        stats.roi_windows_considered,
        stats.roi_rows_considered,
        stats.roi_columns_considered,
        stats.roi_raw_candidates,
    );
    println!(
        "Dense contour-family raw/appended proposals: {}/{}",
        stats.contour_raw_candidates, stats.contour_appended_proposals,
    );
    println!(
        "Proposal ms (mean/p50/p95): {:.3}/{:.3}/{:.3} | grouping: {:.3}/{:.3}/{:.3}",
        proposal_summary.0,
        proposal_summary.1,
        proposal_summary.2,
        grouping_summary.0,
        grouping_summary.1,
        grouping_summary.2,
    );
    if let Some(path) = artifact_json {
        let artifact = format!(
            r#"{{
  "schema_version": 6,
  "evaluator": "finder-proposal-grouping-contained-centres-v6",
  "dataset": "{}",
  "dataset_fingerprint": "{}",
  "label_fingerprint": "{}",
  "images": {},
  "expected_symbols": {},
  "finder_hits": {},
  "proposal_multiplicity": {{"zero": {}, "one": {}, "two": {}, "three_or_more": {}}},
  "grouping_hits": {},
  "finder_eligible_without_group": {},
  "finder_recall": {:.8},
  "grouping_recall": {:.8},
  "spurious_proposals": {},
  "contained_proposals": {},
  "spurious_groups": {},
  "contained_groups": {},
  "duplicate_contained_groups": {},
  "raw_candidates": {},
  "proposals_after_nms": {},
  "roi_recovery": {{"windows": {}, "rows": {}, "columns": {}, "raw_candidates": {}}},
  "contour_recovery": {{"raw_candidates": {}, "appended_proposals": {}}},
  "proposal_latency_ms": {{"mean": {:.6}, "p50": {:.6}, "p95": {:.6}}},
  "grouping_latency_ms": {{"mean": {:.6}, "p50": {:.6}, "p95": {:.6}}}
}}
"#,
            json_escape(&evaluation_root.to_string_lossy()),
            dataset_fingerprint(&evaluation_root),
            label_fingerprint(&evaluation_root),
            stats.images,
            stats.expected_symbols,
            stats.finder_hits,
            stats.proposal_multiplicity[0],
            stats.proposal_multiplicity[1],
            stats.proposal_multiplicity[2],
            stats.proposal_multiplicity[3],
            stats.grouping_hits,
            stats.finder_eligible_without_group,
            finder_recall,
            grouping_recall,
            stats.spurious_proposals,
            stats.contained_proposals,
            stats.spurious_groups,
            stats.contained_groups,
            stats.duplicate_contained_groups,
            stats.raw_candidates,
            stats.proposals_after_nms,
            stats.roi_windows_considered,
            stats.roi_rows_considered,
            stats.roi_columns_considered,
            stats.roi_raw_candidates,
            stats.contour_raw_candidates,
            stats.contour_appended_proposals,
            proposal_summary.0,
            proposal_summary.1,
            proposal_summary.2,
            grouping_summary.0,
            grouping_summary.1,
            grouping_summary.2,
        );
        fs::write(path, artifact).unwrap_or_else(|error| {
            eprintln!("failed to write {}: {error}", path.display());
            std::process::exit(2);
        });
        println!("Artifact: {}", path.display());
    }
}

fn dense_route_audit_cmd(image: &Path, timeout_ms: u64, output: &Path) {
    let loaded = load_rgb_with_geometry(image).unwrap_or_else(|error| {
        eprintln!("failed to load {}: {error}", image.display());
        std::process::exit(2);
    });
    let audit = audit_brightness_vs_otsu(
        &loaded.pixels,
        loaded.width,
        loaded.height,
        std::time::Duration::from_millis(timeout_ms),
    );
    let brightness_boxes: Vec<_> = audit.brightness_codes.iter().map(qr_bbox).collect();
    let otsu_boxes: Vec<_> = audit.otsu_codes.iter().map(qr_bbox).collect();
    let brightness_shared: Vec<_> = brightness_boxes
        .iter()
        .map(|&bbox| {
            otsu_boxes
                .iter()
                .any(|&other| bbox_iou(bbox, other) >= 0.72)
        })
        .collect();
    let otsu_shared: Vec<_> = otsu_boxes
        .iter()
        .map(|&bbox| {
            brightness_boxes
                .iter()
                .any(|&other| bbox_iou(bbox, other) >= 0.72)
        })
        .collect();
    let mut json = String::new();
    let _ = writeln!(&mut json, "{{");
    let _ = writeln!(&mut json, "  \"schema_version\": 1,");
    let _ = writeln!(
        &mut json,
        "  \"image\": \"{}\",",
        json_escape(&image.to_string_lossy())
    );
    let _ = writeln!(
        &mut json,
        "  \"working_dimensions\": [{}, {}],",
        loaded.width, loaded.height
    );
    let _ = writeln!(&mut json, "  \"timeout_ms\": {},", timeout_ms);
    let _ = writeln!(
        &mut json,
        "  \"brightness_elapsed_ms\": {:.6},",
        audit.brightness_elapsed_ms
    );
    let _ = writeln!(
        &mut json,
        "  \"otsu_elapsed_ms\": {:.6},",
        audit.otsu_elapsed_ms
    );
    let _ = writeln!(
        &mut json,
        "  \"otsu_stage_ms\": {{\"binarize\": {:.6}, \"finder\": {:.6}, \"group\": {:.6}, \"decode\": {:.6}}},",
        audit.otsu_binarize_ms, audit.otsu_finder_ms, audit.otsu_group_ms, audit.otsu_decode_ms,
    );
    let _ = writeln!(
        &mut json,
        "  \"otsu_finder_patterns\": {},",
        audit.otsu_finder_patterns
    );
    let _ = writeln!(
        &mut json,
        "  \"otsu_group_candidates\": {},",
        audit.otsu_group_candidates
    );
    write_route_codes(
        &mut json,
        "brightness",
        &audit.brightness_codes,
        &brightness_shared,
    );
    json.push_str(",\n");
    write_route_codes(&mut json, "otsu", &audit.otsu_codes, &otsu_shared);
    json.push_str("\n}\n");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| {
            eprintln!("failed to create {}: {error}", parent.display());
            std::process::exit(2);
        });
    }
    fs::write(output, json).unwrap_or_else(|error| {
        eprintln!("failed to write {}: {error}", output.display());
        std::process::exit(2);
    });
    println!(
        "Brightness/Otsu: {}/{} codes, shared geometry {}/{}; artifact: {}",
        audit.brightness_codes.len(),
        audit.otsu_codes.len(),
        brightness_shared.iter().filter(|shared| **shared).count(),
        otsu_shared.iter().filter(|shared| **shared).count(),
        output.display(),
    );
}

fn qr_bbox(qr: &rust_qr::QRCode) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for point in &qr.position {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    (min_x, min_y, max_x, max_y)
}

fn bbox_iou(left: (f32, f32, f32, f32), right: (f32, f32, f32, f32)) -> f32 {
    let overlap_x = (left.2.min(right.2) - left.0.max(right.0)).max(0.0);
    let overlap_y = (left.3.min(right.3) - left.1.max(right.1)).max(0.0);
    let intersection = overlap_x * overlap_y;
    let left_area = ((left.2 - left.0).max(0.0)) * ((left.3 - left.1).max(0.0));
    let right_area = ((right.2 - right.0).max(0.0)) * ((right.3 - right.1).max(0.0));
    let union = left_area + right_area - intersection;
    if union <= f32::EPSILON {
        0.0
    } else {
        intersection / union
    }
}

fn write_route_codes(json: &mut String, route: &str, codes: &[rust_qr::QRCode], shared: &[bool]) {
    let _ = write!(json, "  \"{}_codes\": [", route);
    for (index, (code, shared)) in codes.iter().zip(shared).enumerate() {
        if index > 0 {
            json.push(',');
        }
        let (min_x, min_y, max_x, max_y) = qr_bbox(code);
        let _ = write!(
            json,
            "\n    {{\"payload\": \"{}\", \"shared_geometry\": {}, \"bbox\": [{:.3}, {:.3}, {:.3}, {:.3}]}},",
            json_escape(&code.content),
            shared,
            min_x,
            min_y,
            max_x,
            max_y,
        );
        json.pop();
    }
    if !codes.is_empty() {
        json.push('\n');
    }
    json.push_str("  ]");
}

fn metric_ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn latency_summary(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let percentile =
        |fraction: f64| sorted[((sorted.len() - 1) as f64 * fraction).round() as usize];
    (
        samples.iter().sum::<f64>() / samples.len() as f64,
        percentile(0.5),
        percentile(0.95),
    )
}

fn detect_cmd(image: &Path) {
    match load_rgb(image) {
        Ok((pixels, width, height)) => {
            let results = detect_qr(&pixels, width, height);
            println!("Image: {} ({}x{})", image.display(), width, height);
            println!("Found {} QR codes", results.len());
            for (i, qr) in results.iter().enumerate() {
                println!(
                    "  QR {}: version={:?}, error_correction={:?}, mask={:?}, content={}",
                    i, qr.version, qr.error_correction, qr.mask_pattern, qr.content
                );
            }
        }
        Err(err) => {
            eprintln!("Failed to load image {}: {}", image.display(), err);
        }
    }
}

const WP005_CATEGORIES: [&str; 7] = [
    "nominal",
    "rotations",
    "perspective",
    "high_version",
    "lots",
    "brightness",
    "bright_spots",
];

/// Export geometry before any evaluator scoring so other branch adapters can
/// feed the shared WP-005 external normalizer.  This intentionally uses the
/// existing image loader/detector API unchanged.
fn prediction_export_cmd(
    root: Option<PathBuf>,
    limit: Option<usize>,
    category: Option<&str>,
    timeout_ms: u64,
    output: &Path,
) {
    let root = root.unwrap_or_else(dataset_root_from_env);
    let limit = limit.or_else(bench_limit_from_env);
    if !root.is_dir() {
        eprintln!("Dataset root not found: {}", root.display());
        std::process::exit(2);
    }
    let selected_categories: Vec<&str> = match category {
        Some(name) if WP005_CATEGORIES.contains(&name) => vec![name],
        Some(name) => {
            eprintln!("Unsupported WP-005 category: {name}");
            std::process::exit(2);
        }
        None => WP005_CATEGORIES.to_vec(),
    };
    let mut rows = String::new();
    let mut first = true;
    let mut exported = 0usize;
    for category in selected_categories {
        let category_root = root.join(category);
        if !category_root.is_dir() {
            eprintln!("Dataset category not found: {}", category_root.display());
            std::process::exit(2);
        }
        for path in dataset_iter(&category_root, limit, false) {
            let start = Instant::now();
            let loaded = load_rgb_with_geometry(&path).unwrap_or_else(|error| {
                eprintln!("failed to load {}: {error}", path.display());
                std::process::exit(2);
            });
            let core_start = Instant::now();
            let results = if timeout_ms == 0 {
                rust_qr::detect(&loaded.pixels, loaded.width, loaded.height)
            } else {
                rust_qr::detect_with_telemetry_timeout(
                    &loaded.pixels,
                    loaded.width,
                    loaded.height,
                    std::time::Duration::from_millis(timeout_ms),
                )
                .0
            };
            let core_elapsed_ms = core_start.elapsed().as_secs_f64() * 1_000.0;
            let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
            let timed_out = deadline_exceeded(elapsed_ms, timeout_ms);
            if !first {
                rows.push_str(",\n");
            }
            first = false;
            let image_id = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy();
            let _ = write!(
                &mut rows,
                "    {{\n      \"image_id\": \"{}\",\n      \"category\": \"{}\",\n      \"original_width\": {},\n      \"original_height\": {},\n      \"working_width\": {},\n      \"working_height\": {},\n      \"predicted_quadrilaterals\": [",
                json_escape(&image_id),
                category,
                loaded.source_width,
                loaded.source_height,
                loaded.width,
                loaded.height,
            );
            for (index, qr) in results.iter().enumerate() {
                if index > 0 {
                    rows.push_str(", ");
                }
                rows.push('[');
                for (point_index, point) in qr.position.iter().enumerate() {
                    if point_index > 0 {
                        rows.push_str(", ");
                    }
                    let x = point.x * loaded.source_width as f32 / loaded.width as f32;
                    let y = point.y * loaded.source_height as f32 / loaded.height as f32;
                    let _ = write!(&mut rows, "[{x:.6}, {y:.6}]");
                }
                rows.push(']');
            }
            rows.push_str("],\n      \"payloads\": [");
            let mut first_payload = true;
            for qr in &results {
                if let Ok(payload) = String::from_utf8(qr.data.clone()) {
                    if !first_payload {
                        rows.push_str(", ");
                    }
                    first_payload = false;
                    let _ = write!(&mut rows, "\"{}\"", json_escape(&payload));
                }
            }
            let _ = write!(
                &mut rows,
                "],\n      \"core_elapsed_ms\": {core_elapsed_ms:.6},\n      \"end_to_end_elapsed_ms\": {elapsed_ms:.6},\n      \"timed_out\": {timed_out}\n    }}"
            );
            exported += 1;
        }
    }
    let preprocessing = format!(
        "rgb8;triangle-resize;max-dim={}",
        std::env::var("QR_MAX_DIM").unwrap_or_else(|_| "none".to_string())
    );
    let artifact = format!(
        "{{\n  \"schema_version\": \"rustqr.wp005.prediction-stream.v1\",\n  \"metadata\": {{\n    \"commit_sha\": \"{}\",\n    \"dataset_fingerprint\": \"{}\",\n    \"preprocessing_fingerprint\": \"{}\",\n    \"limit_per_category\": {},\n    \"timeout_ms\": {}\n  }},\n  \"images\": [\n{}\n  ]\n}}\n",
        json_escape(&commit_sha()),
        json_escape(&dataset_fingerprint(&root)),
        json_escape(&preprocessing),
        limit.map_or_else(|| "null".to_string(), |value| value.to_string()),
        timeout_ms,
        rows,
    );
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("failed to create {}: {error}", parent.display());
            std::process::exit(2);
        }
    }
    if let Err(error) = fs::write(output, artifact) {
        eprintln!("failed to write {}: {error}", output.display());
        std::process::exit(2);
    }
    println!(
        "WP-005 prediction export: {} images -> {}",
        exported,
        output.display()
    );
}

fn debug_detect_cmd(image: &Path) {
    let (pixels, width, height) = match load_rgb(image) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Failed to load image {}: {}", image.display(), err);
            return;
        }
    };

    println!("Image: {} ({}x{})", image.display(), width, height);

    let gray = to_grayscale(&pixels, width, height);
    let gray_stats = grayscale_stats(&gray);
    println!(
        "Grayscale range: {}-{}, average: {}",
        gray_stats.min, gray_stats.max, gray_stats.avg
    );

    let binary = binarize(&gray, width, height);
    let stats = binary_stats(&binary);
    println!(
        "Binary: black_pixels={} total={} black_ratio={:.2}%",
        stats.black_pixels,
        stats.total_pixels,
        stats.black_ratio * 100.0
    );

    let patterns = FinderDetector::detect(&binary);
    println!("Found {} finder patterns", patterns.len());
    for (i, pattern) in patterns.iter().take(10).enumerate() {
        println!(
            "  Pattern {}: center=({:.1}, {:.1}) module_size={:.2}",
            i, pattern.center.x, pattern.center.y, pattern.module_size
        );
    }

    let results = detect_qr(&pixels, width, height);
    println!("Full detection found {} QR codes", results.len());
}

fn debug_decode_cmd(image: &Path, points: Option<&Path>) {
    let (pixels, width, height) = match load_rgb(image) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Failed to load image {}: {}", image.display(), err);
            return;
        }
    };

    println!("Image: {} ({}x{})", image.display(), width, height);

    let gray = to_grayscale(&pixels, width, height);
    let binary = binarize(&gray, width, height);

    let patterns = FinderDetector::detect(&binary);
    println!("Found {} finder patterns", patterns.len());
    for (i, pattern) in patterns.iter().take(10).enumerate() {
        println!(
            "  Pattern {}: center=({:.1}, {:.1}) module_size={:.2}",
            i, pattern.center.x, pattern.center.y, pattern.module_size
        );
    }

    let points_path = points
        .map(PathBuf::from)
        .unwrap_or_else(|| image.with_extension("txt"));

    if points_path.exists() {
        if let Ok(points) = read_points(&points_path) {
            if points.len() >= 4 {
                decode_from_points(&binary, &points);
            } else {
                println!("Not enough points in {}", points_path.display());
            }
        } else {
            println!("Failed to parse points in {}", points_path.display());
        }
    } else {
        println!("Points file not found: {}", points_path.display());
    }

    let results = detect_qr(&pixels, width, height);
    println!("Full detection found {} QR codes", results.len());
}

fn read_points(path: &Path) -> Result<Vec<Point>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let mut vals = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for tok in line.split_whitespace() {
            if let Ok(v) = tok.parse::<f32>() {
                vals.push(v);
            }
        }
    }
    let mut points = Vec::new();
    for chunk in vals.chunks(2) {
        if chunk.len() == 2 {
            points.push(Point::new(chunk[0], chunk[1]));
        }
    }
    Ok(points)
}

fn decode_from_points(binary: &BitMatrix, points: &[Point]) {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| (a.x + a.y).partial_cmp(&(b.x + b.y)).unwrap());
    let top_left = pts[0];
    let bottom_right = pts[3];
    let others = [pts[1], pts[2]];
    let top_right = if others[0].x > others[1].x {
        others[0]
    } else {
        others[1]
    };
    let bottom_left = if others[0].x > others[1].x {
        others[1]
    } else {
        others[0]
    };

    println!("\n--- Testing with hand-labeled corners ---");
    println!(
        "TL=({:.1},{:.1}) TR=({:.1},{:.1}) BL=({:.1},{:.1}) BR=({:.1},{:.1})",
        top_left.x,
        top_left.y,
        top_right.x,
        top_right.y,
        bottom_left.x,
        bottom_left.y,
        bottom_right.x,
        bottom_right.y
    );

    for version in 1..=40u8 {
        let dimension = 17 + 4 * version as usize;
        let src = [
            Point::new(0.0, 0.0),
            Point::new(dimension as f32 - 1.0, 0.0),
            Point::new(dimension as f32 - 1.0, dimension as f32 - 1.0),
            Point::new(0.0, dimension as f32 - 1.0),
        ];
        let dst = [top_left, top_right, bottom_right, bottom_left];
        let transform = match PerspectiveTransform::from_points(&src, &dst) {
            Some(t) => t,
            None => continue,
        };

        let mut qr_matrix = BitMatrix::new(dimension, dimension);
        for y in 0..dimension {
            for x in 0..dimension {
                let p = Point::new(x as f32, y as f32);
                let img_point = transform.transform(&p);
                let img_x = img_point.x.floor() as isize;
                let img_y = img_point.y.floor() as isize;
                if img_x >= 0
                    && img_y >= 0
                    && (img_x as usize) < binary.width()
                    && (img_y as usize) < binary.height()
                {
                    qr_matrix.set(x, y, binary.get(img_x as usize, img_y as usize));
                }
            }
        }

        if let Some(info) = FormatInfo::extract(&qr_matrix) {
            println!(
                "Version {} format: EC={:?} Mask={:?}",
                version, info.ec_level, info.mask_pattern
            );
            break;
        }
    }
}

struct ReadingRateOptions {
    root: Option<PathBuf>,
    limit: Option<usize>,
    smoke: bool,
    artifact_json: Option<PathBuf>,
    non_interactive: bool,
    progress_every: usize,
    category: Option<String>,
    timeout_ms: u64,
    payload_validated: bool,
}

fn reading_rate_cmd(options: ReadingRateOptions) {
    let ReadingRateOptions {
        root,
        limit,
        smoke,
        artifact_json,
        non_interactive,
        progress_every,
        category,
        timeout_ms,
        payload_validated,
    } = options;
    let root = root.unwrap_or_else(dataset_root_from_env);
    let limit = limit.or_else(bench_limit_from_env);
    let smoke = smoke || smoke_from_env();

    if !root.exists() {
        eprintln!("Dataset root not found: {}", root.display());
        return;
    }

    let smoke_images: Option<Vec<PathBuf>> = if smoke {
        Some(dataset_iter(&root, None, true).collect())
    } else {
        None
    };
    if let Some(images) = &smoke_images {
        if images.is_empty() {
            println!("No images found under {}", root.display());
            return;
        }
    }

    let categories = [
        ("blurred", "Blurred QR codes"),
        ("bright_spots", "Bright spots/glare"),
        ("brightness", "Various brightness levels"),
        ("close", "Close-up QR codes"),
        ("controlled_dense", "Controlled dense multi-QR scenes"),
        ("curved", "Curved surface QR codes"),
        ("damaged", "Damaged QR codes"),
        ("glare", "Glare/light reflections"),
        ("high_version", "High capacity QR codes"),
        ("lots", "Many QR codes in one image"),
        ("monitor", "Standard QR codes on monitor"),
        ("nominal", "Standard/nominal conditions"),
        ("noncompliant", "Non-standard QR codes"),
        ("pathological", "Pathological cases"),
        ("perspective", "Perspective distortion"),
        ("rotations", "Rotated QR codes"),
        ("shadows", "Shadows on QR codes"),
    ];

    // Run metadata header
    let datetime = utc_timestamp();
    let commit_sha = commit_sha();
    let data_fingerprint = dataset_fingerprint(&root);
    let labels_fingerprint = label_fingerprint(&root);
    let preprocessing_fingerprint = format!(
        "rgb8;triangle-resize;max-dim={}",
        std::env::var("QR_MAX_DIM").unwrap_or_else(|_| "none".to_string())
    );
    let evaluator_fingerprint = if payload_validated {
        "mode=payload;matching=normalized-exact-one-to-one".to_string()
    } else {
        "mode=localization;quad-iou=0.5;matching=max-cardinality".to_string()
    };

    println!("RustQR QR Code Reading Rate Benchmark");
    println!("=====================================");
    println!("Date:    {}", datetime);
    println!("Commit:  {}", commit_sha);
    println!("Dataset: {}", root.display());
    println!("Data FP: {}", data_fingerprint);
    if let Some(l) = limit {
        println!("Limit:   {} images per category", l);
    } else {
        println!("Limit:   full dataset");
    }
    if smoke {
        println!("Mode:    smoke test");
    }
    if non_interactive {
        println!("Output:  non-interactive");
    }
    if let Some(c) = &category {
        println!("Category filter: {}", c);
    }
    println!("=====================================\n");

    let mut global_hits = 0usize;
    let mut global_expected = 0usize;
    let mut global_images_with_labels = 0usize;
    let mut global_runtime_samples_ms: Vec<f64> = Vec::new();
    let mut global_core_runtime_samples_ms: Vec<f64> = Vec::new();
    let mut global_false_positives = 0usize;
    let mut global_false_negatives = 0usize;
    let mut global_duplicate_predictions = 0usize;
    let mut global_timeouts = 0usize;
    let mut global_stage_telemetry = StageTelemetry::default();
    let mut global_failure_clusters: BTreeMap<String, FailureCluster> = BTreeMap::new();
    let mut category_results: Vec<CategoryResult> = Vec::new();
    let mut categories_found = 0usize;

    let category_filter = category.as_deref();
    for (dir, description) in categories {
        if let Some(filter) = category_filter {
            if dir != filter {
                continue;
            }
        }
        let category_root = root.join(dir);
        if !category_root.exists() {
            continue;
        }
        categories_found += 1;
        println!("Testing: {} - {}", dir, description);
        let images: Vec<PathBuf> = if let Some(images) = &smoke_images {
            let mut filtered: Vec<PathBuf> = images
                .iter()
                .filter(|path| {
                    path.strip_prefix(&root)
                        .ok()
                        .and_then(|rel| rel.components().next())
                        .map(|c| c.as_os_str() == dir)
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            if let Some(limit) = limit {
                filtered.truncate(limit);
            }
            filtered
        } else {
            dataset_iter(&category_root, limit, false).collect()
        };
        if images.is_empty() {
            println!("  {}: no images found\n", dir);
            continue;
        }
        let stats = match reading_rate_for_images(
            images.into_iter(),
            non_interactive,
            progress_every,
            timeout_ms,
            payload_validated,
        ) {
            Ok(stats) => stats,
            Err(error) => {
                eprintln!("Reading-rate evaluation failed: {error}");
                std::process::exit(2);
            }
        };
        if stats.total_expected == 0 {
            println!("  {}: no labeled images found\n", dir);
            continue;
        }
        let rate = (stats.hits as f64 / stats.total_expected as f64) * 100.0;
        println!(
            "  {}: {}/{} QR codes detected across {} images = {:.2}%\n",
            dir, stats.hits, stats.total_expected, stats.images_with_labels, rate,
        );
        global_hits += stats.hits;
        global_expected += stats.total_expected;
        global_images_with_labels += stats.images_with_labels;
        global_runtime_samples_ms.extend(stats.runtime_samples_ms.iter().copied());
        global_core_runtime_samples_ms.extend(stats.core_runtime_samples_ms.iter().copied());
        global_false_positives += stats.false_positives;
        global_false_negatives += stats.false_negatives;
        global_duplicate_predictions += stats.duplicate_predictions;
        global_timeouts += stats.timeouts;
        global_stage_telemetry.accumulate(stats.stage_telemetry);
        for (sig, cluster) in stats.failure_clusters {
            let entry = global_failure_clusters
                .entry(sig)
                .or_insert(FailureCluster {
                    count: 0,
                    qr_weight: 0,
                    examples: Vec::new(),
                });
            entry.count += cluster.count;
            entry.qr_weight += cluster.qr_weight;
            for ex in cluster.examples {
                if entry.examples.len() < 3 && !entry.examples.iter().any(|e| e == &ex) {
                    entry.examples.push(ex);
                }
            }
        }
        category_results.push(CategoryResult {
            name: dir,
            description,
            hits: stats.hits,
            total_expected: stats.total_expected,
            images_with_labels: stats.images_with_labels,
            false_positives: stats.false_positives,
            false_negatives: stats.false_negatives,
            duplicate_predictions: stats.duplicate_predictions,
            timeouts: stats.timeouts,
            stage_telemetry: stats.stage_telemetry,
            runtime: RuntimeSummary::from_samples(&stats.runtime_samples_ms),
            core_runtime: RuntimeSummary::from_samples(&stats.core_runtime_samples_ms),
        });
    }

    if categories_found > 0 && global_expected > 0 {
        let global_rate = (global_hits as f64 / global_expected as f64) * 100.0;
        let global_runtime = RuntimeSummary::from_samples(&global_runtime_samples_ms);

        println!("=====================================");
        println!("Reading Rate Summary");
        println!("=====================================");
        println!(
            "{:<16} {:>6} {:>6} {:>8}",
            "Category", "Hits", "Total", "Rate"
        );
        println!("{}", "-".repeat(40));
        for category in &category_results {
            let rate = if category.total_expected > 0 {
                (category.hits as f64 / category.total_expected as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "{:<16} {:>6} {:>6} {:>7.2}%",
                category.name, category.hits, category.total_expected, rate
            );
        }
        println!("{}", "-".repeat(40));
        println!(
            "{:<16} {:>6} {:>6} {:>7.2}%",
            "TOTAL", global_hits, global_expected, global_rate,
        );
        println!(
            "Runtime median: {:.2} ms/image (mean {:.2} ms, n={})",
            global_runtime.median_per_image_ms,
            global_runtime.mean_per_image_ms,
            global_runtime.samples
        );
        println!("=====================================\n");

        // Stage telemetry table
        println!("Pipeline Stage Telemetry (images passing each stage)");
        println!("=====================================");
        println!(
            "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8} {:>8} {:>8}",
            "Category",
            "Imgs",
            "Binarize",
            "Finders",
            "Groups",
            "Xform",
            "Decode",
            "AvgTry",
            "Budget",
            "Region",
            "AcceptRj"
        );
        println!("{}", "-".repeat(104));
        let mut g_decode_attempts = 0usize;
        let mut g_score_buckets = [0usize; 4];
        for category in &category_results {
            let tel = category.stage_telemetry;
            let avg_attempts = if tel.total > 0 {
                tel.total_decode_attempts as f64 / tel.total as f64
            } else {
                0.0
            };
            println!(
                "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9.2} {:>8} {:>8} {:>8}",
                category.name,
                tel.total,
                tel.binarize_ok,
                tel.finder_ok,
                tel.groups_ok,
                tel.transform_ok,
                tel.decode_ok,
                avg_attempts,
                tel.over_budget_skip,
                tel.router_multi_region,
                tel.acceptance_rejected,
            );
            g_decode_attempts += tel.total_decode_attempts;
            for (i, bucket) in g_score_buckets.iter_mut().enumerate() {
                *bucket += tel.candidate_score_buckets[i];
            }
        }
        println!("{}", "-".repeat(104));
        let g_avg_attempts = if global_stage_telemetry.total > 0 {
            g_decode_attempts as f64 / global_stage_telemetry.total as f64
        } else {
            0.0
        };
        println!(
            "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9.2} {:>8} {:>8} {:>8}",
            "TOTAL",
            global_stage_telemetry.total,
            global_stage_telemetry.binarize_ok,
            global_stage_telemetry.finder_ok,
            global_stage_telemetry.groups_ok,
            global_stage_telemetry.transform_ok,
            global_stage_telemetry.decode_ok,
            g_avg_attempts,
            global_stage_telemetry.over_budget_skip,
            global_stage_telemetry.router_multi_region,
            global_stage_telemetry.acceptance_rejected,
        );
        println!(
            "Deskew attempts/successes: {}/{} | High-version precision attempts: {} | Recovery mode attempts: {}",
            global_stage_telemetry.deskew_attempts,
            global_stage_telemetry.deskew_successes,
            global_stage_telemetry.high_version_precision_attempts,
            global_stage_telemetry.recovery_mode_attempts
        );
        println!(
            "Scale retries attempts/successes/skipped: {}/{}/{}",
            global_stage_telemetry.scale_retry_attempts,
            global_stage_telemetry.scale_retry_successes,
            global_stage_telemetry.scale_retry_skipped_by_budget
        );
        println!(
            "High-version subpixel/refine attempts/successes: {}/{}/{}",
            global_stage_telemetry.hv_subpixel_attempts,
            global_stage_telemetry.hv_refine_attempts,
            global_stage_telemetry.hv_refine_successes
        );
        println!(
            "RS erasure attempts/successes: {}/{} | hist[1,2-3,4-6,7+]=[{},{},{},{}]",
            global_stage_telemetry.rs_erasure_attempts,
            global_stage_telemetry.rs_erasure_successes,
            global_stage_telemetry.rs_erasure_count_hist[0],
            global_stage_telemetry.rs_erasure_count_hist[1],
            global_stage_telemetry.rs_erasure_count_hist[2],
            global_stage_telemetry.rs_erasure_count_hist[3]
        );
        println!(
            "Phase11 time-budget skips: {}",
            global_stage_telemetry.phase11_time_budget_skips
        );
        let router_div = global_stage_telemetry.total.max(1) as f64;
        println!(
            "Router fast signals avg blur/sat/skew/density: {:.2}/{:.3}/{:.2}/{:.2}",
            global_stage_telemetry.router_blur_metric_sum / router_div,
            global_stage_telemetry.router_saturation_ratio_sum / router_div,
            global_stage_telemetry.router_skew_estimate_deg_sum / router_div,
            global_stage_telemetry.router_region_density_proxy_sum / router_div
        );
        println!(
            "Budget lanes H/M/L attempts: {}/{}/{} | Fallback transitions O->A31: {} A31->A21: {} | Fallback successes: {}",
            global_stage_telemetry.budget_lane_high,
            global_stage_telemetry.budget_lane_medium,
            global_stage_telemetry.budget_lane_low,
            global_stage_telemetry.bin_fallback_otsu_to_adaptive31,
            global_stage_telemetry.bin_fallback_adaptive31_to_adaptive21,
            global_stage_telemetry.bin_fallback_successes
        );
        let rerank_top1_rate = if global_stage_telemetry.rerank_top1_attempts > 0 {
            (global_stage_telemetry.rerank_top1_successes as f64
                / global_stage_telemetry.rerank_top1_attempts as f64)
                * 100.0
        } else {
            0.0
        };
        println!(
            "Rerank enabled(images): {} | Top1 success: {}/{} ({:.2}%) | Transform rejects: {}",
            global_stage_telemetry.rerank_enabled,
            global_stage_telemetry.rerank_top1_successes,
            global_stage_telemetry.rerank_top1_attempts,
            rerank_top1_rate,
            global_stage_telemetry.rerank_transform_reject_count
        );
        let saturation_coverage_avg = if global_stage_telemetry.total > 0 {
            global_stage_telemetry.saturation_mask_coverage_sum
                / global_stage_telemetry.total as f64
        } else {
            0.0
        };
        println!(
            "Saturation mask enabled(images): {} | Avg coverage: {:.3} | Decode successes: {}",
            global_stage_telemetry.saturation_mask_enabled,
            saturation_coverage_avg,
            global_stage_telemetry.saturation_mask_decode_successes
        );
        println!(
            "ROI norm attempts/successes/skipped: {}/{}/{}",
            global_stage_telemetry.roi_norm_attempts,
            global_stage_telemetry.roi_norm_successes,
            global_stage_telemetry.roi_norm_skipped
        );
        println!(
            "Attempts/image histogram [0, 1, 2-3, 4-7, 8+]: [{}, {}, {}, {}, {}]",
            global_stage_telemetry.attempts_used_histogram[0],
            global_stage_telemetry.attempts_used_histogram[1],
            global_stage_telemetry.attempts_used_histogram[2],
            global_stage_telemetry.attempts_used_histogram[3],
            global_stage_telemetry.attempts_used_histogram[4]
        );
        println!(
            "Candidate score buckets [<2.0, 2.0-<3.0, 3.0-<5.0, >=5.0]: [{}, {}, {}, {}]",
            g_score_buckets[0], g_score_buckets[1], g_score_buckets[2], g_score_buckets[3]
        );
        if !global_failure_clusters.is_empty() {
            println!("Top failure signatures:");
            let mut ranked: Vec<_> = global_failure_clusters.iter().collect();
            ranked.sort_by(|a, b| {
                b.1.qr_weight
                    .cmp(&a.1.qr_weight)
                    .then_with(|| b.1.count.cmp(&a.1.count))
                    .then_with(|| a.0.cmp(b.0))
            });
            for (sig, cluster) in ranked.into_iter().take(6) {
                println!(
                    "  - {:<16} count={} qr_weight={} example={}",
                    sig,
                    cluster.count,
                    cluster.qr_weight,
                    cluster.examples.first().map_or("-", String::as_str)
                );
            }
        }
        println!("=====================================");

        if let Some(path) = artifact_json {
            let mut failure_rows: Vec<FailureClusterRow> = global_failure_clusters
                .into_iter()
                .map(|(signature, v)| FailureClusterRow {
                    signature,
                    count: v.count,
                    qr_weight: v.qr_weight,
                    examples: v.examples,
                })
                .collect();
            failure_rows.sort_by(|a, b| {
                b.qr_weight
                    .cmp(&a.qr_weight)
                    .then_with(|| b.count.cmp(&a.count))
                    .then_with(|| a.signature.cmp(&b.signature))
            });
            let artifact = ReadingRateArtifact {
                dataset_root: root.display().to_string(),
                dataset_fingerprint: data_fingerprint,
                label_fingerprint: labels_fingerprint,
                evaluator_fingerprint,
                preprocessing_fingerprint,
                selected_category: category.clone(),
                commit_sha,
                timestamp_utc: datetime,
                limit_per_category: limit,
                smoke,
                non_interactive,
                timeout_ms,
                payload_ground_truth_available: payload_validated,
                weighted_global_rate_percent: global_rate,
                total_hits: global_hits,
                total_expected: global_expected,
                total_images_with_labels: global_images_with_labels,
                global_runtime,
                global_core_runtime: RuntimeSummary::from_samples(&global_core_runtime_samples_ms),
                false_positives: global_false_positives,
                false_negatives: global_false_negatives,
                duplicate_predictions: global_duplicate_predictions,
                timeouts: global_timeouts,
                categories: category_results,
                failure_clusters: failure_rows,
            };
            write_reading_rate_artifact(&path, &artifact);
            println!("Artifact: {}", path.display());
            println!(
                "A/B compare: python3 scripts/compare_reading_rate_artifacts.py --baseline <baseline.json> --candidate {}",
                path.display()
            );
        }
        return;
    }

    let images: Vec<PathBuf> = if let Some(images) = smoke_images {
        if let Some(limit) = limit {
            images.into_iter().take(limit).collect()
        } else {
            images
        }
    } else {
        dataset_iter(&root, limit, false).collect()
    };
    if images.is_empty() {
        println!("No images found under {}", root.display());
        return;
    }
    let stats = match reading_rate_for_images(
        images.into_iter(),
        non_interactive,
        progress_every,
        timeout_ms,
        payload_validated,
    ) {
        Ok(stats) => stats,
        Err(error) => {
            eprintln!("Reading-rate evaluation failed: {error}");
            std::process::exit(2);
        }
    };
    if stats.total_expected == 0 {
        println!("No labeled images found under {}", root.display());
        return;
    }
    let rate = (stats.hits as f64 / stats.total_expected as f64) * 100.0;
    println!(
        "Reading rate: {}/{} = {:.2}%",
        stats.hits, stats.total_expected, rate
    );

    if let Some(path) = artifact_json {
        let artifact = ReadingRateArtifact {
            dataset_root: root.display().to_string(),
            dataset_fingerprint: data_fingerprint,
            label_fingerprint: labels_fingerprint,
            evaluator_fingerprint,
            preprocessing_fingerprint,
            selected_category: category.clone(),
            commit_sha,
            timestamp_utc: datetime,
            limit_per_category: limit,
            smoke,
            non_interactive,
            timeout_ms,
            payload_ground_truth_available: payload_validated,
            weighted_global_rate_percent: rate,
            total_hits: stats.hits,
            total_expected: stats.total_expected,
            total_images_with_labels: stats.images_with_labels,
            global_runtime: RuntimeSummary::from_samples(&stats.runtime_samples_ms),
            global_core_runtime: RuntimeSummary::from_samples(&stats.core_runtime_samples_ms),
            false_positives: stats.false_positives,
            false_negatives: stats.false_negatives,
            duplicate_predictions: stats.duplicate_predictions,
            timeouts: stats.timeouts,
            categories: Vec::new(),
            failure_clusters: Vec::new(),
        };
        write_reading_rate_artifact(&path, &artifact);
        println!("Artifact: {}", path.display());
    }
}

/// Per-QR-code scoring results for a set of images.
struct ReadingRateStats {
    /// Number of predictions matched one-to-one to annotated locations.
    hits: usize,
    /// Total expected QR codes from label files.
    total_expected: usize,
    /// Number of images that had a label file.
    images_with_labels: usize,
    /// Aggregated per-stage telemetry across all images.
    stage_telemetry: StageTelemetry,
    /// Runtime samples for successfully loaded images.
    runtime_samples_ms: Vec<f64>,
    /// Detection-only samples; excludes image load, resize, and evaluation.
    core_runtime_samples_ms: Vec<f64>,
    false_positives: usize,
    false_negatives: usize,
    duplicate_predictions: usize,
    /// Images whose end-to-end runtime exceeded the configured deadline.
    timeouts: usize,
    /// Clustered failure signatures for missed images.
    failure_clusters: BTreeMap<String, FailureCluster>,
}

/// Aggregated pipeline-stage failure counts across a set of images.
#[derive(Default, Clone, Copy)]
struct StageTelemetry {
    /// Images where binarization succeeded.
    binarize_ok: usize,
    /// Images where >= 3 finder patterns were found.
    finder_ok: usize,
    /// Images where >= 1 valid group was formed.
    groups_ok: usize,
    /// Images where >= 1 perspective transform was built.
    transform_ok: usize,
    /// Images where >= 1 QR code was decoded.
    decode_ok: usize,
    /// Sum of decode attempts across images.
    total_decode_attempts: usize,
    /// Histogram of candidate group scores:
    /// [<2.0, 2.0-<3.0, 3.0-<5.0, >=5.0]
    candidate_score_buckets: [usize; 4],
    /// Images where decoding was skipped by budget constraints.
    over_budget_skip: usize,
    /// Decode attempts routed through high-confidence lane.
    budget_lane_high: usize,
    /// Decode attempts routed through medium-confidence lane.
    budget_lane_medium: usize,
    /// Decode attempts routed through low-confidence lane.
    budget_lane_low: usize,
    /// Binarization fallback transition count: Otsu -> adaptive(31).
    bin_fallback_otsu_to_adaptive31: usize,
    /// Binarization fallback transition count: adaptive(31) -> adaptive(21).
    bin_fallback_adaptive31_to_adaptive21: usize,
    /// Successful decodes achieved on fallback binarization path.
    bin_fallback_successes: usize,
    /// Images where reranking was enabled.
    rerank_enabled: usize,
    /// Number of top-1 rerank attempts.
    rerank_top1_attempts: usize,
    /// Number of successful top-1 rerank decodes.
    rerank_top1_successes: usize,
    /// Number of rerank candidate transform rejects.
    rerank_transform_reject_count: usize,
    /// Images where saturation-aware scoring was enabled.
    saturation_mask_enabled: usize,
    /// Sum of image-level saturation coverage ratios.
    saturation_mask_coverage_sum: f64,
    /// Successful decodes influenced by saturation-aware scoring.
    saturation_mask_decode_successes: usize,
    /// ROI-local normalization attempts.
    roi_norm_attempts: usize,
    /// Successful decodes from ROI-local normalization.
    roi_norm_successes: usize,
    /// ROI-local normalization skips.
    roi_norm_skipped: usize,
    /// Images where 2-finder fallback was used.
    two_finder_used: usize,
    /// Images where router selected multi-region path.
    router_multi_region: usize,
    /// Sum of router blur metrics.
    router_blur_metric_sum: f64,
    /// Sum of router saturation ratios.
    router_saturation_ratio_sum: f64,
    /// Sum of router skew estimates.
    router_skew_estimate_deg_sum: f64,
    /// Sum of router region density proxies.
    router_region_density_proxy_sum: f64,
    /// Total acceptance-based rejections.
    acceptance_rejected: usize,
    /// Total deskew attempts.
    deskew_attempts: usize,
    /// Total deskew successes.
    deskew_successes: usize,
    /// Total high-version precision attempts.
    high_version_precision_attempts: usize,
    /// Total recovery-mode attempts.
    recovery_mode_attempts: usize,
    /// Total multi-scale retry attempts.
    scale_retry_attempts: usize,
    /// Total successful multi-scale retries.
    scale_retry_successes: usize,
    /// Total multi-scale retries skipped by budget/guardrails.
    scale_retry_skipped_by_budget: usize,
    /// Total high-version subpixel attempts.
    hv_subpixel_attempts: usize,
    /// Total high-version refine attempts.
    hv_refine_attempts: usize,
    /// Total high-version refine successes.
    hv_refine_successes: usize,
    /// Total RS erasure attempts.
    rs_erasure_attempts: usize,
    /// Total RS erasure successes.
    rs_erasure_successes: usize,
    /// RS erasure histogram buckets [1, 2-3, 4-6, 7+].
    rs_erasure_count_hist: [usize; 4],
    /// Phase 9.11 candidate branches skipped due to time budget.
    phase11_time_budget_skips: usize,
    /// Per-image decode-attempt histogram:
    /// [0, 1, 2-3, 4-7, 8+]
    attempts_used_histogram: [usize; 5],
    /// Total images processed.
    total: usize,
}

impl StageTelemetry {
    fn accumulate(&mut self, other: StageTelemetry) {
        self.binarize_ok += other.binarize_ok;
        self.finder_ok += other.finder_ok;
        self.groups_ok += other.groups_ok;
        self.transform_ok += other.transform_ok;
        self.decode_ok += other.decode_ok;
        self.total_decode_attempts += other.total_decode_attempts;
        for i in 0..self.candidate_score_buckets.len() {
            self.candidate_score_buckets[i] += other.candidate_score_buckets[i];
        }
        self.over_budget_skip += other.over_budget_skip;
        self.budget_lane_high += other.budget_lane_high;
        self.budget_lane_medium += other.budget_lane_medium;
        self.budget_lane_low += other.budget_lane_low;
        self.bin_fallback_otsu_to_adaptive31 += other.bin_fallback_otsu_to_adaptive31;
        self.bin_fallback_adaptive31_to_adaptive21 += other.bin_fallback_adaptive31_to_adaptive21;
        self.bin_fallback_successes += other.bin_fallback_successes;
        self.rerank_enabled += other.rerank_enabled;
        self.rerank_top1_attempts += other.rerank_top1_attempts;
        self.rerank_top1_successes += other.rerank_top1_successes;
        self.rerank_transform_reject_count += other.rerank_transform_reject_count;
        self.saturation_mask_enabled += other.saturation_mask_enabled;
        self.saturation_mask_coverage_sum += other.saturation_mask_coverage_sum;
        self.saturation_mask_decode_successes += other.saturation_mask_decode_successes;
        self.roi_norm_attempts += other.roi_norm_attempts;
        self.roi_norm_successes += other.roi_norm_successes;
        self.roi_norm_skipped += other.roi_norm_skipped;
        self.two_finder_used += other.two_finder_used;
        self.router_multi_region += other.router_multi_region;
        self.router_blur_metric_sum += other.router_blur_metric_sum;
        self.router_saturation_ratio_sum += other.router_saturation_ratio_sum;
        self.router_skew_estimate_deg_sum += other.router_skew_estimate_deg_sum;
        self.router_region_density_proxy_sum += other.router_region_density_proxy_sum;
        self.acceptance_rejected += other.acceptance_rejected;
        self.deskew_attempts += other.deskew_attempts;
        self.deskew_successes += other.deskew_successes;
        self.high_version_precision_attempts += other.high_version_precision_attempts;
        self.recovery_mode_attempts += other.recovery_mode_attempts;
        self.scale_retry_attempts += other.scale_retry_attempts;
        self.scale_retry_successes += other.scale_retry_successes;
        self.scale_retry_skipped_by_budget += other.scale_retry_skipped_by_budget;
        self.hv_subpixel_attempts += other.hv_subpixel_attempts;
        self.hv_refine_attempts += other.hv_refine_attempts;
        self.hv_refine_successes += other.hv_refine_successes;
        self.rs_erasure_attempts += other.rs_erasure_attempts;
        self.rs_erasure_successes += other.rs_erasure_successes;
        for i in 0..self.rs_erasure_count_hist.len() {
            self.rs_erasure_count_hist[i] += other.rs_erasure_count_hist[i];
        }
        self.phase11_time_budget_skips += other.phase11_time_budget_skips;
        for i in 0..self.attempts_used_histogram.len() {
            self.attempts_used_histogram[i] += other.attempts_used_histogram[i];
        }
        self.total += other.total;
    }
}

fn attempts_hist_bucket(attempts: usize) -> usize {
    if attempts == 0 {
        0
    } else if attempts == 1 {
        1
    } else if attempts <= 3 {
        2
    } else if attempts <= 7 {
        3
    } else {
        4
    }
}

#[derive(Clone)]
struct FailureCluster {
    count: usize,
    qr_weight: usize,
    examples: Vec<String>,
}

#[derive(Clone, Copy)]
struct RuntimeSummary {
    samples: usize,
    total_ms: f64,
    mean_per_image_ms: f64,
    median_per_image_ms: f64,
    p90_per_image_ms: f64,
    p95_per_image_ms: f64,
    p99_per_image_ms: f64,
    min_per_image_ms: f64,
    max_per_image_ms: f64,
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * quantile).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

impl RuntimeSummary {
    fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self {
                samples: 0,
                total_ms: 0.0,
                mean_per_image_ms: 0.0,
                median_per_image_ms: 0.0,
                p90_per_image_ms: 0.0,
                p95_per_image_ms: 0.0,
                p99_per_image_ms: 0.0,
                min_per_image_ms: 0.0,
                max_per_image_ms: 0.0,
            };
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let total_ms: f64 = sorted.iter().sum();
        let mean_per_image_ms = total_ms / sorted.len() as f64;
        let median_per_image_ms = if sorted.len() % 2 == 0 {
            let mid = sorted.len() / 2;
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };
        Self {
            samples: sorted.len(),
            total_ms,
            mean_per_image_ms,
            median_per_image_ms,
            p90_per_image_ms: percentile(&sorted, 0.90),
            p95_per_image_ms: percentile(&sorted, 0.95),
            p99_per_image_ms: percentile(&sorted, 0.99),
            min_per_image_ms: *sorted.first().unwrap_or(&0.0),
            max_per_image_ms: *sorted.last().unwrap_or(&0.0),
        }
    }
}

struct CategoryResult {
    name: &'static str,
    description: &'static str,
    hits: usize,
    total_expected: usize,
    images_with_labels: usize,
    false_positives: usize,
    false_negatives: usize,
    duplicate_predictions: usize,
    timeouts: usize,
    stage_telemetry: StageTelemetry,
    runtime: RuntimeSummary,
    core_runtime: RuntimeSummary,
}

struct ReadingRateArtifact {
    dataset_root: String,
    dataset_fingerprint: String,
    label_fingerprint: String,
    evaluator_fingerprint: String,
    preprocessing_fingerprint: String,
    selected_category: Option<String>,
    commit_sha: String,
    timestamp_utc: String,
    limit_per_category: Option<usize>,
    smoke: bool,
    non_interactive: bool,
    timeout_ms: u64,
    payload_ground_truth_available: bool,
    weighted_global_rate_percent: f64,
    total_hits: usize,
    total_expected: usize,
    total_images_with_labels: usize,
    global_runtime: RuntimeSummary,
    global_core_runtime: RuntimeSummary,
    false_positives: usize,
    false_negatives: usize,
    duplicate_predictions: usize,
    timeouts: usize,
    categories: Vec<CategoryResult>,
    failure_clusters: Vec<FailureClusterRow>,
}

struct FailureClusterRow {
    signature: String,
    count: usize,
    qr_weight: usize,
    examples: Vec<String>,
}

fn reading_rate_for_images<I>(
    images: I,
    non_interactive: bool,
    progress_every: usize,
    timeout_ms: u64,
    payload_validated: bool,
) -> Result<ReadingRateStats, String>
where
    I: Iterator<Item = PathBuf>,
{
    enum ExpectedLabel {
        Localization(Vec<rust_qr::tools::Quadrilateral>),
        Payload(String),
    }
    let mut stats = ReadingRateStats {
        hits: 0,
        total_expected: 0,
        images_with_labels: 0,
        stage_telemetry: StageTelemetry::default(),
        runtime_samples_ms: Vec::new(),
        core_runtime_samples_ms: Vec::new(),
        false_positives: 0,
        false_negatives: 0,
        duplicate_predictions: 0,
        timeouts: 0,
        failure_clusters: BTreeMap::new(),
    };

    for path in images {
        let txt_file = path.with_extension("txt");
        if !txt_file.exists() {
            continue;
        }
        let label = if payload_validated {
            ExpectedLabel::Payload(parse_payload_label(&txt_file).map_err(|error| {
                format!("invalid payload label {}: {error}", txt_file.display())
            })?)
        } else {
            ExpectedLabel::Localization(
                parse_localization_labels(&txt_file)
                    .map_err(|error| format!("invalid label {}: {error}", txt_file.display()))?
                    .quadrilaterals,
            )
        };
        let expected = match &label {
            ExpectedLabel::Localization(quadrilaterals) => quadrilaterals.len(),
            ExpectedLabel::Payload(_) => 1,
        };
        stats.images_with_labels += 1;
        stats.total_expected += expected;
        stats.stage_telemetry.total += 1;

        let end_to_end_start = Instant::now();
        if let Ok(loaded) = load_rgb_with_geometry(&path) {
            let pixels = loaded.pixels;
            let width = loaded.width;
            let height = loaded.height;
            let expected_quadrilaterals = match &label {
                ExpectedLabel::Localization(quadrilaterals) => scale_quadrilaterals(
                    quadrilaterals,
                    (loaded.source_width, loaded.source_height),
                    (width, height),
                ),
                ExpectedLabel::Payload(_) => Vec::new(),
            };
            let core_start = Instant::now();
            let (results, tel) = if timeout_ms == 0 {
                rust_qr::detect_with_telemetry(&pixels, width, height)
            } else {
                rust_qr::detect_with_telemetry_timeout(
                    &pixels,
                    width,
                    height,
                    std::time::Duration::from_millis(timeout_ms),
                )
            };
            let core_elapsed_ms = core_start.elapsed().as_secs_f64() * 1_000.0;
            let elapsed = end_to_end_start.elapsed();
            let elapsed_ms = elapsed.as_secs_f64() * 1_000.0;
            let timed_out = deadline_exceeded(elapsed_ms, timeout_ms);
            let decoded = results.len();
            let (image_hits, false_positives, false_negatives, duplicates) = match &label {
                ExpectedLabel::Localization(_) => {
                    let predictions: Vec<_> = if timed_out {
                        Vec::new()
                    } else {
                        results
                            .iter()
                            .map(|qr| qr.position.map(|p| [p.x, p.y]))
                            .collect()
                    };
                    let score = score_localizations(&expected_quadrilaterals, &predictions, 0.5);
                    (
                        score.true_positives,
                        score.false_positives,
                        score.false_negatives,
                        score.duplicate_predictions,
                    )
                }
                ExpectedLabel::Payload(expected_payload) => {
                    let predicted_payloads: Vec<String> = if timed_out {
                        Vec::new()
                    } else {
                        results
                            .iter()
                            .filter_map(|qr| String::from_utf8(qr.data.clone()).ok())
                            .collect()
                    };
                    let score =
                        score_payloads(std::slice::from_ref(expected_payload), &predicted_payloads);
                    (score.exact_matches, 0, 0, 0)
                }
            };
            stats.hits += image_hits;
            stats.false_positives += false_positives;
            stats.false_negatives += false_negatives;
            stats.duplicate_predictions += duplicates;
            stats.timeouts += usize::from(timed_out);
            stats.runtime_samples_ms.push(elapsed_ms);
            stats.core_runtime_samples_ms.push(core_elapsed_ms);

            // Accumulate stage telemetry
            if tel.binarize_ok {
                stats.stage_telemetry.binarize_ok += 1;
            }
            if tel.finder_patterns_found >= 3 {
                stats.stage_telemetry.finder_ok += 1;
            }
            if tel.groups_found >= 1 {
                stats.stage_telemetry.groups_ok += 1;
            }
            if tel.transforms_built >= 1 {
                stats.stage_telemetry.transform_ok += 1;
            }
            if decoded >= 1 {
                stats.stage_telemetry.decode_ok += 1;
            }
            stats.stage_telemetry.total_decode_attempts += tel.decode_attempts;
            stats.stage_telemetry.attempts_used_histogram
                [attempts_hist_bucket(tel.decode_attempts)] += 1;
            for i in 0..stats.stage_telemetry.candidate_score_buckets.len() {
                stats.stage_telemetry.candidate_score_buckets[i] += tel.candidate_score_buckets[i];
            }
            if tel.budget_skips > 0 {
                stats.stage_telemetry.over_budget_skip += 1;
            }
            stats.stage_telemetry.budget_lane_high += tel.budget_lane_high;
            stats.stage_telemetry.budget_lane_medium += tel.budget_lane_medium;
            stats.stage_telemetry.budget_lane_low += tel.budget_lane_low;
            stats.stage_telemetry.bin_fallback_otsu_to_adaptive31 +=
                tel.bin_fallback_otsu_to_adaptive31;
            stats.stage_telemetry.bin_fallback_adaptive31_to_adaptive21 +=
                tel.bin_fallback_adaptive31_to_adaptive21;
            stats.stage_telemetry.bin_fallback_successes += tel.bin_fallback_successes;
            if tel.rerank_enabled {
                stats.stage_telemetry.rerank_enabled += 1;
            }
            stats.stage_telemetry.rerank_top1_attempts += tel.rerank_top1_attempts;
            stats.stage_telemetry.rerank_top1_successes += tel.rerank_top1_successes;
            stats.stage_telemetry.rerank_transform_reject_count +=
                tel.rerank_transform_reject_count;
            if tel.saturation_mask_enabled {
                stats.stage_telemetry.saturation_mask_enabled += 1;
            }
            stats.stage_telemetry.saturation_mask_coverage_sum +=
                tel.saturation_mask_coverage as f64;
            stats.stage_telemetry.saturation_mask_decode_successes +=
                tel.saturation_mask_decode_successes;
            stats.stage_telemetry.roi_norm_attempts += tel.roi_norm_attempts;
            stats.stage_telemetry.roi_norm_successes += tel.roi_norm_successes;
            stats.stage_telemetry.roi_norm_skipped += tel.roi_norm_skipped;
            if tel.two_finder_successes > 0 || tel.two_finder_attempts > 0 {
                stats.stage_telemetry.two_finder_used += 1;
            }
            if tel.router_multi_region {
                stats.stage_telemetry.router_multi_region += 1;
            }
            stats.stage_telemetry.router_blur_metric_sum += tel.router_blur_metric as f64;
            stats.stage_telemetry.router_saturation_ratio_sum += tel.router_saturation_ratio as f64;
            stats.stage_telemetry.router_skew_estimate_deg_sum +=
                tel.router_skew_estimate_deg as f64;
            stats.stage_telemetry.router_region_density_proxy_sum +=
                tel.router_region_density_proxy as f64;
            stats.stage_telemetry.acceptance_rejected += tel.acceptance_rejected;
            stats.stage_telemetry.deskew_attempts += tel.deskew_attempts;
            stats.stage_telemetry.deskew_successes += tel.deskew_successes;
            stats.stage_telemetry.high_version_precision_attempts +=
                tel.high_version_precision_attempts;
            stats.stage_telemetry.recovery_mode_attempts += tel.recovery_mode_attempts;
            stats.stage_telemetry.scale_retry_attempts += tel.scale_retry_attempts;
            stats.stage_telemetry.scale_retry_successes += tel.scale_retry_successes;
            stats.stage_telemetry.scale_retry_skipped_by_budget +=
                tel.scale_retry_skipped_by_budget;
            stats.stage_telemetry.hv_subpixel_attempts += tel.hv_subpixel_attempts;
            stats.stage_telemetry.hv_refine_attempts += tel.hv_refine_attempts;
            stats.stage_telemetry.hv_refine_successes += tel.hv_refine_successes;
            stats.stage_telemetry.rs_erasure_attempts += tel.rs_erasure_attempts;
            stats.stage_telemetry.rs_erasure_successes += tel.rs_erasure_successes;
            for i in 0..stats.stage_telemetry.rs_erasure_count_hist.len() {
                stats.stage_telemetry.rs_erasure_count_hist[i] += tel.rs_erasure_count_hist[i];
            }
            stats.stage_telemetry.phase11_time_budget_skips += tel.phase11_time_budget_skips;

            if image_hits == 0 {
                let signature = classify_failure_signature(&tel);
                let row = stats
                    .failure_clusters
                    .entry(signature.to_string())
                    .or_insert(FailureCluster {
                        count: 0,
                        qr_weight: 0,
                        examples: Vec::new(),
                    });
                row.count += 1;
                row.qr_weight += expected;
                if row.examples.len() < 3 {
                    row.examples.push(path.display().to_string());
                }
            }

            if !non_interactive {
                println!(
                    "  [{}] {} -> {}/{} ({:.2?}) [finders={} groups={} transforms={} attempts={}]",
                    stats.images_with_labels,
                    path.display(),
                    decoded,
                    expected,
                    elapsed,
                    tel.finder_patterns_found,
                    tel.groups_found,
                    tel.transforms_built,
                    tel.decode_attempts,
                );
            } else if progress_every > 0 && stats.images_with_labels % progress_every == 0 {
                println!(
                    "  progress: {}/? labeled images, hits {}/{} | last_ms={:.2} | last={}",
                    stats.images_with_labels,
                    stats.hits,
                    stats.total_expected,
                    elapsed_ms,
                    path.display()
                );
            }
        } else {
            let elapsed_ms = end_to_end_start.elapsed().as_secs_f64() * 1_000.0;
            stats.runtime_samples_ms.push(elapsed_ms);
            if !payload_validated {
                stats.false_negatives += expected;
            }
            stats.timeouts += usize::from(deadline_exceeded(elapsed_ms, timeout_ms));
            let row = stats
                .failure_clusters
                .entry("load-failed".to_string())
                .or_insert(FailureCluster {
                    count: 0,
                    qr_weight: 0,
                    examples: Vec::new(),
                });
            row.count += 1;
            row.qr_weight += expected;
            if row.examples.len() < 3 {
                row.examples.push(path.display().to_string());
            }
            if !non_interactive {
                println!(
                    "  [{}] {} -> load_failed (expected {})",
                    stats.images_with_labels,
                    path.display(),
                    expected,
                );
            }
        }
    }

    Ok(stats)
}

fn deadline_exceeded(elapsed_ms: f64, timeout_ms: u64) -> bool {
    timeout_ms > 0 && elapsed_ms > timeout_ms as f64
}

fn classify_failure_signature(tel: &rust_qr::DetectionTelemetry) -> &'static str {
    if tel.budget_skips > 0 && tel.payload_decoded == 0 {
        return "over-budget-skip";
    }
    if tel.finder_patterns_found == 0 {
        return "no-finders";
    }
    if tel.groups_found == 0 {
        return "no-groups";
    }
    if tel.transforms_built == 0 {
        return "transform-fail";
    }
    if tel.format_extracted == 0 {
        return "format-fail";
    }
    if tel.rs_decode_ok == 0 {
        return "rs-fail";
    }
    if tel.payload_decoded == 0 {
        return "payload-fail";
    }
    "unknown-fail"
}

fn utc_timestamp() -> String {
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn commit_sha() -> String {
    if let Ok(value) = std::env::var("GITHUB_SHA") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(&mut out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn write_reading_rate_artifact(path: &Path, artifact: &ReadingRateArtifact) {
    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"schema_version\": \"rustqr.reading_rate.v2\",\n");
    json.push_str("  \"metadata\": {\n");
    let _ = writeln!(
        &mut json,
        "    \"dataset_root\": \"{}\",",
        json_escape(&artifact.dataset_root)
    );
    let _ = writeln!(
        &mut json,
        "    \"dataset_fingerprint\": \"{}\",",
        json_escape(&artifact.dataset_fingerprint)
    );
    let _ = writeln!(
        &mut json,
        "    \"label_fingerprint\": \"{}\",",
        json_escape(&artifact.label_fingerprint)
    );
    let _ = writeln!(
        &mut json,
        "    \"evaluator_fingerprint\": \"{}\",",
        json_escape(&artifact.evaluator_fingerprint)
    );
    let _ = writeln!(
        &mut json,
        "    \"preprocessing_fingerprint\": \"{}\",",
        json_escape(&artifact.preprocessing_fingerprint)
    );
    match &artifact.selected_category {
        Some(category) => {
            let _ = writeln!(
                &mut json,
                "    \"selected_category\": \"{}\",",
                json_escape(category)
            );
        }
        None => json.push_str("    \"selected_category\": null,\n"),
    }
    let _ = writeln!(
        &mut json,
        "    \"commit_sha\": \"{}\",",
        json_escape(&artifact.commit_sha)
    );
    let _ = writeln!(
        &mut json,
        "    \"timestamp_utc\": \"{}\",",
        json_escape(&artifact.timestamp_utc)
    );
    match artifact.limit_per_category {
        Some(limit) => {
            let _ = writeln!(&mut json, "    \"limit_per_category\": {limit},");
        }
        None => json.push_str("    \"limit_per_category\": null,\n"),
    }
    let _ = writeln!(&mut json, "    \"smoke\": {},", artifact.smoke);
    let _ = writeln!(&mut json, "    \"timeout_ms\": {},", artifact.timeout_ms);
    json.push_str("    \"timeout_semantics\": \"cooperative_deadline_discard_late_results\",\n");
    let _ = writeln!(
        &mut json,
        "    \"non_interactive\": {}",
        artifact.non_interactive
    );
    json.push_str("  },\n");
    json.push_str("  \"summary\": {\n");
    let _ = writeln!(
        &mut json,
        "    \"weighted_global_rate_percent\": {:.4},",
        artifact.weighted_global_rate_percent
    );
    let _ = writeln!(&mut json, "    \"total_hits\": {},", artifact.total_hits);
    let _ = writeln!(
        &mut json,
        "    \"total_expected\": {},",
        artifact.total_expected
    );
    let _ = writeln!(
        &mut json,
        "    \"total_images_with_labels\": {},",
        artifact.total_images_with_labels
    );
    let precision_denominator = artifact.total_hits + artifact.false_positives;
    let precision = if precision_denominator == 0 {
        0.0
    } else {
        artifact.total_hits as f64 / precision_denominator as f64
    };
    let recall_denominator = artifact.total_hits + artifact.false_negatives;
    let recall = if recall_denominator == 0 {
        0.0
    } else {
        artifact.total_hits as f64 / recall_denominator as f64
    };
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    if artifact.payload_ground_truth_available {
        json.push_str("    \"localization_precision\": null,\n");
        json.push_str("    \"localization_recall\": null,\n");
        json.push_str("    \"localization_f1\": null,\n");
        json.push_str("    \"payload_ground_truth_available\": true,\n");
        let _ = writeln!(
            &mut json,
            "    \"payload_exact_matches\": {},",
            artifact.total_hits
        );
        let payload_rate = if artifact.total_expected == 0 {
            0.0
        } else {
            artifact.total_hits as f64 / artifact.total_expected as f64
        };
        let _ = writeln!(
            &mut json,
            "    \"payload_exact_match_rate\": {:.6},",
            payload_rate
        );
    } else {
        let _ = writeln!(
            &mut json,
            "    \"localization_precision\": {:.6},",
            precision
        );
        let _ = writeln!(&mut json, "    \"localization_recall\": {:.6},", recall);
        let _ = writeln!(&mut json, "    \"localization_f1\": {:.6},", f1);
        json.push_str("    \"payload_ground_truth_available\": false,\n");
        json.push_str("    \"payload_exact_matches\": null,\n");
        json.push_str("    \"payload_exact_match_rate\": null,\n");
    }
    let _ = writeln!(
        &mut json,
        "    \"false_positives\": {},",
        artifact.false_positives
    );
    let _ = writeln!(
        &mut json,
        "    \"false_negatives\": {},",
        artifact.false_negatives
    );
    let _ = writeln!(
        &mut json,
        "    \"duplicate_predictions\": {},",
        artifact.duplicate_predictions
    );
    let timeout_rate = if artifact.total_images_with_labels == 0 {
        0.0
    } else {
        artifact.timeouts as f64 / artifact.total_images_with_labels as f64
    };
    let _ = writeln!(&mut json, "    \"timeouts\": {},", artifact.timeouts);
    let _ = writeln!(&mut json, "    \"timeout_rate\": {:.6},", timeout_rate);
    let seconds = artifact.global_runtime.total_ms / 1000.0;
    let images_per_second = if seconds > 0.0 {
        artifact.total_images_with_labels as f64 / seconds
    } else {
        0.0
    };
    // Throughput is independent of correctness: count annotated symbols
    // presented to the detector, not only successful matches.
    let symbols_per_second = if seconds > 0.0 {
        artifact.total_expected as f64 / seconds
    } else {
        0.0
    };
    let _ = writeln!(
        &mut json,
        "    \"images_per_second\": {:.6},",
        images_per_second
    );
    let _ = writeln!(
        &mut json,
        "    \"qr_symbols_per_second\": {:.6},",
        symbols_per_second
    );
    write_runtime_json(&mut json, "core_runtime", artifact.global_core_runtime, 4);
    json.push_str(",\n");
    write_runtime_json(&mut json, "end_to_end_runtime", artifact.global_runtime, 4);
    json.push_str("  },\n");
    json.push_str("  \"categories\": [\n");
    for (idx, category) in artifact.categories.iter().enumerate() {
        json.push_str("    {\n");
        let _ = writeln!(
            &mut json,
            "      \"name\": \"{}\",",
            json_escape(category.name)
        );
        let _ = writeln!(
            &mut json,
            "      \"description\": \"{}\",",
            json_escape(category.description)
        );
        let _ = writeln!(&mut json, "      \"hits\": {},", category.hits);
        let _ = writeln!(
            &mut json,
            "      \"total_expected\": {},",
            category.total_expected
        );
        let _ = writeln!(
            &mut json,
            "      \"images_with_labels\": {},",
            category.images_with_labels
        );
        let rate = if category.total_expected == 0 {
            0.0
        } else {
            (category.hits as f64 / category.total_expected as f64) * 100.0
        };
        let _ = writeln!(&mut json, "      \"rate_percent\": {:.4},", rate);
        let _ = writeln!(
            &mut json,
            "      \"false_positives\": {},",
            category.false_positives
        );
        let _ = writeln!(
            &mut json,
            "      \"false_negatives\": {},",
            category.false_negatives
        );
        let _ = writeln!(
            &mut json,
            "      \"duplicate_predictions\": {},",
            category.duplicate_predictions
        );
        let category_timeout_rate = if category.images_with_labels == 0 {
            0.0
        } else {
            category.timeouts as f64 / category.images_with_labels as f64
        };
        let _ = writeln!(&mut json, "      \"timeouts\": {},", category.timeouts);
        let _ = writeln!(
            &mut json,
            "      \"timeout_rate\": {:.6},",
            category_timeout_rate
        );
        json.push_str("      \"stage_telemetry\": {\n");
        let _ = writeln!(
            &mut json,
            "        \"total\": {},",
            category.stage_telemetry.total
        );
        let _ = writeln!(
            &mut json,
            "        \"binarize_ok\": {},",
            category.stage_telemetry.binarize_ok
        );
        let _ = writeln!(
            &mut json,
            "        \"finder_ok\": {},",
            category.stage_telemetry.finder_ok
        );
        let _ = writeln!(
            &mut json,
            "        \"groups_ok\": {},",
            category.stage_telemetry.groups_ok
        );
        let _ = writeln!(
            &mut json,
            "        \"transform_ok\": {},",
            category.stage_telemetry.transform_ok
        );
        let _ = writeln!(
            &mut json,
            "        \"decode_ok\": {},",
            category.stage_telemetry.decode_ok
        );
        let _ = writeln!(
            &mut json,
            "        \"total_decode_attempts\": {},",
            category.stage_telemetry.total_decode_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"over_budget_skip\": {},",
            category.stage_telemetry.over_budget_skip
        );
        let _ = writeln!(
            &mut json,
            "        \"budget_lane_high\": {},",
            category.stage_telemetry.budget_lane_high
        );
        let _ = writeln!(
            &mut json,
            "        \"budget_lane_medium\": {},",
            category.stage_telemetry.budget_lane_medium
        );
        let _ = writeln!(
            &mut json,
            "        \"budget_lane_low\": {},",
            category.stage_telemetry.budget_lane_low
        );
        let _ = writeln!(
            &mut json,
            "        \"bin_fallback_otsu_to_adaptive31\": {},",
            category.stage_telemetry.bin_fallback_otsu_to_adaptive31
        );
        let _ = writeln!(
            &mut json,
            "        \"bin_fallback_adaptive31_to_adaptive21\": {},",
            category
                .stage_telemetry
                .bin_fallback_adaptive31_to_adaptive21
        );
        let _ = writeln!(
            &mut json,
            "        \"bin_fallback_successes\": {},",
            category.stage_telemetry.bin_fallback_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"rerank_enabled\": {},",
            category.stage_telemetry.rerank_enabled
        );
        let _ = writeln!(
            &mut json,
            "        \"rerank_top1_attempts\": {},",
            category.stage_telemetry.rerank_top1_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"rerank_top1_successes\": {},",
            category.stage_telemetry.rerank_top1_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"rerank_transform_reject_count\": {},",
            category.stage_telemetry.rerank_transform_reject_count
        );
        let _ = writeln!(
            &mut json,
            "        \"saturation_mask_enabled\": {},",
            category.stage_telemetry.saturation_mask_enabled
        );
        let _ = writeln!(
            &mut json,
            "        \"saturation_mask_coverage_sum\": {:.6},",
            category.stage_telemetry.saturation_mask_coverage_sum
        );
        let _ = writeln!(
            &mut json,
            "        \"saturation_mask_decode_successes\": {},",
            category.stage_telemetry.saturation_mask_decode_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"roi_norm_attempts\": {},",
            category.stage_telemetry.roi_norm_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"roi_norm_successes\": {},",
            category.stage_telemetry.roi_norm_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"roi_norm_skipped\": {},",
            category.stage_telemetry.roi_norm_skipped
        );
        let _ = writeln!(
            &mut json,
            "        \"two_finder_used\": {},",
            category.stage_telemetry.two_finder_used
        );
        let _ = writeln!(
            &mut json,
            "        \"router_multi_region\": {},",
            category.stage_telemetry.router_multi_region
        );
        let _ = writeln!(
            &mut json,
            "        \"router_blur_metric_sum\": {:.6},",
            category.stage_telemetry.router_blur_metric_sum
        );
        let _ = writeln!(
            &mut json,
            "        \"router_saturation_ratio_sum\": {:.6},",
            category.stage_telemetry.router_saturation_ratio_sum
        );
        let _ = writeln!(
            &mut json,
            "        \"router_skew_estimate_deg_sum\": {:.6},",
            category.stage_telemetry.router_skew_estimate_deg_sum
        );
        let _ = writeln!(
            &mut json,
            "        \"router_region_density_proxy_sum\": {:.6},",
            category.stage_telemetry.router_region_density_proxy_sum
        );
        let _ = writeln!(
            &mut json,
            "        \"acceptance_rejected\": {},",
            category.stage_telemetry.acceptance_rejected
        );
        let _ = writeln!(
            &mut json,
            "        \"deskew_attempts\": {},",
            category.stage_telemetry.deskew_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"deskew_successes\": {},",
            category.stage_telemetry.deskew_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"high_version_precision_attempts\": {},",
            category.stage_telemetry.high_version_precision_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"recovery_mode_attempts\": {},",
            category.stage_telemetry.recovery_mode_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"scale_retry_attempts\": {},",
            category.stage_telemetry.scale_retry_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"scale_retry_successes\": {},",
            category.stage_telemetry.scale_retry_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"scale_retry_skipped_by_budget\": {},",
            category.stage_telemetry.scale_retry_skipped_by_budget
        );
        let _ = writeln!(
            &mut json,
            "        \"hv_subpixel_attempts\": {},",
            category.stage_telemetry.hv_subpixel_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"hv_refine_attempts\": {},",
            category.stage_telemetry.hv_refine_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"hv_refine_successes\": {},",
            category.stage_telemetry.hv_refine_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"rs_erasure_attempts\": {},",
            category.stage_telemetry.rs_erasure_attempts
        );
        let _ = writeln!(
            &mut json,
            "        \"rs_erasure_successes\": {},",
            category.stage_telemetry.rs_erasure_successes
        );
        let _ = writeln!(
            &mut json,
            "        \"rs_erasure_count_hist\": [{}, {}, {}, {}],",
            category.stage_telemetry.rs_erasure_count_hist[0],
            category.stage_telemetry.rs_erasure_count_hist[1],
            category.stage_telemetry.rs_erasure_count_hist[2],
            category.stage_telemetry.rs_erasure_count_hist[3]
        );
        let _ = writeln!(
            &mut json,
            "        \"phase11_time_budget_skips\": {},",
            category.stage_telemetry.phase11_time_budget_skips
        );
        let _ = writeln!(
            &mut json,
            "        \"candidate_score_buckets\": [{}, {}, {}, {}],",
            category.stage_telemetry.candidate_score_buckets[0],
            category.stage_telemetry.candidate_score_buckets[1],
            category.stage_telemetry.candidate_score_buckets[2],
            category.stage_telemetry.candidate_score_buckets[3],
        );
        let _ = writeln!(
            &mut json,
            "        \"attempts_used_histogram\": [{}, {}, {}, {}, {}]",
            category.stage_telemetry.attempts_used_histogram[0],
            category.stage_telemetry.attempts_used_histogram[1],
            category.stage_telemetry.attempts_used_histogram[2],
            category.stage_telemetry.attempts_used_histogram[3],
            category.stage_telemetry.attempts_used_histogram[4],
        );
        json.push_str("      },\n");
        write_runtime_json(&mut json, "core_runtime", category.core_runtime, 6);
        json.push_str(",\n");
        write_runtime_json(&mut json, "end_to_end_runtime", category.runtime, 6);
        json.push_str("    }");
        if idx + 1 != artifact.categories.len() {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ],\n");
    json.push_str("  \"failure_clusters\": [\n");
    for (idx, cluster) in artifact.failure_clusters.iter().enumerate() {
        json.push_str("    {\n");
        let _ = writeln!(
            &mut json,
            "      \"signature\": \"{}\",",
            json_escape(&cluster.signature)
        );
        let _ = writeln!(&mut json, "      \"count\": {},", cluster.count);
        let _ = writeln!(&mut json, "      \"qr_weight\": {},", cluster.qr_weight);
        json.push_str("      \"examples\": [");
        for (ei, ex) in cluster.examples.iter().enumerate() {
            if ei > 0 {
                json.push_str(", ");
            }
            let _ = write!(&mut json, "\"{}\"", json_escape(ex));
        }
        json.push_str("]\n");
        json.push_str("    }");
        if idx + 1 != artifact.failure_clusters.len() {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ]\n");
    json.push_str("}\n");

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Failed to create artifact parent directory {}: {err}",
                parent.display()
            );
            return;
        }
    }
    if let Err(err) = fs::write(path, json) {
        eprintln!("Failed to write artifact {}: {err}", path.display());
    }
}

fn write_runtime_json(json: &mut String, key: &str, runtime: RuntimeSummary, indent: usize) {
    let pad = " ".repeat(indent);
    let child = " ".repeat(indent + 2);
    let _ = writeln!(json, "{pad}\"{key}\": {{");
    let _ = writeln!(json, "{child}\"samples\": {},", runtime.samples);
    let _ = writeln!(json, "{child}\"total_ms\": {:.4},", runtime.total_ms);
    let _ = writeln!(
        json,
        "{child}\"mean_per_image_ms\": {:.4},",
        runtime.mean_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"median_per_image_ms\": {:.4},",
        runtime.median_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"p50_per_image_ms\": {:.4},",
        runtime.median_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"p90_per_image_ms\": {:.4},",
        runtime.p90_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"p95_per_image_ms\": {:.4},",
        runtime.p95_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"p99_per_image_ms\": {:.4},",
        runtime.p99_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"min_per_image_ms\": {:.4},",
        runtime.min_per_image_ms
    );
    let _ = writeln!(
        json,
        "{child}\"max_per_image_ms\": {:.4}",
        runtime.max_per_image_ms
    );
    let _ = writeln!(json, "{pad}}}");
}

fn dataset_bench_cmd(root: Option<PathBuf>, limit: Option<usize>, smoke: bool) {
    let root = root.unwrap_or_else(dataset_root_from_env);
    let limit = limit.or_else(bench_limit_from_env);
    let smoke = smoke || smoke_from_env();

    if !root.exists() {
        eprintln!("Dataset root not found: {}", root.display());
        return;
    }

    let images: Vec<PathBuf> = dataset_iter(&root, limit, smoke).collect();
    if images.is_empty() {
        println!("No images found under {}", root.display());
        return;
    }

    let mut total_elapsed = std::time::Duration::default();

    for path in images {
        let (pixels, width, height) = match load_rgb(&path) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("Failed to load {}: {}", path.display(), err);
                continue;
            }
        };

        let start = Instant::now();
        let results = detect_qr(&pixels, width, height);
        let elapsed = start.elapsed();
        total_elapsed += elapsed;

        println!(
            "{}: {}x{} -> {} results ({:.2?})",
            path.display(),
            width,
            height,
            results.len(),
            elapsed
        );
    }

    println!("Total time: {:.2?}", total_elapsed);
}

#[cfg(test)]
mod tests {
    use super::{deadline_exceeded, reading_rate_for_images};
    use std::fs;

    #[test]
    fn timeout_is_disabled_by_zero_and_strictly_exceeds_deadline() {
        assert!(!deadline_exceeded(10_000.0, 0));
        assert!(!deadline_exceeded(25.0, 25));
        assert!(deadline_exceeded(25.001, 25));
    }

    #[test]
    fn invalid_label_fails_the_evaluation() {
        let stem = format!(
            "rustqr-invalid-label-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        );
        let image = std::env::temp_dir().join(format!("{stem}.png"));
        let label = image.with_extension("txt");
        fs::write(&label, "SETS\nnot-a-quadrilateral\n").expect("write test label");

        let result = reading_rate_for_images(std::iter::once(image), true, 0, 0, false);

        let _ = fs::remove_file(label);
        match result {
            Err(error) => assert!(error.contains("invalid label")),
            Ok(_) => panic!("invalid label unexpectedly succeeded"),
        }
    }

    #[test]
    fn image_load_failure_counts_as_a_miss_and_runtime_sample() {
        let stem = format!("rustqr-missing-image-{}", std::process::id());
        let image = std::env::temp_dir().join(format!("{stem}.png"));
        let label = image.with_extension("txt");
        fs::write(&label, "SETS\n0 0 10 0 10 10 0 10\n").expect("write test label");

        let stats = reading_rate_for_images(std::iter::once(image), true, 0, 0, false)
            .expect("valid labels should evaluate");

        let _ = fs::remove_file(label);
        assert_eq!(stats.total_expected, 1);
        assert_eq!(stats.false_negatives, 1);
        assert_eq!(stats.runtime_samples_ms.len(), 1);
        assert_eq!(stats.failure_clusters["load-failed"].count, 1);
    }

    #[test]
    fn explicit_payload_mode_accepts_multiline_payload_labels() {
        let stem = format!("rustqr-missing-payload-image-{}", std::process::id());
        let image = std::env::temp_dir().join(format!("{stem}.png"));
        let label = image.with_extension("txt");
        fs::write(&label, "BEGIN:VEVENT\r\nSUMMARY: Test\r\nEND:VEVENT\r\n")
            .expect("write payload label");

        let stats = reading_rate_for_images(std::iter::once(image), true, 0, 0, true)
            .expect("payload mode should parse the complete label as one payload");

        let _ = fs::remove_file(label);
        assert_eq!(stats.total_expected, 1);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.false_negatives, 0);
    }

    #[test]
    fn explicit_payload_mode_rejects_empty_payload_labels() {
        let stem = format!("rustqr-empty-payload-{}", std::process::id());
        let image = std::env::temp_dir().join(format!("{stem}.png"));
        let label = image.with_extension("txt");
        fs::write(&label, " \r\n\t").expect("write empty payload label");

        let result = reading_rate_for_images(std::iter::once(image), true, 0, 0, true);

        let _ = fs::remove_file(label);
        match result {
            Err(error) => assert!(error.contains("invalid payload label")),
            Ok(_) => panic!("empty payload label unexpectedly succeeded"),
        }
    }
}
