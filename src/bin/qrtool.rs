use clap::{Parser, Subcommand};
use rust_qr::decoder::format::FormatInfo;
use rust_qr::detector::finder::FinderDetector;
use rust_qr::models::{BitMatrix, Point};
use rust_qr::tools::{
    bench_limit_from_env, binarize, binary_stats, dataset_iter, dataset_root_from_env, detect_qr,
    grayscale_stats, load_rgb, smoke_from_env, to_grayscale,
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
    let others = vec![pts[1], pts[2]];
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

    let mut total_rate = 0.0;
    let mut count = 0usize;
    let mut categories_found = 0usize;

    for (dir, description) in categories {
        let category_root = root.join(dir);
        if !category_root.exists() {
            continue;
        }
        if categories_found == 0 {
            println!("RustQR QR Code Reading Rate Benchmark");
            println!("=====================================\n");
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
            println!("  {}: no images found", dir);
            continue;
        }
        let (successful, total) = reading_rate_for_images(images.into_iter());
        if total == 0 {
            println!("  {}: no labeled images found", dir);
            continue;
        }
        let rate = (successful as f64 / total as f64) * 100.0;
        println!("  {}: {}/{} = {:.2}%", dir, successful, total, rate);
        total_rate += rate;
        count += 1;
    }

    if count > 0 {
        let average = total_rate / count as f64;
        println!("\n=====================================");
        println!("Average Reading Rate: {:.2}%", average);
        println!("=====================================");
        return;
    }

    let images: Vec<PathBuf> = limited_images.unwrap_or_else(|| dataset_iter(&root, None, false).collect());
    if images.is_empty() {
        println!("No images found under {}", root.display());
        return;
    }
    let (successful, total) = reading_rate_for_images(images.into_iter());
    if total == 0 {
        println!("No labeled images found under {}", root.display());
        return;
    }
    let rate = (successful as f64 / total as f64) * 100.0;
    println!("Reading rate: {}/{} = {:.2}%", successful, total, rate);
}

fn reading_rate_for_images<I>(images: I) -> (usize, usize)
where
    I: Iterator<Item = PathBuf>,
{
    let mut total = 0usize;
    let mut successful = 0usize;

    for path in images {
        let txt_file = path.with_extension("txt");
        if !txt_file.exists() {
            continue;
        }
        total += 1;

        if let Ok((pixels, width, height)) = load_rgb(&path) {
            let start = Instant::now();
            let results = detect_qr(&pixels, width, height);
            let elapsed = start.elapsed();
            let hit = !results.is_empty();
            if hit {
                successful += 1;
            }
            println!(
                "  [{}] {} -> {} ({:.2?})",
                total,
                path.display(),
                if hit { "hit" } else { "miss" },
                elapsed
            );
        } else {
            println!("  [{}] {} -> load_failed", total, path.display());
        }
    }

    (successful, total)
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
