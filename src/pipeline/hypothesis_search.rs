use crate::config::DetectConfig;
use crate::types::Hypothesis;

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    state.hypotheses.clear();
    let max_count = config.max_hypotheses.min(state.proposals.len());
    for proposal in state.proposals.iter().take(max_count) {
        state.hypotheses.push(Hypothesis {
            id: proposal.id,
            score: proposal.score,
        });
    }
}
