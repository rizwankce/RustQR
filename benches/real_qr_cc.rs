use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rust_qr::detector::finder::FinderDetector;
use rust_qr::utils::binarization::otsu_binarize;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn dataset_root() -> PathBuf {
    env::var("QR_DATASET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/images/boofcv"))
}

fn bench_limit() -> Option<usize> {
    match env::var("QR_BENCH_LIMIT") {
        Ok(value) => value.parse::<usize>().ok().and_then(|v| if v == 0 { None } else { Some(v) }),
        Err(_) => Some(5),
    }
}

fn smoke_enabled() -> bool {
    matches!(
        env::var("QR_SMOKE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn load_smoke_list(root: &Path) -> Option<Vec<PathBuf>> {
    if !smoke_enabled() {
        return None;
    }
    let smoke_path = root.join("_smoke.txt");
    let contents = fs::read_to_string(&smoke_path).ok()?;
    let mut paths = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let candidate = Path::new(line);
        let path = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        if path.exists() {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

fn collect_images(root: &Path) -> Vec<PathBuf> {
    if let Some(paths) = load_smoke_list(root) {
        return paths;
    }

    let mut stack = vec![root.to_path_buf()];
    let mut images = Vec::new();

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if ext == "png" || ext == "jpg" || ext == "jpeg" {
                    images.push(path);
                }
            }
        }
    }

    images.sort();
    images
}

/// Benchmark connected components detection on real QR code images
fn bench_real_qr_connected_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_qr_connected_components");

    let image_root = dataset_root();
    if !image_root.exists() {
        println!("Warning: No test images found at {:?}", image_root);
        return;
    }

    let mut images = collect_images(&image_root);
    if images.is_empty() {
        println!("Warning: No test images found under {:?}", image_root);
        return;
    }

    if let Some(limit) = bench_limit() {
        images.truncate(limit);
    }

    for path in images {
        let img_name = path
            .strip_prefix(&image_root)
            .unwrap_or(&path)
            .to_string_lossy();

        // Load image
        let img = image::open(&path).unwrap();
        let rgb_img = img.to_rgb8();
        let (width, height) = rgb_img.dimensions();
        let raw_pixels: Vec<u8> = rgb_img.into_raw();

        // Preprocess: convert to grayscale and binarize
        let gray = rgb_to_grayscale(&raw_pixels, width as usize, height as usize);
        let binary = otsu_binarize(&gray, width as usize, height as usize);

        // Benchmark regular detection
        group.bench_with_input(
            BenchmarkId::new("regular_detect", &img_name),
            &binary,
            |b, binary| {
                b.iter(|| FinderDetector::detect(black_box(binary)));
            },
        );

        // Benchmark connected components detection
        group.bench_with_input(
            BenchmarkId::new("connected_components", &img_name),
            &binary,
            |b, binary| {
                b.iter(|| FinderDetector::detect_with_connected_components(black_box(binary)));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_real_qr_connected_components);
criterion_main!(benches);
