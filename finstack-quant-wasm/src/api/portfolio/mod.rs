//! WASM bindings for the `finstack-quant-portfolio` crate.
//!
//! Exposes portfolio spec parsing, validation, and result extraction
//! via JSON round-trip functions for JavaScript/TypeScript consumption.
//!
//! # Stability tiers
//!
//! The exports below fall into three stability tiers. Treat the tier as a
//! contract about how disruptive future changes are likely to be.
//!
//! **Stable** — golden-tested, signatures preserved across releases:
//! - `Portfolio` (typed handle: `fromSpec`, `toSpecJson`, `id`, `asOf`,
//!   `baseCcy`, `numPositions`)
//! - `parsePortfolioSpec`, `buildPortfolioFromSpec`
//! - `valuePortfolio`, `valuePortfolioBuilt`,
//!   `aggregateFullCashflows`, `aggregateFullCashflowsBuilt`,
//!   `applyScenarioAndRevalue`, `applyScenarioAndRevalueBuilt`
//! - `aggregateMetrics`, `portfolioResultTotalValue`,
//!   `portfolioResultGetMetric`
//! - `replayPortfolio`
//!
//! **Stable, JSON-shape may evolve** — function names stable, but the
//! returned / accepted JSON payload structure may grow additive
//! (non-breaking) fields between releases:
//! - `optimizePortfolio`
//!   (`PortfolioOptimizationSpec` / `PortfolioOptimizationResult` JSON)
//! - `parametricVarDecomposition`, `parametricEsDecomposition`,
//!   `historicalVarDecomposition`, `evaluateRiskBudget`
//!
//! **Experimental** — calibration parameters or signatures still under
//! review:
//! - `lvarBangia` — endogenous-cost coefficient is a calibration default; see
//!   `LiquidityConfig::endogenous_spread_coef` in the Rust crate.
//! - `almgrenChrissImpact` — `delta` is fixed at 0.5; the underlying
//!   `optimal_trajectory` accepts only `delta = 1` (linear impact).
//! - `kyleLambda`, `rollEffectiveSpread`, `amihudIlliquidity`,
//!   `daysToLiquidate`, `liquidityTier` — small free functions; may be
//!   re-grouped or renamed.
//!
//! For repeated calls against the same portfolio (scenario sweeps,
//! interactive dashboards), prefer the `*Built` variants which take a
//! `Portfolio` handle and skip the per-call `from_spec` rebuild.

use std::sync::Arc;

use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

pub mod sensitivity;

// ---------------------------------------------------------------------------
// Typed handle: Portfolio
// ---------------------------------------------------------------------------

/// Handle to a built [`finstack_quant_portfolio::Portfolio`] that can be reused
/// across WASM calls without re-parsing and rebuilding from the spec.
///
/// `Portfolio::from_spec` parses positions, builds indices, and validates
/// invariants; for pipelines that call both `valuePortfolio` and
/// `aggregateFullCashflows` on the same portfolio, holding this handle
/// avoids paying that cost twice.
#[wasm_bindgen(js_name = Portfolio)]
pub struct WasmPortfolio {
    #[wasm_bindgen(skip)]
    pub(crate) inner: Arc<finstack_quant_portfolio::Portfolio>,
}

#[wasm_bindgen(js_class = Portfolio)]
impl WasmPortfolio {
    /// Build from a JSON-serialised `PortfolioSpec`.
    /// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(spec_json: &str) -> Result<WasmPortfolio, JsValue> {
        let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
            serde_json::from_str(spec_json).map_err(to_js_err)?;
        let portfolio = finstack_quant_portfolio::Portfolio::from_spec(spec).map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(portfolio),
        })
    }

    /// Portfolio identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Valuation date (ISO 8601).
    #[wasm_bindgen(getter, js_name = asOf)]
    pub fn as_of(&self) -> String {
        self.inner.as_of.to_string()
    }

    /// Base currency code.
    #[wasm_bindgen(getter, js_name = baseCcy)]
    pub fn base_ccy(&self) -> String {
        self.inner.base_ccy.to_string()
    }

    /// Number of positions in the portfolio.
    #[wasm_bindgen(js_name = numPositions)]
    pub fn num_positions(&self) -> usize {
        self.inner.positions().len()
    }

    /// Serialise the canonical spec back to JSON.
    #[wasm_bindgen(js_name = toSpecJson)]
    pub fn to_spec_json(&self) -> Result<String, JsValue> {
        let spec = self.inner.to_spec();
        serde_json::to_string(&spec).map_err(to_js_err)
    }
}

/// Parse and validate a portfolio specification from JSON.
///
/// Returns the re-serialized canonical JSON form.
/// @param json_str - Canonical JSON string to validate, parse, or normalize for this API.
#[wasm_bindgen(js_name = parsePortfolioSpec)]
pub fn parse_portfolio_spec(json_str: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
        serde_json::from_str(json_str).map_err(to_js_err)?;

    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Compute a single-period Brinson-Fachler attribution from sector JSON.
///
/// Accepts a JSON array of `SectorPeriod` objects and returns a JSON
/// `BrinsonPeriodResult`.
/// @param sectors_json - Canonical JSON payload representing the sectors consumed by this API.
#[wasm_bindgen(js_name = brinsonFachler)]
pub fn brinson_fachler(sectors_json: &str) -> Result<String, JsValue> {
    let sectors: Vec<finstack_quant_portfolio::SectorPeriod> =
        serde_json::from_str(sectors_json).map_err(to_js_err)?;
    let result = finstack_quant_portfolio::brinson_fachler(&sectors).map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Compute Carino-linked multi-period Brinson attribution from period JSON.
///
/// Accepts a JSON array of periods, where each period is an array of
/// `SectorPeriod` objects, and returns a JSON `CarinoLinkedAttribution`.
/// @param periods_json - Canonical JSON payload representing the periods consumed by this API.
#[wasm_bindgen(js_name = carinoLink)]
pub fn carino_link(periods_json: &str) -> Result<String, JsValue> {
    let periods: Vec<Vec<finstack_quant_portfolio::SectorPeriod>> =
        serde_json::from_str(periods_json).map_err(to_js_err)?;
    let result =
        finstack_quant_portfolio::carino_link_from_sector_periods(&periods).map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Compute a Modified-Dietz TWRR sub-period return from period JSON.
/// @param period_json - Canonical JSON payload representing the period consumed by this API.
#[wasm_bindgen(js_name = twrrModifiedDietz)]
pub fn twrr_modified_dietz(period_json: &str) -> Result<Option<f64>, JsValue> {
    let period: finstack_quant_portfolio::TwrrPeriod =
        serde_json::from_str(period_json).map_err(to_js_err)?;
    Ok(finstack_quant_portfolio::twrr_modified_dietz(&period))
}

/// Geometrically link TWRR sub-period returns from returns JSON.
/// @param returns_json - Canonical JSON payload representing the returns consumed by this API.
/// @param horizon_years - Return-linking horizon measured in years for annualization.
#[wasm_bindgen(js_name = twrrLinked)]
pub fn twrr_linked(returns_json: &str, horizon_years: f64) -> Result<Option<String>, JsValue> {
    let returns: Vec<f64> = serde_json::from_str(returns_json).map_err(to_js_err)?;
    finstack_quant_portfolio::twrr_linked(&returns, horizon_years)
        .map(|result| serde_json::to_string(&result).map_err(to_js_err))
        .transpose()
}

/// Compute money-weighted return via XIRR from dated cashflow JSON.
/// @param cashflows_json - Canonical JSON payload representing the cashflows consumed by this API.
#[wasm_bindgen(js_name = mwrXirr)]
pub fn mwr_xirr(cashflows_json: &str) -> Result<f64, JsValue> {
    let cashflows: Vec<finstack_quant_portfolio::DatedCashflow> =
        serde_json::from_str(cashflows_json).map_err(to_js_err)?;
    finstack_quant_portfolio::mwr_xirr_from_cashflows(&cashflows).map_err(to_js_err)
}

/// Build a runtime portfolio from a JSON spec, validate, and round-trip.
///
/// Deserializes the spec, constructs the portfolio with live instruments,
/// validates structural invariants, then re-serializes for confirmation.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
#[wasm_bindgen(js_name = buildPortfolioFromSpec)]
pub fn build_portfolio_from_spec(spec_json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
        serde_json::from_str(spec_json).map_err(to_js_err)?;

    let portfolio = finstack_quant_portfolio::Portfolio::from_spec(spec).map_err(to_js_err)?;

    let round_tripped = portfolio.to_spec();
    serde_json::to_string(&round_tripped).map_err(to_js_err)
}

/// Extract the total portfolio value from a JSON result.
/// @param result_json - Canonical JSON payload representing the result consumed by this API.
#[wasm_bindgen(js_name = portfolioResultTotalValue)]
pub fn portfolio_result_total_value(result_json: &str) -> Result<f64, JsValue> {
    let result: finstack_quant_portfolio::results::PortfolioResult =
        serde_json::from_str(result_json).map_err(to_js_err)?;

    Ok(result.total_value().amount())
}

/// Extract a specific metric from a portfolio result JSON.
///
/// Returns `undefined` (via `Option`) if the metric was not produced.
/// @param result_json - Canonical JSON payload representing the result consumed by this API.
/// @param metric_id - Stable metric identifier used to select the required domain object.
#[wasm_bindgen(js_name = portfolioResultGetMetric)]
pub fn portfolio_result_get_metric(result_json: &str, metric_id: &str) -> Result<JsValue, JsValue> {
    let result: finstack_quant_portfolio::results::PortfolioResult =
        serde_json::from_str(result_json).map_err(to_js_err)?;

    match result.get_metric(metric_id) {
        Some(v) => Ok(JsValue::from_f64(v)),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Aggregate portfolio metrics from a valuation JSON.
/// @param valuation_json - Canonical JSON payload representing the valuation consumed by this API.
/// @param base_ccy - ISO-4217 base currency in which aggregate portfolio values are reported.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
#[wasm_bindgen(js_name = aggregateMetrics)]
pub fn aggregate_metrics(
    valuation_json: &str,
    base_ccy: &str,
    market_json: &str,
    as_of: &str,
) -> Result<String, JsValue> {
    let valuation: finstack_quant_portfolio::valuation::PortfolioValuation =
        serde_json::from_str(valuation_json).map_err(to_js_err)?;
    let ccy: finstack_quant_core::currency::Currency = base_ccy.parse().map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    let date = time::Date::parse(as_of, &format).map_err(to_js_err)?;
    let metrics =
        finstack_quant_portfolio::metrics::aggregate_metrics(&valuation, ccy, &market, date)
            .map_err(to_js_err)?;
    serde_json::to_string(&metrics).map_err(to_js_err)
}

/// Value a portfolio from its spec and market context.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param strict_risk - Whether unavailable risk metrics are treated as calculation errors.
#[wasm_bindgen(js_name = valuePortfolio)]
pub fn value_portfolio(
    spec_json: &str,
    market_json: &str,
    strict_risk: bool,
) -> Result<String, JsValue> {
    let portfolio = WasmPortfolio::from_spec(spec_json)?;
    value_portfolio_built(&portfolio, market_json, strict_risk)
}

/// Aggregate the full classified cashflow ladder for a portfolio.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
#[wasm_bindgen(js_name = aggregateFullCashflows)]
pub fn aggregate_full_cashflows(spec_json: &str, market_json: &str) -> Result<String, JsValue> {
    let portfolio = WasmPortfolio::from_spec(spec_json)?;
    aggregate_full_cashflows_built(&portfolio, market_json)
}

/// Aggregate the full classified cashflow ladder for an already-built
/// [`WasmPortfolio`] handle.
///
/// Skips the per-call `PortfolioSpec` parse + `Portfolio::from_spec` rebuild.
/// For batched or chained workflows (repeated cashflow builds across market
/// scenarios on the same portfolio), this is the cheap path.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
#[wasm_bindgen(js_name = aggregateFullCashflowsBuilt)]
pub fn aggregate_full_cashflows_built(
    portfolio: &WasmPortfolio,
    market_json: &str,
) -> Result<String, JsValue> {
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let cashflows =
        finstack_quant_portfolio::cashflows::aggregate_full_cashflows(&portfolio.inner, &market)
            .map_err(to_js_err)?;
    serde_json::to_string(&cashflows).map_err(to_js_err)
}

/// Value an already-built [`WasmPortfolio`] handle. Skips the per-call
/// `PortfolioSpec` parse + `Portfolio::from_spec` rebuild that
/// [`value_portfolio`] performs; use this when sweeping market scenarios
/// against a fixed portfolio.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param strict_risk - Whether unavailable risk metrics are treated as calculation errors.
#[wasm_bindgen(js_name = valuePortfolioBuilt)]
pub fn value_portfolio_built(
    portfolio: &WasmPortfolio,
    market_json: &str,
    strict_risk: bool,
) -> Result<String, JsValue> {
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let options = finstack_quant_portfolio::valuation::PortfolioValuationOptions {
        strict_risk,
        ..Default::default()
    };
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio.inner,
        &market,
        &config,
        &options,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&valuation).map_err(to_js_err)
}

/// Apply a scenario to an already-built [`WasmPortfolio`] handle and revalue.
/// Returns a JS object with structured `valuation` and `report` values.
/// @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
/// @param scenario_json - Canonical JSON payload representing the scenario consumed by this API.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
#[wasm_bindgen(js_name = applyScenarioAndRevalueBuilt)]
pub fn apply_scenario_and_revalue_built(
    portfolio: &WasmPortfolio,
    scenario_json: &str,
    market_json: &str,
) -> Result<JsValue, JsValue> {
    let scenario: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let out = finstack_quant_portfolio::scenarios::apply_and_revalue_envelope(
        &portfolio.inner,
        &scenario,
        &market,
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&out)
}

/// Apply a scenario to a portfolio and revalue.
///
/// Returns a JS object with structured `valuation` and `report` values.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param scenario_json - Canonical JSON payload representing the scenario consumed by this API.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
#[wasm_bindgen(js_name = applyScenarioAndRevalue)]
pub fn apply_scenario_and_revalue(
    spec_json: &str,
    scenario_json: &str,
    market_json: &str,
) -> Result<JsValue, JsValue> {
    let portfolio = WasmPortfolio::from_spec(spec_json)?;
    apply_scenario_and_revalue_built(&portfolio, scenario_json, market_json)
}

/// Optimize portfolio weights using the LP-based optimizer.
///
/// Accepts a `PortfolioOptimizationSpec` JSON (portfolio + objective +
/// constraints + options) and a `MarketContext` JSON.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
#[wasm_bindgen(js_name = optimizePortfolio)]
pub fn optimize_portfolio(spec_json: &str, market_json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::optimization::PortfolioOptimizationSpec =
        serde_json::from_str(spec_json).map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let result =
        finstack_quant_portfolio::optimization::optimize_from_spec(&spec, &market, &config)
            .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Replay a portfolio through dated market snapshots.
///
/// Accepts a portfolio spec, an array of dated market snapshots, and a
/// replay configuration. Returns a JSON-serialized `ReplayResult`.
/// @param spec_json - Canonical portfolio specification JSON defining positions, quantities, and base currency.
/// @param snapshots_json - Canonical JSON payload representing the snapshots consumed by this API.
/// @param config_json - Canonical JSON payload representing the config consumed by this API.
#[wasm_bindgen(js_name = replayPortfolio)]
pub fn replay_portfolio(
    spec_json: &str,
    snapshots_json: &str,
    config_json: &str,
) -> Result<String, JsValue> {
    let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
        serde_json::from_str(spec_json).map_err(to_js_err)?;
    let portfolio = finstack_quant_portfolio::Portfolio::from_spec(spec).map_err(to_js_err)?;
    let config: finstack_quant_portfolio::replay::ReplayConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let timeline =
        finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(snapshots_json)
            .map_err(to_js_err)?;
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let result = finstack_quant_portfolio::replay::replay_portfolio(
        &portfolio,
        &timeline,
        &config,
        &finstack_config,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

// =============================================================================
// Position-level VaR / ES decomposition and risk budgeting
// =============================================================================

/// Decompose portfolio VaR into position contributions via parametric Euler
/// allocation. Inputs mirror the Python binding's signature.
///
/// `covariance_json` must deserialize to an `n x n` row-major nested array.
/// @param position_ids_json - Canonical JSON payload representing the position ids consumed by this API.
/// @param weights_json - Canonical JSON payload representing the weights consumed by this API.
/// @param covariance_json - Canonical JSON payload representing the covariance consumed by this API.
/// @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
#[wasm_bindgen(js_name = parametricVarDecomposition)]
pub fn parametric_var_decomposition(
    position_ids_json: &str,
    weights_json: &str,
    covariance_json: &str,
    confidence: f64,
) -> Result<String, JsValue> {
    use finstack_quant_portfolio::factor_model::{
        DecompositionConfig, ParametricPositionDecomposer, parametric_var_decomposition_view,
    };
    use finstack_quant_portfolio::types::PositionId;

    let ids: Vec<String> = serde_json::from_str(position_ids_json).map_err(to_js_err)?;
    let weights: Vec<f64> = serde_json::from_str(weights_json).map_err(to_js_err)?;
    let covariance: Vec<Vec<f64>> = serde_json::from_str(covariance_json).map_err(to_js_err)?;
    let n = weights.len();
    let cov_flat = flatten_square_matrix(covariance, n, "covariance")?;
    let ids: Vec<PositionId> = ids.into_iter().map(PositionId::new).collect();

    let mut config = DecompositionConfig::parametric_95();
    config.confidence = confidence;

    let decomposer = ParametricPositionDecomposer;
    let result = decomposer
        .decompose_positions(&weights, &cov_flat, &ids, &config)
        .map_err(to_js_err)?;
    let out = parametric_var_decomposition_view(&result);
    serde_json::to_string(&out).map_err(to_js_err)
}

/// Decompose portfolio Expected Shortfall into position contributions via
/// parametric Euler allocation.
///
/// Returns an ES-shaped JSON payload mirroring the Python
/// ``parametric_es_decomposition`` return value: a top-level
/// ``{portfolio_var, portfolio_es, confidence, n_positions, contributions}``
/// object whose ``contributions`` entries are
/// ``{position_id, component_es, marginal_es, pct_contribution}``.
/// @param position_ids_json - Canonical JSON payload representing the position ids consumed by this API.
/// @param weights_json - Canonical JSON payload representing the weights consumed by this API.
/// @param covariance_json - Canonical JSON payload representing the covariance consumed by this API.
/// @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
#[wasm_bindgen(js_name = parametricEsDecomposition)]
pub fn parametric_es_decomposition(
    position_ids_json: &str,
    weights_json: &str,
    covariance_json: &str,
    confidence: f64,
) -> Result<String, JsValue> {
    use finstack_quant_portfolio::factor_model::{
        DecompositionConfig, ParametricPositionDecomposer, parametric_es_decomposition_view,
    };
    use finstack_quant_portfolio::types::PositionId;

    let ids: Vec<String> = serde_json::from_str(position_ids_json).map_err(to_js_err)?;
    let weights: Vec<f64> = serde_json::from_str(weights_json).map_err(to_js_err)?;
    let covariance: Vec<Vec<f64>> = serde_json::from_str(covariance_json).map_err(to_js_err)?;
    let n = weights.len();
    let cov_flat = flatten_square_matrix(covariance, n, "covariance")?;
    let ids: Vec<PositionId> = ids.into_iter().map(PositionId::new).collect();

    let mut config = DecompositionConfig::parametric_95();
    config.confidence = confidence;

    let decomposition = ParametricPositionDecomposer
        .decompose_positions(&weights, &cov_flat, &ids, &config)
        .map_err(to_js_err)?;

    let out = parametric_es_decomposition_view(&decomposition);
    serde_json::to_string(&out).map_err(to_js_err)
}

/// Decompose portfolio VaR/ES from per-position scenario P&Ls via historical
/// simulation.
///
/// `position_pnls_json` is a nested array shaped `[n_positions][n_scenarios]`.
/// @param position_ids_json - Canonical JSON payload representing the position ids consumed by this API.
/// @param position_pnls_json - Canonical JSON payload representing the position pnls consumed by this API.
/// @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
#[wasm_bindgen(js_name = historicalVarDecomposition)]
pub fn historical_var_decomposition(
    position_ids_json: &str,
    position_pnls_json: &str,
    confidence: f64,
) -> Result<String, JsValue> {
    use finstack_quant_portfolio::factor_model::{
        DecompositionConfig, HistoricalPositionDecomposer, flatten_position_pnls,
        parametric_var_decomposition_view,
    };
    use finstack_quant_portfolio::types::PositionId;

    let ids: Vec<String> = serde_json::from_str(position_ids_json).map_err(to_js_err)?;
    let position_pnls: Vec<Vec<f64>> =
        serde_json::from_str(position_pnls_json).map_err(to_js_err)?;
    let n = ids.len();
    let ids: Vec<PositionId> = ids.into_iter().map(PositionId::new).collect();
    let config = DecompositionConfig::historical(confidence);

    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, n).map_err(to_js_err)?;
    let result = HistoricalPositionDecomposer
        .decompose_from_pnls(&flat, &ids, n_scenarios, &config)
        .map_err(to_js_err)?;
    let out = parametric_var_decomposition_view(&result);
    serde_json::to_string(&out).map_err(to_js_err)
}

/// Evaluate a per-position risk budget against actual component VaRs.
/// @param position_ids_json - Canonical JSON payload representing the position ids consumed by this API.
/// @param actual_var_json - Canonical JSON payload representing the actual var consumed by this API.
/// @param target_var_pct_json - Canonical JSON payload representing the target var pct consumed by this API.
/// @param portfolio_var - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
/// @param utilization_threshold - Actual-to-target risk ratio that flags a budget breach.
#[wasm_bindgen(js_name = evaluateRiskBudget)]
pub fn evaluate_risk_budget(
    position_ids_json: &str,
    actual_var_json: &str,
    target_var_pct_json: &str,
    portfolio_var: f64,
    utilization_threshold: f64,
) -> Result<String, JsValue> {
    use finstack_quant_portfolio::factor_model::{RiskBudget, risk_budget_result_view};
    use finstack_quant_portfolio::types::PositionId;
    use indexmap::IndexMap;

    let ids: Vec<String> = serde_json::from_str(position_ids_json).map_err(to_js_err)?;
    let actual_var: Vec<f64> = serde_json::from_str(actual_var_json).map_err(to_js_err)?;
    let target_var_pct: Vec<f64> = serde_json::from_str(target_var_pct_json).map_err(to_js_err)?;
    let n = ids.len();
    if actual_var.len() != n {
        return Err(to_js_err(format!(
            "actual_var length ({}) must match position_ids length ({n})",
            actual_var.len()
        )));
    }
    if target_var_pct.len() != n {
        return Err(to_js_err(format!(
            "target_var_pct length ({}) must match position_ids length ({n})",
            target_var_pct.len()
        )));
    }

    let shared_ids: Vec<PositionId> = ids.into_iter().map(PositionId::new).collect();
    let mut targets: IndexMap<PositionId, f64> = IndexMap::with_capacity(n);
    for (id, &pct) in shared_ids.iter().zip(target_var_pct.iter()) {
        targets.insert(id.clone(), pct);
    }
    let budget = RiskBudget::new(targets).with_threshold(utilization_threshold);
    let result = budget
        .evaluate_components(
            shared_ids.iter().zip(actual_var.iter().copied()),
            portfolio_var,
        )
        .map_err(to_js_err)?;
    let out = risk_budget_result_view(&result, portfolio_var, utilization_threshold);
    serde_json::to_string(&out).map_err(to_js_err)
}

/// Forward to the shared `finstack_quant_portfolio::factor_model::flatten_square_matrix`
/// and remap the validation error to a `JsValue` so the same matrix-validation
/// diagnostics surface from both the WASM and Python bindings.
fn flatten_square_matrix(
    matrix: Vec<Vec<f64>>,
    n: usize,
    label: &str,
) -> Result<Vec<f64>, JsValue> {
    finstack_quant_portfolio::factor_model::flatten_square_matrix(matrix, n, label)
        .map_err(to_js_err)
}

// =============================================================================
// Liquidity: spread estimators, tiering, LVaR, market impact
// =============================================================================

/// Effective bid-ask spread via Roll (1984). Returns `undefined` when the
/// serial covariance is non-negative (Roll assumption violated) or inputs too short.
/// @param returns_json - Canonical JSON payload representing the returns consumed by this API.
#[wasm_bindgen(js_name = rollEffectiveSpread)]
pub fn roll_effective_spread(returns_json: &str) -> Result<Option<f64>, JsValue> {
    let returns: Vec<f64> = serde_json::from_str(returns_json).map_err(to_js_err)?;
    Ok(finstack_quant_portfolio::liquidity::roll_effective_spread(
        &returns,
    ))
}

/// Amihud (2002) illiquidity ratio from returns and volumes.
/// @param returns_json - Canonical JSON payload representing the returns consumed by this API.
/// @param volumes_json - Canonical JSON payload representing the volumes consumed by this API.
#[wasm_bindgen(js_name = amihudIlliquidity)]
pub fn amihud_illiquidity(returns_json: &str, volumes_json: &str) -> Result<Option<f64>, JsValue> {
    let returns: Vec<f64> = serde_json::from_str(returns_json).map_err(to_js_err)?;
    let volumes: Vec<f64> = serde_json::from_str(volumes_json).map_err(to_js_err)?;
    Ok(finstack_quant_portfolio::liquidity::amihud_illiquidity(
        &returns, &volumes,
    ))
}

/// Trading days required to liquidate at the given participation rate.
/// @param position_value - Current position market value in the relevant currency units.
/// @param avg_daily_volume - Average daily trading volume in the same units as the position size.
/// @param participation_rate - Maximum fraction of average daily volume used for execution.
#[wasm_bindgen(js_name = daysToLiquidate)]
pub fn days_to_liquidate(
    position_value: f64,
    avg_daily_volume: f64,
    participation_rate: f64,
) -> f64 {
    finstack_quant_portfolio::liquidity::days_to_liquidate(
        position_value,
        avg_daily_volume,
        participation_rate,
    )
}

/// Classify a position into a liquidity tier from its days-to-liquidate.
///
/// Uses the default `[1, 5, 20, 60]` trading-day thresholds. Returns one of
/// `"tier1" .. "tier5"`.
/// @param days_to_liquidate - Days to liquidate supplied to liquidity tier; follow the type and convention required by the surrounding API.
#[wasm_bindgen(js_name = liquidityTier)]
pub fn liquidity_tier(days_to_liquidate: f64) -> String {
    use finstack_quant_portfolio::liquidity::classify_tier;
    let config = finstack_quant_portfolio::liquidity::LiquidityConfig::default();
    classify_tier(days_to_liquidate, &config.tier_thresholds)
        .as_binding_str()
        .to_string()
}

/// Liquidity-adjusted VaR following Bangia, Diebold, Schuermann & Stroughair (1999).
/// Loss sign convention: `var` and `lvar` are non-positive.
/// @param var - Base market value-at-risk before adding the liquidity adjustment.
/// @param spread_mean - Mean bid-ask spread in the quote units required by the liquidity model.
/// @param spread_vol - Volatility of the bid-ask spread in the liquidity model's units.
/// @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
/// @param position_value - Current position market value in the relevant currency units.
#[wasm_bindgen(js_name = lvarBangia)]
pub fn lvar_bangia(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> Result<String, JsValue> {
    let result = finstack_quant_portfolio::liquidity::lvar_bangia_scalar(
        var,
        spread_mean,
        spread_vol,
        confidence,
        position_value,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Almgren-Chriss (2001) market impact decomposition for a uniform execution.
/// @param position_size - Trade size in shares or notional units for the execution calculation.
/// @param avg_daily_volume - Average daily trading volume in the same units as the position size.
/// @param volatility - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param execution_horizon_days - Planned execution horizon measured in trading days.
/// @param permanent_impact_coef - Permanent market-impact coefficient in the execution-cost model.
/// @param temporary_impact_coef - Temporary market-impact coefficient in the execution-cost model.
/// @param reference_price - Optional reference price used to express execution impact in monetary units.
#[wasm_bindgen(js_name = almgrenChrissImpact)]
pub fn almgren_chriss_impact(
    position_size: f64,
    avg_daily_volume: f64,
    volatility: f64,
    execution_horizon_days: f64,
    permanent_impact_coef: f64,
    temporary_impact_coef: f64,
    reference_price: Option<f64>,
) -> Result<String, JsValue> {
    let out = finstack_quant_portfolio::liquidity::almgren_chriss_uniform_impact(
        position_size,
        avg_daily_volume,
        volatility,
        execution_horizon_days,
        permanent_impact_coef,
        temporary_impact_coef,
        reference_price,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&out).map_err(to_js_err)
}

/// Kyle (1985) linear price impact lambda estimated from observed volumes
/// and returns via the Amihud-ratio proxy. Returns `undefined` on invalid inputs.
/// @param volumes_json - Canonical JSON payload representing the volumes consumed by this API.
/// @param returns_json - Canonical JSON payload representing the returns consumed by this API.
#[wasm_bindgen(js_name = kyleLambda)]
pub fn kyle_lambda(volumes_json: &str, returns_json: &str) -> Result<Option<f64>, JsValue> {
    let volumes: Vec<f64> = serde_json::from_str(volumes_json).map_err(to_js_err)?;
    let returns: Vec<f64> = serde_json::from_str(returns_json).map_err(to_js_err)?;
    Ok(
        finstack_quant_portfolio::liquidity::KyleLambdaModel::lambda_from_series(
            &volumes, &returns,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_portfolio_spec_json() -> String {
        serde_json::json!({
            "id": "test_portfolio",
            "name": "Test",
            "base_ccy": "USD",
            "as_of": "2024-01-15",
            "entities": {},
            "positions": []
        })
        .to_string()
    }

    #[test]
    fn parse_portfolio_spec_roundtrip() {
        let json = minimal_portfolio_spec_json();
        let result = parse_portfolio_spec(&json).expect("parse");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["id"], "test_portfolio");
    }

    #[test]
    fn build_portfolio_from_spec_empty() {
        let json = minimal_portfolio_spec_json();
        let result = build_portfolio_from_spec(&json).expect("build");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["id"], "test_portfolio");
    }

    #[test]
    fn parse_and_rebuild_roundtrip() {
        let json = minimal_portfolio_spec_json();
        let canonical = parse_portfolio_spec(&json).expect("parse");
        let rebuilt = build_portfolio_from_spec(&canonical).expect("rebuild");
        let a: serde_json::Value = serde_json::from_str(&canonical).expect("a");
        let b: serde_json::Value = serde_json::from_str(&rebuilt).expect("b");
        assert_eq!(a["id"], b["id"]);
    }

    fn empty_market_json() -> String {
        let ctx = finstack_quant_core::market_data::context::MarketContext::new();
        serde_json::to_string(&ctx).expect("serialize")
    }

    #[test]
    fn value_empty_portfolio() {
        let spec = minimal_portfolio_spec_json();
        let market = empty_market_json();
        let result = value_portfolio(&spec, &market, false).expect("value");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(parsed.is_object());
    }

    #[test]
    fn aggregate_full_cashflows_empty_portfolio() {
        let spec = minimal_portfolio_spec_json();
        let market = empty_market_json();
        let result = aggregate_full_cashflows(&spec, &market).expect("aggregate full");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");

        assert_eq!(parsed["events"], serde_json::json!([]));
        assert_eq!(parsed["by_position"], serde_json::json!({}));
        assert_eq!(parsed["by_date"], serde_json::json!({}));
        assert_eq!(parsed["position_summaries"], serde_json::json!({}));
        assert_eq!(parsed["issues"], serde_json::json!([]));
    }

    #[test]
    fn portfolio_handle_roundtrip_and_aggregate_cashflows_built() {
        let spec_json = minimal_portfolio_spec_json();
        let handle = WasmPortfolio::from_spec(&spec_json).expect("build handle");
        assert_eq!(handle.id(), "test_portfolio");
        assert_eq!(handle.base_ccy(), "USD");
        assert_eq!(handle.as_of(), "2024-01-15");
        assert_eq!(handle.num_positions(), 0);

        let round = handle.to_spec_json().expect("to spec json");
        let parsed: serde_json::Value = serde_json::from_str(&round).expect("json");
        assert_eq!(parsed["id"], "test_portfolio");

        let market = empty_market_json();
        let via_built =
            aggregate_full_cashflows_built(&handle, &market).expect("aggregate full built");
        let via_spec = aggregate_full_cashflows(&spec_json, &market).expect("aggregate full spec");
        let a: serde_json::Value = serde_json::from_str(&via_built).expect("a");
        let b: serde_json::Value = serde_json::from_str(&via_spec).expect("b");
        assert_eq!(a, b);
    }

    #[test]
    fn portfolio_result_total_value_from_valuation() {
        let spec = minimal_portfolio_spec_json();
        let market = empty_market_json();
        let valuation_json = value_portfolio(&spec, &market, false).expect("value");
        let result = finstack_quant_portfolio::results::PortfolioResult::new(
            serde_json::from_str(&valuation_json).expect("deser"),
            Default::default(),
            Default::default(),
        );
        let result_json = serde_json::to_string(&result).expect("ser");
        let total = portfolio_result_total_value(&result_json).expect("total");
        assert!(total.is_finite());
    }

    #[test]
    fn aggregate_metrics_empty_portfolio() {
        let spec = minimal_portfolio_spec_json();
        let market = empty_market_json();
        let valuation_json = value_portfolio(&spec, &market, false).expect("value");
        let result = aggregate_metrics(&valuation_json, "USD", &market, "2024-01-15").expect("agg");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(parsed.is_object());
    }

    /// Tests the replay_portfolio WASM binding logic by exercising the same
    /// JSON parsing / domain call / serialization pipeline directly.
    /// We call the domain functions instead of the wasm wrapper because
    /// `JsValue::from_str` panics on non-wasm32 targets when an error is
    /// produced.
    #[test]
    fn replay_portfolio_empty_portfolio() {
        let spec_json = minimal_portfolio_spec_json();
        let spec: finstack_quant_portfolio::portfolio::PortfolioSpec =
            serde_json::from_str(&spec_json).expect("parse spec");
        let portfolio =
            finstack_quant_portfolio::Portfolio::from_spec(spec).expect("build portfolio");

        let market_val: serde_json::Value =
            serde_json::from_str(&empty_market_json()).expect("parse market");
        let snapshots_json = serde_json::json!([
            {"date": "2024-01-15", "market": market_val},
            {"date": "2024-01-16", "market": market_val}
        ])
        .to_string();

        let timeline =
            finstack_quant_portfolio::replay::ReplayTimeline::from_json_snapshots(&snapshots_json)
                .expect("build timeline");

        let config_json = serde_json::json!({
            "mode": "PvOnly",
            "attribution_method": "Parallel"
        })
        .to_string();
        let config: finstack_quant_portfolio::replay::ReplayConfig =
            serde_json::from_str(&config_json).expect("parse config");

        let finstack_config = finstack_quant_core::config::FinstackConfig::default();

        let result = finstack_quant_portfolio::replay::replay_portfolio(
            &portfolio,
            &timeline,
            &config,
            &finstack_config,
        )
        .expect("replay");

        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse json");
        assert!(parsed["steps"].is_array());
        assert_eq!(parsed["steps"].as_array().expect("array").len(), 2);
    }

    #[test]
    fn almgren_chriss_impact_uses_reference_price_for_bps() {
        let default_json = almgren_chriss_impact(10_000.0, 1_000_000.0, 0.02, 1.0, 0.0, 0.01, None)
            .expect("default reference price");
        let priced_json =
            almgren_chriss_impact(10_000.0, 1_000_000.0, 0.02, 1.0, 0.0, 0.01, Some(100.0))
                .expect("explicit reference price");

        let default: serde_json::Value = serde_json::from_str(&default_json).expect("json");
        let priced: serde_json::Value = serde_json::from_str(&priced_json).expect("json");
        let default_bps = default["expected_cost_bps"].as_f64().expect("default bps");
        let priced_bps = priced["expected_cost_bps"].as_f64().expect("priced bps");

        assert!((priced_bps - default_bps / 100.0).abs() < 1e-12);
    }

    #[test]
    fn brinson_fachler_reconstructs_active_return() {
        let sectors = serde_json::json!([
            {
                "sector": "A",
                "portfolio_weight": 0.60,
                "benchmark_weight": 0.40,
                "portfolio_return": 0.08,
                "benchmark_return": 0.06
            },
            {
                "sector": "B",
                "portfolio_weight": 0.40,
                "benchmark_weight": 0.60,
                "portfolio_return": 0.01,
                "benchmark_return": 0.03
            }
        ]);
        let result = brinson_fachler(&sectors.to_string()).expect("brinson attribution");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        let reconstructed = parsed["total_allocation"].as_f64().expect("allocation")
            + parsed["total_selection"].as_f64().expect("selection")
            + parsed["total_interaction"].as_f64().expect("interaction");
        let active = parsed["total_excess_return"].as_f64().expect("active");

        assert!((reconstructed - active).abs() < 1e-12);
    }

    #[test]
    fn carino_link_reconstructs_compounded_active_return() {
        let periods = serde_json::json!([
            [
                {
                    "sector": "A",
                    "portfolio_weight": 0.70,
                    "benchmark_weight": 0.50,
                    "portfolio_return": 0.10,
                    "benchmark_return": 0.06
                },
                {
                    "sector": "B",
                    "portfolio_weight": 0.30,
                    "benchmark_weight": 0.50,
                    "portfolio_return": 0.04,
                    "benchmark_return": 0.05
                }
            ],
            [
                {
                    "sector": "A",
                    "portfolio_weight": 0.60,
                    "benchmark_weight": 0.50,
                    "portfolio_return": 0.02,
                    "benchmark_return": 0.03
                },
                {
                    "sector": "B",
                    "portfolio_weight": 0.40,
                    "benchmark_weight": 0.50,
                    "portfolio_return": -0.01,
                    "benchmark_return": 0.00
                }
            ]
        ]);
        let result = carino_link(&periods.to_string()).expect("carino attribution");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        let geometric_active = parsed["portfolio_return_compounded"]
            .as_f64()
            .expect("portfolio")
            - parsed["benchmark_return_compounded"]
                .as_f64()
                .expect("benchmark");
        let reconstructed = parsed["linked_allocation"].as_f64().expect("allocation")
            + parsed["linked_selection"].as_f64().expect("selection")
            + parsed["linked_interaction"].as_f64().expect("interaction");

        assert!((reconstructed - geometric_active).abs() < 1e-10);
    }

    #[test]
    fn twrr_modified_dietz_matches_gips_example() {
        let period = serde_json::json!({
            "beginning_market_value": 10_000_000.0,
            "ending_market_value": 10_500_000.0,
            "cashflows": [
                {
                    "amount": 1_000_000.0,
                    "fraction_of_period_remaining": 0.60
                }
            ]
        });

        let result = twrr_modified_dietz(&period.to_string())
            .expect("modified dietz")
            .expect("defined return");
        let expected = -500_000.0 / 10_600_000.0;
        assert!((result - expected).abs() < 1e-12);
    }

    #[test]
    fn twrr_linked_geometrically_links_returns() {
        let result = twrr_linked(&serde_json::json!([0.05, 0.03]).to_string(), 1.0)
            .expect("linked return")
            .expect("defined return");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");

        assert!((parsed["cumulative"].as_f64().expect("cumulative") - 0.0815).abs() < 1e-12);
        assert!((parsed["annualised"].as_f64().expect("annualised") - 0.0815).abs() < 1e-12);
        assert_eq!(parsed["num_periods"], serde_json::json!(2));
    }

    #[test]
    fn mo24_liquidity_estimators_return_none_for_missing_estimates() {
        assert_eq!(roll_effective_spread("[0.01]").expect("valid json"), None);
        assert_eq!(
            amihud_illiquidity("[0.01]", "[0.0]").expect("valid json"),
            None
        );
        assert_eq!(kyle_lambda("[0.0]", "[0.01]").expect("valid json"), None);
    }

    #[test]
    fn mo28_liquidity_tier_uses_default_config_thresholds() {
        let config = finstack_quant_portfolio::liquidity::LiquidityConfig::default();
        let threshold = config.tier_thresholds[0];
        let expected =
            finstack_quant_portfolio::liquidity::classify_tier(threshold, &config.tier_thresholds)
                .as_binding_str()
                .to_string();

        assert_eq!(liquidity_tier(threshold), expected);
    }

    #[test]
    fn mwr_xirr_solves_money_weighted_return() {
        let cashflows = serde_json::json!([
            {"date": "2025-01-01", "amount": -100.0},
            {"date": "2026-01-01", "amount": 110.0}
        ]);

        let result = mwr_xirr(&cashflows.to_string()).expect("xirr");
        assert!((result - 0.10).abs() < 1e-6);
    }
}
