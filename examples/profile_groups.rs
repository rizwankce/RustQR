use rust_qr::detector::finder::FinderDetector;
use rust_qr::utils::binarization::otsu_binarize;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::time::Instant;

fn main() {
    println!("Testing group formation for image009.png...\n");

    let path = "benches/images/boofcv/pathological/image009.png";

    let img = image::open(path).unwrap();
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let gray = rgb_to_grayscale(rgb.as_raw(), width as usize, height as usize);
    let binary = otsu_binarize(&gray, width as usize, height as usize);
    let patterns = FinderDetector::detect(&binary);

    println!("Found {} finder patterns", patterns.len());

    println!("\nStarting full detect (grouping + decode)...");
    let start = Instant::now();
    let results = rust_qr::detect(rgb.as_raw(), width as usize, height as usize);
    println!(
        "Full detect: {:?} ({} QR codes decoded)",
        start.elapsed(),
        results.len()
    );

    for (i, r) in results.iter().enumerate().take(5) {
        println!("  Result {}: {:?}", i, r.content);
    }
}
