use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::FunctionMask;
use crate::decoder::qr_decoder::{orientation, payload};
use crate::models::{BitMatrix, ECLevel, MaskPattern, QRCode};

pub(super) fn decode_from_matrix(qr_matrix: &BitMatrix, version_num: u8) -> Option<QRCode> {
    decode_from_matrix_internal(qr_matrix, version_num, None)
}

pub(super) fn decode_from_matrix_with_confidence(
    qr_matrix: &BitMatrix,
    version_num: u8,
    module_confidence: &[u8],
) -> Option<QRCode> {
    decode_from_matrix_internal(qr_matrix, version_num, Some(module_confidence))
}

fn decode_from_matrix_internal(
    qr_matrix: &BitMatrix,
    version_num: u8,
    module_confidence: Option<&[u8]>,
) -> Option<QRCode> {
    let mut orientations = orientation::candidate_orientations(qr_matrix);
    if orientations.is_empty() {
        // Quiet-zone reconstruction fallback: tolerate more finder mismatches.
        let mismatches = std::env::var("QR_RELAXED_FINDER_MISMATCH")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(10)
            .clamp(4, 16);
        orientations = orientation::candidate_orientations_relaxed(qr_matrix, mismatches);
    }
    if orientations.is_empty() {
        return None;
    }

    let traversal_opts = [(true, false), (true, true), (false, false), (false, true)];

    // Fast path: if format BCH extraction succeeds, use only that format.
    for oriented in &orientations {
        if !orientation::version_matches_candidate(oriented, version_num) {
            continue;
        }
        if let Some(format_info) = FormatInfo::extract(oriented) {
            for &(start_upward, swap_columns) in &traversal_opts {
                if let Some(qr) = payload::try_decode_single(
                    oriented,
                    version_num,
                    &format_info,
                    start_upward,
                    swap_columns,
                    true,
                    false,
                    module_confidence,
                ) {
                    return Some(qr);
                }
            }
        }
    }

    // Last-resort fallback: limited EC/mask subset (not full 32-combo brute force).
    for oriented in &orientations {
        if !orientation::version_matches_candidate(oriented, version_num) {
            continue;
        }
        let fallback_ec = [ECLevel::L, ECLevel::M];
        for &ec in &fallback_ec {
            for mask in 0..8u8 {
                if let Some(mask_pattern) = MaskPattern::from_bits(mask) {
                    let info = FormatInfo {
                        ec_level: ec,
                        mask_pattern,
                    };
                    for &(start_upward, swap_columns) in &traversal_opts {
                        if let Some(qr) = payload::try_decode_single(
                            oriented,
                            version_num,
                            &info,
                            start_upward,
                            swap_columns,
                            true,
                            false,
                            module_confidence,
                        ) {
                            return Some(qr);
                        }
                    }
                }
            }
        }
    }

    if let Some(conf) = module_confidence {
        if let Some(qr) = attempt_uncertain_module_beam_repair(qr_matrix, version_num, conf) {
            return Some(qr);
        }
    }

    None
}

fn attempt_uncertain_module_beam_repair(
    qr_matrix: &BitMatrix,
    version_num: u8,
    module_confidence: &[u8],
) -> Option<QRCode> {
    if module_confidence.len() != qr_matrix.width() * qr_matrix.height() {
        return None;
    }

    let top_n = std::env::var("QR_BEAM_TOP_N")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(6)
        .clamp(2, 12);
    let max_attempts = std::env::var("QR_BEAM_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(12)
        .clamp(1, 64);
    let max_depth = std::env::var("QR_BEAM_MAX_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(2)
        .clamp(1, 3);
    let conf_threshold = std::env::var("QR_BEAM_CONF_THRESHOLD")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .unwrap_or(36);

    let dim = qr_matrix.width();
    let func = FunctionMask::new(version_num);
    let mut uncertain = Vec::new();
    for y in 0..dim {
        for x in 0..dim {
            if func.is_function(x, y) {
                continue;
            }
            let idx = y * dim + x;
            let c = module_confidence[idx];
            if c <= conf_threshold {
                uncertain.push((idx, c));
            }
        }
    }
    uncertain.sort_by_key(|(idx, c)| (*c, *idx));
    if uncertain.is_empty() {
        return None;
    }
    let positions: Vec<usize> = uncertain
        .into_iter()
        .take(top_n)
        .map(|(idx, _)| idx)
        .collect();

    let mut attempts = 0usize;
    for &i in &positions {
        if attempts >= max_attempts {
            break;
        }
        attempts += 1;
        if let Some(qr) = decode_with_flips(qr_matrix, version_num, &[i]) {
            return Some(qr);
        }
    }
    if max_depth >= 2 {
        for a in 0..positions.len() {
            for b in (a + 1)..positions.len() {
                if attempts >= max_attempts {
                    break;
                }
                attempts += 1;
                if let Some(qr) =
                    decode_with_flips(qr_matrix, version_num, &[positions[a], positions[b]])
                {
                    return Some(qr);
                }
            }
            if attempts >= max_attempts {
                break;
            }
        }
    }
    if max_depth >= 3 {
        for a in 0..positions.len() {
            for b in (a + 1)..positions.len() {
                for c in (b + 1)..positions.len() {
                    if attempts >= max_attempts {
                        break;
                    }
                    attempts += 1;
                    if let Some(qr) = decode_with_flips(
                        qr_matrix,
                        version_num,
                        &[positions[a], positions[b], positions[c]],
                    ) {
                        return Some(qr);
                    }
                }
                if attempts >= max_attempts {
                    break;
                }
            }
            if attempts >= max_attempts {
                break;
            }
        }
    }

    None
}

fn decode_with_flips(qr_matrix: &BitMatrix, version_num: u8, flips: &[usize]) -> Option<QRCode> {
    let dim = qr_matrix.width();
    let mut mutated = qr_matrix.clone();
    for &idx in flips {
        if idx >= dim * dim {
            continue;
        }
        let x = idx % dim;
        let y = idx / dim;
        mutated.set(x, y, !mutated.get(x, y));
    }
    decode_from_matrix_internal(&mutated, version_num, None)
}
