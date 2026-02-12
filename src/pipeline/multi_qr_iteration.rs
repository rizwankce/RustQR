use crate::config::DetectConfig;
use crate::types::QrCode;
use std::collections::HashSet;

use super::state::PipelineState;

const MIN_ACCEPT_CONFIDENCE: f32 = 0.20;
const CENTER_BIN_SIZE_PX: f32 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AcceptedKey {
    payload: String,
    center_x_bin: i32,
    center_y_bin: i32,
}

fn quantize_coordinate(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (value / CENTER_BIN_SIZE_PX).round() as i32
}

fn acceptance_key(qr: &QrCode) -> AcceptedKey {
    let mut center_x = 0.0f32;
    let mut center_y = 0.0f32;
    for corner in qr.corners {
        center_x += corner.x;
        center_y += corner.y;
    }
    center_x /= 4.0;
    center_y /= 4.0;

    AcceptedKey {
        payload: qr.payload.clone(),
        center_x_bin: quantize_coordinate(center_x),
        center_y_bin: quantize_coordinate(center_y),
    }
}

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

    let mut seen_keys = HashSet::new();
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

        if seen_keys.insert(acceptance_key(&candidate.qr)) {
            state.accepted.push(candidate.qr.clone());
        }
    }

    if state.accepted.len() > config.max_multi_qr {
        state.accepted.truncate(config.max_multi_qr);
    }
    state.accepted_count = state.accepted.len();
}
