use std::time::Instant;

fn main() {
    println!("Testing pathological images...\n");

    let mut total = 0;
    let mut decoded = 0;
    for i in 1..=23 {
        let path = format!("benches/images/boofcv/pathological/image{:03}.png", i);

        let img = image::open(&path).unwrap();
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();

        let start = Instant::now();
        let results = rust_qr::detect(rgb.as_raw(), width as usize, height as usize);
        let elapsed = start.elapsed();

        total += 1;
        if !results.is_empty() {
            decoded += 1;
        }
        println!(
            "Image {:03}: {:.2}s (found {} QR codes)",
            i,
            elapsed.as_secs_f64(),
            results.len()
        );
    }
    println!(
        "\nTotal: {}/{} decoded ({:.0}%)",
        decoded,
        total,
        decoded as f64 / total as f64 * 100.0
    );
}
