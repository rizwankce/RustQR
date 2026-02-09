use rust_qr::detector::finder::FinderDetector;
use rust_qr::utils::binarization::otsu_binarize;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::time::Instant;

fn main() {
    println!("Detailed profiling for image009.png...\n");

    let path = "benches/images/boofcv/pathological/image009.png";

    let img = image::open(path).unwrap();
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();

    let t1 = Instant::now();
    let gray = rgb_to_grayscale(rgb.as_raw(), width as usize, height as usize);
    println!("1. Grayscale: {:?}", t1.elapsed());

    let t2 = Instant::now();
    let binary = otsu_binarize(&gray, width as usize, height as usize);
    println!("2. Binarize: {:?}", t2.elapsed());

    let t3 = Instant::now();
    let patterns = FinderDetector::detect(&binary);
    println!(
        "3. Finder detection: {:?} ({} patterns)",
        t3.elapsed(),
        patterns.len()
    );

    if patterns.len() >= 3 {
        println!("\n4. Starting full detect (grouping + decode)...");
        let t4 = Instant::now();
        let results = rust_qr::detect(rgb.as_raw(), width as usize, height as usize);
        println!(
            "4. Full detect: {:?} ({} results)",
            t4.elapsed(),
            results.len()
        );
    } else {
        println!("Too few patterns ({}), skipping decode", patterns.len());
    }
}
