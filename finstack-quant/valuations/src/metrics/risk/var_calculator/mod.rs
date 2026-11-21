//! Historical VaR calculation engine.
//!
//! Implements Historical Value-at-Risk using historical simulation methodology.
//! Supports both full revaluation and Taylor approximation (Greeks-based) approaches.
//!
//! # References
//!
//! - VaR and Expected Shortfall interpretation: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`

use std::sync::Arc;

use super::{MarketHistory, apply_market_scenario, same_grid_coordinate};
use crate::instruments::Instrument;
use crate::pricer::PricingDispatch;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::stats::quantile_sorted;
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxQuery};

pub use finstack_quant_valuations_core::metrics::risk::{VarConfig, VarMethod};

#[cfg(test)]
mod tests;

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
        VarConfig::new(confidence_level).validate()?;

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

        let p = 1.0 - confidence_level;
        let var = quantile_sorted(&pnl_distribution, p.clamp(0.0, 1.0))?
            .unwrap_or(0.0)
            .min(0.0);
        let expected_shortfall = tail_quantile_mean(&pnl_distribution, p)?.min(0.0);

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

/// Mean of the type-7 quantile function over the tail `[0, p]`.
fn tail_quantile_mean(sorted: &[f64], p: f64) -> Result<f64> {
    let n = sorted.len();
    if n == 0 {
        return Ok(0.0);
    }
    if n == 1 {
        return Ok(sorted[0]);
    }
    let n_minus_1 = (n - 1) as f64;
    let p = p.clamp(0.0, 1.0);
    let h_p = n_minus_1 * p;

    let segment_area = (0..n - 1).map_while(|j| {
        let seg_start = j as f64;
        if seg_start >= h_p {
            return None;
        }
        let a_h = seg_start;
        let b_h = h_p.min(seg_start + 1.0);
        let q_a = sorted[j] + (a_h - seg_start) * (sorted[j + 1] - sorted[j]);
        let q_b = sorted[j] + (b_h - seg_start) * (sorted[j + 1] - sorted[j]);
        let width = (b_h - a_h) / n_minus_1;
        Some(width * 0.5 * (q_a + q_b))
    });
    let integral = neumaier_sum(segment_area)?;

    Ok(integral / p)
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
            provider,
        ),
        VarMethod::TaylorApproximation => calculate_var_taylor(
            instruments,
            base_market,
            history,
            as_of,
            config,
            &dispatch,
            provider,
        ),
        other => Err(finstack_quant_core::Error::Validation(format!(
            "unsupported VaR calculation method: {other:?}"
        ))),
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

fn aggregate_scenario_pnls_par<F>(
    history: &MarketHistory,
    base_market: &MarketContext,
    as_of: Date,
    scenario_pnl: F,
    provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
) -> Result<Vec<f64>>
where
    F: Fn(&MarketContext) -> Result<f64> + Send + Sync,
{
    let map_scenario = |scenario: &super::MarketScenario| {
        let scenario = scenario.clone().at_date(as_of);
        let scenario_market = apply_market_scenario(&scenario, base_market, provider.as_deref())?;
        scenario_pnl(&scenario_market)
    };

    finstack_quant_core::parallel::try_map_ordered(&history.scenarios, map_scenario)
}

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

    let base_values_money: Vec<Money> = instruments
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
            convert_amount_to_reporting(
                value.amount(),
                value.currency(),
                reporting_currency,
                base_market,
                as_of,
                "Historical VaR",
            )
        })
        .collect::<Result<_>>()?;

    let scenario_pnl = move |scenario_market: &MarketContext| {
        let mut acc = NeumaierAccumulator::new();
        for (inst, base_amount) in instruments.iter().zip(base_values.iter()) {
            let priced = reprice_with_dispatch(*inst, scenario_market, as_of, dispatch)?;
            let scenario_amount = convert_amount_to_reporting(
                priced.amount(),
                priced.currency(),
                reporting_currency,
                scenario_market,
                as_of,
                "Historical VaR",
            )?;
            acc.add(scenario_amount - base_amount)?;
        }
        acc.get_total()
    };

    let pnls = aggregate_scenario_pnls_par(history, base_market, as_of, scenario_pnl, provider)?;

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
                )));
            }
        }
    }

    observed.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{context_name} requires at least one instrument to infer reporting currency"
        ))
    })
}

fn convert_amount_to_reporting(
    amount: f64,
    from_currency: Currency,
    reporting_currency: Currency,
    market: &MarketContext,
    on: Date,
    context_name: &str,
) -> Result<f64> {
    if from_currency == reporting_currency {
        return Ok(amount);
    }

    let fx = market.fx().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{context_name} requires FX data to convert {from_currency} into reporting currency {reporting_currency}"
        ))
    })?;
    let rate = fx
        .rate(FxQuery::new(
            from_currency,
            reporting_currency,
            on,
            FxConversionPolicy::CashflowDate,
        ))?
        .rate;
    Ok(amount * rate)
}

/// Keep only dependencies of this instrument. FX conversion into the reporting
/// currency is itself a dependency, even for a fixed local-currency cash claim.
fn relevant_taylor_scenario(
    instrument: &dyn Instrument,
    currency: Currency,
    reporting_currency: Currency,
    scenario: &super::MarketScenario,
    market: &MarketContext,
    as_of: Date,
) -> Result<super::MarketScenario> {
    use super::RiskFactorType;
    let deps = instrument.market_dependencies_at(market, as_of)?;
    let mut fx_directions = Vec::new();
    if scenario
        .shifts
        .iter()
        .any(|shift| matches!(shift.factor, RiskFactorType::FxSpot { .. }))
    {
        let queries = deps
            .fx_pairs
            .iter()
            .map(|pair| (pair.base, pair.quote))
            .chain((currency != reporting_currency).then_some((currency, reporting_currency)));
        for (base, quote) in queries.filter(|(base, quote)| base != quote) {
            fx_directions.push((base, quote));
            let fx = market.fx_required()?;
            let rate = fx.rate(FxQuery::new(
                base,
                quote,
                as_of,
                FxConversionPolicy::CashflowDate,
            ))?;
            if rate.triangulated
                && let Some(pivot) = fx.config().pivot_currency
            {
                fx_directions.extend([(base, pivot), (pivot, quote)]);
            }
        }
    }
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
            RiskFactorType::FxSpot { base, quote } => fx_directions
                .iter()
                .any(|&(from, to)| (from, to) == (*base, *quote) || (from, to) == (*quote, *base)),
        })
        .cloned()
        .collect();
    let mut net: Vec<super::RiskFactorShift> = Vec::with_capacity(shifts.len());
    for shift in shifts {
        if matches!(shift.factor, RiskFactorType::ImpliedVol { .. })
            && let Some(existing) = net.iter_mut().find(|value| value.factor == shift.factor)
        {
            existing.shift += shift.shift;
            continue;
        }
        net.push(shift);
    }
    Ok(super::MarketScenario {
        shifts: net,
        ..scenario.clone()
    })
}

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
                    .position(|v| same_grid_coordinate(*v, *expiry_years));
                let col = surface
                    .secondary_values()
                    .iter()
                    .position(|v| same_grid_coordinate(*v, *strike));
                let (Some(row), Some(col)) = (row, col) else {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Taylor volatility point ({expiry_years}, {strike}) must exactly match a grid node on '{vol_surface_id}'"
                    )));
                };
                let vol = surface.vols()[row * surface.secondary_values().len() + col];
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
    let scaled = |scale: f64| super::MarketScenario {
        shifts: scenario
            .shifts
            .iter()
            .map(|shift| super::RiskFactorShift {
                factor: shift.factor.clone(),
                shift: shift.shift * scale,
            })
            .collect(),
        ..scenario.clone()
    };
    let mut h = 1.0 / normalized_size;
    if symmetric_limit > 0.0 {
        h = h.min(symmetric_limit * 0.5);
        let up = price(&scaled(h))?;
        let down = price(&scaled(-h))?;
        Ok((up - down) / (2.0 * h) + 0.5 * (up - 2.0 * base + down) / (h * h))
    } else {
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
                base_market,
                as_of,
            )?
            .at_date(as_of);
            if scenario.shifts.iter().all(|s| s.shift == 0.0) {
                continue;
            }
            let currency = base_money.currency();
            let reporting_value = |market: &MarketContext| {
                let raw = dispatch.price_raw(*instrument, market, as_of)?;
                convert_amount_to_reporting(
                    raw,
                    currency,
                    reporting_currency,
                    market,
                    as_of,
                    "Historical VaR",
                )
            };
            let base = reporting_value(base_market)?;
            total.add(directional_taylor_pnl(
                &scenario,
                base_market,
                base,
                |scaled| {
                    let market = apply_market_scenario(scaled, base_market, provider.as_deref())?;
                    reporting_value(&market)
                },
            )?)?;
        }
        pnls.push(total.get_total()?);
    }
    VarResult::from_distribution(pnls, config.confidence_level)
}
