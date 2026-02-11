use crate::config::DetectConfig;

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    state.accepted.clear();
    state.accepted_count = state.accepted.len().min(config.max_multi_qr);
}
