use crate::types::ProposalView;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageTiming {
    pub stage: &'static str,
    pub elapsed_ms: f64,
}

impl StageTiming {
    pub fn new(stage: &'static str, elapsed_ms: f64) -> Self {
        Self { stage, elapsed_ms }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageCounters {
    pub proposals: usize,
    pub hypotheses: usize,
    pub refined: usize,
    pub decode_candidates: usize,
    pub accepted: usize,
}

impl StageCounters {
    pub fn zero() -> Self {
        Self {
            proposals: 0,
            hypotheses: 0,
            refined: 0,
            decode_candidates: 0,
            accepted: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProposalEnsembleSummary {
    pub view: ProposalView,
    pub x: usize,
    pub y: usize,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposalViewTelemetry {
    pub view: ProposalView,
    pub raw_candidates: usize,
    pub kept_candidates: usize,
    pub raw_score_min: f32,
    pub raw_score_max: f32,
    pub normalized_score_avg: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposalEnsembleReport {
    pub elapsed_ms: f64,
    pub budget_ms: u64,
    pub within_budget: bool,
    pub binary_views_built: usize,
    pub total_raw_candidates: usize,
    pub total_kept_candidates: usize,
    pub views: Vec<ProposalViewTelemetry>,
    pub top_proposals: Vec<ProposalEnsembleSummary>,
}

impl ProposalEnsembleReport {
    pub fn empty_with_budget(budget_ms: u64) -> Self {
        Self {
            elapsed_ms: 0.0,
            budget_ms,
            within_budget: true,
            binary_views_built: 0,
            total_raw_candidates: 0,
            total_kept_candidates: 0,
            views: Vec::new(),
            top_proposals: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionRunReport {
    pub codes: Vec<crate::types::QrCode>,
    pub stage_timings: Vec<StageTiming>,
    pub counters: StageCounters,
    pub proposal_ensemble: ProposalEnsembleReport,
    pub failure_signature: Option<String>,
    pub total_elapsed_ms: f64,
}
