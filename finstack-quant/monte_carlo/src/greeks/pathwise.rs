//! Pathwise differentiation for Greeks.
//!
//! Computes derivatives by differentiating the payoff with respect to parameters
//! along each path. Works best for smooth payoffs without discontinuities.
//!
//! Reference: Glasserman (2003) - "Monte Carlo Methods in Financial Engineering"

use crate::online_stats::OnlineStats;

/// Compute pathwise delta for European call under GBM.
///
/// Delta = ∂V/∂S₀ = e^(-rT) E[1_{S_T > K} * S_T / S_0]
///
/// # Arguments
///
/// * `terminal_spots` - Terminal spot prices from MC paths
/// * `initial_spot` - Initial spot price
/// * `strike` - Strike price
/// * `discount_factor` - Discount factor e^(-rT)
///
/// # Returns
///
/// Delta estimate with standard error
#[must_use]
pub fn pathwise_delta_call(
    terminal_spots: &[f64],
    initial_spot: f64,
    strike: f64,
    discount_factor: f64,
) -> (f64, f64) {
    let mut stats = OnlineStats::new();

    for &s_t in terminal_spots {
        let payoff_deriv = if s_t > strike {
            s_t / initial_spot
        } else {
            0.0
        };

        let delta_contribution = discount_factor * payoff_deriv;
        stats.update(delta_contribution);
    }

    (stats.mean(), stats.stderr())
}

/// Compute pathwise delta for European put under GBM.
#[must_use]
pub fn pathwise_delta_put(
    terminal_spots: &[f64],
    initial_spot: f64,
    strike: f64,
    discount_factor: f64,
) -> (f64, f64) {
    let mut stats = OnlineStats::new();

    for &s_t in terminal_spots {
        let payoff_deriv = if s_t < strike {
            -s_t / initial_spot
        } else {
            0.0
        };

        let delta_contribution = discount_factor * payoff_deriv;
        stats.update(delta_contribution);
    }

    (stats.mean(), stats.stderr())
}

/// Compute pathwise vega for a European call or put under GBM.
///
/// For GBM dynamics `S_T = S_0 · exp((r − q − σ²/2)T + σW_T)` the pathwise
/// derivative of the terminal spot w.r.t. volatility is
///
/// ```text
/// ∂S_T/∂σ = S_T · (W_T − σT)
/// ```
///
/// For a call, `∂(S_T − K)⁺/∂σ = 1_{S_T>K} · ∂S_T/∂σ`. For a put,
/// `∂(K − S_T)⁺/∂σ = 1_{S_T<K} · (−∂S_T/∂σ)` — the payoff's dependence on
/// `S_T` is decreasing, so the sign flips. Put-call parity guarantees the
/// expected vegas agree (both positive).
///
/// Returns Vega scaled by 0.01 (sensitivity per 1% volatility change).
///
/// # Arguments
///
/// * `terminal_spots` - Terminal spot prices
/// * `initial_spot` - Initial spot price (unused for the payoff derivative; kept
///   for API symmetry with the delta helpers)
/// * `strike` - Strike price
/// * `time_to_maturity` - Time to maturity T
/// * `volatility` - Current volatility σ
/// * `discount_factor` - Discount factor e^(-rT)
/// * `wiener_increments` - Total Brownian increment W_T for each path
/// * `is_call` - true for call, false for put
///
/// # Panics
///
/// Debug builds assert `terminal_spots.len() == wiener_increments.len()`.
///
/// # References
///
/// Glasserman (2003) - "Monte Carlo Methods in Financial Engineering", Chapter 7
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn pathwise_vega(
    terminal_spots: &[f64],
    _initial_spot: f64,
    strike: f64,
    time_to_maturity: f64,
    volatility: f64,
    discount_factor: f64,
    wiener_increments: &[f64],
    is_call: bool,
) -> (f64, f64) {
    debug_assert_eq!(
        terminal_spots.len(),
        wiener_increments.len(),
        "terminal_spots and wiener_increments must have the same length"
    );

    let mut stats = OnlineStats::new();
    // Call pays max(S-K, 0); put pays max(K-S, 0). The put's dependence on S_T
    // is decreasing, so ∂payoff/∂σ carries an extra minus sign.
    let payoff_sign = if is_call { 1.0 } else { -1.0 };

    for (i, &s_t) in terminal_spots.iter().enumerate() {
        let in_money = if is_call { s_t > strike } else { s_t < strike };

        if in_money {
            let w_t = wiener_increments[i];
            let ds_dsigma = s_t * (w_t - volatility * time_to_maturity);
            let payoff_deriv = payoff_sign * ds_dsigma;

            let vega_contribution = discount_factor * payoff_deriv;
            stats.update(vega_contribution * 0.01);
        } else {
            stats.update(0.0);
        }
    }

    (stats.mean(), stats.stderr())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathwise_delta_call_itm() {
        // All paths ITM
        let terminal_spots = vec![110.0, 120.0, 115.0];
        let (delta, stderr) = pathwise_delta_call(&terminal_spots, 100.0, 100.0, 1.0);

        // Delta should be positive for ITM calls
        assert!(delta > 0.0);
        assert!(delta < 2.0); // Sanity check
        assert!(stderr >= 0.0);
    }

    #[test]
    fn test_pathwise_delta_call_otm() {
        // All paths OTM
        let terminal_spots = vec![90.0, 85.0, 95.0];
        let (delta, _) = pathwise_delta_call(&terminal_spots, 100.0, 100.0, 1.0);

        // Delta should be zero for OTM
        assert_eq!(delta, 0.0);
    }

    #[test]
    fn test_pathwise_delta_put() {
        // ITM put paths
        let terminal_spots = vec![90.0, 85.0, 95.0];
        let (delta, _) = pathwise_delta_put(&terminal_spots, 100.0, 100.0, 1.0);

        // Delta should be negative for puts
        assert!(delta < 0.0);
        assert!(delta > -2.0);
    }

    #[test]
    fn test_vega_positive() {
        let terminal_spots = vec![110.0, 105.0, 115.0];
        let wiener_increments = vec![0.5, 0.3, 0.7];

        let (vega, _) = pathwise_vega(
            &terminal_spots,
            100.0, // initial_spot
            100.0, // strike
            1.0,   // time_to_maturity
            0.2,   // volatility
            1.0,   // discount_factor
            &wiener_increments,
            true,
        );

        assert!(vega.is_finite());
    }

    /// Put-call parity: Black-Scholes vega is identical for calls and puts
    /// because d/dσ(C − P) = d/dσ(S − K·e^{-rT}) = 0. The pathwise estimator
    /// must reflect this — historically the put branch had the wrong sign.
    #[test]
    fn test_pathwise_vega_put_positive_and_matches_call() {
        use crate::discretization::exact::ExactGbm;
        use crate::process::gbm::{GbmParams, GbmProcess};
        use crate::rng::philox::PhiloxRng;
        use crate::traits::{Discretization, RandomStream};

        let s0 = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.0;
        let sigma = 0.20;
        let maturity: f64 = 1.0;
        let num_paths = 20_000;
        let df = (-r * maturity).exp();

        let process = GbmProcess::new(GbmParams::new(r, q, sigma).expect("valid GBM"));
        let disc = ExactGbm::new();
        let rng = PhiloxRng::new(12345);

        let mut terminal_spots = Vec::with_capacity(num_paths);
        let mut wiener_increments = Vec::with_capacity(num_paths);
        let sqrt_t = maturity.sqrt();
        let mut z = [0.0_f64; 1];
        let mut state = [0.0_f64; 1];
        let mut work = vec![0.0_f64; disc.work_size(&process)];

        for path_id in 0..num_paths {
            let mut path_rng = rng.substream(path_id as u64);
            path_rng.fill_std_normals(&mut z);
            state[0] = s0;
            disc.step(&process, 0.0, maturity, &mut state, &z, &mut work);
            terminal_spots.push(state[0]);
            wiener_increments.push(z[0] * sqrt_t);
        }

        let (call_vega, call_se) = pathwise_vega(
            &terminal_spots,
            s0,
            k,
            maturity,
            sigma,
            df,
            &wiener_increments,
            true,
        );
        let (put_vega, put_se) = pathwise_vega(
            &terminal_spots,
            s0,
            k,
            maturity,
            sigma,
            df,
            &wiener_increments,
            false,
        );

        // Black-Scholes closed-form vega for ATM option: S · φ(d1) · √T · 0.01.
        // d1 = (ln(S/K) + (r - q + σ²/2)T)/(σ√T). Put-call parity implies the
        // expected vegas are equal; at finite n the two sample estimators
        // differ by a mean-zero MC term (their supports are disjoint
        // subsets of the same path population), so we only require both to
        // be positive and close to the BS analytic.
        let d1 = ((s0 / k).ln() + (r - q + 0.5 * sigma * sigma) * maturity) / (sigma * sqrt_t);
        let phi_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let bs_vega = s0 * phi_d1 * sqrt_t * 0.01;

        assert!(
            call_vega > 0.0,
            "call vega should be positive, got {call_vega}"
        );
        assert!(
            put_vega > 0.0,
            "put vega should be positive (historical sign bug), got {put_vega}"
        );
        // 4σ band on each individual estimator: BS ≈ 0.3752 and MC stderr at
        // 20k paths is ~0.003–0.005, so a tolerance of 0.05 comfortably
        // clears natural noise while still catching a sign flip.
        assert!(
            (call_vega - bs_vega).abs() < 0.05,
            "call vega {call_vega} too far from BS {bs_vega} (se={call_se})"
        );
        assert!(
            (put_vega - bs_vega).abs() < 0.05,
            "put vega {put_vega} too far from BS {bs_vega} (se={put_se})"
        );
    }
}
