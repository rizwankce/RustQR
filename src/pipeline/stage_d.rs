use crate::config::DetectConfig;

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    state.decode_candidates.clear();
    let _max = config.max_decode_hypotheses;

    // Intentionally no decode output yet.
    // This crate is a scaffold for upcoming staged implementation.
    let _ = state;
}
