//! Brownian bridge correction for barrier monitoring.
//!
//! When barriers are monitored discretely (at time steps), there is a bias
//! because the continuous path between observations can cross the barrier
//! without detection. The Brownian-bridge formula provides the conditional
//! probability that a barrier was crossed between two observations.
//!
//! # Modelling assumptions
//!
//! The bridge formula below is derived under the assumption that, conditional
//! on `(S_t, S_{t+Δt})`, `ln S_u` is a linear Brownian bridge on `[t, t+Δt]`
//! with **constant** instantaneous volatility `σ`. This is exact for GBM
//! with deterministic (piecewise-constant) volatility on the interval and is
//! widely used as a first-order correction for:
//!
//! - Local-volatility models, provided `σ` is set to an effective local
//!   volatility over the step (bias is `O(σ'·Δt)`).
//! - Stochastic-volatility models (Heston, SABR, …) only over sub-steps
//!   short enough that the variance process is approximately frozen.
//!
//! It is **not** appropriate for jump-diffusion models, for rough
//! volatility (fBM-driven) paths, or for multi-factor processes where the
//! underlying has non-trivial correlation with other state variables. In
//! those cases prefer finer monitoring or a model-specific exit
//! distribution.

/// Compute probability of hitting a barrier between two observations.
///
/// For a Brownian bridge from `S(t)` to `S(t+Δt)`, compute the probability
/// that the path crosses barrier `B` at some point in `[t, t+Δt]` assuming
/// log-normal GBM dynamics with constant volatility `σ`.
///
/// # Arguments
///
/// * `s_t` - Spot at time t
/// * `s_t_dt` - Spot at time t+Δt
/// * `barrier` - Barrier level
/// * `sigma` - Instantaneous volatility over the step (constant assumption)
/// * `dt` - Time step
///
/// # Returns
///
/// Probability of hitting the barrier in `[t, t+Δt]`.
///
/// # Algorithm
///
/// For log-space Brownian motion `X = ln(S)`, the closed-form hit
/// probability of the Brownian bridge (see Glasserman 2003, §6.4) is
///
/// ```text
/// p_hit ≈ exp(−2 · ln(S_t/B) · ln(S_{t+Δt}/B) / (σ² · Δt))
/// ```
///
/// when both `S_t` and `S_{t+Δt}` are on the same side of `B`; otherwise a
/// crossing is guaranteed.
#[must_use]
pub fn bridge_hit_probability(s_t: f64, s_t_dt: f64, barrier: f64, sigma: f64, dt: f64) -> f64 {
    // A barrier between the two observations is a guaranteed crossing
    // regardless of σ or Δt, so this check must run before the
    // degenerate-input guard: σ = 0 or Δt = 0 must not turn a definite
    // discrete crossing into probability 0.
    let min_s = s_t.min(s_t_dt);
    let max_s = s_t.max(s_t_dt);
    if barrier >= min_s && barrier <= max_s {
        return 1.0;
    }

    if dt <= 0.0 || sigma <= 0.0 {
        return 0.0;
    }

    // Both observations on same side of barrier
    if (s_t > barrier && s_t_dt > barrier) || (s_t < barrier && s_t_dt < barrier) {
        // Use log-space approximation
        let ln_ratio_t = (s_t / barrier).ln();
        let ln_ratio_t_dt = (s_t_dt / barrier).ln();

        // Brownian bridge formula
        let numerator = -2.0 * ln_ratio_t * ln_ratio_t_dt;
        let denominator = sigma * sigma * dt;

        if numerator / denominator < -20.0 {
            // Avoid numerical issues for very small probabilities
            return 0.0;
        }

        (numerator / denominator).exp().min(1.0)
    } else {
        // Observations on different sides - must have crossed
        1.0
    }
}

/// Check if a barrier was hit using bridge correction.
///
/// This function combines discrete monitoring with continuous correction:
/// 1. Check if barrier was crossed between observations (discrete check)
/// 2. If not, compute bridge hit probability
/// 3. Generate uniform random and compare
///
/// # Arguments
///
/// * `s_t` - Spot at time t
/// * `s_t_dt` - Spot at time t+Δt
/// * `barrier` - Barrier level
/// * `barrier_type` - Type of barrier (up or down)
/// * `sigma` - Volatility
/// * `dt` - Time step
/// * `uniform_random` - Uniform random number in [0, 1)
///
/// # Returns
///
/// true if barrier was hit, false otherwise
#[allow(clippy::too_many_arguments)]
pub fn check_barrier_hit(
    s_t: f64,
    s_t_dt: f64,
    barrier: f64,
    barrier_type: BarrierDirection,
    sigma: f64,
    dt: f64,
    uniform_random: f64,
) -> bool {
    match barrier_type {
        BarrierDirection::Up => {
            // Check discrete crossing
            if s_t >= barrier || s_t_dt >= barrier {
                return true;
            }

            // Both below barrier - check bridge probability
            if s_t < barrier && s_t_dt < barrier {
                let p_hit = bridge_hit_probability(s_t, s_t_dt, barrier, sigma, dt);
                // Textbook inverse-CDF test: hit iff u < p. `fill_u01`
                // guarantees u ∈ (0, 1), so no boundary guard is needed.
                return uniform_random < p_hit;
            }

            false
        }
        BarrierDirection::Down => {
            // Check discrete crossing
            if s_t <= barrier || s_t_dt <= barrier {
                return true;
            }

            // Both above barrier - check bridge probability
            if s_t > barrier && s_t_dt > barrier {
                let p_hit = bridge_hit_probability(s_t, s_t_dt, barrier, sigma, dt);
                // Textbook inverse-CDF test: hit iff u < p. `fill_u01`
                // guarantees u ∈ (0, 1), so no boundary guard is needed.
                return uniform_random < p_hit;
            }

            false
        }
    }
}

/// Barrier direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierDirection {
    /// Up barrier (knocked out/in when S >= B)
    Up,
    /// Down barrier (knocked out/in when S <= B)
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_hit_probability_definite_hit() {
        // Barrier between observations - should return 1.0
        let p = bridge_hit_probability(90.0, 110.0, 100.0, 0.2, 0.1);
        assert_eq!(p, 1.0);
    }

    #[test]
    fn test_bridge_hit_probability_same_side() {
        // Both above barrier
        let p = bridge_hit_probability(110.0, 120.0, 100.0, 0.2, 0.1);
        assert!(p > 0.0 && p < 1.0);
    }

    #[test]
    fn test_bridge_hit_probability_far_apart() {
        // Far from barrier - low probability
        let p = bridge_hit_probability(150.0, 160.0, 100.0, 0.2, 0.1);
        assert!(p < 0.01);
    }

    #[test]
    fn test_barrier_check_discrete_hit_up() {
        // Discrete hit for up barrier
        assert!(check_barrier_hit(
            95.0,
            105.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
        assert!(check_barrier_hit(
            105.0,
            110.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_check_no_hit_up() {
        // Well below barrier: the bridge hit probability here is ≈ 1.3e-8,
        // so any ordinary uniform draw must report no hit. (`fill_u01`
        // guarantees u ∈ (0, 1), so exactly-zero draws cannot occur.)
        assert!(!check_barrier_hit(
            80.0,
            85.0,
            100.0,
            BarrierDirection::Up,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_check_discrete_hit_down() {
        // Discrete hit for down barrier
        assert!(check_barrier_hit(
            105.0,
            95.0,
            100.0,
            BarrierDirection::Down,
            0.2,
            0.1,
            0.5
        ));
        assert!(check_barrier_hit(
            95.0,
            90.0,
            100.0,
            BarrierDirection::Down,
            0.2,
            0.1,
            0.5
        ));
    }

    #[test]
    fn test_barrier_symmetry() {
        // Up and down should be symmetric
        let p_up = bridge_hit_probability(110.0, 120.0, 100.0, 0.2, 0.1);
        let p_down = bridge_hit_probability(90.0, 80.0, 100.0, 0.2, 0.1);

        // Should be similar due to symmetry
        assert!((p_up - p_down).abs() < 0.1);
    }
}
