use crate::config::DetectConfig;
use crate::types::{Hypothesis, Proposal};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use super::state::PipelineState;

pub(crate) fn run(state: &mut PipelineState, config: &DetectConfig) {
    if config.max_hypotheses == 0 || state.proposals.is_empty() {
        state.hypotheses.clear();
        return;
    }

    let nodes = state
        .proposals
        .iter()
        .map(|proposal| FinderNode::from(*proposal))
        .collect::<Vec<_>>();
    let graph = build_finder_graph(&nodes, state.width, state.height);
    let ranked = generate_ranked_hypotheses(&nodes, &graph, config.max_hypotheses);

    state.hypotheses = if ranked.is_empty() {
        fallback_from_proposals(&state.proposals, config.max_hypotheses)
    } else {
        ranked
            .into_iter()
            .enumerate()
            .map(|(id, hypothesis)| Hypothesis {
                id,
                score: hypothesis.score,
            })
            .collect()
    };
}

const MAX_NEIGHBORS: usize = 8;
const MIN_EDGE_DISTANCE_PX: f32 = 4.0;
const MAX_EDGE_DIAGONAL_RATIO: f32 = 0.75;
const MIN_RIGHT_ANGLE_SCORE: f32 = 0.20;

#[derive(Clone, Copy)]
struct FinderNode {
    id: usize,
    x: f32,
    y: f32,
    score: f32,
}

impl From<Proposal> for FinderNode {
    fn from(value: Proposal) -> Self {
        Self {
            id: value.id,
            x: value.x as f32,
            y: value.y as f32,
            score: value.score,
        }
    }
}

struct FinderGraph {
    neighbors: Vec<Vec<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RankedHypothesis {
    score: f32,
    anchor_id: usize,
    arm_a_id: usize,
    arm_b_id: usize,
}

impl Eq for RankedHypothesis {}

impl PartialOrd for RankedHypothesis {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedHypothesis {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| other.anchor_id.cmp(&self.anchor_id))
            .then_with(|| other.arm_a_id.cmp(&self.arm_a_id))
            .then_with(|| other.arm_b_id.cmp(&self.arm_b_id))
    }
}

fn build_finder_graph(nodes: &[FinderNode], width: usize, height: usize) -> FinderGraph {
    let max_neighbors = MAX_NEIGHBORS.min(nodes.len().saturating_sub(1));
    if max_neighbors == 0 {
        return FinderGraph {
            neighbors: vec![Vec::new(); nodes.len()],
        };
    }

    let diag = (width as f32).hypot(height as f32);
    let min_dist_sq = MIN_EDGE_DISTANCE_PX * MIN_EDGE_DISTANCE_PX;
    let max_dist_sq = (diag * MAX_EDGE_DIAGONAL_RATIO).powi(2);

    let mut neighbors = vec![Vec::new(); nodes.len()];
    for (i, anchor) in nodes.iter().enumerate() {
        let mut edges = Vec::new();
        for (j, other) in nodes.iter().enumerate() {
            if i == j {
                continue;
            }

            let dx = other.x - anchor.x;
            let dy = other.y - anchor.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < min_dist_sq || dist_sq > max_dist_sq {
                continue;
            }

            edges.push((dist_sq, other.id, j));
        }

        edges.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        neighbors[i] = edges
            .into_iter()
            .take(max_neighbors)
            .map(|(_, _, idx)| idx)
            .collect();
    }

    FinderGraph { neighbors }
}

fn generate_ranked_hypotheses(
    nodes: &[FinderNode],
    graph: &FinderGraph,
    max_hypotheses: usize,
) -> Vec<RankedHypothesis> {
    if max_hypotheses == 0 {
        return Vec::new();
    }

    let mut beam: BinaryHeap<Reverse<RankedHypothesis>> = BinaryHeap::with_capacity(max_hypotheses);

    for (anchor_idx, neighbor_idxs) in graph.neighbors.iter().enumerate() {
        if neighbor_idxs.len() < 2 {
            continue;
        }
        for i in 0..(neighbor_idxs.len() - 1) {
            for j in (i + 1)..neighbor_idxs.len() {
                let arm_a_idx = neighbor_idxs[i];
                let arm_b_idx = neighbor_idxs[j];

                let Some(score) =
                    l_shape_score(nodes[anchor_idx], nodes[arm_a_idx], nodes[arm_b_idx])
                else {
                    continue;
                };

                let (arm_a_id, arm_b_id) = sort_pair(nodes[arm_a_idx].id, nodes[arm_b_idx].id);
                let candidate = RankedHypothesis {
                    score,
                    anchor_id: nodes[anchor_idx].id,
                    arm_a_id,
                    arm_b_id,
                };
                retain_top_h(&mut beam, candidate, max_hypotheses);
            }
        }
    }

    let mut out = beam
        .into_iter()
        .map(|wrapped| wrapped.0)
        .collect::<Vec<_>>();
    out.sort_by(best_first_cmp);
    out
}

fn l_shape_score(anchor: FinderNode, arm_a: FinderNode, arm_b: FinderNode) -> Option<f32> {
    let v1x = arm_a.x - anchor.x;
    let v1y = arm_a.y - anchor.y;
    let v2x = arm_b.x - anchor.x;
    let v2y = arm_b.y - anchor.y;

    let len1 = v1x.hypot(v1y);
    let len2 = v2x.hypot(v2y);
    if len1 <= f32::EPSILON || len2 <= f32::EPSILON {
        return None;
    }

    let cos_theta = (v1x * v2x + v1y * v2y) / (len1 * len2);
    if !cos_theta.is_finite() {
        return None;
    }
    let right_angle = 1.0 - cos_theta.abs().clamp(0.0, 1.0);
    if right_angle < MIN_RIGHT_ANGLE_SCORE {
        return None;
    }

    let arm_balance = 1.0 - ((len1 - len2).abs() / len1.max(len2)).clamp(0.0, 1.0);
    let span = (arm_a.x - arm_b.x).hypot(arm_a.y - arm_b.y);
    let ratio = span / (len1 + len2);
    let isosceles_right_ratio = std::f32::consts::SQRT_2 / 2.0;
    let shape_consistency =
        1.0 - ((ratio - isosceles_right_ratio).abs() / isosceles_right_ratio).clamp(0.0, 1.0);

    let proposal_support =
        (0.45 * anchor.score + 0.275 * arm_a.score + 0.275 * arm_b.score).clamp(0.0, 1.0);
    Some(
        (0.45 * proposal_support
            + 0.35 * right_angle
            + 0.15 * arm_balance
            + 0.05 * shape_consistency)
            .clamp(0.0, 1.0),
    )
}

fn retain_top_h(
    beam: &mut BinaryHeap<Reverse<RankedHypothesis>>,
    candidate: RankedHypothesis,
    max_hypotheses: usize,
) {
    if beam.len() < max_hypotheses {
        beam.push(Reverse(candidate));
        return;
    }

    let replace = beam.peek().map(|worst| candidate > worst.0).unwrap_or(true);
    if replace {
        beam.pop();
        beam.push(Reverse(candidate));
    }
}

fn sort_pair(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

fn best_first_cmp(a: &RankedHypothesis, b: &RankedHypothesis) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| a.anchor_id.cmp(&b.anchor_id))
        .then_with(|| a.arm_a_id.cmp(&b.arm_a_id))
        .then_with(|| a.arm_b_id.cmp(&b.arm_b_id))
}

fn fallback_from_proposals(proposals: &[Proposal], max_hypotheses: usize) -> Vec<Hypothesis> {
    let mut ranked = proposals.to_vec();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.view.cmp(&b.view))
            .then_with(|| a.y.cmp(&b.y))
            .then_with(|| a.x.cmp(&b.x))
            .then_with(|| a.id.cmp(&b.id))
    });
    ranked
        .into_iter()
        .take(max_hypotheses)
        .map(|proposal| Hypothesis {
            id: proposal.id,
            score: proposal.score,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::config::DetectConfig;
    use crate::pipeline::state::PipelineState;
    use crate::types::{Proposal, ProposalView};

    #[test]
    fn deterministic_and_bounded_for_same_input() {
        let config = DetectConfig {
            max_hypotheses: 4,
            ..DetectConfig::default()
        };

        let proposals = vec![
            proposal(0, 50, 50, 0.98),
            proposal(1, 50, 94, 0.91),
            proposal(2, 94, 50, 0.90),
            proposal(3, 50, 8, 0.88),
            proposal(4, 8, 50, 0.87),
            proposal(5, 85, 85, 0.82),
            proposal(6, 12, 88, 0.80),
        ];

        let mut state_a = PipelineState::new(128, 128);
        state_a.proposals = proposals.clone();
        run(&mut state_a, &config);

        let mut state_b = PipelineState::new(128, 128);
        state_b.proposals = proposals;
        run(&mut state_b, &config);

        assert_eq!(state_a.hypotheses, state_b.hypotheses);
        assert!(state_a.hypotheses.len() <= config.max_hypotheses);
        assert!(
            state_a
                .hypotheses
                .windows(2)
                .all(|pair| pair[0].score >= pair[1].score)
        );
    }

    #[test]
    fn falls_back_to_top_proposals_when_no_l_shape_exists() {
        let config = DetectConfig {
            max_hypotheses: 2,
            ..DetectConfig::default()
        };

        let mut state = PipelineState::new(64, 64);
        state.proposals = vec![
            proposal(11, 4, 4, 0.35),
            proposal(12, 60, 60, 0.92),
            proposal(13, 32, 32, 0.74),
        ];

        run(&mut state, &config);

        assert_eq!(state.hypotheses.len(), 2);
        assert_eq!(state.hypotheses[0].id, 12);
        assert!(state.hypotheses[0].score >= state.hypotheses[1].score);
    }

    fn proposal(id: usize, x: usize, y: usize, score: f32) -> Proposal {
        Proposal {
            id,
            view: ProposalView::Otsu,
            x,
            y,
            score,
            raw_score: score,
        }
    }
}
