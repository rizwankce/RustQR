use crate::DetectionTelemetry;
use crate::decoder::format::FormatInfo;
use crate::decoder::qr_decoder::{DecodeRequestContext, QrDecoder};
use crate::detector::finder::FinderPattern;
use crate::models::{BitMatrix, ECLevel, Point, QRCode};
use crate::utils::geometry::PerspectiveTransform;
use std::cmp::Ordering;
use std::collections::HashSet;

const MAX_GROUP_CANDIDATES: usize = 40;
const DEFAULT_DECODE_TOP_K: usize = 6;
const MAX_DECODE_TOP_K: usize = 64;
const HIGH_GROUP_CONFIDENCE: f32 = 0.80;
const LOW_TOP_GROUP_CONFIDENCE: f32 = 0.62;
const SINGLE_QR_CONFIDENCE_FLOOR: f32 = 0.78;
const DEFAULT_MAX_TRANSFORMS: usize = 24;
const DEFAULT_MAX_DECODE_ATTEMPTS: usize = 48;
const DEFAULT_MAX_REGIONS: usize = 8;
const DEFAULT_PER_REGION_TOP_K: usize = 4;
const HIGH_CONFIDENCE_LANE_MIN: f32 = 0.78;
const MEDIUM_CONFIDENCE_LANE_MIN: f32 = 0.56;
// Dense scenes need local grouping well before the old global cubic path gets
// expensive.  This is deliberately a proposal-count threshold, not an image
// dimension: a uniformly scaled scene follows the same path.
const SPATIAL_GROUP_TRIGGER: usize = 12;
// Finder scans in dense raster scenes can retain harmless nearby evidence in
// addition to the three true markers.  Keep enough local neighbours to form
// the valid triple without returning to scene-wide cubic grouping.  This is a
// fixed per-anchor bound; the resulting group frontier remains capped below.
const SPATIAL_NEIGHBOR_LIMIT: usize = 16;
const DENSE_MAX_GROUP_CANDIDATES: usize = 128;
// The largest possible finder-to-finder pair in a Model 2 symbol is the
// diagonal of a version-40 grid.  Keep a little perspective headroom, but
// express the bound in modules rather than image pixels: the same QR must be
// groupable after a uniform resize.
const MAX_FINDER_PAIR_MODULES: f32 = 260.0;

#[derive(Clone, Copy)]
struct RankedGroupCandidate {
    group: [usize; 3],
    tl: Point,
    tr: Point,
    bl: Point,
    module_size: f32,
    raw_score: f32,
    rerank_score: f32,
    saturation_coverage: f32,
    geometry_confidence: f32,
}

#[derive(Clone, Copy, Debug)]
enum StrategyProfile {
    FastSingle,
    MultiQrHeavy,
    RotationHeavy,
    HighVersionPrecision,
    LowContrastRecovery,
}

#[derive(Clone, Copy)]
enum ConfidenceLane {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy)]
struct LaneBudget {
    high: usize,
    medium: usize,
    low: usize,
}

impl LaneBudget {
    fn consume(&mut self, lane: ConfidenceLane) -> bool {
        match lane {
            ConfidenceLane::High => {
                if self.high == 0 {
                    false
                } else {
                    self.high -= 1;
                    true
                }
            }
            ConfidenceLane::Medium => {
                if self.medium == 0 {
                    false
                } else {
                    self.medium -= 1;
                    true
                }
            }
            ConfidenceLane::Low => {
                if self.low == 0 {
                    false
                } else {
                    self.low -= 1;
                    true
                }
            }
        }
    }
}

impl StrategyProfile {
    fn as_str(self) -> &'static str {
        match self {
            StrategyProfile::FastSingle => "fast_single",
            StrategyProfile::MultiQrHeavy => "multi_qr_heavy",
            StrategyProfile::RotationHeavy => "rotation_heavy",
            StrategyProfile::HighVersionPrecision => "high_version_precision",
            StrategyProfile::LowContrastRecovery => "low_contrast_recovery",
        }
    }
}

#[derive(Clone)]
struct RegionCluster {
    indices: Vec<usize>,
    center: Point,
}

#[derive(Clone, Copy, Default)]
struct FastSignals {
    blur_metric: f32,
    saturation_ratio: f32,
    skew_estimate_deg: f32,
    region_density_proxy: f32,
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
    } else if (dim1 as isize - dim2 as isize).abs() <= 8 {
        ((dim1 + dim2) / 2).max(21)
    } else {
        return None;
    };

    let module_size = (d_tr + d_bl) / 2.0 / (dim as f32 - 7.0);
    let module_ratio = module_size / avg_module;
    if !(0.5..=1.8).contains(&module_ratio) {
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
pub(crate) fn group_finder_patterns(patterns: &[FinderPattern]) -> Vec<[usize; 3]> {
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
    let max_groups = if patterns.len() > SPATIAL_GROUP_TRIGGER {
        DENSE_MAX_GROUP_CANDIDATES
    } else {
        crate::decoder::config::max_groups_to_rank()
    };
    for i in 0..bins.len() {
        if all_groups.len() >= max_groups {
            break;
        }
        let mut indices = bins[i].clone();
        if i + 1 < bins.len() {
            indices.extend_from_slice(&bins[i + 1]);
        }
        if indices.len() < 3 {
            continue;
        }
        all_groups.extend(build_groups_clustered(patterns, &indices));
        if all_groups.len() >= max_groups {
            break;
        }
    }

    all_groups
}

fn build_groups(patterns: &[FinderPattern], indices: &[usize]) -> Vec<[usize; 3]> {
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
                // Relaxed from 2.0 to 2.5 for perspective-distorted QR codes
                let size_ratio = max_size / min_size;
                if size_ratio > 2.5 {
                    continue;
                }

                let d_ij = pi.center.distance(&pj.center);
                let d_ik = pi.center.distance(&pk.center);
                let d_jk = pj.center.distance(&pk.center);

                let distances = [d_ij, d_ik, d_jk];
                let min_d = distances.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_d = distances.iter().fold(0.0f32, |a, &b| a.max(b));

                let avg_module = (pi.module_size + pj.module_size + pk.module_size) / 3.0;
                if min_d < avg_module * 2.5 || max_d > avg_module * MAX_FINDER_PAIR_MODULES {
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

                groups.push([i, j, k]);
            }
        }
    }

    groups
}

fn build_groups_clustered(patterns: &[FinderPattern], indices: &[usize]) -> Vec<[usize; 3]> {
    if indices.len() <= SPATIAL_GROUP_TRIGGER {
        return build_groups(patterns, indices);
    }

    // Each finder owns a bounded neighbourhood of similarly scaled nearby
    // proposals.  A QR's other two finders are local at its own module scale,
    // whereas unrelated symbols are usually farther away.  This replaces the
    // scene-wide cubic expansion with O(n^2 + n*k^3), and importantly does not
    // use a fixed pixel radius.
    let mut groups: Vec<[usize; 3]> = Vec::new();
    let mut seen = HashSet::new();
    // First retain isolated local components.  In a dense raster the three
    // finders of a small symbol are often separated from the next symbol by a
    // visible gap even when a global triplet search can manufacture many
    // cross-symbol right angles.  The component threshold is derived from
    // each proposal's nearest compatible neighbour, so it scales with pitch.
    let nearest = indices
        .iter()
        .map(|&idx| {
            indices
                .iter()
                .copied()
                .filter(|&other| other != idx)
                .filter_map(|other| {
                    let a = &patterns[idx];
                    let b = &patterns[other];
                    let size_ratio =
                        a.module_size.max(b.module_size) / a.module_size.min(b.module_size);
                    (size_ratio <= 2.5).then(|| a.center.distance(&b.center))
                })
                .min_by(|a, b| a.total_cmp(b))
                .unwrap_or(f32::INFINITY)
        })
        .collect::<Vec<_>>();
    let mut adjacency = vec![Vec::new(); indices.len()];
    for left in 0..indices.len() {
        for right in (left + 1)..indices.len() {
            let distance = patterns[indices[left]]
                .center
                .distance(&patterns[indices[right]].center);
            let local_limit = nearest[left].min(nearest[right]) * 1.55;
            if distance <= local_limit {
                adjacency[left].push(right);
                adjacency[right].push(left);
            }
        }
    }
    let mut visited = vec![false; indices.len()];
    for start in 0..indices.len() {
        if visited[start] {
            continue;
        }
        let mut stack = vec![start];
        visited[start] = true;
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            component.push(indices[current]);
            for &next in &adjacency[current] {
                if !visited[next] {
                    visited[next] = true;
                    stack.push(next);
                }
            }
        }
        if component.len() == 3 && !build_groups(patterns, &component).is_empty() {
            component.sort_unstable();
            seen.insert((component[0], component[1], component[2]));
            groups.push([component[0], component[1], component[2]]);
        }
    }
    // Keep each anchor's strongest local hypothesis before adding the broader
    // neighbourhood alternatives.  A dense finder scan often has extra
    // observations around a real marker; globally sorting every combination
    // lets those cross-symbol triples crowd out the actual symbol.  This
    // produces at most one seed per proposal and remains bounded by the
    // proposal frontier.
    // Before admitting broad neighbourhood combinations, ask each finder for
    // a triangle at its nearest local scale.  On a dense raster grid a
    // symbol's two companion finders are the closest compatible pair; the
    // next symbol is separated by its quiet zone.  This preserves the true
    // triple ahead of cross-symbol right angles without using an image-wide
    // pixel radius.  The broad seed below remains as the perspective-tolerant
    // fallback.
    let mut tight_anchor_seeds = Vec::new();
    let mut anchor_seeds = Vec::new();
    for &anchor in indices {
        let anchor_pattern = &patterns[anchor];
        let mut neighbors = indices
            .iter()
            .copied()
            .filter(|&other| other != anchor)
            .filter_map(|other| {
                let candidate = &patterns[other];
                let size_ratio = anchor_pattern.module_size.max(candidate.module_size)
                    / anchor_pattern.module_size.min(candidate.module_size);
                let distance = anchor_pattern.center.distance(&candidate.center);
                (size_ratio <= 2.5
                    && distance
                        <= anchor_pattern.module_size.max(candidate.module_size)
                            * MAX_FINDER_PAIR_MODULES)
                    .then_some((other, distance))
            })
            .collect::<Vec<_>>();
        neighbors.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        if let Some((_, nearest_distance)) = neighbors.first() {
            let mut tight = Vec::with_capacity(SPATIAL_NEIGHBOR_LIMIT + 1);
            tight.push(anchor);
            tight.extend(
                neighbors
                    .iter()
                    .take(SPATIAL_NEIGHBOR_LIMIT)
                    .filter(|(_, distance)| *distance <= *nearest_distance * 1.20)
                    .map(|(idx, _)| *idx),
            );
            tight.sort_unstable();
            if let Some(best) = build_groups(patterns, &tight).iter().min_by(|a, b| {
                group_raw_score(patterns, a)
                    .total_cmp(&group_raw_score(patterns, b))
                    .then_with(|| a.cmp(b))
            }) {
                tight_anchor_seeds.push(*best);
            }
        }
        let mut local = Vec::with_capacity(SPATIAL_NEIGHBOR_LIMIT + 1);
        local.push(anchor);
        local.extend(
            neighbors
                .into_iter()
                .take(SPATIAL_NEIGHBOR_LIMIT)
                .map(|(idx, _)| idx),
        );
        local.sort_unstable();
        let local_groups = build_groups(patterns, &local);
        if let Some(best) = local_groups.iter().min_by(|a, b| {
            group_raw_score(patterns, a)
                .total_cmp(&group_raw_score(patterns, b))
                .then_with(|| a.cmp(b))
        }) {
            anchor_seeds.push(*best);
        }
    }
    for triple in tight_anchor_seeds.into_iter().chain(anchor_seeds) {
        let mut key = [triple[0], triple[1], triple[2]];
        key.sort_unstable();
        if seen.insert((key[0], key[1], key[2])) {
            groups.push(triple);
        }
    }
    let seeded_count = groups.len();
    for &anchor in indices {
        let anchor_pattern = &patterns[anchor];
        let mut neighbors = indices
            .iter()
            .copied()
            .filter(|&other| other != anchor)
            .filter_map(|other| {
                let candidate = &patterns[other];
                let size_ratio = anchor_pattern.module_size.max(candidate.module_size)
                    / anchor_pattern.module_size.min(candidate.module_size);
                let distance = anchor_pattern.center.distance(&candidate.center);
                (size_ratio <= 2.5
                    && distance
                        <= anchor_pattern.module_size.max(candidate.module_size)
                            * MAX_FINDER_PAIR_MODULES)
                    .then_some((other, distance))
            })
            .collect::<Vec<_>>();
        neighbors.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let mut local = Vec::with_capacity(SPATIAL_NEIGHBOR_LIMIT + 1);
        local.push(anchor);
        local.extend(
            neighbors
                .into_iter()
                .take(SPATIAL_NEIGHBOR_LIMIT)
                .map(|(idx, _)| idx),
        );
        local.sort_unstable();
        for triple in build_groups(patterns, &local) {
            let mut key = [triple[0], triple[1], triple[2]];
            key.sort_unstable();
            if seen.insert((key[0], key[1], key[2])) {
                groups.push(triple);
            }
        }
    }
    // The exact local components are kept first.  The remaining decode budget
    // is intentionally finite, so rank generic neighbourhood triples by
    // geometry rather than by visit order.
    let (_, remaining) = groups.split_at_mut(seeded_count);
    remaining.sort_by(|a, b| {
        group_raw_score(patterns, a)
            .total_cmp(&group_raw_score(patterns, b))
            .then_with(|| a.cmp(b))
    });
    // `groups` deliberately retains its deterministic insertion order here:
    // isolated components and tight local seeds must remain ahead of the
    // generic broad-neighbourhood frontier when the bounded cap is applied.
    groups.truncate(DENSE_MAX_GROUP_CANDIDATES);
    groups
}

fn group_raw_score(patterns: &[FinderPattern], group: &[usize; 3]) -> f32 {
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
    let (best_cos, arm_a, arm_b) = if cos_i <= cos_j && cos_i <= cos_k {
        (cos_i, d01, d02)
    } else if cos_j <= cos_k {
        (cos_j, d01, d12)
    } else {
        (cos_k, d02, d12)
    };
    let arm_imbalance = (arm_a - arm_b).abs() / arm_a.max(arm_b).max(1.0);

    // A real QR's two legs originate at the same finder and have closely
    // related lengths.  Crossing finders from neighboring symbols can make a
    // nominal right angle, but usually fail this local scale relationship.
    size_ratio * 2.0 + distortion + best_cos + arm_imbalance * 4.0
}

fn geometry_confidence(patterns: &[FinderPattern], group: &[usize; 3]) -> f32 {
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

fn right_angle_residual(patterns: &[FinderPattern], group: &[usize]) -> f32 {
    if group.len() < 3 {
        return 1.0;
    }
    let p0 = &patterns[group[0]];
    let p1 = &patterns[group[1]];
    let p2 = &patterns[group[2]];
    let d01 = p0.center.distance(&p1.center);
    let d02 = p0.center.distance(&p2.center);
    let d12 = p1.center.distance(&p2.center);
    if d01 <= 0.0 || d02 <= 0.0 || d12 <= 0.0 {
        return 1.0;
    }
    let a2 = d01 * d01;
    let b2 = d02 * d02;
    let c2 = d12 * d12;
    let cos_i = ((a2 + b2 - c2) / (2.0 * d01 * d02)).abs();
    let cos_j = ((a2 + c2 - b2) / (2.0 * d01 * d12)).abs();
    let cos_k = ((b2 + c2 - a2) / (2.0 * d02 * d12)).abs();
    cos_i.min(cos_j).min(cos_k).clamp(0.0, 1.0)
}

fn module_size_agreement(patterns: &[FinderPattern], group: &[usize]) -> f32 {
    if group.len() < 3 {
        return 0.0;
    }
    let s0 = patterns[group[0]].module_size.max(1e-3);
    let s1 = patterns[group[1]].module_size.max(1e-3);
    let s2 = patterns[group[2]].module_size.max(1e-3);
    let mean = (s0 + s1 + s2) / 3.0;
    let var =
        ((s0 - mean) * (s0 - mean) + (s1 - mean) * (s1 - mean) + (s2 - mean) * (s2 - mean)) / 3.0;
    let cv = var.sqrt() / mean;
    (1.0 - cv.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn stripe_alternation_agreement(binary: &BitMatrix, start: &Point, end: &Point) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 6.0 {
        return 0.0;
    }
    let steps = distance.round() as usize;
    let mut prev: Option<bool> = None;
    let mut transitions = 0usize;
    let mut valid_samples = 0usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (start.x + dx * t).round() as isize;
        let y = (start.y + dy * t).round() as isize;
        if x < 0 || y < 0 || x as usize >= binary.width() || y as usize >= binary.height() {
            continue;
        }
        let bit = binary.get(x as usize, y as usize);
        if let Some(p) = prev {
            if p != bit {
                transitions += 1;
            }
        }
        prev = Some(bit);
        valid_samples += 1;
    }
    if valid_samples < 8 {
        return 0.0;
    }
    let opportunities = valid_samples - 1;
    (transitions as f32 / opportunities as f32).clamp(0.0, 1.0)
}

fn timing_line_agreement(binary: &BitMatrix, tl: &Point, tr: &Point, bl: &Point) -> f32 {
    let h = stripe_alternation_agreement(binary, tl, tr);
    let v = stripe_alternation_agreement(binary, tl, bl);
    (0.5 * h + 0.5 * v).clamp(0.0, 1.0)
}

fn global_saturation_ratio(gray: &[u8]) -> f32 {
    if gray.is_empty() {
        return 0.0;
    }
    let saturated = gray.iter().filter(|&&v| v >= 245).count();
    (saturated as f32 / gray.len() as f32).clamp(0.0, 1.0)
}

fn line_saturation_coverage(
    gray: &[u8],
    width: usize,
    height: usize,
    start: &Point,
    end: &Point,
) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 4.0 {
        return 0.0;
    }
    let steps = distance.round() as usize;
    let mut saturated = 0usize;
    let mut total = 0usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (start.x + dx * t).round() as isize;
        let y = (start.y + dy * t).round() as isize;
        if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
            continue;
        }
        let px = gray[y as usize * width + x as usize];
        if px >= 245 {
            saturated += 1;
        }
        total += 1;
    }
    if total == 0 {
        0.0
    } else {
        (saturated as f32 / total as f32).clamp(0.0, 1.0)
    }
}

#[allow(clippy::too_many_arguments)]
fn geometry_rerank_score(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    saturation_mask_enabled: bool,
    patterns: &[FinderPattern],
    group: &[usize],
    tl: &Point,
    tr: &Point,
    bl: &Point,
) -> (f32, f32) {
    let timing = timing_line_agreement(binary, tl, tr, bl);
    let saturation_coverage = if saturation_mask_enabled {
        let h = line_saturation_coverage(gray, width, height, tl, tr);
        let v = line_saturation_coverage(gray, width, height, tl, bl);
        (0.5 * h + 0.5 * v).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let timing_adjusted = if saturation_mask_enabled {
        (timing * (1.0 - 0.6 * saturation_coverage)).clamp(0.0, 1.0)
    } else {
        timing
    };
    let module_agreement = module_size_agreement(patterns, group);
    let right_angle = 1.0 - right_angle_residual(patterns, group);
    (
        (0.40 * timing_adjusted + 0.35 * module_agreement + 0.25 * right_angle).clamp(0.0, 1.0),
        saturation_coverage,
    )
}

fn rank_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    saturation_mask_enabled: bool,
    patterns: &[FinderPattern],
    raw_groups: Vec<[usize; 3]>,
) -> (Vec<RankedGroupCandidate>, usize) {
    // The normal path only needs a compact ranking frontier.  Once the
    // spatial grouper is active, however, its first entries are isolated
    // local triples for distinct symbols.  Retaining that bounded frontier is
    // necessary for dense routing; collapsing it back to the ordinary 16
    // candidates discards most of a 50-symbol scene before request budgets
    // can make the final decision.
    let max_groups = if patterns.len() > SPATIAL_GROUP_TRIGGER {
        DENSE_MAX_GROUP_CANDIDATES
    } else {
        crate::decoder::config::max_groups_to_rank()
    };
    // Hard cap: truncate groups early to prevent O(n) slowdown on pathological images
    let mut ranked = Vec::with_capacity(raw_groups.len().min(max_groups));
    let mut rejected = 0usize;

    for group in raw_groups.iter().take(max_groups) {
        if ranked.len() >= max_groups {
            break;
        }
        if group.len() < 3 {
            continue;
        }
        let gi = [group[0], group[1], group[2]];
        if let Some((tl, tr, bl, module_size)) =
            order_finder_patterns(&patterns[gi[0]], &patterns[gi[1]], &patterns[gi[2]])
        {
            let (rerank_score, saturation_coverage) = geometry_rerank_score(
                binary,
                gray,
                width,
                height,
                saturation_mask_enabled,
                patterns,
                &gi,
                &tl,
                &tr,
                &bl,
            );
            ranked.push(RankedGroupCandidate {
                group: gi,
                tl,
                tr,
                bl,
                module_size,
                raw_score: group_raw_score(patterns, &gi),
                rerank_score,
                saturation_coverage,
                geometry_confidence: geometry_confidence(patterns, &gi),
            });
        } else {
            rejected += 1;
        }
    }

    ranked.sort_by(|a, b| {
        let rerank_order = b
            .rerank_score
            .partial_cmp(&a.rerank_score)
            .unwrap_or(Ordering::Equal);
        if rerank_order != Ordering::Equal {
            return rerank_order;
        }
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
    (ranked, rejected)
}

fn decode_top_k_limit(total_candidates: usize) -> usize {
    if total_candidates == 0 {
        return 0;
    }
    DEFAULT_DECODE_TOP_K
        .clamp(1, MAX_DECODE_TOP_K)
        .min(total_candidates)
}

/// Choose a bounded number of spatial regions to visit.
///
/// A dense scene normally produces one ranked candidate per physical symbol.
/// Limiting its regions to a fixed 32 silently discards otherwise valid,
/// disjoint candidates before sampling.  The group frontier is already capped
/// at [`DENSE_MAX_GROUP_CANDIDATES`], so allowing one region for each retained
/// dense candidate preserves that safety bound while avoiding this loss.
fn region_limit_for_strategy(strategy: StrategyProfile, candidate_count: usize) -> usize {
    match strategy {
        StrategyProfile::MultiQrHeavy => {
            candidate_count.clamp(DEFAULT_MAX_REGIONS, DENSE_MAX_GROUP_CANDIDATES)
        }
        _ => DEFAULT_MAX_REGIONS,
    }
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

/// Dense routing has an explicit, request-bounded recovery quota.  Keep the
/// historical matrix fallback behavior for every other strategy; only the
/// clean dense lane skips fallback hypotheses after that quota is exhausted.
fn matrix_recovery_allowed(strategy: StrategyProfile, allow_heavy_recovery: bool) -> bool {
    !matches!(strategy, StrategyProfile::MultiQrHeavy) || allow_heavy_recovery
}

#[allow(clippy::too_many_arguments)]
fn decode_candidate(
    candidate: &RankedGroupCandidate,
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    allow_heavy_recovery: bool,
    allow_matrix_recovery: bool,
    blur_metric: f32,
    context: &mut DecodeRequestContext,
) -> Option<QRCode> {
    // Skip heavy recovery for very blurry images - it's unlikely to succeed and wastes time
    let recovery_threshold = crate::decoder::config::blur_disable_recovery_threshold();
    let effective_heavy_recovery = allow_heavy_recovery && blur_metric >= recovery_threshold;

    let mut qr = QrDecoder::decode_with_gray_in_context(
        binary,
        gray,
        width,
        height,
        &candidate.tl,
        &candidate.tr,
        &candidate.bl,
        candidate.module_size,
        effective_heavy_recovery,
        allow_matrix_recovery,
        context,
    )?;
    let proxy = decode_proxy_confidence(&qr);
    qr.confidence = (0.75 * candidate.geometry_confidence + 0.25 * proxy).clamp(0.0, 1.0);
    Some(qr)
}

#[allow(dead_code)]
fn probe_candidate_quality(
    candidate: &RankedGroupCandidate,
    binary: &BitMatrix,
    _gray: &[u8],
    _width: usize,
    _height: usize,
) -> f32 {
    let bottom_right = Point::new(
        candidate.tr.x + candidate.bl.x - candidate.tl.x,
        candidate.tr.y + candidate.bl.y - candidate.tl.y,
    );
    let estimated_dimension = {
        let d_tr = candidate.tl.distance(&candidate.tr);
        let d_bl = candidate.tl.distance(&candidate.bl);
        let avg_d = (d_tr + d_bl) / 2.0;
        let raw = avg_d / candidate.module_size + 7.0;
        let version = ((raw - 17.0) / 4.0).round() as i32;
        if !(1..=40).contains(&version) {
            return 0.0;
        }
        17 + 4 * version as usize
    };

    let src = [
        Point::new(3.5, 3.5),
        Point::new(estimated_dimension as f32 - 3.5, 3.5),
        Point::new(3.5, estimated_dimension as f32 - 3.5),
        Point::new(
            estimated_dimension as f32 - 3.5,
            estimated_dimension as f32 - 3.5,
        ),
    ];
    let dst = [candidate.tl, candidate.tr, candidate.bl, bottom_right];
    let transform = match PerspectiveTransform::from_points(&src, &dst) {
        Some(t) => t,
        None => return 0.0,
    };

    let dim = estimated_dimension;
    let mut qr_matrix = BitMatrix::new(dim, dim);
    for y in 0..dim {
        for x in 0..dim {
            let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let img_point = transform.transform(&module_center);
            let ix = img_point.x.round() as isize;
            let iy = img_point.y.round() as isize;
            if ix >= 0
                && iy >= 0
                && (ix as usize) < binary.width()
                && (iy as usize) < binary.height()
            {
                qr_matrix.set(x, y, binary.get(ix as usize, iy as usize));
            }
        }
    }

    let mut timing_score = 0.0f32;
    if dim >= 21 {
        let mut h_correct = 0usize;
        let mut v_correct = 0usize;
        let timing_len = dim - 14;
        for i in 0..timing_len {
            let expected = (i % 2) == 0;
            if qr_matrix.get(8 + i, 6) == expected {
                h_correct += 1;
            }
            if qr_matrix.get(6, 8 + i) == expected {
                v_correct += 1;
            }
        }
        if timing_len > 0 {
            timing_score = (h_correct + v_correct) as f32 / (2 * timing_len) as f32;
        }
    }

    let format_score = if FormatInfo::extract(&qr_matrix).is_some() {
        1.0
    } else {
        0.3
    };

    0.6 * timing_score + 0.4 * format_score
}

fn candidate_center(c: &RankedGroupCandidate) -> Point {
    let br = Point::new(c.tr.x + c.bl.x - c.tl.x, c.tr.y + c.bl.y - c.tl.y);
    Point::new(
        (c.tl.x + c.tr.x + c.bl.x + br.x) * 0.25,
        (c.tl.y + c.tr.y + c.bl.y + br.y) * 0.25,
    )
}

fn candidate_bbox(c: &RankedGroupCandidate) -> (f32, f32, f32, f32) {
    let br = Point::new(c.tr.x + c.bl.x - c.tl.x, c.tr.y + c.bl.y - c.tl.y);
    let min_x = c.tl.x.min(c.tr.x).min(c.bl.x).min(br.x);
    let max_x = c.tl.x.max(c.tr.x).max(c.bl.x).max(br.x);
    let min_y = c.tl.y.min(c.tr.y).min(c.bl.y).min(br.y);
    let max_y = c.tl.y.max(c.tr.y).max(c.bl.y).max(br.y);
    (min_x, min_y, max_x, max_y)
}

fn bbox_iou(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> f32 {
    let ix0 = a.0.max(b.0);
    let iy0 = a.1.max(b.1);
    let ix1 = a.2.min(b.2);
    let iy1 = a.3.min(b.3);
    let iw = (ix1 - ix0).max(0.0);
    let ih = (iy1 - iy0).max(0.0);
    let inter = iw * ih;
    if inter <= 0.0 {
        return 0.0;
    }
    let area_a = (a.2 - a.0).max(0.0) * (a.3 - a.1).max(0.0);
    let area_b = (b.2 - b.0).max(0.0) * (b.3 - b.1).max(0.0);
    let denom = (area_a + area_b - inter).max(1e-6);
    inter / denom
}

const DECODE_ACCEPTANCE_FLOOR: f32 = 0.56;
const DECODE_RELAXED_ACCEPTANCE_FLOOR: f32 = 0.64;

fn payload_plausibility(content: &str) -> f32 {
    if content.is_empty() {
        return 0.0;
    }
    let len = content.chars().count();
    let printable = content
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ' || *c == '\n' || *c == '\r' || *c == '\t')
        .count();
    let ascii = content.chars().filter(|c| c.is_ascii()).count();
    let printable_ratio = printable as f32 / len as f32;
    let ascii_ratio = ascii as f32 / len as f32;
    let length_bonus = (len.min(128) as f32 / 128.0).clamp(0.0, 1.0);
    (0.5 * printable_ratio + 0.3 * ascii_ratio + 0.2 * length_bonus).clamp(0.0, 1.0)
}

fn acceptance_score(qr: &QRCode, geometry_conf: f32) -> f32 {
    let rs_quality = qr.confidence.clamp(0.0, 1.0);
    let format_version_consistency = match qr.version {
        crate::models::Version::Model2(v) if v >= 7 => 0.95,
        _ => 0.85,
    };
    let ec_strength = match qr.error_correction {
        ECLevel::H => 1.0,
        ECLevel::Q => 0.92,
        ECLevel::M => 0.84,
        ECLevel::L => 0.76,
    };
    let plausibility = payload_plausibility(&qr.content);
    (0.30 * rs_quality
        + 0.20 * geometry_conf
        + 0.20 * format_version_consistency
        + 0.15 * ec_strength
        + 0.15 * plausibility)
        .clamp(0.0, 1.0)
}

fn dedupe_results(
    results: &mut Vec<QRCode>,
    accepted_geometries: &mut Vec<(f32, f32, f32, f32)>,
    accepted_proposals: &mut HashSet<usize>,
    candidate: &RankedGroupCandidate,
    qr: QRCode,
) -> bool {
    // A finder proposal may describe only one accepted symbol.  Do this after
    // decoding rather than while ranking: a high-scoring false triple must not
    // starve a valid neighbouring triple in a dense scene.
    if candidate
        .group
        .iter()
        .any(|proposal| accepted_proposals.contains(proposal))
    {
        return false;
    }
    let geom = candidate_bbox(candidate);
    if accepted_geometries
        .iter()
        .any(|&existing| bbox_iou(existing, geom) >= 0.72)
    {
        return false;
    }
    accepted_geometries.push(geom);
    accepted_proposals.extend(candidate.group);
    results.push(qr);
    true
}

fn cluster_regions(candidates: &[RankedGroupCandidate], max_regions: usize) -> Vec<RegionCluster> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut regions: Vec<RegionCluster> = Vec::new();
    for (idx, c) in candidates.iter().enumerate() {
        let center = candidate_center(c);
        let scale = ((c.tl.distance(&c.tr) + c.tl.distance(&c.bl)) * 0.5).max(20.0);
        let attach = regions.iter().position(|r| {
            let d = center.distance(&r.center);
            d <= scale * 1.2
        });
        if let Some(region_idx) = attach {
            let r = &mut regions[region_idx];
            r.indices.push(idx);
            let n = r.indices.len() as f32;
            r.center = Point::new(
                (r.center.x * (n - 1.0) + center.x) / n,
                (r.center.y * (n - 1.0) + center.y) / n,
            );
        } else if regions.len() < max_regions {
            regions.push(RegionCluster {
                indices: vec![idx],
                center,
            });
        }
    }
    regions.sort_by(|a, b| b.indices.len().cmp(&a.indices.len()));
    regions
}

fn estimate_blur_metric(gray: &[u8], width: usize, height: usize) -> f32 {
    if width < 3 || height < 3 || gray.len() != width * height {
        return 0.0;
    }
    let mut acc = 0.0f32;
    let mut count = 0usize;
    for y in (1..height - 1).step_by(2) {
        for x in (1..width - 1).step_by(2) {
            let idx = y * width + x;
            let c = gray[idx] as f32;
            let lap = (4.0 * c)
                - gray[idx - 1] as f32
                - gray[idx + 1] as f32
                - gray[idx - width] as f32
                - gray[idx + width] as f32;
            acc += lap.abs();
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        (acc / count as f32).clamp(0.0, 255.0)
    }
}

fn estimate_skew_deg(candidate: &RankedGroupCandidate) -> f32 {
    let dx = candidate.tr.x - candidate.tl.x;
    let dy = candidate.tr.y - candidate.tl.y;
    dy.atan2(dx).to_degrees().abs()
}

fn extract_fast_signals(
    gray: &[u8],
    width: usize,
    height: usize,
    candidates: &[RankedGroupCandidate],
) -> FastSignals {
    let saturation_ratio = global_saturation_ratio(gray);
    let blur_metric = estimate_blur_metric(gray, width, height);
    let skew_estimate_deg = candidates.first().map(estimate_skew_deg).unwrap_or(0.0);
    let megapixels = ((width * height) as f32 / 1_000_000.0).max(0.1);
    let region_density_proxy = (candidates.len() as f32 / megapixels).clamp(0.0, 10_000.0);
    FastSignals {
        blur_metric,
        saturation_ratio,
        skew_estimate_deg,
        region_density_proxy,
    }
}

fn select_strategy(candidates: &[RankedGroupCandidate], signals: FastSignals) -> StrategyProfile {
    if candidates.is_empty() {
        return StrategyProfile::FastSingle;
    }
    let high_conf = candidates
        .iter()
        .filter(|c| c.geometry_confidence >= 0.76)
        .count();
    let top_conf = candidates[0].geometry_confidence;
    let top_module = candidates[0].module_size;
    let spread = if candidates.len() >= 3 {
        (candidates[0].geometry_confidence - candidates[2].geometry_confidence).abs()
    } else {
        0.0
    };

    if signals.region_density_proxy >= 18.0 && candidates.len() >= 3 {
        return StrategyProfile::MultiQrHeavy;
    }
    if signals.skew_estimate_deg >= 16.0 {
        return StrategyProfile::RotationHeavy;
    }
    if signals.saturation_ratio >= 0.08 || signals.blur_metric < 14.0 {
        return StrategyProfile::LowContrastRecovery;
    }
    if high_conf >= 3 {
        return StrategyProfile::MultiQrHeavy;
    }
    if top_module <= 2.0 {
        return StrategyProfile::HighVersionPrecision;
    }
    if top_conf < 0.55 {
        return StrategyProfile::LowContrastRecovery;
    }
    if spread > 0.28 {
        return StrategyProfile::RotationHeavy;
    }
    StrategyProfile::FastSingle
}

fn confidence_lane(geometry_confidence: f32) -> ConfidenceLane {
    if geometry_confidence >= HIGH_CONFIDENCE_LANE_MIN {
        ConfidenceLane::High
    } else if geometry_confidence >= MEDIUM_CONFIDENCE_LANE_MIN {
        ConfidenceLane::Medium
    } else {
        ConfidenceLane::Low
    }
}

fn lane_budget_from_attempts(max_decode_attempts: usize, strategy: StrategyProfile) -> LaneBudget {
    if max_decode_attempts <= 1 {
        return LaneBudget {
            high: max_decode_attempts,
            medium: 0,
            low: 0,
        };
    }

    let mut high = ((max_decode_attempts as f32) * 0.5).floor() as usize;
    let mut medium = ((max_decode_attempts as f32) * 0.3).floor() as usize;
    let reserved = high + medium;
    let mut low = max_decode_attempts.saturating_sub(reserved);

    if high == 0 {
        high = 1;
        low = low.saturating_sub(1);
    }
    if max_decode_attempts >= 3 && medium == 0 {
        medium = 1;
        low = low.saturating_sub(1);
    }

    match strategy {
        StrategyProfile::MultiQrHeavy => {
            if high > 1 {
                high -= 1;
                low += 1;
            }
        }
        StrategyProfile::HighVersionPrecision => {
            if low > 0 {
                low -= 1;
                high += 1;
            } else if medium > 1 {
                medium -= 1;
                high += 1;
            }
        }
        StrategyProfile::LowContrastRecovery => {
            if medium > 0 {
                medium -= 1;
                low += 1;
            }
        }
        StrategyProfile::RotationHeavy => {
            if low > 0 {
                low -= 1;
                medium += 1;
            }
            if low > 0 {
                low -= 1;
                high += 1;
            }
        }
        StrategyProfile::FastSingle => {}
    }

    while high + medium + low > max_decode_attempts {
        if low > 0 {
            low -= 1;
        } else if medium > 0 {
            medium -= 1;
        } else {
            high = high.saturating_sub(1);
        }
    }
    while high + medium + low < max_decode_attempts {
        high += 1;
    }

    LaneBudget { high, medium, low }
}

fn record_lane_attempt(telemetry: &mut Option<&mut DetectionTelemetry>, lane: ConfidenceLane) {
    if let Some(tel) = telemetry.as_mut() {
        match lane {
            ConfidenceLane::High => tel.budget_lane_high += 1,
            ConfidenceLane::Medium => tel.budget_lane_medium += 1,
            ConfidenceLane::Low => tel.budget_lane_low += 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_ranked_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
    attempt_limit: Option<usize>,
    context: &mut DecodeRequestContext,
    mut telemetry: Option<&mut DetectionTelemetry>,
) -> Vec<QRCode> {
    let saturation_ratio = global_saturation_ratio(gray);
    let saturation_mask_enabled = saturation_ratio >= 0.06;
    let raw_groups = group_finder_patterns(finder_patterns);
    let (ranked, rerank_rejected) = rank_groups(
        binary,
        gray,
        width,
        height,
        saturation_mask_enabled,
        finder_patterns,
        raw_groups,
    );
    // Keep the complete bounded dense frontier.  `attempt_limit`, supplied by
    // the public request context, remains the hard decode/transform budget;
    // this only prevents an arbitrary pre-routing truncation.
    let consider_limit = if finder_patterns.len() > SPATIAL_GROUP_TRIGGER {
        DENSE_MAX_GROUP_CANDIDATES
    } else {
        MAX_GROUP_CANDIDATES
    };
    let consider = ranked.len().min(consider_limit);
    let pre_probe = &ranked[..consider];

    let candidates = pre_probe;

    if let Some(tel) = telemetry.as_mut() {
        tel.groups_found = candidates.len();
        tel.candidate_groups_scored = ranked.len();
        tel.decode_attempts = 0;
        tel.rerank_enabled = true;
        tel.rerank_transform_reject_count += rerank_rejected;
        tel.saturation_mask_enabled = saturation_mask_enabled;
        if saturation_mask_enabled {
            tel.saturation_mask_coverage = saturation_ratio;
        }
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
    let mut max_decode_attempts = DEFAULT_MAX_DECODE_ATTEMPTS;
    if let Some(limit) = attempt_limit {
        max_decode_attempts = max_decode_attempts.min(limit);
    }
    if max_decode_attempts == 0 {
        if let Some(tel) = telemetry.as_mut() {
            tel.budget_skips += 1;
        }
        return Vec::new();
    }
    let mut max_transforms = DEFAULT_MAX_TRANSFORMS.min(max_decode_attempts.max(1));
    let high_group_conf = HIGH_GROUP_CONFIDENCE;
    let low_top_group_conf = LOW_TOP_GROUP_CONFIDENCE;
    let single_qr_floor = SINGLE_QR_CONFIDENCE_FLOOR;
    let top = candidates[0];
    let fast_signals = extract_fast_signals(gray, width, height, candidates);
    let strategy = select_strategy(candidates, fast_signals);
    if matches!(strategy, StrategyProfile::MultiQrHeavy) {
        let base_regions = DEFAULT_MAX_REGIONS;
        let mut base_top_k = DEFAULT_PER_REGION_TOP_K;
        base_top_k = base_top_k.max(16);
        let scaled_budget = (base_regions * base_top_k * 2).min(512);
        // Multi-QR images require substantially larger attempt budgets.
        max_decode_attempts = max_decode_attempts.max(scaled_budget);
        // Keep transform and decode budgets aligned for dense scenes.
        max_transforms = max_transforms.max(max_decode_attempts).min(512);
    }
    // A caller-supplied limit is a request-wide safety boundary. Strategy
    // selection may redistribute that budget, but must never expand it.
    if let Some(limit) = attempt_limit {
        max_decode_attempts = max_decode_attempts.min(limit);
        max_transforms = max_transforms.min(max_decode_attempts);
    }
    if let Some(tel) = telemetry.as_mut() {
        tel.strategy_profile = strategy.as_str().to_string();
        tel.router_blur_metric = fast_signals.blur_metric;
        tel.router_saturation_ratio = fast_signals.saturation_ratio;
        tel.router_skew_estimate_deg = fast_signals.skew_estimate_deg;
        tel.router_region_density_proxy = fast_signals.region_density_proxy;
    }
    let mut lane_budget = lane_budget_from_attempts(max_decode_attempts, strategy);
    let heavy_recovery_top_n = 2;
    let mut should_expand = candidates
        .iter()
        .filter(|c| c.geometry_confidence >= high_group_conf)
        .take(2)
        .count()
        >= 2
        || top.geometry_confidence < low_top_group_conf;
    if matches!(strategy, StrategyProfile::MultiQrHeavy) {
        should_expand = true;
    }

    let mut used_transforms = 0usize;
    let mut used_attempts = 0usize;
    let mut results = Vec::new();
    let mut accepted_geometries: Vec<(f32, f32, f32, f32)> = Vec::new();
    let mut accepted_proposals: HashSet<usize> = HashSet::new();

    let first = top;
    if used_transforms < max_transforms && used_attempts < max_decode_attempts {
        if let Some(tel) = telemetry.as_mut() {
            tel.rerank_top1_attempts += 1;
        }
        let lane = confidence_lane(first.geometry_confidence);
        if !lane_budget.consume(lane) {
            if let Some(tel) = telemetry.as_mut() {
                tel.budget_skips += 1;
            }
            return results;
        }
        record_lane_attempt(&mut telemetry, lane);
        if let Some(tel) = telemetry.as_mut() {
            tel.transforms_built += 1;
            tel.decode_attempts += 1;
        }
        used_transforms += 1;
        used_attempts += 1;
        let allow_heavy = used_attempts <= heavy_recovery_top_n;
        let allow_matrix_recovery = matrix_recovery_allowed(strategy, allow_heavy);
        if let Some(qr) = decode_candidate(
            &first,
            binary,
            gray,
            width,
            height,
            allow_heavy,
            allow_matrix_recovery,
            fast_signals.blur_metric,
            context,
        ) {
            let acceptance = acceptance_score(&qr, first.geometry_confidence);
            let floor = DECODE_ACCEPTANCE_FLOOR;
            if acceptance >= floor {
                if let Some(tel) = telemetry.as_mut() {
                    tel.rs_decode_ok += 1;
                    tel.payload_decoded += 1;
                }
                if qr.confidence < single_qr_floor {
                    should_expand = true;
                }
                if dedupe_results(
                    &mut results,
                    &mut accepted_geometries,
                    &mut accepted_proposals,
                    &first,
                    qr,
                ) {
                    if let Some(tel) = telemetry.as_mut() {
                        tel.rerank_top1_successes += 1;
                        if saturation_mask_enabled && first.saturation_coverage > 0.08 {
                            tel.saturation_mask_decode_successes += 1;
                        }
                    }
                }
                if !should_expand && !matches!(strategy, StrategyProfile::MultiQrHeavy) {
                    return results;
                }
            } else if let Some(tel) = telemetry.as_mut() {
                tel.acceptance_rejected += 1;
            }
        }
    } else {
        if let Some(tel) = telemetry.as_mut() {
            tel.budget_skips += 1;
        }
        return results;
    }

    if !should_expand && !matches!(strategy, StrategyProfile::MultiQrHeavy) {
        return results;
    }

    let max_regions = region_limit_for_strategy(strategy, candidates.len());
    let mut per_region_top_k = DEFAULT_PER_REGION_TOP_K;
    let mut per_region_attempt_cap = 3;
    match strategy {
        StrategyProfile::MultiQrHeavy => {
            per_region_top_k = per_region_top_k.max(16);
            per_region_attempt_cap = per_region_attempt_cap.max(48);
        }
        StrategyProfile::HighVersionPrecision => {
            per_region_attempt_cap = per_region_attempt_cap.min(2);
        }
        StrategyProfile::LowContrastRecovery => {
            per_region_top_k = per_region_top_k.min(3);
        }
        StrategyProfile::RotationHeavy | StrategyProfile::FastSingle => {}
    }
    per_region_top_k = per_region_top_k.min(top_k);

    let regions = cluster_regions(candidates, max_regions);
    let multi_region = regions.len() > 1;
    if let Some(tel) = telemetry.as_mut() {
        tel.router_multi_region = multi_region;
        tel.regions_considered = regions.len();
    }

    if matches!(strategy, StrategyProfile::MultiQrHeavy) && regions.len() <= 1 {
        let remaining_attempts = max_decode_attempts.saturating_sub(used_attempts);
        per_region_top_k = per_region_top_k.max(remaining_attempts.min(64));
        per_region_attempt_cap = per_region_attempt_cap.max(remaining_attempts.min(128));
    }

    let relaxed_floor = DECODE_RELAXED_ACCEPTANCE_FLOOR;
    // Only a highly diverse dense frontier needs a breadth-first schedule.
    // Below that bound, preserve the established region-local ordering. Many
    // disjoint regions can each contain near-duplicate triples; exhausting one
    // cluster first can starve a separate symbol even though both were kept.
    let attempts_per_region = per_region_top_k.min(per_region_attempt_cap);
    let breadth_first_dense_schedule =
        matches!(strategy, StrategyProfile::MultiQrHeavy) && regions.len() >= 40;
    let mut schedule = Vec::with_capacity(regions.len() * attempts_per_region);
    if breadth_first_dense_schedule {
        for region_attempts in 0..attempts_per_region {
            for region_index in 0..regions.len() {
                schedule.push((region_attempts, region_index));
            }
        }
    } else {
        for region_index in 0..regions.len() {
            for region_attempts in 0..attempts_per_region {
                schedule.push((region_attempts, region_index));
            }
        }
    }
    for (region_attempts, region_index) in schedule {
        let region = &regions[region_index];
        if used_transforms >= max_transforms || used_attempts >= max_decode_attempts {
            if let Some(tel) = telemetry.as_mut() {
                tel.budget_skips += 1;
            }
            break;
        }
        let Some(&idx) = region.indices.get(region_attempts) else {
            continue;
        };
        let candidate = &candidates[idx];
        if candidate
            .group
            .iter()
            .any(|proposal| accepted_proposals.contains(proposal))
        {
            continue;
        }
        let lane = confidence_lane(candidate.geometry_confidence);
        if !lane_budget.consume(lane) {
            if let Some(tel) = telemetry.as_mut() {
                tel.budget_skips += 1;
            }
            continue;
        }
        record_lane_attempt(&mut telemetry, lane);
        if let Some(tel) = telemetry.as_mut() {
            tel.transforms_built += 1;
            tel.decode_attempts += 1;
        }
        used_transforms += 1;
        used_attempts += 1;

        let allow_heavy = used_attempts <= heavy_recovery_top_n;
        let allow_matrix_recovery = matrix_recovery_allowed(strategy, allow_heavy);
        if let Some(qr) = decode_candidate(
            candidate,
            binary,
            gray,
            width,
            height,
            allow_heavy,
            allow_matrix_recovery,
            fast_signals.blur_metric,
            context,
        ) {
            let acceptance = acceptance_score(&qr, candidate.geometry_confidence);
            if acceptance < relaxed_floor {
                if let Some(tel) = telemetry.as_mut() {
                    tel.acceptance_rejected += 1;
                }
                continue;
            }
            if dedupe_results(
                &mut results,
                &mut accepted_geometries,
                &mut accepted_proposals,
                candidate,
                qr,
            ) {
                if let Some(tel) = telemetry.as_mut() {
                    tel.rs_decode_ok += 1;
                    tel.payload_decoded += 1;
                    tel.router_region_decodes += 1;
                    if saturation_mask_enabled && candidate.saturation_coverage > 0.08 {
                        tel.saturation_mask_decode_successes += 1;
                    }
                }
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
    let mut context = DecodeRequestContext::default();
    decode_ranked_groups(
        binary,
        gray,
        width,
        height,
        finder_patterns,
        None,
        &mut context,
        None,
    )
}

/// Like `decode_groups_with_telemetry` but enforces a hard decode-attempt cap.
pub(crate) fn decode_groups_with_telemetry_limited(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
    max_attempts: usize,
) -> (Vec<QRCode>, DetectionTelemetry) {
    let mut context = DecodeRequestContext::default();
    decode_groups_with_telemetry_limited_in_context(
        binary,
        gray,
        width,
        height,
        finder_patterns,
        max_attempts,
        &mut context,
    )
}

/// Like [`decode_groups_with_telemetry_limited`] but charges all candidate
/// recovery work to a caller-owned request context.
pub(crate) fn decode_groups_with_telemetry_limited_in_context(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
    max_attempts: usize,
    context: &mut DecodeRequestContext,
) -> (Vec<QRCode>, DetectionTelemetry) {
    let mut tel = DetectionTelemetry::default();
    let results = decode_ranked_groups(
        binary,
        gray,
        width,
        height,
        finder_patterns,
        Some(max_attempts),
        context,
        Some(&mut tel),
    );
    (results, tel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MaskPattern, Version};

    fn candidate(group: [usize; 3], origin_x: f32, origin_y: f32) -> RankedGroupCandidate {
        RankedGroupCandidate {
            group,
            tl: Point::new(origin_x, origin_y),
            tr: Point::new(origin_x + 40.0, origin_y),
            bl: Point::new(origin_x, origin_y + 40.0),
            module_size: 2.0,
            raw_score: 1.0,
            rerank_score: 1.0,
            saturation_coverage: 0.0,
            geometry_confidence: 0.9,
        }
    }

    #[test]
    fn dense_matrix_recovery_respects_the_existing_heavy_quota() {
        assert!(matrix_recovery_allowed(StrategyProfile::MultiQrHeavy, true));
        assert!(!matrix_recovery_allowed(
            StrategyProfile::MultiQrHeavy,
            false
        ));
        assert!(matrix_recovery_allowed(
            StrategyProfile::LowContrastRecovery,
            false
        ));
        assert!(matrix_recovery_allowed(StrategyProfile::FastSingle, false));
    }

    fn decoded(content: &str) -> QRCode {
        QRCode::new(
            content.as_bytes().to_vec(),
            content.to_owned(),
            Version::Model2(1),
            ECLevel::M,
            MaskPattern::Pattern0,
        )
    }

    #[test]
    fn accepted_symbols_keep_a_one_to_one_finder_assignment() {
        let mut results = Vec::new();
        let mut geometries = Vec::new();
        let mut proposals = HashSet::new();

        assert!(dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([0, 1, 2], 0.0, 0.0),
            decoded("first"),
        ));
        // A different-looking triple cannot claim a finder already assigned to
        // an accepted symbol, even if its bounding box does not overlap.
        assert!(!dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([0, 3, 4], 120.0, 0.0),
            decoded("spurious"),
        ));
        assert!(dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([3, 4, 5], 120.0, 0.0),
            decoded("second"),
        ));
        assert_eq!(results.len(), 2);
        assert_eq!(proposals.len(), 6);
    }

    #[test]
    fn result_dedupe_preserves_distinct_symbols_with_equal_payloads() {
        let mut results = Vec::new();
        let mut geometries = Vec::new();
        let mut proposals = HashSet::new();

        assert!(dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([0, 1, 2], 0.0, 0.0),
            decoded("same"),
        ));
        assert!(dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([3, 4, 5], 120.0, 0.0),
            decoded("same"),
        ));
        // Re-decoding the same region through another detector path is still
        // a duplicate even when it happens to use different proposal IDs.
        assert!(!dedupe_results(
            &mut results,
            &mut geometries,
            &mut proposals,
            &candidate([6, 7, 8], 3.0, 2.0),
            decoded("same"),
        ));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn grouping_is_scale_invariant_beyond_legacy_pixel_limit() {
        // These finder centres form a valid, large Model 2 symbol.  Their
        // diagonal is intentionally wider than the old 3,000-pixel global
        // cutoff; grouping must depend on pitch, not raster size.
        let patterns = vec![
            FinderPattern::new(1_000.0, 1_000.0, 30.0),
            FinderPattern::new(5_000.0, 1_000.0, 30.0),
            FinderPattern::new(1_000.0, 5_000.0, 30.0),
        ];
        assert_eq!(group_finder_patterns(&patterns), vec![[0, 1, 2]]);
    }

    #[test]
    fn grouping_rejects_module_span_beyond_model_two_limit() {
        let patterns = vec![
            FinderPattern::new(1_000.0, 1_000.0, 30.0),
            FinderPattern::new(11_000.0, 1_000.0, 30.0),
            FinderPattern::new(1_000.0, 11_000.0, 30.0),
        ];
        assert!(group_finder_patterns(&patterns).is_empty());
    }

    #[test]
    fn spatial_grouping_preserves_independent_dense_symbols() {
        // 25 nearby symbols exercise the region-aware path.  The expected
        // triples are deliberately interleaved in one module-size bucket, so
        // this catches a regression back to first-visited global grouping.
        let mut patterns = Vec::new();
        for symbol in 0..25usize {
            let x = 20.0 + (symbol % 5) as f32 * 110.0;
            let y = 20.0 + (symbol / 5) as f32 * 110.0;
            patterns.extend([
                FinderPattern::new(x, y, 2.0),
                FinderPattern::new(x + 42.0, y, 2.0),
                FinderPattern::new(x, y + 42.0, 2.0),
            ]);
        }

        let groups = group_finder_patterns(&patterns);
        let found = groups
            .iter()
            .map(|group| {
                let mut key = [group[0], group[1], group[2]];
                key.sort_unstable();
                key
            })
            .collect::<HashSet<_>>();
        for symbol in 0..25usize {
            assert!(
                found.contains(&[symbol * 3, symbol * 3 + 1, symbol * 3 + 2]),
                "symbol {symbol} must retain its own local finder triple"
            );
        }
    }

    #[test]
    fn spatial_grouping_retains_tight_raster_grid_triples() {
        // This mirrors the controlled raster corpus: a symbol's finders are
        // 70 pixels apart, while the next symbol starts 87 pixels beyond the
        // bottom-left finder. The gap is small enough for broad neighbourhood
        // grouping to manufacture cross-symbol triangles.
        let mut patterns = Vec::new();
        for symbol in 0..50usize {
            let x = 53.5 + (symbol % 5) as f32 * 157.0;
            let y = 53.5 + (symbol / 5) as f32 * 157.0;
            patterns.extend([
                FinderPattern::new(x, y, 5.0),
                FinderPattern::new(x + 70.0, y, 5.0),
                FinderPattern::new(x, y + 70.0, 5.0),
            ]);
        }

        let groups = group_finder_patterns(&patterns);
        let found = groups
            .iter()
            .map(|group| {
                let mut key = [group[0], group[1], group[2]];
                key.sort_unstable();
                key
            })
            .collect::<HashSet<_>>();
        for symbol in 0..50usize {
            assert!(
                found.contains(&[symbol * 3, symbol * 3 + 1, symbol * 3 + 2]),
                "tight raster symbol {symbol} must retain its own finder triple"
            );
        }
    }

    #[test]
    fn dense_strategy_visits_each_retained_disjoint_region() {
        // A 50-symbol controlled scene has one local region per retained
        // triple.  Do not let the historical 32-region router cap turn the
        // final 18 valid triples into sampling misses.
        assert_eq!(
            region_limit_for_strategy(StrategyProfile::MultiQrHeavy, 50),
            50
        );
        assert_eq!(
            region_limit_for_strategy(StrategyProfile::MultiQrHeavy, 500),
            DENSE_MAX_GROUP_CANDIDATES
        );
        assert_eq!(
            region_limit_for_strategy(StrategyProfile::FastSingle, 50),
            DEFAULT_MAX_REGIONS
        );
    }
}
