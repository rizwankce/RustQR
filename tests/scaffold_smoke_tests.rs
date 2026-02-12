use rust_qr::{DetectConfig, detect, detect_with_report, pipeline};

#[test]
fn empty_or_invalid_input_returns_no_codes() {
    assert!(detect(&[], 0, 0).is_empty());
    assert!(detect(&[0u8; 3], 10, 10).is_empty());
}

#[test]
fn oversized_dimensions_return_invalid_input_without_panicking() {
    let image = [7u8, 8u8, 9u8];
    let report = detect_with_report(&image, usize::MAX, 2);

    assert!(report.codes.is_empty());
    assert_eq!(report.failure_signature.as_deref(), Some("invalid-input"));
}

#[test]
fn report_contains_pipeline_stage_timings() {
    let image = vec![0u8; 3 * 8 * 8];
    let report = detect_with_report(&image, 8, 8);

    assert_eq!(report.stage_timings.len(), 5);
    assert_eq!(report.proposal_ensemble.binary_views_built, 4);
    assert_eq!(
        report.proposal_ensemble.total_kept_candidates,
        report.counters.proposals
    );
    assert!(report.proposal_ensemble.elapsed_ms >= 0.0);
    assert_eq!(
        report.proposal_ensemble.budget_ms,
        DetectConfig::default().proposal_ensemble_budget_ms
    );
    assert!(report.codes.is_empty());
    assert_eq!(report.failure_signature.as_deref(), Some("no-decode-yet"));
}

#[test]
fn proposal_ensemble_proposals_are_deterministic_and_bounded() {
    let width = 64usize;
    let height = 64usize;
    let image = checkerboard_rgb(width, height, 4);
    let config = DetectConfig {
        max_proposals: 12,
        ..DetectConfig::default()
    };

    let report_a = pipeline::detect_with_config(&image, width, height, &config);
    let report_b = pipeline::detect_with_config(&image, width, height, &config);

    assert!(report_a.counters.proposals <= 12);
    assert_eq!(
        report_a.proposal_ensemble.top_proposals,
        report_b.proposal_ensemble.top_proposals
    );
    assert!(!report_a.proposal_ensemble.top_proposals.is_empty());
    assert_eq!(
        report_a.proposal_ensemble.total_kept_candidates,
        report_a
            .proposal_ensemble
            .views
            .iter()
            .map(|v| v.kept_candidates)
            .sum()
    );
}

fn checkerboard_rgb(width: usize, height: usize, tile: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let on = ((x / tile) + (y / tile)) % 2 == 0;
            let v = if on { 230u8 } else { 20u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}
