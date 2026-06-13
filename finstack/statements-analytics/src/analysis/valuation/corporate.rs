//! Corporate valuation using DCF methodology.
//!
//! This module provides integration between financial statement models and
//! DCF (Discounted Cash Flow) valuation, allowing direct valuation of companies
//! from forecast models.

use finstack_core::currency::Currency;
use finstack_core::explain::{ExplanationTrace, TraceEntry};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_statements::error::Result;
use finstack_statements::evaluator::{Evaluator, StatementResult};
use finstack_statements::types::FinancialModelSpec;
use finstack_valuations::instruments::equity::dcf_equity::{
    DiscountedCashFlow, EquityBridge, TerminalValueSpec, ValuationDiscounts,
};
use finstack_valuations::instruments::{Attributes, Instrument};
use serde_json::json;

/// Corporate valuation result containing DCF outputs.
///
/// Monetary outputs are returned in the model currency inferred from
/// `FinancialModelSpec::meta["currency"]`. Ratios such as
/// `equity_value_per_share` are plain scalars.
#[derive(Debug, Clone)]
pub struct CorporateValuationResult {
    /// Equity value (EV - Net Debt, after discounts)
    pub equity_value: Money,
    /// Enterprise value (PV of all cash flows + terminal value)
    pub enterprise_value: Money,
    /// Net debt (or effective bridge amount) used in calculation
    pub net_debt: Money,
    /// Terminal value (present value)
    pub terminal_value_pv: Money,
    /// Equity value per diluted share (if shares_outstanding was provided)
    pub equity_value_per_share: Option<f64>,
    /// Diluted share count (if shares_outstanding was provided)
    pub diluted_shares: Option<f64>,
    /// The underlying DCF instrument (for further analysis)
    pub dcf_instrument: Option<DiscountedCashFlow>,
}

/// Optional configuration for DCF valuation beyond the core WACC/terminal parameters.
///
/// All fields default to `None`/`false`.
///
/// Percentage-style inputs use decimal form, so `0.10` means `10%`.
#[derive(Debug, Clone)]
pub struct DcfOptions {
    /// Enable mid-year discounting convention (default: false).
    pub mid_year_convention: bool,
    /// Structured equity bridge (replaces flat net_debt when `Some`).
    pub equity_bridge: Option<EquityBridge>,
    /// Basic shares outstanding for per-share value.
    pub shares_outstanding: Option<f64>,
    /// Private company valuation discounts (DLOM, DLOC).
    ///
    /// Discounts apply after the DCF EV-to-equity bridge. The reported DCF
    /// enterprise value remains pre-discount and will not reconcile to
    /// discounted equity value plus net debt when discounts are present.
    pub valuation_discounts: Option<ValuationDiscounts>,
    /// WACC sensitivity bump, in decimal (default `0.01` = ±100 bp).
    ///
    /// The down-shock is clamped so the terminal-value denominator
    /// `wacc - g` cannot collapse (the resulting EV would blow up and
    /// obscure the true sensitivity). See
    /// [`DcfOptions::wacc_denominator_epsilon`].
    pub wacc_sensitivity_bump: f64,
    /// Minimum spread between WACC-down and the Gordon / H-Model growth
    /// rate in the terminal-value formula (default `0.005` = 50 bp).
    ///
    /// When `wacc - bump < growth + epsilon`, the down-shock is clamped
    /// to `growth + epsilon` so the `1/(wacc - g)` denominator stays
    /// well defined. The clamp is reported in the trace so the caller
    /// knows the bump was shortened.
    pub wacc_denominator_epsilon: f64,
    /// Exit-multiple sensitivity bump (default: `ExitMultipleBump::Absolute(1.0)`).
    ///
    /// Use `ExitMultipleBump::Relative` for a proportional shock,
    /// e.g. `Relative(0.10)` for ±10% of the base multiple. Absolute
    /// bumps are clamped at zero on the downside.
    pub exit_multiple_bump: ExitMultipleBump,
    /// Explicit discount curve id stamped on the DCF instrument (default:
    /// `None`).
    ///
    /// The curve is used for *risk attribution only* — all DCF components
    /// (explicit flows, terminal value, equity) always discount at the
    /// WACC on a single consistent basis. When `None`, a model-scoped
    /// placeholder id (`"{model_id}-DCF-WACC"`) is used instead of the
    /// conventional `"{CCY}-DISCOUNT"` name, so a market context that
    /// happens to contain a curve with the conventional name cannot be
    /// mistaken for the discounting basis.
    pub discount_curve_id: Option<CurveId>,
}

impl Default for DcfOptions {
    fn default() -> Self {
        Self {
            mid_year_convention: false,
            equity_bridge: None,
            shares_outstanding: None,
            valuation_discounts: None,
            wacc_sensitivity_bump: 0.01,
            wacc_denominator_epsilon: 0.005,
            exit_multiple_bump: ExitMultipleBump::default(),
            discount_curve_id: None,
        }
    }
}

/// Exit-multiple sensitivity shock shape.
///
/// Absolute bumps are in multiple-turn units (e.g. `Absolute(1.0)` is
/// ±1.0x). Relative bumps are decimal fractions of the base multiple
/// (e.g. `Relative(0.10)` is ±10%).
#[derive(Debug, Clone, Copy)]
pub enum ExitMultipleBump {
    /// Absolute bump in turns of the multiple.
    Absolute(f64),
    /// Proportional bump in decimal (0.10 = ±10%).
    Relative(f64),
}

impl Default for ExitMultipleBump {
    fn default() -> Self {
        ExitMultipleBump::Absolute(1.0)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DcfEvalContext<'a> {
    pub(crate) net_debt_override: Option<f64>,
    pub(crate) options: &'a DcfOptions,
    pub(crate) market: Option<&'a MarketContext>,
}

/// Evaluate a financial model using DCF methodology with optional market context.
///
/// Accepts a [`MarketContext`] for curve-based discounting when instruments
/// reference discount curves.
///
/// `wacc` and any growth rates embedded in `terminal_value` must be provided as
/// decimal fractions. Cash flows are sourced from the model's non-actual
/// periods and anchored to the first forecast boundary when historical actuals
/// are present.
///
/// # Arguments
///
/// * `model` - Statement model containing forecast periods plus a currency in
///   metadata
/// * `wacc` - Discount rate in decimal form (`0.10` means `10%`)
/// * `terminal_value` - Terminal-value convention applied after the explicit
///   forecast period
/// * `ufcf_node` - Node id containing unlevered free cash flow for forecast
///   periods
/// * `net_debt_override` - Optional flat net-debt amount used instead of the
///   model-derived bridge
/// * `options` - Mid-year, bridge, share-count, and discount configuration
/// * `market` - Optional market context used when the DCF instrument references
///   discount curves
///
/// # Returns
///
/// Returns [`CorporateValuationResult`] containing enterprise value, equity
/// value, the bridge inputs used in the calculation, and optional per-share
/// outputs.
///
/// # Errors
///
/// Returns an error if the model cannot be evaluated, if `ufcf_node` has no
/// forecast cash flows, if the model currency cannot be inferred, or if the
/// terminal-value assumptions are internally inconsistent.
///
/// # Examples
///
/// ```ignore
/// use finstack_statements_analytics::analysis::{evaluate_dcf_with_market, DcfOptions};
/// use finstack_statements::builder::ModelBuilder;
/// use finstack_statements::types::AmountOrScalar;
/// use finstack_core::dates::PeriodId;
/// use finstack_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("acme")
///     .periods("2025Q1..Q4", Some("2025Q1"))?
///     .value(
///         "ufcf",
///         &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1_000_000.0))],
///     )
///     .with_meta("currency", serde_json::json!("USD"))
///     .build()?;
///
/// let result = evaluate_dcf_with_market(
///     &model,
///     0.10,
///     TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
///     "ufcf",
///     None,
///     &DcfOptions::default(),
///     None,
/// )?;
///
/// assert_eq!(result.enterprise_value.currency().to_string(), "USD");
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Discounting and terminal-value context: `docs/REFERENCES.md#hull-options-futures`
pub fn evaluate_dcf_with_market(
    model: &FinancialModelSpec,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    options: &DcfOptions,
    market: Option<&MarketContext>,
) -> Result<CorporateValuationResult> {
    let (result, _trace) = evaluate_dcf_impl(
        model,
        wacc,
        terminal_value,
        ufcf_node,
        DcfEvalContext {
            net_debt_override,
            options,
            market,
        },
    )?;
    Ok(result)
}

/// Core implementation shared by all `evaluate_dcf*` entry points.
fn evaluate_dcf_impl(
    model: &FinancialModelSpec,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: &str,
    context: DcfEvalContext<'_>,
) -> Result<(CorporateValuationResult, ExplanationTrace)> {
    // Create evaluator and evaluate the model. Market context is applied later
    // during DCF discounting; passing market here with `as_of = None` is a no-op
    // in the evaluator, so we use plain `evaluate` for clarity.
    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(model)?;

    evaluate_dcf_from_results_impl(model, &results, wacc, terminal_value, ufcf_node, context)
}

pub(crate) fn evaluate_dcf_from_results_impl(
    model: &FinancialModelSpec,
    results: &StatementResult,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: &str,
    context: DcfEvalContext<'_>,
) -> Result<(CorporateValuationResult, ExplanationTrace)> {
    let first_forecast_period = model.periods.iter().find(|period| !period.is_actual);
    let last_actual_period = model.periods.iter().rfind(|period| period.is_actual);

    // Initialize explanation trace
    let mut trace = ExplanationTrace::new("corporate_dcf");

    // Extract UFCF series from results
    let mut flows = Vec::new();
    let currency = extract_currency_from_model(model)?;

    for period in &model.periods {
        if period.is_actual {
            continue;
        }
        if let Some(ufcf_value) = results.get(ufcf_node, &period.id) {
            // Use the last inclusive day of the period.  Periods use
            // half-open semantics [start, end), so `end` is the first
            // day of the *next* period.  Subtracting one day gives the
            // correct economic period-end for discounting.
            let date = period.end - time::Duration::days(1);
            flows.push((date, ufcf_value));

            // Record UFCF contribution in the explanation trace
            trace.push(
                TraceEntry::ComputationStep {
                    name: "ufcf_period".to_string(),
                    description: "Unlevered free cash flow by period".to_string(),
                    metadata: Some(json!({
                        "period_id": format!("{:?}", period.id),
                        "ufcf": ufcf_value,
                        "date": date.to_string(),
                    })),
                },
                None,
            );
        }
    }

    if flows.is_empty() {
        return Err(finstack_statements::error::Error::Eval(format!(
            "No UFCF values found for node '{}'",
            ufcf_node
        )));
    }

    // Validate terminal value constraints. Guards are written fail-closed
    // (negated comparisons) so NaN parameters error instead of silently
    // producing NaN valuations.
    use std::cmp::Ordering;
    match &terminal_value {
        TerminalValueSpec::GordonGrowth { growth_rate }
            if growth_rate.partial_cmp(&wacc) != Some(Ordering::Less) =>
        {
            return Err(finstack_statements::error::Error::Eval(format!(
                "Gordon Growth terminal value requires growth_rate ({:.4}) < WACC ({:.4}). \
                 A growth rate >= WACC produces an infinite terminal value.",
                growth_rate, wacc
            )));
        }
        TerminalValueSpec::HModel {
            high_growth_rate,
            stable_growth_rate,
            half_life_years,
        } => {
            if stable_growth_rate.partial_cmp(&wacc) != Some(Ordering::Less) {
                return Err(finstack_statements::error::Error::Eval(format!(
                    "H-Model terminal value requires stable_growth_rate ({:.4}) < WACC ({:.4}).",
                    stable_growth_rate, wacc
                )));
            }
            if !matches!(
                high_growth_rate.partial_cmp(stable_growth_rate),
                Some(Ordering::Greater | Ordering::Equal)
            ) {
                return Err(finstack_statements::error::Error::Eval(format!(
                    "H-Model requires high_growth_rate ({:.4}) >= stable_growth_rate ({:.4}).",
                    high_growth_rate, stable_growth_rate
                )));
            }
            if half_life_years.partial_cmp(&0.0) != Some(Ordering::Greater) {
                return Err(finstack_statements::error::Error::Eval(format!(
                    "H-Model requires half_life_years > 0, got {:.4}.",
                    half_life_years
                )));
            }
        }
        _ => {}
    }

    // Growth-perpetuity terminal values (Gordon, H-Model) capitalize an
    // *annual* flow with annual WACC/g. When the model's period grid is
    // sub-annual, annualize the terminal flow as the trailing sum of the
    // final year's period flows (standard trailing-twelve-month
    // convention; Koller et al., Damodaran). If fewer than a full year of
    // forecast flows exists, the trailing sum is scaled up pro-rata.
    // Annual grids pass the last flow through unchanged.
    let terminal_flow_override = match &terminal_value {
        TerminalValueSpec::GordonGrowth { .. } | TerminalValueSpec::HModel { .. } => {
            let periods_per_year = model
                .periods
                .iter()
                .rfind(|period| !period.is_actual)
                .map(|period| usize::from(period.id.periods_per_year()))
                .unwrap_or(1);
            if periods_per_year > 1 {
                let trailing = flows.len().min(periods_per_year);
                let trailing_sum: f64 = flows
                    .iter()
                    .rev()
                    .take(trailing)
                    .map(|(_, amount)| amount)
                    .sum();
                let annualized = trailing_sum * (periods_per_year as f64 / trailing as f64);
                trace.push(
                    TraceEntry::ComputationStep {
                        name: "terminal_flow_annualization".to_string(),
                        description: "Trailing-year annualization of the terminal flow for a \
                                      growth-perpetuity terminal value on a sub-annual grid"
                            .to_string(),
                        metadata: Some(json!({
                            "periods_per_year": periods_per_year,
                            "trailing_periods_used": trailing,
                            "trailing_sum": trailing_sum,
                            "annualized_terminal_flow": annualized,
                        })),
                    },
                    None,
                );
                Some(annualized)
            } else {
                None
            }
        }
        TerminalValueSpec::ExitMultiple { .. } => None,
    };

    // Determine net debt
    let net_debt_period = last_actual_period
        .map(|period| period.id)
        .or_else(|| first_forecast_period.map(|period| period.id));
    let net_debt = if let Some(override_val) = context.net_debt_override {
        override_val
    } else {
        calculate_net_debt_from_model(model, results, net_debt_period)?
    };

    // Determine valuation date. When historical actuals exist, anchor the DCF to the
    // first forecast boundary so explicit cashflows and bridge values share the same cut.
    let valuation_date = if let Some(forecast_period) = first_forecast_period {
        forecast_period.start
    } else {
        model
            .periods
            .first()
            .ok_or_else(|| finstack_statements::error::Error::Eval("Model has no periods".into()))?
            .start
    };

    // Create DCF instrument.
    //
    // The discount curve id is risk-attribution metadata only: the DCF
    // pricer always discounts every component (explicit flows, terminal
    // value, equity) at the WACC. The conventional "{CCY}-DISCOUNT" name
    // is no longer synthesized by default — callers opt in via
    // `DcfOptions::discount_curve_id`; otherwise a model-scoped
    // placeholder keeps the instrument id-complete without colliding
    // with any market curve.
    let discount_curve_id = context
        .options
        .discount_curve_id
        .clone()
        .unwrap_or_else(|| CurveId::new(format!("{}-DCF-WACC", model.id)));
    let mut builder = DiscountedCashFlow::builder()
        .id(InstrumentId::new(format!("{}-DCF", model.id)))
        .currency(currency)
        .flows(flows)
        .wacc(wacc)
        .terminal_value(terminal_value)
        .net_debt(net_debt)
        .valuation_date(valuation_date)
        .discount_curve_id(discount_curve_id)
        .mid_year_convention(context.options.mid_year_convention)
        .terminal_flow_override_opt(terminal_flow_override)
        .attributes(Attributes::new());

    if let Some(ref bridge) = context.options.equity_bridge {
        builder = builder.equity_bridge(bridge.clone());
    }
    if let Some(shares) = context.options.shares_outstanding {
        builder = builder.shares_outstanding(shares);
    }
    if let Some(ref discounts) = context.options.valuation_discounts {
        builder = builder.valuation_discounts(discounts.clone());
    }

    let dcf = builder
        .build()
        .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;

    // Calculate valuation
    let default_market = MarketContext::default();
    let market_ref = context.market.unwrap_or(&default_market);
    let equity_value = dcf
        .value(market_ref, valuation_date)
        .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;

    // Calculate components for result
    let pv_explicit = dcf.calculate_pv_explicit_flows();
    let tv = dcf
        .calculate_terminal_value()
        .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
    let pv_terminal = dcf
        .discount_terminal_value(tv)
        .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
    let enterprise_value = pv_explicit + pv_terminal;

    // Record base valuation in the explanation trace
    trace.push(
        TraceEntry::ComputationStep {
            name: "dcf_base_valuation".to_string(),
            description: "Base DCF valuation (enterprise and equity value)".to_string(),
            metadata: Some(json!({
                "wacc": wacc,
                "pv_explicit_flows": pv_explicit,
                "terminal_value": tv,
                "pv_terminal_value": pv_terminal,
                "enterprise_value": enterprise_value,
                "net_debt": net_debt,
                "equity_value": equity_value.amount(),
            })),
        },
        None,
    );

    // Sensitivity of EV to WACC (configurable bump, default +/- 100 bps).
    // Compute EV directly from PV components (not from equity + bridge) so that
    // the result is independent of valuation discounts (DLOM/DLOC).
    //
    // The down-shock is clamped against the terminal-value growth rate
    // so `1/(wacc - g)` does not explode (or go negative) when the bump
    // lands at or below `g`. The effective down-shock is reported in
    // the trace so the caller sees when the bump was shortened.
    let wacc_bump = context.options.wacc_sensitivity_bump.abs();
    let wacc_epsilon = context.options.wacc_denominator_epsilon.max(0.0);
    let growth_floor: f64 = match dcf.terminal_value {
        TerminalValueSpec::GordonGrowth { growth_rate } => growth_rate,
        TerminalValueSpec::HModel {
            stable_growth_rate, ..
        } => stable_growth_rate,
        TerminalValueSpec::ExitMultiple { .. } => f64::NEG_INFINITY,
    };
    let wacc_up = wacc + wacc_bump;
    let wacc_down_raw = wacc - wacc_bump;
    let wacc_down_floor = (growth_floor + wacc_epsilon).max(wacc_epsilon);
    let wacc_down = wacc_down_raw.max(wacc_down_floor);
    let wacc_down_clamped = (wacc_down - wacc_down_raw).abs() > 1e-12;

    let ev_wacc_up = {
        let mut dcf_up = Clone::clone(&dcf);
        dcf_up.wacc = wacc_up;
        let pv_exp = dcf_up.calculate_pv_explicit_flows();
        let tv_up = dcf_up
            .calculate_terminal_value()
            .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
        let pv_tv = dcf_up
            .discount_terminal_value(tv_up)
            .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
        pv_exp + pv_tv
    };

    let ev_wacc_down = {
        let mut dcf_down = Clone::clone(&dcf);
        dcf_down.wacc = wacc_down;
        let pv_exp = dcf_down.calculate_pv_explicit_flows();
        let tv_down = dcf_down
            .calculate_terminal_value()
            .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
        let pv_tv = dcf_down
            .discount_terminal_value(tv_down)
            .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
        pv_exp + pv_tv
    };

    trace.push(
        TraceEntry::ComputationStep {
            name: "wacc_sensitivity".to_string(),
            description: "Sensitivity of enterprise value to WACC".to_string(),
            metadata: Some(json!({
                "wacc": wacc,
                "ev_base": enterprise_value,
                "wacc_up_bp": wacc_bump * 10_000.0,
                "wacc_up": wacc_up,
                "ev_wacc_up": ev_wacc_up,
                "wacc_down_bp": wacc_bump * 10_000.0,
                "wacc_down": wacc_down,
                "wacc_down_clamped": wacc_down_clamped,
                "wacc_down_growth_floor": growth_floor,
                "ev_wacc_down": ev_wacc_down,
            })),
        },
        None,
    );

    // Sensitivity of EV to Exit Multiple (if applicable).
    if let TerminalValueSpec::ExitMultiple {
        terminal_metric,
        multiple,
    } = dcf.terminal_value
    {
        let (bump_up, bump_down) = match context.options.exit_multiple_bump {
            ExitMultipleBump::Absolute(b) => (b.abs(), b.abs()),
            ExitMultipleBump::Relative(r) => {
                let shock = multiple.abs() * r.abs();
                (shock, shock)
            }
        };
        let multiple_up = multiple + bump_up;
        let multiple_down = (multiple - bump_down).max(0.0);

        let mut dcf_up = dcf.clone();
        dcf_up.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric,
            multiple: multiple_up,
        };
        let ev_up = {
            let pv_explicit_up = dcf_up.calculate_pv_explicit_flows();
            let tv_up = dcf_up
                .calculate_terminal_value()
                .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
            let pv_tv_up = dcf_up
                .discount_terminal_value(tv_up)
                .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
            pv_explicit_up + pv_tv_up
        };

        let mut dcf_down = Clone::clone(&dcf);
        dcf_down.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric,
            multiple: multiple_down,
        };
        let ev_down = {
            let pv_explicit_down = dcf_down.calculate_pv_explicit_flows();
            let tv_down = dcf_down
                .calculate_terminal_value()
                .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
            let pv_tv_down = dcf_down
                .discount_terminal_value(tv_down)
                .map_err(|e| finstack_statements::error::Error::Eval(e.to_string()))?;
            pv_explicit_down + pv_tv_down
        };

        trace.push(
            TraceEntry::ComputationStep {
                name: "exit_multiple_sensitivity".to_string(),
                description: "Sensitivity of enterprise value to terminal exit multiple"
                    .to_string(),
                metadata: Some(json!({
                    "terminal_metric": terminal_metric,
                    "multiple_base": multiple,
                    "ev_base": enterprise_value,
                    "multiple_up": multiple_up,
                    "ev_multiple_up": ev_up,
                    "multiple_down": multiple_down,
                    "ev_multiple_down": ev_down,
                    "bump_shape": match context.options.exit_multiple_bump {
                        ExitMultipleBump::Absolute(b) => format!("absolute({:.4})", b),
                        ExitMultipleBump::Relative(r) => format!("relative({:.4})", r),
                    },
                })),
            },
            None,
        );
    }

    // Compute per-share metrics if shares outstanding is set
    let equity_val = equity_value.amount();
    let equity_value_per_share = dcf.equity_value_per_share(equity_val);
    let diluted_shares = dcf.diluted_shares(equity_val);

    Ok((
        CorporateValuationResult {
            equity_value,
            enterprise_value: Money::new(enterprise_value, currency),
            net_debt: Money::new(dcf.effective_net_debt(), currency),
            terminal_value_pv: Money::new(pv_terminal, currency),
            equity_value_per_share,
            diluted_shares,
            dcf_instrument: Some(dcf),
        },
        trace,
    ))
}

/// Extract currency from the model (assumes uniform currency).
///
/// Checks model metadata for a `"currency"` key (string ISO code) and
/// returns the parsed [`Currency`]. Returns an error if the key is
/// missing or not a string: we never silently default to USD because
/// that corrupts per-share and enterprise-value reporting for non-USD
/// models. Callers that want a USD default must set `meta["currency"]`
/// explicitly on the model.
fn extract_currency_from_model(model: &FinancialModelSpec) -> Result<Currency> {
    if let Some(currency_meta) = model.meta.get("currency") {
        if let Some(currency_str) = currency_meta.as_str() {
            return currency_str.parse::<Currency>().map_err(|_| {
                finstack_statements::error::Error::Eval(format!(
                    "Invalid currency: {}",
                    currency_str
                ))
            });
        }
        return Err(finstack_statements::error::Error::Eval(
            "Model metadata key 'currency' must be a string ISO currency code".into(),
        ));
    }

    Err(finstack_statements::error::Error::Eval(format!(
        "Model '{}' is missing required metadata key 'currency'. \
         Set model.meta[\"currency\"] to an ISO currency code such as 'USD'.",
        model.id
    )))
}

/// Calculate net debt from the model.
///
/// Net Debt = Total Debt - Cash
///
/// This function attempts to find debt and cash nodes in the model results.
fn calculate_net_debt_from_model(
    model: &FinancialModelSpec,
    results: &finstack_statements::evaluator::StatementResult,
    balance_sheet_period: Option<finstack_core::dates::PeriodId>,
) -> Result<f64> {
    // Use the valuation boundary balance sheet when available; otherwise fall back
    // to the latest model period for fully forecast-only models.
    let selected_period_id = if let Some(period_id) = balance_sheet_period {
        period_id
    } else {
        model
            .periods
            .last()
            .ok_or_else(|| finstack_statements::error::Error::Eval("Model has no periods".into()))?
            .id
    };

    // Try to find total debt — warn if not found so users know the value is assumed
    let total_debt = results
        .get("total_debt", &selected_period_id)
        .or_else(|| results.get("debt", &selected_period_id));

    let cash = results
        .get("cash", &selected_period_id)
        .or_else(|| results.get("cash_and_equivalents", &selected_period_id));

    let total_debt = total_debt.ok_or_else(|| {
        finstack_statements::error::Error::Eval(format!(
            "Net debt calculation requires a 'total_debt' or 'debt' node at period {}. \
             Provide the balance-sheet node or use net_debt_override.",
            selected_period_id
        ))
    })?;
    let cash = cash.ok_or_else(|| {
        finstack_statements::error::Error::Eval(format!(
            "Net debt calculation requires a 'cash' or 'cash_and_equivalents' node at period {}. \
             Provide the balance-sheet node or use net_debt_override.",
            selected_period_id
        ))
    })?;

    Ok(total_debt - cash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::PeriodId;
    use finstack_statements::builder::ModelBuilder;
    use finstack_statements::types::AmountOrScalar;

    #[test]
    fn evaluate_dcf_requires_explicit_currency_metadata() {
        let model = ModelBuilder::new("dcf-missing-currency")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "ufcf",
                &[
                    (
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(100_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2),
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .value(
                "total_debt",
                &[(PeriodId::quarter(2025, 2), AmountOrScalar::scalar(50_000.0))],
            )
            .value(
                "cash",
                &[(PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10_000.0))],
            )
            .build()
            .expect("valid model");

        let result = evaluate_dcf_with_market(
            &model,
            0.10,
            TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
            "ufcf",
            None,
            &DcfOptions::default(),
            None,
        );
        assert!(result.is_err(), "currency metadata must be explicit");
    }

    #[test]
    fn evaluate_dcf_requires_balance_sheet_inputs_without_override() {
        let model = ModelBuilder::new("dcf-missing-balance-sheet")
            .periods("2025Q1..Q2", None)
            .expect("valid periods")
            .value(
                "ufcf",
                &[
                    (
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(100_000.0),
                    ),
                    (
                        PeriodId::quarter(2025, 2),
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .with_meta("currency", serde_json::json!("USD"))
            .build()
            .expect("valid model");

        let result = evaluate_dcf_with_market(
            &model,
            0.10,
            TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
            "ufcf",
            None,
            &DcfOptions::default(),
            None,
        );
        assert!(
            result.is_err(),
            "missing debt and cash inputs must fail without an override"
        );
    }
}
