//! Utilities for instrument pricing and metrics assembly.
//!
//! Contains helpers shared across instrument implementations, notably metric
//! context construction and deterministic measure assembly.

use crate::metrics::risk::MarketHistory;
use crate::metrics::{standard_registry, MetricContext, MetricId};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::{
    context::MarketContext,
    scalars::{InflationIndex, InflationInterpolation, MarketScalar},
};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::sync::Arc;

/// Validated pricing boundary shared by direct and registry-backed routes.
///
/// Construction performs the only mandatory invariant/override validation
/// step. Callers may then resolve the effective date, run their chosen model
/// kernel, and apply the instrument-owned scenario adjustment exactly once.
pub(crate) struct ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::common_impl::traits::Instrument + ?Sized,
{
    instrument: &'a I,
}

impl<'a, I> ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::common_impl::traits::Instrument + ?Sized,
{
    /// Validate the complete instrument pricing boundary.
    pub(crate) fn new(instrument: &'a I) -> finstack_quant_core::Result<Self> {
        instrument.validate_for_pricing()?;
        Ok(Self { instrument })
    }

    /// Resolve the instrument's effective valuation date after validation.
    pub(crate) fn effective_as_of(&self, market: &MarketContext, requested: Date) -> Date {
        self.instrument.resolve_pricing_as_of(market, requested)
    }

    /// Apply the instrument scenario to a model `Money` result exactly once.
    pub(crate) fn apply_value(&self, base_value: Money) -> Money {
        apply_scenario_value(self.instrument, base_value)
    }

    /// Apply the instrument scenario to a raw model result exactly once.
    pub(crate) fn apply_raw_value(&self, base_value: f64) -> f64 {
        apply_scenario_raw_value(self.instrument, base_value)
    }
}

/// Apply an instrument's scenario price adjustment to a base present value.
///
/// Registry, direct-value, and metric assembly paths all route through this
/// helper so the adjustment remains an exactly-once lifecycle step.
#[inline]
pub(crate) fn apply_scenario_value<I>(instrument: &I, base_value: Money) -> Money
where
    I: crate::instruments::common_impl::traits::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .map_or(base_value, |overrides| overrides.apply_to_value(base_value))
}

/// Apply an instrument's scenario price adjustment without currency rounding.
///
/// This is the raw-f64 counterpart to [`apply_scenario_value`]. Keeping the
/// multiplier outside `Money` preserves the precision expected by finite-
/// difference risk calculations.
#[inline]
pub(crate) fn apply_scenario_raw_value<I>(instrument: &I, base_value: f64) -> f64
where
    I: crate::instruments::common_impl::traits::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .and_then(|overrides| overrides.scenario_price_shock_pct)
        .map_or(base_value, |shock| base_value * (1.0 + shock))
}

/// Convert a discount factor to an effective continuously-compounded zero rate.
///
/// Returns `r` such that `exp(-r * t) = df`. Returns `Ok(0.0)` at expiry
/// (`t <= 0`), which is the correct mathematical limit and matches the
/// behaviour required by callers that short-circuit on `t <= 0` before using
/// the returned rate.
///
/// # Errors
///
/// Returns a `Validation` error when `df` is not finite or non-positive.
/// `df <= 0` would yield `NaN` or `+inf` from `ln`, masking a corrupted curve
/// or extreme rate environment.
#[inline]
pub(crate) fn zero_rate_from_df(
    df: f64,
    t: f64,
    context: &str,
) -> finstack_quant_core::Result<f64> {
    if t <= 0.0 {
        return Ok(0.0);
    }
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount factor must be finite and > 0, got {df:.6e} for t={t:.6}"
        )));
    }
    Ok(-df.ln() / t)
}

/// Compute year fraction between two dates using the given day-count convention.
///
/// This is the canonical helper for all instrument code that needs a plain
/// `(start, end, dc) → year_fraction` call without extra context.
/// Avoids duplicating `dc.year_fraction(start, end, DayCountContext::default())`
/// in every pricer / calculator module.
#[inline]
pub fn year_fraction(dc: DayCount, start: Date, end: Date) -> finstack_quant_core::Result<f64> {
    dc.year_fraction(start, end, DayCountContext::default())
}

/// Schedule → PV helper that uses the curve's own day count convention.
///
/// This variant ensures consistency between:
/// - Metric calculations (e.g., par rate using `df_on_date_curve`)
/// - NPV calculations
///
/// Routes through `core::cashflow::npv`, which excludes flows on or before
/// the valuation date. Valuation on a day assumes cash settling that day has
/// already been paid.
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows
/// * `curves` - Market data context
/// * `as_of` - Valuation date
/// * `discount_curve_id` - ID of the discount curve to use
pub fn schedule_pv<S>(
    instrument: &S,
    curves: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
) -> finstack_quant_core::Result<Money>
where
    S: crate::cashflow::traits::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv;

    let flows = S::dated_cashflows(instrument, curves, as_of)?;
    let disc = curves.get_discount(discount_curve_id.as_str())?;
    // Use None to use the curve's day count for consistent pricing with metrics
    npv(disc.as_ref(), as_of, &flows)
}

/// Schedule → PV helper that uses the curve's own day count convention (raw f64).
///
/// Returns unrounded NPV for high-precision calibration/risk.
///
/// Cashflows on or before `as_of` are excluded, matching [`schedule_pv`]. The
/// only distinction is the unrounded scalar output used by calibration and risk.
pub fn schedule_pv_raw<S>(
    instrument: &S,
    curves: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
) -> finstack_quant_core::Result<f64>
where
    S: crate::cashflow::traits::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv_amounts_with_curve;

    let flows = S::dated_cashflows(instrument, curves, as_of)?;
    let disc = curves.get_discount(discount_curve_id.as_str())?;

    let amounts = flows
        .into_iter()
        .map(|(date, amount)| (date, amount.amount()))
        .collect::<Vec<_>>();
    npv_amounts_with_curve(disc.as_ref(), as_of, &amounts)
}

/// Schedule → raw trade NPV, including cashflows dated exactly on `as_of`.
///
/// This is narrower than [`schedule_pv_raw`]: truly past cashflows are excluded,
/// while an inception exchange settling on the valuation date is retained at
/// unit discount factor. Calibration instruments such as T+0 deposits need this
/// convention so their quoted rate can zero the complete trade NPV.
pub fn schedule_trade_pv_raw<S>(
    instrument: &S,
    curves: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
) -> finstack_quant_core::Result<f64>
where
    S: crate::cashflow::traits::CashflowProvider,
{
    use finstack_quant_core::math::NeumaierAccumulator;

    let flows = S::dated_cashflows(instrument, curves, as_of)?;
    let disc = curves.get_discount(discount_curve_id.as_str())?;
    let mut total = NeumaierAccumulator::new();

    for (date, amount) in flows {
        if date < as_of {
            continue;
        }
        total.add(amount.amount() * disc.df_between_dates(as_of, date)?);
    }

    Ok(total.total())
}

/// Resolve an optional dividend-yield scalar from the market context.
///
/// Returns `0.0` only when no dividend yield ID is configured. If an ID is
/// configured, missing or wrongly-typed market data is treated as a validation
/// error rather than silently assuming zero carry.
pub fn resolve_optional_dividend_yield(
    curves: &MarketContext,
    div_yield_id: Option<&finstack_quant_core::types::CurveId>,
) -> finstack_quant_core::Result<f64> {
    let Some(div_id) = div_yield_id else {
        return Ok(0.0);
    };

    let scalar = curves.get_price(div_id.as_str()).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "Failed to fetch dividend yield '{}': {}",
            div_id, e
        ))
    })?;

    match scalar {
        MarketScalar::Unitless(v) => Ok(*v),
        MarketScalar::Price(m) => Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar, got Price({})",
            div_id,
            m.currency()
        ))),
    }
}

/// Build a time-varying GBM drift schedule from a discount curve.
///
/// Samples the cumulative risk-neutral log-drift `M(t) = ∫₀ᵗ (r(u) − q) du` at
/// `num_steps + 1` evenly-spaced knots over `[0, t]`. The rate term structure
/// comes from `disc_curve`; its cumulative log-DF shape is rescaled so `M(t)`
/// matches the maturity-effective rate `r` exactly at `t` (terminal
/// forward/discount consistency). The dividend leg uses the scalar yield `q`.
///
/// Attaching the result to a [`GbmProcess`](finstack_quant_monte_carlo::process::gbm::GbmProcess)
/// removes the per-fixing forward bias the constant maturity-averaged drift
/// introduces for path-dependent (Asian, lookback) Monte Carlo pricing on a
/// non-flat curve. On a flat curve `M(t) = (r − q)·t`, so the schedule is
/// bit-equivalent to the constant drift.
///
/// # Errors
///
/// Returns an error if the resulting schedule is degenerate (see
/// `DriftSchedule::new`) — e.g. a non-finite curve evaluation.
pub fn build_gbm_drift_schedule(
    disc_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    as_of: Date,
    r: f64,
    q: f64,
    t: f64,
    num_steps: usize,
) -> finstack_quant_core::Result<finstack_quant_monte_carlo::process::gbm::DriftSchedule> {
    use finstack_quant_monte_carlo::process::gbm::DriftSchedule;

    let knots = num_steps.max(1);
    // `DiscountCurve::df(t)` is measured from the curve base date.  The
    // simulation, however, starts at `as_of`; use the curve's time coordinate
    // at that date and normalize every sampled DF by DF(as_of).  Sampling
    // `df(tk)` directly would splice historical curve time into the future
    // path whenever the curve base predates valuation.
    let as_of_curve_time = disc_curve.day_count().signed_year_fraction(
        disc_curve.base_date(),
        as_of,
        DayCountContext::default(),
    )?;
    let as_of_df = disc_curve.df(as_of_curve_time);
    if !as_of_df.is_finite() || as_of_df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "GBM drift schedule: invalid discount factor at as_of {as_of}: {as_of_df}"
        )));
    }
    // Rescale the curve's cumulative log-DF shape so the terminal cumulative
    // rate equals r·t exactly — this keeps the simulated terminal forward
    // consistent with the date-based discount factor used to discount the
    // payoff. The curve supplies only the *shape* of the term structure.
    let terminal_rate_cum = -(disc_curve.df(as_of_curve_time + t) / as_of_df).ln();
    let scale = if terminal_rate_cum.abs() > 1e-12 {
        (r * t) / terminal_rate_cum
    } else {
        1.0
    };

    let mut times = Vec::with_capacity(knots + 1);
    let mut cumulative = Vec::with_capacity(knots + 1);
    for k in 0..=knots {
        let tk = t * (k as f64) / (knots as f64);
        let rate_cum = if tk > 0.0 {
            -(disc_curve.df(as_of_curve_time + tk) / as_of_df).ln() * scale
        } else {
            0.0
        };
        times.push(tk);
        cumulative.push(rate_cum - q * tk);
    }
    DriftSchedule::new(times, cumulative)
}

/// Workspace-wide Monte Carlo defaults and resource limits.
///
/// These are the single source of truth for MC pricers across the
/// equity / exotics / commodities / FX modules; per-pricer overrides go
/// through [`resolve_mc_paths`] so the upper bound is always enforced.
pub mod mc_defaults {
    /// Default Monte Carlo path count when no instrument override is supplied.
    pub const DEFAULT_MC_PATHS: usize = 100_000;

    /// Default time-grid resolution (steps per year) for daily-discretised
    /// path-dependent pricers.
    pub const DEFAULT_STEPS_PER_YEAR: f64 = 252.0;

    /// Default Monte Carlo step count for rough-volatility pricers
    /// (rough Heston, rough Bergomi). These models discretise fractional
    /// Brownian motion and a fixed step count is more meaningful than a
    /// time-density.
    pub const DEFAULT_ROUGH_VOL_STEPS: usize = 100;

    /// Hard ceiling on the number of MC paths a single pricer call is
    /// allowed to allocate. Enforced by [`resolve_mc_paths`] to prevent a
    /// malformed `pricing_overrides.model_config.mc_paths` (or a typo) from
    /// taking down a pricing service via OOM.
    /// The cap is set conservatively for multi-tenant pricing hosts.
    pub const MAX_MC_PATHS: usize = 5_000_000;
}

/// Resolve the effective Monte Carlo path count for a pricer call.
///
/// - If `override_paths` is `Some(n)` with `0 < n <= MAX_MC_PATHS`, returns `n`.
/// - If `override_paths` is `Some(n)` with `n > MAX_MC_PATHS`, returns an
///   error rather than silently clamping (silent clamps mask data errors and
///   distort variance estimates).
/// - If `override_paths` is `Some(0)` or `None`, returns `default`.
///
/// This is the single entry point all MC pricers should use to honour the
/// per-instrument `pricing_overrides.model_config.mc_paths` knob.
///
/// # Errors
///
/// Returns `Validation` when the override exceeds `MAX_MC_PATHS`.
#[inline]
pub fn resolve_mc_paths(
    override_paths: Option<usize>,
    default: usize,
) -> finstack_quant_core::Result<usize> {
    let n = match override_paths {
        Some(n) if n > 0 => n,
        _ => default,
    };
    if n > mc_defaults::MAX_MC_PATHS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Monte Carlo path count {} exceeds workspace cap MAX_MC_PATHS = {}; \
             reduce `pricing_overrides.model_config.mc_paths` or raise the cap.",
            n,
            mc_defaults::MAX_MC_PATHS
        )));
    }
    Ok(n)
}

/// Apply the per-instrument `mc_paths` override (if any) to a base
/// `PathDependentPricerConfig`, enforcing [`mc_defaults::MAX_MC_PATHS`].
///
/// Centralizes the merge logic shared by all path-dependent MC pricers
/// (autocallable, cliquet, …).
///
/// # Errors
///
/// Returns `Validation` when the override exceeds `MAX_MC_PATHS`.
#[inline]
pub fn merged_path_config(
    base: &finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
    overrides: &crate::instruments::InstrumentPricingOverrides,
) -> finstack_quant_core::Result<
    finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
> {
    let mut c = base.clone();
    c.num_paths = resolve_mc_paths(overrides.model_config.mc_paths, c.num_paths)?;
    Ok(c)
}

/// Extract a unitless market scalar with a fallback default.
///
/// Commonly used to fetch model parameters (e.g. Heston kappa, rough vol Hurst
/// exponent) from the market context. Returns the `default` when the scalar is
/// absent or has a non-unitless type.
pub fn get_unitless_scalar(market: &MarketContext, key: &str, default: f64) -> f64 {
    market
        .get_price(key)
        .ok()
        .and_then(|s| match s {
            MarketScalar::Unitless(v) => Some(*v),
            MarketScalar::Price(_) => None,
        })
        .unwrap_or(default)
}

/// Strict variant of [`get_unitless_scalar`] that errors when the scalar is
/// missing or carries a non-unitless type.
///
/// Production model-parameter resolvers should prefer this over the lenient
/// fallback form so missing or mistyped model scalars are surfaced.
/// The `model` argument is purely diagnostic and appears in the error
/// message (e.g. `"Heston"`, `"rough Bergomi"`).
pub fn get_unitless_scalar_strict(
    market: &MarketContext,
    key: &str,
    model: &str,
) -> finstack_quant_core::Result<f64> {
    match market.get_price(key) {
        Ok(MarketScalar::Unitless(v)) => Ok(*v),
        Ok(other) => Err(finstack_quant_core::Error::Validation(format!(
            "{model} parameter '{key}' must be a unitless market scalar, got {other:?}"
        ))),
        Err(_) => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: format!("{key} (required by {model} strict from_market resolver)"),
            },
        )),
    }
}

/// Compute requested metric measures for an already-priced instrument.
///
/// The caller owns the `ValuationResult` envelope and passes the final,
/// scenario-adjusted base value. This helper only builds the metric context and
/// returns deterministic measures, preventing metric attachment from replacing
/// model details or metadata.
///
/// This function uses trait objects to avoid generic monomorphization across
/// compilation units, which can cause coverage metadata mismatches.
///
/// # Arguments
///
/// * `instrument` - The instrument to price (wrapped in Arc for efficiency)
/// * `curves` - Market data context (wrapped in Arc for efficiency)
/// * `as_of` - Valuation date
/// * `base_value` - Final scenario-adjusted base value (NPV)
/// * `metrics` - List of metrics to compute
/// * `cfg` - Optional FinstackConfig for user-tunable metric defaults (e.g., bump sizes).
///   When `None`, uses global defaults.
/// * `market_history` - Optional market history for Historical VaR / Expected Shortfall metrics.
///   When `None`, these metrics will not be available.
///
/// # Performance
///
/// Accepts Arc-wrapped arguments to avoid cloning on every call. Callers should
/// clone the instrument and market context once into Arc at the call boundary.
///
/// # Thread Safety
///
/// The `curves` parameter is wrapped in `Arc` for efficiency, not thread synchronization.
/// Callers must ensure the market context is not mutated concurrently. For multi-threaded
/// pricing with market data updates, create a new `MarketContext` snapshot for each
/// pricing batch.
///
/// The `instrument` parameter is also `Arc`-wrapped. Instruments are generally immutable
/// after construction, so this is safe for concurrent reads.
#[derive(Default)]
pub(crate) struct MetricBuildOptions {
    pub(crate) cfg: Option<Arc<FinstackConfig>>,
    pub(crate) market_history: Option<Arc<MarketHistory>>,
    pub(crate) hazard_recalibration_cache:
        Option<Arc<crate::calibration::bumps::hazard::HazardRecalibrationCache>>,
    pub(crate) metric_registry: Option<Arc<crate::metrics::MetricRegistry>>,
    pub(crate) pricing_model: Option<crate::pricer::ModelKey>,
    pub(crate) pricer_registry: Option<Arc<crate::pricer::PricerRegistry>>,
}

pub(crate) fn compute_metrics_dyn(
    instrument: Arc<dyn crate::instruments::common_impl::traits::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<IndexMap<crate::metrics::MetricId, f64>> {
    let MetricBuildOptions {
        cfg,
        market_history,
        hazard_recalibration_cache,
        metric_registry,
        pricing_model,
        pricer_registry,
    } = options;
    let finstack_config = cfg.unwrap_or_else(MetricContext::default_config);
    let mut context = MetricContext::new(
        Arc::clone(&instrument),
        curves,
        as_of,
        base_value,
        finstack_config,
    );

    // Attach market history if provided (for Historical VaR / Expected Shortfall metrics)
    if let Some(history) = market_history {
        context = context.with_market_history(history);
    }
    context.set_hazard_recalibration_cache(hazard_recalibration_cache);
    context.set_pricer_dispatch(pricing_model, pricer_registry);

    // Preserve only the subsets consumed by the metric layer.
    context.set_instrument_overrides(instrument.get_instrument_pricing_overrides().cloned());
    context.set_metric_overrides(instrument.get_metric_pricing_overrides().cloned());

    // Allow instruments to pre-seed the metric context with cached data (e.g., pre-computed
    // cashflows) to avoid redundant computation during metric calculation.
    let market_ref: Arc<MarketContext> = Arc::clone(&context.curves);
    instrument.seed_metric_context(&mut context, market_ref.as_ref(), as_of);

    let registry = match metric_registry.as_deref() {
        Some(registry) => registry,
        None => standard_registry(),
    };
    let instrument_type = instrument.key();
    let applicable: Vec<MetricId> = metrics
        .iter()
        .filter(|m| registry.is_applicable(m, instrument_type))
        .cloned()
        .collect();
    let metric_measures = registry.compute(&applicable, &mut context)?;

    // Pre-allocate capacity to avoid reallocations during insertion.
    // Estimate: requested metrics + a few extras from composite keys.
    let mut measures: IndexMap<MetricId, f64> = IndexMap::with_capacity(metrics.len() + 4);

    // Deterministic insertion order: follow the requested metrics slice order
    for metric_id in metrics {
        if let Some(value) = metric_measures.get(metric_id) {
            measures.insert(metric_id.clone(), *value);
        }
    }

    // Include any composite keys (bucketed series, matrices, tensors, etc.) that were stored into
    // `context.computed` during calculation.
    //
    // IMPORTANT:
    // - We only include *custom* (composite) metric IDs to avoid leaking dependency metrics that
    //   were computed internally but not requested by the caller.
    // - We insert in a stable order (sorted by key) to ensure deterministic results.
    let mut extras: Vec<(&crate::metrics::MetricId, f64)> = context
        .computed
        .iter()
        .filter_map(|(metric_id, value)| {
            if metric_id.is_custom() && !measures.contains_key(metric_id) {
                Some((metric_id, *value))
            } else {
                None
            }
        })
        .collect();
    extras.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    for (metric_id, value) in extras {
        measures.insert(metric_id.clone(), value);
    }

    Ok(measures)
}

/// Test-only result builder for metric calculator unit fixtures.
///
/// Production pricing enriches the model-produced result envelope in the
/// registry and must not construct a parallel `ValuationResult` here.
#[cfg(test)]
pub(crate) fn build_with_metrics_dyn(
    instrument: Arc<dyn crate::instruments::common_impl::traits::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<crate::results::ValuationResult> {
    let cfg = options
        .cfg
        .clone()
        .unwrap_or_else(MetricContext::default_config);
    let instrument_id = instrument.id().to_string();
    let measures = compute_metrics_dyn(instrument, curves, as_of, base_value, metrics, options)?;
    let mut result = crate::results::ValuationResult::stamped_with_config(
        &instrument_id,
        as_of,
        base_value,
        cfg.as_ref(),
    );
    result.measures = measures;
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::cashflow::builder::CashFlowSchedule;
    use crate::cashflow::traits::{schedule_from_dated_flows, ScheduleBuildOpts};
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use std::any::Any;
    use std::sync::Arc;

    #[test]
    fn resolve_mc_paths_uses_default_when_override_missing() {
        let n = resolve_mc_paths(None, 50_000).expect("default returned");
        assert_eq!(n, 50_000);
    }

    #[test]
    fn resolve_mc_paths_uses_default_when_override_is_zero() {
        let n = resolve_mc_paths(Some(0), 50_000).expect("zero falls back");
        assert_eq!(n, 50_000);
    }

    #[test]
    fn resolve_mc_paths_honours_positive_override() {
        let n = resolve_mc_paths(Some(123_456), 50_000).expect("override honoured");
        assert_eq!(n, 123_456);
    }

    #[test]
    fn resolve_mc_paths_rejects_override_above_cap() {
        let too_many = mc_defaults::MAX_MC_PATHS + 1;
        let err =
            resolve_mc_paths(Some(too_many), 50_000).expect_err("override above cap must error");
        let msg = err.to_string();
        assert!(msg.contains("MAX_MC_PATHS"));
        assert!(msg.contains(&too_many.to_string()));
    }

    #[test]
    fn resolve_mc_paths_accepts_override_at_cap() {
        let at_cap = mc_defaults::MAX_MC_PATHS;
        let n = resolve_mc_paths(Some(at_cap), 50_000).expect("exact cap is allowed");
        assert_eq!(n, at_cap);
    }

    #[test]
    fn resolve_mc_paths_rejects_default_above_cap() {
        let too_many = mc_defaults::MAX_MC_PATHS + 1;
        // Even when the default itself exceeds the cap (a programmer bug),
        // we surface it rather than silently allocating.
        let err = resolve_mc_paths(None, too_many).expect_err("default above cap must error");
        assert!(err.to_string().contains("MAX_MC_PATHS"));
    }
    use time::macros::date;
    use time::Duration;

    #[derive(Clone)]
    struct StubInstrument {
        id: String,
        attrs: Attributes,
        instrument_pricing_overrides:
            crate::instruments::pricing_overrides::InstrumentPricingOverrides,
        metric_pricing_overrides: crate::instruments::pricing_overrides::MetricPricingOverrides,
        scenario_pricing_overrides: crate::instruments::pricing_overrides::ScenarioPricingOverrides,
    }

    crate::impl_empty_cashflow_provider!(
        StubInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl StubInstrument {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                attrs: Attributes::default(),
                instrument_pricing_overrides: Default::default(),
                metric_pricing_overrides: Default::default(),
                scenario_pricing_overrides: Default::default(),
            }
        }
    }

    struct SingleFlowProvider;

    impl finstack_quant_cashflows::CashflowScheduleSource for SingleFlowProvider {
        fn notional(&self) -> Option<Money> {
            Some(Money::new(100.0, Currency::USD))
        }

        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(schedule_from_dated_flows(
                vec![
                    (as_of, Money::new(10_000.0, Currency::USD)),
                    (as_of + Duration::days(30), Money::new(100.0, Currency::USD)),
                ],
                crate::cashflow::primitives::CFKind::Fixed,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional(),
                    ..Default::default()
                },
            ))
        }
    }

    impl Instrument for StubInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(123.45, Currency::USD))
        }

        fn attributes(&self) -> &Attributes {
            &self.attrs
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attrs
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        crate::impl_focused_pricing_overrides!();

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_quant_core::Result<crate::results::ValuationResult> {
            let base = self.value(market, as_of)?;
            build_with_metrics_dyn(
                Arc::from(self.clone_box()),
                Arc::new(market.clone()),
                as_of,
                base,
                metrics,
                MetricBuildOptions {
                    cfg: options.config,
                    market_history: options.market_history,
                    ..MetricBuildOptions::default()
                },
            )
        }
    }

    #[test]
    fn stamped_result_uses_provided_config() -> finstack_quant_core::Result<()> {
        let instrument = Arc::new(StubInstrument::new("STUB"));
        let market = Arc::new(MarketContext::new());
        let as_of = date!(2024 - 01 - 01);
        let base_value = Money::new(10.0, Currency::USD);

        let mut cfg = FinstackConfig::default();
        // Set a non-default output scale to verify it is propagated into meta
        cfg.rounding.output_scale.overrides.insert(Currency::USD, 4);
        let cfg = Arc::new(cfg);

        let result = build_with_metrics_dyn(
            instrument,
            market,
            as_of,
            base_value,
            &[],
            MetricBuildOptions {
                cfg: Some(cfg),
                ..MetricBuildOptions::default()
            },
        )?;

        let usd_scale = result
            .meta
            .rounding
            .output_scale_by_ccy
            .get(&Currency::USD)
            .copied();
        assert_eq!(usd_scale, Some(4), "meta should reflect provided config");
        Ok(())
    }

    #[test]
    fn metric_result_preserves_scenario_adjusted_base_value() -> finstack_quant_core::Result<()> {
        let mut instrument = StubInstrument::new("STUB-SHOCK");
        instrument.scenario_pricing_overrides = instrument
            .scenario_pricing_overrides
            .with_price_shock_pct(-0.10);

        let market = MarketContext::new();
        let result = instrument.price_with_metrics(
            &market,
            date!(2024 - 01 - 01),
            &[],
            crate::instruments::PricingOptions::default(),
        )?;

        assert!((result.value.amount() - 111.105).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn instrument_value_default_applies_scenario_price_shock_once(
    ) -> finstack_quant_core::Result<()> {
        // base_value returns 123.45; -10% shock should yield 111.105 from value(),
        // and value() == price_with_metrics().value to guarantee a single application.
        let mut instrument = StubInstrument::new("STUB-VALUE");
        instrument.scenario_pricing_overrides = instrument
            .scenario_pricing_overrides
            .with_price_shock_pct(-0.10);

        let market = MarketContext::new();
        let as_of = date!(2024 - 01 - 01);

        let direct = instrument.value(&market, as_of)?;
        assert!((direct.amount() - 111.105).abs() < 1e-9);

        let via_metrics = instrument
            .price_with_metrics(
                &market,
                as_of,
                &[],
                crate::instruments::PricingOptions::default(),
            )?
            .value;
        assert!((direct.amount() - via_metrics.amount()).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn instrument_base_value_is_unshocked() -> finstack_quant_core::Result<()> {
        // base_value must ignore scenario overrides; only value() applies them.
        let mut instrument = StubInstrument::new("STUB-BASE");
        instrument.scenario_pricing_overrides = instrument
            .scenario_pricing_overrides
            .with_price_shock_pct(-0.10);

        let market = MarketContext::new();
        let base = instrument.base_value(&market, date!(2024 - 01 - 01))?;
        assert!((base.amount() - 123.45).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn black_scholes_inputs_df_r_eff_consistency() {
        use super::BlackScholesInputsDf;

        // Test that r_eff is consistent with df and t
        // Given df = exp(-r * t), we should have r_eff = -ln(df) / t
        let inputs = BlackScholesInputsDf {
            spot: 100.0,
            df: 0.95, // ~5% discount over the period
            q: 0.02,
            sigma: 0.20,
            t: 1.0, // 1 year
        };

        let r_eff = inputs.r_eff();
        // r_eff should be approximately -ln(0.95) / 1.0 ≈ 0.0513
        let expected_r = -0.95_f64.ln() / 1.0;
        assert!(
            (r_eff - expected_r).abs() < 1e-10,
            "r_eff = {}, expected = {}",
            r_eff,
            expected_r
        );

        // Verify round-trip: exp(-r_eff * t) should equal df
        let reconstructed_df = (-r_eff * inputs.t).exp();
        assert!(
            (reconstructed_df - inputs.df).abs() < 1e-10,
            "reconstructed_df = {}, original df = {}",
            reconstructed_df,
            inputs.df
        );
    }

    #[test]
    fn black_scholes_inputs_df_edge_cases() {
        use super::BlackScholesInputsDf;

        // At expiry (t = 0), r_eff should return 0.0
        let at_expiry = BlackScholesInputsDf {
            spot: 100.0,
            df: 1.0,
            q: 0.02,
            sigma: 0.20,
            t: 0.0,
        };
        assert_eq!(at_expiry.r_eff(), 0.0, "r_eff at expiry should be 0");

        // Very short time horizon
        let short_horizon = BlackScholesInputsDf {
            spot: 100.0,
            df: 0.9999,
            q: 0.0,
            sigma: 0.20,
            t: 0.001,
        };
        let r_short = short_horizon.r_eff();
        // Should be approximately -ln(0.9999) / 0.001 ≈ 0.1 (10%)
        assert!(r_short > 0.0, "r_eff should be positive for df < 1");
        assert!(r_short.is_finite(), "r_eff should be finite");
    }

    #[test]
    fn configured_dividend_yield_must_exist() {
        let market = MarketContext::new();
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_quant_core::types::CurveId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(err
            .as_deref()
            .is_some_and(|msg| msg.contains("Failed to fetch dividend yield")));
    }

    #[test]
    fn configured_dividend_yield_must_be_unitless() {
        let market = MarketContext::new().insert_price(
            "DIV",
            finstack_quant_core::market_data::scalars::MarketScalar::Price(Money::new(
                1.0,
                Currency::USD,
            )),
        );
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_quant_core::types::CurveId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(err
            .as_deref()
            .is_some_and(|msg| msg.contains("should be a unitless scalar")));
    }

    #[test]
    fn schedule_pv_raw_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .interp(InterpStyle::Linear)
                .build()?,
        );

        let pv = schedule_pv_raw(&SingleFlowProvider, &market, as_of, &CurveId::new("DISC"))?;

        assert!(pv > 0.0);
        assert!(pv < 100.0);
        Ok(())
    }

    #[test]
    fn schedule_pv_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .interp(InterpStyle::Linear)
                .build()?,
        );

        let pv = schedule_pv(&SingleFlowProvider, &market, as_of, &CurveId::new("DISC"))?;

        assert!(pv.amount() > 0.0);
        assert!(pv.amount() < 100.0);
        assert_eq!(pv.currency(), Currency::USD);
        Ok(())
    }
}

/// Convert a trait object reference to Arc-wrapped trait object.
///
/// This helper clones the instrument via `clone_box()` and converts it to Arc.
/// Used by language bindings (Python/WASM) that work with trait object references.
///
/// # Implementation
///
/// Uses `clone_box()` to get a `Box<dyn Instrument>`, then converts it to `Arc`
/// using `Arc::from()`. This works because `Arc::from()` can convert from `Box<T>`
/// when `T: ?Sized` (which trait objects are).
pub(crate) fn instrument_to_arc(
    instrument: &dyn crate::instruments::common_impl::traits::Instrument,
) -> Arc<dyn crate::instruments::common_impl::traits::Instrument> {
    // Clone via clone_box() to get Box<dyn Instrument>
    let boxed = instrument.clone_box();
    // Convert Box to Arc using Arc::from()
    // This works because Arc::from() can convert Box<T> to Arc<T> for any T
    Arc::from(boxed)
}

/// Black-Scholes inputs with discount factor (DF-first approach).
///
/// This struct provides the source-of-truth inputs for Black-Scholes/Garman-Kohlhagen
/// pricing where discounting is done via date-based discount factors rather than rates.
/// This avoids day-count basis mismatches between the rate `r` and time `t`.
///
/// # Fields
///
/// - `spot`: Current spot price
/// - `df`: Discount factor from `as_of` to `expiry` (date-based, no year-fraction ambiguity)
/// - `q`: Dividend yield / foreign rate (continuous)
/// - `sigma`: Implied volatility from the vol surface
/// - `t`: Time to expiry using the vol surface day count basis (for vol interpolation and Greeks)
#[derive(Debug, Clone, Copy)]
pub struct BlackScholesInputsDf {
    /// Current spot price
    pub spot: f64,
    /// Discount factor from as_of to expiry (date-based)
    pub df: f64,
    /// Dividend yield / foreign rate (continuous)
    pub q: f64,
    /// Implied volatility
    pub sigma: f64,
    /// Time to expiry in years (vol surface basis)
    pub t: f64,
}

impl BlackScholesInputsDf {
    /// Derive an effective continuously-compounded rate consistent with `t` and `df`.
    ///
    /// This returns `r_eff = -ln(df) / t` such that `exp(-r_eff * t) = df`.
    /// Use this when analytical formulas require a scalar rate.
    ///
    /// # Returns
    ///
    /// `r_eff` if `t > 0`, otherwise returns 0.0 (at expiry, rate is irrelevant).
    #[inline]
    pub fn r_eff(&self) -> f64 {
        if self.t > 0.0 && self.df > 0.0 {
            -self.df.ln() / self.t
        } else {
            0.0
        }
    }
}

/// Collect Black-Scholes inputs with discount factor (DF-first approach).
///
/// This is the preferred helper for option pricing as it avoids day-count basis
/// mismatches. The discount factor is computed directly from dates, ensuring
/// `exp(-r_eff * t) = df` when `r_eff` is derived via [`BlackScholesInputsDf::r_eff`].
///
/// # Arguments
///
/// * `spot_id` - ID of the spot price scalar
/// * `discount_curve_id` - ID of the discount curve
/// * `div_yield_id` - Optional ID of the dividend yield scalar (defaults to 0.0 if None)
/// * `vol_surface_id` - ID of the volatility surface
/// * `strike` - Strike price for volatility lookup
/// * `expiry` - Expiry date
/// * `day_count` - Day count convention for vol surface time calculation
/// * `curves` - Market data context
/// * `as_of` - Valuation date
///
/// # Returns
///
/// [`BlackScholesInputsDf`] containing (spot, df, q, sigma, t_vol).
#[allow(clippy::too_many_arguments)]
pub fn collect_black_scholes_inputs_df(
    spot_id: &str,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    div_yield_id: Option<&finstack_quant_core::types::CurveId>,
    vol_surface_id: &str,
    strike: f64,
    expiry: Date,
    day_count: DayCount,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<BlackScholesInputsDf> {
    // Get discount curve
    let disc_curve = curves.get_discount(discount_curve_id.as_str())?;

    // Time to expiry for vol surface lookup (using instrument's day count, which should
    // match how the vol surface was calibrated - typically ACT/365F for equity options)
    let t_vol = day_count.year_fraction(as_of, expiry, DayCountContext::default())?;

    // Discount factor from as_of to expiry (date-based, no year-fraction ambiguity)
    // This is the source of truth for discounting.
    let df = disc_curve.df_between_dates(as_of, expiry)?;

    // Validate DF is usable
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid discount factor ({:.6e}) between {} and {}",
            df, as_of, expiry
        )));
    }

    // Spot price (S)
    let spot_scalar = curves.get_price(spot_id)?;
    let spot = match spot_scalar {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };

    // Dividend yield (q)
    let q = resolve_optional_dividend_yield(curves, div_yield_id)?;

    // Volatility (sigma) using vol surface's time basis
    let vol_surface = curves.get_surface(vol_surface_id)?;
    let sigma = vol_surface.value_clamped(t_vol, strike);

    Ok(BlackScholesInputsDf {
        spot,
        df,
        q,
        sigma,
        t: t_vol,
    })
}

/// Collect standard Black-Scholes inputs (spot, r, q, sigma, t) from market context.
///
/// Retrieves and calculates the 5 standard parameters required for Black-Scholes pricing:
/// - Spot price (S)
/// - Risk-free rate (r) for the period to expiry
/// - Dividend/Continuous yield (q)
/// - Volatility (sigma) at strike/maturity
/// - Time to expiry (t) in years
///
/// # Time-Consistency
///
/// This function derives `r` from the discount factor such that `exp(-r * t) = df`.
/// This ensures the rate and time are on the same basis, avoiding day-count mismatches
/// that can cause pricing errors in barrier options and other path-dependent derivatives.
///
/// # Arguments
///
/// * `spot_id` - ID of the spot price scalar
/// * `discount_curve_id` - ID of the discount curve
/// * `div_yield_id` - Optional ID of the dividend yield scalar (defaults to 0.0 if None)
/// * `vol_surface_id` - ID of the volatility surface
/// * `strike` - Strike price for volatility lookup
/// * `expiry` - Expiry date
/// * `day_count` - Day count convention for vol surface time calculation (should match vol surface calibration basis)
/// * `curves` - Market data context
/// * `as_of` - Valuation date
///
/// # Returns
///
/// A tuple `(spot, r, q, sigma, t)` where:
/// - `spot`: Current spot price
/// - `r`: Effective continuously compounded rate such that `exp(-r*t) = df`
/// - `q`: Dividend yield (0.0 if not provided)
/// - `sigma`: Implied volatility from the vol surface at (t_vol, strike)
/// - `t`: Time to expiry using the vol surface day count basis (t_vol)
#[allow(clippy::too_many_arguments)]
pub fn collect_black_scholes_inputs(
    spot_id: &str,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    div_yield_id: Option<&finstack_quant_core::types::CurveId>,
    vol_surface_id: &str,
    strike: f64,
    expiry: Date,
    day_count: DayCount,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64, f64, f64, f64)> {
    // Delegate to DF-based helper and derive r_eff
    let inputs = collect_black_scholes_inputs_df(
        spot_id,
        discount_curve_id,
        div_yield_id,
        vol_surface_id,
        strike,
        expiry,
        day_count,
        curves,
        as_of,
    )?;

    // Derive effective rate: r_eff = -ln(df) / t such that exp(-r_eff * t) = df
    let r_eff = inputs.r_eff();

    Ok((inputs.spot, r_eff, inputs.q, inputs.sigma, inputs.t))
}

// =============================================================================
// Inflation Lag Helpers
// =============================================================================

use finstack_quant_core::dates::DateExt;
use finstack_quant_core::market_data::scalars::InflationLag;

/// Apply an inflation lag to a date.
///
/// - `Months(m)` subtracts m calendar months
/// - `Days(d)` subtracts d calendar days
/// - `None` returns the date unchanged
///
/// `InflationLag` is `#[non_exhaustive]`. Any future variant added upstream
/// without a matching arm here will trip a release-mode `tracing::warn!` so the
/// silent fallback is auditable in production logs (the previous
/// `debug_assert!` was stripped in release builds, hiding the gap).
pub(crate) fn apply_inflation_lag(date: Date, lag: InflationLag) -> Date {
    match lag {
        InflationLag::None => date,
        InflationLag::Months(m) => date.add_months(-(m as i32)),
        InflationLag::Days(d) => date - time::Duration::days(d as i64),
        #[allow(unreachable_patterns)]
        _unknown => {
            tracing::warn!(
                target: "finstack_quant_valuations::inflation",
                lag = ?_unknown,
                "Unhandled InflationLag variant; falling back to no lag. \
                 A new variant was added to InflationLag in finstack-quant-core \
                 without a matching arm in apply_inflation_lag."
            );
            debug_assert!(
                false,
                "Unhandled InflationLag variant: {:?}. Falling back to no lag.",
                _unknown
            );
            date
        }
    }
}

/// Resolve the effective lag for an inflation instrument.
///
/// Priority: (1) explicit `lag_override`, (2) index lag from market context,
/// (3) `InflationLag::None`.
pub(crate) fn resolve_inflation_lag(
    lag_override: Option<InflationLag>,
    index_id: &str,
    curves: &MarketContext,
) -> InflationLag {
    if let Some(lag) = lag_override {
        return lag;
    }
    if let Ok(index) = curves.get_inflation_index(index_id) {
        return index.lag();
    }
    InflationLag::None
}

/// Resolve a realized CPI/RPI value from a supplied index history.
///
/// Historical observations are contractual fixings: once the effective fixing
/// date is on or before `as_of`, missing coverage must be surfaced rather than
/// silently replaced with a curve projection or an indefinitely stale value.
/// Step interpolation may carry the published level through the remainder of
/// its calendar month; linear interpolation requires a published right anchor.
///
/// The index is cloned with the instrument's effective lag so a contract-level
/// lag override is honored without mutating shared market data.
pub(crate) fn realized_inflation_index_value(
    index: &InflationIndex,
    unlagged_date: Date,
    effective_date: Date,
    effective_lag: InflationLag,
) -> finstack_quant_core::Result<f64> {
    let (first, last) = index.date_range()?;
    let is_covered = if effective_date < first {
        false
    } else {
        match index.interpolation {
            InflationInterpolation::Step => {
                (effective_date.year(), effective_date.month()) <= (last.year(), last.month())
            }
            #[allow(unreachable_patterns)]
            _ => effective_date <= last,
        }
    };

    if !is_covered {
        return Err(finstack_quant_core::InputError::NotFound {
            id: format!(
                "inflation index '{}' fixing coverage for {} (available {} through {})",
                index.id, effective_date, first, last
            ),
        }
        .into());
    }

    index
        .clone()
        .with_lag(effective_lag)
        .value_on(unlagged_date)
}

#[cfg(test)]
mod realized_inflation_index_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    fn step_index() -> InflationIndex {
        InflationIndex::new(
            "US-CPI",
            vec![
                (date!(2025 - 01 - 01), 300.0),
                (date!(2025 - 02 - 01), 301.0),
            ],
            Currency::USD,
        )
        .expect("valid index")
        .with_lag(InflationLag::Months(3))
    }

    #[test]
    fn rejects_historical_dates_outside_published_coverage() {
        let index = step_index();
        for effective_date in [date!(2024 - 12 - 15), date!(2025 - 03 - 01)] {
            let result = realized_inflation_index_value(
                &index,
                effective_date,
                effective_date,
                InflationLag::None,
            );
            assert!(result.is_err(), "{effective_date} must require a fixing");
        }
    }

    #[test]
    fn step_interpolation_carries_only_within_last_published_month() {
        let index = step_index();
        let value = realized_inflation_index_value(
            &index,
            date!(2025 - 02 - 20),
            date!(2025 - 02 - 20),
            InflationLag::None,
        )
        .expect("same-month step fixing is covered");
        assert_eq!(value, 301.0);
    }

    #[test]
    fn contract_lag_override_replaces_index_default_lag() {
        let index = step_index();
        let value = realized_inflation_index_value(
            &index,
            date!(2025 - 02 - 15),
            date!(2025 - 02 - 15),
            InflationLag::None,
        )
        .expect("explicit no-lag override should use February fixing");
        assert_eq!(value, 301.0);
    }

    #[test]
    fn linear_interpolation_requires_a_published_right_anchor() {
        let index = InflationIndex::new(
            "US-CPI",
            vec![(date!(2025 - 01 - 01), 300.0)],
            Currency::USD,
        )
        .expect("valid index")
        .with_interpolation(InflationInterpolation::Linear);
        let result = realized_inflation_index_value(
            &index,
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 15),
            InflationLag::None,
        );
        assert!(
            result.is_err(),
            "linear interpolation needs the next anchor"
        );
    }
}
