//! Historical VaR calculation engine.
//!
//! Implements Historical Value-at-Risk using historical simulation methodology.
//! Supports both full revaluation and Taylor approximation (Greeks-based) approaches.
//!
//! # References
//!
//! - VaR and Expected Shortfall interpretation: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`

use crate::instruments::Instrument;
use crate::metrics::risk::MarketHistory;
use crate::pricer::PricingDispatch;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::quantile_sorted;
use finstack_quant_core::math::summation::{neumaier_sum, NeumaierAccumulator};
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use std::sync::Arc;

/// VaR calculation method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// Computes first and second directional derivatives by selected-model
    /// repricing along each exact historical shock. Curve interpolation support,
    /// quote calibration, surface nodes, and reporting-currency FX all remain
    /// part of the direction. It may be inaccurate for large nonlinear shocks
    /// and need not be faster than full revaluation. At zero volatility a
    /// second-order forward stencil replaces the central stencil.
    TaylorApproximation,
}

/// Configuration for VaR calculation.
///
/// Controls statistical properties such as confidence level and pricing method.
/// The historical window/observation count is derived from [`MarketHistory`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct VarConfig {
    /// Confidence level (e.g., 0.95 for 95% VaR, 0.99 for 99% VaR)
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_open_unit_interval_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_open_unit_interval_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::OpenUnitIntervalF64Wire")
    )]
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
        Err(finstack_quant_core::Error::Validation(format!(
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VarResult {
    /// Value-at-Risk at specified confidence level.
    ///
    /// Loss convention (workspace-wide): follows the P&L sign, so losses are
    /// reported as **negative** numbers (zero for all-gain distributions),
    /// matching `analytics::value_at_risk` and the portfolio risk engines.
    /// The signed P&L at the `(1 - α)` quantile of the distribution, computed
    /// by linear interpolation between bracketing order statistics.
    pub var: f64,

    /// Expected Shortfall (CVaR) at specified confidence level.
    ///
    /// Same loss convention as [`Self::var`]. The conditional tail
    /// expectation: the average signed P&L over the worst `(1 - α)` fraction
    /// of the distribution, computed as the mean of the interpolated quantile
    /// function over `[0, 1 - α]`. By construction
    /// `expected_shortfall <= var` (ES lies at or beyond VaR in the loss tail).
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
            return Err(finstack_quant_core::Error::Validation(
                "VaR P&L distribution contains non-finite values (NaN or inf)".to_string(),
            ));
        }

        // Sort P&L distribution (ascending = worst losses first)
        pnl_distribution.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let num_scenarios = pnl_distribution.len();

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
        // Loss convention (workspace-wide): VaR/ES follow the P&L sign, so
        // losses are negative. Clamp to zero only when the tail quantile is
        // actually a gain (all-profit distributions).
        let p = 1.0 - confidence_level;
        let var = quantile_sorted(&pnl_distribution, p.clamp(0.0, 1.0)).min(0.0);
        let expected_shortfall = tail_quantile_mean(&pnl_distribution, p).min(0.0);

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

/// Mean of the type-7 quantile function over the tail `[0, p]` — the genuine
/// Expected Shortfall of the empirical distribution at tail probability `p`.
///
/// `ES = (1 / p) * integral over [0, p] of Q(u) du`, where `Q` is the
/// piecewise-linear [`quantile_sorted`] function. The integral is evaluated
/// exactly by summing the trapezoidal area of each linear piece of `Q` that
/// the interval `[0, p]` crosses (knots sit at `u = i / (n - 1)`).
///
/// This integrates the SAME quantile function used by [`quantile_sorted`], so
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

/// Calculate historical portfolio VaR and expected shortfall.
///
/// Every supplied instrument is revalued under every scenario in
/// [`MarketHistory`] and the P&Ls are aggregated into one loss distribution.
/// An empty instrument list returns zero VaR/ES. A non-empty portfolio
/// requires at least one scenario; missing history returns an error.
///
/// # Arguments
///
/// * `instruments` - Instrument references to reprice under every scenario.
/// * `base_market` - Unshocked market context used for baseline valuations and
///   as the source market that each scenario perturbs.
/// * `history` - Ordered market scenarios or return history for the loss
///   distribution.
/// * `as_of` - Valuation date applied consistently to baseline and scenarios.
/// * `config` - VaR method, confidence level, horizon, and scenario policy
///   controlling the returned risk measures.
/// * `dispatch` - Pricing path reused for baseline, bumped, and scenario
///   valuations. [`PricingDispatch::InstrumentDefault`] uses each instrument's
///   canonical default; [`PricingDispatch::Registered`] preserves one explicit
///   model-registry pair throughout the calculation.
/// * `provider` - Quote recalibration service forwarded to historical credit
///   shocks and nested quote sensitivities; required when those operations run.
///
/// # Returns
///
/// VaR result including VaR, ES, and the full P&L distribution.
pub fn calculate_var(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    dispatch: PricingDispatch,
    provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
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
    if history.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "Historical VaR requires at least one market scenario for a non-empty portfolio"
                .to_string(),
        ));
    }

    match config.method {
        VarMethod::FullRevaluation => calculate_var_full_revaluation(
            instruments,
            base_market,
            history,
            as_of,
            config,
            &dispatch,
            provider.clone(),
        ),
        VarMethod::TaylorApproximation => calculate_var_taylor(
            instruments,
            base_market,
            history,
            as_of,
            config,
            &dispatch,
            provider.clone(),
        ),
    }
}

fn reprice_with_dispatch(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    dispatch: &PricingDispatch,
) -> Result<Money> {
    dispatch.price_money(instrument, market, as_of, Default::default())
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
    provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
) -> Result<Vec<f64>>
where
    F: Fn(&MarketContext) -> Result<f64> + Send + Sync,
{
    let map_scenario = |scenario: &super::market_history::MarketScenario| {
        let scenario_market = scenario.apply(base_market, provider.as_deref())?;
        scenario_pnl(&scenario_market)
    };

    // Several failing scenarios report the first failing scenario in history
    // order on every target and thread count.
    finstack_quant_core::parallel::try_map_ordered(&history.scenarios, map_scenario)
}

/// Calculate VaR using full revaluation method.
fn calculate_var_full_revaluation(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    dispatch: &PricingDispatch,
    provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
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
        .map(|inst| reprice_with_dispatch(*inst, base_market, as_of, dispatch))
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
                reprice_with_dispatch(*inst, scenario_market, as_of, dispatch)?,
                reporting_currency,
                scenario_market,
                as_of,
                "Historical VaR",
            )?;
            acc.add(scenario_amount - base_amount);
        }
        Ok(acc.total())
    };

    let pnls = aggregate_scenario_pnls_par(history, base_market, scenario_pnl, provider.clone())?;

    VarResult::from_distribution(pnls, config.confidence_level)
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
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context_name} requires an explicit reporting currency for mixed-currency portfolios"
                )))
            }
        }
    }

    observed.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
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
        finstack_quant_core::Error::Validation(format!(
            "{context_name} requires FX data to convert {} into reporting currency {}",
            value.currency(),
            reporting_currency
        ))
    })?;
    let rate = fx
        .rate(FxQuery::new(
            value.currency(),
            reporting_currency,
            on,
            FxConversionPolicy::CashflowDate,
        ))?
        .rate;
    Ok(value.amount() * rate)
}

/// Keep only dependencies of this instrument. FX conversion into the reporting
/// currency is itself a dependency, even for a fixed local-currency cash claim.
pub(crate) fn relevant_taylor_scenario(
    instrument: &dyn Instrument,
    currency: Currency,
    reporting_currency: Currency,
    scenario: &super::MarketScenario,
) -> Result<super::MarketScenario> {
    use super::RiskFactorType;
    let deps = instrument.market_dependencies()?;
    let shifts: Vec<super::RiskFactorShift> = scenario
        .shifts
        .iter()
        .filter(|shift| match &shift.factor {
            RiskFactorType::DiscountRate { curve_id, .. } => {
                deps.curves.discount_curves.contains(curve_id)
            }
            RiskFactorType::ForwardRate { curve_id, .. } => {
                deps.curves.forward_curves.contains(curve_id)
            }
            RiskFactorType::CreditSpread { curve_id, .. } => {
                deps.curves.credit_curves.contains(curve_id)
            }
            RiskFactorType::EquitySpot { ticker } => deps.market_scalar_ids.contains(ticker),
            RiskFactorType::ImpliedVol { vol_surface_id, .. } => deps
                .volatility_dependencies
                .iter()
                .any(|d| &d.vol_surface_id == vol_surface_id),
            RiskFactorType::FxSpot { base, quote } => {
                currency != reporting_currency
                    || deps.fx_pairs.iter().any(|p| {
                        (p.base == *base && p.quote == *quote)
                            || (p.base == *quote && p.quote == *base)
                    })
            }
        })
        .cloned()
        .collect();
    // Replay aggregates volatility shifts at the same node before checking
    // its domain. Preserve that net direction when selecting feasible steps.
    let mut net: Vec<super::RiskFactorShift> = Vec::with_capacity(shifts.len());
    for shift in shifts {
        if matches!(shift.factor, RiskFactorType::ImpliedVol { .. }) {
            if let Some(existing) = net.iter_mut().find(|value| value.factor == shift.factor) {
                existing.shift += shift.shift;
                continue;
            }
        }
        net.push(shift);
    }
    Ok(super::MarketScenario::new(scenario.date, net))
}

/// A scalar parameter multiplies the exact dated scenario, preserving every
/// curve triangle, quote-replay binding, and volatility node. Differentiating
/// along this direction automatically includes its Hessian cross terms.
fn directional_taylor_pnl(
    scenario: &super::MarketScenario,
    market: &MarketContext,
    base: f64,
    mut price: impl FnMut(&super::MarketScenario) -> Result<f64>,
) -> Result<f64> {
    use super::RiskFactorType;
    let mut normalized_size: f64 = 1.0;
    let mut symmetric_limit = f64::INFINITY;
    for shift in &scenario.shifts {
        if !shift.shift.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "Taylor scenario shifts must be finite".into(),
            ));
        }
        if shift.shift == 0.0 {
            continue;
        }
        let unit = match &shift.factor {
            RiskFactorType::DiscountRate { .. }
            | RiskFactorType::ForwardRate { .. }
            | RiskFactorType::CreditSpread { .. } => 1e-4,
            _ => 0.01,
        };
        normalized_size = normalized_size.max(shift.shift.abs() / unit);
        match &shift.factor {
            RiskFactorType::ImpliedVol {
                vol_surface_id,
                expiry_years,
                strike,
            } => {
                let surface = market.get_surface(vol_surface_id)?;
                let row = surface
                    .expiries()
                    .iter()
                    .position(|v| v.to_bits() == expiry_years.to_bits());
                let col = surface
                    .strikes()
                    .iter()
                    .position(|v| v.to_bits() == strike.to_bits());
                let (Some(row), Some(col)) = (row, col) else {
                    return Err(finstack_quant_core::Error::Validation(format!("Taylor volatility point ({expiry_years}, {strike}) must exactly match a grid node on '{vol_surface_id}'")));
                };
                let vol = surface.vols()[row * surface.strikes().len() + col];
                if vol + shift.shift < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "Taylor volatility scenario would be negative".into(),
                    ));
                }
                symmetric_limit = symmetric_limit.min(vol / shift.shift.abs());
            }
            RiskFactorType::EquitySpot { .. } | RiskFactorType::FxSpot { .. } => {
                if shift.shift <= -1.0 {
                    return Err(finstack_quant_core::Error::Validation(
                        "Taylor relative-price scenario must leave a positive price".into(),
                    ));
                }
                symmetric_limit = symmetric_limit.min(1.0 / shift.shift.abs());
            }
            _ => {}
        }
    }
    let scaled = |scale: f64| {
        super::MarketScenario::new(
            scenario.date,
            scenario
                .shifts
                .iter()
                .map(|shift| super::RiskFactorShift {
                    factor: shift.factor.clone(),
                    shift: shift.shift * scale,
                })
                .collect(),
        )
    };
    let mut h = 1.0 / normalized_size;
    if symmetric_limit > 0.0 {
        h = h.min(symmetric_limit * 0.5);
        let up = price(&scaled(h))?;
        let down = price(&scaled(-h))?;
        Ok((up - down) / (2.0 * h) + 0.5 * (up - 2.0 * base + down) / (h * h))
    } else {
        // A zero-volatility node admits only positive volatility shocks.
        // Four points give second-order first AND second derivatives.
        h /= 3.0;
        let p1 = price(&scaled(h))?;
        let p2 = price(&scaled(2.0 * h))?;
        let p3 = price(&scaled(3.0 * h))?;
        let first = (-11.0 * base + 18.0 * p1 - 9.0 * p2 + 2.0 * p3) / (6.0 * h);
        let second = (2.0 * base - 5.0 * p1 + 4.0 * p2 - p3) / (h * h);
        Ok(first + 0.5 * second)
    }
}

fn calculate_var_taylor(
    instruments: &[&dyn Instrument],
    base_market: &MarketContext,
    history: &MarketHistory,
    as_of: Date,
    config: &VarConfig,
    dispatch: &PricingDispatch,
    provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
) -> Result<VarResult> {
    let base_values: Vec<_> = instruments
        .iter()
        .map(|instrument| reprice_with_dispatch(*instrument, base_market, as_of, dispatch))
        .collect::<Result<_>>()?;
    let reporting_currency = resolve_reporting_currency_from_values(
        &base_values,
        config.reporting_currency,
        "Historical VaR",
    )?;
    let mut pnls = Vec::with_capacity(history.len());
    for scenario in history.iter() {
        let mut total = NeumaierAccumulator::new();
        for (instrument, base_money) in instruments.iter().zip(&base_values) {
            let scenario = relevant_taylor_scenario(
                *instrument,
                base_money.currency(),
                reporting_currency,
                scenario,
            )?;
            if scenario.shifts.iter().all(|s| s.shift == 0.0) {
                continue;
            }
            let currency = base_money.currency();
            let reporting_value = |market: &MarketContext| {
                let raw = dispatch.price_raw(*instrument, market, as_of)?;
                let fx = convert_money_to_reporting(
                    Money::new(1.0, currency)?,
                    reporting_currency,
                    market,
                    as_of,
                    "Historical VaR",
                )?;
                Ok(raw * fx)
            };
            let base = reporting_value(base_market)?;
            total.add(directional_taylor_pnl(
                &scenario,
                base_market,
                base,
                |scaled| {
                    let market = scaled.apply(base_market, provider.as_deref())?;
                    reporting_value(&market)
                },
            )?);
        }
        pnls.push(total.total());
    }
    VarResult::from_distribution(pnls, config.confidence_level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::Instrument;
    use crate::metrics::risk::{MarketScenario, RiskFactorShift, RiskFactorType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::CurveId;
    use std::sync::Arc;
    use time::macros::date;

    fn sample_as_of() -> finstack_quant_core::dates::Date {
        date!(2024 - 01 - 01)
    }

    fn usd_ois_market(as_of: finstack_quant_core::dates::Date) -> Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
            .build()?;
        Ok(MarketContext::new().insert(curve))
    }

    fn history_from_scenarios(
        as_of: finstack_quant_core::dates::Date,
        window_days: u32,
        scenarios: Vec<MarketScenario>,
    ) -> MarketHistory {
        MarketHistory::new(as_of, window_days, scenarios)
    }

    #[test]
    fn var_result_round_trips_through_strict_json() -> Result<()> {
        let result = VarResult::from_distribution(vec![-3.0, 1.0, -1.0, 2.0], 0.95)?;
        let json = serde_json::to_string(&result)
            .map_err(|e| finstack_quant_core::Error::Internal(e.to_string()))?;
        let back: VarResult = serde_json::from_str(&json)
            .map_err(|e| finstack_quant_core::Error::Internal(e.to_string()))?;
        assert_eq!(back, result);
        assert!(serde_json::from_str::<VarResult>(&json.replace("\"var\"", "\"vaR\"")).is_err());
        Ok(())
    }

    #[test]
    fn pricing_dispatch_from_model_maps_default_and_explicit_keys() -> Result<()> {
        let registry = || crate::pricer::Family::unregistered().shared_pricers();
        assert!(PricingDispatch::from_model("default", registry())?
            .model()
            .is_none());
        assert_eq!(
            PricingDispatch::from_model("discounting", registry())?.model(),
            Some(crate::pricer::ModelKey::Discounting)
        );
        assert!(PricingDispatch::from_model("no_such_model", registry()).is_err());
        Ok(())
    }

    #[test]
    fn several_invalid_scenarios_report_the_first_scenario_in_history_order() {
        // 64 scenarios, each with an invalid credit tenor on its own curve. The
        // native parallel map must name CRV-000 on every run, as the serial
        // (wasm32) loop does.
        let as_of = sample_as_of();
        let market = usd_ois_market(as_of).expect("market");
        let scenarios = (0..64)
            .map(|i| {
                MarketScenario::new(
                    as_of,
                    vec![RiskFactorShift {
                        factor: RiskFactorType::CreditSpread {
                            curve_id: CurveId::from(format!("CRV-{i:03}")),
                            tenor_years: -1.0,
                        },
                        shift: 0.001,
                    }],
                )
            })
            .collect();
        let history = history_from_scenarios(as_of, 64, scenarios);
        for _ in 0..20 {
            let error = aggregate_scenario_pnls_par(&history, &market, |_| Ok(0.0), None)
                .expect_err("every scenario is invalid")
                .to_string();
            assert!(
                error.contains("'CRV-000'"),
                "expected the first scenario, got {error}"
            );
        }
    }

    #[test]
    fn var_config_wire_contract_is_strict_and_bounded() {
        for confidence_level in [0.0, 1.0, -0.01, 1.01] {
            let value = serde_json::json!({ "confidence_level": confidence_level });
            assert!(
                serde_json::from_value::<VarConfig>(value).is_err(),
                "confidence level {confidence_level} must be rejected"
            );
        }
        assert!(serde_json::from_value::<VarConfig>(serde_json::json!({
            "confidence_level": 0.95,
            "unexpected": true
        }))
        .is_err());

        let schema =
            serde_json::to_value(schemars::schema_for!(VarConfig)).expect("VaR config schema");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["$defs"]["OpenUnitIntervalF64Wire"]["exclusiveMinimum"],
            0.0
        );
        assert_eq!(
            schema["$defs"]["OpenUnitIntervalF64Wire"]["exclusiveMaximum"],
            1.0
        );
    }

    #[derive(Clone, Debug)]
    struct CurrencyScalarInstrument {
        id: String,
        price_id: String,
        currency: Currency,
        discount_curves: Vec<CurveId>,
        attributes: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        CurrencyScalarInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl CurrencyScalarInstrument {
        fn new(id: &str, price_id: &str, currency: Currency) -> Self {
            Self {
                id: id.to_string(),
                price_id: price_id.to_string(),
                currency,
                discount_curves: Vec::new(),
                attributes: Attributes::new(),
            }
        }

        fn with_discount_curve(mut self, curve_id: &str) -> Self {
            self.discount_curves.push(CurveId::from(curve_id));
            self
        }
    }

    impl Instrument for CurrencyScalarInstrument {
        /// Test mock: declares only the discount curves it was given.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
            let mut deps = crate::instruments::MarketDependencies::new();
            for curve_id in &self.discount_curves {
                deps.add_discount_curve(curve_id.clone());
            }
            Ok(deps)
        }

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
                MarketScalar::Unitless(v) => {
                    Ok(Money::new(*v, self.currency).expect("valid money fixture"))
                }
            }
        }
    }

    /// Workspace sign convention: VaR and ES follow the P&L sign, so losses
    /// are reported as **negative** numbers — matching
    /// `analytics::value_at_risk` and the portfolio factor/position risk
    /// engines. ES lies at or beyond VaR in the loss tail.
    #[test]
    fn var_result_reports_losses_as_negative() -> Result<()> {
        // 100 P&Ls: -50..49. The 1% tail is the worst loss region.
        let pnls: Vec<f64> = (0..100).map(|s| f64::from(s) - 50.0).collect();
        let result = VarResult::from_distribution(pnls, 0.99)?;

        assert!(
            result.var < 0.0,
            "VaR must be negative (losses-negative convention), got {}",
            result.var
        );
        assert!(
            result.expected_shortfall <= result.var,
            "ES ({}) must lie at or beyond VaR ({}) in the loss tail",
            result.expected_shortfall,
            result.var
        );
        // Type-7 quantile at p = 0.01 over -50..49: h = 0.99 -> -49.01.
        assert!((result.var - (-49.01)).abs() < 1e-9);
        Ok(())
    }

    /// The Taylor path differentiates only along the instrument's own
    /// dependencies: a shift on a curve the instrument never reads is dropped.
    #[test]
    fn relevant_taylor_scenario_keeps_only_instrument_dependencies() -> Result<()> {
        let as_of = sample_as_of();
        let instrument = CurrencyScalarInstrument::new("USD", "USD-PX", Currency::USD)
            .with_discount_curve("USD-OIS");
        let scenario = MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: "USD-OIS".into(),
                        tenor_years: 1.4,
                    },
                    shift: 0.0005,
                },
                RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: "EUR-OIS".into(),
                        tenor_years: 2.3,
                    },
                    shift: -0.0004,
                },
            ],
        );
        let relevant =
            relevant_taylor_scenario(&instrument, Currency::USD, Currency::USD, &scenario)?;
        assert_eq!(relevant.shifts.len(), 1);
        assert!(matches!(
            &relevant.shifts[0].factor,
            RiskFactorType::DiscountRate { curve_id, .. } if curve_id == "USD-OIS"
        ));
        Ok(())
    }

    /// An all-gains distribution clamps VaR/ES to zero rather than
    /// reporting a positive "risk" number.
    #[test]
    fn var_result_clamps_all_gain_distribution_to_zero() -> Result<()> {
        let pnls: Vec<f64> = (1..=100).map(f64::from).collect();
        let result = VarResult::from_distribution(pnls, 0.95)?;
        assert_eq!(result.var, 0.0);
        assert_eq!(result.expected_shortfall, 0.0);
        Ok(())
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
                MarketScalar::Price(Money::from((100_i64, Currency::USD))),
            )
            .insert_price(
                "EUR-INST",
                MarketScalar::Price(Money::from((100_i64, Currency::EUR))),
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

        let err = calculate_var(&[&usd as &dyn Instrument, &eur as &dyn Instrument], &base_market, &history, as_of, &VarConfig::var_95(), PricingDispatch::InstrumentDefault, None)
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

        let provider = finstack_quant_core::money::fx::SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 2.0)
            .expect("valid rate");
        let fx = finstack_quant_core::money::fx::FxMatrix::new(Arc::new(provider));

        let base_market = MarketContext::new()
            .insert_price(
                "USD-INST",
                MarketScalar::Price(Money::from((100_i64, Currency::USD))),
            )
            .insert_price(
                "EUR-INST",
                MarketScalar::Price(Money::from((100_i64, Currency::EUR))),
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

        let result = calculate_var(
            &[&usd as &dyn Instrument, &eur as &dyn Instrument],
            &base_market,
            &history,
            as_of,
            &VarConfig::var_95().with_reporting_currency(Currency::USD),
            PricingDispatch::InstrumentDefault,
            None,
        )?;
        assert!((result.pnl_distribution[0] - 30.0).abs() < 1e-12);
        Ok(())
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

        let provider = finstack_quant_core::money::fx::SimpleFxProvider::new();
        // Base FX: EUR/USD = 2.0.
        provider
            .set_quote(Currency::EUR, Currency::USD, 2.0)
            .expect("valid base rate");
        let fx = finstack_quant_core::money::fx::FxMatrix::new(Arc::new(provider));
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
        let local_pnl = Money::from((10_i64, Currency::EUR));

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
        let scenario_market = scenario.apply(&base_market, None)?;
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
        //        = -200 + 0.35 * 50 = -182.5  ->  VaR = -182.5
        // (losses-negative convention: VaR keeps the P&L sign).
        assert!(
            (result.var - (-182.5)).abs() < 1e-9,
            "interpolated 95% VaR should be 182.5, got {}",
            result.var
        );

        // ES is the mean of Q over the tail [0, 0.05]; that interval lies
        // entirely within the first linear segment (knot at 1/7 ~= 0.1429),
        // so the integral is a single trapezoid:
        //   integral = p * (Q(0) + Q(p)) / 2 = 0.05 * (-200 + -182.5) / 2
        //            = 0.05 * -191.25 = -9.5625
        //   ES = integral / p = -9.5625 / 0.05 = -191.25
        assert!(
            (result.expected_shortfall - (-191.25)).abs() < 1e-9,
            "interpolated 95% ES should be -191.25, got {}",
            result.expected_shortfall
        );

        // VaR and ES come from one quantile definition: ES averages Q over
        // [0, p] and VaR is its least-extreme endpoint Q(p), so as signed
        // P&L numbers ES <= VaR (ES lies deeper in the loss tail).
        assert!(result.expected_shortfall <= result.var);
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
            finstack_quant_core::Error::Validation(msg) => {
                assert!(
                    msg.contains("non-finite"),
                    "error message should mention non-finite values"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn empty_portfolio_has_explicit_zero_var_state() -> Result<()> {
        let as_of = sample_as_of();
        let base_market = usd_ois_market(as_of)?;
        let history = history_from_scenarios(as_of, 0, vec![]);
        let result = calculate_var(
            &[],
            &base_market,
            &history,
            as_of,
            &VarConfig::var_95(),
            PricingDispatch::InstrumentDefault,
            None,
        )?;
        assert_eq!(result.var, 0.0);
        assert_eq!(result.expected_shortfall, 0.0);
        assert_eq!(result.num_scenarios, 0);
        Ok(())
    }

    #[test]
    fn directional_taylor_has_zero_curvature_for_fixed_discount_linear_forward() -> Result<()> {
        let as_of = sample_as_of();
        let scenario = MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::ForwardRate {
                    curve_id: "F".into(),
                    tenor_years: 1.4,
                },
                shift: 0.001,
            }],
        );
        // A simple term-index coupon with held discount has PV = N*alpha*D*L.
        // No discount shock is supplied, so its exact directional Hessian is zero.
        let coefficient = 1_000_000.0 * 0.25 * 0.97;
        let value =
            directional_taylor_pnl(&scenario, &MarketContext::new(), coefficient * 0.04, |s| {
                Ok(coefficient * (0.04 + s.shifts[0].shift))
            })?;
        assert!((value - coefficient * 0.001).abs() < 1e-8);
        Ok(())
    }
}
