//! Corporate DCF evaluation engine and model-extraction helpers.

use super::types::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
use finstack_quant_statements::types::{FinancialModelSpec, NodeValueType};
use finstack_quant_statements::{Error, Result};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    DiscountedCashFlow, EquityBridge, ForecastTiming, TerminalValueSpec,
};

/// Evaluate a financial model using DCF methodology with optional market context.
///
/// `as_of` is the DCF valuation date and, when `market` is present, the
/// statement-evaluation visibility and curve date. DCF discounting remains
/// WACC-based; `market` is used only for statement-level instruments.
///
/// - `Some(market)` + `Some(as_of)` evaluates the model point-in-time and
///   discounts future UFCF from the same date.
/// - `Some(market)` + `None` is an error.
/// - `None` market uses plain [`Evaluator::evaluate`]; a supplied `as_of`
///   still anchors DCF discounting.
/// - `None` `as_of` uses the first forecast-period start as the valuation date.
///
/// `wacc` and terminal growth rates use decimal fractions. Cashflows dated on
/// or before the valuation date are excluded.
/// Growth terminals require a complete contiguous calendar year of monetary
/// UFCF ending at the last forecast boundary. The window can
/// include actual periods. Partial periods are not prorated; weekly boundaries
/// that do not align with the calendar-year start are rejected.
///
/// # Arguments
///
/// * `model` - Statement model containing forecast periods plus a currency in
///   metadata
/// * `inputs` - Headline assumptions: `wacc` (decimal, `0.10` means `10%`),
///   the `terminal_value` convention applied after the explicit forecast
///   period, the `ufcf_node` holding unlevered free cash flow for forecast
///   periods, and an optional flat `net_debt_override` used instead of the
///   model-derived bridge
/// * `options` - Mid-year, bridge, share-count, exit-multiple node, and
///   discount configuration
/// * `market` - Optional market context for statement-level curve lookups
/// * `as_of` - Optional DCF valuation date and statement visibility cutoff;
///   required when `market` is `Some`, otherwise defaults to the first forecast
///   boundary. As a visibility cutoff it hides each actual whose availability
///   date (by default the period's exclusive end, the first day of the next
///   period) is after it, so a quarter-end date such as `2025-03-31` hides
///   2025Q1's actuals: pass the next period's first day (`2025-04-01`) or set
///   the nodes' `availability_dates`. Only forecast periods ending after
///   `as_of` are discounted
///
/// # Returns
///
/// Returns [`CorporateValuationResult`] containing enterprise value, equity
/// value, the bridge inputs used in the calculation, and optional per-share
/// outputs.
///
/// # Errors
///
/// Returns an error if `market` is provided without `as_of`, if the model
/// cannot be evaluated (including a value node whose actual `as_of` hides),
/// if `inputs.ufcf_node` has no forecast cash flows ending after `as_of`, if
/// the model currency cannot be inferred, if monetary nodes use another currency
/// or no opening balance is available without a net-debt override, if an
/// exit-multiple metric node is missing or non-finite, or if the
/// terminal-value assumptions are internally inconsistent. Growth terminals
/// also reject missing, overlapping, or partial trailing-year UFCF history.
///
/// # Examples
///
/// ```
/// use finstack_quant_statements_analytics::analysis::{evaluate_dcf, DcfInputs, DcfOptions};
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_core::{currency::Currency, dates::PeriodId, money::Money};
/// use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = {
///     let mut model_builder = ModelBuilder::new("acme");
///     model_builder.periods("2025Q1..Q4", Some("2025Q1"))?;
///     model_builder.value_money(
///         "ufcf",
///         &[
///             (PeriodId::quarter(2025, 1).expect("valid period fixture"), Money::from((1_000_000_i64, Currency::USD))),
///             (PeriodId::quarter(2025, 2).expect("valid period fixture"), Money::from((1_050_000_i64, Currency::USD))),
///             (PeriodId::quarter(2025, 3).expect("valid period fixture"), Money::from((1_100_000_i64, Currency::USD))),
///             (PeriodId::quarter(2025, 4).expect("valid period fixture"), Money::from((1_150_000_i64, Currency::USD))),
///         ],
///     );
///     model_builder.with_meta("currency", serde_json::json!("USD"));
///     model_builder.build()?
/// };
///
/// let inputs = DcfInputs {
///     wacc: 0.10,
///     terminal_value: TerminalValueSpec::GordonGrowth { stable_growth_rate: 0.02 },
///     ufcf_node: "ufcf".to_string(),
///     net_debt_override: Some(4_000_000.0),
/// };
/// let result = evaluate_dcf(&model, &inputs, &DcfOptions::default(), None, None)?;
///
/// assert_eq!(result.enterprise_value.currency().to_string(), "USD");
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Discounting and terminal-value context: `docs/REFERENCES.md#hull-options-futures`
pub fn evaluate_dcf(
    model: &FinancialModelSpec,
    inputs: &DcfInputs,
    options: &DcfOptions,
    market: Option<&MarketContext>,
    as_of: Option<Date>,
) -> Result<CorporateValuationResult> {
    // Market is for statement evaluation (curve-dependent CS nodes), not
    // DCF discounting. Require as_of whenever a market is supplied so
    // curves cannot be silently dropped.
    let context = DcfEvalContext {
        net_debt_override: inputs.net_debt_override,
        options,
        projection: None,
    };
    let mut evaluator = Evaluator::new();
    let results = match (market, as_of) {
        (Some(market), Some(as_of)) => evaluator.evaluate_with_market(
            model,
            market,
            as_of,
            finstack_quant_statements::evaluator::ObservationCutoff::FullSnapshot,
        )?,
        (Some(_), None) => {
            return Err(finstack_quant_statements::Error::Eval(
                "evaluate_dcf requires as_of when a market context is provided; \
                 market is used for statement evaluation, not DCF discounting"
                    .into(),
            ));
        }
        (None, _) => evaluator.evaluate(model)?,
    };

    evaluate_dcf_from_results_impl(
        model,
        &results,
        inputs.wacc,
        inputs.terminal_value.clone(),
        &inputs.ufcf_node,
        context,
        as_of,
    )
}

/// Price a point-in-time corporate DCF on a verified evaluated statement run.
///
/// # Arguments
/// * `model` - The unchanged statement model, including its identified source snapshot.
/// * `results` - The precomputed statement run at the exact requested currency, cut and valuation date.
/// * `request` - The authored WACC, cashflow node, terminal basis and equity bridge.
///
/// # Errors
/// Rejects mismatched run provenance, missing or unavailable observations,
/// mixed currencies, incomplete selected periods, or invalid DCF assumptions.
pub fn evaluate_dcf_from_results(
    model: &FinancialModelSpec,
    results: &StatementResult,
    request: &CorporateDcfRequest,
) -> Result<CorporateValuationResult> {
    request.cut.check_result(model, results)?;
    let inputs = &request.inputs;
    inputs.validate()?;
    evaluate_dcf_from_results_impl(
        model,
        results,
        inputs.wacc,
        inputs.terminal_value.clone(),
        &inputs.ufcf_node,
        DcfEvalContext {
            net_debt_override: inputs.net_debt_override,
            options: &request.options,
            projection: Some(&request.projection),
        },
        Some(request.cut.as_of),
    )
}

/// Evaluate a statement model at its elected observation cutoff, then price its DCF.
///
/// # Arguments
/// * `model` - Statement model with a source snapshot and observation availability dates.
/// * `request` - The authored interest rate, currency, forecast and terminal terms.
/// * `market` - Market input for any capital-structure-linked statement nodes.
///   Omit when valuation only needs the statements and authored assumptions.
///
/// # Errors
/// Rejects missing source or cutoff evidence, an invalid capital structure,
/// a missing cash flow, a currency mismatch or invalid valuation assumptions.
pub fn evaluate_corporate_dcf(
    model: &FinancialModelSpec,
    request: &CorporateDcfRequest,
    market: Option<&MarketContext>,
) -> Result<CorporateValuationResult> {
    let results = request.cut.evaluate(model, market)?;
    evaluate_dcf_from_results(model, &results, request)
}

pub(crate) fn evaluate_dcf_from_results_impl(
    model: &FinancialModelSpec,
    results: &StatementResult,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: &str,
    context: DcfEvalContext<'_>,
    valuation_date: Option<Date>,
) -> Result<CorporateValuationResult> {
    let first_forecast_period = model.periods.iter().find(|period| !period.is_actual);
    let fallback_valuation_date = first_forecast_period
        .or_else(|| model.periods.first())
        .map(|period| period.start)
        .ok_or_else(|| finstack_quant_statements::Error::Eval("Model has no periods".into()))?;
    let valuation_date = valuation_date.unwrap_or(fallback_valuation_date);

    let currency = extract_currency_from_model(model)?;
    match results.node_value_types.get(ufcf_node) {
        Some(NodeValueType::Monetary {
            currency: node_currency,
        }) if *node_currency == currency => {}
        Some(NodeValueType::Monetary {
            currency: node_currency,
        }) => {
            return Err(finstack_quant_statements::Error::Eval(format!(
                "UFCF node '{ufcf_node}' uses {node_currency}, but the DCF model currency is \
                 {currency}"
            )));
        }
        _ => {
            return Err(finstack_quant_statements::Error::Eval(format!(
                "UFCF node '{ufcf_node}' must be monetary in {currency}; scalar statement \
                 nodes cannot be used as DCF cashflows"
            )));
        }
    }

    let mut flows = Vec::new();
    let mut flow_periods = Vec::new();
    let mut starts = Vec::new();
    for period in &model.periods {
        if period.is_actual {
            continue;
        }
        let date = period.end - time::Duration::days(1);
        if date <= valuation_date {
            continue;
        }
        let (node, start) = match context.projection {
            Some(projection) if period.start < valuation_date => {
                let node = projection.forward_remainder_node.as_deref().ok_or_else(|| {
                    Error::invalid_input(format!(
                        "Period {} crosses the DCF valuation date and requires a modeled forward remainder", period.id
                    ))
                })?;
                if node == ufcf_node {
                    return Err(Error::invalid_input(
                        "DCF forward remainder must name its own modeled amount",
                    ));
                }
                (node, valuation_date)
            }
            _ => (ufcf_node, period.start),
        };
        flows.push((
            date,
            monetary_node_money(results, node, &period.id, currency)?,
        ));
        flow_periods.push(period.id);
        starts.push(start);
    }

    if flows.is_empty() {
        return Err(finstack_quant_statements::Error::Eval(format!(
            "No UFCF values found for node '{ufcf_node}' in forecast periods ending after the \
             valuation date {valuation_date}"
        )));
    }

    let terminal_value = resolve_exit_multiple_metric(
        model,
        results,
        terminal_value,
        context.options.exit_multiple_metric_node.as_deref(),
    )?;

    let growth_ceiling = match context.projection {
        Some(projection) => projection.stable_growth_ceiling,
        None => Some(context.options.max_stable_growth_rate),
    };
    if let Some(ceiling) = growth_ceiling {
        if !ceiling.is_finite() {
            return Err(Error::invalid_input(
                "DCF stable-growth ceiling must be finite",
            ));
        }
        let stable = match terminal_value {
            TerminalValueSpec::GordonGrowth { stable_growth_rate }
            | TerminalValueSpec::HModel {
                stable_growth_rate, ..
            } => Some(stable_growth_rate),
            TerminalValueSpec::ExitMultiple { .. } => None,
        };
        if stable.is_some_and(|rate| rate > ceiling) {
            return Err(Error::invalid_input(format!(
                "DCF stable growth exceeds the elected ceiling ({ceiling})"
            )));
        }
    }

    // Growth-perpetuity terminal values (Gordon, H-Model) capitalize an
    // *annual* flow with annual WACC/g. Require a complete contiguous trailing
    // calendar year, including for explicitly dated annual stub periods.
    // Sparse observations or partial years cannot be substituted for that
    // history without changing the terminal-flow basis.
    let projection_end = flows.last().map(|(end, _)| *end).ok_or_else(|| {
        Error::eval("DCF requires a terminal horizon after at least one forecast receipt")
    })?;
    let annual_terminal_fcff = match context.projection {
        Some(projection) => projection.normalized_fcff(
            model,
            results,
            &terminal_value,
            ufcf_node,
            currency,
            projection_end,
        )?,
        None => match &terminal_value {
            TerminalValueSpec::GordonGrowth { .. } | TerminalValueSpec::HModel { .. } => {
                let last_forecast = model
                    .periods
                    .iter()
                    .rfind(|period| !period.is_actual)
                    .ok_or_else(|| {
                        Error::eval("Growth terminal value requires a forecast period")
                    })?;
                Some(trailing_year_metric(
                    model,
                    results,
                    ufcf_node,
                    last_forecast.end,
                    currency,
                )?)
            }
            TerminalValueSpec::ExitMultiple { .. } => None,
        },
    };

    // Determine net debt from the latest balance-sheet period ending on or
    // before the valuation date. This keeps the equity bridge on the same
    // temporal anchor as discounted cashflows.
    let net_debt_period = model
        .periods
        .iter()
        .filter(|period| period.end - time::Duration::days(1) <= valuation_date)
        .max_by_key(|period| period.end)
        .map(|period| period.id);
    // One EV-to-equity channel: an explicit bridge wins; otherwise a flat
    // net-debt override (debt only); otherwise debt and cash from the model.
    let equity_bridge = if let Some(bridge) = &context.options.equity_bridge {
        bridge.clone()
    } else if let Some(override_val) = context.net_debt_override {
        EquityBridge {
            total_debt: override_val,
            ..EquityBridge::default()
        }
    } else {
        equity_bridge_from_model(model, results, net_debt_period)?
    };

    let terminal_date = context
        .projection
        .map_or(projection_end, |projection| projection.terminal_date);
    let forecast_timing = if context.options.mid_year_convention {
        ForecastTiming::Midpoints {
            period_starts: starts,
        }
    } else {
        ForecastTiming::DatedReceipts
    };
    // The DCF pricer discounts every component (explicit flows, terminal
    // value, equity) at the WACC; it reads no market curve.
    let mut builder = DiscountedCashFlow::builder()
        .id(InstrumentId::new(format!("{}-DCF", model.id)))
        .currency(currency)
        .flows(flows)
        .wacc(wacc)
        .terminal_value(terminal_value)
        .equity_bridge(equity_bridge)
        .valuation_date(valuation_date)
        .forecast_timing(forecast_timing)
        .terminal_date(terminal_date)
        .normalized_annual_fcff_opt(annual_terminal_fcff)
        .attributes(Attributes::new());

    if let Some(shares) = context.options.shares_outstanding {
        builder = builder.shares_outstanding(shares);
    }
    if let Some(ref discounts) = context.options.valuation_discounts {
        builder = builder.valuation_discounts(discounts.clone());
    }

    let dcf = builder
        .build()
        .map_err(|e| finstack_quant_statements::Error::Eval(e.to_string()))?;

    let equity_value = dcf
        .value(&MarketContext::default(), valuation_date)
        .map_err(|e| finstack_quant_statements::Error::Eval(e.to_string()))?;

    let pv_explicit = dcf.calculate_pv_explicit_flows()?;
    let tv = dcf
        .calculate_terminal_value()
        .map_err(|e| finstack_quant_statements::Error::Eval(e.to_string()))?;
    let pv_terminal = dcf
        .discount_terminal_value(tv)
        .map_err(|e| finstack_quant_statements::Error::Eval(e.to_string()))?;
    let enterprise_value = pv_explicit + pv_terminal;
    let terminal_discount_years = dcf
        .terminal_discount_years()
        .map_err(|e| finstack_quant_statements::Error::Eval(e.to_string()))?;
    // Same expression per flow as `calculate_pv_explicit_flows`, so the
    // `present_value` column sums to `pv_explicit`.
    let periods = flow_periods
        .iter()
        .zip(&dcf.flows)
        .map(|(period_id, (date, amount))| {
            let discount_years = dcf.explicit_flow_discount_years(*date)?;
            let compounding = (1.0 + dcf.wacc).powf(discount_years);
            Ok(DcfPeriodRow {
                period_id: *period_id,
                date: *date,
                free_cash_flow: amount.amount(),
                discount_years,
                discount_factor: 1.0 / compounding,
                present_value: amount.amount() / compounding,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let equity_val = equity_value.amount();
    let equity_value_per_share = dcf.equity_value_per_share(equity_val);
    let diluted_shares = dcf.diluted_shares(equity_val);
    let net_debt = dcf.equity_bridge.total_debt - dcf.equity_bridge.cash;
    let other_equity_claims = dcf.equity_bridge.preferred_equity
        + dcf.equity_bridge.minority_interest
        - dcf.equity_bridge.non_operating_assets
        - dcf
            .equity_bridge
            .other_adjustments
            .iter()
            .map(|(_, value)| value)
            .sum::<f64>();
    let valuation_discount = if context.options.valuation_discounts.is_some() {
        enterprise_value - net_debt - other_equity_claims - equity_val
    } else {
        0.0
    };

    Ok(CorporateValuationResult {
        equity_value,
        enterprise_value: Money::new(enterprise_value, currency)?,
        net_debt: Money::new(net_debt, currency)?,
        other_equity_claims: Money::new(other_equity_claims, currency)?,
        valuation_discount: Money::new(valuation_discount, currency)?,
        terminal_value_pv: Money::new(pv_terminal, currency)?,
        equity_value_per_share,
        diluted_shares,
        wacc: Some(dcf.wacc),
        periods,
        pv_explicit: Some(Money::new(pv_explicit, currency)?),
        terminal_value: Some(Money::new(tv, currency)?),
        terminal_discount_years: Some(terminal_discount_years),
        equity_bridge: Some(dcf.equity_bridge.clone()),
        dcf_instrument: Some(dcf),
    })
}

/// Extract currency from the model (assumes uniform currency).
///
/// Checks model metadata for a `"currency"` key (string ISO code) and
/// returns the parsed [`Currency`]. Returns an error if the key is
/// missing or not a string: we never silently default to USD because
/// that corrupts per-share and enterprise-value reporting for non-USD
/// models. Callers that want a USD default must set `meta["currency"]`
/// explicitly on the model.
pub(crate) fn extract_currency_from_model(model: &FinancialModelSpec) -> Result<Currency> {
    if let Some(currency_meta) = model.meta.get("currency") {
        if let Some(currency_str) = currency_meta.as_str() {
            return currency_str.parse::<Currency>().map_err(|_| {
                finstack_quant_statements::Error::Eval(format!(
                    "Invalid currency: {}",
                    currency_str
                ))
            });
        }
        return Err(finstack_quant_statements::Error::Eval(
            "Model metadata key 'currency' must be a string ISO currency code".into(),
        ));
    }

    Err(finstack_quant_statements::Error::Eval(format!(
        "Model '{}' is missing required metadata key 'currency'. \
         Set model.meta[\"currency\"] to an ISO currency code such as 'USD'.",
        model.id
    )))
}

/// Build the debt-and-cash equity bridge from the model's balance sheet.
///
/// Net Debt = Total Debt - Cash
///
/// This function attempts to find debt and cash nodes in the model results.
pub(super) fn equity_bridge_from_model(
    model: &FinancialModelSpec,
    results: &finstack_quant_statements::evaluator::StatementResult,
    balance_sheet_period: Option<finstack_quant_core::dates::PeriodId>,
) -> Result<EquityBridge> {
    // A future forecast balance cannot stand in for the opening balance sheet.
    let selected_period_id = balance_sheet_period.ok_or_else(|| finstack_quant_statements::Error::Eval(
        "No balance-sheet period ends on or before the valuation date; provide an opening balance or net_debt_override".into()
    ))?;

    let currency = extract_currency_from_model(model)?;
    let component = |names: [&str; 2]| -> Result<f64> {
        let node = names.into_iter().find(|name| crate::cell_float(results, name, &selected_period_id).is_some())
            .ok_or_else(|| finstack_quant_statements::Error::Eval(format!(
                "Net debt requires '{}' or '{}' at period {}; provide the balance-sheet node or net_debt_override",
                names[0], names[1], selected_period_id
            )))?;
        monetary_node_value(results, node, &selected_period_id, currency)
    };
    let total_debt = component(["total_debt", "debt"])?;
    let cash = component(["cash", "cash_and_equivalents"])?;

    Ok(EquityBridge {
        total_debt,
        cash,
        ..EquityBridge::default()
    })
}

/// Replace an exit-multiple `terminal_metric` with the last forecast
/// period's statement node when `metric_node` is set.
///
/// Without a metric node the spec's own `terminal_metric` is used, and it must
/// be positive and finite: a zero placeholder would silently price a zero
/// terminal value.
pub(super) fn resolve_exit_multiple_metric(
    model: &FinancialModelSpec,
    results: &StatementResult,
    terminal_value: TerminalValueSpec,
    metric_node: Option<&str>,
) -> Result<TerminalValueSpec> {
    let Some(node) = metric_node else {
        if let TerminalValueSpec::ExitMultiple {
            terminal_metric, ..
        } = terminal_value
            && !terminal_metric.is_some_and(|metric| metric.is_finite() && metric > 0.0)
        {
            return Err(finstack_quant_statements::Error::InvalidInput(format!(
                "exit-multiple terminal_metric must be positive and finite unless \
                     exit_multiple_metric_node supplies it (got {terminal_metric:?})"
            )));
        }
        return Ok(terminal_value);
    };
    let TerminalValueSpec::ExitMultiple { multiple, .. } = terminal_value else {
        return Ok(terminal_value);
    };
    let last_forecast = model.periods.iter().rfind(|period| !period.is_actual);
    let Some(last_forecast) = last_forecast else {
        return Err(finstack_quant_statements::Error::Eval(
            "Exit-multiple metric node requires a forecast period".into(),
        ));
    };
    let currency = extract_currency_from_model(model)?;
    let metric = trailing_year_metric(model, results, node, last_forecast.end, currency)?;
    Ok(TerminalValueSpec::ExitMultiple {
        terminal_metric: Some(metric.amount()),
        multiple,
    })
}

pub(crate) fn trailing_year_metric(
    model: &FinancialModelSpec,
    results: &StatementResult,
    node: &str,
    end: Date,
    currency: Currency,
) -> Result<Money> {
    let year = end.year() - 1;
    let start = Date::from_calendar_date(year, end.month(), end.day())
        .or_else(|_| Date::from_calendar_date(year, end.month(), 28))
        .map_err(|_| {
            finstack_quant_statements::Error::Eval("Trailing-year date is out of range".into())
        })?;
    let incomplete = || {
        finstack_quant_statements::Error::Eval(format!(
            "Terminal metric '{node}' requires complete contiguous history from {start} to {end}"
        ))
    };
    let mut cursor = end;
    let mut metric = Money::zero(currency);
    for period in model
        .periods
        .iter()
        .rev()
        .filter(|period| period.end <= end)
    {
        if cursor == start {
            break;
        }
        if period.end != cursor || period.start < start || period.start >= period.end {
            return Err(incomplete());
        }
        metric = metric.checked_add(monetary_node_money(results, node, &period.id, currency)?)?;
        cursor = period.start;
    }
    if cursor != start {
        return Err(incomplete());
    }
    Ok(metric)
}

/// Read a monetary statement node without discarding its currency.
pub(crate) fn monetary_node_value(
    results: &StatementResult,
    node: &str,
    period: &finstack_quant_core::dates::PeriodId,
    currency: Currency,
) -> Result<f64> {
    monetary_node_money(results, node, period, currency).map(|amount| amount.amount())
}

pub(crate) fn monetary_node_money(
    results: &StatementResult,
    node: &str,
    period: &PeriodId,
    currency: Currency,
) -> Result<Money> {
    let amount = results.get_money(node, period).ok_or_else(|| {
        finstack_quant_statements::Error::Eval(format!(
            "Node '{node}' at {period} must be monetary in {currency}"
        ))
    })?;
    if amount.currency() != currency || !amount.amount().is_finite() {
        return Err(finstack_quant_statements::Error::Eval(format!(
            "Node '{node}' at {period} must be finite monetary data in {currency}, got {}",
            amount.currency()
        )));
    }
    Ok(amount)
}
