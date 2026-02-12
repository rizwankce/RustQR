use rust_qr::{DetectConfig, pipeline};

#[test]
fn proposal_budget_zero_skips_pipeline_fail_safe() {
    let width = 16usize;
    let height = 16usize;
    let image = checkerboard_rgb(width, height, 2);
    let config = DetectConfig {
        proposal_ensemble_budget_ms: 0,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    let stages = report
        .stage_timings
        .iter()
        .map(|timing| timing.stage)
        .collect::<Vec<_>>();

    assert_eq!(
        stages,
        vec![
            "proposal_ensemble:skipped-budget",
            "hypothesis_search:skipped-budget",
            "geometry_refinement:skipped-budget",
            "decode_engine:skipped-budget",
            "multi_qr_iteration:skipped-budget",
        ]
    );
    assert_eq!(report.counters.proposals, 0);
    assert_eq!(report.counters.hypotheses, 0);
    assert_eq!(report.counters.refined, 0);
    assert_eq!(report.counters.decode_candidates, 0);
    assert_eq!(report.counters.accepted, 0);
    assert!(!report.proposal_ensemble.within_budget);
    assert_eq!(report.failure_signature.as_deref(), Some("over-budget"));
}

#[test]
fn emergency_cutoff_zero_skips_all_stages() {
    let width = 16usize;
    let height = 16usize;
    let image = checkerboard_rgb(width, height, 2);
    let config = DetectConfig {
        emergency_cutoff_ms: 0,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    let stages = report
        .stage_timings
        .iter()
        .map(|timing| timing.stage)
        .collect::<Vec<_>>();

    assert_eq!(
        stages,
        vec![
            "proposal_ensemble:skipped-cutoff",
            "hypothesis_search:skipped-cutoff",
            "geometry_refinement:skipped-cutoff",
            "decode_engine:skipped-cutoff",
            "multi_qr_iteration:skipped-cutoff",
        ]
    );
    assert_eq!(report.counters.proposals, 0);
    assert_eq!(report.counters.hypotheses, 0);
    assert_eq!(report.counters.refined, 0);
    assert_eq!(report.counters.decode_candidates, 0);
    assert_eq!(report.counters.accepted, 0);
    assert!(!report.proposal_ensemble.within_budget);
    assert_eq!(
        report.failure_signature.as_deref(),
        Some("emergency-cutoff")
    );
}

#[test]
fn decode_budget_zero_skips_decode_and_later_stage() {
    let width = 16usize;
    let height = 16usize;
    let image = checkerboard_rgb(width, height, 2);
    let config = DetectConfig {
        decode_budget_ms: 0,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    let stages = report
        .stage_timings
        .iter()
        .map(|timing| timing.stage)
        .collect::<Vec<_>>();

    assert_eq!(stages[0], "proposal_ensemble:ok");
    assert_eq!(stages[1], "hypothesis_search:ok");
    assert_eq!(stages[2], "geometry_refinement:ok");
    assert_eq!(stages[3], "decode_engine:skipped-budget");
    assert_eq!(stages[4], "multi_qr_iteration:skipped-budget");
    assert_eq!(report.failure_signature.as_deref(), Some("over-budget"));
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
