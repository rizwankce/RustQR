use crate::config::DetectConfig;
use crate::types::Hypothesis;

use super::state::PipelineState;

const REFINE_PASSES: usize = 3;
const PASS_GAINS: [f32; REFINE_PASSES] = [0.16, 0.11, 0.07];

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    if config.max_hypotheses == 0 || state.hypotheses.is_empty() {
        state.refined_hypotheses.clear();
        return;
    }

    let mut refined = state
        .hypotheses
        .iter()
        .map(|hypothesis| Hypothesis {
            id: hypothesis.id,
            score: refine_score(hypothesis.score),
        })
        .collect::<Vec<_>>();

    refined.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    refined.truncate(config.max_hypotheses);
    state.refined_hypotheses = refined;
}

fn refine_score(initial_score: f32) -> f32 {
    let mut score = clamp_unit(initial_score);
    for pass in 0..REFINE_PASSES {
        let residual = 1.0 - score;
        if residual <= 0.0 {
            break;
        }
        let pass_decay = 1.0 - (pass as f32 * 0.10);
        let confidence_scale = 0.85 + 0.15 * score;
        let gain = PASS_GAINS[pass] * pass_decay * confidence_scale;
        score = clamp_unit(score + residual * gain);
    }
    score
}

fn clamp_unit(score: f32) -> f32 {
    if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
