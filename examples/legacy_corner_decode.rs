use rust_qr::decoder::qr_decoder::QrDecoder;
use rust_qr::models::Point;
use rust_qr::utils::binarization::adaptive_binarize;
use rust_qr::utils::geometry::PerspectiveTransform;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::fs;
use std::path::Path;

fn order_points(points: &[Point; 4]) -> (Point, Point, Point, Point) {
    let mut tl = points[0];
    let mut tr = points[0];
    let mut br = points[0];
    let mut bl = points[0];

    let mut min_sum = f32::INFINITY;
    let mut max_sum = f32::NEG_INFINITY;
    let mut min_diff = f32::INFINITY;
    let mut max_diff = f32::NEG_INFINITY;

    for &p in points.iter() {
        let sum = p.x + p.y;
        let diff = p.x - p.y;
        if sum < min_sum {
            min_sum = sum;
            tl = p;
        }
        if sum > max_sum {
            max_sum = sum;
            br = p;
        }
        if diff < min_diff {
            min_diff = diff;
            bl = p;
        }
        if diff > max_diff {
            max_diff = diff;
            tr = p;
        }
    }

    (tl, tr, br, bl)
}

fn load_points(txt_path: &str) -> Option<[Point; 4]> {
    let content = fs::read_to_string(txt_path).ok()?;
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
    if vals.len() < 8 {
        return None;
    }
    Some([
        Point::new(vals[0], vals[1]),
        Point::new(vals[2], vals[3]),
        Point::new(vals[4], vals[5]),
        Point::new(vals[6], vals[7]),
    ])
}

fn main() {
    let image_path = "benches/images/monitor/image001.jpg";
    let points_path = "benches/images/monitor/image001.txt";

    if !Path::new(image_path).exists() || !Path::new(points_path).exists() {
        eprintln!("Missing image or points file.");
        return;
    }

    let points = load_points(points_path).expect("Failed to parse points");
    let (tl, tr, br, bl) = order_points(&points);

    let img = image::open(image_path).expect("Failed to open image");
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let raw = rgb.into_raw();
    let gray = rgb_to_grayscale(&raw, width as usize, height as usize);
    let binary = adaptive_binarize(&gray, width as usize, height as usize, 31);

    let offsets = [0.0f32, 0.5, 1.0];
    for version in 1..=40u8 {
        let dimension = 17 + 4 * version as usize;
        for &offset in &offsets {
            let src_min = offset;
            let src_max = dimension as f32 - offset;
            let src = [
                Point::new(src_min, src_min),
                Point::new(src_max, src_min),
                Point::new(src_max, src_max),
                Point::new(src_min, src_max),
            ];
            let dst = [tl, tr, br, bl];
            let transform = match PerspectiveTransform::from_points(&src, &dst) {
                Some(t) => t,
                None => continue,
            };

            let tl_f = transform.transform(&Point::new(3.5, 3.5));
            let tr_f = transform.transform(&Point::new(dimension as f32 - 3.5, 3.5));
            let bl_f = transform.transform(&Point::new(3.5, dimension as f32 - 3.5));
            let module_size = tl_f.distance(&tr_f) / (dimension as f32 - 7.0);

            if let Some(qr) = QrDecoder::decode_with_gray(
                &binary,
                &gray,
                width as usize,
                height as usize,
                &tl_f,
                &tr_f,
                &bl_f,
                module_size,
                true,
            ) {
                println!(
                    "Decoded (version {}, offset {}): {}",
                    version, offset, qr.content
                );
                if qr.content == "4376471154038" {
                    return;
                }
            }
        }
    }

    println!("No decode");
}
