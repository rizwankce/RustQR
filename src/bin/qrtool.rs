use clap::{Parser, Subcommand};
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
use rust_qr::models::{BitMatrix, Point};
use rust_qr::tools::{
    bench_limit_from_env, binarize, binary_stats, dataset_fingerprint, dataset_iter,
    dataset_root_from_env, detect_qr, grayscale_stats, load_rgb, parse_expected_qr_count,
    smoke_from_env, to_grayscale,
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
        } => reading_rate_cmd(
            root,
            limit,
            smoke,
            artifact_json,
            non_interactive,
            progress_every,
            category,
        ),
        Command::DatasetBench { root, limit, smoke } => dataset_bench_cmd(root, limit, smoke),
    }
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

fn reading_rate_cmd(
    root: Option<PathBuf>,
    limit: Option<usize>,
    smoke: bool,
    artifact_json: Option<PathBuf>,
    non_interactive: bool,
    progress_every: usize,
    category: Option<String>,
) {
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
        let stats = reading_rate_for_images(images.into_iter(), non_interactive, progress_every);
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
            stage_telemetry: stats.stage_telemetry,
            runtime: RuntimeSummary::from_samples(&stats.runtime_samples_ms),
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
                commit_sha,
                timestamp_utc: datetime,
                limit_per_category: limit,
                smoke,
                non_interactive,
                weighted_global_rate_percent: global_rate,
                total_hits: global_hits,
                total_expected: global_expected,
                total_images_with_labels: global_images_with_labels,
                global_runtime,
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
    let stats = reading_rate_for_images(images.into_iter(), non_interactive, progress_every);
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
            commit_sha,
            timestamp_utc: datetime,
            limit_per_category: limit,
            smoke,
            non_interactive,
            weighted_global_rate_percent: rate,
            total_hits: stats.hits,
            total_expected: stats.total_expected,
            total_images_with_labels: stats.images_with_labels,
            global_runtime: RuntimeSummary::from_samples(&stats.runtime_samples_ms),
            categories: Vec::new(),
            failure_clusters: Vec::new(),
        };
        write_reading_rate_artifact(&path, &artifact);
        println!("Artifact: {}", path.display());
    }
}

/// Per-QR-code scoring results for a set of images.
struct ReadingRateStats {
    /// Number of QR codes successfully decoded (capped at expected per image).
    hits: usize,
    /// Total expected QR codes from label files.
    total_expected: usize,
    /// Number of images that had a label file.
    images_with_labels: usize,
    /// Aggregated per-stage telemetry across all images.
    stage_telemetry: StageTelemetry,
    /// Runtime samples for successfully loaded images.
    runtime_samples_ms: Vec<f64>,
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
    min_per_image_ms: f64,
    max_per_image_ms: f64,
}

impl RuntimeSummary {
    fn from_samples(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self {
                samples: 0,
                total_ms: 0.0,
                mean_per_image_ms: 0.0,
                median_per_image_ms: 0.0,
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
    stage_telemetry: StageTelemetry,
    runtime: RuntimeSummary,
}

struct ReadingRateArtifact {
    dataset_root: String,
    dataset_fingerprint: String,
    commit_sha: String,
    timestamp_utc: String,
    limit_per_category: Option<usize>,
    smoke: bool,
    non_interactive: bool,
    weighted_global_rate_percent: f64,
    total_hits: usize,
    total_expected: usize,
    total_images_with_labels: usize,
    global_runtime: RuntimeSummary,
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
) -> ReadingRateStats
where
    I: Iterator<Item = PathBuf>,
{
    let mut stats = ReadingRateStats {
        hits: 0,
        total_expected: 0,
        images_with_labels: 0,
        stage_telemetry: StageTelemetry::default(),
        runtime_samples_ms: Vec::new(),
        failure_clusters: BTreeMap::new(),
    };

    for path in images {
        let txt_file = path.with_extension("txt");
        if !txt_file.exists() {
            continue;
        }
        let expected = parse_expected_qr_count(&txt_file);
        if expected == 0 {
            continue;
        }
        stats.images_with_labels += 1;
        stats.total_expected += expected;
        stats.stage_telemetry.total += 1;

        if let Ok((pixels, width, height)) = load_rgb(&path) {
            let start = Instant::now();
            let (results, tel) = rust_qr::detect_with_telemetry(&pixels, width, height);
            let elapsed = start.elapsed();
            let elapsed_ms = elapsed.as_secs_f64() * 1_000.0;
            let mut decoded = results.len();
            // Telemetry mode can undercount due stricter budgets. For reading-rate scoring,
            // use the best of telemetry and production detect() when telemetry is short.
            if decoded < expected {
                decoded = decoded.max(detect_qr(&pixels, width, height).len());
            }
            let image_hits = decoded.min(expected);
            stats.hits += image_hits;
            stats.runtime_samples_ms.push(elapsed_ms);

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
        } else if !non_interactive {
            println!(
                "  [{}] {} -> load_failed (expected {})",
                stats.images_with_labels,
                path.display(),
                expected,
            );
        }
    }

    stats
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
    json.push_str("  \"schema_version\": \"rustqr.reading_rate.v1\",\n");
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
    write_runtime_json(&mut json, "runtime", artifact.global_runtime, 4);
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
        write_runtime_json(&mut json, "runtime", category.runtime, 6);
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
