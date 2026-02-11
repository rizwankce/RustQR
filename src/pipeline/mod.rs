mod stage_a;
mod stage_b;
mod stage_c;
mod stage_d;
mod stage_e;
mod state;

use std::time::Instant;

use crate::config::DetectConfig;
use crate::telemetry::{DetectionRunReport, StageCounters, StageTiming};

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
            failure_signature: Some("invalid-input".to_string()),
            total_elapsed_ms: global_start.elapsed().as_secs_f64() * 1_000.0,
        };
    }

    let mut state = state::PipelineState::new(width, height);

    let t0 = Instant::now();
    stage_a::run(image, &mut state, config);
    timings.push(StageTiming::new(
        "stage_a_preprocess_and_proposal",
        t0.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t1 = Instant::now();
    stage_b::run(&mut state, config);
    timings.push(StageTiming::new(
        "stage_b_graph_search",
        t1.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t2 = Instant::now();
    stage_c::run(&mut state, config);
    timings.push(StageTiming::new(
        "stage_c_geometry_refinement",
        t2.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t3 = Instant::now();
    stage_d::run(&mut state, config);
    timings.push(StageTiming::new(
        "stage_d_decode",
        t3.elapsed().as_secs_f64() * 1_000.0,
    ));

    let t4 = Instant::now();
    stage_e::run(&mut state, config);
    timings.push(StageTiming::new(
        "stage_e_multi_qr_iteration",
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
        failure_signature,
        total_elapsed_ms: global_start.elapsed().as_secs_f64() * 1_000.0,
    }
}
