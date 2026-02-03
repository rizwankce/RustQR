// Quick test on specific images
use std::path::Path;

fn main() {
    let test_images = [
        "benches/images/boofcv/monitor/image001.jpg",
        "benches/images/boofcv/monitor/image002.jpg",
        "benches/images/boofcv/nominal/image001.jpg",
        "benches/images/boofcv/nominal/image002.jpg",
    ];

    let mut success = 0;
    let mut total = 0;

    for img_path in &test_images {
        let path = Path::new(img_path);
        if !path.exists() {
            continue;
        }

        total += 1;

        let img = match image::open(path) {
            Ok(img) => img,
            Err(_) => continue,
        };

        let rgb_img = img.to_rgb8();
        let (width, height) = (rgb_img.width() as usize, rgb_img.height() as usize);
        let raw_pixels: Vec<u8> = rgb_img.into_raw();

        let results = rust_qr::detect(&raw_pixels, width, height);

        if !results.is_empty() {
            success += 1;
            println!("OK: {} -> {} QR codes found", img_path, results.len());
            for (i, qr) in results.iter().enumerate() {
                println!("  [{}] version={:?}, content={}", i, qr.version, qr.content);
            }
        } else {
            println!("FAIL: {} -> no QR codes", img_path);
        }
    }

    println!("\nResult: {}/{} ({:.1}%)", success, total, 100.0 * success as f64 / total as f64);
}
