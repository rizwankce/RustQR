use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::FunctionMask;
use crate::decoder::qr_decoder::{orientation, payload};
use crate::models::{BitMatrix, ECLevel, MaskPattern, QRCode};

fn fallback_ec_levels() -> &'static [ECLevel] {
    &[ECLevel::L, ECLevel::M, ECLevel::Q, ECLevel::H]
}

fn strict_fallback_version_match() -> bool {
    crate::decoder::config::strict_fallback_version_match()
}

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
        let mismatches = crate::decoder::config::relaxed_finder_mismatch();
        orientations = orientation::candidate_orientations_relaxed(qr_matrix, mismatches);
    }
    if orientations.is_empty() {
        return None;
    }

    let corrected_versions = if version_num >= 7 {
        let mut versions = Vec::with_capacity(4);
        for oriented in &orientations {
            if let Some(v) = crate::decoder::version::VersionInfo::extract(oriented) {
                if (7..=40).contains(&v) && !versions.contains(&v) {
                    versions.push(v);
                }
            }
        }
        if !versions.contains(&version_num) {
            versions.push(version_num);
        }
        versions
    } else {
        vec![version_num]
    };

    let traversal_opts = [(true, false), (true, true), (false, false), (false, true)];

    for &v_num in &corrected_versions {
        let dim_check = 17 + 4 * v_num as usize;
        if dim_check != qr_matrix.width() {
            continue;
        }

        for oriented in &orientations {
            if !orientation::version_matches_candidate(oriented, v_num) {
                continue;
            }
            if let Some(format_info) = FormatInfo::extract(oriented) {
                for &(start_upward, swap_columns) in &traversal_opts {
                    if let Some(qr) = payload::try_decode_single(
                        oriented,
                        v_num,
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

        for oriented in &orientations {
            if super::global_deadline_expired() {
                return None;
            }
            if !orientation::version_matches_candidate(oriented, v_num) {
                continue;
            }
            let soft_candidates = FormatInfo::extract_soft(oriented, 6);
            for format_info in &soft_candidates {
                for &(start_upward, swap_columns) in &traversal_opts {
                    if let Some(qr) = payload::try_decode_single(
                        oriented,
                        v_num,
                        format_info,
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

    let strict_version_match = strict_fallback_version_match();
    for &v_num in &corrected_versions {
        let dim_check = 17 + 4 * v_num as usize;
        if dim_check != qr_matrix.width() {
            continue;
        }

        for oriented in &orientations {
            if super::global_deadline_expired() {
                return None;
            }
            if strict_version_match && !orientation::version_matches_candidate(oriented, v_num) {
                continue;
            }
            for &ec in fallback_ec_levels() {
                if super::global_deadline_expired() {
                    return None;
                }
                for mask in 0..8u8 {
                    if let Some(mask_pattern) = MaskPattern::from_bits(mask) {
                        let info = FormatInfo {
                            ec_level: ec,
                            mask_pattern,
                        };
                        for &(start_upward, swap_columns) in &traversal_opts {
                            if let Some(qr) = payload::try_decode_single(
                                oriented,
                                v_num,
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
    use std::time::Instant;

    if module_confidence.len() != qr_matrix.width() * qr_matrix.height() {
        return None;
    }

    let top_n = crate::decoder::config::beam_top_n();
    let max_attempts = crate::decoder::config::beam_max_attempts();
    let max_depth = crate::decoder::config::beam_max_depth();
    let conf_threshold = crate::decoder::config::beam_conf_threshold();
    let time_budget_ms = crate::decoder::config::beam_time_budget_ms();
    let uncertain_max = crate::decoder::config::beam_uncertain_max();

    let started = Instant::now();
    let budget_exhausted = || started.elapsed().as_millis() as u64 >= time_budget_ms;

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

    // Pathological case: too many uncertain modules - skip beam repair entirely
    if uncertain.len() > uncertain_max {
        return None;
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
        if attempts >= max_attempts || budget_exhausted() {
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
                if attempts >= max_attempts || budget_exhausted() {
                    break;
                }
                attempts += 1;
                if let Some(qr) =
                    decode_with_flips(qr_matrix, version_num, &[positions[a], positions[b]])
                {
                    return Some(qr);
                }
            }
            if attempts >= max_attempts || budget_exhausted() {
                break;
            }
        }
    }
    if max_depth >= 3 {
        for a in 0..positions.len() {
            for b in (a + 1)..positions.len() {
                for c in (b + 1)..positions.len() {
                    if attempts >= max_attempts || budget_exhausted() {
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
                if attempts >= max_attempts || budget_exhausted() {
                    break;
                }
            }
            if attempts >= max_attempts || budget_exhausted() {
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
