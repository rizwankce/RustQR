use crate::config::DetectConfig;
use crate::types::Proposal;

use super::state::PipelineState;

pub(crate) fn run(_image: &[u8], state: &mut PipelineState, config: &DetectConfig) {
    // Scratch scaffold: proposals are intentionally empty until detector implementation lands.
    state.proposals = Vec::with_capacity(config.max_proposals.min(8));

    // Seed a deterministic placeholder proposal to keep later stages testable.
    // This keeps the pipeline wiring alive without claiming real detection accuracy.
    if state.width > 0 && state.height > 0 {
        state.proposals.push(Proposal { id: 0, score: 0.0 });
    }
}
