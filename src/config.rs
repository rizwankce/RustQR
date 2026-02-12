#[derive(Debug, Clone)]
pub struct DetectConfig {
    pub proposal_ensemble_budget_ms: u64,
    pub hypothesis_and_refinement_budget_ms: u64,
    pub decode_budget_ms: u64,
    pub multi_qr_budget_ms: u64,
    pub emergency_cutoff_ms: u64,
    pub max_working_dim: usize,
    pub max_proposals: usize,
    pub max_hypotheses: usize,
    pub max_decode_hypotheses: usize,
    pub max_multi_qr: usize,
    pub enable_glare_suppression_view: bool,
    pub enable_ml_proposals: bool,
}

impl Default for DetectConfig {
    fn default() -> Self {
        Self {
            proposal_ensemble_budget_ms: 120,
            hypothesis_and_refinement_budget_ms: 320,
            decode_budget_ms: 420,
            multi_qr_budget_ms: 120,
            emergency_cutoff_ms: 1200,
            max_working_dim: 1024,
            max_proposals: 256,
            max_hypotheses: 64,
            max_decode_hypotheses: 16,
            max_multi_qr: 128,
            enable_glare_suppression_view: true,
            enable_ml_proposals: false,
        }
    }
}
