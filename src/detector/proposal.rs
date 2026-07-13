//! Finder-proposal scoring and non-maximum suppression.
//!
//! This module deliberately stops before QR geometry.  A proposal is evidence
//! for *one* finder marker, not an asserted three-finder QR symbol.  Keeping
//! that boundary explicit lets callers measure scan and proposal recall before
//! transform/sampling failures are mixed into the result.

use crate::models::{BitMatrix, Point};

use super::finder::FinderPattern;

/// Quality evidence attached to a finder proposal.  Each component is in the
/// inclusive `0.0..=1.0` range; the final score is a deterministic weighted
/// combination rather than a decode-derived confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinderProposalEvidence {
    /// Agreement with the 1:1:3:1:1 run ratio on the horizontal cross-check.
    pub horizontal_ratio: f32,
    /// Agreement with the 1:1:3:1:1 run ratio on the vertical cross-check.
    pub vertical_ratio: f32,
    /// Agreement between independently measured horizontal and vertical pitch.
    pub pitch_agreement: f32,
    /// Local black/white contrast around the proposed finder centre.
    pub local_contrast: f32,
    /// Fraction of probes immediately outside the finder that are white.
    /// This is intentionally a weak ranking signal: QR data legitimately sits
    /// beside the interior two sides of a corner finder.
    pub quiet_zone: f32,
}

/// A ranked candidate for a single finder marker.
#[derive(Debug, Clone)]
pub struct FinderProposal {
    pub pattern: FinderPattern,
    pub score: f32,
    pub evidence: FinderProposalEvidence,
}

impl FinderProposal {
    pub(crate) fn from_pattern(matrix: &BitMatrix, pattern: FinderPattern) -> Self {
        let (horizontal_ratio, horizontal_pitch) = axis_evidence(matrix, &pattern.center, true);
        let (vertical_ratio, vertical_pitch) = axis_evidence(matrix, &pattern.center, false);
        let pitch_agreement = match (horizontal_pitch, vertical_pitch) {
            (Some(a), Some(b)) if a > 0.0 && b > 0.0 => {
                (1.0 - (a - b).abs() / a.max(b)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        let evidence = FinderProposalEvidence {
            horizontal_ratio,
            vertical_ratio,
            pitch_agreement,
            local_contrast: local_contrast(matrix, &pattern.center, pattern.module_size),
            quiet_zone: quiet_zone_score(matrix, &pattern.center, pattern.module_size),
        };
        let score = (0.30 * evidence.horizontal_ratio
            + 0.30 * evidence.vertical_ratio
            + 0.20 * evidence.pitch_agreement
            + 0.15 * evidence.local_contrast
            + 0.05 * evidence.quiet_zone)
            .clamp(0.0, 1.0);
        Self {
            pattern,
            score,
            evidence,
        }
    }
}

/// Rank raw scanline candidates and suppress only duplicate observations of
/// the same finder.  The suppression radius scales with estimated pitch, so it
/// does not impose a scene-wide pixel-distance constant and cannot collapse
/// distinct QR symbols merely because an image is small.
pub(crate) fn rank_and_suppress(
    matrix: &BitMatrix,
    raw: Vec<FinderPattern>,
) -> Vec<FinderProposal> {
    let mut proposals = raw
        .into_iter()
        .filter(|pattern| pattern.module_size.is_finite() && pattern.module_size >= 1.0)
        .map(|pattern| FinderProposal::from_pattern(matrix, pattern))
        .collect::<Vec<_>>();
    proposals.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.pattern.center.x.total_cmp(&b.pattern.center.x))
            .then_with(|| a.pattern.center.y.total_cmp(&b.pattern.center.y))
    });

    let mut kept: Vec<FinderProposal> = Vec::with_capacity(proposals.len());
    'proposal: for proposal in proposals {
        for accepted in &kept {
            let radius = 2.5
                * proposal
                    .pattern
                    .module_size
                    .max(accepted.pattern.module_size);
            if proposal.pattern.center.distance(&accepted.pattern.center) <= radius {
                continue 'proposal;
            }
        }
        kept.push(proposal);
    }
    kept
}

fn axis_evidence(matrix: &BitMatrix, center: &Point, horizontal: bool) -> (f32, Option<f32>) {
    let x = center.x.round() as isize;
    let y = center.y.round() as isize;
    if x < 0 || y < 0 || x as usize >= matrix.width() || y as usize >= matrix.height() {
        return (0.0, None);
    }
    if !matrix.get(x as usize, y as usize) {
        return (0.0, None);
    }
    let (dx, dy) = if horizontal { (1, 0) } else { (0, 1) };
    let mut runs = [0usize; 5];
    let mut px = x;
    let mut py = y;
    while in_bounds(matrix, px, py) && matrix.get(px as usize, py as usize) {
        runs[2] += 1;
        px -= dx;
        py -= dy;
    }
    while in_bounds(matrix, px, py) && !matrix.get(px as usize, py as usize) {
        runs[1] += 1;
        px -= dx;
        py -= dy;
    }
    while in_bounds(matrix, px, py) && matrix.get(px as usize, py as usize) {
        runs[0] += 1;
        px -= dx;
        py -= dy;
    }
    px = x + dx;
    py = y + dy;
    while in_bounds(matrix, px, py) && matrix.get(px as usize, py as usize) {
        runs[2] += 1;
        px += dx;
        py += dy;
    }
    while in_bounds(matrix, px, py) && !matrix.get(px as usize, py as usize) {
        runs[3] += 1;
        px += dx;
        py += dy;
    }
    while in_bounds(matrix, px, py) && matrix.get(px as usize, py as usize) {
        runs[4] += 1;
        px += dx;
        py += dy;
    }
    if runs.iter().any(|run| *run == 0) {
        return (0.0, None);
    }
    let total = runs.iter().sum::<usize>() as f32;
    let pitch = total / 7.0;
    if pitch <= 0.0 {
        return (0.0, None);
    }
    let expected = [1.0, 1.0, 3.0, 1.0, 1.0];
    let error = runs
        .iter()
        .zip(expected)
        .map(|(&run, expected)| ((run as f32 / pitch) - expected).abs() / expected)
        .sum::<f32>()
        / 5.0;
    ((1.0 - error).clamp(0.0, 1.0), Some(pitch))
}

fn local_contrast(matrix: &BitMatrix, center: &Point, module_size: f32) -> f32 {
    let radius = (module_size * 3.5).ceil().max(2.0) as isize;
    let x = center.x.round() as isize;
    let y = center.y.round() as isize;
    let mut black = 0usize;
    let mut total = 0usize;
    for py in (y - radius)..=(y + radius) {
        for px in (x - radius)..=(x + radius) {
            if in_bounds(matrix, px, py) {
                black += usize::from(matrix.get(px as usize, py as usize));
                total += 1;
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    let fraction = black as f32 / total as f32;
    (1.0 - (fraction - 0.5).abs() * 2.0).clamp(0.0, 1.0)
}

fn quiet_zone_score(matrix: &BitMatrix, center: &Point, module_size: f32) -> f32 {
    let offset = (module_size * 4.25).round().max(1.0) as isize;
    let x = center.x.round() as isize;
    let y = center.y.round() as isize;
    let probes = [
        (x - offset, y),
        (x + offset, y),
        (x, y - offset),
        (x, y + offset),
    ];
    let mut white = 0usize;
    let mut total = 0usize;
    for (px, py) in probes {
        if in_bounds(matrix, px, py) {
            white += usize::from(!matrix.get(px as usize, py as usize));
            total += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        white as f32 / total as f32
    }
}

fn in_bounds(matrix: &BitMatrix, x: isize, y: isize) -> bool {
    x >= 0 && y >= 0 && (x as usize) < matrix.width() && (y as usize) < matrix.height()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nms_merges_duplicate_scans_but_preserves_distinct_markers() {
        let matrix = BitMatrix::new(80, 30);
        let raw = vec![
            FinderPattern::new(10.0, 10.0, 3.0),
            FinderPattern::new(12.0, 10.0, 3.0),
            FinderPattern::new(45.0, 10.0, 3.0),
        ];
        let proposals = rank_and_suppress(&matrix, raw);
        assert_eq!(proposals.len(), 2);
        assert!(proposals[0].score.is_finite());
    }

    #[test]
    fn controlled_dense_scenes_preserve_each_symbols_finders() {
        // These are deliberately detector-stage scenes: each synthetic symbol
        // contributes the three finder proposals that its QR geometry would
        // expose. Keeping them module-scaled and close together exercises NMS
        // without coupling this proposal contract to sampling/decoding.
        let matrix = BitMatrix::new(704, 704);
        for symbols in [1usize, 2, 5, 10, 25, 50, 100] {
            let raw = (0..symbols)
                .flat_map(|index| {
                    let x = 16.0 + (index % 10) as f32 * 64.0;
                    let y = 16.0 + (index / 10) as f32 * 64.0;
                    [
                        FinderPattern::new(x, y, 2.0),
                        FinderPattern::new(x + 36.0, y, 2.0),
                        FinderPattern::new(x, y + 36.0, 2.0),
                    ]
                })
                .collect();
            let proposals = rank_and_suppress(&matrix, raw);
            assert_eq!(
                proposals.len(),
                symbols * 3,
                "NMS must preserve all three finders in a {symbols}-symbol scene"
            );
        }
    }
}
