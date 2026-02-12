use rust_qr::{DetectConfig, pipeline};

#[test]
fn hypothesis_count_is_bounded_by_config() {
    let width = 96usize;
    let height = 96usize;
    let image = checkerboard_rgb(width, height, 4);
    let config = DetectConfig {
        max_hypotheses: 5,
        ..DetectConfig::default()
    };

    let report = pipeline::detect_with_config(&image, width, height, &config);
    assert!(report.counters.hypotheses <= config.max_hypotheses);
    assert_eq!(report.counters.hypotheses, report.counters.refined);
}

#[test]
fn hypothesis_stage_is_deterministic_for_fixed_input() {
    let width = 96usize;
    let height = 96usize;
    let image = striped_rgb(width, height, 6);
    let config = DetectConfig {
        max_hypotheses: 9,
        ..DetectConfig::default()
    };

    let report_a = pipeline::detect_with_config(&image, width, height, &config);
    let report_b = pipeline::detect_with_config(&image, width, height, &config);

    assert_eq!(report_a.counters.hypotheses, report_b.counters.hypotheses);
    assert_eq!(report_a.counters.refined, report_b.counters.refined);
    assert_eq!(report_a.failure_signature, report_b.failure_signature);
}

fn checkerboard_rgb(width: usize, height: usize, tile: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let on = ((x / tile) + (y / tile)) % 2 == 0;
            let v = if on { 220u8 } else { 30u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}

fn striped_rgb(width: usize, height: usize, stripe: usize) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let v = if (x / stripe) % 2 == 0 { 210u8 } else { 45u8 };
            let idx = (y * width + x) * 3;
            out[idx] = v;
            out[idx + 1] = v;
            out[idx + 2] = v;
        }
    }
    out
}
