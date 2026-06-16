//! Two-dimensional market data surfaces.
//!
//! Provides 2D interpolation structures for market observables that vary by
//! two parameters (e.g., volatility by strike and maturity). Currently supports
//! volatility surfaces with planned expansion for correlation and dividend surfaces.
//!
//! # Surface Types
//!
//! - `VolSurface`: Implied volatility by strike and maturity (bilinear interpolation)
//! - `FxDeltaVolSurface`: FX smile representation quoted in delta space; each
//!   expiry's smile is built independently from its own pillar strikes
//!   (derived from that expiry's forward) and queried via `implied_vol`
//! - `FxDeltaVolSurfaceBuilder`: Builder for market-standard FX ATM / risk-reversal
//!   / butterfly inputs, materializing a strike-based `VolSurface` that samples
//!   each expiry's own smile on the union of all pillar strikes
//!
//! # When to use which surface
//!
//! - Use [`crate::market_data::surfaces::VolSurface`] when market data is already quoted on a strike grid.
//! - Use [`crate::market_data::surfaces::FxDeltaVolSurface`] when FX options are quoted in ATM, risk-reversal,
//!   and butterfly form at standard deltas.
//!
//! # Conventions
//!
//! Surface expiries are expressed as year fractions. Equity-style surfaces are
//! typically indexed by strike, while FX smile inputs may begin in forward-delta
//! space before being mapped onto strikes.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::types::CurveId;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let surface = VolSurface::builder("EQ-FLAT")
//!     .expiries(&[1.0, 2.0])
//!     .strikes(&[90.0, 100.0])
//!     .row(&[0.2, 0.2])
//!     .row(&[0.2, 0.2])
//!     .build()
//!     ?;
//! assert_eq!(surface.id(), &CurveId::from("EQ-FLAT"));
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - General volatility-surface conventions:
//!   `docs/REFERENCES.md#gatheral-volatility-surface`
//! - FX volatility quoting:
//!   `docs/REFERENCES.md#clark-fx-options`, `docs/REFERENCES.md#wystup-fx-options`

mod delta_vol_surface;
pub mod fx_delta_vol_surface;
mod vol_cube;
mod vol_surface;

/// Minimum vol substituted when a SABR expansion yields a non-finite or
/// non-positive value (degenerate parameters or extreme strikes).
pub(crate) const SABR_VOL_FLOOR: f64 = 0.001;

/// Floor a SABR-expanded vol at [`SABR_VOL_FLOOR`], counting replacements so
/// callers can emit one aggregated warning via [`warn_sabr_vol_floored`].
#[inline]
pub(crate) fn floor_sabr_vol(v: f64, floored: &mut usize) -> f64 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        *floored += 1;
        SABR_VOL_FLOOR
    }
}

/// Emit a single aggregated warning when SABR expansion vols were floored.
#[inline]
pub(crate) fn warn_sabr_vol_floored(context: &str, id: &crate::types::CurveId, floored: usize) {
    if floored > 0 {
        tracing::warn!(
            surface_id = %id,
            count = floored,
            floor = SABR_VOL_FLOOR,
            context = context,
            "SABR expansion produced non-finite or non-positive vols; floored to minimum"
        );
    }
}

/// Recover 25d/10d wing vols from ATM/RR/BF quotes, treating BF as a
/// **smile (broker) strangle**: `sigma_wing = ATM + BF ± RR/2` exactly.
/// No market-strangle consistency solve is performed.
#[inline]
pub(crate) fn recover_fx_wing_vols(atm: f64, rr: f64, bf: f64) -> (f64, f64) {
    let sigma_call = atm + bf + 0.5 * rr;
    let sigma_put = atm + bf - 0.5 * rr;
    (sigma_put, sigma_call)
}

/// Garman-Kohlhagen FX forward `F = S * exp((r_d - r_f) * T)` with
/// continuously compounded rates.
#[inline]
pub(crate) fn fx_forward(spot: f64, domestic_rate: f64, foreign_rate: f64, expiry: f64) -> f64 {
    spot * ((domestic_rate - foreign_rate) * expiry).exp()
}

/// Delta-neutral-straddle ATM strike `K = F * exp(sigma^2 T / 2)` under the
/// premium-unadjusted **forward delta** convention.
#[inline]
pub(crate) fn fx_atm_dns_strike(forward: f64, vol: f64, expiry: f64) -> f64 {
    forward * (0.5 * vol * vol * expiry).exp()
}

/// Strikes for put/call at absolute delta `delta_abs` using the
/// premium-unadjusted **forward delta** convention (`Delta_call = N(d1)`),
/// i.e. `K = F * exp(∓ N⁻¹(Δ) σ √T + σ² T / 2)`. Spot-delta and
/// premium-adjusted conventions are intentionally not supported here.
#[inline]
pub(crate) fn fx_put_call_delta_strikes(
    forward: f64,
    sigma_put: f64,
    sigma_call: f64,
    expiry: f64,
    delta_abs: f64,
) -> (f64, f64) {
    let sqrt_t = expiry.sqrt();
    let z_delta = crate::math::special_functions::standard_normal_inv_cdf(delta_abs);
    let k_put =
        forward * (z_delta * sigma_put * sqrt_t + 0.5 * sigma_put * sigma_put * expiry).exp();
    let k_call =
        forward * (-z_delta * sigma_call * sqrt_t + 0.5 * sigma_call * sigma_call * expiry).exp();
    (k_put, k_call)
}

#[inline]
pub(crate) fn fx_put_call_25d_strikes(
    forward: f64,
    sigma_put: f64,
    sigma_call: f64,
    expiry: f64,
) -> (f64, f64) {
    fx_put_call_delta_strikes(forward, sigma_put, sigma_call, expiry, 0.25)
}

/// Per-expiry FX smile pillars: the (strikes, vols) of one expiry's own
/// 3-point (25Δ put, ATM DNS, 25Δ call) or 5-point (plus 10Δ wings) smile.
///
/// This is the canonical per-expiry smile representation shared by
/// [`FxDeltaVolSurface::implied_vol`](fx_delta_vol_surface::FxDeltaVolSurface::implied_vol)
/// (the query path) and [`FxDeltaVolSurfaceBuilder`] (the rectangular
/// materialization). Each expiry's strikes are derived from *that expiry's*
/// forward and vol scale; no strikes from other expiries are involved.
///
/// `wings_10d` carries `(rr_10d, bf_10d)` when 10-delta quotes are available.
///
/// # Errors
///
/// Returns [`InputError::NegativeValue`](crate::error::InputError) if any
/// recovered wing vol is non-positive.
pub(crate) fn fx_smile_pillars(
    forward: f64,
    expiry: f64,
    atm: f64,
    rr_25d: f64,
    bf_25d: f64,
    wings_10d: Option<(f64, f64)>,
) -> crate::Result<(Vec<f64>, Vec<f64>)> {
    let (sigma_put, sigma_call) = recover_fx_wing_vols(atm, rr_25d, bf_25d);
    if sigma_call <= 0.0 || sigma_put <= 0.0 {
        return Err(crate::error::InputError::NegativeValue.into());
    }

    let k_atm = fx_atm_dns_strike(forward, atm, expiry);
    let (k_put, k_call) = fx_put_call_25d_strikes(forward, sigma_put, sigma_call, expiry);

    if let Some((rr_10d, bf_10d)) = wings_10d {
        let (sigma_put_10d, sigma_call_10d) = recover_fx_wing_vols(atm, rr_10d, bf_10d);
        if sigma_call_10d <= 0.0 || sigma_put_10d <= 0.0 {
            return Err(crate::error::InputError::NegativeValue.into());
        }
        let (k_put_10d, k_call_10d) =
            fx_put_call_delta_strikes(forward, sigma_put_10d, sigma_call_10d, expiry, 0.10);
        Ok((
            vec![k_put_10d, k_put, k_atm, k_call, k_call_10d],
            vec![sigma_put_10d, sigma_put, atm, sigma_call, sigma_call_10d],
        ))
    } else {
        Ok((vec![k_put, k_atm, k_call], vec![sigma_put, atm, sigma_call]))
    }
}

/// Piecewise-linear interpolation on sorted knots with flat extrapolation.
pub(crate) fn interp_linear_clamp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    debug_assert!(!xs.is_empty());
    debug_assert_eq!(xs.len(), ys.len());

    if x <= xs[0] {
        return ys[0];
    }
    let n = xs.len();
    if x >= xs[n - 1] {
        return ys[n - 1];
    }

    let idx = xs.partition_point(|&xi| xi < x);
    // idx is now the first index where xs[idx] >= x
    // idx >= 1 (because we already handled x <= xs[0])
    // idx < n (because we already handled x >= xs[n-1])
    let t = (x - xs[idx - 1]) / (xs[idx] - xs[idx - 1]);
    ys[idx - 1] + t * (ys[idx] - ys[idx - 1])
}

// Re-export for ergonomic access (curated list)
pub use delta_vol_surface::FxDeltaVolSurfaceBuilder;
pub use fx_delta_vol_surface::FxDeltaVolSurface;
pub use vol_cube::{VolCube, VolCubeBuilder};
pub use vol_surface::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis, VolSurfaceBuilder,
};
