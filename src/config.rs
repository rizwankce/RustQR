#[derive(Debug, Clone)]
pub struct DetectConfig {
    pub stage_a_budget_ms: u64,
    pub stage_bc_budget_ms: u64,
    pub stage_d_budget_ms: u64,
    pub stage_e_budget_ms: u64,
    pub emergency_cutoff_ms: u64,
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
            stage_a_budget_ms: 120,
            stage_bc_budget_ms: 320,
            stage_d_budget_ms: 420,
            stage_e_budget_ms: 120,
            emergency_cutoff_ms: 1200,
            max_proposals: 256,
            max_hypotheses: 64,
            max_decode_hypotheses: 16,
            max_multi_qr: 128,
            enable_glare_suppression_view: true,
            enable_ml_proposals: false,
        }
    }
}
