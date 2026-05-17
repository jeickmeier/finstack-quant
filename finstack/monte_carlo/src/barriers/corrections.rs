//! Barrier adjustment corrections.
//!
//! Implements the Broadie–Glasserman–Kou (1997) continuity correction which
//! adjusts the barrier level to reduce discretization bias when a continuous
//! barrier is monitored discretely. Gobet & Miri (2001) later generalized the
//! same shift to local-volatility models.

/// Continuity-correction coefficient `β = −ζ(1/2)/√(2π)`.
///
/// Under discrete monitoring with equal time steps, evaluating a *continuous*
/// barrier formula at the shifted barrier `B · exp(±β · σ · √Δt)` reproduces
/// the discretely-monitored price up to `o(1/√n)` error in the number of
/// monitoring points. The shift moves the barrier *away* from spot (down for
/// a down barrier, up for an up barrier), because discrete monitoring misses
/// crossings and so knocks out less often than continuous monitoring.
///
/// The adjusted barrier is `B' = B · exp(±β · σ · √Δt)`: `exp(−…)` for a
/// down barrier, `exp(+…)` for an up barrier.
///
/// # Numerical value
///
/// Using `ζ(1/2) = -1.4603545088095868…` and `√(2π) = 2.5066282746310002…`
/// gives `β ≈ 0.5825971579390106`, i.e. full f64 precision. Previously this
/// constant was rounded to 4 decimal digits (`0.5826`), introducing a
/// systematic bias of a few parts per 10⁴ in every barrier shift. The extra
/// digits cost nothing at runtime and align the implementation with the
/// published formula.
///
/// References:
/// - Broadie, Glasserman & Kou (1997). "A Continuity Correction for Discrete
///   Barrier Options." *Mathematical Finance*, 7(4), 325–349.
/// - Gobet & Miri (2001). "Weak approximation of averaged diffusion
///   processes" (extension to local-volatility models; same leading
///   coefficient β).
pub const GOBET_MIRI_BETA: f64 = 0.582_597_157_939_010_6;

/// Apply the Broadie–Glasserman–Kou (1997) / Gobet–Miri barrier shift.
///
/// Adjusts the barrier level to reduce discretization bias when monitoring is
/// discrete. Named for historical reasons; the leading coefficient is from
/// Broadie–Glasserman–Kou.
///
/// # Arguments
///
/// * `barrier` - Original barrier level
/// * `sigma` - Volatility
/// * `dt` - Time step
/// * `is_down_barrier` - true for down barrier, false for up barrier
///
/// # Returns
///
/// Adjusted barrier level
///
/// # Formula
///
/// - Down barrier: B' = B * exp(-β * σ * √Δt)  (shift down, away from spot)
/// - Up barrier: B' = B * exp(+β * σ * √Δt)  (shift up, away from spot)
///
/// The shift moves the barrier *away* from spot: discrete monitoring misses
/// some crossings between steps, so a continuous formula must use a barrier
/// that is harder to reach to reproduce the discretely-monitored price. This
/// is the canonical Broadie–Glasserman–Kou (1997) result and matches the
/// analytical barrier pricer's discrete-monitoring shift.
pub fn gobet_miri_adjusted_barrier(
    barrier: f64,
    sigma: f64,
    dt: f64,
    is_down_barrier: bool,
) -> f64 {
    let shift = GOBET_MIRI_BETA * sigma * dt.sqrt();

    if is_down_barrier {
        barrier * (-shift).exp()
    } else {
        barrier * shift.exp()
    }
}

/// Alternative barrier adjustment using the "half-step" method.
///
/// This simpler method shifts the barrier by approximately `0.5 · σ · √Δt`
/// *away* from spot, in the same direction as the Broadie–Glasserman–Kou
/// shift in [`gobet_miri_adjusted_barrier`] but with a rounded coefficient.
#[must_use]
pub fn half_step_adjusted_barrier(barrier: f64, sigma: f64, dt: f64, is_down_barrier: bool) -> f64 {
    let shift = 0.5 * sigma * dt.sqrt();

    if is_down_barrier {
        barrier * (-shift).exp()
    } else {
        barrier * shift.exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gobet_miri_down_barrier() {
        let barrier = 100.0;
        let sigma = 0.2;
        let dt = 1.0 / 252.0; // Daily monitoring

        let adjusted = gobet_miri_adjusted_barrier(barrier, sigma, dt, true);

        // BGK: down barrier shifts DOWN, away from spot
        assert!(adjusted < barrier);

        // Shift should be small for small dt
        assert!((adjusted / barrier - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_gobet_miri_up_barrier() {
        let barrier = 100.0;
        let sigma = 0.2;
        let dt = 1.0 / 252.0;

        let adjusted = gobet_miri_adjusted_barrier(barrier, sigma, dt, false);

        // BGK: up barrier shifts UP, away from spot
        assert!(adjusted > barrier);
    }

    #[test]
    fn test_gobet_miri_zero_vol() {
        // With zero volatility, no adjustment
        let barrier = 100.0;
        let adjusted = gobet_miri_adjusted_barrier(barrier, 0.0, 0.01, true);
        assert_eq!(adjusted, barrier);
    }

    #[test]
    fn test_gobet_miri_vs_half_step() {
        let barrier = 100.0;
        let sigma = 0.2;
        let dt = 1.0 / 252.0;

        let gm_down = gobet_miri_adjusted_barrier(barrier, sigma, dt, true);
        let hs_down = half_step_adjusted_barrier(barrier, sigma, dt, true);

        // BGK: both shift down (away from spot) for a down barrier
        assert!(gm_down < barrier);
        assert!(hs_down < barrier);

        // Should be similar magnitude (GobetMiri beta ~0.58, half-step=0.5)
        assert!((gm_down - hs_down).abs() < 0.2);
    }

    #[test]
    fn test_bgk_sign_matches_canonical_down_barrier() {
        // W-44: canonical Broadie–Glasserman–Kou (1997): a DOWN barrier is
        // replaced with H·exp(−βσ√Δt) — the barrier moves DOWN (away from a
        // spot above it). The MC correction must agree in sign with the
        // analytical pricer, which shifts a down barrier by exp(−shift).
        let barrier = 100.0;
        let sigma = 0.2;
        let dt = 1.0 / 252.0;

        let adjusted = gobet_miri_adjusted_barrier(barrier, sigma, dt, true);
        let expected = barrier * (-(GOBET_MIRI_BETA * sigma * dt.sqrt())).exp();

        assert!(
            (adjusted - expected).abs() < 1e-12,
            "down barrier must be shifted by exp(-βσ√Δt): got {adjusted}, expected {expected}",
        );
        // BGK down-barrier shift moves the barrier strictly below H.
        assert!(adjusted < barrier);
    }

    #[test]
    fn test_bgk_sign_matches_canonical_up_barrier() {
        // W-44: canonical BGK: an UP barrier is replaced with H·exp(+βσ√Δt) —
        // the barrier moves UP (away from a spot below it).
        let barrier = 100.0;
        let sigma = 0.2;
        let dt = 1.0 / 252.0;

        let adjusted = gobet_miri_adjusted_barrier(barrier, sigma, dt, false);
        let expected = barrier * (GOBET_MIRI_BETA * sigma * dt.sqrt()).exp();

        assert!(
            (adjusted - expected).abs() < 1e-12,
            "up barrier must be shifted by exp(+βσ√Δt): got {adjusted}, expected {expected}",
        );
        assert!(adjusted > barrier);
    }

    #[test]
    fn test_adjustment_scales_with_dt() {
        let barrier = 100.0;
        let sigma = 0.2;

        let adj_small_dt = gobet_miri_adjusted_barrier(barrier, sigma, 0.01, true);
        let adj_large_dt = gobet_miri_adjusted_barrier(barrier, sigma, 0.1, true);

        // Larger dt should give larger adjustment magnitude (further from
        // the original barrier). The BGK down-barrier shift is negative.
        assert!((adj_large_dt - barrier).abs() > (adj_small_dt - barrier).abs());
    }
}
