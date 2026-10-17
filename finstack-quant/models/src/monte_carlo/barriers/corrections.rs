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
/// gives `β ≈ 0.5825971579390106`, i.e. full f64 precision.
///
/// References:
/// - Broadie, Glasserman & Kou (1997). "A Continuity Correction for Discrete
///   Barrier Options." *Mathematical Finance*, 7(4), 325–349.
/// - Gobet & Miri (2001). "Weak approximation of averaged diffusion
///   processes" (extension to local-volatility models; same leading
///   coefficient β).
pub const GOBET_MIRI_BETA: f64 = 0.582_597_157_939_010_6;
