//! Exercise-kink correction for one level of a uniformly spaced lattice.
//!
//! An exercise decision `max(hold, exercise)` sampled at lattice nodes has a
//! kink where the two values cross. A lattice expectation is a quadrature on
//! the node grid, and its error for a kinked integrand depends on where the
//! kink sits between two nodes. That position moves with the market level, so
//! the price ripples at the node spacing and bump-and-reprice sensitivities
//! become noisy.
//!
//! With the difference `exercise - hold` linear between the two nodes that
//! bracket a crossing, rising by `m` across the segment, and the crossing a
//! fraction `θ` of the way along, the sampled sum misses the integral of the
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
        let rise = left.abs() + right.abs();
        // Distance of the crossing from the left node, in node spacings.
        let theta = left.abs() / rise;
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
    let differences: Vec<f64> = (0..len).map(|i| alternatives[i] - values[i]).collect();
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
    let differences: Vec<f64> = (0..len).map(|i| values[i] - alternatives[i]).collect();
    let corrections = kink_corrections(&differences);
    for ((value, alternative), correction) in values.iter_mut().zip(alternatives).zip(corrections) {
        *value = value.min(*alternative) - correction;
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
        // The crossing sits a quarter of the way from node 1 to node 2 with a
        // rise of 0.4: node 1 nets 0.4 * (1/32 - 3/96) = 0 and node 2 gives
        // back 0.4 * (1/4) / 24.
        assert!(capped[1].abs() < 1e-15);
        assert!((capped[2] - (-0.3 + 0.4 * 0.25 / 24.0)).abs() < 1e-15);
        assert_eq!(capped[0].to_bits(), 0.0_f64.to_bits());
        assert_eq!(capped[3].to_bits(), (-0.9_f64).to_bits());
    }

    #[test]
    fn edge_cases_are_well_defined() {
        let mut single = [1.0];
        smoothed_max(&mut single, &[2.0]);
        assert_eq!(single, [2.0]);

        let mut empty: [f64; 0] = [];
        smoothed_max(&mut empty, &[]);

        // A crossing exactly on a node: the node takes 1/8 - 1/24 of the rise.
        let mut level = [1.0, 1.0, 1.0];
        smoothed_max(&mut level, &[1.0, 1.0, 2.0]);
        assert_eq!(level[0], 1.0);
        assert!((level[1] - (1.0 + 1.0 / 12.0)).abs() < 1e-15);
        assert_eq!(level[2], 2.0);

        // An unusable exercise value surfaces instead of being dropped.
        let mut with_nan = [1.0, 1.0, 1.0];
        smoothed_max(&mut with_nan, &[0.0, f64::NAN, 0.0]);
        assert!(with_nan[1].is_nan());
        assert_eq!(with_nan[0], 1.0);
        assert_eq!(with_nan[2], 1.0);
    }
}
