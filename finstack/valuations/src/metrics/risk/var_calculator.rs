//! Historical VaR calculation engine.
//!
//! Implements Historical Value-at-Risk using historical simulation methodology.
//! Supports both full revaluation and Taylor approximation (Greeks-based) approaches.

use crate::instruments::common_impl::helpers::instrument_to_arc;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::risk::MarketHistory;
use crate::metrics::sensitivities::config::format_bucket_label_cow;
use crate::metrics::{standard_registry, MetricContext, MetricId};
use crate::pricer::{ModelKey, PricerRegistry};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::math::{neumaier_sum, NeumaierAccumulator};
use finstack_core::money::fx::FxConversionPolicy;
use finstack_core::money::fx::FxQuery;
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_core::Result;
use std::sync::Arc;

/// VaR calculation method.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VarMethod {
    /// Full revaluation of instrument under each historical scenario.
    ///
    /// Most accurate method - reprices the instrument under each historical
    /// market scenario. Captures all non-linearities and path dependencies.
    #[default]
    FullRevaluation,

    /// Taylor approximation using sensitivities (Greeks).
    ///
    /// Faster method - approximates P&L using pre-computed sensitivities.
    /// Good for linear instruments and large portfolios, but may be
    /// inaccurate for highly non-linear instruments (deep OTM options).
    TaylorApproximation,
}

/// Configuration for VaR calculation.
///
/// Controls statistical properties such as confidence level and pricing method.
/// The historical window/observation count is derived from [`MarketHistory`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct VarConfig {
    /// Confidence level (e.g., 0.95 for 95% VaR, 0.99 for 99% VaR)
    pub confidence_level: f64,

    /// VaR calculation method
    pub method: VarMethod,

    /// Optional reporting currency for portfolio aggregation.
    ///
    /// When omitted, same-currency portfolios use their natural currency.
    /// Mixed-currency portfolios must set this explicitly.
    pub reporting_currency: Option<Currency>,
}

impl Default for VarConfig {
    fn default() -> Self {
        Self::var_95()
    }
}

impl VarConfig {
    /// Create a new VaR configuration with standard settings.
    ///
    /// # Arguments
    ///
    /// * `confidence_level` - Confidence level (e.g., 0.95, 0.99)
    pub fn new(confidence_level: f64) -> Self {
        Self {
            confidence_level,
            method: VarMethod::FullRevaluation,
            reporting_currency: None,
        }
    }

    /// Standard 95% VaR configuration.
    pub fn var_95() -> Self {
        Self::new(0.95)
    }

    /// Standard 99% VaR configuration.
    pub fn var_99() -> Self {
        Self::new(0.99)
    }

    /// Set the calculation method.
    pub fn with_method(mut self, method: VarMethod) -> Self {
        self.method = method;
        self
    }

    /// Set the reporting currency for portfolio aggregation.
    pub fn with_reporting_currency(mut self, currency: Currency) -> Self {
        self.reporting_currency = Some(currency);
        self
    }

    /// Validate that this VaR configuration is numerically well-defined.
    ///
    /// # Errors
    ///
    /// Returns an error when `confidence_level` is not finite or is outside
    /// the open interval `(0, 1)`.
    pub fn validate(&self) -> Result<()> {
        validate_confidence_level(self.confidence_level)
    }
}

fn validate_confidence_level(confidence_level: f64) -> Result<()> {
    if confidence_level.is_finite() && confidence_level > 0.0 && confidence_level < 1.0 {
        Ok(())
    } else {
        Err(finstack_core::Error::Validation(format!(
            "VaR confidence level must be finite and strictly between 0 and 1, got {confidence_level}"
        )))
    }
}

/// VaR calculation results.
///
/// VaR and Expected Shortfall are both derived from a single quantile
/// definition: the R type-7 / numpy-default linear-interpolated empirical
/// quantile function (see [`VarResult::from_distribution`]). This guarantees
/// the two measures are mutually consistent.
#[derive(Debug, Clone)]
pub struct VarResult {
    /// Value-at-Risk at specified confidence level (always positive).
    ///
    /// The loss at the `(1 - α)` quantile of the P&L distribution, computed
    /// by linear interpolation between bracketing order statistics.
    pub var: f64,

    /// Expected Shortfall (CVaR) at specified confidence level (always positive).
    ///
    /// The conditional tail expectation: the average loss over the worst
    /// `(1 - α)` fraction of the distribution, computed as the mean of the
    /// interpolated quantile function over `[0, 1 - α]`. By construction
    /// `expected_shortfall >= var`.
    pub expected_shortfall: f64,

    /// Full P&L distribution from historical simulation (sorted, worst first)
    pub pnl_distribution: Vec<f64>,

    /// Number of scenarios used in calculation
    pub num_scenarios: usize,

    /// Confidence level used
    pub confidence_level: f64,
}

impl VarResult {
    /// Create VaR result from P&L distribution.
    ///
    /// # Arguments
    ///
    /// * `pnl_distribution` - Unsorted P&L values from historical simulation
    /// * `confidence_level` - Confidence level for VaR/ES calculation
    pub fn from_distribution(
        mut pnl_distribution: Vec<f64>,
        confidence_level: f64,
    ) -> Result<Self> {
        validate_confidence_level(confidence_level)?;

        if pnl_distribution.iter().any(|v| !v.is_finite()) {
            return Err(finstack_core::Error::Validation(
                "VaR P&L distribution contains non-finite values (NaN or inf)".to_string(),
            ));
        }

        // Sort P&L distribution (ascending = worst losses first)
        pnl_distribution.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let num_scenarios = pnl_distribution.len();

        // Handle empty distribution
        if num_scenarios == 0 {
            return Ok(Self {
                var: 0.0,
                expected_shortfall: 0.0,
                pnl_distribution,
                num_scenarios,
                confidence_level,
            });
        }

        // VaR and ES are both derived from ONE quantile definition: the R
        // type-7 / numpy-default linear-interpolated empirical quantile (see
        // `Jorion, Value at Risk, 3rd ed.`). This avoids the discontinuous
        // step in `n` of an integer-index estimator and keeps VaR and ES
        // mutually consistent.
        //
        // * VaR is the loss at the lower tail probability `p = 1 - α`,
        //   interpolated between the two bracketing order statistics.
        // * ES is the conditional tail expectation: the mean of the quantile
        //   function over the tail `[0, p]`, i.e. the genuine average loss
        //   beyond the `p` quantile. Because it integrates the SAME quantile
        //   function used for VaR, `ES >= VaR` holds by construction.
        let p = 1.0 - confidence_level;
        let var = (-tail_quantile(&pnl_distribution, p)).max(0.0);
        let expected_shortfall = (-tail_quantile_mean(&pnl_distribution, p)).max(0.0);

        // Warn about statistical reliability for small sample sizes
        // With fewer than 20 scenarios, the quantile estimates may be unreliable
        // and the discrete nature of the distribution becomes more significant
        const MIN_RELIABLE_SCENARIOS: usize = 20;
        if num_scenarios < MIN_RELIABLE_SCENARIOS && num_scenarios > 0 {
            tracing::warn!(
                num_scenarios = num_scenarios,
                confidence_level = confidence_level,
                var = var,
                expected_shortfall = expected_shortfall,
                "VaR calculated with fewer than {} scenarios. Statistical reliability is limited: \
                 the interpolated tail quantile and ES have high sampling variance. \
                 Consider using more historical observations or stress scenarios.",
                MIN_RELIABLE_SCENARIOS
            );
        }

        Ok(Self {
            var,
            expected_shortfall,
            pnl_distribution,
            num_scenarios,
            confidence_level,
        })
    }
}

/// Evaluate the R type-7 / numpy-default linear-interpolated empirical
/// quantile function `Q(u)` of an ascending-sorted sample.
///
/// For `n` observations the quantile at probability `u in [0, 1]` is found at
/// fractional rank `h = (n - 1) * u`: linearly interpolating between order
/// statistics `sorted[floor(h)]` and `sorted[floor(h) + 1]`. This is the
/// estimator used by `numpy.quantile` (default) and R's `quantile(type = 7)`.
///
/// `sorted` must be non-empty and sorted ascending; `u` is clamped to `[0, 1]`.
fn type7_quantile(sorted: &[f64], u: f64) -> f64 {
    let n = sorted.len();
    // Single observation: the quantile function is the constant `sorted[0]`.
    if n <= 1 {
        return sorted[0];
    }
    let n_minus_1 = (n - 1) as f64;
    let h = (n_minus_1 * u.clamp(0.0, 1.0)).clamp(0.0, n_minus_1);
    // `lo` indexes the lower bracketing order statistic; clamping to `n - 2`
    // keeps `lo + 1` in range when `h` lands exactly on the last knot.
    let lo = (h.floor() as usize).min(n - 2);
    let frac = h - lo as f64;
    sorted[lo] + frac * (sorted[lo + 1] - sorted[lo])
}

/// Lower-tail quantile of the P&L distribution at tail probability `p`.
///
/// Uses the [`type7_quantile`] estimator. `sorted` must be the ascending
/// (worst-loss-first) P&L distribution; `p = 1 - confidence_level`.
fn tail_quantile(sorted: &[f64], p: f64) -> f64 {
    type7_quantile(sorted, p)
}

/// Mean of the type-7 quantile function over the tail `[0, p]` — the genuine
/// Expected Shortfall of the empirical distribution at tail probability `p`.
///
/// `ES = (1 / p) * integral over [0, p] of Q(u) du`, where `Q` is the
/// piecewise-linear [`type7_quantile`] function. The integral is evaluated
/// exactly by summing the trapezoidal area of each linear piece of `Q` that
/// the interval `[0, p]` crosses (knots sit at `u = i / (n - 1)`).
///
/// This integrates the SAME quantile function used by [`tail_quantile`], so
/// VaR and ES are mutually consistent: ES is the average of `Q` over `[0, p]`
/// and VaR is `Q(p)`, the least-extreme point of that interval. Since `Q` is
/// non-decreasing, the tail average is at least as extreme as its endpoint,
/// hence (with the positive-loss sign convention) `ES >= VaR` always holds.
///
/// `sorted` must be non-empty and sorted ascending; `p` must lie in `(0, 1)`.
fn tail_quantile_mean(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    // Single observation: the quantile function is constant, so its tail mean
    // is that single value.
    if n <= 1 {
        return sorted[0];
    }
    let n_minus_1 = (n - 1) as f64;
    let p = p.clamp(0.0, 1.0);
    let h_p = n_minus_1 * p;

    // Accumulate the exact area under `Q` over `[0, p]` piece by piece. Each
    // linear piece `j` of `Q` spans `u in [j / (n-1), (j+1) / (n-1)]`; the
    // contributing sub-interval is its intersection with `[0, p]`.
    let segment_area = (0..n - 1).map_while(|j| {
        let seg_start = j as f64;
        if seg_start >= h_p {
            return None;
        }
        // Sub-interval `[a, b]` of segment `j`, expressed in fractional-rank
        // units `h = (n-1) * u`, then in probability units for the width.
        let a_h = seg_start;
        let b_h = h_p.min(seg_start + 1.0);
        let q_a = sorted[j] + (a_h - seg_start) * (sorted[j + 1] - sorted[j]);
        let q_b = sorted[j] + (b_h - seg_start) * (sorted[j + 1] - sorted[j]);
        // Width in probability units: (b_h - a_h) / (n - 1). Trapezoid area.
        let width = (b_h - a_h) / n_minus_1;
        Some(width * 0.5 * (q_a + q_b))
    });
    let integral = neumaier_sum(segment_area);

    // `p` is strictly positive (confidence_level is validated to (0, 1)).
    integral / p
}

/// Calculate Historical VaR for a single instrument using full revaluation.
///
/// # Arguments
///
/// * `instrument` - The instrument to calculate VaR for
/// * `base_market` - Current market context (base case)
/// * `history` - Historical market scenarios
/// * `as_of` - Valuation date
/// * `config` - VaR configuration
///
/// # Returns
///
/// VaR result including VaR, ES, and full P&L distribution
///
/// This function revalues the instrument under every scenario contained in
/// [`MarketHistory`]. If the history is empty, the returned VaR/ES will be zero.
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_valuations::instruments::{Bond, Instrument};
/// use finstack_valuations::metrics::risk::{calculate_var, MarketHistory, MarketScenario, VarConfig};
/// use finstack_core::market_data::context::MarketContext;
/// use time::macros::date;
///
/// # fn main() -> finstack_core::Result<()> {
/// let bond = Bond::example().unwrap();
/// let market = MarketContext::new();
/// let as_of = date!(2025-01-01);
/// let history = MarketHistory::new(as_of, 0, Vec::<MarketScenario>::new());
/// let config = VarConfig::var_95();
///
/// let instruments = [&bond];
/// let result = calculate_var(&instruments, &market, &history, as_of, &config)?;
/// println!("95% VaR: ${:.2}", result.var);
/// println!("95% ES: ${:.2}", result.expected_shortfall);
/// # Ok(())
/// # }
/// ```
pub fn calculate_var<I>(
    instruments: &[&I],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
) -> Result<VarResult>
where
    I: Instrument,
{
    let instrument_refs: Vec<&dyn Instrument> = instruments
        .iter()
        .map(|inst| *inst as &dyn Instrument)
        .collect();
    calculate_var_dyn(&instrument_refs, base_market, history, as_of, config)
}

fn calculate_var_dyn(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
) -> Result<VarResult> {
    calculate_var_with_pricing(instruments, base_market, history, as_of, config, None, None)
}

/// Variant of [`calculate_var`] that reuses a caller-selected pricing engine.
pub fn calculate_var_with_pricing(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<VarResult> {
    config.validate()?;

    let _span = tracing::debug_span!(
        "historical_var.calculate",
        instrument_count = instruments.len(),
        scenario_count = history.len(),
        method = ?config.method,
        confidence_level = config.confidence_level,
    )
    .entered();

    if instruments.is_empty() {
        return VarResult::from_distribution(Vec::new(), config.confidence_level);
    }

    match config.method {
        VarMethod::FullRevaluation => calculate_var_full_revaluation(
            instruments,
            base_market,
            history,
            as_of,
            config,
            pricing_model,
            pricer_registry,
        ),
        VarMethod::TaylorApproximation => calculate_var_taylor(
            instruments,
            base_market,
            history,
            as_of,
            config,
            pricing_model,
            pricer_registry,
        ),
    }
}

fn reprice_with_dispatch(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<&Arc<PricerRegistry>>,
) -> Result<Money> {
    if let (Some(model), Some(registry)) = (pricing_model, pricer_registry) {
        return Ok(registry
            .price_with_metrics(instrument, model, market, as_of, &[], Default::default())?
            .value);
    }
    instrument.value(market, as_of)
}

/// Aggregate scenario P&Ls in parallel using rayon.
///
/// Each scenario is independent (creates its own `MarketContext`), making this
/// embarrassingly parallel. The closure must be `Fn + Send + Sync` (not `FnMut`)
/// because multiple threads may invoke it concurrently.
fn aggregate_scenario_pnls_par<F>(
    history: &MarketHistory,
    base_market: &MarketContext,
    scenario_pnl: F,
) -> Result<Vec<f64>>
where
    F: Fn(&MarketContext) -> Result<f64> + Send + Sync,
{
    use rayon::prelude::*;
    history
        .scenarios
        .par_iter()
        .map(|scenario| {
            let scenario_market = scenario.apply(base_market)?;
            scenario_pnl(&scenario_market)
        })
        .collect()
}

/// Calculate VaR using full revaluation method.
fn calculate_var_full_revaluation(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<VarResult> {
    let _span = tracing::debug_span!(
        "historical_var.full_revaluation",
        instrument_count = instruments.len(),
        scenario_count = history.len(),
    )
    .entered();

    let instrument_refs: Vec<&dyn Instrument> = instruments.to_vec();
    let base_values_money: Vec<Money> = instrument_refs
        .iter()
        .map(|inst| {
            reprice_with_dispatch(
                *inst,
                base_market,
                as_of,
                pricing_model,
                pricer_registry.as_ref(),
            )
        })
        .collect::<Result<_>>()?;
    let reporting_currency = resolve_reporting_currency_from_values(
        &base_values_money,
        config.reporting_currency,
        "Historical VaR",
    )?;
    let base_values: Vec<f64> = base_values_money
        .iter()
        .map(|value| {
            convert_money_to_reporting(
                *value,
                reporting_currency,
                base_market,
                as_of,
                "Historical VaR",
            )
        })
        .collect::<Result<_>>()?;

    let scenario_pnl = move |scenario_market: &MarketContext| {
        let mut acc = NeumaierAccumulator::new();
        for (inst, base_amount) in instrument_refs.iter().zip(base_values.iter()) {
            let scenario_amount = convert_money_to_reporting(
                reprice_with_dispatch(
                    *inst,
                    scenario_market,
                    as_of,
                    pricing_model,
                    pricer_registry.as_ref(),
                )?,
                reporting_currency,
                scenario_market,
                as_of,
                "Historical VaR",
            )?;
            acc.add(scenario_amount - base_amount);
        }
        Ok(acc.total())
    };

    let pnls = aggregate_scenario_pnls_par(history, base_market, scenario_pnl)?;

    // Calculate VaR and ES from P&L distribution
    VarResult::from_distribution(pnls, config.confidence_level)
}

fn calculate_var_taylor(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<VarResult> {
    let _span = tracing::debug_span!(
        "historical_var.taylor",
        instrument_count = instruments.len(),
        scenario_count = history.len(),
    )
    .entered();

    if instruments.len() == 1 {
        // Use clone_box to get a sized type for Taylor approximation
        let boxed = instruments[0].clone_box();
        return calculate_var_taylor_approximation(
            &*boxed,
            base_market,
            history,
            as_of,
            config,
            pricing_model,
            pricer_registry,
        );
    }

    let boxed: Vec<Box<dyn Instrument>> = instruments.iter().map(|inst| inst.clone_box()).collect();
    let refs: Vec<&dyn Instrument> = boxed.iter().map(|b| b.as_ref()).collect();
    calculate_portfolio_var_taylor(
        &refs,
        base_market,
        history,
        as_of,
        config,
        pricing_model,
        pricer_registry,
    )
}

// =============================================================================
// Taylor Approximation
// =============================================================================

#[derive(Default)]
struct BucketedSeries {
    per_curve: HashMap<String, HashMap<String, f64>>,
    fallback: HashMap<String, f64>,
}

impl BucketedSeries {
    fn get(&self, curve_id: &str, bucket: &str) -> Option<f64> {
        if let Some(curve) = self.per_curve.get(curve_id) {
            if let Some(value) = curve.get(bucket) {
                return Some(*value);
            }
        }
        self.fallback.get(bucket).copied()
    }
}

struct TaylorSensitivities {
    currency: Currency,
    dv01: BucketedSeries,
    cs01: BucketedSeries,
    ir_convexity: f64,
    equity_delta: f64,
    equity_gamma: f64,
    vega_rel: f64,
}

impl Default for TaylorSensitivities {
    fn default() -> Self {
        Self {
            currency: Currency::USD,
            dv01: BucketedSeries::default(),
            cs01: BucketedSeries::default(),
            ir_convexity: 0.0,
            equity_delta: 0.0,
            equity_gamma: 0.0,
            vega_rel: 0.0,
        }
    }
}

fn resolve_reporting_currency_from_values(
    values: &[Money],
    requested: Option<Currency>,
    context_name: &str,
) -> Result<Currency> {
    if let Some(currency) = requested {
        return Ok(currency);
    }

    let mut observed: Option<Currency> = None;
    for value in values {
        match observed {
            None => observed = Some(value.currency()),
            Some(existing) if existing == value.currency() => {}
            Some(_) => {
                return Err(finstack_core::Error::Validation(format!(
                    "{context_name} requires an explicit reporting currency for mixed-currency portfolios"
                )))
            }
        }
    }

    observed.ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "{context_name} requires at least one instrument to infer reporting currency"
        ))
    })
}

fn convert_money_to_reporting(
    value: Money,
    reporting_currency: Currency,
    market: &MarketContext,
    on: Date,
    context_name: &str,
) -> Result<f64> {
    if value.currency() == reporting_currency {
        return Ok(value.amount());
    }

    let fx = market.fx().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "{context_name} requires FX data to convert {} into reporting currency {}",
            value.currency(),
            reporting_currency
        ))
    })?;
    let rate = fx
        .rate(FxQuery::with_policy(
            value.currency(),
            reporting_currency,
            on,
            FxConversionPolicy::CashflowDate,
        ))?
        .rate;
    Ok(value.amount() * rate)
}

fn calculate_var_taylor_approximation(
    instrument: &dyn Instrument,
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<VarResult> {
    let base_value = reprice_with_dispatch(
        instrument,
        base_market,
        as_of,
        pricing_model,
        pricer_registry.as_ref(),
    )?;
    let reporting_currency = resolve_reporting_currency_from_values(
        &[base_value],
        config.reporting_currency,
        "Historical VaR",
    )?;
    let sensitivities = compute_taylor_sensitivities(
        instrument,
        base_market,
        as_of,
        base_value,
        pricing_model,
        pricer_registry,
    )?;

    let mut spot_cache: HashMap<String, f64> = HashMap::default();
    let mut pnls = Vec::with_capacity(history.len());

    for scenario in history.iter() {
        let pnl_local =
            taylor_pnl_for_scenario(&sensitivities, base_market, scenario, &mut spot_cache)?;
        // Convert P&L using the scenario-shifted market's FX rates, not the
        // base market's: when a scenario shocks FX, the reporting-currency
        // value of the P&L must reflect the shifted rates. This mirrors the
        // full-revaluation path (`calculate_var_full_revaluation`).
        let scenario_market = scenario.apply(base_market)?;
        pnls.push(convert_money_to_reporting(
            Money::new(pnl_local, sensitivities.currency),
            reporting_currency,
            &scenario_market,
            as_of,
            "Historical VaR",
        )?);
    }

    VarResult::from_distribution(pnls, config.confidence_level)
}

fn compute_taylor_sensitivities(
    instrument: &dyn Instrument,
    base_market: &MarketContext,
    as_of: Date,
    base_value: Money,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<TaylorSensitivities> {
    let instrument_type = instrument.key();
    let registry = standard_registry();
    let instrument_arc = instrument_to_arc(instrument);
    let mut context = MetricContext::new(
        instrument_arc,
        Arc::new(base_market.clone()),
        as_of,
        base_value,
        MetricContext::default_config(),
    );
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
    context.set_pricer_dispatch(pricing_model, pricer_registry);

    let candidate_metrics = [
        MetricId::BucketedDv01,
        MetricId::Dv01,
        MetricId::BucketedCs01,
        MetricId::Cs01,
        MetricId::IrConvexity,
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::IndexDelta,
        MetricId::EquityShares,
        MetricId::Vega,
    ];
    let metrics: Vec<MetricId> = candidate_metrics
        .into_iter()
        .filter(|metric_id| registry.is_applicable(metric_id, instrument_type))
        .collect();

    let computed = registry.compute(&metrics, &mut context)?;

    let dv01 = collect_bucketed_series(&context.computed_series, MetricId::BucketedDv01.as_str());
    let cs01 = collect_bucketed_series(&context.computed_series, MetricId::BucketedCs01.as_str());

    let ir_convexity = required_metric(
        &computed,
        &MetricId::IrConvexity,
        registry.is_applicable(&MetricId::IrConvexity, instrument_type),
        instrument.id(),
    )?;

    let has_delta = registry.is_applicable(&MetricId::Delta, instrument_type);
    let has_gamma = registry.is_applicable(&MetricId::Gamma, instrument_type);
    let has_index_delta = registry.is_applicable(&MetricId::IndexDelta, instrument_type);
    let has_equity_shares = registry.is_applicable(&MetricId::EquityShares, instrument_type);
    let has_vega = registry.is_applicable(&MetricId::Vega, instrument_type);

    let delta = if has_delta {
        required_metric(&computed, &MetricId::Delta, true, instrument.id())?
    } else if has_index_delta {
        required_metric(&computed, &MetricId::IndexDelta, true, instrument.id())?
    } else if has_equity_shares {
        required_metric(&computed, &MetricId::EquityShares, true, instrument.id())?
    } else {
        0.0
    };

    let gamma = if has_gamma {
        required_metric(&computed, &MetricId::Gamma, true, instrument.id())?
    } else {
        0.0
    };

    let vega_rel = if has_vega {
        required_metric(&computed, &MetricId::Vega, true, instrument.id())?
    } else {
        0.0
    };

    Ok(TaylorSensitivities {
        currency: base_value.currency(),
        dv01,
        cs01,
        ir_convexity,
        equity_delta: delta,
        equity_gamma: gamma,
        vega_rel,
    })
}

/// Tolerance (in basis points) for treating a set of rate-bucket shifts as a
/// parallel curve move. If every bucket shift lies within this band of the
/// first observed shift, the move is considered parallel and the second-order
/// convexity term is applied; otherwise it is omitted (see `taylor_pnl_for_scenario`).
const PARALLEL_SHIFT_TOLERANCE_BP: f64 = 1e-6;

fn taylor_pnl_for_scenario(
    sensitivities: &TaylorSensitivities,
    base_market: &MarketContext,
    scenario: &crate::metrics::risk::MarketScenario,
    spot_cache: &mut HashMap<String, f64>,
) -> Result<f64> {
    let mut pnl = 0.0;
    let mut rate_shift_count = 0u32;
    // Track the first rate-bucket shift and whether all subsequent rate
    // shifts match it (within tolerance), i.e. the move is a parallel shift.
    let mut first_rate_shift_bp = 0.0;
    let mut rate_shifts_parallel = true;

    for shift in &scenario.shifts {
        match &shift.factor {
            crate::metrics::risk::RiskFactorType::DiscountRate {
                curve_id,
                tenor_years,
            }
            | crate::metrics::risk::RiskFactorType::ForwardRate {
                curve_id,
                tenor_years,
            } => {
                let bucket = format_bucket_label_cow(*tenor_years);
                let dv01 = sensitivities
                    .dv01
                    .get(curve_id.as_str(), bucket.as_ref())
                    .ok_or_else(|| {
                        missing_taylor_sensitivity_error("DV01", curve_id.as_str(), bucket.as_ref())
                    })?;
                let shift_bp = shift.shift * 10_000.0;
                pnl += dv01 * shift_bp;
                if rate_shift_count == 0 {
                    first_rate_shift_bp = shift_bp;
                } else if (shift_bp - first_rate_shift_bp).abs() > PARALLEL_SHIFT_TOLERANCE_BP {
                    rate_shifts_parallel = false;
                }
                rate_shift_count += 1;
            }
            crate::metrics::risk::RiskFactorType::CreditSpread {
                curve_id,
                tenor_years,
            } => {
                let bucket = format_bucket_label_cow(*tenor_years);
                let cs01 = sensitivities
                    .cs01
                    .get(curve_id.as_str(), bucket.as_ref())
                    .ok_or_else(|| {
                        missing_taylor_sensitivity_error("CS01", curve_id.as_str(), bucket.as_ref())
                    })?;
                pnl += cs01 * shift.shift * 10_000.0;
            }
            crate::metrics::risk::RiskFactorType::EquitySpot { ticker } => {
                if sensitivities.equity_delta.abs() > 0.0 || sensitivities.equity_gamma.abs() > 0.0
                {
                    let spot = *spot_cache
                        .entry(ticker.clone())
                        .or_insert_with(|| spot_from_market(base_market, ticker).unwrap_or(-1.0));
                    if spot <= 0.0 {
                        return Err(finstack_core::Error::Validation(format!(
                            "Historical VaR missing positive finite spot for equity factor '{}'",
                            ticker
                        )));
                    }
                    let d_spot = spot * shift.shift;
                    pnl += sensitivities.equity_delta * d_spot
                        + 0.5 * sensitivities.equity_gamma * d_spot * d_spot;
                }
            }
            crate::metrics::risk::RiskFactorType::FxSpot { base, quote } => {
                return Err(finstack_core::Error::Validation(format!(
                    "Historical VaR Taylor approximation does not support FX spot shocks for {base}/{quote}. Use full revaluation or an FX-delta Taylor implementation."
                )));
            }
            crate::metrics::risk::RiskFactorType::ImpliedVol {
                surface_id,
                expiry_years,
                strike,
            } => {
                if sensitivities.vega_rel.abs() > 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "Historical VaR Taylor approximation does not support ImpliedVol point shocks with aggregate Vega (surface='{}', expiry_years={}, strike={}). Use full revaluation or a bucketed-vol Taylor implementation.",
                        surface_id.as_str(),
                        expiry_years,
                        strike
                    )));
                }
            }
        }
    }

    // Second-order rate term: 0.5 * convexity * (shift_bp)^2.
    //
    // `ir_convexity` is an aggregate (parallel-shift) quantity: it is only
    // meaningful when multiplied by a single parallel shift magnitude.
    // Averaging signed per-bucket shifts is incorrect for non-parallel moves
    // (e.g. a steepener of +10bp/-10bp averages to ~0, which would zero out
    // the convexity P&L and badly understate risk for options on rates).
    //
    // We therefore only apply the convexity term when the scenario's rate
    // shifts are a (near-)parallel move, using the common parallel shift.
    // For non-parallel moves we omit the second-order term entirely rather
    // than computing a wrong number; the first-order DV01 contribution is
    // still captured per bucket above. Per-bucket convexity data is not
    // available here (only an aggregate `ir_convexity`), so summing
    // 0.5*convexity_bucket*shift_bucket^2 is not an option.
    if sensitivities.ir_convexity.abs() > 0.0 && rate_shift_count > 0 && rate_shifts_parallel {
        pnl += 0.5 * sensitivities.ir_convexity * first_rate_shift_bp * first_rate_shift_bp;
    }

    Ok(pnl)
}

fn missing_taylor_sensitivity_error(
    metric: &str,
    curve_id: &str,
    bucket: &str,
) -> finstack_core::Error {
    finstack_core::Error::Validation(format!(
        "Historical VaR Taylor approximation missing {metric} sensitivity for curve '{curve_id}' bucket '{bucket}'"
    ))
}

fn required_metric(
    computed: &HashMap<MetricId, f64>,
    metric_id: &MetricId,
    required: bool,
    instrument_id: &str,
) -> Result<f64> {
    if !required {
        return Ok(0.0);
    }

    computed.get(metric_id).copied().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "Historical VaR missing required metric '{}' for instrument '{}'",
            metric_id.as_str(),
            instrument_id,
        ))
    })
}

fn collect_bucketed_series(
    series_map: &HashMap<MetricId, Vec<(String, f64)>>,
    base_id: &str,
) -> BucketedSeries {
    let mut result = BucketedSeries::default();

    for (metric_id, series) in series_map {
        let id_str = metric_id.as_str();
        if id_str == base_id {
            result.fallback = series.iter().cloned().collect();
        } else if let Some(curve_id) = id_str.strip_prefix(&format!("{base_id}::")) {
            let entry = result
                .per_curve
                .entry(curve_id.to_string())
                .or_insert_with(HashMap::default);
            for (bucket, value) in series {
                entry.insert(bucket.clone(), *value);
            }
        }
    }

    result
}

fn spot_from_market(market: &MarketContext, ticker: &str) -> Option<f64> {
    match market.get_price(ticker) {
        Ok(MarketScalar::Unitless(v)) => Some(*v),
        Ok(MarketScalar::Price(m)) => Some(m.amount()),
        _ => None,
    }
}

fn calculate_portfolio_var_taylor(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    pricing_model: Option<ModelKey>,
    pricer_registry: Option<Arc<PricerRegistry>>,
) -> Result<VarResult> {
    if instruments.is_empty() {
        return Ok(VarResult {
            var: 0.0,
            expected_shortfall: 0.0,
            pnl_distribution: vec![],
            confidence_level: config.confidence_level,
            num_scenarios: 0,
        });
    }

    let mut sensitivities: Vec<TaylorSensitivities> = Vec::with_capacity(instruments.len());
    let mut base_values: Vec<Money> = Vec::with_capacity(instruments.len());
    for instrument in instruments {
        let base_value = reprice_with_dispatch(
            *instrument,
            base_market,
            as_of,
            pricing_model,
            pricer_registry.as_ref(),
        )?;
        base_values.push(base_value);
        sensitivities.push(compute_taylor_sensitivities(
            *instrument,
            base_market,
            as_of,
            base_value,
            pricing_model,
            pricer_registry.clone(),
        )?);
    }
    let reporting_currency = resolve_reporting_currency_from_values(
        &base_values,
        config.reporting_currency,
        "Historical VaR",
    )?;

    let mut spot_cache: HashMap<String, f64> = HashMap::default();
    let mut pnls = Vec::with_capacity(history.len());

    for scenario in history.iter() {
        // Build the scenario-shifted market once per scenario so P&L is
        // converted to the reporting currency at the scenario's FX rates,
        // matching the full-revaluation path.
        let scenario_market = scenario.apply(base_market)?;
        let mut acc = NeumaierAccumulator::new();
        for sens in &sensitivities {
            let pnl_local = taylor_pnl_for_scenario(sens, base_market, scenario, &mut spot_cache)?;
            let term = convert_money_to_reporting(
                Money::new(pnl_local, sens.currency),
                reporting_currency,
                &scenario_market,
                as_of,
                "Historical VaR",
            )?;
            acc.add(term);
        }
        pnls.push(acc.total());
    }

    VarResult::from_distribution(pnls, config.confidence_level)
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/metrics_risk_test_utils.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Attributes;
    use crate::metrics::risk::{MarketScenario, RiskFactorShift, RiskFactorType};
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use std::sync::Arc;
    use test_utils::{
        history_from_rate_shifts, history_from_scenarios, sample_as_of, standard_bond,
        usd_ois_market,
    };
    use time::macros::date;

    #[derive(Clone, Debug)]
    struct CurrencyScalarInstrument {
        id: String,
        price_id: String,
        currency: Currency,
        attributes: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        CurrencyScalarInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl CurrencyScalarInstrument {
        fn new(id: &str, price_id: &str, currency: Currency) -> Self {
            Self {
                id: id.to_string(),
                price_id: price_id.to_string(),
                currency,
                attributes: Attributes::new(),
            }
        }
    }

    impl Instrument for CurrencyScalarInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> crate::pricer::InstrumentType {
            crate::pricer::InstrumentType::Equity
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
            match market.get_price(&self.price_id)? {
                MarketScalar::Price(m) => Ok(*m),
                MarketScalar::Unitless(v) => Ok(Money::new(*v, self.currency)),
            }
        }
    }

    #[test]
    fn test_var_config_creation() {
        let config = VarConfig::var_95();
        assert_eq!(config.confidence_level, 0.95);
        assert_eq!(config.method, VarMethod::FullRevaluation);

        let config = VarConfig::var_99().with_method(VarMethod::TaylorApproximation);
        assert_eq!(config.confidence_level, 0.99);
        assert_eq!(config.method, VarMethod::TaylorApproximation);
    }

    #[test]
    fn mixed_currency_var_requires_explicit_reporting_currency() {
        let as_of = sample_as_of();
        let usd = CurrencyScalarInstrument::new("USD-INST", "USD-INST", Currency::USD);
        let eur = CurrencyScalarInstrument::new("EUR-INST", "EUR-INST", Currency::EUR);

        let base_market = MarketContext::new()
            .insert_price(
                "USD-INST",
                MarketScalar::Price(Money::new(100.0, Currency::USD)),
            )
            .insert_price(
                "EUR-INST",
                MarketScalar::Price(Money::new(100.0, Currency::EUR)),
            );

        let scenario = MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::EquitySpot {
                        ticker: "USD-INST".to_string(),
                    },
                    shift: 0.10,
                },
                RiskFactorShift {
                    factor: RiskFactorType::EquitySpot {
                        ticker: "EUR-INST".to_string(),
                    },
                    shift: 0.10,
                },
            ],
        );
        let history = MarketHistory::new(as_of, 1, vec![scenario]);

        let err = calculate_var_dyn(
            &[&usd as &dyn Instrument, &eur as &dyn Instrument],
            &base_market,
            &history,
            as_of,
            &VarConfig::var_95(),
        )
        .expect_err(
            "mixed-currency VaR must not aggregate native-currency P&L without a reporting currency",
        );
        assert!(
            err.to_string().contains("reporting currency"),
            "expected reporting currency validation error, got: {err}"
        );
    }

    #[test]
    fn mixed_currency_var_converts_pnl_into_reporting_currency() -> Result<()> {
        let as_of = sample_as_of();
        let usd = CurrencyScalarInstrument::new("USD-INST", "USD-INST", Currency::USD);
        let eur = CurrencyScalarInstrument::new("EUR-INST", "EUR-INST", Currency::EUR);

        let provider = finstack_core::money::fx::SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 2.0)
            .expect("valid rate");
        let fx = finstack_core::money::fx::FxMatrix::new(Arc::new(provider));

        let base_market = MarketContext::new()
            .insert_price(
                "USD-INST",
                MarketScalar::Price(Money::new(100.0, Currency::USD)),
            )
            .insert_price(
                "EUR-INST",
                MarketScalar::Price(Money::new(100.0, Currency::EUR)),
            )
            .insert_fx(fx);

        let scenario = MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::EquitySpot {
                        ticker: "USD-INST".to_string(),
                    },
                    shift: 0.10,
                },
                RiskFactorShift {
                    factor: RiskFactorType::EquitySpot {
                        ticker: "EUR-INST".to_string(),
                    },
                    shift: 0.10,
                },
            ],
        );
        let history = MarketHistory::new(as_of, 1, vec![scenario]);

        let result = calculate_var_dyn(
            &[&usd as &dyn Instrument, &eur as &dyn Instrument],
            &base_market,
            &history,
            as_of,
            &VarConfig::var_95().with_reporting_currency(Currency::USD),
        )?;
        assert!((result.pnl_distribution[0] - 30.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_taylor_method_matches_full_revaluation_small_shifts() -> Result<()> {
        let as_of = sample_as_of();
        let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
        let base_market = usd_ois_market(as_of)?;
        let history = history_from_rate_shifts(
            as_of,
            &[
                (date!(2023 - 12 - 31), 0.0005),
                (date!(2023 - 12 - 30), -0.0003),
                (date!(2023 - 12 - 29), 0.0002),
            ],
        );
        let full_config = VarConfig::var_95();
        let taylor_config = VarConfig::var_95().with_method(VarMethod::TaylorApproximation);

        let full = calculate_var(&[&bond], &base_market, &history, as_of, &full_config)?;
        let taylor = calculate_var(&[&bond], &base_market, &history, as_of, &taylor_config)?;

        assert!(taylor.var > 0.0, "Taylor VaR should be positive");
        let diff = (taylor.var - full.var).abs();
        let rel = diff / full.var.max(1.0);
        assert!(
            rel < 0.15,
            "Taylor VaR should be close to full revaluation (diff: {:.4}%)",
            rel * 100.0
        );

        Ok(())
    }

    /// Verifies the Taylor VaR shift convention: shifts are in decimal form (e.g., 0.0001 = 1bp)
    /// and DV01 is "per basis point", so the P&L formula is:
    ///
    ///   P&L = DV01_per_bp × shift_decimal × 10,000
    ///       = DV01_per_bp × shift_bp
    ///
    /// This test uses a known linear instrument (bond) with a single scenario
    /// to verify the scaling convention produces matching results.
    #[test]
    fn test_taylor_shift_convention_matches_full_revaluation_linear_instrument() -> Result<()> {
        let as_of = sample_as_of();
        // 5-year bond: approximately linear in rate changes for small shifts
        let bond = standard_bond("LINEAR-BOND", as_of, date!(2029 - 01 - 01));
        let base_market = usd_ois_market(as_of)?;

        // Single scenario with a small parallel rate shift (5bp = 0.0005 decimal)
        // This tests the convention: shift.shift is in decimal form
        let shift_decimal = 0.0005; // 5 basis points
        let shift_bp = shift_decimal * 10_000.0; // = 5.0 bp

        let history = history_from_rate_shifts(as_of, &[(date!(2023 - 12 - 31), shift_decimal)]);

        // Full revaluation: direct repricing under shifted curve
        let full_config = VarConfig::var_95();
        let full_result = calculate_var(&[&bond], &base_market, &history, as_of, &full_config)?;

        // Taylor approximation: P&L ≈ DV01 × shift_bp
        let taylor_config = VarConfig::var_95().with_method(VarMethod::TaylorApproximation);
        let taylor_result = calculate_var(&[&bond], &base_market, &history, as_of, &taylor_config)?;

        // With a single scenario, VaR = P&L magnitude
        // For a linear instrument, Taylor should closely match full revaluation
        let full_pnl = full_result.pnl_distribution[0];
        let taylor_pnl = taylor_result.pnl_distribution[0];

        // Verify P&L has correct sign (rates up → bond value down → negative P&L)
        assert!(
            full_pnl < 0.0,
            "Full revaluation P&L should be negative for rate increase"
        );
        assert!(
            taylor_pnl < 0.0,
            "Taylor P&L should be negative for rate increase"
        );

        // Verify Taylor approximation is within 5% of full revaluation for this linear case
        // (Small difference expected due to convexity and discrete bumping)
        let rel_diff = (taylor_pnl - full_pnl).abs() / full_pnl.abs();
        assert!(
            rel_diff < 0.05,
            "Taylor P&L ({:.2}) should be within 5% of full revaluation P&L ({:.2}), got {:.2}%",
            taylor_pnl,
            full_pnl,
            rel_diff * 100.0
        );

        // Document the convention explicitly in the test
        // The Taylor formula in taylor_pnl_for_scenario is:
        //   pnl += dv01 * shift.shift * 10_000.0
        //
        // This means:
        //   - shift.shift is in DECIMAL form (0.0001 = 1bp)
        //   - DV01 is in currency units PER BASIS POINT
        //   - The 10,000 factor converts shift_decimal to shift_bp
        //
        // Example: DV01 = -500 (loses $500 per 1bp rise)
        //          shift = 0.0005 (5bp rise)
        //          P&L = -500 * 0.0005 * 10,000 = -500 * 5 = -2500
        println!(
            "Shift convention test passed: \
             shift_decimal={}, shift_bp={}, full_pnl={:.2}, taylor_pnl={:.2}, rel_diff={:.2}%",
            shift_decimal,
            shift_bp,
            full_pnl,
            taylor_pnl,
            rel_diff * 100.0
        );

        Ok(())
    }

    #[test]
    fn test_taylor_vol_pnl_rejects_aggregate_vega_for_vol_point() {
        let as_of = sample_as_of();
        let base_market = MarketContext::new().insert_surface(
            VolSurface::from_grid(
                "EQ-VOL",
                &[1.0, 2.0],
                &[100.0, 120.0],
                &[0.20, 0.20, 0.20, 0.20],
            )
            .expect("valid flat volatility surface for Vega scaling test"),
        );
        let scenario = MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::ImpliedVol {
                    surface_id: CurveId::new("EQ-VOL"),
                    expiry_years: 1.0,
                    strike: 100.0,
                },
                shift: 0.01,
            }],
        );
        let sensitivities = TaylorSensitivities {
            vega_rel: 250.0,
            ..Default::default()
        };

        let err = taylor_pnl_for_scenario(
            &sensitivities,
            &base_market,
            &scenario,
            &mut HashMap::default(),
        )
        .expect_err("Taylor VaR should not apply aggregate Vega to a vol point");

        assert!(
            err.to_string().contains("ImpliedVol") && err.to_string().contains("EQ-VOL"),
            "unsupported vol Taylor error should identify the surface, got: {err}"
        );
    }

    #[test]
    fn taylor_sensitivity_build_preserves_calculator_error() {
        let as_of = sample_as_of();
        let bond = standard_bond("MISSING-CURVE-BOND", as_of, date!(2029 - 01 - 01));
        let err = match compute_taylor_sensitivities(
            &bond,
            &MarketContext::new(),
            as_of,
            Money::new(100.0, Currency::USD),
            None,
            None,
        ) {
            Ok(_) => panic!("Taylor sensitivity build should fail on missing market inputs"),
            Err(err) => err,
        };

        let msg = err.to_string();
        assert!(
            !msg.contains("Historical VaR missing required metric"),
            "Taylor sensitivity errors should preserve calculator context, got: {msg}"
        );
        assert!(
            msg.contains("USD-TREASURY") || msg.contains("curve") || msg.contains("not found"),
            "Taylor sensitivity error should identify the missing curve, got: {msg}"
        );
    }

    #[test]
    fn taylor_rate_shift_errors_when_curve_bucket_sensitivity_is_missing() {
        let as_of = sample_as_of();
        let base_market = MarketContext::new();
        let scenario = MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::new("USD-LIBOR-3M"),
                    tenor_years: 7.0,
                },
                shift: 0.0001,
            }],
        );
        let sensitivities = TaylorSensitivities {
            ..Default::default()
        };

        let err = taylor_pnl_for_scenario(
            &sensitivities,
            &base_market,
            &scenario,
            &mut HashMap::default(),
        )
        .expect_err("Taylor VaR must not use a cross-curve parallel DV01 fallback");

        assert!(
            err.to_string().contains("USD-LIBOR-3M"),
            "missing sensitivity error should identify the curve, got: {err}"
        );
    }

    #[test]
    fn taylor_rate_shift_errors_when_same_curve_bucket_is_missing() {
        let as_of = sample_as_of();
        let base_market = MarketContext::new();
        let scenario = MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: CurveId::new("USD-LIBOR-3M"),
                    tenor_years: 7.0,
                },
                shift: 0.0001,
            }],
        );
        let mut curve_buckets = HashMap::default();
        curve_buckets.insert("5y".to_string(), 30.0);
        let mut dv01_by_curve = HashMap::default();
        dv01_by_curve.insert("USD-LIBOR-3M".to_string(), curve_buckets);
        let sensitivities = TaylorSensitivities {
            dv01: BucketedSeries {
                per_curve: dv01_by_curve,
                fallback: HashMap::default(),
            },
            ..Default::default()
        };

        let err = taylor_pnl_for_scenario(
            &sensitivities,
            &base_market,
            &scenario,
            &mut HashMap::default(),
        )
        .expect_err("missing tenor bucket should not fall back to same-curve total");

        assert!(
            err.to_string().contains("USD-LIBOR-3M") && err.to_string().contains("7y"),
            "missing sensitivity error should identify curve and bucket, got: {err}"
        );
    }

    #[test]
    fn test_taylor_vol_shock_matches_full_revaluation_for_fx_option() -> Result<()> {
        use crate::instruments::fx::fx_option::FxOption;
        use crate::instruments::{
            Attributes, ExerciseStyle, OptionType, PricingOverrides, SettlementType,
        };
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        use finstack_core::market_data::context::MarketContext;
        use finstack_core::market_data::scalars::MarketScalar;
        use finstack_core::market_data::surfaces::VolSurface;
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
        use finstack_core::money::Money;
        use finstack_core::types::{CurveId, InstrumentId};
        use std::sync::Arc;
        use time::macros::date;

        let as_of = date!(2025 - 01 - 02);
        let expiry = date!(2025 - 07 - 02);
        let option = FxOption::builder()
            .id(InstrumentId::new("FX-VAR-OPTION"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.15)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("valid FX option");

        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.17)
            .expect("valid rate");
        let fx = FxMatrix::new(Arc::new(provider));

        let usd_disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("valid USD curve");
        let eur_disc = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .build()
            .expect("valid EUR curve");
        let vol_surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[0.5, 1.0])
            .strikes(&[1.0, 1.15, 1.3])
            .row(&[0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20])
            .build()
            .expect("valid FX vol surface");

        let base_market = MarketContext::new()
            .insert(usd_disc)
            .insert(eur_disc)
            .insert_surface(vol_surface)
            .insert_price("FX_VOL_OVERRIDE", MarketScalar::Unitless(0.20))
            .insert_fx(fx);

        let scenario = crate::metrics::risk::MarketScenario::new(
            as_of,
            vec![crate::metrics::risk::RiskFactorShift {
                factor: crate::metrics::risk::RiskFactorType::ImpliedVol {
                    surface_id: CurveId::new("EURUSD-VOL"),
                    expiry_years: 0.5,
                    strike: 1.15,
                },
                shift: 0.01,
            }],
        );
        let history = MarketHistory::new(as_of, 1, vec![scenario]);

        let err = calculate_var_dyn(
            &[&option as &dyn Instrument],
            &base_market,
            &history,
            as_of,
            &VarConfig::var_95().with_method(VarMethod::TaylorApproximation),
        )
        .expect_err("Taylor VaR should reject aggregate Vega for point vol shocks");

        let msg = err.to_string();
        assert!(
            msg.contains("ImpliedVol") && msg.contains("EURUSD-VOL"),
            "Taylor vol rejection should identify the vol surface, got: {msg}"
        );

        Ok(())
    }

    /// BUG 1 regression: a non-parallel (steepener) rate scenario must still
    /// produce a non-zero Taylor P&L driven by first-order DV01. The previous
    /// implementation averaged signed bucket shifts; a +shift/-shift steepener
    /// averaged to ~0 and (with the asymmetric DV01 weighting per bucket) the
    /// first-order term is what carries the risk. We assert the steepener P&L
    /// is non-trivial and reflects net first-order exposure.
    #[test]
    fn test_taylor_steepener_reflects_first_order_risk() {
        let as_of = sample_as_of();
        // Bond DV01 buckets supplied directly so the test is deterministic and
        // independent of curve calibration.
        let mut dv01_buckets = HashMap::default();
        dv01_buckets.insert("2y".to_string(), -40.0);
        dv01_buckets.insert("10y".to_string(), -120.0);
        let mut dv01_by_curve = HashMap::default();
        dv01_by_curve.insert("USD-OIS".to_string(), dv01_buckets);

        // Non-zero aggregate convexity: must NOT contribute for a steepener.
        let sensitivities = TaylorSensitivities {
            currency: Currency::USD,
            dv01: BucketedSeries {
                per_curve: dv01_by_curve,
                fallback: HashMap::default(),
            },
            ir_convexity: 5_000.0,
            ..Default::default()
        };

        // Steepener: +10bp at 2y, -10bp at 10y. Signed average ~= 0.
        let steepener = MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::new("USD-OIS"),
                        tenor_years: 2.0,
                    },
                    shift: 0.0010,
                },
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::new("USD-OIS"),
                        tenor_years: 10.0,
                    },
                    shift: -0.0010,
                },
            ],
        );

        let pnl = taylor_pnl_for_scenario(
            &sensitivities,
            &MarketContext::new(),
            &steepener,
            &mut HashMap::default(),
        )
        .expect("steepener Taylor P&L should compute");

        // First-order only: (-40 * 10bp) + (-120 * -10bp) = -400 + 1200 = 800.
        // The convexity term must be omitted (non-parallel), so P&L == 800.
        assert!(
            (pnl - 800.0).abs() < 1e-9,
            "steepener Taylor P&L should be pure first-order DV01 (800), got {pnl}"
        );
        assert!(
            pnl.abs() > 1.0,
            "steepener Taylor P&L must not silently zero out, got {pnl}"
        );

        // Sanity: a genuine parallel shift DOES apply the convexity term.
        let parallel = MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::new("USD-OIS"),
                        tenor_years: 2.0,
                    },
                    shift: 0.0010,
                },
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::new("USD-OIS"),
                        tenor_years: 10.0,
                    },
                    shift: 0.0010,
                },
            ],
        );
        let parallel_pnl = taylor_pnl_for_scenario(
            &sensitivities,
            &MarketContext::new(),
            &parallel,
            &mut HashMap::default(),
        )
        .expect("parallel Taylor P&L should compute");
        // First-order: (-40 - 120) * 10bp = -1600.
        // Convexity: 0.5 * 5000 * 10^2 = 250000.
        assert!(
            (parallel_pnl - (-1600.0 + 250_000.0)).abs() < 1e-6,
            "parallel shift should include the convexity term, got {parallel_pnl}"
        );
    }

    /// BUG 2 regression: the Taylor path must convert scenario P&L to the
    /// reporting currency using the scenario-shifted FX rates, not the base
    /// market's.
    ///
    /// The Taylor path (`calculate_var_taylor_approximation` /
    /// `calculate_portfolio_var_taylor`) now builds `scenario.apply(base_market)`
    /// per scenario and feeds that scenario-shifted market into
    /// `convert_money_to_reporting`. This test verifies the load-bearing
    /// behavior directly: converting a fixed local P&L through a market that
    /// has had an FX shock applied yields the SCENARIO rate, not the base rate.
    #[test]
    fn test_taylor_var_converts_pnl_at_scenario_fx() -> Result<()> {
        let as_of = sample_as_of();

        let provider = finstack_core::money::fx::SimpleFxProvider::new();
        // Base FX: EUR/USD = 2.0.
        provider
            .set_quote(Currency::EUR, Currency::USD, 2.0)
            .expect("valid base rate");
        let fx = finstack_core::money::fx::FxMatrix::new(Arc::new(provider));
        let base_market = MarketContext::new().insert_fx(fx);

        // Scenario: +50% EUR/USD shock (2.0 -> 3.0).
        let scenario = MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::FxSpot {
                    base: Currency::EUR,
                    quote: Currency::USD,
                },
                shift: 0.50,
            }],
        );

        // A local P&L of 10 EUR, as produced by `taylor_pnl_for_scenario`.
        let local_pnl = Money::new(10.0, Currency::EUR);

        // Base-market conversion (the OLD, buggy behavior) would give 20 USD.
        let base_converted = convert_money_to_reporting(
            local_pnl,
            Currency::USD,
            &base_market,
            as_of,
            "Historical VaR",
        )?;
        assert!(
            (base_converted - 20.0).abs() < 1e-9,
            "base-market conversion should use the base rate (2.0), got {base_converted}"
        );

        // Scenario-market conversion (the FIXED behavior used by the Taylor
        // path) must use the shifted rate (3.0) -> 30 USD.
        let scenario_market = scenario.apply(&base_market)?;
        let scenario_converted = convert_money_to_reporting(
            local_pnl,
            Currency::USD,
            &scenario_market,
            as_of,
            "Historical VaR",
        )?;
        assert!(
            (scenario_converted - 30.0).abs() < 1e-9,
            "Taylor P&L must be converted at scenario FX (expected 30 USD), got {scenario_converted}"
        );
        assert!(
            (scenario_converted - base_converted).abs() > 1.0,
            "scenario FX conversion must differ from base FX conversion"
        );
        Ok(())
    }

    #[test]
    fn test_var_result_from_distribution() {
        // Create synthetic P&L distribution with known values.
        let pnls = vec![
            100.0,  // gain
            50.0,   // gain
            0.0,    // no change
            -25.0,  // small loss
            -50.0,  // medium loss
            -100.0, // large loss
            -150.0, // very large loss
            -200.0, // worst loss
        ];

        let result = VarResult::from_distribution(pnls, 0.95).expect("pnl distribution is finite");

        assert_eq!(result.num_scenarios, 8);

        // Sorted ascending: [-200, -150, -100, -50, -25, 0, 50, 100]; n = 8.
        // 95% VaR uses the R type-7 linear-interpolated quantile at p = 0.05:
        //   h = (n - 1) * p = 7 * 0.05 = 0.35  ->  lo = 0, frac = 0.35
        //   Q(p) = pnl[0] + 0.35 * (pnl[1] - pnl[0])
        //        = -200 + 0.35 * 50 = -182.5  ->  VaR = 182.5
        // (The previous ceil-index estimator reported the worst loss, 200.0.)
        assert!(
            (result.var - 182.5).abs() < 1e-9,
            "interpolated 95% VaR should be 182.5, got {}",
            result.var
        );

        // ES is the mean of Q over the tail [0, 0.05]; that interval lies
        // entirely within the first linear segment (knot at 1/7 ~= 0.1429),
        // so the integral is a single trapezoid:
        //   integral = p * (Q(0) + Q(p)) / 2 = 0.05 * (-200 + -182.5) / 2
        //            = 0.05 * -191.25 = -9.5625
        //   ES = integral / p = -9.5625 / 0.05 = -191.25  ->  191.25
        assert!(
            (result.expected_shortfall - 191.25).abs() < 1e-9,
            "interpolated 95% ES should be 191.25, got {}",
            result.expected_shortfall
        );

        // VaR and ES come from one quantile definition: ES averages Q over
        // [0, p] and VaR is its least-extreme endpoint Q(p), so ES >= VaR.
        assert!(result.expected_shortfall >= result.var);
    }

    #[test]
    fn test_var_result_rejects_invalid_confidence() {
        for confidence_level in [0.0, 1.0, f64::NAN, f64::INFINITY] {
            let err = VarResult::from_distribution(vec![-10.0, 5.0], confidence_level)
                .expect_err("invalid confidence should be rejected");
            assert!(
                err.to_string().contains("strictly between 0 and 1"),
                "unexpected error for confidence={confidence_level}: {err}"
            );
        }
    }

    #[test]
    fn test_var_result_all_gain_distribution_has_zero_loss() {
        let result = VarResult::from_distribution(vec![10.0, 20.0, 30.0], 0.95)
            .expect("valid all-gain distribution");

        assert_eq!(result.var, 0.0);
        assert_eq!(result.expected_shortfall, 0.0);
    }

    #[test]
    fn test_var_result_rejects_nan() {
        let pnls = vec![10.0, f64::NAN, -5.0];
        let err = VarResult::from_distribution(pnls, 0.95).expect_err("should reject NaNs");
        match err {
            finstack_core::Error::Validation(msg) => {
                assert!(
                    msg.contains("non-finite"),
                    "error message should mention non-finite values"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn test_var_calculation_simple_bond() -> Result<()> {
        let as_of = sample_as_of();

        let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
        let base_market = usd_ois_market(as_of)?;

        let history = history_from_rate_shifts(
            as_of,
            &[
                (date!(2023 - 12 - 31), 0.0050),
                (date!(2023 - 12 - 30), -0.0030),
                (date!(2023 - 12 - 29), 0.0010),
            ],
        );
        let config = VarConfig::var_95();

        // Calculate VaR
        let result = calculate_var(&[&bond], &base_market, &history, as_of, &config)?;

        // Verify results
        assert_eq!(result.num_scenarios, 3);
        assert!(result.var > 0.0, "VaR should be positive");
        assert!(
            result.expected_shortfall >= result.var,
            "ES should be >= VaR"
        );

        // P&L distribution should have 3 values
        assert_eq!(result.pnl_distribution.len(), 3);

        // Distribution should be sorted (worst first)
        for i in 1..result.pnl_distribution.len() {
            assert!(
                result.pnl_distribution[i] >= result.pnl_distribution[i - 1],
                "P&L distribution should be sorted"
            );
        }

        Ok(())
    }

    #[test]
    fn test_var_empty_history() -> Result<()> {
        let as_of = sample_as_of();
        let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
        let base_market = usd_ois_market(as_of)?;

        // Empty history
        let history = history_from_scenarios(as_of, 0, vec![]);
        let config = VarConfig::var_95();

        let result = calculate_var(&[&bond], &base_market, &history, as_of, &config)?;

        assert_eq!(result.num_scenarios, 0);
        assert_eq!(result.pnl_distribution.len(), 0);

        Ok(())
    }

    #[test]
    fn test_portfolio_var_with_diversification() -> Result<()> {
        let as_of = sample_as_of();

        let bond1 = standard_bond("BOND-5Y", as_of, date!(2029 - 01 - 01));
        let bond2 = standard_bond("BOND-2Y", as_of, date!(2026 - 01 - 01));

        let market = Arc::new(usd_ois_market(as_of)?);

        let history = history_from_rate_shifts(
            as_of,
            &[
                (date!(2023 - 12 - 31), 0.0100),
                (date!(2023 - 12 - 30), -0.0075),
                (date!(2023 - 12 - 29), 0.0025),
                (date!(2023 - 12 - 28), -0.0050),
            ],
        );
        let config = VarConfig::var_95();

        // Calculate individual VaRs
        let var1 = calculate_var(&[&bond1], market.as_ref(), &history, as_of, &config)?;
        let var2 = calculate_var(&[&bond2], market.as_ref(), &history, as_of, &config)?;
        let sum_individual_vars = var1.var.abs() + var2.var.abs();

        // Calculate portfolio VaR
        let instruments: Vec<&dyn Instrument> = vec![&bond1, &bond2];
        let portfolio_var =
            calculate_var_dyn(&instruments, market.as_ref(), &history, as_of, &config)?;

        // Verify portfolio VaR <= sum of individual VaRs
        // With only a few scenarios and both bonds having similar rate sensitivity,
        // we might not see diversification benefit in this simple test
        assert!(
            portfolio_var.var.abs() <= sum_individual_vars + 0.01, // Allow small numerical tolerance
            "Portfolio VaR ({}) should be <= sum of individual VaRs ({})",
            portfolio_var.var.abs(),
            sum_individual_vars
        );

        // Calculate diversification benefit (may be zero or small with limited scenarios)
        let diversification_benefit = sum_individual_vars - portfolio_var.var.abs();
        assert!(
            diversification_benefit >= -0.01, // Allow small numerical t olerance
            "Diversification benefit should be non-negative, got {}",
            diversification_benefit
        );

        println!("Individual VaR 1: ${:.2}", var1.var);
        println!("Individual VaR 2: ${:.2}", var2.var);
        println!("Sum of individual VaRs: ${:.2}", sum_individual_vars);
        println!("Portfolio VaR: ${:.2}", portfolio_var.var.abs());
        println!("Diversification benefit: ${:.2}", diversification_benefit);

        Ok(())
    }
}
