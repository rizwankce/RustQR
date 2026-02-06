use crate::models::BitMatrix;
use crate::utils::binarization::{adaptive_binarize, otsu_binarize};
use crate::utils::grayscale::rgb_to_grayscale;
use crate::{QRCode, detect};
use image::GenericImageView;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn max_dim_from_env() -> Option<u32> {
    match env::var("QR_MAX_DIM") {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(0) => None,
            Ok(v) => Some(v),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

/// Load an image as RGB bytes along with its dimensions.
pub fn load_rgb<P: AsRef<Path>>(path: P) -> Result<(Vec<u8>, usize, usize), image::ImageError> {
    let img = image::open(path)?;
    let rgb = if let Some(max_dim) = max_dim_from_env() {
        let (orig_w, orig_h) = img.dimensions();
        let max_side = orig_w.max(orig_h);
        if max_side > max_dim {
            let resized = img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle);
            resized.to_rgb8()
        } else {
            img.to_rgb8()
        }
    } else {
        img.to_rgb8()
    };
    let (width, height) = rgb.dimensions();
    Ok((rgb.into_raw(), width as usize, height as usize))
}

/// Convert RGB bytes into grayscale.
pub fn to_grayscale(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    rgb_to_grayscale(rgb, width, height)
}

/// Binarize a grayscale image using the same policy as detection.
pub fn binarize(gray: &[u8], width: usize, height: usize) -> BitMatrix {
    if width >= 800 || height >= 800 {
        adaptive_binarize(gray, width, height, 31)
    } else {
        otsu_binarize(gray, width, height)
    }
}

/// Binarize a grayscale image using Otsu's method.
pub fn binarize_otsu(gray: &[u8], width: usize, height: usize) -> BitMatrix {
    otsu_binarize(gray, width, height)
}

/// Detect QR codes in an RGB image.
pub fn detect_qr(rgb: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    detect(rgb, width, height)
}

/// Summary statistics for grayscale data.
#[derive(Debug, Clone, Copy)]
pub struct GrayStats {
    /// Minimum grayscale value.
    pub min: u8,
    /// Maximum grayscale value.
    pub max: u8,
    /// Average grayscale value.
    pub avg: u8,
}

/// Summary statistics for a binary matrix.
#[derive(Debug, Clone, Copy)]
pub struct BinaryStats {
    /// Count of black pixels.
    pub black_pixels: usize,
    /// Total pixels in the matrix.
    pub total_pixels: usize,
    /// Ratio of black pixels to total pixels.
    pub black_ratio: f64,
}

/// Compute min/max/avg for grayscale values.
pub fn grayscale_stats(gray: &[u8]) -> GrayStats {
    let mut min = u8::MAX;
    let mut max = u8::MIN;
    let mut sum: u64 = 0;
    for &v in gray {
        min = min.min(v);
        max = max.max(v);
        sum += v as u64;
    }
    let avg = if gray.is_empty() {
        0
    } else {
        (sum / gray.len() as u64) as u8
    };
    GrayStats { min, max, avg }
}

/// Compute black pixel stats for a binary matrix.
pub fn binary_stats(binary: &BitMatrix) -> BinaryStats {
    let mut black = 0usize;
    for y in 0..binary.height() {
        for x in 0..binary.width() {
            if binary.get(x, y) {
                black += 1;
            }
        }
    }
    let total = binary.width() * binary.height();
    let ratio = if total == 0 {
        0.0
    } else {
        black as f64 / total as f64
    };
    BinaryStats {
        black_pixels: black,
        total_pixels: total,
        black_ratio: ratio,
    }
}

/// Default dataset root from environment variables.
pub fn dataset_root_from_env() -> PathBuf {
    env::var("QR_DATASET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/images/boofcv"))
}

/// Default bench limit from environment variables.
///
/// Returns `None` (full dataset) when `QR_BENCH_LIMIT` is unset or set to `0`.
/// Previously defaulted to 5 when unset, which silently sampled only a tiny
/// subset and produced misleading reading-rate numbers.
pub fn bench_limit_from_env() -> Option<usize> {
    match env::var("QR_BENCH_LIMIT") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .and_then(|v| if v == 0 { None } else { Some(v) }),
        Err(_) => None,
    }
}

/// Count the number of expected QR codes from a BoofCV-format label file.
///
/// The `.txt` files contain hand-selected corner coordinates: one line with
/// `# list of hand selected 2D points`, a `SETS` marker, then one line per
/// expected QR code with 8 floating-point values (4 corner points × x,y).
///
/// Returns `0` if the file cannot be read or parsed.
pub fn parse_expected_qr_count<P: AsRef<Path>>(txt_path: P) -> usize {
    let content = match fs::read_to_string(txt_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut count = 0usize;
    let mut past_header = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "SETS" {
            past_header = true;
            continue;
        }
        if past_header {
            // Each data line should have 8 floats (4 corner points)
            let nums: Vec<f64> = trimmed
                .split_whitespace()
                .filter_map(|t| t.parse::<f64>().ok())
                .collect();
            if nums.len() >= 8 {
                count += 1;
            }
        }
    }
    count
}

/// Smoke test flag from environment variables.
pub fn smoke_from_env() -> bool {
    matches!(
        env::var("QR_SMOKE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// Iterate dataset image paths with optional smoke list and limit.
pub fn dataset_iter<P: AsRef<Path>>(
    root: P,
    limit: Option<usize>,
    smoke: bool,
) -> impl Iterator<Item = PathBuf> {
    let root = root.as_ref();
    let mut images = if smoke {
        load_smoke_list(root).unwrap_or_else(|| collect_images(root))
    } else {
        collect_images(root)
    };

    images.sort();
    if let Some(limit) = limit {
        images.truncate(limit);
    }
    images.into_iter()
}

fn load_smoke_list(root: &Path) -> Option<Vec<PathBuf>> {
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
    if paths.is_empty() { None } else { Some(paths) }
}

fn collect_images(root: &Path) -> Vec<PathBuf> {
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
                if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "gif" || ext == "bmp" {
                    images.push(path);
                }
            }
        }
    }

    images
}
