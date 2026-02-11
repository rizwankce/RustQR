mod decode_engine;
mod geometry_refinement;
mod hypothesis_search;
mod multi_qr_iteration;
mod proposal_ensemble;
mod state;

use std::time::Instant;

use crate::config::DetectConfig;
use crate::telemetry::{DetectionRunReport, ProposalEnsembleReport, StageCounters, StageTiming};

#[derive(Clone, Copy)]
enum StageId {
    ProposalEnsemble,
    HypothesisSearch,
    GeometryRefinement,
    DecodeEngine,
    MultiQrIteration,
}

#[derive(Clone, Copy)]
enum StageBudgetStatus {
    Ok,
    OverBudget,
    Reserve,
    SkippedBudget,
    SkippedCutoff,
}

fn stage_label(stage: StageId, status: StageBudgetStatus) -> &'static str {
    match (stage, status) {
        (StageId::ProposalEnsemble, StageBudgetStatus::Ok) => "proposal_ensemble:ok",
        (StageId::ProposalEnsemble, StageBudgetStatus::OverBudget) => {
            "proposal_ensemble:over-budget"
        }
        (StageId::ProposalEnsemble, StageBudgetStatus::Reserve) => "proposal_ensemble:reserve",
        (StageId::ProposalEnsemble, StageBudgetStatus::SkippedBudget) => {
            "proposal_ensemble:skipped-budget"
        }
        (StageId::ProposalEnsemble, StageBudgetStatus::SkippedCutoff) => {
            "proposal_ensemble:skipped-cutoff"
        }
        (StageId::HypothesisSearch, StageBudgetStatus::Ok) => "hypothesis_search:ok",
        (StageId::HypothesisSearch, StageBudgetStatus::OverBudget) => {
            "hypothesis_search:over-budget"
        }
        (StageId::HypothesisSearch, StageBudgetStatus::Reserve) => "hypothesis_search:reserve",
        (StageId::HypothesisSearch, StageBudgetStatus::SkippedBudget) => {
            "hypothesis_search:skipped-budget"
        }
        (StageId::HypothesisSearch, StageBudgetStatus::SkippedCutoff) => {
            "hypothesis_search:skipped-cutoff"
        }
        (StageId::GeometryRefinement, StageBudgetStatus::Ok) => "geometry_refinement:ok",
        (StageId::GeometryRefinement, StageBudgetStatus::OverBudget) => {
            "geometry_refinement:over-budget"
        }
        (StageId::GeometryRefinement, StageBudgetStatus::Reserve) => "geometry_refinement:reserve",
        (StageId::GeometryRefinement, StageBudgetStatus::SkippedBudget) => {
            "geometry_refinement:skipped-budget"
        }
        (StageId::GeometryRefinement, StageBudgetStatus::SkippedCutoff) => {
            "geometry_refinement:skipped-cutoff"
        }
        (StageId::DecodeEngine, StageBudgetStatus::Ok) => "decode_engine:ok",
        (StageId::DecodeEngine, StageBudgetStatus::OverBudget) => "decode_engine:over-budget",
        (StageId::DecodeEngine, StageBudgetStatus::Reserve) => "decode_engine:reserve",
        (StageId::DecodeEngine, StageBudgetStatus::SkippedBudget) => "decode_engine:skipped-budget",
        (StageId::DecodeEngine, StageBudgetStatus::SkippedCutoff) => "decode_engine:skipped-cutoff",
        (StageId::MultiQrIteration, StageBudgetStatus::Ok) => "multi_qr_iteration:ok",
        (StageId::MultiQrIteration, StageBudgetStatus::OverBudget) => {
            "multi_qr_iteration:over-budget"
        }
        (StageId::MultiQrIteration, StageBudgetStatus::Reserve) => "multi_qr_iteration:reserve",
        (StageId::MultiQrIteration, StageBudgetStatus::SkippedBudget) => {
            "multi_qr_iteration:skipped-budget"
        }
        (StageId::MultiQrIteration, StageBudgetStatus::SkippedCutoff) => {
            "multi_qr_iteration:skipped-cutoff"
        }
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn cutoff_reached(global_start: Instant, config: &DetectConfig) -> bool {
    elapsed_ms(global_start) >= config.emergency_cutoff_ms as f64
}

pub fn detect_with_config(
    image: &[u8],
    width: usize,
    height: usize,
    config: &DetectConfig,
) -> DetectionRunReport {
    let global_start = Instant::now();
    let mut timings = Vec::with_capacity(5);

    if width == 0 || height == 0 || image.is_empty() || image.len() != width * height * 3 {
        return DetectionRunReport {
            codes: Vec::new(),
            stage_timings: timings,
            counters: StageCounters::zero(),
            proposal_ensemble: ProposalEnsembleReport::empty_with_budget(
                config.proposal_ensemble_budget_ms,
            ),
            failure_signature: Some("invalid-input".to_string()),
            total_elapsed_ms: elapsed_ms(global_start),
        };
    }

    let mut state = state::PipelineState::new(width, height);
    let mut proposal_ensemble_report =
        ProposalEnsembleReport::empty_with_budget(config.proposal_ensemble_budget_ms);
    let mut budget_exhausted = false;
    let mut emergency_cutoff_hit = false;
    let mut hypothesis_and_refinement_elapsed_ms = 0.0f64;
    let mut decode_reserve_allowed = false;
    let mut decode_executed_in_reserve_lane = false;

    if cutoff_reached(global_start, config) {
        emergency_cutoff_hit = true;
        proposal_ensemble_report.within_budget = false;
        timings.push(StageTiming::new(
            stage_label(StageId::ProposalEnsemble, StageBudgetStatus::SkippedCutoff),
            0.0,
        ));
    } else if config.proposal_ensemble_budget_ms == 0 {
        budget_exhausted = true;
        proposal_ensemble_report.within_budget = false;
        timings.push(StageTiming::new(
            stage_label(StageId::ProposalEnsemble, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else {
        let t0 = Instant::now();
        proposal_ensemble_report = proposal_ensemble::run(image, &mut state, config);
        let stage_elapsed_ms = elapsed_ms(t0);
        let within_budget = stage_elapsed_ms <= config.proposal_ensemble_budget_ms as f64;
        if !within_budget {
            budget_exhausted = true;
            decode_reserve_allowed = true;
        }
        timings.push(StageTiming::new(
            stage_label(
                StageId::ProposalEnsemble,
                if within_budget {
                    StageBudgetStatus::Ok
                } else {
                    StageBudgetStatus::OverBudget
                },
            ),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    }

    if emergency_cutoff_hit {
        timings.push(StageTiming::new(
            stage_label(StageId::HypothesisSearch, StageBudgetStatus::SkippedCutoff),
            0.0,
        ));
    } else if budget_exhausted {
        timings.push(StageTiming::new(
            stage_label(StageId::HypothesisSearch, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else if config.hypothesis_and_refinement_budget_ms == 0 {
        budget_exhausted = true;
        timings.push(StageTiming::new(
            stage_label(StageId::HypothesisSearch, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else {
        let t1 = Instant::now();
        hypothesis_search::run(&mut state, config);
        let stage_elapsed_ms = elapsed_ms(t1);
        hypothesis_and_refinement_elapsed_ms += stage_elapsed_ms;
        let within_budget = hypothesis_and_refinement_elapsed_ms
            <= config.hypothesis_and_refinement_budget_ms as f64;
        if !within_budget {
            budget_exhausted = true;
            decode_reserve_allowed = true;
        }
        timings.push(StageTiming::new(
            stage_label(
                StageId::HypothesisSearch,
                if within_budget {
                    StageBudgetStatus::Ok
                } else {
                    StageBudgetStatus::OverBudget
                },
            ),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    }

    if emergency_cutoff_hit {
        timings.push(StageTiming::new(
            stage_label(
                StageId::GeometryRefinement,
                StageBudgetStatus::SkippedCutoff,
            ),
            0.0,
        ));
    } else if budget_exhausted {
        timings.push(StageTiming::new(
            stage_label(
                StageId::GeometryRefinement,
                StageBudgetStatus::SkippedBudget,
            ),
            0.0,
        ));
    } else if config.hypothesis_and_refinement_budget_ms == 0 {
        budget_exhausted = true;
        timings.push(StageTiming::new(
            stage_label(
                StageId::GeometryRefinement,
                StageBudgetStatus::SkippedBudget,
            ),
            0.0,
        ));
    } else {
        let t2 = Instant::now();
        geometry_refinement::run(&mut state, config);
        let stage_elapsed_ms = elapsed_ms(t2);
        hypothesis_and_refinement_elapsed_ms += stage_elapsed_ms;
        let within_budget = hypothesis_and_refinement_elapsed_ms
            <= config.hypothesis_and_refinement_budget_ms as f64;
        if !within_budget {
            budget_exhausted = true;
            decode_reserve_allowed = true;
        }
        timings.push(StageTiming::new(
            stage_label(
                StageId::GeometryRefinement,
                if within_budget {
                    StageBudgetStatus::Ok
                } else {
                    StageBudgetStatus::OverBudget
                },
            ),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    }

    if emergency_cutoff_hit {
        timings.push(StageTiming::new(
            stage_label(StageId::DecodeEngine, StageBudgetStatus::SkippedCutoff),
            0.0,
        ));
    } else if config.decode_budget_ms == 0 {
        budget_exhausted = true;
        timings.push(StageTiming::new(
            stage_label(StageId::DecodeEngine, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else if budget_exhausted && decode_reserve_allowed {
        let t3 = Instant::now();
        decode_engine::run(image, &mut state, config);
        let stage_elapsed_ms = elapsed_ms(t3);
        decode_executed_in_reserve_lane = true;
        timings.push(StageTiming::new(
            stage_label(StageId::DecodeEngine, StageBudgetStatus::Reserve),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
            decode_executed_in_reserve_lane = false;
        }
    } else if budget_exhausted {
        timings.push(StageTiming::new(
            stage_label(StageId::DecodeEngine, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else {
        let t3 = Instant::now();
        decode_engine::run(image, &mut state, config);
        let stage_elapsed_ms = elapsed_ms(t3);
        let within_budget = stage_elapsed_ms <= config.decode_budget_ms as f64;
        if !within_budget {
            budget_exhausted = true;
        }
        timings.push(StageTiming::new(
            stage_label(
                StageId::DecodeEngine,
                if within_budget {
                    StageBudgetStatus::Ok
                } else {
                    StageBudgetStatus::OverBudget
                },
            ),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    }

    if emergency_cutoff_hit {
        timings.push(StageTiming::new(
            stage_label(StageId::MultiQrIteration, StageBudgetStatus::SkippedCutoff),
            0.0,
        ));
    } else if config.multi_qr_budget_ms == 0 {
        budget_exhausted = true;
        timings.push(StageTiming::new(
            stage_label(StageId::MultiQrIteration, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else if budget_exhausted
        && (decode_executed_in_reserve_lane || !state.decode_candidates.is_empty())
    {
        let t4 = Instant::now();
        multi_qr_iteration::run(&mut state, config);
        let stage_elapsed_ms = elapsed_ms(t4);
        timings.push(StageTiming::new(
            stage_label(StageId::MultiQrIteration, StageBudgetStatus::Reserve),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    } else if budget_exhausted {
        timings.push(StageTiming::new(
            stage_label(StageId::MultiQrIteration, StageBudgetStatus::SkippedBudget),
            0.0,
        ));
    } else {
        let t4 = Instant::now();
        multi_qr_iteration::run(&mut state, config);
        let stage_elapsed_ms = elapsed_ms(t4);
        let within_budget = stage_elapsed_ms <= config.multi_qr_budget_ms as f64;
        if !within_budget {
            budget_exhausted = true;
        }
        timings.push(StageTiming::new(
            stage_label(
                StageId::MultiQrIteration,
                if within_budget {
                    StageBudgetStatus::Ok
                } else {
                    StageBudgetStatus::OverBudget
                },
            ),
            stage_elapsed_ms,
        ));
        if cutoff_reached(global_start, config) {
            emergency_cutoff_hit = true;
            budget_exhausted = false;
        }
    }

    let failure_signature = if !state.accepted.is_empty() {
        None
    } else if emergency_cutoff_hit {
        Some("emergency-cutoff".to_string())
    } else if budget_exhausted {
        Some("over-budget".to_string())
    } else {
        Some("no-decode-yet".to_string())
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
        total_elapsed_ms: elapsed_ms(global_start),
    }
}
