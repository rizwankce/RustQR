// Direct debug test that traces through the detect function
use std::path::Path;

fn main() {
    let test_image = Path::new("benches/images/monitor/image001.jpg");

    if !test_image.exists() {
        println!("Test image not found");
        return;
    }

    println!("Testing with: {}", test_image.display());

    let img = image::open(test_image).expect("Failed to open image");
    let rgb_img = img.to_rgb8();
    let (width, height) = rgb_img.dimensions();
    let raw_pixels: Vec<u8> = rgb_img.into_raw();

    println!("Image: {}x{}", width, height);

    // Call the library detect function
    let results = rust_qr::detect(&raw_pixels, width as usize, height as usize);
    println!("\nResults: {} QR codes found", results.len());

    for (i, qr) in results.iter().enumerate() {
        println!("QR {}: {:?}", i, qr);
    }
}
