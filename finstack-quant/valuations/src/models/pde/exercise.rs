//! Penalty method for American and Bermudan early exercise.
//!
//! After each time step, the penalty method enforces `u >= payoff` by adding
//! a large penalty term to the main diagonal at nodes where the constraint
//! is violated. This is simpler than PSOR and works naturally with all theta
//! schemes without inner iteration tuning.

/// Early exercise constraint enforced via the penalty method.
///
/// At exercise-eligible time steps, nodes where `u_i < payoff_i` get a large
/// penalty `lambda` added to the diagonal, driving the solution toward the
/// intrinsic value. One penalty iteration usually suffices; the solver optionally
/// does 2–3 for convergence assurance.
#[derive(Debug, Clone)]
pub struct PenaltyExercise {
    /// Penalty scaling factor (default `1e8`). The effective penalty per step
    /// is `penalty_factor / dt`.
    pub penalty_factor: f64,
    /// Intrinsic payoff value at each interior grid node.
    pub payoff_values: Vec<f64>,
    /// Exercise schedule (American = every step, Bermudan = specific times).
    pub exercise_type: ExerciseType,
    /// Number of penalty iterations per step (default 1; 2–3 for convergence assurance).
    pub iterations: usize,
}

/// Exercise schedule type.
#[derive(Debug, Clone)]
pub enum ExerciseType {
    /// Exercisable at every time step.
    American,
    /// Exercisable only at specified times (must align with time grid).
    Bermudan {
        /// Exercise times (year fractions from valuation date).
        exercise_times: Vec<f64>,
    },
}

impl PenaltyExercise {
    /// Create an American exercise constraint.
    ///
    /// # Arguments
    ///
    /// * `payoff_values` — intrinsic value at each interior grid node
    pub fn american(payoff_values: Vec<f64>) -> Self {
        Self {
            penalty_factor: 1e8,
            payoff_values,
            exercise_type: ExerciseType::American,
            iterations: 1,
        }
    }

    /// Create a Bermudan exercise constraint.
    ///
    /// # Arguments
    ///
    /// * `payoff_values` — intrinsic value at each interior grid node
    /// * `exercise_times` — times at which exercise is allowed
    pub fn bermudan(payoff_values: Vec<f64>, exercise_times: Vec<f64>) -> Self {
        Self {
            penalty_factor: 1e8,
            payoff_values,
            exercise_type: ExerciseType::Bermudan { exercise_times },
            iterations: 1,
        }
    }

    /// Check whether exercise is allowed at time `t`.
    pub fn is_exercise_time(&self, t: f64) -> bool {
        match &self.exercise_type {
            ExerciseType::American => true,
            ExerciseType::Bermudan { exercise_times } => {
                exercise_times.iter().any(|&et| (et - t).abs() < 1e-10)
            }
        }
    }

    /// Apply the penalty method to enforce the exercise constraint.
    ///
    /// After the linear solve, nodes where `u_i < payoff_i` are pushed
    /// toward the intrinsic value. Modifies `u` in place.
    ///
    /// Returns the early exercise boundary (leftmost grid index where the
    /// continuation value strictly exceeds intrinsic, or `None` if fully
    /// exercised).
    ///
    /// # Boundary detection
    ///
    /// The boundary is read from the **converged** solution *after* all
    /// penalty iterations, never from an intermediate iterate. Recording it
    /// inside the iteration loop (as a previous implementation did) is wrong
    /// for `iterations >= 2`: the penalty drives an exercised node's value to
    /// `payoff` so tightly that, after the first iteration, the `u_i < payoff`
    /// test can flip to false purely through floating-point round-off — which
    /// would misclassify an exercised node as the continuation boundary.
    ///
    /// On the converged solution an exercised node satisfies `u_i <= payoff_i`
    /// (the penalty is a convex pull toward `payoff` from below, so it never
    /// overshoots above it), while a continuation node keeps its untouched
    /// value `u_i > payoff_i`. A *strict* `u_i > payoff_i` test therefore
    /// separates the two regions robustly, with no tolerance.
    pub fn apply(&self, u: &mut [f64], dt: f64) -> Option<usize> {
        debug_assert_eq!(u.len(), self.payoff_values.len());

        let lambda = self.penalty_factor / dt;

        // Run all penalty iterations first — no boundary tracking here.
        for _ in 0..self.iterations {
            for (&payoff, u_val) in self.payoff_values.iter().zip(u.iter_mut()) {
                if *u_val < payoff {
                    // Apply penalty: push u toward payoff.
                    // In the continuous limit: u = (u + lambda*dt*payoff) / (1 + lambda*dt).
                    // With lambda*dt = penalty_factor >> 1, this ≈ payoff.
                    *u_val = (*u_val + lambda * dt * payoff) / (1.0 + lambda * dt);
                }
            }
        }

        // Record the early-exercise boundary from the CONVERGED solution:
        // the leftmost node where the constraint is slack (continuation
        // value strictly above intrinsic).
        self.payoff_values
            .iter()
            .zip(u.iter())
            .position(|(&payoff, &u_val)| u_val > payoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn american_penalty_enforces_floor() {
        let payoff = vec![5.0, 3.0, 1.0, 0.0, 0.0];
        let exercise = PenaltyExercise::american(payoff.clone());

        // Solution below intrinsic should be pushed up
        let mut u = vec![4.0, 2.0, 0.5, 1.0, 2.0];
        exercise.apply(&mut u, 0.01);

        for (i, (&u_val, &p_val)) in u.iter().zip(payoff.iter()).enumerate() {
            if p_val > 0.0 {
                assert!(
                    u_val >= p_val - 0.01,
                    "u[{i}]={u_val} should be near payoff={p_val}"
                );
            }
        }
    }

    #[test]
    fn bermudan_respects_schedule() {
        let payoff = vec![1.0, 1.0, 1.0];
        let exercise = PenaltyExercise::bermudan(payoff, vec![0.5, 1.0]);
        assert!(exercise.is_exercise_time(0.5));
        assert!(exercise.is_exercise_time(1.0));
        assert!(!exercise.is_exercise_time(0.75));
    }

    /// [P6-3] The early-exercise boundary index must be recorded from the
    /// **converged** post-projection solution and must NOT depend on the
    /// number of penalty iterations.
    ///
    /// Failure mode being guarded: the old `apply` recorded `boundary_idx`
    /// inside the iteration loop via `else if boundary_idx.is_none()`. Once
    /// set on the first iteration the value was frozen — never refreshed
    /// against the converged `u`. For `iterations >= 2` the boundary the
    /// caller receives is then a snapshot of an *un-converged* intermediate
    /// state rather than of the solution that `apply` actually returns.
    ///
    /// This test runs the same American put projection with 1, 2 and 3
    /// penalty iterations and asserts:
    ///   1. The boundary index is identical for every iteration count.
    ///   2. The boundary is consistent with the *returned* `u`: at the
    ///      boundary node the constraint is slack (`u > payoff` strictly),
    ///      and the node immediately to its left is in the exercise region
    ///      (`u <= payoff` — the penalty clamps it to at most intrinsic, and
    ///      for `iterations >= 2` it converges to exactly `payoff`).
    #[test]
    fn exercise_boundary_is_converged_and_iteration_count_invariant() {
        // American put: intrinsic decreasing in the (spot) index.
        let payoff = vec![5.0, 4.0, 3.0, 2.0, 1.0, 0.0];
        // Raw continuation values: nodes 0,1,2 below intrinsic (exercise
        // region), nodes 3,4,5 above intrinsic (continuation region). The
        // converged early-exercise boundary is therefore index 3.
        let u_raw = vec![4.0, 3.5, 2.5, 2.5, 1.5, 0.5];
        let expected_boundary = 3_usize;
        let dt = 0.01_f64;

        let mut boundaries = Vec::new();
        for iterations in [1_usize, 2, 3] {
            let exercise = PenaltyExercise {
                penalty_factor: 1e8,
                payoff_values: payoff.clone(),
                exercise_type: ExerciseType::American,
                iterations,
            };
            let mut u = u_raw.clone();
            let boundary = exercise.apply(&mut u, dt);
            boundaries.push((iterations, boundary, u));
        }

        // (1) Iteration-count invariance + correct converged boundary.
        for (iterations, boundary, _) in &boundaries {
            assert_eq!(
                *boundary,
                Some(expected_boundary),
                "with {iterations} penalty iteration(s) the early-exercise boundary must be \
                 the converged leftmost continuation node ({expected_boundary}), got {boundary:?}"
            );
        }

        // (2) The boundary must be consistent with the RETURNED u for every
        // iteration count: strictly slack at the boundary node, binding
        // (clamped to at most intrinsic) just left of it.
        for (iterations, boundary, u) in &boundaries {
            let b = boundary.expect("boundary recorded");
            assert!(
                u[b] > payoff[b],
                "[{iterations} iters] returned u[{b}]={} must be > payoff[{b}]={} \
                 (boundary node is strictly in the continuation region)",
                u[b],
                payoff[b],
            );
            assert!(
                b > 0 && u[b - 1] <= payoff[b - 1],
                "[{iterations} iters] returned u[{}]={} must be <= payoff[{}]={} \
                 (node left of the boundary is in the exercise region — the penalty \
                 clamps it to at most intrinsic)",
                b - 1,
                u[b - 1],
                b - 1,
                payoff[b - 1],
            );
        }
    }

    /// [P6-3] When every node is in the continuation region (no early
    /// exercise is optimal anywhere) the boundary is the first node, for any
    /// iteration count — and when every node is exercised it is `None`.
    #[test]
    fn exercise_boundary_handles_all_continuation_and_all_exercise() {
        let payoff = vec![3.0, 2.0, 1.0];

        // All continuation: every u strictly above intrinsic.
        let all_cont = PenaltyExercise {
            penalty_factor: 1e8,
            payoff_values: payoff.clone(),
            exercise_type: ExerciseType::American,
            iterations: 3,
        };
        let mut u = vec![10.0, 9.0, 8.0];
        assert_eq!(
            all_cont.apply(&mut u, 0.01),
            Some(0),
            "all-continuation boundary must be node 0"
        );

        // All exercise: every u below intrinsic → no continuation node.
        let all_ex = PenaltyExercise {
            penalty_factor: 1e8,
            payoff_values: payoff,
            exercise_type: ExerciseType::American,
            iterations: 3,
        };
        let mut u = vec![0.1, 0.1, 0.1];
        assert_eq!(
            all_ex.apply(&mut u, 0.01),
            None,
            "all-exercise boundary must be None"
        );
    }
}
