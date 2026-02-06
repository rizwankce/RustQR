use clap::{Parser, Subcommand};
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
use rust_qr::models::{BitMatrix, Point};
use rust_qr::tools::{
    bench_limit_from_env, binarize, binary_stats, dataset_iter, dataset_root_from_env, detect_qr,
    grayscale_stats, load_rgb, parse_expected_qr_count, smoke_from_env, to_grayscale,
};
use rust_qr::utils::geometry::PerspectiveTransform;
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
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        smoke: bool,
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
        Command::ReadingRate { root, limit, smoke } => reading_rate_cmd(root, limit, smoke),
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

fn reading_rate_cmd(root: Option<PathBuf>, limit: Option<usize>, smoke: bool) {
    let root = root.unwrap_or_else(dataset_root_from_env);
    let limit = limit.or_else(bench_limit_from_env);
    let smoke = smoke || smoke_from_env();

    if !root.exists() {
        eprintln!("Dataset root not found: {}", root.display());
        return;
    }

    let limited_images: Option<Vec<PathBuf>> = if smoke || limit.is_some() {
        Some(dataset_iter(&root, limit, smoke).collect())
    } else {
        None
    };
    if let Some(images) = &limited_images {
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
    let datetime = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let commit_sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("RustQR QR Code Reading Rate Benchmark");
    println!("=====================================");
    println!("Date:    {}", datetime);
    println!("Commit:  {}", commit_sha);
    println!("Dataset: {}", root.display());
    if let Some(l) = limit {
        println!("Limit:   {} images per category", l);
    } else {
        println!("Limit:   full dataset");
    }
    if smoke {
        println!("Mode:    smoke test");
    }
    println!("=====================================\n");

    let mut global_hits = 0usize;
    let mut global_expected = 0usize;
    let mut category_results: Vec<(&str, usize, usize, usize, StageTelemetry)> = Vec::new();
    let mut categories_found = 0usize;

    for (dir, description) in categories {
        let category_root = root.join(dir);
        if !category_root.exists() {
            continue;
        }
        categories_found += 1;
        println!("Testing: {} - {}", dir, description);
        let images: Vec<PathBuf> = if let Some(images) = &limited_images {
            images
                .iter()
                .filter(|path| {
                    path.strip_prefix(&root)
                        .ok()
                        .and_then(|rel| rel.components().next())
                        .map(|c| c.as_os_str() == dir)
                        .unwrap_or(false)
                })
                .cloned()
                .collect()
        } else {
            dataset_iter(&category_root, None, false).collect()
        };
        if images.is_empty() {
            println!("  {}: no images found\n", dir);
            continue;
        }
        let stats = reading_rate_for_images(images.into_iter());
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
        category_results.push((
            dir,
            stats.hits,
            stats.total_expected,
            stats.images_with_labels,
            stats.stage_telemetry,
        ));
    }

    if categories_found > 0 && global_expected > 0 {
        let global_rate = (global_hits as f64 / global_expected as f64) * 100.0;

        println!("=====================================");
        println!("Reading Rate Summary");
        println!("=====================================");
        println!(
            "{:<16} {:>6} {:>6} {:>8}",
            "Category", "Hits", "Total", "Rate"
        );
        println!("{}", "-".repeat(40));
        for (dir, hits, expected, _, _) in &category_results {
            let rate = if *expected > 0 {
                (*hits as f64 / *expected as f64) * 100.0
            } else {
                0.0
            };
            println!("{:<16} {:>6} {:>6} {:>7.2}%", dir, hits, expected, rate);
        }
        println!("{}", "-".repeat(40));
        println!(
            "{:<16} {:>6} {:>6} {:>7.2}%",
            "TOTAL", global_hits, global_expected, global_rate,
        );
        println!("=====================================\n");

        // Stage telemetry table
        println!("Pipeline Stage Telemetry (images passing each stage)");
        println!("=====================================");
        println!(
            "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "Category", "Imgs", "Binarize", "Finders", "Groups", "Xform", "Decode"
        );
        println!("{}", "-".repeat(74));
        let mut g_total = 0usize;
        let mut g_bin = 0usize;
        let mut g_find = 0usize;
        let mut g_grp = 0usize;
        let mut g_xfm = 0usize;
        let mut g_dec = 0usize;
        for (dir, _, _, _, tel) in &category_results {
            println!(
                "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
                dir,
                tel.total,
                tel.binarize_ok,
                tel.finder_ok,
                tel.groups_ok,
                tel.transform_ok,
                tel.decode_ok,
            );
            g_total += tel.total;
            g_bin += tel.binarize_ok;
            g_find += tel.finder_ok;
            g_grp += tel.groups_ok;
            g_xfm += tel.transform_ok;
            g_dec += tel.decode_ok;
        }
        println!("{}", "-".repeat(74));
        println!(
            "{:<16} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "TOTAL", g_total, g_bin, g_find, g_grp, g_xfm, g_dec,
        );
        println!("=====================================");
        return;
    }

    let images: Vec<PathBuf> =
        limited_images.unwrap_or_else(|| dataset_iter(&root, None, false).collect());
    if images.is_empty() {
        println!("No images found under {}", root.display());
        return;
    }
    let stats = reading_rate_for_images(images.into_iter());
    if stats.total_expected == 0 {
        println!("No labeled images found under {}", root.display());
        return;
    }
    let rate = (stats.hits as f64 / stats.total_expected as f64) * 100.0;
    println!(
        "Reading rate: {}/{} = {:.2}%",
        stats.hits, stats.total_expected, rate
    );
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
}

/// Aggregated pipeline-stage failure counts across a set of images.
#[derive(Default)]
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
    /// Total images processed.
    total: usize,
}

fn reading_rate_for_images<I>(images: I) -> ReadingRateStats
where
    I: Iterator<Item = PathBuf>,
{
    let mut stats = ReadingRateStats {
        hits: 0,
        total_expected: 0,
        images_with_labels: 0,
        stage_telemetry: StageTelemetry::default(),
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
            let decoded = results.len();
            let image_hits = decoded.min(expected);
            stats.hits += image_hits;

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

            println!(
                "  [{}] {} -> {}/{} ({:.2?}) [finders={} groups={} transforms={}]",
                stats.images_with_labels,
                path.display(),
                decoded,
                expected,
                elapsed,
                tel.finder_patterns_found,
                tel.groups_found,
                tel.transforms_built,
            );
        } else {
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
