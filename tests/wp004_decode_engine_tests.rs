mod config {
    pub use rust_qr::config::*;
}

mod types {
    pub use rust_qr::types::*;
}

#[path = "../src/pipeline/decode_engine.rs"]
mod decode_engine;
#[allow(dead_code)]
#[path = "../src/pipeline/state.rs"]
mod state;

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use config::DetectConfig;
use state::PipelineState;
use types::Hypothesis;

#[test]
fn decode_candidate_count_is_globally_bounded() {
    let (image, width, height) = find_first_decodable_nominal_image();

    let mut state = PipelineState::new(width, height);
    state.refined_hypotheses = seed_hypotheses();

    let config = DetectConfig {
        max_decode_hypotheses: 1,
        ..DetectConfig::default()
    };

    decode_engine::run(&image, &mut state, &config);

    assert!(!state.decode_candidates.is_empty());
    assert!(state.decode_candidates.len() <= config.max_decode_hypotheses);
    assert!(
        state
            .decode_candidates
            .iter()
            .all(|candidate| !candidate.qr.payload.starts_with("wp004-hypothesis-"))
    );
}

#[test]
fn decode_candidates_are_deterministic_across_runs() {
    let (image, width, height) = find_first_decodable_nominal_image();

    let config = DetectConfig {
        max_decode_hypotheses: 6,
        ..DetectConfig::default()
    };

    let mut state_a = PipelineState::new(width, height);
    state_a.refined_hypotheses = seed_hypotheses();
    decode_engine::run(&image, &mut state_a, &config);
    let first_run = state_a.decode_candidates.clone();

    decode_engine::run(&image, &mut state_a, &config);
    assert_eq!(state_a.decode_candidates, first_run);

    let mut state_b = PipelineState::new(width, height);
    state_b.refined_hypotheses = seed_hypotheses();
    decode_engine::run(&image, &mut state_b, &config);
    assert_eq!(state_a.decode_candidates, state_b.decode_candidates);
}

#[test]
fn decode_candidates_have_bounded_scores_and_stable_ordering() {
    let (image, width, height) = find_first_decodable_nominal_image();

    let mut state = PipelineState::new(width, height);
    state.refined_hypotheses = vec![
        Hypothesis {
            id: 10,
            score: 0.55,
        },
        Hypothesis { id: 3, score: 1.40 },
        Hypothesis {
            id: 1,
            score: -0.20,
        },
        Hypothesis { id: 8, score: 0.55 },
        Hypothesis {
            id: 6,
            score: f32::NAN,
        },
    ];

    let config = DetectConfig {
        max_decode_hypotheses: 10,
        ..DetectConfig::default()
    };

    decode_engine::run(&image, &mut state, &config);

    assert!(!state.decode_candidates.is_empty());

    for candidate in &state.decode_candidates {
        assert!(candidate.score.is_finite());
        assert!((0.0..=1.0).contains(&candidate.score));
        assert_eq!(candidate.score, candidate.qr.confidence);
        assert!(!candidate.qr.payload.is_empty());
    }

    for pair in state.decode_candidates.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let score_order = left.score.total_cmp(&right.score);
        assert_ne!(score_order, Ordering::Less);
        if score_order == Ordering::Equal {
            assert!(left.qr.payload <= right.qr.payload);
        }
    }
}

fn seed_hypotheses() -> Vec<Hypothesis> {
    vec![
        Hypothesis {
            id: 42,
            score: 0.77,
        },
        Hypothesis { id: 2, score: 0.15 },
        Hypothesis {
            id: 11,
            score: 0.93,
        },
        Hypothesis { id: 5, score: 0.77 },
    ]
}

fn find_first_decodable_nominal_image() -> (Vec<u8>, usize, usize) {
    let root = Path::new("benches/images/boofcv/nominal");
    let mut labels = fs::read_dir(root)
        .expect("read nominal dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("list nominal dir")
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("txt"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    labels.sort();

    let probe_config = DetectConfig {
        max_decode_hypotheses: 8,
        ..DetectConfig::default()
    };

    for label in labels.into_iter().take(16) {
        let Some(image_path) = paired_image_path(&label) else {
            continue;
        };

        let (image, width, height) = load_rgb_image(&image_path);
        let mut state = PipelineState::new(width, height);
        state.refined_hypotheses = seed_hypotheses();
        decode_engine::run(&image, &mut state, &probe_config);

        if !state.decode_candidates.is_empty() {
            return (image, width, height);
        }
    }

    panic!("no decodable nominal image found in sampled cases");
}

fn paired_image_path(label_path: &Path) -> Option<PathBuf> {
    for ext in ["png", "jpg", "jpeg", "gif", "bmp"] {
        let candidate = label_path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
        let uppercase = label_path.with_extension(ext.to_ascii_uppercase());
        if uppercase.is_file() {
            return Some(uppercase);
        }
    }
    None
}

fn load_rgb_image(path: &Path) -> (Vec<u8>, usize, usize) {
    let decoded = image::io::Reader::open(path)
        .expect("open image")
        .decode()
        .expect("decode image")
        .to_rgb8();
    let (width, height) = decoded.dimensions();
    (decoded.into_raw(), width as usize, height as usize)
}
