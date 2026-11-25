//! Kink-corrected exercise values of one convertible lattice level.
//!
//! Both convertible lattices decide conversion, issuer call and holder put
//! node by node. Each decision is a `max` or `min` of two values that cross
//! somewhere between two nodes, and where that crossing sits moves with the
//! share price, so the price ripples at the node spacing and a spot bump
//! reads lattice noise as gamma. [`corrected_values`] applies the zero-mean
//! correction of
//! [`kink_smoothing`](finstack_quant_models::trees::kink_smoothing) to each
//! decision in the order the lattices take them.
//!
//! The nodes of a CRR level are uniformly spaced in log-spot, which is what
//! the correction requires.

use finstack_quant_models::trees::kink_smoothing::{smoothed_max, smoothed_min};

/// Exercise terms of one node, before any coupon due there.
#[derive(Debug, Clone, Copy)]
pub(super) struct NodeExercise {
    /// Value of holding the bond: the discounted continuation, or the cash
    /// redemption at maturity.
    pub(super) hold: f64,
    /// Conversion value, when the holder may convert here.
    pub(super) conversion: Option<f64>,
    /// Value if the issuer calls here, given the holder's best response,
    /// when the call can be exercised.
    pub(super) called: Option<f64>,
    /// Put price, when the holder may put here.
    pub(super) put: Option<f64>,
}

/// Values of one level after conversion, call and put, kink-corrected.
///
/// Away from a crossing a node keeps `max(hold, conversion)` capped at
/// `called` and floored at `put`, bit for bit. A decision applies over each
/// run of adjacent nodes that carry it; a run boundary is a jump in the
/// contract terms (a conversion window or soft-call trigger), not a kink,
/// and is left alone.
///
/// # Arguments
///
/// * `nodes` - Exercise terms per node, ordered by share price.
pub(super) fn corrected_values(nodes: &[NodeExercise]) -> Vec<f64> {
    let mut values: Vec<f64> = nodes.iter().map(|node| node.hold).collect();
    over_runs(&mut values, nodes, |node| node.conversion, smoothed_max);
    over_runs(&mut values, nodes, |node| node.called, smoothed_min);
    over_runs(&mut values, nodes, |node| node.put, smoothed_max);
    values
}

/// Overwrite the total of every deciding node of a level with its
/// kink-corrected value plus the cash due at the level.
///
/// # Arguments
///
/// * `level` - Exercise terms per node, ordered by share price; `None` for a
///   node with no decision (its stored total is kept).
/// * `cash_due` - Coupon and principal paid at this level, added after the
///   exercise decision.
/// * `states` - Lattice state per node, in the same order as `level`.
/// * `total` - Accessor for the total value inside one node's state.
pub(super) fn apply_corrected_totals<S>(
    level: &[Option<NodeExercise>],
    cash_due: f64,
    states: &mut [S],
    total: impl Fn(&mut S) -> &mut f64,
) {
    // A node without a decision breaks the run on both sides of it.
    let mut start = 0;
    while start < level.len() {
        let run: Vec<NodeExercise> = level[start..].iter().map_while(|node| *node).collect();
        if run.is_empty() {
            start += 1;
            continue;
        }
        for (state, value) in states[start..].iter_mut().zip(corrected_values(&run)) {
            *total(state) = value + cash_due;
        }
        start += run.len();
    }
}

/// Apply `smooth` over each run of adjacent nodes for which `alternative`
/// is present.
fn over_runs(
    values: &mut [f64],
    nodes: &[NodeExercise],
    alternative: impl Fn(&NodeExercise) -> Option<f64>,
    smooth: fn(&mut [f64], &[f64]),
) {
    let mut start = 0;
    while start < nodes.len() {
        let run: Vec<f64> = nodes[start..].iter().map_while(&alternative).collect();
        if run.is_empty() {
            start += 1;
            continue;
        }
        let end = start + run.len();
        smooth(&mut values[start..end], &run);
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(
        hold: f64,
        conversion: Option<f64>,
        called: Option<f64>,
        put: Option<f64>,
    ) -> NodeExercise {
        NodeExercise {
            hold,
            conversion,
            called,
            put,
        }
    }

    #[test]
    fn nodes_away_from_a_crossing_keep_the_plain_decision_bit_for_bit() {
        // Conversion wins at every node, the call caps every node and the put
        // is below all of them: no decision crosses inside the level.
        let nodes: Vec<NodeExercise> = [(90.0_f64, 120.0_f64), (95.0, 130.0), (99.0, 141.5)]
            .into_iter()
            .map(|(hold, conversion)| node(hold, Some(conversion), Some(110.0), Some(80.0)))
            .collect();
        for value in corrected_values(&nodes) {
            assert_eq!(value.to_bits(), 110.0_f64.to_bits());
        }

        // A node without a decision keeps its stored total and splits the level.
        let level = [Some(nodes[0]), None, Some(nodes[2])];
        let mut states = [(1.0, 0.0), (7.25, 0.0), (3.0, 0.0)];
        apply_corrected_totals(&level, 2.5, &mut states, |state| &mut state.0);
        assert_eq!(states, [(112.5, 0.0), (7.25, 0.0), (112.5, 0.0)]);
    }

    #[test]
    fn a_crossing_is_corrected_only_inside_a_run() {
        // Conversion crosses the hold value between nodes 1 and 2.
        let crossing = [
            node(100.0, Some(80.0), None, None),
            node(100.0, Some(95.0), None, None),
            node(100.0, Some(110.0), None, None),
            node(100.0, Some(125.0), None, None),
        ];
        let corrected = corrected_values(&crossing);
        let plain = [100.0_f64, 100.0, 110.0, 125.0];
        assert_eq!(corrected[0].to_bits(), plain[0].to_bits());
        assert_eq!(corrected[3].to_bits(), plain[3].to_bits());
        assert!((corrected[1] - plain[1]).abs() > 1e-6 || (corrected[2] - plain[2]).abs() > 1e-6);
        // The correction is bounded by a fraction of the node-to-node rise.
        for i in [1, 2] {
            assert!((corrected[i] - plain[i]).abs() < 15.0 / 8.0);
        }

        // The same values with conversion barred below node 2: the boundary
        // of the conversion window is not a kink.
        let windowed = [
            node(100.0, None, None, None),
            node(100.0, None, None, None),
            node(100.0, Some(110.0), None, None),
            node(100.0, Some(125.0), None, None),
        ];
        let corrected = corrected_values(&windowed);
        for (value, expected) in corrected.iter().zip(plain) {
            assert_eq!(value.to_bits(), expected.to_bits());
        }
    }
}
