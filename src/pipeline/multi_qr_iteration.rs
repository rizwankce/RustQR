use crate::config::DetectConfig;
use std::collections::HashSet;

use super::state::PipelineState;

const MIN_ACCEPT_CONFIDENCE: f32 = 0.20;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    state.accepted.clear();

    if config.max_multi_qr == 0 || state.decode_candidates.is_empty() {
        state.accepted_count = 0;
        return;
    }

    let mut ranked = state
        .decode_candidates
        .iter()
        .cloned()
        .enumerate()
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| {
        b.1.score
            .total_cmp(&a.1.score)
            .then_with(|| b.1.qr.confidence.total_cmp(&a.1.qr.confidence))
            .then_with(|| a.1.qr.payload.cmp(&b.1.qr.payload))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut seen_payloads = HashSet::new();
    let mut cursor = 0usize;

    while state.accepted.len() < config.max_multi_qr && cursor < ranked.len() {
        let candidate = &ranked[cursor].1;
        cursor += 1;

        if !candidate.score.is_finite()
            || !candidate.qr.confidence.is_finite()
            || candidate.qr.confidence < MIN_ACCEPT_CONFIDENCE
        {
            continue;
        }

        if seen_payloads.insert(candidate.qr.payload.clone()) {
            state.accepted.push(candidate.qr.clone());
        }
    }

    if state.accepted.len() > config.max_multi_qr {
        state.accepted.truncate(config.max_multi_qr);
    }
    state.accepted_count = state.accepted.len();
}
