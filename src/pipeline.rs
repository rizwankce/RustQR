use crate::DetectionTelemetry;
use crate::decoder::qr_decoder::QrDecoder;
use crate::detector::finder::FinderPattern;
use crate::models::{BitMatrix, Point, QRCode};

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

/// Simplified finder pattern grouping with relaxed constraints
pub(crate) fn group_finder_patterns(patterns: &[FinderPattern]) -> Vec<Vec<usize>> {
    if patterns.len() < 3 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f32)> = patterns
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.module_size))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

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
        let groups = build_groups(patterns, &indices);
        if !groups.is_empty() {
            all_groups.extend(groups);
        }
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
                if min_d < avg_module * 2.5 {
                    continue;
                }
                if max_d > 3000.0 {
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

fn score_and_trim_groups(
    groups: &mut Vec<Vec<usize>>,
    patterns: &[FinderPattern],
    max_groups: usize,
) {
    if groups.len() <= max_groups {
        return;
    }

    groups.sort_by(|a, b| {
        let sa = group_score(patterns, a);
        let sb = group_score(patterns, b);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
    });
    groups.truncate(max_groups);
}

fn group_score(patterns: &[FinderPattern], group: &[usize]) -> f32 {
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

    // Prefer near-right angle (small cosine) and size consistency
    let a2 = d01 * d01;
    let b2 = d02 * d02;
    let c2 = d12 * d12;
    let cos_i = ((a2 + b2 - c2) / (2.0 * d01 * d02)).abs();
    let cos_j = ((a2 + c2 - b2) / (2.0 * d01 * d12)).abs();
    let cos_k = ((b2 + c2 - a2) / (2.0 * d02 * d12)).abs();
    let best_cos = cos_i.min(cos_j).min(cos_k);

    size_ratio * 2.0 + distortion + best_cos
}

pub(crate) fn decode_groups(
    binary: &BitMatrix,
    gray: &[u8],
    width: usize,
    height: usize,
    finder_patterns: &[FinderPattern],
) -> Vec<QRCode> {
    let mut results = Vec::new();
    let mut groups = group_finder_patterns(finder_patterns);
    score_and_trim_groups(&mut groups, finder_patterns, 40);

    if cfg!(debug_assertions) && crate::debug::debug_enabled() {
        eprintln!(
            "DEBUG: Found {} finder patterns, formed {} groups",
            finder_patterns.len(),
            groups.len()
        );
    }

    for (group_idx, group) in groups.iter().enumerate() {
        if group.len() < 3 {
            continue;
        }
        if cfg!(debug_assertions) && crate::debug::debug_enabled() {
            eprintln!(
                "DEBUG: Trying group {} with patterns {:?}",
                group_idx, group
            );
        }

        if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
            &finder_patterns[group[0]],
            &finder_patterns[group[1]],
            &finder_patterns[group[2]],
        ) {
            if let Some(qr) =
                QrDecoder::decode_with_gray(binary, gray, width, height, &tl, &tr, &bl, module_size)
            {
                if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                    eprintln!("DEBUG: Group {} decoded successfully!", group_idx);
                }
                results.push(qr);
            } else if cfg!(debug_assertions) && crate::debug::debug_enabled() {
                eprintln!("DEBUG: Group {} failed to decode", group_idx);
            }
        }
    }

    results
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
    let mut results = Vec::new();
    let mut groups = group_finder_patterns(finder_patterns);
    score_and_trim_groups(&mut groups, finder_patterns, 40);
    tel.groups_found = groups.len();

    for group in &groups {
        if group.len() < 3 {
            continue;
        }

        if let Some((tl, tr, bl, module_size)) = order_finder_patterns(
            &finder_patterns[group[0]],
            &finder_patterns[group[1]],
            &finder_patterns[group[2]],
        ) {
            tel.transforms_built += 1;
            if let Some(qr) =
                QrDecoder::decode_with_gray(binary, gray, width, height, &tl, &tr, &bl, module_size)
            {
                tel.rs_decode_ok += 1;
                tel.payload_decoded += 1;
                results.push(qr);
            }
        }
    }

    (results, tel)
}
