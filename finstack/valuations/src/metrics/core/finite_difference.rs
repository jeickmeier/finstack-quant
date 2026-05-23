//! Generic finite difference utilities for risk metric calculations.
//!
//! Provides standard bump sizes and helper functions for bump-and-reprice
//! sensitivity calculations. Each metric calculator implements its own
//! bumping and revaluation logic based on the instrument type.
//!
//! # Bump Size Standards
//!
//! Following market conventions:
//! - Spot/Underlying: 1% (0.01)
//! - Volatility: 1% (0.01)
//! - Interest rates: 1bp (**expressed as 1.0**) when using `BumpSpec::parallel_bp`
//! - Credit spreads: 1bp (**expressed as 1.0**) when using `BumpSpec::parallel_bp`
//! - Correlations: 1% (0.01)

/// Standard bump sizes for finite difference calculations.
pub(crate) mod bump_sizes {
    /// Spot/underlying price bump: 1% (0.01)
    pub(crate) const SPOT: f64 = 0.01;
    /// Volatility bump: **absolute** 1 vol point (0.01).
    ///
    /// This represents an **absolute** change in implied volatility, e.g. 20% → 21%.
    /// (Not a 1% relative scaling of the surface.)
    pub(crate) const VOLATILITY: f64 = 0.01;
    /// Correlation bump: 1% (0.01)
    ///
    /// Used by correlation sensitivity calculators (e.g., quanto options).
    pub(crate) const CORRELATION: f64 = 0.01;
}

/// Minimum width tolerated when normalizing a finite difference.
const MIN_FINITE_DIFF_WIDTH: f64 = 1e-12;

/// Extract the numeric value from a scalar quote.
#[inline]
pub(crate) fn scalar_numeric_value(
    scalar: &finstack_core::market_data::scalars::MarketScalar,
) -> f64 {
    match scalar {
        finstack_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        finstack_core::market_data::scalars::MarketScalar::Price(money) => money.amount(),
    }
}

/// Rebuild a scalar quote using the same variant and currency as `template`.
#[inline]
pub(crate) fn scalar_with_numeric_value(
    template: &finstack_core::market_data::scalars::MarketScalar,
    value: f64,
) -> finstack_core::market_data::scalars::MarketScalar {
    match template {
        finstack_core::market_data::scalars::MarketScalar::Unitless(_) => {
            finstack_core::market_data::scalars::MarketScalar::Unitless(value)
        }
        finstack_core::market_data::scalars::MarketScalar::Price(money) => {
            finstack_core::market_data::scalars::MarketScalar::Price(
                finstack_core::money::Money::new(value, money.currency()),
            )
        }
    }
}

/// Clone a market context and replace a scalar quote with a new numeric value.
pub(crate) fn replace_scalar_value(
    context: &finstack_core::market_data::context::MarketContext,
    scalar_id: &str,
    template: &finstack_core::market_data::scalars::MarketScalar,
    value: f64,
) -> finstack_core::market_data::context::MarketContext {
    context
        .clone()
        .insert_price(scalar_id, scalar_with_numeric_value(template, value))
}

/// Compute a central difference normalized by the full bump width.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`](finstack_core::InputError::NonPositiveValue)
/// if `width` is non-finite or its magnitude is at or below
/// [`MIN_FINITE_DIFF_WIDTH`]. A degenerate bump width cannot yield a meaningful
/// sensitivity; returning a silent `0.0` would be indistinguishable from a true
/// zero Greek, so the degenerate case is surfaced as an error instead.
#[inline]
pub(crate) fn central_diff_by_width(
    pv_up: f64,
    pv_down: f64,
    width: f64,
) -> finstack_core::Result<f64> {
    if !width.is_finite() || width.abs() <= MIN_FINITE_DIFF_WIDTH {
        tracing::warn!(
            width,
            min_width = MIN_FINITE_DIFF_WIDTH,
            "finite-difference bump width is degenerate; cannot normalize sensitivity"
        );
        return Err(finstack_core::InputError::NonPositiveValue.into());
    }
    Ok((pv_up - pv_down) / width)
}

/// Compute a central difference normalized by a symmetric half-bump.
///
/// # Errors
///
/// Propagates the degenerate-width error from [`central_diff_by_width`] when
/// `2 * half_bump` is non-finite or negligibly small.
#[inline]
pub(crate) fn central_diff_by_half_bump(
    pv_up: f64,
    pv_down: f64,
    half_bump: f64,
) -> finstack_core::Result<f64> {
    central_diff_by_width(pv_up, pv_down, 2.0 * half_bump)
}

/// Compute a scaled central difference using the actual bump width.
///
/// # Errors
///
/// Propagates the degenerate-width error from [`central_diff_by_width`] when
/// `width` is non-finite or negligibly small.
#[inline]
pub(crate) fn scaled_central_diff_by_width(
    pv_up: f64,
    pv_down: f64,
    width: f64,
    scale: f64,
) -> finstack_core::Result<f64> {
    Ok(central_diff_by_width(pv_up, pv_down, width)? * scale)
}

/// Helper to bump a scalar price in MarketContext.
///
/// Creates a new MarketContext with the bumped price, leaving other data unchanged.
/// Used for computing delta and other price sensitivities via finite differences.
///
/// # Arguments
///
/// * `context` - Original market context containing the price to bump
/// * `price_id` - ID of the price scalar to bump (e.g., equity spot price)
/// * `bump_pct` - Relative bump size as decimal (e.g., 0.01 for 1%)
///
/// # Returns
///
/// New MarketContext with the specified price bumped by the given percentage.
/// All other market data remains unchanged.
///
/// # Errors
///
/// Returns an error if:
/// - The price ID is not found in the market context
/// - The price data is invalid or corrupted
///
/// # Examples
///
/// ```ignore
/// // This function is internal - use Delta metric calculators for public API
/// use finstack_valuations::metrics::core::finite_difference::bump_scalar_price;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::market_data::scalars::MarketScalar;
///
/// let context = MarketContext::new()
///     .insert_price("AAPL", MarketScalar::Unitless(150.0));
///
/// // Bump the price up by 1%
/// let bumped = bump_scalar_price(&context, "AAPL", 0.01)?;
/// ```
pub(crate) fn bump_scalar_price(
    context: &finstack_core::market_data::context::MarketContext,
    price_id: &str,
    bump_pct: f64,
) -> finstack_core::Result<finstack_core::market_data::context::MarketContext> {
    let mut bumped = context.clone();
    let current = bumped.get_price(price_id)?;

    let bumped_value = match current {
        finstack_core::market_data::scalars::MarketScalar::Unitless(v) => {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v * (1.0 + bump_pct))
        }
        finstack_core::market_data::scalars::MarketScalar::Price(m) => {
            finstack_core::market_data::scalars::MarketScalar::Price(
                finstack_core::money::Money::new(m.amount() * (1.0 + bump_pct), m.currency()),
            )
        }
    };

    bumped = bumped.insert_price(price_id, bumped_value);
    Ok(bumped)
}

/// Compute a central finite difference of an instrument's PV with respect to
/// a single scalar price (typically a spot rate / underlying level), using a
/// **relative** bump.
///
/// Returns `(PV(s * (1 + h)) - PV(s * (1 - h))) / (2 * s * h)`.
///
/// Used by FX/equity delta calculators that share the "look up scalar, bump
/// up/down by relative %, central difference" pattern.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`](finstack_core::InputError::NonPositiveValue)
/// if the implied bump width `s * h` is non-finite or non-positive (e.g. when
/// the spot `s` is zero). A degenerate bump width cannot yield a meaningful
/// sensitivity, so it is surfaced as an error rather than a silent `0.0`.
pub(crate) fn central_diff_scalar_relative<I>(
    inst: &I,
    market: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    scalar_id: &str,
    bump_pct: f64,
) -> finstack_core::Result<f64>
where
    I: crate::instruments::common_impl::traits::Instrument + ?Sized,
{
    let scalar = market.get_price(scalar_id)?;
    let current = scalar_numeric_value(scalar);
    let bump_size = current * bump_pct;
    if !bump_size.is_finite() || bump_size <= 0.0 {
        tracing::warn!(
            scalar_id,
            spot = current,
            bump_pct,
            bump_size,
            "relative finite-difference bump width is degenerate (zero/negative spot?); \
             cannot compute sensitivity"
        );
        return Err(finstack_core::InputError::NonPositiveValue.into());
    }
    let up = bump_scalar_price(market, scalar_id, bump_pct)?;
    let down = bump_scalar_price(market, scalar_id, -bump_pct)?;
    let pv_up = inst.value(&up, as_of)?.amount();
    let pv_dn = inst.value(&down, as_of)?.amount();
    central_diff_by_width(pv_up, pv_dn, 2.0 * bump_size)
}

/// Helper to bump a discount curve with parallel shift.
///
/// Creates a new MarketContext with a bumped discount curve using a parallel shift.
/// Used for computing DV01 (interest rate sensitivity) via finite differences.
///
/// # Arguments
///
/// * `context` - Original market context containing the discount curve
/// * `curve_id` - ID of the discount curve to bump (e.g., "USD-OIS")
/// * `bump_bp` - Bump size in basis points (e.g., 1.0 for 1bp)
///
/// # Returns
///
/// New MarketContext with the specified discount curve shifted in parallel by
/// the given amount. All other market data remains unchanged.
///
/// # Errors
///
/// Returns an error if:
/// - The curve ID is not found in the market context
/// - The bump operation fails due to invalid curve data
///
/// # Examples
///
/// ```ignore
/// // This function is internal - use DV01 metric calculators for public API
/// use finstack_valuations::metrics::core::finite_difference::bump_discount_curve_parallel;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::types::CurveId;
///
/// let context = MarketContext::new();
/// let curve_id = CurveId::from("USD-OIS");
///
/// // Bump the curve by 1bp (1.0 in bp units)
/// let bumped = bump_discount_curve_parallel(&context, &curve_id, 1.0)?;
/// ```
pub(crate) fn bump_discount_curve_parallel(
    context: &finstack_core::market_data::context::MarketContext,
    curve_id: &finstack_core::types::CurveId,
    bump_bp: f64,
) -> finstack_core::Result<finstack_core::market_data::context::MarketContext> {
    use finstack_core::market_data::bumps::{BumpSpec, MarketBump};

    context.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(bump_bp),
    }])
}

/// Helper to bump a volatility surface by an **absolute** volatility amount (vol points).
///
/// This applies an additive parallel bump to the surface:
/// \[
/// \sigma'(t, k) = \max(0, \sigma(t, k) + \Delta\sigma)
/// \]
///
/// where `bump_abs` is expressed in *absolute* volatility units (e.g., `0.01` for +1 vol point).
///
/// # Non-Negativity Guarantee
///
/// The underlying [`VolSurface::bump`] implementation ensures that bumped volatilities
/// are floored at zero. This means:
/// - For negative bumps (e.g., `bump_abs = -0.15` on a 10% vol surface), vols are clamped to 0.0
/// - This prevents mathematically invalid negative volatilities
/// - Vega calculations near zero vol may exhibit non-linearity due to this floor
///
/// Prefer this helper for market-standard vega/volga/vanna definitions (derivatives w.r.t. σ).
pub(crate) fn bump_surface_vol_absolute(
    context: &finstack_core::market_data::context::MarketContext,
    vol_surface_id: &str,
    bump_abs: f64,
) -> finstack_core::Result<finstack_core::market_data::context::MarketContext> {
    use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump};
    use finstack_core::types::CurveId;

    if !bump_abs.is_finite() {
        return Err(finstack_core::InputError::Invalid.into());
    }
    if bump_abs == 0.0 {
        return Ok(context.clone());
    }

    context.bump([MarketBump::Curve {
        id: CurveId::from(vol_surface_id),
        spec: BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: bump_abs,
            bump_type: BumpType::Parallel,
        },
    }])
}

/// Compute a mixed partial derivative using central differences for two
/// variables with absolute bump sizes `h` (first variable) and `k` (second).
///
/// Returns `[f(+h,+k) - f(+h,-k) - f(-h,+k) + f(-h,-k)] / (4 h k)`.
pub(crate) fn central_mixed<EEpp, EEpm, EEmp, Eemm, E1, E2, E3, E4>(
    eval_pp: EEpp,
    eval_pm: EEpm,
    eval_mp: EEmp,
    eval_mm: Eemm,
    h_abs: f64,
    k_abs: f64,
) -> finstack_core::Result<f64>
where
    EEpp: FnOnce() -> finstack_core::Result<E1>,
    EEpm: FnOnce() -> finstack_core::Result<E2>,
    EEmp: FnOnce() -> finstack_core::Result<E3>,
    Eemm: FnOnce() -> finstack_core::Result<E4>,
    E1: Into<f64>,
    E2: Into<f64>,
    E3: Into<f64>,
    E4: Into<f64>,
{
    // Guard against invalid bump sizes
    if !h_abs.is_finite() || h_abs <= 0.0 {
        return Err(finstack_core::InputError::NonPositiveValue.into());
    }
    if !k_abs.is_finite() || k_abs <= 0.0 {
        return Err(finstack_core::InputError::NonPositiveValue.into());
    }
    let v_pp = eval_pp()?.into();
    let v_pm = eval_pm()?.into();
    let v_mp = eval_mp()?.into();
    let v_mm = eval_mm()?.into();
    Ok((v_pp - v_pm - v_mp + v_mm) / (4.0 * h_abs * k_abs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::money::Money;

    #[test]
    fn central_mixed_rejects_nonpositive_or_invalid_hk() {
        let err_h = central_mixed(
            || Ok(0.0f64),
            || Ok(0.0f64),
            || Ok(0.0f64),
            || Ok(0.0f64),
            0.0,
            1.0,
        )
        .expect_err("should fail with non-positive h or k");
        match err_h {
            finstack_core::Error::Input(finstack_core::InputError::NonPositiveValue) => {}
            e => panic!("unexpected error: {e:?}"),
        }
        let err_k = central_mixed(
            || Ok(0.0f64),
            || Ok(0.0f64),
            || Ok(0.0f64),
            || Ok(0.0f64),
            1.0,
            0.0,
        )
        .expect_err("should fail with non-positive h or k");
        match err_k {
            finstack_core::Error::Input(finstack_core::InputError::NonPositiveValue) => {}
            e => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn central_mixed_computes_known_cross_derivative() {
        let h = 0.01;
        let k = 0.01;
        let result = central_mixed(
            || Ok((1.0 + h) * (1.0 + k)),
            || Ok((1.0 + h) * (1.0 - k)),
            || Ok((1.0 - h) * (1.0 + k)),
            || Ok((1.0 - h) * (1.0 - k)),
            h,
            k,
        )
        .expect("central mixed derivative should succeed");

        assert!(
            (result - 1.0).abs() < 1e-10,
            "cross derivative of x*y should be 1.0, got {result}"
        );
    }

    #[test]
    fn scalar_helpers_preserve_variant_and_currency() {
        let price_scalar = MarketScalar::Price(Money::new(99.0, Currency::USD));
        let rebuilt = scalar_with_numeric_value(&price_scalar, 101.0);
        match rebuilt {
            MarketScalar::Price(money) => {
                assert_eq!(money.currency(), Currency::USD);
                assert_eq!(money.amount(), 101.0);
            }
            MarketScalar::Unitless(_) => panic!("expected price scalar"),
        }

        let unitless = MarketScalar::Unitless(0.02);
        let rebuilt = scalar_with_numeric_value(&unitless, 0.03);
        assert_eq!(scalar_numeric_value(&rebuilt), 0.03);
    }

    #[test]
    fn replace_scalar_value_updates_only_requested_quote() {
        let market = MarketContext::new()
            .insert_price("A", MarketScalar::Unitless(1.0))
            .insert_price("B", MarketScalar::Price(Money::new(2.0, Currency::USD)));
        let current = market.get_price("B").expect("quote should exist");
        let bumped = replace_scalar_value(&market, "B", current, 3.5);

        assert_eq!(
            scalar_numeric_value(bumped.get_price("A").expect("quote should exist")),
            1.0
        );
        assert_eq!(
            scalar_numeric_value(bumped.get_price("B").expect("quote should exist")),
            3.5
        );
    }

    #[test]
    fn central_difference_helpers_error_on_degenerate_width() {
        // A degenerate bump width must surface as an `Err`, never a silent
        // `0.0` (which is indistinguishable from a true zero Greek).
        for err in [
            central_diff_by_width(2.0, 1.0, 0.0).expect_err("zero width must error"),
            central_diff_by_width(2.0, 1.0, 1e-13).expect_err("sub-tolerance width must error"),
            central_diff_by_width(2.0, 1.0, f64::NAN).expect_err("non-finite width must error"),
            central_diff_by_half_bump(2.0, 1.0, 0.0).expect_err("zero half-bump must error"),
            scaled_central_diff_by_width(2.0, 1.0, 0.0, 1e-4).expect_err("zero width must error"),
        ] {
            match err {
                finstack_core::Error::Input(finstack_core::InputError::NonPositiveValue) => {}
                e => panic!("unexpected error variant for degenerate width: {e:?}"),
            }
        }
    }

    #[test]
    fn central_difference_helpers_compute_known_values() {
        assert_eq!(
            central_diff_by_width(3.0, 1.0, 4.0).expect("non-degenerate width"),
            0.5
        );
        assert_eq!(
            central_diff_by_half_bump(3.0, 1.0, 0.5).expect("non-degenerate half-bump"),
            2.0
        );
        assert_eq!(
            scaled_central_diff_by_width(3.0, 1.0, 4.0, 0.5).expect("non-degenerate width"),
            0.25
        );
    }
}
