use crate::config::DetectConfig;

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, _config: &DetectConfig) {
    // Scratch scaffold: refinement currently forwards hypotheses unchanged.
    state.refined_hypotheses = state.hypotheses.clone();
}
