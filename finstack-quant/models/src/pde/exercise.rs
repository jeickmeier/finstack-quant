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

/// Invalid early-exercise inputs or numerical configuration.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExerciseError {
    /// Payoffs do not cover exactly the solver's interior spatial nodes.
    #[error("exercise payoff length {actual} does not match {expected} interior nodes")]
    PayoffLength {
        /// Number of interior solution values.
        expected: usize,
        /// Number of supplied intrinsic values.
        actual: usize,
    },
    /// A payoff or continuation value is not finite.
    #[error("non-finite exercise {kind} at interior node {index}")]
    NonFiniteValue {
        /// Whether the invalid value is a payoff or continuation value.
        kind: &'static str,
        /// Index into the interior-node value vector.
        index: usize,
    },
    /// The penalty factor is not finite and positive, or no iteration is requested.
    #[error("exercise requires a positive finite penalty factor and at least one iteration")]
    InvalidPenalty,
    /// The time-step interval is not finite and strictly positive.
    #[error("exercise time step must be positive and finite, got {dt}")]
    InvalidTimeStep {
        /// Rejected step width in model years.
        dt: f64,
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

    /// Check the obstacle shape and penalty settings before any state is changed.
    pub(super) fn validate(&self, interior_nodes: usize) -> Result<(), ExerciseError> {
        if self.payoff_values.len() != interior_nodes {
            return Err(ExerciseError::PayoffLength {
                expected: interior_nodes,
                actual: self.payoff_values.len(),
            });
        }
        if let Some(index) = self
            .payoff_values
            .iter()
            .position(|value| !value.is_finite())
        {
            return Err(ExerciseError::NonFiniteValue {
                kind: "payoff",
                index,
            });
        }
        if !self.penalty_factor.is_finite() || self.penalty_factor <= 0.0 || self.iterations == 0 {
            return Err(ExerciseError::InvalidPenalty);
        }
        Ok(())
    }

    /// Apply the penalty method to enforce the exercise constraint.
    ///
    /// After the linear solve, nodes where `u_i < payoff_i` are pushed
    /// toward the intrinsic value. Modifies `u` in place.
    ///
    /// Returns every transition between the binding obstacle and strictly
    /// better continuation. Each index identifies the higher-coordinate node
    /// of the adjacent pair straddling a transition. This handles exercise on
    /// either side and payoffs with multiple exercise regions; an entirely
    /// exercised or entirely continuing grid has no interior boundary.
    ///
    /// # Arguments
    ///
    /// * `u` - Finite continuation values at every interior spatial node,
    ///   overwritten by the penalized exercise values.
    /// * `dt` - Positive finite time-step width in model years.
    ///
    /// # Errors
    ///
    /// Returns [`ExerciseError`] for a length mismatch, non-finite payoff or
    /// continuation value, invalid penalty settings, or invalid `dt`. All
    /// validation occurs before modifying `u`.
    pub fn apply(&self, u: &mut [f64], dt: f64) -> Result<Vec<usize>, ExerciseError> {
        self.validate(u.len())?;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(ExerciseError::InvalidTimeStep { dt });
        }
        if let Some(index) = u.iter().position(|value| !value.is_finite()) {
            return Err(ExerciseError::NonFiniteValue {
                kind: "continuation",
                index,
            });
        }

        // The convex pull cannot change which side of the obstacle a node
        // occupies. Determine transitions before rounding can collapse a
        // penalized value exactly onto its payoff.
        let boundaries = (1..u.len())
            .filter(|&i| (u[i - 1] > self.payoff_values[i - 1]) != (u[i] > self.payoff_values[i]))
            .collect();

        // lambda*dt is exactly the configured factor; forming lambda first
        // can overflow on short intervals even though the update is finite.
        let continuation_weight = 1.0 / (1.0 + self.penalty_factor);
        let payoff_weight = self.penalty_factor / (1.0 + self.penalty_factor);

        // Run all penalty iterations first — no boundary tracking here.
        for _ in 0..self.iterations {
            for (&payoff, u_val) in self.payoff_values.iter().zip(u.iter_mut()) {
                if *u_val < payoff {
                    *u_val = continuation_weight * *u_val + payoff_weight * payoff;
                }
            }
        }

        Ok(boundaries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn american_penalty_enforces_floor() {
        let payoff = vec![5.0, 3.0, 1.0, 0.0, 0.0];
        let exercise = PenaltyExercise::american(payoff.clone());

        let mut u = vec![4.0, 2.0, 0.5, 1.0, 2.0];
        exercise.apply(&mut u, 0.01).expect("valid exercise values");

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
            let boundary = exercise.apply(&mut u, dt).expect("valid exercise values");
            boundaries.push((iterations, boundary, u));
        }

        // (1) Iteration-count invariance + correct converged boundary.
        for (iterations, boundary, _) in &boundaries {
            assert_eq!(
                boundary.as_slice(),
                &[expected_boundary],
                "with {iterations} penalty iteration(s) the early-exercise boundary must be \
                 the converged leftmost continuation node ({expected_boundary}), got {boundary:?}"
            );
        }

        // (2) The boundary must be consistent with the RETURNED u for every
        // iteration count: strictly slack at the boundary node, binding
        // (clamped to at most intrinsic) just left of it.
        for (iterations, boundary, u) in &boundaries {
            let b = boundary[0];
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

    /// A homogeneous exercise/continuation region has no interior transition.
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
            all_cont.apply(&mut u, 0.01).expect("valid inputs"),
            Vec::<usize>::new(),
            "an all-continuation grid has no exercise boundary"
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
            all_ex.apply(&mut u, 0.01).expect("valid inputs"),
            Vec::<usize>::new(),
            "an all-exercise grid has no interior exercise boundary"
        );
    }

    #[test]
    fn exercise_boundaries_cover_calls_and_multiple_regions() {
        let call = PenaltyExercise::american(vec![0.0, 0.0, 1.0, 2.0]);
        let mut values = vec![0.2, 0.5, 0.9, 1.8];
        assert_eq!(
            call.apply(&mut values, 0.1).expect("call exercise"),
            vec![2]
        );

        let multiple = PenaltyExercise::american(vec![2.0; 5]);
        let mut values = vec![1.0, 3.0, 4.0, 1.0, 0.0];
        assert_eq!(
            multiple.apply(&mut values, 0.1).expect("multiple regions"),
            vec![1, 3]
        );
    }

    #[test]
    fn exercise_rejects_malformed_inputs_before_mutation() {
        let exercise = PenaltyExercise::american(vec![1.0, 2.0]);
        let mut values = vec![0.0];
        assert!(matches!(
            exercise.apply(&mut values, 0.1),
            Err(ExerciseError::PayoffLength {
                expected: 1,
                actual: 2
            })
        ));
        assert_eq!(values, vec![0.0]);

        let invalid = PenaltyExercise::american(vec![f64::NAN]);
        assert!(matches!(
            invalid.apply(&mut values, 0.1),
            Err(ExerciseError::NonFiniteValue {
                kind: "payoff",
                index: 0
            })
        ));
        assert_eq!(values, vec![0.0]);

        let exercise = PenaltyExercise::american(vec![1.0]);
        for dt in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                exercise.apply(&mut values, dt),
                Err(ExerciseError::InvalidTimeStep { .. })
            ));
            assert_eq!(values, vec![0.0]);
        }
        exercise
            .apply(&mut values, f64::MIN_POSITIVE)
            .expect("finite update on short interval");
        assert!(values[0].is_finite() && values[0] > 0.999);
    }
}
