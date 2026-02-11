use crate::types::{DecodeCandidate, Hypothesis, Proposal, QrCode};

pub(crate) struct PipelineState {
    pub width: usize,
    pub height: usize,
    pub proposals: Vec<Proposal>,
    pub hypotheses: Vec<Hypothesis>,
    pub refined_hypotheses: Vec<Hypothesis>,
    pub decode_candidates: Vec<DecodeCandidate>,
    pub accepted: Vec<QrCode>,
    pub accepted_count: usize,
}

impl PipelineState {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            proposals: Vec::new(),
            hypotheses: Vec::new(),
            refined_hypotheses: Vec::new(),
            decode_candidates: Vec::new(),
            accepted: Vec::new(),
            accepted_count: 0,
        }
    }
}
