mod decode_engine;
mod geometry_refinement;
mod hypothesis_search;
mod multi_qr_iteration;
mod proposal_ensemble;
mod state;

use std::time::Instant;

use crate::config::DetectConfig;
use crate::telemetry::{DetectionRunReport, ProposalEnsembleReport, StageCounters, StageTiming};

pub fn detect_with_config(
    image: &[u8],
    width: usize,
    height: usize,
    config: &DetectConfig,
) -> DetectionRunReport {
    let global_start = Instant::now();
    let mut timings = Vec::new();

    if width == 0 || height == 0 || image.is_empty() || image.len() != width * height * 3 {
        return DetectionRunReport {
            codes: Vec::new(),
            stage_timings: timings,
            counters: StageCounters::zero(),
            proposal_ensemble: ProposalEnsembleReport::empty_with_budget(
                config.proposal_ensemble_budget_ms,
            ),
            failure_signature: Some("invalid-input".to_string()),
            total_elapsed_ms: global_start.elapsed().as_secs_f64() * 1_000.0,
        };
    }

    let mut state = state::PipelineState::new(width, height);

    let t0 = Instant::now();
    let proposal_ensemble_report = proposal_ensemble::run(image, &mut state, config);
    timings.push(StageTiming::new(
        "proposal_ensemble",
        t0.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t1 = Instant::now();
    hypothesis_search::run(&mut state, config);
    timings.push(StageTiming::new(
        "hypothesis_search",
        t1.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t2 = Instant::now();
    geometry_refinement::run(&mut state, config);
    timings.push(StageTiming::new(
        "geometry_refinement",
        t2.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t3 = Instant::now();
    decode_engine::run(&mut state, config);
    timings.push(StageTiming::new(
        "decode_engine",
        t3.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t4 = Instant::now();
    multi_qr_iteration::run(&mut state, config);
    timings.push(StageTiming::new(
        "multi_qr_iteration",
        t4.elapsed().as_secs_f64() * 1_000.0,
    ));

    let failure_signature = if state.accepted.is_empty() {
        Some("no-decode-yet".to_string())
    } else {
        None
    };

    DetectionRunReport {
        codes: state.accepted,
        stage_timings: timings,
        counters: StageCounters {
            proposals: state.proposals.len(),
            hypotheses: state.hypotheses.len(),
            refined: state.refined_hypotheses.len(),
            decode_candidates: state.decode_candidates.len(),
            accepted: state.accepted_count,
        },
        proposal_ensemble: proposal_ensemble_report,
        failure_signature,
        total_elapsed_ms: global_start.elapsed().as_secs_f64() * 1_000.0,
    }
}
