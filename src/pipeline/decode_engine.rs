use std::cmp::Ordering;

use crate::config::DetectConfig;
use crate::types::{DecodeCandidate, Point, QrCode};

use super::state::PipelineState;

pub(crate) fn run(image: &[u8], state: &mut PipelineState, config: &DetectConfig) {
    state.decode_candidates.clear();

    let max_candidates = config.max_decode_hypotheses;
    if max_candidates == 0 {
        return;
    }

    let expected_len = state.width.saturating_mul(state.height).saturating_mul(3);
    if image.len() != expected_len {
        return;
    }

    let mut ranked_hypotheses = state.refined_hypotheses.clone();
    ranked_hypotheses.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));

    let grayscale = rgb_to_grayscale(image);
    let mut decoder = quircs::Quirc::default();
    let mut decoded = decoder
        .identify(state.width, state.height, &grayscale)
        .filter_map(Result::ok)
        .filter_map(|code| decode_code(code))
        .collect::<Vec<_>>();

    decoded.sort_by(|a, b| {
        a.payload
            .cmp(&b.payload)
            .then_with(|| corners_cmp(&a.corners, &b.corners))
    });

    let hypothesis_tail = ranked_hypotheses.last().copied();
    let mut candidates = decoded
        .into_iter()
        .take(max_candidates)
        .enumerate()
        .map(|(idx, decoded_qr)| {
            let hypothesis_score = ranked_hypotheses
                .get(idx)
                .copied()
                .or(hypothesis_tail)
                .map(|hypothesis| normalize_score(hypothesis.score))
                .unwrap_or(0.0);
            let score = calibrated_score(hypothesis_score);
            DecodeCandidate {
                score,
                qr: QrCode::new(decoded_qr.payload, score, decoded_qr.corners),
            }
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.qr.payload.cmp(&b.qr.payload))
            .then_with(|| corners_cmp(&a.qr.corners, &b.qr.corners))
    });
    candidates.truncate(max_candidates);
    state.decode_candidates = candidates;
}

#[derive(Debug)]
struct DecodedQr {
    payload: String,
    corners: [Point; 4],
}

fn decode_code(code: quircs::Code) -> Option<DecodedQr> {
    let decoded = code.decode().ok()?;
    let payload = String::from_utf8_lossy(&decoded.payload)
        .trim_end_matches('\0')
        .to_string();
    if payload.is_empty() {
        return None;
    }

    Some(DecodedQr {
        payload,
        corners: map_corners(code.corners),
    })
}

fn map_corners(input: [quircs::Point; 4]) -> [Point; 4] {
    input.map(|corner| Point {
        x: corner.x as f32,
        y: corner.y as f32,
    })
}

fn corners_cmp(left: &[Point; 4], right: &[Point; 4]) -> Ordering {
    for idx in 0..4 {
        let x_order = left[idx].x.total_cmp(&right[idx].x);
        if x_order != Ordering::Equal {
            return x_order;
        }

        let y_order = left[idx].y.total_cmp(&right[idx].y);
        if y_order != Ordering::Equal {
            return y_order;
        }
    }

    Ordering::Equal
}

fn rgb_to_grayscale(image: &[u8]) -> Vec<u8> {
    image
        .chunks_exact(3)
        .map(|chunk| {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            ((299 * r + 587 * g + 114 * b + 500) / 1000) as u8
        })
        .collect()
}

fn normalize_score(score: f32) -> f32 {
    if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn calibrated_score(base: f32) -> f32 {
    (0.2 + 0.8 * base).clamp(0.0, 1.0)
}
