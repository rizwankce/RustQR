use rust_qr::detect_with_report;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("smoke") => {
            let image = vec![0u8; 3 * 4 * 4];
            let report = detect_with_report(&image, 4, 4);
            println!("codes={} failure={:?}", report.codes.len(), report.failure_signature);
        }
        _ => {
            eprintln!("RustQR scaffold CLI");
            eprintln!("usage: qrtool smoke");
            std::process::exit(2);
        }
    }
}
