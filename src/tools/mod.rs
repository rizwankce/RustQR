#![allow(clippy::items_after_test_module)]

use crate::detector::finder::{FinderDetector, FinderScanTelemetry};
use crate::detector::proposal::FinderProposalEvidence;
use crate::models::BitMatrix;
use crate::pipeline::{decode_groups, group_finder_patterns};
use crate::utils::binarization::{adaptive_binarize, otsu_binarize};
use crate::utils::grayscale::rgb_to_grayscale;
use crate::{QRCode, detect_with_telemetry_timeout};
use image::GenericImageView;
use std::env;
use std::fs;
use std::hash::Hasher;
use std::path::{Path, PathBuf};

/// A quadrilateral annotation or prediction in image coordinates.
pub type Quadrilateral = [[f32; 2]; 4];

/// Strict result of parsing a BoofCV point-annotation file.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizationLabels {
    pub quadrilaterals: Vec<Quadrilateral>,
}

/// Localization counts produced by one-to-one matching.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalizationScore {
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
    pub duplicate_predictions: usize,
}

/// Exact payload-match counts, kept separate from localization scoring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PayloadScore {
    pub exact_matches: usize,
    pub expected: usize,
    pub predicted: usize,
}

/// Stage-level result for one labeled image.  This deliberately measures
/// single-finder proposals and three-finder groups before transform/sampling
/// and payload decoding can hide a localization failure.
#[derive(Debug, Clone)]
pub struct FinderGroupingEvaluation {
    /// Number of annotated QR symbols in the image.
    pub expected_symbols: usize,
    /// Evidence aligned by index with the retained post-NMS proposal centres
    /// used for grouping. This is diagnostic-only; it does not affect ranking.
    pub proposal_evidence: Vec<FinderProposalEvidence>,
    pub contained_evidence_buckets: [usize; 4],
    pub spurious_evidence_buckets: [usize; 4],
    /// Symbols containing at least three retained finder proposals.
    pub finder_hits: usize,
    /// Annotated symbols by the number of retained proposal centres they
    /// contain: zero, one, two, and three-or-more respectively. The final
    /// bucket is exactly the finder-stage-eligible population, which keeps
    /// proposal loss distinct from a later grouping loss.
    pub proposal_multiplicity: [usize; 4],
    /// Symbols containing at least one geometrically valid finder group.
    pub grouping_hits: usize,
    /// Finder-stage-eligible symbols for which no contained group was emitted.
    pub finder_eligible_without_group: usize,
    /// Proposals whose centres are outside every annotated symbol.
    pub spurious_proposals: usize,
    /// Retained proposal centres inside at least one annotated symbol.
    pub contained_proposals: usize,
    /// Groups whose three centres are not contained by one annotated symbol.
    pub spurious_groups: usize,
    /// Groups whose three centres are contained by an annotated symbol.
    pub contained_groups: usize,
    /// Contained groups beyond the first one assigned to each symbol.
    ///
    /// This is separate from `grouping_hits`: many valid-looking groups for
    /// one label improve neither symbol recall nor downstream work.
    pub duplicate_contained_groups: usize,
    /// Scan-stage counters, recorded alongside latency for diagnosis.
    pub scan_telemetry: FinderScanTelemetry,
    /// Time spent in the scan/rank/NMS proposal stage.
    pub proposal_latency_ms: f64,
    /// Time spent grouping the retained proposal patterns.
    pub grouping_latency_ms: f64,
}

/// Comparable observations from the normal brightness-aware route and one
/// direct strict-Otsu route. This is a diagnostic surface only: it does not
/// merge results or alter production route selection.
#[derive(Debug, Clone)]
pub struct DenseRouteAudit {
    pub brightness_codes: Vec<QRCode>,
    pub brightness_elapsed_ms: f64,
    pub otsu_codes: Vec<QRCode>,
    pub otsu_elapsed_ms: f64,
    pub otsu_binarize_ms: f64,
    pub otsu_finder_ms: f64,
    pub otsu_group_ms: f64,
    pub otsu_decode_ms: f64,
    pub otsu_finder_patterns: usize,
    pub otsu_group_candidates: usize,
}

/// Audit dense route divergence from the same RGB input using fresh work.
pub fn audit_brightness_vs_otsu(
    rgb: &[u8],
    width: usize,
    height: usize,
    timeout: std::time::Duration,
) -> DenseRouteAudit {
    let brightness_start = std::time::Instant::now();
    let brightness_codes = detect_with_telemetry_timeout(rgb, width, height, timeout).0;
    let brightness_elapsed_ms = brightness_start.elapsed().as_secs_f64() * 1_000.0;

    let gray = rgb_to_grayscale(rgb, width, height);
    let otsu_start = std::time::Instant::now();
    let otsu = otsu_binarize(&gray, width, height);
    let otsu_binarize_ms = otsu_start.elapsed().as_secs_f64() * 1_000.0;
    let finder_start = std::time::Instant::now();
    let finder_patterns = FinderDetector::detect(&otsu);
    let otsu_finder_ms = finder_start.elapsed().as_secs_f64() * 1_000.0;
    let group_start = std::time::Instant::now();
    let otsu_group_candidates = group_finder_patterns(&finder_patterns).len();
    let otsu_group_ms = group_start.elapsed().as_secs_f64() * 1_000.0;
    let decode_start = std::time::Instant::now();
    let otsu_codes = decode_groups(&otsu, &gray, width, height, &finder_patterns);
    let otsu_decode_ms = decode_start.elapsed().as_secs_f64() * 1_000.0;
    let otsu_elapsed_ms = otsu_start.elapsed().as_secs_f64() * 1_000.0;

    DenseRouteAudit {
        brightness_codes,
        brightness_elapsed_ms,
        otsu_codes,
        otsu_elapsed_ms,
        otsu_binarize_ms,
        otsu_finder_ms,
        otsu_group_ms,
        otsu_decode_ms,
        otsu_finder_patterns: finder_patterns.len(),
        otsu_group_candidates,
    }
}

impl FinderGroupingEvaluation {
    pub fn finder_recall(&self) -> f64 {
        ratio(self.finder_hits, self.expected_symbols)
    }

    pub fn grouping_recall(&self) -> f64 {
        ratio(self.grouping_hits, self.expected_symbols)
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Measure finder-proposal and grouping recall against labeled symbol bounds.
///
/// A symbol is a finder-stage hit when at least three retained proposal
/// centres lie within its annotated quadrilateral.  It is a grouping hit when
/// one output group has all three centres within that same quadrilateral.
/// This is intentionally a conservative stage contract: it makes no claim
/// about transform quality or successful payload decoding.
pub fn evaluate_finder_and_grouping(
    matrix: &BitMatrix,
    expected: &[Quadrilateral],
) -> FinderGroupingEvaluation {
    let proposal_start = std::time::Instant::now();
    let report = FinderDetector::detect_proposals(matrix);
    let proposal_latency_ms = proposal_start.elapsed().as_secs_f64() * 1_000.0;

    let patterns: Vec<_> = report
        .proposals
        .iter()
        .map(|proposal| proposal.pattern.clone())
        .collect();
    let proposal_evidence: Vec<FinderProposalEvidence> = report
        .proposals
        .iter()
        .map(|proposal| proposal.evidence)
        .collect();
    let grouping_start = std::time::Instant::now();
    let groups = group_finder_patterns(&patterns);
    let grouping_latency_ms = grouping_start.elapsed().as_secs_f64() * 1_000.0;

    let proposal_in_symbol = |proposal_index: usize, symbol: &Quadrilateral| {
        let center = &patterns[proposal_index].center;
        point_in_quadrilateral([center.x, center.y], symbol)
    };
    let proposal_counts: Vec<_> = expected
        .iter()
        .map(|symbol| {
            patterns
                .iter()
                .enumerate()
                .filter(|(index, _)| proposal_in_symbol(*index, symbol))
                .count()
        })
        .collect();
    let mut proposal_multiplicity = [0usize; 4];
    for count in &proposal_counts {
        proposal_multiplicity[(*count).min(3)] += 1;
    }
    let finder_hits = proposal_multiplicity[3];
    let mut grouped_symbols = vec![false; expected.len()];
    let mut contained_groups = 0;
    let mut duplicate_contained_groups = 0;
    for group in &groups {
        let Some(symbol_index) = expected.iter().position(|symbol| {
            group.len() == 3 && group.iter().all(|index| proposal_in_symbol(*index, symbol))
        }) else {
            continue;
        };
        contained_groups += 1;
        if std::mem::replace(&mut grouped_symbols[symbol_index], true) {
            duplicate_contained_groups += 1;
        }
    }
    let grouping_hits = grouped_symbols.into_iter().filter(|hit| *hit).count();
    let spurious_proposals = patterns
        .iter()
        .filter(|pattern| {
            !expected
                .iter()
                .any(|symbol| point_in_quadrilateral([pattern.center.x, pattern.center.y], symbol))
        })
        .count();
    let mut contained_evidence_buckets = [0; 4];
    let mut spurious_evidence_buckets = [0; 4];
    for (pattern, evidence) in patterns.iter().zip(&proposal_evidence) {
        let score = (0.30_f32 * evidence.horizontal_ratio
            + 0.30_f32 * evidence.vertical_ratio
            + 0.20_f32 * evidence.pitch_agreement
            + 0.15_f32 * evidence.local_contrast
            + 0.05_f32 * evidence.quiet_zone)
            .clamp(0.0, 1.0);
        let bucket = (score * 4.0).floor() as usize;
        let target = if expected
            .iter()
            .any(|symbol| point_in_quadrilateral([pattern.center.x, pattern.center.y], symbol))
        {
            &mut contained_evidence_buckets
        } else {
            &mut spurious_evidence_buckets
        };
        target[bucket.min(3)] += 1;
    }
    let spurious_groups = groups
        .iter()
        .filter(|group| {
            !expected.iter().any(|symbol| {
                group.len() == 3 && group.iter().all(|index| proposal_in_symbol(*index, symbol))
            })
        })
        .count();

    FinderGroupingEvaluation {
        expected_symbols: expected.len(),
        proposal_evidence,
        contained_evidence_buckets,
        spurious_evidence_buckets,
        finder_hits,
        proposal_multiplicity,
        grouping_hits,
        finder_eligible_without_group: finder_hits - grouping_hits,
        spurious_proposals,
        contained_proposals: patterns.len() - spurious_proposals,
        spurious_groups,
        contained_groups,
        duplicate_contained_groups,
        scan_telemetry: report.telemetry,
        proposal_latency_ms,
        grouping_latency_ms,
    }
}

fn point_in_quadrilateral(point: [f32; 2], quadrilateral: &Quadrilateral) -> bool {
    let orientation = signed_area(quadrilateral).signum();
    if orientation == 0.0 {
        return false;
    }
    quadrilateral
        .iter()
        .zip(quadrilateral.iter().cycle().skip(1))
        .take(quadrilateral.len())
        .all(|(a, b)| {
            let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
            cross * orientation >= -1e-3
        })
}

impl PayloadScore {
    pub fn exact_match_rate(self) -> f64 {
        if self.expected == 0 {
            0.0
        } else {
            self.exact_matches as f64 / self.expected as f64
        }
    }
}

/// Normalize payload labels according to the established custom-dataset format.
///
/// A same-stem `.txt` file contains one complete payload, which may span lines.
/// Only surrounding whitespace and CRLF line endings are normalized.
pub fn normalize_payload(payload: &str) -> String {
    payload.trim().replace("\r\n", "\n")
}

/// Parse one exact payload from a custom payload-label file.
///
/// Callers must select payload-label mode explicitly. A malformed localization
/// label must never be reinterpreted as a payload label after parsing fails.
pub fn parse_payload_label<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
    let payload = normalize_payload(&content);
    if payload.is_empty() {
        Err("payload label is empty".to_string())
    } else {
        Ok(payload)
    }
}

/// Score normalized payloads one-to-one using exact equality.
pub fn score_payloads(expected: &[String], predicted: &[String]) -> PayloadScore {
    let mut remaining = std::collections::HashMap::<String, usize>::new();
    for payload in expected {
        *remaining.entry(normalize_payload(payload)).or_default() += 1;
    }
    let mut exact_matches = 0;
    for payload in predicted {
        let payload = normalize_payload(payload);
        if let Some(count) = remaining.get_mut(&payload) {
            if *count > 0 {
                *count -= 1;
                exact_matches += 1;
            }
        }
    }
    PayloadScore {
        exact_matches,
        expected: expected.len(),
        predicted: predicted.len(),
    }
}

impl LocalizationScore {
    pub fn precision(self) -> f64 {
        let n = self.true_positives + self.false_positives;
        if n == 0 {
            0.0
        } else {
            self.true_positives as f64 / n as f64
        }
    }

    pub fn recall(self) -> f64 {
        let n = self.true_positives + self.false_negatives;
        if n == 0 {
            0.0
        } else {
            self.true_positives as f64 / n as f64
        }
    }

    pub fn f1(self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

/// Parse the two BoofCV label layouts used by the benchmark dataset.
///
/// Unlike the historical count parser, malformed or incomplete labels are
/// rejected so they cannot silently enter the denominator as zero symbols.
pub fn parse_localization_labels<P: AsRef<Path>>(path: P) -> Result<LocalizationLabels, String> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
    let mut saw_sets = false;
    let mut rows: Vec<Vec<f32>> = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.eq_ignore_ascii_case("SETS") {
            if saw_sets || !rows.is_empty() {
                return Err(format!("invalid SETS marker on line {}", line_index + 1));
            }
            saw_sets = true;
            continue;
        }
        let values = line
            .split_whitespace()
            .map(|token| token.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| format!("invalid numeric label on line {}", line_index + 1))?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err(format!("non-finite label on line {}", line_index + 1));
        }
        rows.push(values);
    }
    if rows.is_empty() {
        return Err("label contains no quadrilaterals".to_string());
    }
    let mut quadrilaterals = Vec::new();
    if saw_sets || rows.iter().all(|row| row.len() == 8) {
        for row in rows {
            if row.len() != 8 {
                return Err("SETS labels require exactly 8 values per symbol".to_string());
            }
            quadrilaterals.push([
                [row[0], row[1]],
                [row[2], row[3]],
                [row[4], row[5]],
                [row[6], row[7]],
            ]);
        }
    } else {
        if !rows.iter().all(|row| row.len() == 2) || rows.len() % 4 != 0 {
            return Err("legacy labels require groups of four x/y rows".to_string());
        }
        for rows in rows.chunks_exact(4) {
            quadrilaterals.push([
                [rows[0][0], rows[0][1]],
                [rows[1][0], rows[1][1]],
                [rows[2][0], rows[2][1]],
                [rows[3][0], rows[3][1]],
            ]);
        }
    }
    Ok(LocalizationLabels { quadrilaterals })
}

/// Scale annotations from source-image coordinates into processed-image coordinates.
pub fn scale_quadrilaterals(
    quadrilaterals: &[Quadrilateral],
    source_dimensions: (usize, usize),
    processed_dimensions: (usize, usize),
) -> Vec<Quadrilateral> {
    let (source_width, source_height) = source_dimensions;
    let (processed_width, processed_height) = processed_dimensions;
    if source_width == 0 || source_height == 0 {
        return quadrilaterals.to_vec();
    }
    let scale_x = processed_width as f32 / source_width as f32;
    let scale_y = processed_height as f32 / source_height as f32;
    quadrilaterals
        .iter()
        .map(|quad| quad.map(|[x, y]| [x * scale_x, y * scale_y]))
        .collect()
}

fn signed_area(polygon: &[[f32; 2]]) -> f32 {
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f32>()
        * 0.5
}

fn line_intersection(start: [f32; 2], end: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let segment = [end[0] - start[0], end[1] - start[1]];
    let edge = [b[0] - a[0], b[1] - a[1]];
    let denominator = segment[0] * edge[1] - segment[1] * edge[0];
    if denominator.abs() <= f32::EPSILON {
        return end;
    }
    let offset = [a[0] - start[0], a[1] - start[1]];
    let t = (offset[0] * edge[1] - offset[1] * edge[0]) / denominator;
    [start[0] + t * segment[0], start[1] + t * segment[1]]
}

fn polygon_intersection(subject: &Quadrilateral, clip: &Quadrilateral) -> Vec<[f32; 2]> {
    let mut output = subject.to_vec();
    let orientation = signed_area(clip).signum();
    if orientation == 0.0 {
        return Vec::new();
    }
    for edge_index in 0..clip.len() {
        let a = clip[edge_index];
        let b = clip[(edge_index + 1) % clip.len()];
        let input = std::mem::take(&mut output);
        let Some(mut start) = input.last().copied() else {
            break;
        };
        for end in input {
            let start_cross = (b[0] - a[0]) * (start[1] - a[1]) - (b[1] - a[1]) * (start[0] - a[0]);
            let end_cross = (b[0] - a[0]) * (end[1] - a[1]) - (b[1] - a[1]) * (end[0] - a[0]);
            let start_inside = start_cross * orientation >= 0.0;
            let end_inside = end_cross * orientation >= 0.0;
            if end_inside {
                if !start_inside {
                    output.push(line_intersection(start, end, a, b));
                }
                output.push(end);
            } else if start_inside {
                output.push(line_intersection(start, end, a, b));
            }
            start = end;
        }
    }
    output
}

/// Intersection over union of two convex quadrilaterals.
pub fn quadrilateral_iou(a: &Quadrilateral, b: &Quadrilateral) -> f32 {
    let area_a = signed_area(a).abs();
    let area_b = signed_area(b).abs();
    if area_a <= f32::EPSILON || area_b <= f32::EPSILON {
        return 0.0;
    }
    let intersection = signed_area(&polygon_intersection(a, b)).abs();
    let union = area_a + area_b - intersection;
    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Match predictions to annotations once each, preferring the greatest overlap.
pub fn score_localizations(
    expected: &[Quadrilateral],
    predicted: &[Quadrilateral],
    minimum_iou: f32,
) -> LocalizationScore {
    let mut edges = vec![Vec::new(); predicted.len()];
    for (pi, prediction) in predicted.iter().enumerate() {
        for (ei, annotation) in expected.iter().enumerate() {
            let overlap = quadrilateral_iou(prediction, annotation);
            if overlap >= minimum_iou {
                edges[pi].push((ei, overlap));
            }
        }
        edges[pi].sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    fn augment(
        prediction: usize,
        edges: &[Vec<(usize, f32)>],
        seen: &mut [bool],
        expected_match: &mut [Option<usize>],
    ) -> bool {
        for &(expected, _) in &edges[prediction] {
            if seen[expected] {
                continue;
            }
            seen[expected] = true;
            if expected_match[expected]
                .is_none_or(|other| augment(other, edges, seen, expected_match))
            {
                expected_match[expected] = Some(prediction);
                return true;
            }
        }
        false
    }
    let mut expected_match = vec![None; expected.len()];
    for prediction in 0..predicted.len() {
        augment(
            prediction,
            &edges,
            &mut vec![false; expected.len()],
            &mut expected_match,
        );
    }
    let true_positives = expected_match.iter().flatten().count();
    let matched_predictions: std::collections::HashSet<_> =
        expected_match.iter().flatten().copied().collect();
    let duplicates = edges
        .iter()
        .enumerate()
        .filter(|(prediction, candidates)| {
            !matched_predictions.contains(prediction) && !candidates.is_empty()
        })
        .count();
    LocalizationScore {
        true_positives,
        false_positives: predicted.len() - true_positives,
        false_negatives: expected.len() - true_positives,
        duplicate_predictions: duplicates,
    }
}

fn max_dim_from_env() -> Option<u32> {
    match env::var("QR_MAX_DIM") {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(0) => None,
            Ok(v) => Some(v),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

/// RGB image data together with both source and processed dimensions.
pub struct LoadedRgb {
    pub pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub source_width: usize,
    pub source_height: usize,
}

/// Load an image as RGB bytes while retaining resize geometry for annotation scaling.
pub fn load_rgb_with_geometry<P: AsRef<Path>>(path: P) -> Result<LoadedRgb, image::ImageError> {
    let img = image::open(path)?;
    let (source_width, source_height) = img.dimensions();
    let rgb = if let Some(max_dim) = max_dim_from_env() {
        let max_side = source_width.max(source_height);
        if max_side > max_dim {
            let resized = img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle);
            resized.to_rgb8()
        } else {
            img.to_rgb8()
        }
    } else {
        img.to_rgb8()
    };
    let (width, height) = rgb.dimensions();
    Ok(LoadedRgb {
        pixels: rgb.into_raw(),
        width: width as usize,
        height: height as usize,
        source_width: source_width as usize,
        source_height: source_height as usize,
    })
}

/// Load an image as RGB bytes along with its processed dimensions.
pub fn load_rgb<P: AsRef<Path>>(path: P) -> Result<(Vec<u8>, usize, usize), image::ImageError> {
    load_rgb_with_geometry(path).map(|image| (image.pixels, image.width, image.height))
}

/// Convert RGB bytes into grayscale.
pub fn to_grayscale(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    rgb_to_grayscale(rgb, width, height)
}

/// Binarize a grayscale image using the same policy as detection.
pub fn binarize(gray: &[u8], width: usize, height: usize) -> BitMatrix {
    if width >= 800 || height >= 800 {
        adaptive_binarize(gray, width, height, 31)
    } else {
        otsu_binarize(gray, width, height)
    }
}

/// Binarize a grayscale image using Otsu's method.
pub fn binarize_otsu(gray: &[u8], width: usize, height: usize) -> BitMatrix {
    otsu_binarize(gray, width, height)
}

/// Detect QR codes in an RGB image.
pub fn detect_qr(rgb: &[u8], width: usize, height: usize) -> Vec<QRCode> {
    crate::detect(rgb, width, height)
}

/// Summary statistics for grayscale data.
#[derive(Debug, Clone, Copy)]
pub struct GrayStats {
    /// Minimum grayscale value.
    pub min: u8,
    /// Maximum grayscale value.
    pub max: u8,
    /// Average grayscale value.
    pub avg: u8,
}

/// Summary statistics for a binary matrix.
#[derive(Debug, Clone, Copy)]
pub struct BinaryStats {
    /// Count of black pixels.
    pub black_pixels: usize,
    /// Total pixels in the matrix.
    pub total_pixels: usize,
    /// Ratio of black pixels to total pixels.
    pub black_ratio: f64,
}

/// Compute min/max/avg for grayscale values.
pub fn grayscale_stats(gray: &[u8]) -> GrayStats {
    let mut min = u8::MAX;
    let mut max = u8::MIN;
    let mut sum: u64 = 0;
    for &v in gray {
        min = min.min(v);
        max = max.max(v);
        sum += v as u64;
    }
    let avg = if gray.is_empty() {
        0
    } else {
        (sum / gray.len() as u64) as u8
    };
    GrayStats { min, max, avg }
}

/// Compute black pixel stats for a binary matrix.
pub fn binary_stats(binary: &BitMatrix) -> BinaryStats {
    let mut black = 0usize;
    for y in 0..binary.height() {
        for x in 0..binary.width() {
            if binary.get(x, y) {
                black += 1;
            }
        }
    }
    let total = binary.width() * binary.height();
    let ratio = if total == 0 {
        0.0
    } else {
        black as f64 / total as f64
    };
    BinaryStats {
        black_pixels: black,
        total_pixels: total,
        black_ratio: ratio,
    }
}

/// Default dataset root from environment variables.
pub fn dataset_root_from_env() -> PathBuf {
    env::var("QR_DATASET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/images/boofcv"))
}

/// Deterministic fingerprint of dataset contents for benchmark provenance.
///
/// The fingerprint includes every file path and file bytes under `root`.
/// It is intended for change detection and traceability, not cryptographic use.
fn file_set_fingerprint<P: AsRef<Path>>(root: P, extensions: &[&str]) -> String {
    struct Fnv1a64(u64);

    impl Fnv1a64 {
        const OFFSET: u64 = 0xcbf29ce484222325;
        const PRIME: u64 = 0x100000001b3;
    }

    impl Default for Fnv1a64 {
        fn default() -> Self {
            Self(Self::OFFSET)
        }
    }

    impl Hasher for Fnv1a64 {
        fn write(&mut self, bytes: &[u8]) {
            for b in bytes {
                self.0 ^= u64::from(*b);
                self.0 = self.0.wrapping_mul(Self::PRIME);
            }
        }

        fn finish(&self) -> u64 {
            self.0
        }
    }

    fn collect_files(root: &Path) -> Vec<PathBuf> {
        let mut stack = vec![root.to_path_buf()];
        let mut files = Vec::new();

        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    files.push(path);
                }
            }
        }

        files.sort();
        files
    }

    let root = root.as_ref();
    if !root.exists() {
        return "missing".to_string();
    }

    let mut hasher = Fnv1a64::default();
    for path in collect_files(root) {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if !extension
            .as_deref()
            .is_some_and(|value| extensions.contains(&value))
        {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"));
        hasher.write(rel.as_bytes());
        hasher.write(&[0]);

        if let Ok(meta) = fs::metadata(&path) {
            hasher.write(&meta.len().to_le_bytes());
        }
        if let Ok(bytes) = fs::read(&path) {
            hasher.write(&bytes);
        }
        hasher.write(&[0xff]);
    }

    format!("{:016x}", hasher.finish())
}

/// Fingerprint only benchmark image inputs, excluding labels and documentation.
pub fn dataset_fingerprint<P: AsRef<Path>>(root: P) -> String {
    file_set_fingerprint(
        root,
        &["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp"],
    )
}

/// Fingerprint localization and payload label inputs separately from images.
pub fn label_fingerprint<P: AsRef<Path>>(root: P) -> String {
    file_set_fingerprint(root, &["txt", "payload"])
}

/// Default bench limit from environment variables.
///
/// Returns `None` (full dataset) when `QR_BENCH_LIMIT` is unset or set to `0`.
/// Previously defaulted to 5 when unset, which silently sampled only a tiny
/// subset and produced misleading reading-rate numbers.
pub fn bench_limit_from_env() -> Option<usize> {
    match env::var("QR_BENCH_LIMIT") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .and_then(|v| if v == 0 { None } else { Some(v) }),
        Err(_) => None,
    }
}

/// Count the number of expected QR codes from a BoofCV-format label file.
///
/// Supports both label layouts found in this dataset:
/// - Modern layout: header + `SETS`, then one line per QR with 8 floats.
/// - Legacy layout: no `SETS`, one corner point per line (2 floats), 4 lines per QR.
///
/// Returns `0` if the file cannot be read or parsed.
pub fn parse_expected_qr_count<P: AsRef<Path>>(txt_path: P) -> usize {
    let content = match fs::read_to_string(txt_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    fn parse_numeric_token_count(line: &str) -> Option<usize> {
        let mut count = 0usize;
        for token in line.split_whitespace() {
            token.parse::<f64>().ok()?;
            count += 1;
        }
        if count == 0 { None } else { Some(count) }
    }

    let mut saw_sets = false;
    let mut post_sets_qr_lines = 0usize;
    let mut pre_sets_qr_lines = 0usize;
    let mut pre_sets_corner_lines = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("SETS") {
            saw_sets = true;
            continue;
        }

        let Some(token_count) = parse_numeric_token_count(trimmed) else {
            continue;
        };

        if saw_sets {
            if token_count >= 8 {
                post_sets_qr_lines += 1;
            }
        } else if token_count >= 8 {
            pre_sets_qr_lines += 1;
        } else if token_count == 2 {
            pre_sets_corner_lines += 1;
        }
    }

    if saw_sets {
        post_sets_qr_lines
    } else {
        let legacy_qrs = pre_sets_corner_lines / 4;
        if pre_sets_qr_lines > 0 {
            pre_sets_qr_lines
        } else {
            legacy_qrs
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        dataset_fingerprint, evaluate_finder_and_grouping, label_fingerprint, normalize_payload,
        parse_expected_qr_count, parse_localization_labels, parse_payload_label,
        point_in_quadrilateral, quadrilateral_iou, scale_quadrilaterals, score_localizations,
        score_payloads,
    };
    use crate::models::BitMatrix;
    use std::fs::{self, create_dir_all};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp_file(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before UNIX epoch")
            .as_nanos();
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("rustqr_expected_qr_count_{nanos}_{sequence}.txt"));
        fs::write(&path, contents).expect("failed to write temp label file");
        path
    }

    #[test]
    fn parse_expected_qr_count_supports_sets_layout() {
        let path = write_temp_file(
            "# list of hand selected 2D points\n\
             SETS\n\
             1.0 2.0 3.0 4.0 5.0 6.0 7.0 8.0\n\
             9.0 10.0 11.0 12.0 13.0 14.0 15.0 16.0\n",
        );
        assert_eq!(parse_expected_qr_count(&path), 2);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_expected_qr_count_supports_legacy_corner_layout() {
        let path = write_temp_file(
            "# list of hand selected 2D points\n\
             10.0 20.0\n\
             30.0 40.0\n\
             50.0 60.0\n\
             70.0 80.0\n\
             11.0 21.0\n\
             31.0 41.0\n\
             51.0 61.0\n\
             71.0 81.0\n",
        );
        assert_eq!(parse_expected_qr_count(&path), 2);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_expected_qr_count_returns_zero_for_invalid_content() {
        let path = write_temp_file("foo bar baz\n# comment only\n");
        assert_eq!(parse_expected_qr_count(&path), 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn dataset_fingerprint_changes_when_dataset_changes() {
        let mut root = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before UNIX epoch")
            .as_nanos();
        root.push(format!("rustqr_dataset_fingerprint_{nanos}"));
        create_dir_all(root.join("nominal")).expect("failed to create temp dataset");
        fs::write(root.join("nominal").join("a.png"), b"abc").expect("failed to write file");

        let before = dataset_fingerprint(&root);
        fs::write(root.join("nominal").join("b.png"), b"image").expect("failed to write file");
        let after = dataset_fingerprint(&root);

        assert_ne!(before, after);
        let label_before = label_fingerprint(&root);
        fs::write(root.join("nominal").join("b.txt"), b"label").expect("failed to write file");
        assert_ne!(label_before, label_fingerprint(&root));
        let _ = fs::remove_dir_all(root);
    }

    fn square(x: f32, y: f32, size: f32) -> [[f32; 2]; 4] {
        [[x, y], [x + size, y], [x + size, y + size], [x, y + size]]
    }

    fn draw_finder(matrix: &mut BitMatrix, start_x: usize, start_y: usize, module: usize) {
        for module_y in 0..7 {
            for module_x in 0..7 {
                let border = module_x == 0 || module_x == 6 || module_y == 0 || module_y == 6;
                let centre = (2..=4).contains(&module_x) && (2..=4).contains(&module_y);
                if border || centre {
                    for y in start_y + module_y * module..start_y + (module_y + 1) * module {
                        for x in start_x + module_x * module..start_x + (module_x + 1) * module {
                            matrix.set(x, y, true);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn proposal_evaluator_separates_finder_and_grouping_stages() {
        let mut matrix = BitMatrix::new(128, 128);
        draw_finder(&mut matrix, 12, 12, 3);
        draw_finder(&mut matrix, 84, 12, 3);
        draw_finder(&mut matrix, 12, 84, 3);
        let evaluation = evaluate_finder_and_grouping(&matrix, &[square(0.0, 0.0, 120.0)]);
        assert_eq!(evaluation.expected_symbols, 1);
        assert_eq!(evaluation.finder_hits, 1);
        assert_eq!(evaluation.proposal_multiplicity, [0, 0, 0, 1]);
        assert_eq!(evaluation.grouping_hits, 1);
        assert_eq!(evaluation.finder_eligible_without_group, 0);
        assert_eq!(evaluation.contained_proposals, 3);
        assert_eq!(
            evaluation.proposal_evidence.len(),
            evaluation.contained_proposals
        );
        assert!(
            evaluation
                .proposal_evidence
                .iter()
                .all(|evidence| (0.0..=1.0).contains(&evidence.horizontal_ratio)
                    && (0.0..=1.0).contains(&evidence.vertical_ratio))
        );
        assert_eq!(
            evaluation.contained_evidence_buckets.iter().sum::<usize>(),
            evaluation.contained_proposals
        );
        assert_eq!(
            evaluation.spurious_evidence_buckets.iter().sum::<usize>(),
            evaluation.spurious_proposals
        );
        assert_eq!(evaluation.spurious_proposals, 0);
        assert_eq!(evaluation.contained_groups, 1);
        assert_eq!(evaluation.duplicate_contained_groups, 0);
        assert_eq!(evaluation.spurious_groups, 0);
        assert!(evaluation.proposal_latency_ms >= 0.0);
        assert!(evaluation.grouping_latency_ms >= 0.0);
    }

    #[test]
    fn stage_evaluator_contains_points_for_both_quad_windings() {
        let clockwise = square(0.0, 0.0, 10.0);
        let counter_clockwise = [clockwise[3], clockwise[2], clockwise[1], clockwise[0]];
        assert!(point_in_quadrilateral([5.0, 5.0], &clockwise));
        assert!(point_in_quadrilateral([5.0, 5.0], &counter_clockwise));
        assert!(!point_in_quadrilateral([15.0, 5.0], &clockwise));
    }

    #[test]
    fn localization_matching_is_one_to_one_and_counts_duplicates() {
        let expected = [square(0.0, 0.0, 10.0)];
        let predicted = [square(0.0, 0.0, 10.0), square(1.0, 1.0, 10.0)];
        let score = score_localizations(&expected, &predicted, 0.5);
        assert_eq!(score.true_positives, 1);
        assert_eq!(score.false_positives, 1);
        assert_eq!(score.duplicate_predictions, 1);
        assert_eq!(score.precision(), 0.5);
    }

    #[test]
    fn localization_matching_counts_missing_and_wrong_location() {
        let expected = [square(0.0, 0.0, 10.0), square(20.0, 20.0, 10.0)];
        let score = score_localizations(&expected, &[square(100.0, 100.0, 10.0)], 0.5);
        assert_eq!(
            (
                score.true_positives,
                score.false_positives,
                score.false_negatives
            ),
            (0, 1, 2)
        );
        assert_eq!(score.f1(), 0.0);
    }

    #[test]
    fn localization_matching_handles_multi_qr_scenes() {
        let expected = [square(0.0, 0.0, 10.0), square(20.0, 20.0, 10.0)];
        let predicted = [square(20.0, 20.0, 10.0), square(0.0, 0.0, 10.0)];
        let score = score_localizations(&expected, &predicted, 0.5);
        assert_eq!(
            (
                score.true_positives,
                score.false_positives,
                score.false_negatives
            ),
            (2, 0, 0)
        );
    }

    #[test]
    fn quadrilateral_overlap_uses_polygon_area_not_bounding_boxes() {
        let bounding_square = square(0.0, 0.0, 10.0);
        let rotated_diamond = [[5.0, 0.0], [10.0, 5.0], [5.0, 10.0], [0.0, 5.0]];
        assert!((quadrilateral_iou(&bounding_square, &rotated_diamond) - 0.5).abs() < 1e-5);
        let score = score_localizations(&[bounding_square], &[rotated_diamond], 0.75);
        assert_eq!((score.true_positives, score.false_positives), (0, 1));
    }

    #[test]
    fn quadrilateral_overlap_handles_skew_and_reversed_winding() {
        let skewed = [[1.0, 0.0], [12.0, 2.0], [9.0, 11.0], [0.0, 8.0]];
        let reversed = [skewed[3], skewed[2], skewed[1], skewed[0]];
        assert!((quadrilateral_iou(&skewed, &reversed) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn localization_matching_maximizes_cardinality() {
        let expected = [square(0.0, 0.0, 10.0), square(8.0, 0.0, 10.0)];
        let predicted = [square(4.0, 0.0, 10.0), square(0.0, 0.0, 10.0)];
        let score = score_localizations(&expected, &predicted, 0.4);
        assert_eq!(
            (
                score.true_positives,
                score.false_positives,
                score.false_negatives
            ),
            (2, 0, 0)
        );
    }

    #[test]
    fn localization_matching_empty_ground_truth_counts_all_predictions_as_extras() {
        let score = score_localizations(&[], &[square(0.0, 0.0, 10.0)], 0.5);
        assert_eq!(
            (
                score.true_positives,
                score.false_positives,
                score.false_negatives,
                score.duplicate_predictions,
            ),
            (0, 1, 0, 0)
        );
    }

    #[test]
    fn annotation_scaling_uses_each_axis_processed_ratio() {
        let annotations = [square(10.0, 20.0, 10.0)];
        let scaled = scale_quadrilaterals(&annotations, (100, 200), (50, 50));
        assert_eq!(
            scaled[0],
            [[5.0, 5.0], [10.0, 5.0], [10.0, 7.5], [5.0, 7.5]]
        );
    }

    #[test]
    fn strict_label_parser_rejects_invalid_and_incomplete_labels() {
        let invalid = write_temp_file("SETS\n0 0 1 nope 1 1 0 1\n");
        assert!(parse_localization_labels(&invalid).is_err());
        let incomplete = write_temp_file("0 0\n1 0\n1 1\n");
        assert!(parse_localization_labels(&incomplete).is_err());
        let _ = fs::remove_file(invalid);
        let _ = fs::remove_file(incomplete);
    }

    #[test]
    fn payload_label_preserves_multiline_content_and_normalizes_crlf() {
        let path = write_temp_file("  BEGIN:VEVENT\r\nSUMMARY: Test\r\nEND:VEVENT\r\n  ");
        assert_eq!(
            parse_payload_label(&path).unwrap(),
            "BEGIN:VEVENT\nSUMMARY: Test\nEND:VEVENT"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn payload_label_rejects_empty_content() {
        let path = write_temp_file(" \r\n\t");
        assert_eq!(
            parse_payload_label(&path).unwrap_err(),
            "payload label is empty"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn payload_scoring_is_exact_normalized_and_one_to_one() {
        let expected = vec![
            "alpha".to_string(),
            "alpha".to_string(),
            "a\r\nb".to_string(),
        ];
        let predicted = vec![
            "alpha".to_string(),
            " alpha ".to_string(),
            "alpha".to_string(),
            "a\nb".to_string(),
            "ALPHA".to_string(),
        ];
        let score = score_payloads(&expected, &predicted);
        assert_eq!(
            (score.exact_matches, score.expected, score.predicted),
            (3, 3, 5)
        );
        assert_eq!(score.exact_match_rate(), 1.0);
        assert_ne!(normalize_payload("ALPHA"), normalize_payload("alpha"));
    }

    #[test]
    fn wrong_payload_never_counts_as_an_exact_match() {
        let score = score_payloads(&["expected".to_string()], &["wrong".to_string()]);
        assert_eq!(score.exact_matches, 0);
        assert_eq!(score.exact_match_rate(), 0.0);
    }
}

/// Smoke test flag from environment variables.
pub fn smoke_from_env() -> bool {
    matches!(
        env::var("QR_SMOKE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// Iterate dataset image paths with optional smoke list and limit.
pub fn dataset_iter<P: AsRef<Path>>(
    root: P,
    limit: Option<usize>,
    smoke: bool,
) -> impl Iterator<Item = PathBuf> {
    let root = root.as_ref();
    let mut images = if smoke {
        load_smoke_list(root).unwrap_or_else(|| collect_images(root))
    } else {
        collect_images(root)
    };

    images.sort();
    if let Some(limit) = limit {
        images.truncate(limit);
    }
    images.into_iter()
}

fn load_smoke_list(root: &Path) -> Option<Vec<PathBuf>> {
    let smoke_path = root.join("_smoke.txt");
    let contents = fs::read_to_string(&smoke_path).ok()?;
    let mut paths = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let candidate = Path::new(line);
        let path = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        if path.exists() {
            paths.push(path);
        }
    }
    if paths.is_empty() { None } else { Some(paths) }
}

fn collect_images(root: &Path) -> Vec<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut images = Vec::new();

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "gif" || ext == "bmp" {
                    images.push(path);
                }
            }
        }
    }

    images
}

#[cfg(test)]
mod timing_tests {
    use std::time::Instant;

    #[test]
    fn test_image_load_timing() {
        for i in 1..=10 {
            let path = format!("benches/images/boofcv/pathological/image{:03}.png", i);
            let start = Instant::now();
            let img = image::open(&path).unwrap();
            let rgb = img.to_rgb8();
            let elapsed = start.elapsed();
            println!(
                "Image {:03}: load+to_rgb8={:.2}ms, dims={:?}",
                i,
                elapsed.as_secs_f64() * 1000.0,
                rgb.dimensions()
            );
        }
    }
}
