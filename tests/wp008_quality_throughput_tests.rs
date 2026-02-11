use rust_qr::{DetectConfig, pipeline};

#[test]
fn downscaled_working_grid_is_deterministic_and_bounded() {
    let width = 256usize;
    let height = 192usize;
    let image = checkerboard_rgb(width, height, 6);
    let config = DetectConfig {
        max_proposals: 32,
        max_working_dim: 64,
        ..DetectConfig::default()
    };

    let report_a = pipeline::detect_with_config(&image, width, height, &config);
    let report_b = pipeline::detect_with_config(&image, width, height, &config);

    assert_eq!(
        report_a.proposal_ensemble.top_proposals,
        report_b.proposal_ensemble.top_proposals
    );
    assert!(report_a.counters.proposals <= config.max_proposals);
    assert_eq!(
        report_a.counters.proposals,
        report_a.proposal_ensemble.total_kept_candidates
    );

    for proposal in &report_a.proposal_ensemble.top_proposals {
        assert!(proposal.x < width, "x out of range: {}", proposal.x);
        assert!(proposal.y < height, "y out of range: {}", proposal.y);
    }
}

#[test]
fn small_images_match_with_or_without_working_dim_cap() {
    let width = 64usize;
    let height = 64usize;
    let image = stripes_rgb(width, height, 5);

    let with_cap = DetectConfig {
        max_working_dim: 1024,
        max_proposals: 20,
        ..DetectConfig::default()
    };
    let no_cap = DetectConfig {
        max_working_dim: 0,
        max_proposals: 20,
        ..DetectConfig::default()
    };

    let report_a = pipeline::detect_with_config(&image, width, height, &with_cap);
    let report_b = pipeline::detect_with_config(&image, width, height, &no_cap);

    assert_eq!(
        report_a.proposal_ensemble.top_proposals,
        report_b.proposal_ensemble.top_proposals
    );
    assert_eq!(report_a.counters.proposals, report_b.counters.proposals);
}

#[test]
fn proposal_view_keep_counts_stay_consistent_after_optimization() {
    let width = 180usize;
    let height = 140usize;
    let image = checkerboard_rgb(width, height, 7);
    let config = DetectConfig {
        max_working_dim: 72,
        max_proposals: 48,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    let sum_kept: usize = report
        .proposal_ensemble
        .views
        .iter()
        .map(|view| view.kept_candidates)
        .sum();

    assert_eq!(sum_kept, report.proposal_ensemble.total_kept_candidates);
    assert_eq!(sum_kept, report.counters.proposals);
    assert!(report.proposal_ensemble.binary_views_built >= 3);
}

fn checkerboard_rgb(width: usize, height: usize, tile: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let on = ((x / tile) + (y / tile)) % 2 == 0;
            let v = if on { 232u8 } else { 24u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}

fn stripes_rgb(width: usize, height: usize, stripe: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let v = if (x / stripe) % 2 == 0 { 210u8 } else { 44u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}
