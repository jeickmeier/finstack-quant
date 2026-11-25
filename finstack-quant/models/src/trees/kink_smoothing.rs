//! Exercise-kink correction for one level of a uniformly spaced lattice.
//!
//! An exercise decision `max(hold, exercise)` sampled at lattice nodes has a
//! kink where the two values cross. A lattice expectation is a quadrature on
//! the node grid, and its error for a kinked integrand depends on where the
//! kink sits between two nodes. That position moves with the market level, so
//! the price ripples at the node spacing and bump-and-reprice sensitivities
//! become noisy.
//!
//! With the difference `exercise - hold` crossing zero a fraction `θ` of the way
//! between two nodes, and changing by `m` per node spacing at the crossing,
//! the sampled sum misses the integral of the
//! positive part by `m·((θ - 1/2)² / 2 - 1/24)` node weights. That is the
//! Euler-Maclaurin term for a slope discontinuity; it averages to zero over
//! `θ`, so the correction removes the ripple without shifting the price
//! level. The functions here add it to the two bracketing nodes:
//!
//! - the cell-average excess `m·(1/2 - s)² / 2` goes to the node whose half
//!   cell contains the crossing, `s` being the crossing's distance from it;
//! - the mean `m / 24` is taken off the two nodes in proportion to their
//!   closeness to the crossing.
//!
//! A node whose difference keeps its sign against both neighbours keeps the
//! plain `max` or `min`, bit for bit.
//!
//! The slope `m` is measured on the exercising side of the crossing, where
//! the difference is positive: it is the slope of the kinked function just
//! past the kink. Where the two values only touch (a deep in-the-money hold
//! value that equals the exercise value to round-off) that slope is zero, so
//! a sign that round-off decides carries no correction.
//!
//! The term is derived here from the Euler-Maclaurin expansion of the
//! trapezoid rule at a slope discontinuity; it is not taken from a published
//! lattice scheme. Because it is zero-mean, a node just beyond a crossing can
//! end slightly below the plain `max` (or above the plain `min`), by at most
//! `m / 24`. Flooring it per node would restore the bias the term removes.
//!
//! # References
//!
//! Background on non-smooth payoffs in lattices and grids (these describe
//! payoff averaging, not the term used here):
//!
//! - Heston, S. & Zhou, G. (2000). "On the Rate of Convergence of Discrete-Time
//!   Contingent Claims." *Mathematical Finance*, 10(1), 53-75.
//! - Pooley, D., Vetzal, K. & Forsyth, P. (2003). "Convergence remedies for
//!   non-smooth payoffs in option pricing." *Journal of Computational
//!   Finance*, 6(4), 25-40.

/// Kink corrections to `max(difference, 0)` sampled at each node of a level.
///
/// A segment brackets a crossing when the difference is positive at one end
/// and not positive at the other; a node exactly on the crossing counts as
/// not positive, which keeps the correction continuous as the crossing passes
/// through a node. A NaN difference yields NaN at its node so an unusable
/// exercise value is not silently discarded.
fn kink_corrections(differences: &[f64]) -> Vec<f64> {
    let mut corrections: Vec<f64> = differences
        .iter()
        .map(|difference| if difference.is_nan() { f64::NAN } else { 0.0 })
        .collect();
    for lower in 0..differences.len().saturating_sub(1) {
        let (left, right) = (differences[lower], differences[lower + 1]);
        let crosses = (left > 0.0 && right <= 0.0) || (left <= 0.0 && right > 0.0);
        if !crosses {
            continue;
        }
        // Distance of the crossing from the left node, in node spacings.
        let theta = left.abs() / (left.abs() + right.abs());
        // Slope of the positive part just past the crossing, per node
        // spacing. It is this segment's slope when the crossing sits at the
        // non-positive node and the slope of the next segment on the positive
        // side when it sits at the positive node, blended in between. The two
        // limits agree across a node, so the correction stays continuous as
        // the crossing passes through it, and a difference that stops rising
        // past the crossing (the two values only touch) gets none.
        let this_rise = (right - left).abs();
        let (beyond, from_positive) = if right > 0.0 {
            (
                differences.get(lower + 2).map(|next| next - right),
                1.0 - theta,
            )
        } else {
            (
                lower
                    .checked_sub(1)
                    .map(|previous| differences[previous] - left),
                theta,
            )
        };
        // A positive node on the edge of the level has no segment beyond it.
        let beyond_rise = beyond
            .filter(|slope| slope.is_finite())
            .map_or(this_rise, |slope| slope.max(0.0));
        // A crossing that reaches the first or last node of the level is
        // leaving the lattice: on its far side nothing is sampled, and one
        // step further no crossing is seen at all. The correction fades to
        // zero over the half cell next to an edge node so that it does not
        // switch off abruptly there.
        let last = differences.len() - 1;
        let edge_fade = |at_edge: bool, distance: f64| {
            if at_edge {
                (2.0 * distance).min(1.0)
            } else {
                1.0
            }
        };
        let fade = edge_fade(lower == 0, theta) * edge_fade(lower + 1 == last, 1.0 - theta);
        let rise = fade * ((1.0 - from_positive) * beyond_rise + from_positive * this_rise);
        let reach = 0.5 - theta;
        let excess = 0.5 * rise * reach * reach;
        let (left_excess, right_excess) = if reach > 0.0 {
            (excess, 0.0)
        } else {
            (0.0, excess)
        };
        corrections[lower] += left_excess - rise * (1.0 - theta) / 24.0;
        corrections[lower + 1] += right_excess - rise * theta / 24.0;
    }
    corrections
}

/// Relative size below which two values are treated as equal.
///
/// Where holding and exercising are worth the same over a whole region (zero
/// rates, or deep in the money) the computed difference is round-off of a few
/// units in the last place and its sign is arbitrary. Reading a crossing into
/// that sign would move the correction with the noise.
const TIE_RELATIVE: f64 = 1e-12;

/// `minuend - subtrahend`, with a difference at round-off level set to zero.
fn difference(minuend: f64, subtrahend: f64) -> f64 {
    let difference = minuend - subtrahend;
    if difference.abs() <= TIE_RELATIVE * minuend.abs().max(subtrahend.abs()) {
        0.0
    } else {
        difference
    }
}

/// Replace each value by the kink-corrected `max(value, alternative)`.
///
/// Use this for a holder's exercise right at one lattice level: `values` are
/// the hold (continuation) values and `alternatives` the exercise values at
/// the same nodes, ordered by state on a uniformly spaced level.
///
/// # Arguments
///
/// * `values` - Hold values per node, in the lattice's value units, ordered by
///   state. Overwritten in place with the corrected maximum, which can sit
///   up to 1/24 of the crossing segment's rise below the plain maximum at
///   the node beside a crossing.
/// * `alternatives` - Exercise values at the same nodes and in the same
///   units. Must have the same length as `values`; only the common prefix
///   is corrected otherwise.
pub fn smoothed_max(values: &mut [f64], alternatives: &[f64]) {
    debug_assert_eq!(values.len(), alternatives.len());
    let len = values.len().min(alternatives.len());
    let differences: Vec<f64> = (0..len)
        .map(|i| difference(alternatives[i], values[i]))
        .collect();
    let corrections = kink_corrections(&differences);
    for ((value, alternative), correction) in values.iter_mut().zip(alternatives).zip(corrections) {
        *value = value.max(*alternative) + correction;
    }
}

/// Replace each value by the kink-corrected `min(value, alternative)`.
///
/// Use this for an issuer's call right at one lattice level: `values` are
/// the hold values and `alternatives` the call prices at the same nodes,
/// ordered by state on a uniformly spaced level.
///
/// # Arguments
///
/// * `values` - Hold values per node, in the lattice's value units, ordered by
///   state. Overwritten in place with the corrected minimum, which can sit
///   up to 1/24 of the crossing segment's rise above the plain minimum at
///   the node beside a crossing.
/// * `alternatives` - Capping values at the same nodes and in the same units.
///   Must have the same length as `values`; only the common prefix is
///   corrected otherwise.
pub fn smoothed_min(values: &mut [f64], alternatives: &[f64]) {
    debug_assert_eq!(values.len(), alternatives.len());
    let len = values.len().min(alternatives.len());
    let differences: Vec<f64> = (0..len)
        .map(|i| difference(values[i], alternatives[i]))
        .collect();
    let corrections = kink_corrections(&differences);
    for ((value, alternative), correction) in values.iter_mut().zip(alternatives).zip(corrections) {
        *value = value.min(*alternative) - correction;
    }
}

/// Corrections that turn the indicator `difference > 0`, sampled at each node,
/// into the fraction of the node's cell on which the difference is positive.
///
/// A node's cell is the half spacing on either side of it. The difference is
/// taken as linear between nodes, and beyond the first and last node it is
/// continued with the slope of the edge segment, so the fraction moves
/// continuously while a crossing enters or leaves the level. A crossing
/// `θ` of the way along a segment makes the sampled sum miss the positive
/// length by `1/2 - θ` node weights, which averages to zero over `θ`.
fn step_corrections(differences: &[f64]) -> Vec<f64> {
    let mut corrections: Vec<f64> = differences
        .iter()
        .map(|difference| if difference.is_nan() { f64::NAN } else { 0.0 })
        .collect();
    for lower in 0..differences.len().saturating_sub(1) {
        let (left, right) = (differences[lower], differences[lower + 1]);
        let crosses = (left > 0.0 && right <= 0.0) || (left <= 0.0 && right > 0.0);
        if !crosses {
            continue;
        }
        let theta = left.abs() / (left.abs() + right.abs());
        let (positive, other, from_other) = if right > 0.0 {
            (lower + 1, lower, theta)
        } else {
            (lower, lower + 1, 1.0 - theta)
        };
        if from_other < 0.5 {
            corrections[other] += 0.5 - from_other;
        } else {
            corrections[positive] -= from_other - 0.5;
        }
    }
    let last = differences.len().saturating_sub(1);
    if last > 0 {
        for (edge, inner) in [(0, 1), (last, last - 1)] {
            let (at_edge, outward) = (differences[edge], differences[edge] - differences[inner]);
            // Distance beyond the edge node at which the continued difference
            // changes sign, when it does so inside the node's own cell.
            let beyond = -at_edge / outward;
            if beyond.is_finite() && (0.0..0.5).contains(&beyond) {
                if at_edge > 0.0 {
                    corrections[edge] -= 0.5 - beyond;
                } else if outward > 0.0 {
                    corrections[edge] += 0.5 - beyond;
                }
            }
        }
    }
    corrections
}

/// Replace each value by the corrected value of a claim an issuer can call.
///
/// The issuer redeems at `price` once the hold value exceeds
/// `price + friction`, so the claim is worth the hold value up to that
/// threshold and `price` beyond it. That is a cap at the threshold, which has
/// a kink and is corrected as in [`smoothed_min`], less a drop of `friction`
/// beyond the threshold, which is a jump. The jump is sampled as the fraction
/// of each node's cell that lies beyond the threshold instead of all or
/// nothing at the node, so the value moves continuously as the threshold
/// passes between nodes. With zero friction the result is that of
/// [`smoothed_min`], bit for bit.
///
/// # Arguments
///
/// * `values` - Hold values per node, in the lattice's value units, ordered by
///   state on a uniformly spaced level. Overwritten in place with the
///   corrected called-or-held value.
/// * `prices` - Redemption price at each node, in the same units and order.
///   Must have the same length as `values`; only the common prefix is
///   corrected otherwise.
/// * `frictions` - Non-negative amount at each node by which the hold value
///   must exceed the price before the issuer calls, in the same units.
pub fn smoothed_call(values: &mut [f64], prices: &[f64], frictions: &[f64]) {
    debug_assert_eq!(values.len(), prices.len());
    debug_assert_eq!(values.len(), frictions.len());
    let len = values.len().min(prices.len()).min(frictions.len());
    let thresholds: Vec<f64> = (0..len).map(|i| prices[i] + frictions[i]).collect();
    let differences: Vec<f64> = (0..len)
        .map(|i| difference(values[i], thresholds[i]))
        .collect();
    let kinks = kink_corrections(&differences);
    if frictions[..len].iter().all(|friction| *friction == 0.0) {
        for i in 0..len {
            values[i] = values[i].min(thresholds[i]) - kinks[i];
        }
        return;
    }
    let steps = step_corrections(&differences);
    for i in 0..len {
        let called = if differences[i] > 0.0 { 1.0 } else { 0.0 };
        values[i] = values[i].min(thresholds[i]) - kinks[i] - frictions[i] * (called + steps[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_away_from_a_crossing_keep_the_plain_extremum_bit_for_bit() {
        let hold = [3.0_f64, 2.5, 0.1, 1.7, 9.25];
        let exercise = [1.0_f64, 2.0, 0.05, 1.2, 9.0];
        let mut max_values = hold;
        smoothed_max(&mut max_values, &exercise);
        let mut min_values = exercise;
        smoothed_min(&mut min_values, &hold);
        for i in 0..hold.len() {
            assert_eq!(max_values[i].to_bits(), hold[i].max(exercise[i]).to_bits());
            assert_eq!(min_values[i].to_bits(), exercise[i].min(hold[i]).to_bits());
        }

        let mut all_exercise = exercise;
        smoothed_max(&mut all_exercise, &hold);
        for i in 0..hold.len() {
            assert_eq!(all_exercise[i].to_bits(), hold[i].to_bits());
        }
    }

    /// Sweep the root of a linear difference of slope 2 over [-1, 1] on the
    /// nodes x = -2..=2 and return, per crossing position, the corrections
    /// added to the plain max. The outer nodes keep the crossing interior.
    fn linear_sweep() -> Vec<(f64, [f64; 5])> {
        let slope = 2.0;
        (0..=400)
            .map(|step| {
                let root = -1.0 + f64::from(step) / 200.0;
                let exercise: Vec<f64> = [-2.0, -1.0, 0.0, 1.0, 2.0]
                    .iter()
                    .map(|x| slope * (x - root))
                    .collect();
                let mut values = vec![0.0; 5];
                smoothed_max(&mut values, &exercise);
                let mut corrections = [0.0; 5];
                for i in 0..5 {
                    corrections[i] = values[i] - exercise[i].max(0.0);
                }
                (root, corrections)
            })
            .collect()
    }

    #[test]
    fn total_correction_is_the_euler_maclaurin_kink_term() {
        for (root, corrections) in linear_sweep() {
            // Crossing position within its segment, measured from the left node.
            let theta = root - root.floor();
            let expected = 2.0 * (0.5 * (theta - 0.5) * (theta - 0.5) - 1.0 / 24.0);
            let total: f64 = corrections.iter().sum();
            assert!(
                (total - expected).abs() < 1e-13,
                "root {root}: total correction {total} vs {expected}"
            );
        }
    }

    #[test]
    fn correction_has_zero_mean_and_varies_continuously_with_the_crossing() {
        let sweep = linear_sweep();
        // One full segment: crossings in [0, 1), 200 equally spaced samples.
        let segment: Vec<f64> = sweep
            .iter()
            .filter(|(root, _)| (0.0..1.0).contains(root))
            .map(|(_, corrections)| corrections.iter().sum::<f64>())
            .collect();
        let mean = segment.iter().sum::<f64>() / segment.len() as f64;
        assert!(mean.abs() < 1e-5, "mean correction {mean}");

        for pair in sweep.windows(2) {
            for node in 0..5 {
                let jump: f64 = pair[1].1[node] - pair[0].1[node];
                assert!(
                    jump.abs() < 0.01,
                    "node {node} jumps by {jump} at root {}",
                    pair[1].0
                );
            }
        }
    }

    #[test]
    fn values_that_only_touch_carry_no_correction() {
        // The hold value approaches the exercise value from above and equals
        // it to round-off deep in the money. Whichever way round-off sets the
        // sign there, the result is the plain maximum to that precision.
        let exercise = [100.0_f64, 110.0, 121.0, 133.1, 146.41];
        for noise in [-3e-13_f64, 0.0, 3e-13] {
            let mut hold = [100.9_f64, 110.15, 121.0, 133.1, 146.41];
            hold[2] -= noise;
            hold[3] += noise;
            let plain: Vec<f64> = hold.iter().zip(exercise).map(|(h, e)| h.max(e)).collect();
            smoothed_max(&mut hold, &exercise);
            for (value, expected) in hold.iter().zip(plain) {
                assert!((value - expected).abs() < 1e-12, "{value} vs {expected}");
            }
        }
    }

    #[test]
    fn a_region_of_ties_gives_the_same_result_whatever_round_off_does() {
        // Exercise is worth 2.75 more than holding at node 1 and exactly the
        // same from node 2 on. Round-off on the tied nodes must not move the
        // kink that sits where the tie begins.
        let exercise = [900.0_f64, 1100.0, 1300.0, 1500.0, 1700.0];
        let reference = {
            let mut hold = [1046.0_f64, 1097.25, 1300.0, 1500.0, 1700.0];
            smoothed_max(&mut hold, &exercise);
            hold
        };
        for noise in [
            [4.5e-13_f64, 0.0, -4.5e-13],
            [-4.5e-13, 4.5e-13, 0.0],
            [0.0, -9e-13, 4.5e-13],
        ] {
            let mut hold = [1046.0_f64, 1097.25, 1300.0, 1500.0, 1700.0];
            for (value, shift) in hold[2..].iter_mut().zip(noise) {
                *value += shift;
            }
            smoothed_max(&mut hold, &exercise);
            for (value, expected) in hold.iter().zip(reference) {
                assert!((value - expected).abs() < 1e-11, "{value} vs {expected}");
            }
        }
        // The kink at the start of the tie is corrected, not ignored.
        assert!((reference[1] - 1100.0).abs() > 1e-3 || (reference[2] - 1300.0).abs() > 1e-3);
    }

    #[test]
    fn smoothed_min_mirrors_smoothed_max() {
        let hold = [0.4_f64, 0.1, -0.3, -0.9];
        let cap = [0.0_f64; 4];
        let mut capped = hold;
        smoothed_min(&mut capped, &cap);
        let mut floored: Vec<f64> = hold.iter().map(|v| -v).collect();
        smoothed_max(&mut floored, &cap);
        for i in 0..hold.len() {
            assert!((capped[i] + floored[i]).abs() < 1e-15);
        }
        // The crossing sits a quarter of the way from node 1 to node 2. The
        // slope on the exercising side is 0.3 beyond node 1 and 0.4 in this
        // segment, blended to 0.325: node 1 nets 0.325 * (1/32 - 3/96) = 0
        // and node 2 gives back 0.325 * (1/4) / 24.
        assert!(capped[1].abs() < 1e-15);
        assert!((capped[2] - (-0.3 + 0.325 * 0.25 / 24.0)).abs() < 1e-15);
        assert_eq!(capped[0].to_bits(), 0.0_f64.to_bits());
        assert_eq!(capped[3].to_bits(), (-0.9_f64).to_bits());
    }

    #[test]
    fn a_call_without_friction_is_the_smoothed_minimum_bit_for_bit() {
        let hold = [0.4_f64, 0.1, -0.3, -0.9, 0.2];
        let price = [0.0_f64, 0.05, -0.1, 0.0, 0.1];
        let mut capped = hold;
        smoothed_min(&mut capped, &price);
        let mut called = hold;
        smoothed_call(&mut called, &price, &[0.0; 5]);
        for (a, b) in capped.iter().zip(called) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    /// Hold values `100 + 2 (x - root)` on the nodes x = -2..=2, called at 100
    /// with the given friction once they exceed `100 + friction`.
    fn called_level(root: f64, friction: f64) -> [f64; 5] {
        let mut values = [-2.0_f64, -1.0, 0.0, 1.0, 2.0].map(|x| 100.0 + 2.0 * (x - root));
        smoothed_call(&mut values, &[100.0; 5], &[friction; 5]);
        values
    }

    #[test]
    fn friction_drops_the_value_over_the_part_of_each_cell_beyond_the_threshold() {
        let friction = 3.0;
        // The threshold 100 + friction is crossed at x = root + friction / 2.
        for step in 0..=1200 {
            let root = -4.5 + f64::from(step) / 200.0;
            let crossing = root + friction / 2.0;
            // The same level capped at the threshold, without the drop.
            let mut capped = [-2.0_f64, -1.0, 0.0, 1.0, 2.0].map(|x| 100.0 + 2.0 * (x - root));
            smoothed_min(&mut capped, &[100.0 + friction; 5]);
            let dropped: f64 = called_level(root, friction)
                .iter()
                .zip(capped)
                .map(|(called, capped)| capped - called)
                .sum();
            // Length of the cells [-2.5, 2.5] beyond the crossing.
            let beyond = (2.5 - crossing).clamp(0.0, 5.0);
            assert!(
                (dropped - friction * beyond).abs() < 1e-11,
                "root {root}: dropped {dropped} vs {}",
                friction * beyond
            );
        }
    }

    #[test]
    fn a_call_with_friction_moves_continuously_as_the_threshold_crosses_the_level() {
        let friction = 3.0;
        let mut previous = called_level(-4.5, friction);
        for step in 1..=2400 {
            let root = -4.5 + f64::from(step) / 400.0;
            let current = called_level(root, friction);
            for (now, before) in current.iter().zip(previous) {
                // Hold values move by 0.005 per sample; the jump would be 3.
                assert!(
                    (now - before).abs() < 0.02,
                    "root {root}: {before} -> {now}"
                );
            }
            previous = current;
        }
    }

    #[test]
    fn a_hold_value_that_only_touches_the_threshold_is_not_called() {
        let mut values = [102.0_f64, 103.0 + 1e-9, 102.0];
        smoothed_call(&mut values, &[100.0; 3], &[3.0; 3]);
        for (value, hold) in values.iter().zip([102.0, 103.0, 102.0]) {
            assert!((value - hold).abs() < 1e-8, "{value} vs {hold}");
        }

        // Round-off around a tie with the threshold decides nothing.
        let mut tied = [103.0_f64 + 4.5e-13, 103.0 - 4.5e-13, 103.0];
        smoothed_call(&mut tied, &[100.0; 3], &[3.0; 3]);
        for value in tied {
            assert!((value - 103.0).abs() < 1e-11, "{value}");
        }
    }

    #[test]
    fn edge_cases_are_well_defined() {
        let mut single = [1.0];
        smoothed_max(&mut single, &[2.0]);
        assert_eq!(single, [2.0]);

        let mut empty: [f64; 0] = [];
        smoothed_max(&mut empty, &[]);

        // A crossing exactly on an interior node: the node takes 1/8 - 1/24
        // of the slope past it, which is 1.
        let mut level = [1.0, 1.0, 1.0, 1.0];
        smoothed_max(&mut level, &[1.0, 1.0, 2.0, 3.0]);
        assert_eq!(level[0], 1.0);
        assert!((level[1] - (1.0 + 1.0 / 12.0)).abs() < 1e-15);
        assert_eq!(level[2], 2.0);
        assert_eq!(level[3], 3.0);

        // A crossing on the last node but one is leaving the lattice: as it
        // reaches the edge node the correction fades out instead of switching
        // off when the edge node stops exercising.
        // The correction shrinks in proportion to the edge node's exercise margin.
        for edge_exercise in [1.4_f64, 1.01, 1.0001, 1.0] {
            let bound = 0.2 * (edge_exercise - 1.0);
            let mut level = [1.0, 1.0, 1.0];
            smoothed_max(&mut level, &[0.0, 0.5, edge_exercise]);
            let plain = [1.0, 1.0, edge_exercise];
            for (value, expected) in level.iter().zip(plain) {
                assert!((value - expected).abs() <= bound, "{value} vs {expected}");
            }
        }

        // An unusable exercise value surfaces instead of being dropped.
        let mut with_nan = [1.0, 1.0, 1.0];
        smoothed_max(&mut with_nan, &[0.0, f64::NAN, 0.0]);
        assert!(with_nan[1].is_nan());
        assert_eq!(with_nan[0], 1.0);
        assert_eq!(with_nan[2], 1.0);
    }
}
