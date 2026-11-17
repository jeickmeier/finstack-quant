//! FX delta-quoted volatility smile conversion and evaluation.
//!
//! Market FX volatility surfaces quote implied volatility in delta space using:
//! - `ATM` (at-the-money delta-neutral straddle volatility)
//! - `25D` (and optionally `10D`) risk reversal (`RR = sigma_call - sigma_put`)
//! - `25D` (and optionally `10D`) smile butterfly (`BF = 0.5 * (sigma_call + sigma_put) - sigma_ATM`)
//!
//! Given these quotes and a forward FX rate `F`, this module delegates wing
//! recovery, delta-to-strike inversion, and smile monotonicity checks to the
//! checked kernels in [`finstack_quant_core::market_data::surfaces`], interpolates
//! quote-space curves linearly in expiry, and interpolates linearly in strike across
//! the recovered pillar smile with flat extrapolation beyond the extreme wings.

use finstack_quant_core::market_data::surfaces::{
    DEFAULT_FX_DELTA_REL_TOL, FxDeltaVolSurface, VolGridOpts, VolInterpolationMode, VolQuoteType,
    VolSurface, VolSurfaceAxis, checked_ln_ratio, fx_option_delta_to_strike,
    fx_smile_pillars_checked, interpolate_fx_wing_vols, recover_fx_wing_vols,
};
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::{InputError, Result};

type SmileQuoteRow = (f64, (f64, f64), Option<(f64, f64)>);

fn recover_row(surface: &FxDeltaVolSurface, index: usize) -> Result<SmileQuoteRow> {
    let atm = surface.atm_vols()[index];
    let wings_25d = recover_fx_wing_vols(atm, surface.rr_25d()[index], surface.bf_25d()[index])?;
    let wings_10d = match (surface.rr_10d(), surface.bf_10d()) {
        (Some(rr10), Some(bf10)) => Some(recover_fx_wing_vols(atm, rr10[index], bf10[index])?),
        _ => None,
    };
    Ok((atm, wings_25d, wings_10d))
}

fn interpolate_row(
    surface: &FxDeltaVolSurface,
    i0: usize,
    i1: usize,
    expiry: f64,
) -> Result<SmileQuoteRow> {
    if i0 == i1 {
        return recover_row(surface, i0);
    }
    let expiries = surface.expiries();
    let t0 = expiries[i0];
    let t1 = expiries[i1];
    if expiry.total_cmp(&t0).is_eq() {
        return recover_row(surface, i0);
    }
    if expiry.total_cmp(&t1).is_eq() {
        return recover_row(surface, i1);
    }
    let dt = t1 - t0;
    if !dt.is_finite() || dt <= 0.0 {
        return Err(crate::invalid_input(
            "FX delta volatility surface expiries must be strictly increasing across brackets",
        ));
    }
    // Form both weights directly from the endpoints to avoid `1 - u` rounding cancellation.
    let w0 = (t1 - expiry) / dt;
    let w1 = (expiry - t0) / dt;
    let atm0 = surface.atm_vols()[i0];
    let atm1 = surface.atm_vols()[i1];
    let (atm, sp25, sc25) = interpolate_fx_wing_vols(
        w0,
        (atm0, surface.rr_25d()[i0], surface.bf_25d()[i0]),
        w1,
        (atm1, surface.rr_25d()[i1], surface.bf_25d()[i1]),
    )?;
    let wings_10d = match (surface.rr_10d(), surface.bf_10d()) {
        (Some(rr10), Some(bf10)) => {
            let (_, sp10, sc10) = interpolate_fx_wing_vols(
                w0,
                (atm0, rr10[i0], bf10[i0]),
                w1,
                (atm1, rr10[i1], bf10[i1]),
            )?;
            Some((sp10, sc10))
        }
        _ => None,
    };
    Ok((atm, (sp25, sc25), wings_10d))
}

/// Convert an unsigned Garman-Kohlhagen forward delta into a domestic-per-foreign FX strike.
///
/// Uses the premium-unadjusted forward delta convention:
/// - For calls (`is_call = true`), `Delta_call = N(d1) = delta_abs`.
/// - For puts (`is_call = false`), `|Delta_put| = N(-d1) = delta_abs`.
///
/// # Arguments
///
/// * `is_call` - Selects call delta (`true`) or put delta magnitude (`false`).
/// * `delta_abs` - Unsigned forward delta magnitude strictly inside `(0, 1)` (for example `0.25` for a 25-delta call or 25-delta put).
/// * `forward` - Strictly positive finite FX forward rate (domestic per unit foreign).
/// * `vol` - Strictly positive finite Black implied volatility as an annualized decimal (for example `0.10` for 10%).
/// * `expiry` - Strictly positive finite time to expiry in years.
///
/// # Returns
///
/// Returns the strictly positive strike `K` satisfying the forward delta convention within [`DEFAULT_FX_DELTA_REL_TOL`].
///
/// # Errors
///
/// Returns an error if any numerical input is non-finite or outside its positive/`(0, 1)` domain,
/// or if extreme `vol * sqrt(expiry)` / tail inputs cause the inverted strike to overflow,
/// underflow to zero, or lose delta-roundtrip accuracy beyond [`DEFAULT_FX_DELTA_REL_TOL`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::volatility::{delta_to_strike, strike_to_delta};
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let strike = delta_to_strike(true, 0.25, 1.10, 0.12, 1.0)?;
/// let delta = strike_to_delta(true, strike, 1.10, 0.12, 1.0)?;
/// assert!((delta - 0.25).abs() < 1e-6);
/// # Ok(())
/// # }
/// ```
pub fn delta_to_strike(
    is_call: bool,
    delta_abs: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
) -> Result<f64> {
    fx_option_delta_to_strike(
        is_call,
        delta_abs,
        forward,
        vol,
        expiry,
        DEFAULT_FX_DELTA_REL_TOL,
    )
}

/// Compute unsigned Garman-Kohlhagen forward delta magnitude from an FX strike.
///
/// Uses the premium-unadjusted forward delta convention:
/// - For calls (`is_call = true`), returns `N(d1)`.
/// - For puts (`is_call = false`), returns `|Delta_put| = N(-d1)`.
///
/// # Arguments
///
/// * `is_call` - Selects call delta (`true`) or unsigned put delta magnitude (`false`).
/// * `strike` - Strictly positive finite option strike in domestic-per-foreign units.
/// * `forward` - Strictly positive finite FX forward rate in the same units as `strike`.
/// * `vol` - Strictly positive finite annualized decimal Black volatility.
/// * `expiry` - Strictly positive finite time to expiry in years.
///
/// # Returns
///
/// Returns the unsigned forward delta in `[0, 1]`.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when any input is non-finite or non-positive, or a
/// validation error when `vol * sqrt(expiry)` underflows to zero or overflows `f64`.
pub fn strike_to_delta(
    is_call: bool,
    strike: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
) -> Result<f64> {
    if !strike.is_finite()
        || strike <= 0.0
        || !forward.is_finite()
        || forward <= 0.0
        || !vol.is_finite()
        || vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    let vol_sqrt_t = vol * expiry.sqrt();
    if !vol_sqrt_t.is_finite() || vol_sqrt_t <= 0.0 {
        return Err(crate::invalid_input(
            "FX volatility * sqrt(expiry) must be positive and finite",
        ));
    }
    let ln_fk = checked_ln_ratio(forward, strike);
    let d1 = ln_fk / vol_sqrt_t + 0.5 * vol_sqrt_t;
    if !d1.is_finite() {
        return Err(crate::invalid_input(
            "FX Black d1 overflowed finite f64 range",
        ));
    }
    let signed_d1 = if is_call { d1 } else { -d1 };
    Ok(norm_cdf(signed_d1))
}

/// Evaluate an [`FxDeltaVolSurface`] at `(expiry, strike)` given an explicit FX `forward`.
///
/// Enforces strict expiry bounds `[expiries.first(), expiries.last()]`. At an exact stored expiry
/// knot, only that row's ATM/RR/BF quotes are recovered; between expiries, ATM, risk-reversal,
/// and butterfly quotes are interpolated linearly in expiry before recovering wing volatilities and
/// mapping delta pillars to strikes at `(expiry, forward)`. Strike interpolation is linear between
/// pillar strikes with flat clamping outside the extreme wings.
///
/// # Arguments
///
/// * `surface` - Validated FX delta volatility surface supplying ATM, RR, and BF term structures.
/// * `expiry` - Strictly positive finite query expiry in years; must fall within the surface's stored `[T_min, T_max]` expiry span.
/// * `strike` - Strictly positive finite query strike in domestic-per-foreign units.
/// * `forward` - Strictly positive finite FX forward rate at `expiry` in the same units as `strike`.
///
/// # Returns
///
/// Returns the annualized decimal Black volatility implied at `(expiry, strike)`.
///
/// # Errors
///
/// Returns an error if `expiry`, `strike`, or `forward` is non-positive or non-finite, if
/// `expiry` lies outside `[T_min, T_max]`, if recovered wing volatilities are non-positive, or if
/// inverted pillar strikes are non-monotone or violate [`DEFAULT_FX_DELTA_REL_TOL`].
pub fn get_fx_delta_vol(
    surface: &FxDeltaVolSurface,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> Result<f64> {
    if !expiry.is_finite()
        || expiry <= 0.0
        || !strike.is_finite()
        || strike <= 0.0
        || !forward.is_finite()
        || forward <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    let (i0, i1, _) = super::source::segment(surface.expiries(), expiry)?;
    let (atm, wings_25d, wings_10d) = interpolate_row(surface, i0, i1, expiry)?;
    let (strikes, vols) = fx_smile_pillars_checked(
        forward,
        expiry,
        atm,
        wings_25d,
        wings_10d,
        DEFAULT_FX_DELTA_REL_TOL,
    )?;
    Ok(super::source::linear_clamped(&strikes, &vols, strike))
}

/// Recover the pillar volatilities stored at `expiry_index` on `surface`.
///
/// # Arguments
///
/// * `surface` - Validated FX delta volatility surface.
/// * `expiry_index` - Zero-based index into `surface.expiries()`.
///
/// # Returns
///
/// Returns `[atm, sigma_put_25d, sigma_call_25d]` for 3-point surfaces, or
/// `[atm, sigma_put_25d, sigma_call_25d, sigma_put_10d, sigma_call_10d]` when 10-delta quotes are present.
///
/// # Errors
///
/// Returns a validation error when `expiry_index` is out of range for the FX delta surface or when
/// the stored quotes fail wing recovery.
pub fn get_fx_delta_pillar_vols(
    surface: &FxDeltaVolSurface,
    expiry_index: usize,
) -> Result<Vec<f64>> {
    let expiries = surface.expiries();
    if expiry_index >= expiries.len() {
        return Err(crate::invalid_input(format!(
            "expiry_index {expiry_index} out of range for the FX delta surface '{}' (len={})",
            surface.id().as_str(),
            expiries.len()
        )));
    }
    let (atm, (sp25, sc25), wings_10d) = recover_row(surface, expiry_index)?;
    let mut out = Vec::with_capacity(if wings_10d.is_some() { 5 } else { 3 });
    out.push(atm);
    out.push(sp25);
    out.push(sc25);
    if let Some((sp10, sc10)) = wings_10d {
        out.push(sp10);
        out.push(sc10);
    }
    Ok(out)
}

/// Materialize an [`FxDeltaVolSurface`] onto a strike-by-expiry [`VolSurface`] grid.
///
/// Converts each expiry slice's delta pillars into strikes using its explicit delivery forward
/// `delivery_forwards[i]`, takes the exact IEEE-754 union (`f64::total_cmp` + exact dedup without
/// tolerance coalescing) as the strike axis, and samples every expiry row linearly with flat wing
/// clamping using [`VolInterpolationMode::Vol`].
///
/// # Arguments
///
/// * `surface` - Validated FX delta volatility surface to materialize.
/// * `delivery_forwards` - Positive finite outright FX forward rates (`quote` per `base`), one per expiry in `surface.expiries()`.
///
/// # Returns
///
/// Returns a strike-indexed [`VolSurface`] storing `VolQuoteType::BlackLognormal` and
/// `VolInterpolationMode::Vol`.
///
/// # Errors
///
/// Returns [`InputError::DimensionMismatch`] when `delivery_forwards.len() != surface.expiries().len()`,
/// [`InputError::NonPositiveValue`] when any delivery forward is non-positive or non-finite, or an
/// error if any expiry row produces invalid wings, non-monotone pillar strikes, or invalid
/// delta-to-strike accuracy.
pub fn materialize_fx_delta_surface(
    surface: &FxDeltaVolSurface,
    delivery_forwards: &[f64],
) -> Result<VolSurface> {
    let expiries = surface.expiries();
    if delivery_forwards.len() != expiries.len() {
        return Err(InputError::DimensionMismatch.into());
    }
    if delivery_forwards
        .iter()
        .any(|&fwd| !fwd.is_finite() || fwd <= 0.0)
    {
        return Err(InputError::NonPositiveValue.into());
    }

    let mut row_smiles = Vec::with_capacity(expiries.len());
    let mut all_strikes = Vec::new();
    for (idx, (&expiry, &forward)) in expiries.iter().zip(delivery_forwards.iter()).enumerate() {
        let (atm, wings_25d, wings_10d) = recover_row(surface, idx)?;
        let (strikes, vols) = fx_smile_pillars_checked(
            forward,
            expiry,
            atm,
            wings_25d,
            wings_10d,
            DEFAULT_FX_DELTA_REL_TOL,
        )?;
        all_strikes.extend_from_slice(&strikes);
        row_smiles.push((strikes, vols));
    }

    all_strikes.sort_by(f64::total_cmp);
    all_strikes.dedup_by(|a, b| a.total_cmp(b).is_eq());

    let mut flat_vols = Vec::with_capacity(expiries.len() * all_strikes.len());
    for (row_strikes, row_vols) in &row_smiles {
        for &strike in &all_strikes {
            flat_vols.push(super::source::linear_clamped(row_strikes, row_vols, strike));
        }
    }

    let out = VolSurface::from_grid(
        surface.id().as_str(),
        expiries,
        &all_strikes,
        &flat_vols,
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        },
        None,
    )?;
    Ok(match surface.expiry_clock() {
        Some(clock) => out.with_expiry_clock(clock.clone()),
        None => out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::surfaces::{
        FxAtmConvention, FxButterflyConvention, FxDeltaConvention,
    };
    use finstack_quant_core::money::fx::CurrencyPair;

    fn sample_surface() -> FxDeltaVolSurface {
        FxDeltaVolSurface::builder(
            "EURUSD-VOL",
            CurrencyPair::try_new(Currency::EUR, Currency::USD).unwrap(),
        )
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(&[0.5, 1.0])
        .atm_vols(&[0.10, 0.11])
        .rr_25d(&[0.01, 0.015])
        .bf_25d(&[0.002, 0.003])
        .build()
        .unwrap()
    }

    #[test]
    fn materialize_builds_vol_surface_for_valid_quotes() {
        use finstack_quant_core::dates::{DayCount, DayCountContextState, VolatilityClock};
        use time::macros::date;

        let clock = VolatilityClock::try_new(
            date!(2025 - 01 - 01),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .unwrap();
        let surface = FxDeltaVolSurface::builder(
            "EURUSD-VOL",
            CurrencyPair::try_new(Currency::EUR, Currency::USD).unwrap(),
        )
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiry_clock(clock.clone())
        .expiries(&[0.5, 1.0])
        .atm_vols(&[0.10, 0.11])
        .rr_25d(&[0.01, 0.015])
        .bf_25d(&[0.002, 0.003])
        .build()
        .unwrap();
        let materialized = materialize_fx_delta_surface(&surface, &[1.10, 1.12]).unwrap();
        assert_eq!(materialized.expiries(), &[0.5, 1.0]);
        assert_eq!(materialized.interpolation_mode(), VolInterpolationMode::Vol);
        assert!(materialized.secondary_values().len() >= 3);
        assert_eq!(materialized.expiry_clock(), Some(&clock));
    }

    #[test]
    fn materialize_rejects_mismatched_or_non_positive_forwards() {
        let surface = sample_surface();
        assert!(materialize_fx_delta_surface(&surface, &[1.10]).is_err());
        assert!(materialize_fx_delta_surface(&surface, &[1.10, 0.0]).is_err());
    }
}
