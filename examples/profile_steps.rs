use rust_qr::detector::finder::FinderDetector;
use rust_qr::utils::binarization::otsu_binarize;
use rust_qr::utils::grayscale::rgb_to_grayscale;
use std::time::Instant;

fn main() {
    println!("Step-by-step profiling for image009.png...\n");

    let path = "benches/images/boofcv/pathological/image009.png";

    let start = Instant::now();
    let img = image::open(path).unwrap();
    println!("Load: {:?}", start.elapsed());

    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    println!("Dimensions: {}x{}", width, height);

    let start = Instant::now();
    let gray = rgb_to_grayscale(rgb.as_raw(), width as usize, height as usize);
    println!("Grayscale: {:?}", start.elapsed());

    let start = Instant::now();
    let binary = otsu_binarize(&gray, width as usize, height as usize);
    println!("Binarize: {:?}", start.elapsed());

    let start = Instant::now();
    let patterns = FinderDetector::detect(&binary);
    println!(
        "Finder patterns: {:?} (found {} patterns)",
        start.elapsed(),
        patterns.len()
    );

    for (i, p) in patterns.iter().enumerate() {
        println!(
            "  Pattern {}: center=({:.1}, {:.1}), module={:.2}",
            i, p.center.x, p.center.y, p.module_size
        );
    }

    println!("\nThe hang is likely in decode_groups or later pipeline stages...");
}
