//! Generic finite difference greek calculators for equity instruments.
//!
//! Provides reusable implementations of Delta, Gamma, and Vega calculators
//! that work with any instrument implementing the required traits.
//!
//! This eliminates code duplication across exotic options (AsianOption, Autocallable,
//! BarrierOption, LookbackOption, etc.) that all use the same finite difference pattern.
//!
//! # Numerical Stability
//!
//! These calculators implement guards against numerical instability:
//! - **Minimum bump size**: Absolute bump sizes are floored at [`MIN_ABSOLUTE_BUMP`] to
//!   prevent division by zero or explosive greeks at low spot levels.
//! - **Common random numbers (CRN)**: For Monte Carlo priced instruments, all bump
//!   scenarios use the same seed ("greeks_crn") to ensure variance reduction and
//!   stable finite differences.
//! - **Non-positive spot validation**: Returns an error if spot price is non-positive.

use std::marker::PhantomData;

use crate::instruments::common_impl::traits::{EquityDependencies, Instrument};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

/// Minimum absolute bump size for spot-based finite differences.
///
/// This floor prevents division by zero or numerically unstable greeks when
/// the underlying spot price is very small. The denominator in finite differences
/// scales with this bump size, so an excessively small bump leads to explosive greeks.
///
/// # Value Rationale
///
/// The value 1e-8 is chosen as a balance:
/// - **Equity prices** (typically $1-$10,000): 1% bump on a $0.01 stock = $0.0001,
///   well above 1e-8, so the floor rarely activates for normal equities.
/// - **FX rates** (typically 0.5-200): 1% bump on a 0.01 rate = 1e-4, still safe.
/// - **Fractional shares or near-zero prices**: The floor activates to prevent
///   instability when spot × bump_pct would be dangerously small.
///
/// # Limitations
///
/// This is a conservative fixed floor. For instruments with atypical price scales
/// (e.g., cryptocurrencies with 8+ decimal places, or very large notionals), users
/// may need to adjust bump percentages in `FinstackConfig` rather than relying on
/// this floor.
pub const MIN_ABSOLUTE_BUMP: f64 = 1e-8;

const VOL_POINTS_PER_ABSOLUTE_VOL: f64 = 100.0;

/// Common random number seed scenario for MC greek calculations.
///
/// Using the same seed for all bump scenarios (up/down/base) ensures that
/// Monte Carlo noise cancels in finite differences, providing stable greeks.
/// This is the standard "common random numbers" (CRN) variance reduction technique.
const CRN_SEED_SCENARIO: &str = "greeks_crn";

/// Validate that spot price is positive and finite.
///
/// Returns an error if the spot is non-positive, NaN, or infinite.
fn validate_spot(spot: f64, greek_name: &str) -> Result<()> {
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "Non-positive or invalid spot price ({}) for {} calculation. \
             Spot must be positive and finite.",
            spot, greek_name
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SpotBump {
    relative: f64,
    absolute: f64,
}

/// Compute a spot bump whose applied relative shock matches the absolute denominator.
///
/// Returns the configured relative bump unless its absolute spot move is smaller
/// than [`MIN_ABSOLUTE_BUMP`]. In that case the relative bump is increased so
/// the market scenario and finite-difference denominator use the same move.
#[inline]
fn effective_spot_bump(spot: f64, bump_pct: f64) -> SpotBump {
    let signed_relative = bump_pct.signum() * (spot * bump_pct).abs().max(MIN_ABSOLUTE_BUMP) / spot;
    SpotBump {
        relative: signed_relative,
        absolute: (spot * signed_relative).abs(),
    }
}

/// Smallest implied vol on a surface's `(expiry, strike)` grid.
///
/// An additive parallel vol bump of size `h` clamps the down-bumped surface
/// (`σ - h`) at zero wherever `σ < h` (see `VolSurface`'s `Bumpable` impl).
/// When that happens the down-bump no longer represents a `-h` move, so a
/// central difference divided by the full `2h` is wrong. Callers use this
/// minimum to detect the clamp and fall back to a one-sided difference.
///
/// Returns `None` for an empty grid.
fn min_surface_vol(surface: &finstack_core::market_data::surfaces::VolSurface) -> Option<f64> {
    let mut min_vol: Option<f64> = None;
    for &expiry in surface.expiries() {
        for &strike in surface.strikes() {
            // Exact grid points evaluate to the stored vol with no interpolation.
            if let Ok(vol) = surface.value_checked(expiry, strike) {
                min_vol = Some(min_vol.map_or(vol, |m: f64| m.min(vol)));
            }
        }
    }
    min_vol
}

/// Guard against NaN / ±Inf leaking out of finite-difference calculations.
fn ensure_finite(value: f64, metric_name: &str) -> finstack_core::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(finstack_core::Error::Validation(format!(
            "{metric_name} produced non-finite result: {value}"
        )))
    }
}

fn clone_with_crn_seed<I>(instrument: &I) -> I
where
    I: Clone + HasPricingOverrides,
{
    let mut seeded = instrument.clone();
    <I as HasPricingOverrides>::pricing_overrides_mut(&mut seeded)
        .metrics
        .mc_seed_scenario = Some(CRN_SEED_SCENARIO.to_string());
    seeded
}

fn eval_raw_with_scratch_bumps<I>(
    context: &MetricContext,
    scratch: &mut MarketContext,
    instrument: &I,
    as_of: Date,
    spot_bump: Option<(&str, f64)>,
    vol_bump: Option<(&str, f64)>,
) -> Result<f64>
where
    I: Instrument,
{
    use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits};

    let price_token = if let Some((spot_id, bump_pct)) = spot_bump {
        Some(scratch.apply_price_bump_pct_in_place(spot_id, bump_pct)?)
    } else {
        None
    };

    let surface_token = match vol_bump {
        Some((surface_id, bump_abs)) => {
            let spec = BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: bump_abs,
                bump_type: BumpType::Parallel,
            };
            match scratch.apply_surface_bump_in_place(surface_id, spec) {
                Ok(token) => Some(token),
                Err(err) => {
                    if let Some(token) = price_token {
                        scratch.revert_scratch_bump(token)?;
                    }
                    return Err(err);
                }
            }
        }
        None => None,
    };

    let value = context.reprice_instrument_raw(instrument, scratch, as_of);
    if let Some(token) = surface_token {
        scratch.revert_scratch_bump(token)?;
    }
    if let Some(token) = price_token {
        scratch.revert_scratch_bump(token)?;
    }
    value
}

fn with_market_scratch<T>(
    context: &mut MetricContext,
    f: impl FnOnce(&MetricContext, &mut MarketContext) -> Result<T>,
) -> Result<T> {
    context.with_market_scratch(f)
}

// ================================================================================================
// Traits for Instruments with Expiry and DayCount Information
// ================================================================================================

/// Trait for instruments that have an expiry date.
///
/// Used for adaptive bump size calculations based on time to expiry.
/// Instruments with shorter time to expiry typically require smaller bumps
/// to maintain numerical accuracy in finite difference calculations.
///
/// # Examples
///
/// Implementing for a custom option:
///
/// ```rust,ignore
/// use finstack_valuations::metrics::HasExpiry;
/// use finstack_core::dates::Date;
///
/// struct CustomOption {
///     expiry: Date,
///     // ... other fields
/// }
///
/// impl HasExpiry for CustomOption {
///     fn expiry(&self) -> Date {
///         self.expiry
///     }
/// }
/// ```
pub trait HasExpiry {
    /// Returns the expiry date for this instrument.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use finstack_valuations::metrics::HasExpiry;
    /// use finstack_core::dates::Date;
    ///
    /// # let instrument: &dyn HasExpiry = todo!("an instrument that has an expiry date");
    /// let expiry_date: Date = instrument.expiry();
    /// # let _ = expiry_date;
    /// ```
    fn expiry(&self) -> Date;
}

/// Trait for instruments that have a day count convention.
///
/// Used for computing time fractions in adaptive bump calculations.
/// The day count convention determines how time between dates is measured,
/// affecting the calculation of year fractions for time-to-expiry.
///
/// # Examples
///
/// Implementing for a custom option:
///
/// ```rust,ignore
/// use finstack_valuations::metrics::HasDayCount;
/// use finstack_core::dates::DayCount;
///
/// struct CustomOption {
///     day_count: DayCount,
///     // ... other fields
/// }
///
/// impl HasDayCount for CustomOption {
///     fn day_count(&self) -> DayCount {
///         self.day_count
///     }
/// }
/// ```
pub trait HasDayCount {
    /// Returns the day count convention for this instrument.
    ///
    /// Common conventions include Act/365F, Act/360, 30/360, etc.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use finstack_valuations::metrics::HasDayCount;
    /// use finstack_core::dates::DayCount;
    ///
    /// # let instrument: &dyn HasDayCount = todo!("an instrument that specifies a day count");
    /// let day_count: DayCount = instrument.day_count();
    /// # let _ = day_count;
    /// ```
    fn day_count(&self) -> DayCount;
}

/// Trait for instruments that have pricing overrides.
///
/// This trait allows generic metric calculators to set MC seed scenarios
/// and other pricing overrides for deterministic greek calculations.
/// Only implemented by instruments that use Monte Carlo pricing.
///
/// # Purpose
///
/// For Monte Carlo priced instruments, setting different random seeds for
/// up/down bumps ensures deterministic and numerically stable greeks.
/// Without this, random variation would contaminate the finite difference
/// calculations.
///
/// # Examples
///
/// Implementing for a Monte Carlo option:
///
/// ```rust,ignore
/// use finstack_valuations::metrics::HasPricingOverrides;
/// use finstack_valuations::instruments::PricingOverrides;
///
/// struct McOption {
///     overrides: PricingOverrides,
///     // ... other fields
/// }
///
/// impl HasPricingOverrides for McOption {
///     fn pricing_overrides_mut(&mut self) -> &mut PricingOverrides {
///         &mut self.overrides
///     }
/// }
/// ```
pub trait HasPricingOverrides {
    /// Returns mutable access to pricing overrides.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use finstack_valuations::metrics::HasPricingOverrides;
    ///
    /// // Set deterministic MC seed for greek calculation
    /// # let instrument: &mut dyn HasPricingOverrides = todo!("a Monte Carlo priced instrument");
    /// instrument.pricing_overrides_mut().metrics.mc_seed_scenario = Some("delta_up".to_string());
    /// ```
    fn pricing_overrides_mut(&mut self) -> &mut crate::instruments::PricingOverrides;
}

// ================================================================================================
// Generic FD Greeks Calculators
// ================================================================================================

/// Generic delta calculator using finite differences.
///
/// Calculates delta (price sensitivity to underlying spot) using the central
/// finite difference method. Works with any instrument that implements the
/// required traits: [`Instrument`], [`EquityDependencies`], [`HasExpiry`],
/// [`HasDayCount`], and [`HasPricingOverrides`].
///
/// # Mathematical Foundation
///
/// Delta is computed using the central finite difference formula:
/// ```text
/// Δ = (PV(S + h) - PV(S - h)) / (2h)
/// ```
/// where `S` is the spot price and `h` is the bump size.
///
/// # Type Parameters
///
/// * `I` - Instrument type that implements all required traits for delta calculation
///
/// # Examples
///
/// ```rust,ignore
/// use finstack_valuations::instruments::BarrierOption;
/// use finstack_valuations::metrics::{GenericFdDelta, MetricId, MetricRegistry};
/// use finstack_valuations::pricer::InstrumentType;
/// use std::sync::Arc;
///
/// // Create delta calculator for barrier options (generic FD greeks apply to exotics)
/// let calculator = GenericFdDelta::<BarrierOption>::default();
///
/// // Register in metric registry
/// let mut registry = MetricRegistry::new();
/// registry.register_metric(
///     MetricId::Delta,
///     Arc::new(calculator),
///     &[InstrumentType::BarrierOption],
/// );
/// ```
pub struct GenericFdDelta<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericFdDelta<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericFdDelta<I>
where
    I: Instrument
        + EquityDependencies
        + HasExpiry
        + HasDayCount
        + HasPricingOverrides
        + Clone
        + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        // Get equity dependencies
        let eq_deps = instrument.equity_dependencies()?;
        let spot_id = eq_deps.spot_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Instrument missing spot_id for delta calculation".to_string(),
            )
        })?;

        // Get current spot for bump size calculation
        let spot_scalar = context.curves.get_price(spot_id)?;
        let current_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Validate spot is positive and finite
        validate_spot(current_spot, "delta")?;

        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        let bump_pct = defaults.spot_bump_pct;

        // Use the same effective bump in the market and denominator.
        let bump = effective_spot_bump(current_spot, bump_pct);

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(instrument);

        let (pv_up, pv_down) = with_market_scratch(context, |context, scratch| {
            let pv_up = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, bump.relative)),
                None,
            )?;
            let pv_down = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, -bump.relative)),
                None,
            )?;
            Ok((pv_up, pv_down))
        })?;

        // Central difference: delta = (PV_up - PV_down) / (2 * bump_size).
        // Route through the guarded helper so a degenerate bump width is
        // surfaced as an error rather than producing a silent NaN/∞.
        let delta = crate::metrics::core::finite_difference::central_diff_by_width(
            pv_up,
            pv_down,
            2.0 * bump.absolute,
        )?;

        ensure_finite(delta, "fd_delta")
    }
}

/// Generic gamma calculator using finite differences.
///
/// Computes gamma as the second derivative with respect to spot price:
/// Gamma = [Delta(spot + bump) - Delta(spot - bump)] / (2 * bump_size)
pub struct GenericFdGamma<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericFdGamma<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericFdGamma<I>
where
    I: Instrument
        + EquityDependencies
        + HasExpiry
        + HasDayCount
        + HasPricingOverrides
        + Clone
        + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        // Get equity dependencies
        let eq_deps = instrument.equity_dependencies()?;
        let spot_id = eq_deps.spot_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Instrument missing spot_id for gamma calculation".to_string(),
            )
        })?;

        // Get current spot for bump size calculation
        let spot_scalar = context.curves.get_price(spot_id)?;
        let current_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Validate spot is positive and finite
        validate_spot(current_spot, "gamma")?;

        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        let bump_pct = defaults.spot_bump_pct;

        // Use the same effective bump in the market and denominator.
        let bump = effective_spot_bump(current_spot, bump_pct);

        // Use the 3-point central difference formula directly:
        //
        //   Γ ≈ (PV(S+h) - 2 PV(S) + PV(S-h)) / h²
        //
        // This avoids "bump-of-bump" scaling artifacts when bumps are applied multiplicatively
        // (percentage bumps) and yields exact results for quadratic payoffs.

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(instrument);
        let (base_pv, pv_up, pv_down) = with_market_scratch(context, |context, scratch| {
            let base_pv = context.reprice_instrument_raw(&seeded_instrument, scratch, as_of)?;
            let pv_up = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, bump.relative)),
                None,
            )?;
            let pv_down = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, -bump.relative)),
                None,
            )?;
            Ok((base_pv, pv_up, pv_down))
        })?;

        let gamma = (pv_up - 2.0 * base_pv + pv_down) / (bump.absolute * bump.absolute);

        ensure_finite(gamma, "fd_gamma")
    }
}

/// Generic vega calculator using finite differences on a volatility surface.
///
/// Works with any instrument that has an associated volatility surface.
pub struct GenericFdVega<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericFdVega<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericFdVega<I>
where
    I: Instrument + EquityDependencies + HasPricingOverrides + Clone + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        // Get equity dependencies
        let eq_deps = instrument.equity_dependencies()?;

        // Get vol surface id from instrument - error if missing
        let vol_surface_id = eq_deps.vol_surface_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "Instrument {} missing vol_surface_id for vega calculation. \
                 Cannot compute vega without a volatility surface.",
                instrument.id()
            ))
        })?;

        // Verify the vol surface exists in the market context
        let surface = match context.curves.get_surface(vol_surface_id.as_str()) {
            Ok(surface) => surface,
            Err(_) => {
                return Err(finstack_core::Error::from(
                    finstack_core::InputError::NotFound {
                        id: format!("vol_surface:{}", vol_surface_id),
                    },
                ))
            }
        };

        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        // Interpreted as an **absolute** implied vol bump in decimal units (e.g., 0.01 = +1 vol point).
        let bump_abs = defaults.vol_bump_pct;

        let seeded_instrument = clone_with_crn_seed(instrument);
        let instrument_id = instrument.id().to_string();

        // The additive down-bump `σ - h` is clamped at zero wherever `σ < h`
        // (VolSurface `Bumpable`). When that clamp triggers, the down-bumped
        // surface does NOT represent a uniform `-h` move, so a central
        // difference divided by the full `2h` is asymmetric and biased. Detect
        // the clamp and fall back to a one-sided (forward) difference with the
        // correct `h` divisor near zero vol.
        let min_vol = min_surface_vol(&surface);
        let clamp_active = min_vol.map(|m| m < bump_abs).unwrap_or(false);

        let vega = if clamp_active {
            tracing::warn!(
                instrument_id = %instrument_id,
                surface_id = %vol_surface_id,
                min_vol = min_vol,
                bump_abs = bump_abs,
                "vega down-bump would clamp σ at 0; using one-sided forward difference"
            );
            with_market_scratch(context, |context, scratch| {
                let pv_up = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    &seeded_instrument,
                    as_of,
                    None,
                    Some((vol_surface_id.as_str(), bump_abs)),
                )?;
                let pv_base = context.reprice_instrument_raw(&seeded_instrument, scratch, as_of)?;
                // Forward difference: (PV(σ+h) - PV(σ)) / h.
                crate::metrics::core::finite_difference::central_diff_by_width(
                    pv_up,
                    pv_base,
                    bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL,
                )
            })?
        } else {
            with_market_scratch(context, |context, scratch| {
                let pv_up = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    &seeded_instrument,
                    as_of,
                    None,
                    Some((vol_surface_id.as_str(), bump_abs)),
                )?;
                let pv_down = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    &seeded_instrument,
                    as_of,
                    None,
                    Some((vol_surface_id.as_str(), -bump_abs)),
                )?;
                // Central difference: (PV(σ+h) - PV(σ-h)) / 2h.
                crate::metrics::core::finite_difference::central_diff_by_width(
                    pv_up,
                    pv_down,
                    2.0 * bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL,
                )
            })?
        };

        // MetricId::Vega is reported per 1 vol point (0.01 absolute vol), not per 1.00 vol.
        ensure_finite(vega, "fd_vega")
    }
}

/// Generic volga (∂²V/∂σ²) via central differences on a volatility surface.
pub struct GenericFdVolga<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericFdVolga<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericFdVolga<I>
where
    I: Instrument
        + EquityDependencies
        + HasExpiry
        + HasDayCount
        + HasPricingOverrides
        + Clone
        + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        // If expired, volga is zero — mirror vanna's guard. On an expired
        // option, vega ≈ 0, so volga = (vega(σ+h) - vega(σ-h))/h^2 amplifies
        // near-zero noise into NaN/garbage.
        let t = instrument.day_count().year_fraction(
            as_of,
            HasExpiry::expiry(instrument),
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Get equity dependencies
        let eq_deps = instrument.equity_dependencies()?;

        // Get vol surface id from instrument - error if missing
        let vol_surface_id = eq_deps.vol_surface_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "Instrument {} missing vol_surface_id for volga calculation. \
                 Cannot compute volga without a volatility surface.",
                instrument.id()
            ))
        })?;

        // Verify the vol surface exists in the market context
        if context.curves.get_surface(vol_surface_id.as_str()).is_err() {
            return Err(finstack_core::Error::from(
                finstack_core::InputError::NotFound {
                    id: format!("vol_surface:{}", vol_surface_id),
                },
            ));
        }

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(instrument);

        // Absolute implied vol bump (vol points).
        let bump_abs = defaults.vol_bump_pct;

        let (base_pv, pv_up, pv_down) = with_market_scratch(context, |context, scratch| {
            let base_pv = context.reprice_instrument_raw(&seeded_instrument, scratch, as_of)?;
            let pv_up = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                None,
                Some((vol_surface_id.as_str(), bump_abs)),
            )?;
            let pv_down = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                None,
                Some((vol_surface_id.as_str(), -bump_abs)),
            )?;
            Ok((base_pv, pv_up, pv_down))
        })?;

        let volga = (pv_up - 2.0 * base_pv + pv_down) / (bump_abs * bump_abs);

        ensure_finite(volga, "fd_volga")
    }
}

/// Generic vanna (∂²V/∂S∂σ) via central mixed differences on spot and vol.
pub struct GenericFdVanna<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for GenericFdVanna<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for GenericFdVanna<I>
where
    I: Instrument
        + EquityDependencies
        + HasExpiry
        + HasDayCount
        + HasPricingOverrides
        + Clone
        + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        // If expired, vanna is zero (avoid bumping / repricing beyond expiry).
        let t = instrument.day_count().year_fraction(
            as_of,
            HasExpiry::expiry(instrument),
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Get equity dependencies
        let eq_deps = instrument.equity_dependencies()?;
        let spot_id = eq_deps.spot_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Instrument missing spot_id for vanna calculation".to_string(),
            )
        })?;
        let vol_surface_id = eq_deps.vol_surface_id.as_ref().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Instrument missing vol_surface_id for vanna calculation".to_string(),
            )
        })?;

        // Spot level for bump sizing
        let spot_scalar = context.curves.get_price(spot_id)?;
        let current_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Validate spot is positive and finite
        validate_spot(current_spot, "vanna")?;

        // Bump sizes with minimum floor for numerical stability
        let (spot_bump_pct, vol_bump_abs) = (defaults.spot_bump_pct, defaults.vol_bump_pct);

        let spot_bump = effective_spot_bump(current_spot, spot_bump_pct);
        let h_abs = spot_bump.absolute; // absolute spot change
        let k_abs = vol_bump_abs; // absolute vol change (vol points)

        let seeded_instrument = clone_with_crn_seed(instrument);

        let (v_pp, v_pm, v_mp, v_mm) = with_market_scratch(context, |context, scratch| {
            let v_pp = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, spot_bump.relative)),
                Some((vol_surface_id.as_str(), k_abs)),
            )?;
            let v_pm = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, spot_bump.relative)),
                Some((vol_surface_id.as_str(), -k_abs)),
            )?;
            let v_mp = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, -spot_bump.relative)),
                Some((vol_surface_id.as_str(), k_abs)),
            )?;
            let v_mm = eval_raw_with_scratch_bumps(
                context,
                scratch,
                &seeded_instrument,
                as_of,
                Some((spot_id, -spot_bump.relative)),
                Some((vol_surface_id.as_str(), -k_abs)),
            )?;
            Ok((v_pp, v_pm, v_mp, v_mm))
        })?;

        let vanna = (v_pp - v_pm - v_mp + v_mm) / (4.0 * h_abs * k_abs);

        ensure_finite(vanna, "fd_vanna")
    }
}

// ================================================================================================
// Unit tests (internal)
// ================================================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, dead_code)]
mod tests {
    use super::*;

    use crate::instruments::common_impl::traits::{
        Attributes, EquityDependencies, EquityInstrumentDeps,
    };
    use crate::instruments::PricingOverrides;
    use crate::metrics::{MetricContext, MetricId, MetricRegistry};
    use crate::pricer::InstrumentType;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::money::Money;
    use finstack_core::types::PriceId;
    use std::sync::Arc;
    use time::macros::date;

    #[allow(dead_code)]
    #[derive(Clone)]
    struct TestFdInstrument {
        id: String,
        expiry: Date,
        day_count: DayCount,
        spot_id: PriceId,
        overrides: PricingOverrides,
        attributes: Attributes,
    }

    impl TestFdInstrument {
        fn new(id: &str, expiry: Date, spot_id: &str) -> Self {
            Self {
                id: id.to_string(),
                expiry,
                day_count: DayCount::Act365F,
                spot_id: spot_id.into(),
                overrides: PricingOverrides::default(),
                attributes: Attributes::new(),
            }
        }
    }

    #[allow(dead_code)]
    #[derive(Clone)]
    struct RoundingSensitiveInstrument {
        id: String,
        expiry: Date,
        day_count: DayCount,
        spot_id: PriceId,
        overrides: PricingOverrides,
        attributes: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        TestFdInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );
    crate::impl_empty_cashflow_provider!(
        RoundingSensitiveInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl RoundingSensitiveInstrument {
        fn new(id: &str, expiry: Date, spot_id: &str) -> Self {
            Self {
                id: id.to_string(),
                expiry,
                day_count: DayCount::Act365F,
                spot_id: spot_id.into(),
                overrides: PricingOverrides::default(),
                attributes: Attributes::new(),
            }
        }

        fn raw_pv(&self, market: &MarketContext) -> finstack_core::Result<f64> {
            let spot_scalar = market.get_price(self.spot_id.as_str())?;
            let spot = match spot_scalar {
                MarketScalar::Price(m) => m.amount(),
                MarketScalar::Unitless(v) => *v,
            };
            Ok(spot * 0.00003)
        }
    }

    impl EquityDependencies for TestFdInstrument {
        fn equity_dependencies(&self) -> finstack_core::Result<EquityInstrumentDeps> {
            EquityInstrumentDeps::builder()
                .spot(self.spot_id.as_str().to_string())
                .build()
        }
    }

    impl HasExpiry for TestFdInstrument {
        fn expiry(&self) -> Date {
            self.expiry
        }
    }

    impl HasDayCount for TestFdInstrument {
        fn day_count(&self) -> DayCount {
            self.day_count
        }
    }

    impl HasPricingOverrides for TestFdInstrument {
        fn pricing_overrides_mut(&mut self) -> &mut PricingOverrides {
            &mut self.overrides
        }
    }

    impl EquityDependencies for RoundingSensitiveInstrument {
        fn equity_dependencies(&self) -> finstack_core::Result<EquityInstrumentDeps> {
            EquityInstrumentDeps::builder()
                .spot(self.spot_id.as_str().to_string())
                .build()
        }
    }

    impl HasExpiry for RoundingSensitiveInstrument {
        fn expiry(&self) -> Date {
            self.expiry
        }
    }

    impl HasDayCount for RoundingSensitiveInstrument {
        fn day_count(&self) -> DayCount {
            self.day_count
        }
    }

    impl HasPricingOverrides for RoundingSensitiveInstrument {
        fn pricing_overrides_mut(&mut self) -> &mut PricingOverrides {
            &mut self.overrides
        }
    }

    impl crate::instruments::common_impl::traits::Instrument for TestFdInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Equity
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn clone_box(&self) -> Box<dyn crate::instruments::common_impl::traits::Instrument> {
            Box::new(self.clone())
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn base_value(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
            if as_of >= self.expiry {
                return Ok(Money::new(0.0, Currency::USD));
            }
            let spot_scalar = market.get_price(self.spot_id.as_str())?;
            let spot = match spot_scalar {
                MarketScalar::Price(m) => m.amount(),
                MarketScalar::Unitless(v) => *v,
            };
            // Simple analytic PV = S^2 (currency USD)
            Ok(Money::new(spot * spot, Currency::USD))
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_core::Result<crate::results::ValuationResult> {
            let base_value = self.value(market, as_of)?;
            crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                Arc::from(self.clone_box()),
                Arc::new(market.clone()),
                as_of,
                base_value,
                metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg: options.config,
                    market_history: options.market_history,
                    ..crate::instruments::common_impl::helpers::MetricBuildOptions::default()
                },
            )
        }
    }

    impl crate::instruments::common_impl::traits::Instrument for RoundingSensitiveInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Equity
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn clone_box(&self) -> Box<dyn crate::instruments::common_impl::traits::Instrument> {
            Box::new(self.clone())
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
            // This intentionally rounds away most of the PV to exercise the raw path
            let raw = self.raw_pv(market)?;
            Ok(Money::new(raw, Currency::USD))
        }

        fn value_raw(&self, market: &MarketContext, _as_of: Date) -> finstack_core::Result<f64> {
            self.raw_pv(market)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_core::Result<crate::results::ValuationResult> {
            let base_value = self.value(market, as_of)?;
            crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                Arc::from(self.clone_box()),
                Arc::new(market.clone()),
                as_of,
                base_value,
                metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg: options.config,
                    market_history: options.market_history,
                    ..crate::instruments::common_impl::helpers::MetricBuildOptions::default()
                },
            )
        }
    }

    fn registry_for_test<I>() -> MetricRegistry
    where
        I: crate::instruments::common_impl::traits::Instrument
            + EquityDependencies
            + HasExpiry
            + HasDayCount
            + HasPricingOverrides
            + Clone
            + 'static,
    {
        let mut registry = MetricRegistry::new();
        registry.register_metric(
            MetricId::Delta,
            Arc::new(GenericFdDelta::<I>::default()),
            &[InstrumentType::Equity],
        );
        registry.register_metric(
            MetricId::Gamma,
            Arc::new(GenericFdGamma::<I>::default()),
            &[InstrumentType::Equity],
        );
        registry
    }

    fn market_with_spot(spot_id: &str, price: f64) -> MarketContext {
        MarketContext::new().insert_price(
            spot_id,
            MarketScalar::Price(Money::new(price, Currency::USD)),
        )
    }

    #[test]
    fn fd_delta_matches_analytic_for_quadratic_pv() {
        let as_of = date!(2025 - 01 - 01);
        let spot = 100.0;
        let inst = TestFdInstrument::new("FD-TEST", date!(2026 - 01 - 01), "SPOT");
        let market = market_with_spot("SPOT", spot);

        let base_value = inst.value(&market, as_of).expect("base pv");
        let registry = registry_for_test::<TestFdInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry
            .compute(&[MetricId::Delta], &mut ctx)
            .expect("delta");
        let delta = *result.get(&MetricId::Delta).expect("delta value");

        // For PV = S^2, dPV/dS = 2S.
        assert!((delta - 2.0 * spot).abs() < 1e-8, "delta mismatch");
    }

    #[test]
    fn fd_gamma_matches_analytic_for_quadratic_pv() {
        let as_of = date!(2025 - 01 - 01);
        let spot = 80.0;
        let inst = TestFdInstrument::new("FD-TEST", date!(2026 - 01 - 01), "SPOT");
        let market = market_with_spot("SPOT", spot);

        let base_value = inst.value(&market, as_of).expect("base pv");
        let registry = registry_for_test::<TestFdInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry
            .compute(&[MetricId::Gamma], &mut ctx)
            .expect("gamma");
        let gamma = *result.get(&MetricId::Gamma).expect("gamma value");

        // For PV = S^2, d2PV/dS2 = 2.
        assert!((gamma - 2.0).abs() < 1e-8, "gamma mismatch");
    }

    #[test]
    fn delta_uses_raw_value_when_money_rounds_down() {
        let as_of = date!(2025 - 01 - 01);
        let spot = 100.0;
        let inst = RoundingSensitiveInstrument::new("ROUNDING", date!(2026 - 01 - 01), "SENS");
        let market = market_with_spot("SENS", spot);

        // Rounded Money path remains tiny, while the raw path preserves the
        // sensitivity needed for finite differences.
        let base_value = inst.value(&market, as_of).expect("base pv");
        assert!(
            base_value.amount().abs() < 0.01,
            "rounded Money should remain tiny"
        );

        let registry = registry_for_test::<RoundingSensitiveInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry
            .compute(&[MetricId::Delta], &mut ctx)
            .expect("delta");
        let delta = *result.get(&MetricId::Delta).expect("delta value");

        // Raw path preserves sensitivity: raw PV = spot * 0.00003, so delta ≈ 3e-5
        assert!(
            (delta - 0.00003).abs() < 1e-8,
            "delta should reflect raw pv"
        );
    }

    #[test]
    fn fd_greeks_zero_when_expired() {
        let as_of = date!(2027 - 01 - 02);
        let spot = 50.0;
        let inst = TestFdInstrument::new("FD-TEST", date!(2027 - 01 - 01), "SPOT");
        let market = market_with_spot("SPOT", spot);

        let base_value = inst.value(&market, as_of).expect("base pv");
        assert_eq!(base_value.amount(), 0.0, "expired base pv should be zero");

        let registry = registry_for_test::<TestFdInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry
            .compute(&[MetricId::Delta, MetricId::Gamma], &mut ctx)
            .expect("greeks");
        assert_eq!(
            *result.get(&MetricId::Delta).expect("delta"),
            0.0,
            "expired delta should be zero"
        );
        assert_eq!(
            *result.get(&MetricId::Gamma).expect("gamma"),
            0.0,
            "expired gamma should be zero"
        );
    }

    #[test]
    fn delta_errors_on_zero_spot() {
        let as_of = date!(2025 - 01 - 01);
        let inst = TestFdInstrument::new("FD-TEST", date!(2026 - 01 - 01), "SPOT");
        let market = market_with_spot("SPOT", 0.0);

        let base_value = Money::new(0.0, Currency::USD);
        let registry = registry_for_test::<TestFdInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry.compute(&[MetricId::Delta], &mut ctx);
        assert!(result.is_err(), "delta should error on zero spot");
        let err_msg = result.expect_err("already asserted is_err").to_string();
        assert!(
            err_msg.contains("Non-positive") || err_msg.contains("invalid spot"),
            "error should mention non-positive spot: {}",
            err_msg
        );
    }

    #[test]
    fn gamma_errors_on_negative_spot() {
        let as_of = date!(2025 - 01 - 01);
        let inst = TestFdInstrument::new("FD-TEST", date!(2026 - 01 - 01), "SPOT");
        let market = market_with_spot("SPOT", -10.0);

        let base_value = Money::new(0.0, Currency::USD);
        let registry = registry_for_test::<TestFdInstrument>();
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry.compute(&[MetricId::Gamma], &mut ctx);
        assert!(result.is_err(), "gamma should error on negative spot");
    }

    #[test]
    fn spot_bump_applies_minimum_floor_to_applied_relative_bump() {
        // Very small spot should apply a relative bump that matches the floored denominator.
        let tiny_spot = 1e-12;
        let bump_pct = 0.01;
        let bump = super::effective_spot_bump(tiny_spot, bump_pct);

        // The market bump and finite-difference denominator should agree.
        assert_eq!(bump.absolute, super::MIN_ABSOLUTE_BUMP);
        assert_eq!(bump.relative, super::MIN_ABSOLUTE_BUMP / tiny_spot);
        assert!(
            (tiny_spot * bump.relative - bump.absolute).abs() < f64::EPSILON,
            "applied relative bump should reproduce absolute denominator"
        );
    }

    #[test]
    fn validate_spot_rejects_nan() {
        let result = super::validate_spot(f64::NAN, "test");
        assert!(result.is_err(), "NaN spot should be rejected");
    }

    #[test]
    fn validate_spot_rejects_infinity() {
        let result = super::validate_spot(f64::INFINITY, "test");
        assert!(result.is_err(), "infinite spot should be rejected");
    }

    #[test]
    fn validate_spot_accepts_positive_finite() {
        let result = super::validate_spot(100.0, "test");
        assert!(result.is_ok(), "positive finite spot should be accepted");
    }

    // ── Audit item #4: vega clamp on the down-bump ──────────────────────────

    /// Instrument whose PV is linear in the surface's ATM vol: `PV = K · σ`.
    /// Vega is therefore the constant `K` (per unit absolute vol).
    #[allow(dead_code)]
    #[derive(Clone)]
    struct VolLinearInstrument {
        id: String,
        expiry: Date,
        day_count: DayCount,
        spot_id: PriceId,
        vol_surface_id: String,
        slope: f64,
        overrides: PricingOverrides,
        attributes: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        VolLinearInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl VolLinearInstrument {
        fn new(id: &str, expiry: Date, spot_id: &str, vol_surface_id: &str, slope: f64) -> Self {
            Self {
                id: id.to_string(),
                expiry,
                day_count: DayCount::Act365F,
                spot_id: spot_id.into(),
                vol_surface_id: vol_surface_id.to_string(),
                slope,
                overrides: PricingOverrides::default(),
                attributes: Attributes::new(),
            }
        }

        fn raw_pv(&self, market: &MarketContext) -> finstack_core::Result<f64> {
            let surface = market.get_surface(self.vol_surface_id.as_str())?;
            // Sample the surface at its first grid point (ATM-ish): an exact
            // grid hit so `value_checked` returns the stored vol verbatim.
            let expiry = surface
                .expiries()
                .first()
                .copied()
                .ok_or_else(|| finstack_core::Error::Validation("empty expiries".into()))?;
            let strike = surface
                .strikes()
                .first()
                .copied()
                .ok_or_else(|| finstack_core::Error::Validation("empty strikes".into()))?;
            let sigma = surface.value_checked(expiry, strike)?;
            Ok(self.slope * sigma)
        }
    }

    impl EquityDependencies for VolLinearInstrument {
        fn equity_dependencies(&self) -> finstack_core::Result<EquityInstrumentDeps> {
            EquityInstrumentDeps::builder()
                .spot(self.spot_id.as_str().to_string())
                .vol_surface(self.vol_surface_id.clone())
                .build()
        }
    }

    impl HasExpiry for VolLinearInstrument {
        fn expiry(&self) -> Date {
            self.expiry
        }
    }

    impl HasDayCount for VolLinearInstrument {
        fn day_count(&self) -> DayCount {
            self.day_count
        }
    }

    impl HasPricingOverrides for VolLinearInstrument {
        fn pricing_overrides_mut(&mut self) -> &mut PricingOverrides {
            &mut self.overrides
        }
    }

    impl crate::instruments::common_impl::traits::Instrument for VolLinearInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Equity
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn clone_box(&self) -> Box<dyn crate::instruments::common_impl::traits::Instrument> {
            Box::new(self.clone())
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
            Ok(Money::new(self.raw_pv(market)?, Currency::USD))
        }

        fn value_raw(&self, market: &MarketContext, _as_of: Date) -> finstack_core::Result<f64> {
            self.raw_pv(market)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_core::Result<crate::results::ValuationResult> {
            let base_value = self.value(market, as_of)?;
            crate::instruments::common_impl::helpers::build_with_metrics_dyn(
                Arc::from(self.clone_box()),
                Arc::new(market.clone()),
                as_of,
                base_value,
                metrics,
                crate::instruments::common_impl::helpers::MetricBuildOptions {
                    cfg: options.config,
                    market_history: options.market_history,
                    ..crate::instruments::common_impl::helpers::MetricBuildOptions::default()
                },
            )
        }
    }

    /// Flat vol surface at the given (low) vol level.
    fn flat_vol_surface(id: &str, vol: f64) -> finstack_core::market_data::surfaces::VolSurface {
        finstack_core::market_data::surfaces::VolSurface::builder(id)
            .expiries(&[0.5, 1.0])
            .strikes(&[90.0, 100.0, 110.0])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .build()
            .expect("flat vol surface")
    }

    #[test]
    fn vega_uses_one_sided_difference_when_down_bump_clamps_at_zero() {
        // Surface min vol 0.005 is BELOW the 0.01 default vol bump, so the
        // additive down-bump `σ - h` clamps at 0. A central difference over
        // `2h` would mix a +0.01 up-move with a clamped −0.005 down-move and
        // understate vega; the fix detects the clamp and uses a forward
        // difference `(PV(σ+h) − PV(σ)) / h`.
        let as_of = date!(2025 - 01 - 01);
        let slope = 1_000_000.0; // PV = slope · σ.
        let inst =
            VolLinearInstrument::new("VEGA-CLAMP", date!(2026 - 01 - 01), "SPOT", "LOWVOL", slope);
        let market =
            market_with_spot("SPOT", 100.0).insert_surface(flat_vol_surface("LOWVOL", 0.005));

        let base_value = inst.value(&market, as_of).expect("base pv");
        let mut registry = MetricRegistry::new();
        registry.register_metric(
            MetricId::Vega,
            Arc::new(GenericFdVega::<VolLinearInstrument>::default()),
            &[InstrumentType::Equity],
        );
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry.compute(&[MetricId::Vega], &mut ctx).expect("vega");
        let vega = *result.get(&MetricId::Vega).expect("vega value");

        // PV = slope·σ ⇒ dPV/dσ = slope. Vega is reported per 1 vol point
        // (0.01 absolute vol): expected = slope · 0.01 = 10_000.
        let expected = slope * 0.01;
        assert!(
            (vega - expected).abs() < 1e-6,
            "clamp-aware forward difference must recover the true vega {expected}, got {vega}"
        );
        // The contaminated central difference would have produced
        // slope·0.015 / 2 = 7_500 — assert we are NOT near that wrong value.
        assert!(
            (vega - slope * 0.0075).abs() > 1.0,
            "vega must not equal the contaminated central-difference value"
        );
    }

    #[test]
    fn vega_uses_central_difference_when_no_clamp() {
        // Surface min vol 0.20 is well above the 0.01 bump: standard central
        // difference path, still exact for a PV linear in σ.
        let as_of = date!(2025 - 01 - 01);
        let slope = 500_000.0;
        let inst = VolLinearInstrument::new(
            "VEGA-NOCLAMP",
            date!(2026 - 01 - 01),
            "SPOT",
            "HIVOL",
            slope,
        );
        let market =
            market_with_spot("SPOT", 100.0).insert_surface(flat_vol_surface("HIVOL", 0.20));

        let base_value = inst.value(&market, as_of).expect("base pv");
        let mut registry = MetricRegistry::new();
        registry.register_metric(
            MetricId::Vega,
            Arc::new(GenericFdVega::<VolLinearInstrument>::default()),
            &[InstrumentType::Equity],
        );
        let mut ctx = MetricContext::new(
            Arc::new(inst),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let result = registry.compute(&[MetricId::Vega], &mut ctx).expect("vega");
        let vega = *result.get(&MetricId::Vega).expect("vega value");
        let expected = slope * 0.01;
        assert!(
            (vega - expected).abs() < 1e-6,
            "central difference must recover the true vega {expected}, got {vega}"
        );
    }
}
