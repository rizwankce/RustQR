use rust_qr::detect_with_telemetry_timeout;
use std::time::{Duration, Instant};

#[test]
fn zero_budget_stops_before_detection_work() {
    let pixels = vec![255_u8; 64 * 64 * 3];
    let started = Instant::now();
    let (results, telemetry) = detect_with_telemetry_timeout(&pixels, 64, 64, Duration::ZERO);

    assert!(results.is_empty());
    assert_eq!(telemetry.qr_codes_found, 0);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "zero-budget detection should return cooperatively"
    );
}

#[test]
fn malformed_input_is_rejected_before_budgeted_detection() {
    let (results, telemetry) =
        detect_with_telemetry_timeout(&[0; 3], 2, 2, Duration::from_millis(1));
    assert!(results.is_empty());
    assert_eq!(telemetry.qr_codes_found, 0);
}
