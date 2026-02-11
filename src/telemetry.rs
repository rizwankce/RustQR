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

#[derive(Debug, Clone, PartialEq)]
pub struct DetectionRunReport {
    pub codes: Vec<crate::types::QrCode>,
    pub stage_timings: Vec<StageTiming>,
    pub counters: StageCounters,
    pub failure_signature: Option<String>,
    pub total_elapsed_ms: f64,
}
