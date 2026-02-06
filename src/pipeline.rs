use crate::DetectionTelemetry;
use crate::decoder::qr_decoder::QrDecoder;
use crate::detector::finder::FinderPattern;
use crate::models::{BitMatrix, ECLevel, Point, QRCode};
use std::cmp::Ordering;
use std::env;

const MAX_GROUP_CANDIDATES: usize = 40;
const DEFAULT_DECODE_TOP_K: usize = 6;
const MAX_DECODE_TOP_K: usize = 64;
const HIGH_GROUP_CONFIDENCE: f32 = 0.80;
const LOW_TOP_GROUP_CONFIDENCE: f32 = 0.62;
const SINGLE_QR_CONFIDENCE_FLOOR: f32 = 0.78;
const DEFAULT_MAX_TRANSFORMS: usize = 24;
const DEFAULT_MAX_DECODE_ATTEMPTS: usize = 48;

#[derive(Clone, Copy)]
struct RankedGroupCandidate {
    group: [usize; 3],
    tl: Point,
    tr: Point,
    bl: Point,
    module_size: f32,
    raw_score: f32,
    geometry_confidence: f32,
}

fn order_finder_patterns(
    a: &FinderPattern,
    b: &FinderPattern,
    c: &FinderPattern,
) -> Option<(Point, Point, Point, f32)> {
    let patterns = [a, b, c];

    if patterns.iter().any(|p| p.module_size < 1.0) {
        return None;
    }

    // Find the right-angle corner (top-left)
    let mut best_idx = 0usize;
    let mut best_cos = f32::INFINITY;
    for i in 0..3 {
        let p = &patterns[i].center;
        let p1 = &patterns[(i + 1) % 3].center;
        let p2 = &patterns[(i + 2) % 3].center;

        let v1x = p1.x - p.x;
        let v1y = p1.y - p.y;
        let v2x = p2.x - p.x;
        let v2y = p2.y - p.y;
        let dot = v1x * v2x + v1y * v2y;
        let denom = (v1x * v1x + v1y * v1y).sqrt() * (v2x * v2x + v2y * v2y).sqrt();
        if denom == 0.0 {
            continue;
        }
        let cos = (dot / denom).abs();
        if cos < best_cos {
            best_cos = cos;
            best_idx = i;
        }
    }

    let tl = patterns[best_idx];
    let p1 = patterns[(best_idx + 1) % 3];
    let p2 = patterns[(best_idx + 2) % 3];

    let v1x = p1.center.x - tl.center.x;
    let v1y = p1.center.y - tl.center.y;
    let v2x = p2.center.x - tl.center.x;
    let v2y = p2.center.y - tl.center.y;
    let cross = v1x * v2y - v1y * v2x;

    let (tr, bl) = if cross > 0.0 { (p1, p2) } else { (p2, p1) };
    let avg_module = (tl.module_size + tr.module_size + bl.module_size) / 3.0;
    let d_tr = tl.center.distance(&tr.center);
    let d_bl = tl.center.distance(&bl.center);

    let dim1 = estimate_dimension_from_distance(d_tr, avg_module)?;
    let dim2 = estimate_dimension_from_distance(d_bl, avg_module)?;
    let dim = if dim1 == dim2 {
        dim1
    } else if (dim1 as isize - dim2 as isize).abs() <= 4 {
        ((dim1 + dim2) / 2).max(21)
    } else {
        return None;
    };

    let module_size = (d_tr + d_bl) / 2.0 / (dim as f32 - 7.0);
    let module_ratio = module_size / avg_module;
    if !(0.7..=1.3).contains(&module_ratio) {
        return None;
    }

    Some((tl.center, tr.center, bl.center, module_size))
}

fn estimate_dimension_from_distance(distance: f32, module_size: f32) -> Option<usize> {
    if module_size <= 0.0 {
        return None;
    }
    let raw_dim = distance / module_size + 7.0;
    if raw_dim < 21.0 {
        return None;
    }
    let version = ((raw_dim - 17.0) / 4.0).round() as i32;
    if !(1..=40).contains(&version) {
        return None;
    }
    Some(17 + 4 * version as usize)
}

/// Simplified finder pattern grouping with relaxed constraints.
pub(crate) fn group_finder_patterns(patterns: &[FinderPattern]) -> Vec<Vec<usize>> {
    if patterns.len() < 3 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f32)> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.module_size))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

    let mut bins: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut bin_min = 0.0f32;
    let bin_ratio = 1.25f32;

    for (idx, size) in indexed {
        if current.is_empty() {
            current.push(idx);
            bin_min = size;
            continue;
        }

        if size <= bin_min * bin_ratio {
            current.push(idx);
        } else {
            bins.push(current);
            current = vec![idx];
            bin_min = size;
        }
    }
    if !current.is_empty() {
        bins.push(current);
    }

    #[cfg(debug_assertions)]
    if cfg!(debug_assertions) && crate::debug::debug_enabled() {
        eprintln!("GROUP: Binned into {} size buckets", bins.len());
    }

    // Try each bin and its neighbor to allow slight size mismatch.
    let mut all_groups = Vec::new();
    for i in 0..bins.len() {
        let mut indices = bins[i].clone();
        if i + 1 < bins.len() {
            indices.extend_from_slice(&bins[i + 1]);
        }
        if indices.len() < 3 {
            continue;
        }
        all_groups.extend(build_groups(patterns, &indices));
    }

    all_groups
}

fn build_groups(patterns: &[FinderPattern], indices: &[usize]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();

    for idx_i in 0..indices.len() {
        let i = indices[idx_i];
        for idx_j in (idx_i + 1)..indices.len() {
            let j = indices[idx_j];
            for &k in indices.iter().skip(idx_j + 1) {
                let pi = &patterns[i];
                let pj = &patterns[j];
                let pk = &patterns[k];

                let sizes = [pi.module_size, pj.module_size, pk.module_size];
                let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
                let size_ratio = max_size / min_size;
                if size_ratio > 2.0 {
                    continue;
                }

                let d_ij = pi.center.distance(&pj.center);
                let d_ik = pi.center.distance(&pk.center);
                let d_jk = pj.center.distance(&pk.center);

                let distances = [d_ij, d_ik, d_jk];
                let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));

                let avg_module = (pi.module_size + pj.module_size + pk.module_size) / 3.0;
                if min_d < avg_module * 2.5 || max_d > 3000.0 {
                    continue;
                }
                let distortion_ratio = max_d / min_d;
                if distortion_ratio > 5.0 {
                    continue;
                }

                let a2 = d_ij * d_ij;
                let b2 = d_ik * d_ik;
                let c2 = d_jk * d_jk;

                let cos_i = (a2 + b2 - c2) / (2.0 * d_ij * d_ik);
                let cos_j = (a2 + c2 - b2) / (2.0 * d_ij * d_jk);
                let cos_k = (b2 + c2 - a2) / (2.0 * d_ik * d_jk);
                let has_right_angle = cos_i.abs() < 0.4 || cos_j.abs() < 0.4 || cos_k.abs() < 0.4;
                if !has_right_angle {
                    continue;
                }

                groups.push(vec![i, j, k]);
            }
        }
    }

    groups
}

fn group_raw_score(patterns: &[FinderPattern], group: &[usize]) -> f32 {
    if group.len() < 3 {
        return f32::INFINITY;
    }
    let p0 = &patterns[group[0]];
    let p1 = &patterns[group[1]];
    let p2 = &patterns[group[2]];

    let sizes = [p0.module_size, p1.module_size, p2.module_size];
    let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
    let size_ratio = max_size / min_size;

    let d01 = p0.center.distance(&p1.center);
    let d02 = p0.center.distance(&p2.center);
    let d12 = p1.center.distance(&p2.center);
    let distances = [d01, d02, d12];
    let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));
    let distortion = max_d / min_d;

    let a2 = d01 * d01;
    let b2 = d02 * d02;
    let c2 = d12 * d12;
    let cos_i = ((a2 + b2 - c2) / (2.0 * d01 * d02)).abs();
    let cos_j = ((a2 + c2 - b2) / (2.0 * d01 * d12)).abs();
    let cos_k = ((b2 + c2 - a2) / (2.0 * d02 * d12)).abs();
    let best_cos = cos_i.min(cos_j).min(cos_k);

    size_ratio * 2.0 + distortion + best_cos
}

fn geometry_confidence(patterns: &[FinderPattern], group: &[usize]) -> f32 {
    if group.len() < 3 {
        return 0.0;
    }

    let p0 = &patterns[group[0]];
    let p1 = &patterns[group[1]];
    let p2 = &patterns[group[2]];

    let sizes = [p0.module_size, p1.module_size, p2.module_size];
    let min_size = sizes.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_size = sizes.iter().fold(0.0f32, |a, &b| a.max(b));
    if min_size <= 0.0 {
        return 0.0;
    }
    let size_ratio = max_size / min_size;
    let size_consistency = (1.0 - (size_ratio - 1.0)).clamp(0.0, 1.0);

    let d01 = p0.center.distance(&p1.center);
    let d02 = p0.center.distance(&p2.center);
    let d12 = p1.center.distance(&p2.center);
    let distances = [d01, d02, d12];
    let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));
    if min_d <= 0.0 {
        return 0.0;
    }
    let distortion = max_d / min_d;
    let distortion_consistency = (1.0 - ((distortion - 1.0) / 4.0)).clamp(0.0, 1.0);

    let a2 = d01 * d01;
    let b2 = d02 * d02;
    let c2 = d12 * d12;
    let cos_i = ((a2 + b2 - c2) / (2.0 * d01 * d02)).abs();
    let cos_j = ((a2 + c2 - b2) / (2.0 * d01 * d12)).abs();
    let cos_k = ((b2 + c2 - a2) / (2.0 * d02 * d12)).abs();
    let best_cos = cos_i.min(cos_j).min(cos_k);
    let right_angle_consistency = (1.0 - best_cos).clamp(0.0, 1.0);

    let (tl, tr, bl, _) = match order_finder_patterns(p0, p1, p2) {
        Some(v) => v,
        None => return 0.0,
    };
    let arm_a = tl.distance(&tr);
    let arm_b = tl.distance(&bl);
    let arm_balance = if arm_a > 0.0 || arm_b > 0.0 {
        let max_arm = arm_a.max(arm_b);
        (1.0 - (arm_a - arm_b).abs() / max_arm).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (0.35 * right_angle_consistency
        + 0.25 * size_consistency
        + 0.20 * distortion_consistency
        + 0.20 * arm_balance)
        .clamp(0.0, 1.0)
}

fn rank_groups(
    patterns: &[FinderPattern],
    raw_groups: Vec<Vec<usize>>,
) -> Vec<RankedGroupCandidate> {
    let mut ranked = Vec::with_capacity(raw_groups.len());

    for group in raw_groups {
        if group.len() < 3 {
            continue;
        }
        let gi = [group[0], group[1], group[2]];
        if let Some((tl, tr, bl, module_size)) =
            order_finder_patterns(&patterns[gi[0]], &patterns[gi[1]], &patterns[gi[2]])
        {
            ranked.push(RankedGroupCandidate {
                group: gi,
                tl,
                tr,
                bl,
                module_size,
                raw_score: group_raw_score(patterns, &gi),
                geometry_confidence: geometry_confidence(patterns, &gi),
            });
        }
    }

    ranked.sort_by(|a, b| {
        let conf_order = b
            .geometry_confidence
            .partial_cmp(&a.geometry_confidence)
            .unwrap_or(Ordering::Equal);
        if conf_order != Ordering::Equal {
            return conf_order;
        }
        let raw_order = a
            .raw_score
            .partial_cmp(&b.raw_score)
            .unwrap_or(Ordering::Equal);
        if raw_order != Ordering::Equal {
            return raw_order;
        }
        a.group.cmp(&b.group)
    });
    ranked
}

fn decode_top_k_limit(total_candidates: usize) -> usize {
    if total_candidates == 0 {
        return 0;
    }
    let parsed = env::var("QR_DECODE_TOP_K")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_DECODE_TOP_K)
        .clamp(1, MAX_DECODE_TOP_K);
    parsed.min(total_candidates)
}

fn decode_f32_env(key: &str, default: f32, min: f32, max: f32) -> f32 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn decode_usize_env(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

fn high_group_confidence() -> f32 {
    decode_f32_env("QR_GROUP_HIGH_CONF", HIGH_GROUP_CONFIDENCE, 0.3, 0.99)
}

fn low_top_group_confidence() -> f32 {
    decode_f32_env("QR_GROUP_LOW_TOP_CONF", LOW_TOP_GROUP_CONFIDENCE, 0.2, 0.95)
}

fn single_qr_confidence_floor() -> f32 {
    decode_f32_env(
        "QR_SINGLE_QR_CONF_FLOOR",
        SINGLE_QR_CONFIDENCE_FLOOR,
        0.2,
        0.99,
    )
}

fn decode_proxy_confidence(qr: &QRCode) -> f32 {
    let bytes_component = (qr.data.len().min(64) as f32 / 64.0).clamp(0.0, 1.0);
    let content_len = qr.content.chars().count();
    let content_component = (content_len.min(64) as f32 / 64.0).clamp(0.0, 1.0);
    let ec_component = match qr.error_correction {
        ECLevel::L => 0.70,
        ECLevel::M => 0.80,
        ECLevel::Q => 0.90,
        ECLevel::H => 1.00,
    };
    (0.45 * bytes_component + 0.35 * content_component + 0.20 * ec_component).clamp(0.0, 1.0)
}

fn decode_candidate(
    candidate: &RankedGroupCandidate,
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
) -> Option<QRCode> {
    let mut qr = QrDecoder::decode_with_gray(
        binary,
        gray,
        width,
        height,
        &candidate.tl,
        &candidate.tr,
        &candidate.bl,
        candidate.module_size,
    )?;
    let proxy = decode_proxy_confidence(&qr);
    qr.confidence = (0.75 * candidate.geometry_confidence + 0.25 * proxy).clamp(0.0, 1.0);
    Some(qr)
}

fn decode_ranked_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
    mut telemetry: Option<&mut DetectionTelemetry>,
) -> Vec<QRCode> {
    let raw_groups = group_finder_patterns(finder_patterns);
    let ranked = rank_groups(finder_patterns, raw_groups);
    let consider = ranked.len().min(MAX_GROUP_CANDIDATES);
    let candidates = &ranked[..consider];

    if let Some(tel) = telemetry.as_mut() {
        tel.groups_found = candidates.len();
        tel.candidate_groups_scored = ranked.len();
        tel.decode_attempts = 0;
        for candidate in &ranked {
            tel.add_candidate_score(candidate.raw_score);
        }
    }

    if cfg!(debug_assertions) && crate::debug::debug_enabled() {
        eprintln!(
            "DEBUG: Found {} finder patterns, ranked {} groups",
            finder_patterns.len(),
            candidates.len()
        );
    }

    if candidates.is_empty() {
        return Vec::new();
    }

    let top_k = decode_top_k_limit(candidates.len());
    let max_transforms = decode_usize_env("QR_MAX_TRANSFORMS", DEFAULT_MAX_TRANSFORMS, 1, 512);
    let max_decode_attempts = decode_usize_env(
        "QR_MAX_DECODE_ATTEMPTS",
        DEFAULT_MAX_DECODE_ATTEMPTS,
        1,
        1024,
    );
    let high_group_conf = high_group_confidence();
    let low_top_group_conf = low_top_group_confidence();
    let single_qr_floor = single_qr_confidence_floor();
    let top = candidates[0];
    let mut should_expand = candidates
        .iter()
        .filter(|c| c.geometry_confidence >= high_group_conf)
        .take(2)
        .count()
        >= 2
        || top.geometry_confidence < low_top_group_conf;

    let mut used_transforms = 0usize;
    let mut used_attempts = 0usize;

    let mut results = Vec::new();
    if used_transforms < max_transforms && used_attempts < max_decode_attempts {
        if let Some(tel) = telemetry.as_mut() {
            tel.transforms_built += 1;
            tel.decode_attempts += 1;
        }
        used_transforms += 1;
        used_attempts += 1;
    } else {
        if let Some(tel) = telemetry.as_mut() {
            tel.budget_skips += 1;
        }
        return results;
    }

    if let Some(qr) = decode_candidate(&top, binary, gray, width, height) {
        if let Some(tel) = telemetry.as_mut() {
            tel.rs_decode_ok += 1;
            tel.payload_decoded += 1;
        }
        if qr.confidence < single_qr_floor {
            should_expand = true;
        }
        results.push(qr);
        if !should_expand {
            return results;
        }
    } else {
        should_expand = true;
    }

    if should_expand {
        for candidate in candidates.iter().take(top_k).skip(1) {
            if used_transforms >= max_transforms || used_attempts >= max_decode_attempts {
                if let Some(tel) = telemetry.as_mut() {
                    tel.budget_skips += 1;
                }
                break;
            }
            if let Some(tel) = telemetry.as_mut() {
                tel.transforms_built += 1;
                tel.decode_attempts += 1;
            }
            used_transforms += 1;
            used_attempts += 1;
            if let Some(qr) = decode_candidate(candidate, binary, gray, width, height) {
                if let Some(tel) = telemetry.as_mut() {
                    tel.rs_decode_ok += 1;
                    tel.payload_decoded += 1;
                }
                results.push(qr);
            }
        }
    }

    results
}

pub(crate) fn decode_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> Vec<QRCode> {
    decode_ranked_groups(binary, gray, width, height, finder_patterns, None)
}

/// Like `decode_groups` but also collects stage-level telemetry counters.
pub(crate) fn decode_groups_with_telemetry(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> (Vec<QRCode>, DetectionTelemetry) {
    let mut tel = DetectionTelemetry::default();
    let results =
        decode_ranked_groups(binary, gray, width, height, finder_patterns, Some(&mut tel));
    (results, tel)
}
