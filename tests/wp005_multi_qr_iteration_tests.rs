mod config {
    pub use rust_qr::config::*;
}

mod types {
    pub use rust_qr::types::*;
}

mod pipeline_impl {
    #[allow(dead_code)]
    pub mod state {
        include!("../src/pipeline/state.rs");
    }

    pub mod multi_qr_iteration {
        include!("../src/pipeline/multi_qr_iteration.rs");
    }
}

use config::DetectConfig;
use pipeline_impl::multi_qr_iteration::run;
use pipeline_impl::state::PipelineState;
use types::{DecodeCandidate, Point, QrCode};

#[test]
fn dedupes_payloads_and_skips_low_confidence_candidates() {
    let config = DetectConfig {
        max_multi_qr: 8,
        ..DetectConfig::default()
    };
    let mut state = state_with_candidates(vec![
        candidate("duplicate", 0.95, 0.96),
        candidate("unique", 0.88, 0.91),
        candidate("duplicate", 0.90, 0.90),
        candidate("too-low-confidence", 0.99, 0.05),
    ]);

    run(&mut state, &config);

    assert_eq!(payloads(&state.accepted), vec!["duplicate", "unique"]);
    assert!((state.accepted[0].confidence - 0.96).abs() < f32::EPSILON);
    assert_eq!(state.accepted_count, state.accepted.len());
    assert!(state.accepted_count <= config.max_multi_qr);
}

#[test]
fn allows_identical_payloads_for_distinct_positions() {
    let config = DetectConfig {
        max_multi_qr: 8,
        ..DetectConfig::default()
    };
    let mut state = state_with_candidates(vec![
        candidate_with_corners("same", 0.96, 0.95, corners()),
        candidate_with_corners("same", 0.94, 0.92, shifted_corners(20.0, 5.0)),
        candidate_with_corners("same", 0.92, 0.90, shifted_corners(0.4, 0.3)),
    ]);

    run(&mut state, &config);

    assert_eq!(state.accepted.len(), 2);
    assert_eq!(state.accepted_count, 2);
    assert_eq!(state.accepted[0].payload, "same");
    assert_eq!(state.accepted[1].payload, "same");
    assert_eq!(state.accepted[0].corners, corners());
    assert_eq!(state.accepted[1].corners, shifted_corners(20.0, 5.0));
}

#[test]
fn respects_max_multi_qr_bound() {
    let config = DetectConfig {
        max_multi_qr: 2,
        ..DetectConfig::default()
    };
    let mut state = state_with_candidates(vec![
        candidate("gamma", 0.70, 0.82),
        candidate("alpha", 0.99, 0.95),
        candidate("delta", 0.75, 0.84),
        candidate("beta", 0.92, 0.90),
    ]);

    run(&mut state, &config);

    assert_eq!(payloads(&state.accepted), vec!["alpha", "beta"]);
    assert_eq!(state.accepted.len(), config.max_multi_qr);
    assert_eq!(state.accepted_count, state.accepted.len());
    assert!(state.accepted_count <= config.max_multi_qr);
}

#[test]
fn acceptance_order_is_deterministic_and_strongest_first() {
    let config = DetectConfig {
        max_multi_qr: 8,
        ..DetectConfig::default()
    };
    let candidates = vec![
        candidate("zeta", 0.80, 0.86),
        candidate("omega", 0.95, 0.88),
        candidate("alpha", 0.80, 0.86),
        candidate("beta", 0.60, 0.89),
    ];

    let mut state_a = state_with_candidates(candidates.clone());
    run(&mut state_a, &config);

    let mut state_b = state_with_candidates(candidates);
    run(&mut state_b, &config);

    assert_eq!(state_a.accepted, state_b.accepted);
    assert_eq!(
        payloads(&state_a.accepted),
        vec!["omega", "alpha", "zeta", "beta"]
    );
    assert_eq!(state_a.accepted_count, state_a.accepted.len());
    assert!(state_a.accepted_count <= config.max_multi_qr);
}

fn state_with_candidates(candidates: Vec<DecodeCandidate>) -> PipelineState {
    let mut state = PipelineState::new(128, 128);
    state.decode_candidates = candidates;
    state
}

fn candidate(payload: &str, score: f32, confidence: f32) -> DecodeCandidate {
    candidate_with_corners(payload, score, confidence, corners())
}

fn candidate_with_corners(
    payload: &str,
    score: f32,
    confidence: f32,
    corners: [Point; 4],
) -> DecodeCandidate {
    DecodeCandidate {
        score,
        qr: QrCode::new(payload.to_string(), confidence, corners),
    }
}

fn corners() -> [Point; 4] {
    [
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
        Point { x: 1.0, y: 1.0 },
        Point { x: 0.0, y: 1.0 },
    ]
}

fn shifted_corners(dx: f32, dy: f32) -> [Point; 4] {
    [
        Point {
            x: 0.0 + dx,
            y: 0.0 + dy,
        },
        Point {
            x: 1.0 + dx,
            y: 0.0 + dy,
        },
        Point {
            x: 1.0 + dx,
            y: 1.0 + dy,
        },
        Point {
            x: 0.0 + dx,
            y: 1.0 + dy,
        },
    ]
}

fn payloads(codes: &[QrCode]) -> Vec<&str> {
    codes.iter().map(|code| code.payload.as_str()).collect()
}
