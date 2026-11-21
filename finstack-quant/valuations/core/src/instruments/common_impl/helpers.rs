//! Utilities for instrument pricing and metrics assembly.
//!
//! Contains helpers shared across instrument implementations, notably metric
//! context construction and deterministic measure assembly.

use crate::instruments::PricingOptions;
use crate::metrics::{MetricContext, MetricId};
use crate::pricer::PricingDispatch;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::{
    context::MarketContext,
    scalars::{InflationExpectedRelease, InflationObservation, MarketScalar, SeriesInterpolation},
};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::sync::Arc;

/// Read a scalar price amount without discarding an attached currency.
pub fn scalar_price_amount(
    scalar: &MarketScalar,
    expected_currency: Currency,
) -> finstack_quant_core::Result<f64> {
    if let Some(value) = scalar.get_unitless() {
        return Ok(value);
    }
    if let Some(price) = scalar.get_price() {
        if price.currency() == expected_currency {
            return Ok(price.amount());
        }
        return Err(finstack_quant_core::Error::CurrencyMismatch {
            expected: expected_currency,
            actual: price.currency(),
        });
    }
    Err(finstack_quant_core::Error::Validation(
        "Unsupported market scalar variant".into(),
    ))
}

/// Publish a Monte Carlo estimate's standard error, 95% confidence bounds and
/// path counts as `mc_*` custom measures on `result`.
pub fn attach_mc_diagnostics(
    result: &mut crate::results::ValuationResult,
    estimate: &finstack_quant_models::monte_carlo::results::MoneyEstimate,
) {
    for (id, value) in [
        ("mc_stderr", estimate.stderr),
        ("mc_ci_95_low", estimate.ci_95.0.amount()),
        ("mc_ci_95_high", estimate.ci_95.1.amount()),
        ("mc_num_paths", estimate.num_paths as f64),
        (
            "mc_num_simulated_paths",
            estimate.num_simulated_paths as f64,
        ),
        ("mc_relative_stderr", estimate.relative_stderr()),
    ] {
        result.measures.insert(MetricId::custom(id), value);
    }
}

/// Validated pricing boundary shared by direct and registry-backed routes.
///
/// Construction performs the only mandatory invariant/override validation
/// step. Callers may then resolve the effective date, run their chosen model
/// kernel, and apply the instrument-owned scenario adjustment exactly once.
pub struct ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument: &'a I,
}

impl<'a, I> ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::Instrument + ?Sized,
{
    /// Validate the complete instrument pricing boundary.
    pub fn new(instrument: &'a I) -> finstack_quant_core::Result<Self> {
        instrument.validate_for_pricing()?;
        Ok(Self { instrument })
    }

    /// Enter the lifecycle after a trusted boundary already validated the instrument.
    pub fn from_validated(instrument: &'a I) -> Self {
        Self { instrument }
    }

    /// Resolve the instrument's effective valuation date after validation.
    pub fn effective_as_of(&self, market: &MarketContext, requested: Date) -> Date {
        self.instrument.resolve_pricing_as_of(market, requested)
    }

    /// Apply the instrument scenario to a model `Money` result exactly once.
    pub fn apply_value(&self, base_value: Money) -> finstack_quant_core::Result<Money> {
        apply_scenario_value(self.instrument, base_value)
    }

    /// Apply the instrument scenario to a raw model result exactly once.
    pub fn apply_raw_value(&self, base_value: f64) -> f64 {
        apply_scenario_raw_value(self.instrument, base_value)
    }
}

/// Apply an instrument's scenario price adjustment to a base present value.
///
/// Registry, direct-value, and metric assembly paths all route through this
/// helper so the adjustment remains an exactly-once lifecycle step.
#[inline]
pub fn apply_scenario_value<I>(
    instrument: &I,
    base_value: Money,
) -> finstack_quant_core::Result<Money>
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .map_or(Ok(base_value), |overrides| {
            overrides.apply_to_value(base_value)
        })
}

/// Apply an instrument's scenario price adjustment without currency rounding.
///
/// This is the raw-f64 counterpart to [`apply_scenario_value`]. Keeping the
/// multiplier outside `Money` preserves the precision expected by finite-
/// difference risk calculations.
#[inline]
pub fn apply_scenario_raw_value<I>(instrument: &I, base_value: f64) -> f64
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .and_then(|overrides| overrides.scenario_price_shock_decimal)
        .map_or(base_value, |shock| base_value * (1.0 + shock))
}

/// Convert a discount factor to an effective continuously-compounded zero rate.
///
/// Returns `r` such that `exp(-r * t) = df`. Returns `Ok(0.0)` at expiry
/// (`t <= 0`), which is the correct mathematical limit and matches the
/// behaviour required by callers that short-circuit on `t <= 0` before using
/// the returned rate.
///
/// # Arguments
///
/// * `df` - Finite positive unitless discount factor.
/// * `t` - Finite year fraction on the discount curve's clock.
/// * `context` - Pricing operation reported in input validation errors.
///
/// # Errors
///
/// Returns a `Validation` error for a non-finite time or a discount factor
/// that is non-finite or non-positive, including at expiry.
#[inline]
pub fn zero_rate_from_df(df: f64, t: f64, context: &str) -> finstack_quant_core::Result<f64> {
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount factor must be finite and > 0, got {df:.6e} for t={t:.6}"
        )));
    }
    if !t.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount time must be finite, got {t}"
        )));
    }
    if t <= 0.0 {
        return Ok(0.0);
    }
    Ok(-df.ln() / t)
}

/// Schedule → PV helper that uses the curve's own day count convention.
///
/// This variant ensures consistency between:
/// - Metric calculations (e.g., par rate using `df_on_date`)
/// - NPV calculations
///
/// Routes through `core::cashflow::npv` (record CORE-CONVENTIONS-016):
/// flows before `as_of` are excluded and a flow dated `as_of` counts at
/// factor 1 exactly when `include_today` is true.
/// An empty remaining schedule returns zero in the instrument's notional
/// currency; an instrument without a defined notional still returns an error.
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows
/// * `market` - Market data context
/// * `as_of` - Valuation date
/// * `discount_curve_id` - ID of the discount curve to use
/// * `include_today` - Whether cash dated `as_of` is still owed; chosen by the
///   product, with no default.
pub fn schedule_pv<S>(
    instrument: &S,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    include_today: bool,
) -> finstack_quant_core::Result<Money>
where
    S: finstack_quant_cashflows::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv;

    let flows = S::dated_cashflows(instrument, market, as_of)?;
    if flows.is_empty() {
        let notional = instrument
            .notional()?
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        return Ok(Money::from((0_i64, notional.currency())));
    }
    let disc = market.get_discount(discount_curve_id.as_str())?;
    // Discount on the curve's own time axis, consistent with curve metrics.
    npv(disc.as_ref(), as_of, &flows, include_today)
}

/// Schedule → PV helper that uses the curve's own day count convention (raw f64).
///
/// Returns unrounded NPV for high-precision calibration/risk.
///
/// Selection matches [`schedule_pv`], including the explicit `include_today`;
/// the only distinction is the unrounded scalar output used by calibration and
/// risk. Empty remaining schedules return zero only when the instrument
/// defines a notional, matching the known-currency boundary of
/// [`schedule_pv`].
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows
/// * `market` - Market data context
/// * `as_of` - Valuation date
/// * `discount_curve_id` - ID of the discount curve to use
/// * `include_today` - Whether cash dated `as_of` is still owed; chosen by the
///   product, with no default.
pub fn schedule_pv_raw<S>(
    instrument: &S,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    include_today: bool,
) -> finstack_quant_core::Result<f64>
where
    S: finstack_quant_cashflows::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv_amounts;

    let flows = S::dated_cashflows(instrument, market, as_of)?;
    if flows.is_empty() {
        instrument
            .notional()?
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        return Ok(0.0);
    }
    let disc = market.get_discount(discount_curve_id.as_str())?;

    let amounts = flows
        .into_iter()
        .map(|(date, amount)| (date, amount.amount()))
        .collect::<Vec<_>>();
    npv_amounts(disc.as_ref(), as_of, &amounts, include_today)
}

/// Resolve an optional dividend-yield scalar from the market context.
///
/// Returns `0.0` only when no dividend yield ID is configured. If an ID is
/// configured, missing or wrongly-typed market data is treated as a validation
/// error rather than silently assuming zero carry.
pub fn resolve_optional_dividend_yield(
    market: &MarketContext,
    div_yield_id: Option<&finstack_quant_core::types::PriceId>,
) -> finstack_quant_core::Result<f64> {
    let Some(div_id) = div_yield_id else {
        return Ok(0.0);
    };

    let scalar = market.get_price(div_id.as_str()).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "Failed to fetch dividend yield '{}': {}",
            div_id, e
        ))
    })?;

    if let Some(v) = scalar.get_unitless() {
        Ok(v)
    } else if let Some(m) = scalar.get_price() {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar, got Price({})",
            div_id,
            m.currency()
        )))
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar",
            div_id
        )))
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
/// Attaching the result to a [`GbmProcess`](finstack_quant_models::monte_carlo::process::gbm::GbmProcess)
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
) -> finstack_quant_core::Result<finstack_quant_models::monte_carlo::process::gbm::DriftSchedule> {
    use finstack_quant_models::monte_carlo::process::gbm::DriftSchedule;

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
    let as_of_df = disc_curve.df(as_of_curve_time)?;
    if !as_of_df.is_finite() || as_of_df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "GBM drift schedule: invalid discount factor at as_of {as_of}: {as_of_df}"
        )));
    }
    // Rescale the curve's cumulative log-DF shape so the terminal cumulative
    // rate equals r·t exactly — this keeps the simulated terminal forward
    // consistent with the date-based discount factor used to discount the
    // payoff. The curve supplies only the *shape* of the term structure.
    let terminal_rate_cum = -(disc_curve.df(as_of_curve_time + t)? / as_of_df).ln();
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
            -(disc_curve.df(as_of_curve_time + tk)? / as_of_df).ln() * scale
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
    /// allowed to allocate. Enforced by [`resolve_mc_paths`](super::resolve_mc_paths) to prevent a
    /// malformed `instrument_pricing_overrides.model_config.mc_paths` (or a typo) from
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
/// per-instrument `instrument_pricing_overrides.model_config.mc_paths` knob.
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
             reduce `instrument_pricing_overrides.model_config.mc_paths` or raise the cap.",
            n,
            mc_defaults::MAX_MC_PATHS
        )));
    }
    Ok(n)
}

/// Apply per-instrument path count and antithetic overrides to a base
/// `PathDependentPricerConfig`, enforcing [`mc_defaults::MAX_MC_PATHS`].
///
/// Centralizes the merge logic shared by path-dependent MC pricers.
///
/// # Errors
///
/// Returns `Validation` when the override exceeds `MAX_MC_PATHS`.
#[inline]
pub fn merged_path_config(
    base: &finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
    overrides: &crate::instruments::pricing_overrides::InstrumentPricingOverrides,
) -> finstack_quant_core::Result<
    finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
> {
    let mut c = base.clone();
    c.num_paths = resolve_mc_paths(overrides.model_config.mc_paths, c.num_paths)?;
    if let Some(antithetic) = overrides.model_config.mc_antithetic {
        c.antithetic = antithetic;
    }
    Ok(c)
}

/// Read a required unitless market scalar, erroring when it is missing or typed.
///
/// The `model` argument is purely diagnostic and appears in the error
/// message (e.g. `"Heston"`, `"rough Bergomi"`).
pub fn get_unitless_scalar_strict(
    market: &MarketContext,
    key: &str,
    model: &str,
) -> finstack_quant_core::Result<f64> {
    match market.get_price(key) {
        Ok(scalar) => scalar.get_unitless().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "{model} parameter '{key}' must be a unitless market scalar, got {scalar:?}"
            ))
        }),
        Err(_) => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: format!("{key} (required by {model})"),
            },
        )),
    }
}

/// Pricing options, dispatch and pricer-published measures handed from the
/// registry to [`compute_metrics_dyn`].
#[derive(Default)]
pub(crate) struct MetricBuildOptions {
    /// Config, market history and recalibration provider for the metric context.
    pub pricing: PricingOptions,
    /// Pricer dispatch metric calculators use when they reprice bumped markets.
    pub pricing_dispatch: PricingDispatch,
    /// Measures the model pricer already published for this valuation, taken
    /// verbatim from its `ValuationResult`. Standard identifiers among them
    /// seed the metric context; see [`compute_metrics_dyn`].
    pub pricer_measures: IndexMap<MetricId, f64>,
    /// State the model pricer attached to its `ValuationResult`; installed into
    /// the metric context before the instrument seeds it.
    pub pricer_seed: Option<Arc<dyn crate::metrics::MetricSeed>>,
}

/// Compute requested metric measures for an already-priced instrument.
///
/// The caller owns the `ValuationResult` envelope and passes the final,
/// scenario-adjusted base value. This helper only builds the metric context and
/// returns deterministic measures, preventing metric attachment from replacing
/// model details or metadata.
///
/// # Arguments
///
/// * `instrument` - The priced instrument, shared with the metric context.
/// * `curves` - Market data context the base value was priced against.
/// * `as_of` - Valuation date.
/// * `base_value` - Final scenario-adjusted base value (NPV).
/// * `metrics` - Metrics to compute. Every entry is forwarded to the metric
///   registry unfiltered, so each one either lands in the returned map or
///   aborts the call with an error; requested metrics are never silently
///   dropped. Callers that tolerate partial support across heterogeneous
///   instruments (composites) must narrow this list themselves.
/// * `options` - Pricing options (config, market history for historical
///   VaR/ES, recalibration provider), the pricer dispatch used for bumped
///   reprices, and the measures the model pricer already published.
///
/// # Errors
///
/// Returns `Error::UnknownMetric` when a requested identifier is not
/// registered, `Error::MetricNotApplicable` when it is registered but has no
/// calculator for this instrument type, and `Error::MetricCalculationFailed`
/// when a calculator runs and fails (for example on a missing market input).
pub(crate) fn compute_metrics_dyn(
    instrument: Arc<dyn crate::instruments::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<IndexMap<crate::metrics::MetricId, f64>> {
    let MetricBuildOptions {
        pricing,
        pricing_dispatch,
        pricer_measures,
        pricer_seed,
    } = options;
    let finstack_config = pricing.config.unwrap_or_else(MetricContext::default_config);
    let mut context = MetricContext::new(
        Arc::clone(&instrument),
        curves,
        as_of,
        base_value,
        finstack_config,
    );

    // Attach market history if provided (for Historical VaR / Expected Shortfall metrics)
    if let Some(history) = pricing.market_history {
        context = context.with_market_history(history);
    }
    context.set_recalibration_provider(pricing.recalibration_provider);
    context.set_pricer_dispatch(pricing_dispatch);

    // Preserve only the subsets consumed by the metric layer.
    context.set_metric_pricing_overrides(instrument.get_metric_pricing_overrides().cloned());

    // Allow instruments to pre-seed the metric context with cached data (e.g., pre-computed
    // cashflows) to avoid redundant computation during metric calculation.
    if let Some(seed) = pricer_seed {
        seed.seed(&mut context);
    }
    let market_ref: Arc<MarketContext> = Arc::clone(&context.curves);
    instrument.seed_metric_context(&mut context, market_ref.as_ref(), as_of);

    if !metrics.is_empty()
        && let PricingDispatch::Registered { model, registry } = context.clone_pricer_dispatch()
    {
        let key = crate::pricer::PricerKey::new(instrument.key(), model);
        if let Some(pricer) = registry.get_pricer(key) {
            pricer.seed_metric_context(&mut context, metrics)?;
        }
    }

    // A model may publish standard metric identifiers as side outputs of its own
    // pricing pass — structured credit's `expected_loss`, the Merton bond
    // engine's `expected_loss` — instead of leaving them to a registered
    // calculator. Those values are the authoritative ones: the result envelope
    // gives model measures precedence over calculator output. Seeding them here
    // makes the metric layer read the same number rather than recompute it, and
    // satisfies the registry's up-front applicability check, which exempts
    // identifiers the context already carries.
    //
    // Only standard identifiers are seeded. Custom and composite keys are merged
    // into the result envelope by the pricer itself, are not part of the
    // registry's applicability contract, and seeding them would reorder the
    // envelope's custom-measure tail.
    for (metric_id, value) in pricer_measures {
        if !metric_id.is_custom() {
            context.computed.insert(metric_id, value);
        }
    }

    let registry = match pricing.metric_registry.as_deref() {
        Some(registry) => registry,
        None => instrument.family().metrics(),
    };
    // Every requested metric is forwarded to the registry unfiltered: the
    // registry rejects unknown identifiers and identifiers with no calculator
    // for this instrument type. Silently dropping a caller-requested metric
    // here would make "not supported" indistinguishable from "computed but
    // missing" at the host-language boundary.
    let metric_measures = registry.compute(metrics, &mut context)?;

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
    // - Include composite/custom outputs and the bucketed-vega reconciliation
    //   residual, while keeping unrequested internal dependency metrics private.
    // - We insert in a stable order (sorted by key) to ensure deterministic results.
    let mut extras: Vec<(&crate::metrics::MetricId, f64)> = context
        .computed
        .iter()
        .filter_map(|(metric_id, value)| {
            if (metric_id.is_custom() || MetricId::SIDE_OUTPUT_METRICS.contains(metric_id))
                && !measures.contains_key(metric_id)
            {
                Some((metric_id, *value))
            } else {
                None
            }
        })
        .collect();
    extras.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    for (metric_id, value) in extras {
        crate::metrics::registry::validate_measure(metric_id, value)?;
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
    instrument: Arc<dyn crate::instruments::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<crate::results::ValuationResult> {
    let cfg = options
        .pricing
        .config
        .clone()
        .unwrap_or_else(MetricContext::default_config);
    let instrument_id = instrument.id().to_string();
    let measures = compute_metrics_dyn(instrument, curves, as_of, base_value, metrics, options)?;
    // Stamp the exact `FinstackConfig` used during pricing so execution policy
    // and reported metadata cannot disagree.
    let mut result = crate::results::ValuationResult::stamped_with_meta(
        &instrument_id,
        as_of,
        base_value,
        finstack_quant_core::config::results_meta(cfg.as_ref()),
    );
    result.measures = measures;
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::instruments::Instrument;
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use finstack_quant_cashflows::builder::CashFlowSchedule;
    use finstack_quant_cashflows::{ScheduleBuildOpts, schedule_from_dated_flows};
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::CurveId;
    use std::any::Any;
    use std::sync::Arc;

    #[test]
    fn zero_rate_validates_inputs_before_expiry_return() {
        for df in [f64::NAN, 0.0] {
            assert!(zero_rate_from_df(df, 0.0, "test").is_err());
        }
        for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(zero_rate_from_df(1.0, time, "test").is_err());
        }
        assert_eq!(zero_rate_from_df(1.0, -1.0, "test").unwrap(), 0.0);
    }

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
    use time::Duration;
    use time::macros::date;

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
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
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
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(Some(Money::from((100_i64, Currency::USD))))
        }

        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            schedule_from_dated_flows(
                vec![
                    (as_of, Money::from((10_000_i64, Currency::USD))),
                    (
                        as_of + Duration::days(30),
                        Money::from((100_i64, Currency::USD)),
                    ),
                ],
                finstack_quant_core::cashflow::CFKind::Fixed,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional().expect("valid notional"),
                    ..Default::default()
                },
            )
        }
    }

    impl Instrument for StubInstrument {
        fn includes_valuation_date_cashflows(&self) -> bool {
            true
        }
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
            Ok(crate::instruments::MarketDependencies::new())
        }

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
            Ok(Money::new(123.45, Currency::USD).expect("valid money fixture"))
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
            options: crate::instruments::PricingOptions,
        ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError>
        {
            let base = self.value(market, as_of)?;
            Ok(build_with_metrics_dyn(
                Arc::from(self.clone_box()),
                Arc::new(market.clone()),
                as_of,
                base,
                metrics,
                MetricBuildOptions {
                    pricing: options,
                    ..MetricBuildOptions::default()
                },
            )?)
        }
    }

    #[test]
    fn stamped_result_uses_provided_config() -> finstack_quant_core::Result<()> {
        let instrument = Arc::new(StubInstrument::new("STUB"));
        let market = Arc::new(MarketContext::new());
        let as_of = date!(2024 - 01 - 01);
        let base_value = Money::from((10_i64, Currency::USD));

        let mut cfg = FinstackConfig::default();
        // Set a non-default output scale to verify it is propagated into meta
        cfg.rounding
            .output_scale
            .set_scale(Currency::USD, 4)
            .expect("valid decimal scale");
        let cfg = Arc::new(cfg);

        let result = build_with_metrics_dyn(
            instrument,
            market,
            as_of,
            base_value,
            &[],
            MetricBuildOptions {
                pricing: PricingOptions {
                    config: Some(cfg),
                    ..PricingOptions::default()
                },
                ..MetricBuildOptions::default()
            },
        )?;

        let usd_scale = result.meta.rounding.output_scale(Currency::USD);
        assert_eq!(usd_scale, 4, "meta should reflect provided config");
        Ok(())
    }

    #[test]
    fn metric_result_preserves_scenario_adjusted_base_value() -> finstack_quant_core::Result<()> {
        let mut instrument = StubInstrument::new("STUB-SHOCK");
        instrument.scenario_pricing_overrides = instrument
            .scenario_pricing_overrides
            .with_scenario_price_shock_decimal(-0.10);

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
    fn instrument_value_default_applies_scenario_price_shock_once()
    -> finstack_quant_core::Result<()> {
        // base_value returns 123.45; -10% shock should yield 111.105 from value(),
        // and value() == price_with_metrics().value to guarantee a single application.
        let mut instrument = StubInstrument::new("STUB-VALUE");
        instrument.scenario_pricing_overrides = instrument
            .scenario_pricing_overrides
            .with_scenario_price_shock_decimal(-0.10);

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
            .with_scenario_price_shock_decimal(-0.10);

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
            Some(&finstack_quant_core::types::PriceId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(
            err.as_deref()
                .is_some_and(|msg| msg.contains("Failed to fetch dividend yield"))
        );
    }

    #[test]
    fn configured_dividend_yield_must_be_unitless() {
        let market = MarketContext::new().insert_price(
            "DIV",
            finstack_quant_core::market_data::scalars::MarketScalar::price(Money::from((
                1_i64,
                Currency::USD,
            ))),
        );
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_quant_core::types::PriceId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(
            err.as_deref()
                .is_some_and(|msg| msg.contains("should be a unitless scalar"))
        );
    }

    #[test]
    fn schedule_pv_raw_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::builder("DISC")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .interp(InterpStyle::Linear)
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()?,
            )
            .expect("insert market curve");

        let pv = schedule_pv_raw(
            &SingleFlowProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
            false,
        )?;

        assert!(pv > 0.0);
        assert!(pv < 100.0);
        Ok(())
    }

    #[test]
    fn empty_schedule_without_known_currency_remains_an_error() {
        let empty = StubInstrument::new("EMPTY-CURRENCY");
        let market = MarketContext::new();
        let curve_id = CurveId::new("DISC");
        let as_of = date!(2025 - 01 - 16);
        assert!(matches!(
            schedule_pv(&empty, &market, as_of, &curve_id, false),
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints
            ))
        ));
        assert!(matches!(
            schedule_pv_raw(&empty, &market, as_of, &curve_id, false),
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints
            ))
        ));
    }

    #[test]
    fn schedule_pv_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::builder("DISC")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .interp(InterpStyle::Linear)
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()?,
            )
            .expect("insert market curve");

        let pv = schedule_pv(
            &SingleFlowProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
            false,
        )?;

        assert!(pv.amount() > 0.0);
        assert!(pv.amount() < 100.0);
        assert_eq!(pv.currency(), Currency::USD);
        Ok(())
    }
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

// Inflation Lag Helpers

use finstack_quant_core::dates::DateExt;
use finstack_quant_core::market_data::scalars::InflationLag;

/// Apply an inflation lag to a date.
///
/// - `Months(m)` subtracts m calendar months
/// - `Days(d)` subtracts d calendar days
/// - `None` returns the date unchanged
///
/// Returns a validation error when the lag underflows the supported calendar
/// or the lag convention is unsupported.
///
/// # Arguments
///
/// * `date` - Contractual date before applying the observation shift.
/// * `lag` - Explicit contractual calendar-month or calendar-day shift.
pub fn apply_inflation_lag(date: Date, lag: InflationLag) -> finstack_quant_core::Result<Date> {
    match lag {
        InflationLag::None => Ok(date),
        InflationLag::Months(m) => date.add_months(-i32::from(m.get())),
        InflationLag::Days(d) => date.add_days(-i64::from(d.get())),
        _ => Err(finstack_quant_core::Error::Validation(
            "unsupported inflation lag convention".into(),
        )),
    }
}

/// Selected observation or release assumption admitting a monthly CPI contribution.
#[derive(Debug, Clone, Copy)]
pub enum InflationReferenceSource {
    /// Exact published fixing in the selected snapshot.
    Observed(InflationObservation),
    /// Projection admitted by an outstanding release assumption known at the cutoff.
    Projected(InflationExpectedRelease),
}

/// A monthly contribution to a contractual reference CPI.
#[derive(Debug, Clone, Copy)]
pub struct InflationReferenceAnchor {
    /// Canonical first day of the contributing reference month.
    pub reference_month: Date,
    /// Positive convex interpolation weight; contributing weights sum to one.
    pub weight: f64,
    /// Positive finite observed or projected index level.
    pub value: f64,
    /// Actual availability or expected-release facts admitting this contribution.
    pub source: InflationReferenceSource,
}

/// Resolved reference CPI together with the information needed to distinguish
/// observed fixings from projections for option valuation.
pub struct InflationReferenceValue {
    /// Reference CPI after observation lag and monthly interpolation.
    pub value: f64,
    /// Whether every required anchor has an available historical fixing.
    pub is_known: bool,
    /// Latest expected release among outstanding anchors, or `None` when all
    /// anchors are observed. This is a schedule assumption, not actual availability.
    pub publication_date: Option<Date>,
    /// Exact index identifier shared by observations and projection.
    pub index_id: String,
    /// Inclusive knowledge cutoff of the immutable fixing snapshot.
    pub knowledge_date: Date,
    /// Required monthly contributions, without unused zero-weight anchors.
    pub anchors: Vec<InflationReferenceAnchor>,
}

/// Resolve a contractual reference CPI, applying month lag and daily weight
/// once, together with the availability of every interpolation anchor.
///
/// Exact observations in the selected knowledge snapshot take priority over
/// release assumptions. A missing observation projects only when a matching
/// expected release is strictly after the cutoff. Missing or overdue release
/// facts error; valuation-date changes never publish a fixing.
///
/// # Arguments
///
/// * `market` - Market context holding the inflation index history and, for
///   unpublished anchors, the unlagged inflation curve named `index_id`.
/// * `index_id` - Identifier shared by the inflation index and inflation curve.
/// * `date` - Contractual reference date before the observation lag is applied.
/// * `knowledge_date` - Inclusive information cutoff; must match the supplied
///   index snapshot. This is independent of the product's valuation date.
/// * `lag` - Observation lag: month lags keep the contractual month's daily
///   weight, day lags shift the reference date itself.
/// * `interpolation` - `Step` uses the first-of-month anchor only; `Linear`
///   interpolates daily between consecutive monthly anchors.
///
/// # Errors
///
/// Returns an error for a mismatched snapshot cutoff, missing/overdue fixing
/// facts, missing projection coverage, invalid levels, or calendar overflow.
pub fn resolve_reference_inflation(
    market: &MarketContext,
    index_id: &str,
    date: Date,
    knowledge_date: Date,
    lag: InflationLag,
    interpolation: SeriesInterpolation,
) -> finstack_quant_core::Result<InflationReferenceValue> {
    let index = market.get_inflation_index(index_id)?;
    if index.knowledge_date() != knowledge_date {
        return Err(finstack_quant_core::Error::Validation(format!(
            "inflation index '{index_id}' snapshot cutoff {} does not match requested knowledge date {knowledge_date}",
            index.knowledge_date()
        )));
    }
    let (weight_date, months) = match lag {
        InflationLag::Months(months) => (date, months.get()),
        _ => (apply_inflation_lag(date, lag)?, 0),
    };
    let first = weight_date
        .replace_day(1)
        .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?
        .add_months(-i32::from(months))?;
    let weight = match interpolation {
        SeriesInterpolation::Step => 0.0,
        SeriesInterpolation::Linear => {
            f64::from(weight_date.day() - 1)
                / f64::from(weight_date.month().length(weight_date.year()))
        }
    };
    let resolve = |reference_month,
                   weight|
     -> finstack_quant_core::Result<InflationReferenceAnchor> {
        let (value, source) = match index.get_observation(reference_month) {
            Ok(observation) => (
                observation.value,
                InflationReferenceSource::Observed(observation),
            ),
            Err(finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                ..
            })) => {
                let release = index.get_expected_release(reference_month)?.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "inflation index '{index_id}' has no observation or expected release for {reference_month}"
                    ))
                })?;
                if release.expected_release <= knowledge_date {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "inflation index '{index_id}' fixing for {reference_month} is overdue on {knowledge_date}"
                    )));
                }
                (
                    market
                        .get_inflation_curve(index_id)?
                        .cpi_on_date(reference_month)?,
                    InflationReferenceSource::Projected(release),
                )
            }
            Err(error) => return Err(error),
        };
        Ok(InflationReferenceAnchor {
            reference_month,
            weight,
            value,
            source,
        })
    };
    let mut anchors = vec![resolve(first, 1.0 - weight)?];
    let value = if weight == 0.0 {
        anchors[0].value
    } else {
        anchors.push(resolve(first.add_months(1)?, weight)?);
        let left = anchors[0].value;
        let right = anchors[1].value;
        if left <= right {
            left + weight * (right - left)
        } else {
            right + (1.0 - weight) * (left - right)
        }
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "inflation index '{index_id}' reference CPI is not finite and positive"
        )));
    }
    let publication_date = anchors
        .iter()
        .filter_map(|anchor| match anchor.source {
            InflationReferenceSource::Observed(_) => None,
            InflationReferenceSource::Projected(release) => Some(release.expected_release),
        })
        .max();
    Ok(InflationReferenceValue {
        value,
        is_known: publication_date.is_none(),
        publication_date,
        index_id: index_id.to_owned(),
        knowledge_date,
        anchors,
    })
}

#[cfg(test)]
mod realized_inflation_index_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::InflationIndex;
    use finstack_quant_core::market_data::term_structures::InflationCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use time::macros::date;

    fn market(
        cutoff: Date,
        observations: Vec<InflationObservation>,
        releases: Vec<InflationExpectedRelease>,
    ) -> MarketContext {
        let index = InflationIndex::new("CPI", observations, Currency::USD, cutoff)
            .unwrap()
            .with_expected_releases(releases)
            .unwrap();
        MarketContext::new()
            .insert_inflation_index(index)
            .try_insert(
                InflationCurve::builder("CPI")
                    .base_date(date!(2024 - 01 - 01))
                    .day_count(DayCount::Act365F)
                    .interp(InterpStyle::Linear)
                    .extrapolation(ExtrapolationPolicy::Flat)
                    .knots([(0.0, 306.0), (5.0, 306.0)])
                    .build()
                    .unwrap(),
            )
            .expect("insert market curve")
    }

    #[test]
    fn observations_win_and_outstanding_releases_admit_projection() {
        let cutoff = date!(2024 - 02 - 20);
        let january = date!(2024 - 01 - 01);
        let february = date!(2024 - 02 - 01);
        let market = market(
            cutoff,
            vec![InflationObservation {
                reference_month: january,
                value: 300.0,
                available_on: date!(2024 - 02 - 12),
            }],
            vec![
                InflationExpectedRelease {
                    reference_month: january,
                    expected_release: date!(2024 - 02 - 24),
                    known_on: january,
                },
                InflationExpectedRelease {
                    reference_month: february,
                    expected_release: date!(2024 - 03 - 12),
                    known_on: january,
                },
            ],
        );
        let result = resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 04 - 15),
            cutoff,
            InflationLag::months(3),
            SeriesInterpolation::Linear,
        )
        .unwrap();
        assert!((result.value - 302.8).abs() < 1e-12);
        assert!(!result.is_known);
        assert_eq!(result.publication_date, Some(date!(2024 - 03 - 12)));
        assert_eq!(result.knowledge_date, cutoff);
        assert_eq!(result.anchors.len(), 2);
        assert!(matches!(
            result.anchors[0].source,
            InflationReferenceSource::Observed(_)
        ));
        assert!(matches!(
            result.anchors[1].source,
            InflationReferenceSource::Projected(_)
        ));
        assert_eq!(result.anchors[1].reference_month, february);
        assert!((result.anchors[1].weight - 14.0 / 30.0).abs() < 1e-15);
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 04 - 15),
                date!(2024 - 03 - 12),
                InflationLag::months(3),
                SeriesInterpolation::Linear,
            )
            .is_err()
        );
    }

    #[test]
    fn missing_or_overdue_release_facts_never_project() {
        let cutoff = date!(2024 - 03 - 12);
        for releases in [
            vec![],
            vec![InflationExpectedRelease {
                reference_month: date!(2024 - 02 - 01),
                expected_release: cutoff,
                known_on: date!(2024 - 01 - 01),
            }],
        ] {
            let market = market(cutoff, vec![], releases);
            assert!(
                resolve_reference_inflation(
                    &market,
                    "CPI",
                    date!(2024 - 02 - 01),
                    cutoff,
                    InflationLag::None,
                    SeriesInterpolation::Step,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn monthly_geometry_and_unused_anchors_do_not_depend_on_metadata() {
        let cutoff = date!(2024 - 03 - 01);
        let january = date!(2024 - 01 - 01);
        let observations = vec![InflationObservation {
            reference_month: january,
            value: 300.0,
            available_on: date!(2024 - 02 - 12),
        }];
        let market = market(cutoff, observations, vec![]);
        for (lag, contract_date) in [
            (InflationLag::None, january),
            (InflationLag::months(0), january),
            (InflationLag::days(0), january),
            (InflationLag::days(10), date!(2024 - 01 - 11)),
            (InflationLag::months(3), date!(2024 - 04 - 01)),
        ] {
            let result = resolve_reference_inflation(
                &market,
                "CPI",
                contract_date,
                cutoff,
                lag,
                SeriesInterpolation::Linear,
            )
            .unwrap();
            assert_eq!(result.value, 300.0);
            assert!(result.is_known);
            assert_eq!(result.anchors.len(), 1);
        }
        let step = resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 01 - 15),
            cutoff,
            InflationLag::None,
            SeriesInterpolation::Step,
        )
        .unwrap();
        assert_eq!(step.value, 300.0);
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 01 - 15),
                cutoff,
                InflationLag::None,
                SeriesInterpolation::Linear,
            )
            .is_err()
        );
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2023 - 12 - 15),
                cutoff,
                InflationLag::None,
                SeriesInterpolation::Step,
            )
            .is_err()
        );
    }

    #[test]
    fn month_lag_uses_contract_month_weight_and_preserves_extreme_levels() {
        let cutoff = date!(2024 - 03 - 01);
        for (left, right, expected) in [
            (285.0, 314.0, 313.0),
            (f64::from_bits(1), f64::from_bits(1), f64::from_bits(1)),
            (f64::MAX, f64::MAX, f64::MAX),
        ] {
            let market = market(
                cutoff,
                vec![
                    InflationObservation {
                        reference_month: date!(2023 - 11 - 01),
                        value: left,
                        available_on: date!(2023 - 12 - 12),
                    },
                    InflationObservation {
                        reference_month: date!(2023 - 12 - 01),
                        value: right,
                        available_on: date!(2024 - 01 - 12),
                    },
                ],
                vec![],
            );
            let result = resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 02 - 29),
                cutoff,
                InflationLag::months(3),
                SeriesInterpolation::Linear,
            )
            .unwrap();
            assert_eq!(result.value, expected);
            assert!(result.is_known);
        }
    }

    #[test]
    fn date_offset_rejects_inflation_lag_underflow() {
        for lag in [InflationLag::months(1), InflationLag::days(1)] {
            assert!(apply_inflation_lag(Date::MIN, lag).is_err());
        }
        assert_eq!(
            apply_inflation_lag(Date::MIN, InflationLag::None).expect("no date shift"),
            Date::MIN
        );
    }
}
