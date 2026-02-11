use rust_qr::{detect, detect_with_report};

#[test]
fn empty_or_invalid_input_returns_no_codes() {
    assert!(detect(&[], 0, 0).is_empty());
    assert!(detect(&[0u8; 3], 10, 10).is_empty());
}

#[test]
fn report_contains_pipeline_stage_timings() {
    let image = vec![0u8; 3 * 8 * 8];
    let report = detect_with_report(&image, 8, 8);

    assert_eq!(report.stage_timings.len(), 5);
    assert_eq!(report.counters.proposals, 1);
    assert!(report.codes.is_empty());
    assert_eq!(report.failure_signature.as_deref(), Some("no-decode-yet"));
}
