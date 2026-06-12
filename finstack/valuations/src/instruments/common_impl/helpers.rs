//! Utilities for instrument pricing and metrics assembly.
//!
//! Contains helpers shared across instrument implementations, notably the
//! function to assemble a `ValuationResult` with computed metrics.

use crate::metrics::risk::MarketHistory;
use crate::metrics::{standard_registry, MetricContext, MetricId};
use finstack_core::config::{results_meta_now, FinstackConfig};
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::{context::MarketContext, scalars::MarketScalar};
use finstack_core::money::Money;
use indexmap::IndexMap;
use std::sync::Arc;

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
pub(crate) fn zero_rate_from_df(df: f64, t: f64, context: &str) -> finstack_core::Result<f64> {
    if t <= 0.0 {
        return Ok(0.0);
    }
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
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
pub fn year_fraction(dc: DayCount, start: Date, end: Date) -> finstack_core::Result<f64> {
    dc.year_fraction(start, end, DayCountContext::default())
}

/// Schedule → PV helper that uses the curve's own day count convention.
///
/// This variant ensures consistency between:
/// - Metric calculations (e.g., par rate using `df_on_date_curve`)
/// - NPV calculations
///
/// # Cashflow-on-as_of Policy: HOLDER-VIEW (Excludes `date <= as_of`)
///
/// Routes through `core::cashflow::npv`, which excludes flows on or before
/// the valuation date (market-standard position value; 2026-06-09 core quant
/// review). For a T+0 deposit valued on its effective date, the initial
/// notional exchange settles on `as_of` and is excluded: `base_value` is the
/// PV of the *remaining* flows (≈ +notional·DF), not the trade NPV (≈ 0 at
/// inception). For pricing-view semantics that include `date == as_of` flows
/// (needed for calibration bracketing of T+0 instruments), use
/// [`schedule_pv_using_curve_dc_raw`].
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows
/// * `curves` - Market data context
/// * `as_of` - Valuation date
/// * `discount_curve_id` - ID of the discount curve to use
pub fn schedule_pv_using_curve_dc<S>(
    instrument: &S,
    curves: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_core::types::CurveId,
) -> finstack_core::Result<Money>
where
    S: crate::cashflow::traits::CashflowProvider,
{
    use finstack_core::cashflow::npv;

    let flows = S::dated_cashflows(instrument, curves, as_of)?;
    let disc = curves.get_discount(discount_curve_id.as_str())?;
    // Use None to use the curve's day count for consistent pricing with metrics
    npv(disc.as_ref(), as_of, None, &flows)
}

/// Schedule → PV helper that uses the curve's own day count convention (raw f64).
///
/// Returns unrounded NPV for high-precision calibration/risk.
///
/// # Cashflow-on-as_of Policy: PRICING-VIEW (Includes `date == as_of`)
///
/// This helper uses **pricing-view** semantics:
/// - Cashflows where `date < as_of` are excluded (truly past)
/// - Cashflows where `date == as_of` are **included** at DF=1 (t=0)
/// - Future cashflows (`date > as_of`) are discounted
///
/// This is critical for:
/// - **Calibration instruments**: T+0 deposits require initial exchange for bracketing
/// - **FRAs and same-day settling instruments**: Payment on as_of is part of value
///
/// For holder-view semantics (excludes `date <= as_of`), see
/// [`crate::instruments::common_impl::discountable::npv_by_date`].
///
/// # Numerical Stability
///
/// Uses Neumaier compensated summation instead of Kahan summation because
/// cashflow schedules often contain mixed-sign values (positive inflows and
/// negative outflows). Neumaier's algorithm handles cases where the sum and
/// the next value have similar magnitudes but opposite signs better than Kahan.
///
/// Reference: Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren
/// zur Summation endlicher Summen." *ZAMM*, 54(1), 39-51.
pub fn schedule_pv_using_curve_dc_raw<S>(
    instrument: &S,
    curves: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_core::types::CurveId,
) -> finstack_core::Result<f64>
where
    S: crate::cashflow::traits::CashflowProvider,
{
    use finstack_core::math::neumaier_sum;

    let flows = S::dated_cashflows(instrument, curves, as_of)?;
    let disc = curves.get_discount(discount_curve_id.as_str())?;

    let mut terms = Vec::with_capacity(flows.len());

    for (date, amount) in flows {
        // PRICING-VIEW: Include cashflows on `as_of` (t=0, df=1).
        // Only exclude truly past cashflows (date < as_of).
        // This ensures calibration bracketing works for T+0 instruments.
        if date < as_of {
            continue;
        }
        // Date-based DF handles the case where as_of != curve base_date correctly
        let df = disc.df_between_dates(as_of, date)?;
        terms.push(amount.amount() * df);
    }

    Ok(neumaier_sum(terms))
}

/// Resolve an optional dividend-yield scalar from the market context.
///
/// Returns `0.0` only when no dividend yield ID is configured. If an ID is
/// configured, missing or wrongly-typed market data is treated as a validation
/// error rather than silently assuming zero carry.
pub fn resolve_optional_dividend_yield(
    curves: &MarketContext,
    div_yield_id: Option<&finstack_core::types::CurveId>,
) -> finstack_core::Result<f64> {
    let Some(div_id) = div_yield_id else {
        return Ok(0.0);
    };

    let scalar = curves.get_price(div_id.as_str()).map_err(|e| {
        finstack_core::Error::Validation(format!(
            "Failed to fetch dividend yield '{}': {}",
            div_id, e
        ))
    })?;

    match scalar {
        MarketScalar::Unitless(v) => Ok(*v),
        MarketScalar::Price(m) => Err(finstack_core::Error::Validation(format!(
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
/// Attaching the result to a [`GbmProcess`](finstack_monte_carlo::process::gbm::GbmProcess)
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
    disc_curve: &finstack_core::market_data::term_structures::DiscountCurve,
    r: f64,
    q: f64,
    t: f64,
    num_steps: usize,
) -> finstack_core::Result<finstack_monte_carlo::process::gbm::DriftSchedule> {
    use finstack_monte_carlo::process::gbm::DriftSchedule;

    let knots = num_steps.max(1);
    // Rescale the curve's cumulative log-DF shape so the terminal cumulative
    // rate equals r·t exactly — this keeps the simulated terminal forward
    // consistent with the date-based discount factor used to discount the
    // payoff. The curve supplies only the *shape* of the term structure.
    let terminal_rate_cum = -disc_curve.df(t).ln();
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
            -disc_curve.df(tk).ln() * scale
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
) -> finstack_core::Result<usize> {
    let n = match override_paths {
        Some(n) if n > 0 => n,
        _ => default,
    };
    if n > mc_defaults::MAX_MC_PATHS {
        return Err(finstack_core::Error::Validation(format!(
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
    base: &finstack_monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
    overrides: &crate::instruments::PricingOverrides,
) -> finstack_core::Result<finstack_monte_carlo::pricer::path_dependent::PathDependentPricerConfig>
{
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
) -> finstack_core::Result<f64> {
    match market.get_price(key) {
        Ok(MarketScalar::Unitless(v)) => Ok(*v),
        Ok(other) => Err(finstack_core::Error::Validation(format!(
            "{model} parameter '{key}' must be a unitless market scalar, got {other:?}"
        ))),
        Err(_) => Err(finstack_core::Error::from(
            finstack_core::InputError::NotFound {
                id: format!("{key} (required by {model} strict from_market resolver)"),
            },
        )),
    }
}

/// Shared helper to build a ValuationResult with a set of metrics.
///
/// Centralizes the repeated pattern across instruments to compute base value,
/// build metric context, compute metrics and stamp a result.
///
/// This function uses trait objects to avoid generic monomorphization across
/// compilation units, which can cause coverage metadata mismatches.
///
/// # Arguments
///
/// * `instrument` - The instrument to price (wrapped in Arc for efficiency)
/// * `curves` - Market data context (wrapped in Arc for efficiency)
/// * `as_of` - Valuation date
/// * `base_value` - Pre-computed base value (NPV)
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
    pub(crate) pricing_model: Option<crate::pricer::ModelKey>,
    pub(crate) pricer_registry: Option<Arc<crate::pricer::PricerRegistry>>,
}

pub(crate) fn build_with_metrics_dyn(
    instrument: Arc<dyn crate::instruments::common_impl::traits::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_core::Result<crate::results::ValuationResult> {
    let MetricBuildOptions {
        cfg,
        market_history,
        pricing_model,
        pricer_registry,
    } = options;
    let finstack_config = cfg.unwrap_or_else(MetricContext::default_config);
    let mut context = MetricContext::new(
        Arc::clone(&instrument),
        curves,
        as_of,
        instrument
            .scenario_overrides()
            .map_or(base_value, |overrides| overrides.apply_to_value(base_value)),
        finstack_config,
    );

    // Attach market history if provided (for Historical VaR / Expected Shortfall metrics)
    if let Some(history) = market_history {
        context = context.with_market_history(history);
    }
    context.set_pricer_dispatch(pricing_model, pricer_registry);

    // Preserve only the subsets consumed by the metric layer.
    context.set_instrument_overrides(
        instrument
            .pricing_overrides()
            .map(crate::instruments::InstrumentPricingOverrides::from_pricing_overrides),
    );
    context.set_metric_overrides(
        instrument
            .pricing_overrides()
            .map(crate::instruments::MetricPricingOverrides::from_pricing_overrides),
    );
    context.set_scenario_overrides(instrument.scenario_overrides().cloned());

    // Allow instruments to pre-seed the metric context with cached data (e.g., pre-computed
    // cashflows) to avoid redundant computation during metric calculation.
    let market_ref: Arc<MarketContext> = Arc::clone(&context.curves);
    instrument.seed_metric_context(&mut context, market_ref.as_ref(), as_of);

    let registry = standard_registry();
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

    let meta = results_meta_now(context.config());
    let mut result = crate::results::ValuationResult::stamped_with_meta(
        context.instrument.id(),
        as_of,
        context.base_value,
        meta,
    );
    result.measures = measures;

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::cashflow::builder::CashFlowSchedule;
    use crate::cashflow::traits::{CashflowProvider, DatedFlows};
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
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
        pricing_overrides: crate::instruments::pricing_overrides::PricingOverrides,
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
                pricing_overrides: crate::instruments::pricing_overrides::PricingOverrides::default(
                ),
            }
        }
    }

    struct DatedFlowsOnlyProvider;

    impl CashflowProvider for DatedFlowsOnlyProvider {
        fn notional(&self) -> Option<Money> {
            Some(Money::new(100.0, Currency::USD))
        }

        fn cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_core::Result<CashFlowSchedule> {
            Err(finstack_core::Error::Validation(
                "shared PV helpers should use dated_cashflows".to_string(),
            ))
        }

        fn dated_cashflows(
            &self,
            _curves: &MarketContext,
            as_of: Date,
        ) -> finstack_core::Result<DatedFlows> {
            Ok(vec![(
                as_of + Duration::days(30),
                Money::new(100.0, Currency::USD),
            )])
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
        ) -> finstack_core::Result<Money> {
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

        fn pricing_overrides_mut(
            &mut self,
        ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
            Some(&mut self.pricing_overrides)
        }

        fn pricing_overrides(
            &self,
        ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
            Some(&self.pricing_overrides)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            metrics: &[MetricId],
            options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> finstack_core::Result<crate::results::ValuationResult> {
            let base = self.base_value(market, as_of)?;
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
    fn stamped_result_uses_provided_config() -> finstack_core::Result<()> {
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
    fn build_with_metrics_applies_scenario_price_shock_to_base_value() -> finstack_core::Result<()>
    {
        let mut instrument = StubInstrument::new("STUB-SHOCK");
        instrument.pricing_overrides = instrument.pricing_overrides.with_price_shock_pct(-0.10);

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
    fn instrument_value_default_applies_scenario_price_shock_once() -> finstack_core::Result<()> {
        // base_value returns 123.45; -10% shock should yield 111.105 from value(),
        // and value() == price_with_metrics().value to guarantee a single application.
        let mut instrument = StubInstrument::new("STUB-VALUE");
        instrument.pricing_overrides = instrument.pricing_overrides.with_price_shock_pct(-0.10);

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
    fn instrument_base_value_is_unshocked() -> finstack_core::Result<()> {
        // base_value must ignore scenario overrides; only value() applies them.
        let mut instrument = StubInstrument::new("STUB-BASE");
        instrument.pricing_overrides = instrument.pricing_overrides.with_price_shock_pct(-0.10);

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
            Some(&finstack_core::types::CurveId::new("DIV")),
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
            finstack_core::market_data::scalars::MarketScalar::Price(Money::new(
                1.0,
                Currency::USD,
            )),
        );
        let err = resolve_optional_dividend_yield(
            &market,
            Some(&finstack_core::types::CurveId::new("DIV")),
        )
        .err()
        .map(|err| err.to_string());
        assert!(err
            .as_deref()
            .is_some_and(|msg| msg.contains("should be a unitless scalar")));
    }

    #[test]
    fn schedule_pv_using_curve_dc_raw_uses_dated_cashflows_path() -> finstack_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .interp(InterpStyle::Linear)
                .build()?,
        );

        let pv = schedule_pv_using_curve_dc_raw(
            &DatedFlowsOnlyProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
        )?;

        assert!(pv > 0.0);
        Ok(())
    }

    #[test]
    fn schedule_pv_using_curve_dc_uses_dated_cashflows_path() -> finstack_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .interp(InterpStyle::Linear)
                .build()?,
        );

        let pv = schedule_pv_using_curve_dc(
            &DatedFlowsOnlyProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
        )?;

        assert!(pv.amount() > 0.0);
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
    discount_curve_id: &finstack_core::types::CurveId,
    div_yield_id: Option<&finstack_core::types::CurveId>,
    vol_surface_id: &str,
    strike: f64,
    expiry: Date,
    day_count: DayCount,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<BlackScholesInputsDf> {
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
        return Err(finstack_core::Error::Validation(format!(
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
    discount_curve_id: &finstack_core::types::CurveId,
    div_yield_id: Option<&finstack_core::types::CurveId>,
    vol_surface_id: &str,
    strike: f64,
    expiry: Date,
    day_count: DayCount,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<(f64, f64, f64, f64, f64)> {
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

use finstack_core::dates::DateExt;
use finstack_core::market_data::scalars::InflationLag;

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
                target: "finstack_valuations::inflation",
                lag = ?_unknown,
                "Unhandled InflationLag variant; falling back to no lag. \
                 A new variant was added to InflationLag in finstack-core \
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
