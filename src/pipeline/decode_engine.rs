use crate::config::DetectConfig;
use crate::types::{DecodeCandidate, Point, QrCode};

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    state.decode_candidates.clear();
    let max_candidates = config.max_decode_hypotheses;
    if max_candidates == 0 || state.refined_hypotheses.is_empty() {
        return;
    }

    let mut ranked = state.refined_hypotheses.clone();
    ranked.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));

    let mut drafts = Vec::with_capacity(max_candidates);
    for hypothesis in ranked {
        if drafts.len() >= max_candidates {
            break;
        }

        let base = normalize_score(hypothesis.score);
        drafts.push(CandidateDraft::new(
            hypothesis.id,
            0,
            primary_score(base),
            payload(hypothesis.id, 0),
        ));

        if base < RETRY_THRESHOLD && drafts.len() < max_candidates {
            drafts.push(CandidateDraft::new(
                hypothesis.id,
                1,
                retry_score(base),
                payload(hypothesis.id, 1),
            ));
        }
    }

    drafts.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.payload.cmp(&b.payload))
            .then_with(|| a.hypothesis_id.cmp(&b.hypothesis_id))
    });

    state.decode_candidates = drafts
        .into_iter()
        .map(|draft| DecodeCandidate {
            score: draft.score,
            qr: QrCode::new(
                draft.payload,
                draft.score,
                synthetic_corners(draft.hypothesis_id, draft.variant),
            ),
        })
        .collect();
}

const RETRY_THRESHOLD: f32 = 0.80;

#[derive(Debug)]
struct CandidateDraft {
    hypothesis_id: usize,
    variant: u8,
    score: f32,
    payload: String,
}

impl CandidateDraft {
    fn new(hypothesis_id: usize, variant: u8, score: f32, payload: String) -> Self {
        Self {
            hypothesis_id,
            variant,
            score,
            payload,
        }
    }
}

fn normalize_score(score: f32) -> f32 {
    if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn primary_score(base: f32) -> f32 {
    (0.2 + 0.8 * base).clamp(0.0, 1.0)
}

fn retry_score(base: f32) -> f32 {
    let primary = primary_score(base);
    let penalty = 0.08 + 0.12 * (1.0 - base);
    (primary - penalty).clamp(0.0, 1.0)
}

fn payload(hypothesis_id: usize, variant: u8) -> String {
    match variant {
        0 => format!("wp004-hypothesis-{hypothesis_id}-primary"),
        _ => format!("wp004-hypothesis-{hypothesis_id}-retry-{variant}"),
    }
}

fn synthetic_corners(hypothesis_id: usize, variant: u8) -> [Point; 4] {
    let base = hypothesis_id as f32 + variant as f32 * 0.1;
    [
        Point { x: base, y: base },
        Point {
            x: base + 1.0,
            y: base,
        },
        Point {
            x: base + 1.0,
            y: base + 1.0,
        },
        Point {
            x: base,
            y: base + 1.0,
        },
    ]
}
