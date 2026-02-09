use std::time::Instant;

fn main() {
    println!("Testing with QR_MAX_IMAGE_DECODE_ATTEMPTS=1 to limit decode attempts...\n");

    unsafe { std::env::set_var("QR_MAX_IMAGE_DECODE_ATTEMPTS", "1") };
    unsafe { std::env::set_var("QR_CANDIDATE_TIME_BUDGET_MS", "10") };
    unsafe { std::env::set_var("QR_BEAM_TIME_BUDGET_MS", "10") };

    for i in 1..=10 {
        let path = format!("benches/images/boofcv/pathological/image{:03}.png", i);

        let img = image::open(&path).unwrap();
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();

        let start = Instant::now();
        let results = rust_qr::detect(rgb.as_raw(), width as usize, height as usize);
        let elapsed = start.elapsed();

        println!(
            "Image {:03}: {:?} (found {} QR codes)",
            i,
            elapsed,
            results.len()
        );
    }
}
