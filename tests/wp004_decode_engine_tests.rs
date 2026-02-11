mod config {
    pub use rust_qr::config::*;
}

mod types {
    pub use rust_qr::types::*;
}

#[path = "../src/pipeline/decode_engine.rs"]
mod decode_engine;
#[allow(dead_code)]
#[path = "../src/pipeline/state.rs"]
mod state;

use std::cmp::Ordering;

use config::DetectConfig;
use state::PipelineState;
use types::Hypothesis;

#[test]
fn decode_candidate_count_is_globally_bounded() {
    let mut state = PipelineState::new(128, 128);
    state.refined_hypotheses = vec![
        Hypothesis { id: 7, score: 0.91 },
        Hypothesis { id: 4, score: 0.42 },
        Hypothesis { id: 2, score: 0.33 },
        Hypothesis { id: 9, score: 0.12 },
    ];

    let config = DetectConfig {
        max_decode_hypotheses: 5,
        ..DetectConfig::default()
    };

    decode_engine::run(&mut state, &config);

    assert_eq!(state.decode_candidates.len(), config.max_decode_hypotheses);
    assert!(
        state
            .decode_candidates
            .iter()
            .any(|candidate| candidate.qr.payload.contains("hypothesis-7"))
    );
    assert!(
        state
            .decode_candidates
            .iter()
            .any(|candidate| candidate.qr.payload.contains("retry"))
    );
}

#[test]
fn decode_candidates_are_deterministic_across_runs() {
    let config = DetectConfig {
        max_decode_hypotheses: 6,
        ..DetectConfig::default()
    };

    let mut state_a = build_state();
    decode_engine::run(&mut state_a, &config);
    let first_run = state_a.decode_candidates.clone();

    decode_engine::run(&mut state_a, &config);
    assert_eq!(state_a.decode_candidates, first_run);

    let mut state_b = build_state();
    decode_engine::run(&mut state_b, &config);
    assert_eq!(state_a.decode_candidates, state_b.decode_candidates);
}

#[test]
fn decode_candidates_have_bounded_scores_and_stable_ordering() {
    let mut state = PipelineState::new(64, 64);
    state.refined_hypotheses = vec![
        Hypothesis {
            id: 10,
            score: 0.55,
        },
        Hypothesis { id: 3, score: 1.40 },
        Hypothesis {
            id: 1,
            score: -0.20,
        },
        Hypothesis { id: 8, score: 0.55 },
        Hypothesis {
            id: 6,
            score: f32::NAN,
        },
    ];

    let config = DetectConfig {
        max_decode_hypotheses: 10,
        ..DetectConfig::default()
    };

    decode_engine::run(&mut state, &config);

    assert!(!state.decode_candidates.is_empty());

    for candidate in &state.decode_candidates {
        assert!(candidate.score.is_finite());
        assert!((0.0..=1.0).contains(&candidate.score));
        assert_eq!(candidate.score, candidate.qr.confidence);
    }

    for pair in state.decode_candidates.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let score_order = left.score.total_cmp(&right.score);
        assert_ne!(score_order, Ordering::Less);
        if score_order == Ordering::Equal {
            assert!(left.qr.payload <= right.qr.payload);
        }
    }
}

fn build_state() -> PipelineState {
    let mut state = PipelineState::new(96, 96);
    state.refined_hypotheses = vec![
        Hypothesis {
            id: 42,
            score: 0.77,
        },
        Hypothesis { id: 2, score: 0.15 },
        Hypothesis {
            id: 11,
            score: 0.93,
        },
        Hypothesis { id: 5, score: 0.77 },
    ];
    state
}
