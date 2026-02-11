mod config {
    pub use rust_qr::config::*;
}

mod types {
    pub use rust_qr::types::*;
}

#[path = "../src/pipeline/geometry_refinement.rs"]
mod geometry_refinement;
#[allow(dead_code)]
#[path = "../src/pipeline/state.rs"]
mod state;

use config::DetectConfig;
use state::PipelineState;
use types::Hypothesis;

#[test]
fn refinement_is_deterministic_bounded_sorted_and_clamped() {
    let config = DetectConfig {
        max_hypotheses: 3,
        ..DetectConfig::default()
    };
    let input = vec![
        Hypothesis {
            id: 41,
            score: 0.22,
        },
        Hypothesis { id: 7, score: 0.63 },
        Hypothesis {
            id: 13,
            score: -0.40,
        },
        Hypothesis { id: 3, score: 1.25 },
        Hypothesis {
            id: 19,
            score: 0.63,
        },
    ];

    let mut state_a = PipelineState::new(128, 96);
    state_a.hypotheses = input.clone();
    geometry_refinement::run(&mut state_a, &config);

    let mut state_b = PipelineState::new(128, 96);
    state_b.hypotheses = input;
    geometry_refinement::run(&mut state_b, &config);

    assert_eq!(state_a.refined_hypotheses, state_b.refined_hypotheses);
    assert_eq!(state_a.refined_hypotheses.len(), config.max_hypotheses);

    for hypothesis in &state_a.refined_hypotheses {
        assert!(
            (0.0..=1.0).contains(&hypothesis.score),
            "score out of range: {}",
            hypothesis.score
        );
    }

    for pair in state_a.refined_hypotheses.windows(2) {
        let left = pair[0];
        let right = pair[1];
        assert!(
            left.score > right.score || (left.score == right.score && left.id <= right.id),
            "unexpected order left={left:?} right={right:?}"
        );
    }
}

#[test]
fn refinement_outputs_empty_for_empty_input() {
    let config = DetectConfig::default();
    let mut state = PipelineState::new(32, 32);
    geometry_refinement::run(&mut state, &config);
    assert!(state.refined_hypotheses.is_empty());
}
