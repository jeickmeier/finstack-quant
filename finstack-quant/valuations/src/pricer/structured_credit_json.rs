//! JSON binding entry points for structured-credit tranche analytics.
//!
//! The standalone structured-credit tranche metrics — discount margin, OAS,
//! break-even CDR and the scenario table — take a tranche id and metric-specific
//! configuration that the generic metric registry does not carry, so they are
//! exposed here as dedicated JSON entry points the Python and WASM bindings
//! wrap. Each parses tagged instrument JSON, recovers the [`StructuredCredit`]
//! deal, parses the as-of date, and dispatches to the corresponding metric.
//!
//! # Examples
//! ```no_run
//! use finstack_quant_valuations::pricer::structured_credit_tranche_breakeven_cdr_json;
//! use finstack_quant_core::market_data::context::MarketContext;
//!
//! # fn run(instrument_json: &str, market: &MarketContext) -> finstack_quant_core::Result<()> {
//! let cdr =
//!     structured_credit_tranche_breakeven_cdr_json(instrument_json, "SR", market, "2024-01-01")?;
//! assert!(cdr >= 0.0);
//! # Ok(())
//! # }
//! ```

use crate::instruments::fixed_income::structured_credit::{
    calculate_tranche_breakeven_cdr, calculate_tranche_discount_margin, calculate_tranche_metrics,
    calculate_tranche_oas, scenario_table, OasConfig, OasResult, ScenarioGrid, ScenarioTable,
    StructuredCredit, TrancheMetrics,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::parse_iso_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};

/// Recover a [`StructuredCredit`] deal from tagged instrument JSON.
///
/// # Errors
///
/// Returns an error if the JSON is invalid or does not describe a
/// `structured_credit` instrument.
fn structured_credit_from_json(instrument_json: &str) -> Result<StructuredCredit> {
    let instrument = super::json::parse_boxed_instrument_json(instrument_json, None)?;
    instrument
        .as_any()
        .downcast_ref::<StructuredCredit>()
        .cloned()
        .ok_or_else(|| Error::Validation("expected a structured_credit instrument".to_string()))
}

/// Currency of the named tranche, used to interpret scalar money inputs.
fn tranche_currency(deal: &StructuredCredit, tranche_id: &str) -> Result<Currency> {
    deal.tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .map(|t| t.original_balance.currency())
        .ok_or_else(|| {
            Error::from(finstack_quant_core::InputError::NotFound {
                id: format!("tranche:{tranche_id}"),
            })
        })
}

/// Discount margin (decimal) for a floating-rate tranche from tagged
/// instrument JSON.
///
/// `target_pv` is a scalar amount interpreted in the named tranche's currency.
///
/// # Errors
///
/// Returns an error if the JSON is not a structured-credit deal, the as-of date
/// is malformed, the tranche is missing or fixed-rate, or the solve fails.
pub fn structured_credit_tranche_discount_margin_json(
    instrument_json: &str,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    target_pv: f64,
) -> Result<f64> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    let currency = tranche_currency(&deal, tranche_id)?;
    calculate_tranche_discount_margin(
        &deal,
        tranche_id,
        market,
        as_of,
        discount_margin_target_money(target_pv, currency)?,
    )
}

fn discount_margin_target_money(
    target_pv: f64,
    currency: finstack_quant_core::currency::Currency,
) -> Result<Money> {
    Money::try_new(target_pv, currency)
}

/// Break-even constant default rate (CDR, decimal) for a tranche from tagged
/// instrument JSON — the highest CDR at which the tranche takes no writedown.
///
/// # Errors
///
/// Returns an error if the JSON is not a structured-credit deal, the as-of date
/// is malformed, or the tranche is missing.
pub fn structured_credit_tranche_breakeven_cdr_json(
    instrument_json: &str,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
) -> Result<f64> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    calculate_tranche_breakeven_cdr(&deal, tranche_id, market, as_of)
}

/// Option-adjusted spread for a tranche from tagged instrument JSON.
///
/// `market_price_pct` is the quoted price as a percentage of original balance.
/// `config_json`, when present, is a serialized [`OasConfig`]; otherwise the
/// default configuration is used.
///
/// # Errors
///
/// Returns an error if the JSON (instrument or config) is invalid, the as-of
/// date is malformed, the tranche or discount curve is missing, or the solve
/// fails.
pub fn structured_credit_tranche_oas_json(
    instrument_json: &str,
    tranche_id: &str,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: &str,
    config_json: Option<&str>,
) -> Result<OasResult> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    let config = match config_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid OAS config JSON: {e}")))?,
        None => OasConfig::default(),
    };
    calculate_tranche_oas(&deal, tranche_id, market_price_pct, market, as_of, &config)
}

/// Per-tranche risk/spread metrics ([`TrancheMetrics`]) for a tranche from
/// tagged instrument JSON — PV, price, WAL, z-spread, CS01, spread/modified
/// duration and convexity, all computed from that tranche's own cashflows.
///
/// `market_price_pct`, when present, is the quoted price (% of original balance)
/// the z-spread and CS01 are solved against; when `None` the tranche's own model
/// price is used (giving a zero z-spread).
///
/// # Errors
///
/// Returns an error if the JSON is not a structured-credit deal, the as-of date
/// is malformed, the tranche or discount curve is missing, or a metric fails to
/// compute.
pub fn structured_credit_tranche_metrics_json(
    instrument_json: &str,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    market_price_pct: Option<f64>,
) -> Result<TrancheMetrics> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    calculate_tranche_metrics(&deal, tranche_id, market, as_of, market_price_pct)
}

/// Scenario (CPR × CDR × severity) price/WAL/writedown table for a tranche from
/// tagged instrument JSON. `grid_json` is a serialized [`ScenarioGrid`].
///
/// # Errors
///
/// Returns an error if the JSON (instrument or grid) is invalid, the as-of date
/// is malformed, or a scenario fails to evaluate.
pub fn structured_credit_tranche_scenario_table_json(
    instrument_json: &str,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    grid_json: &str,
) -> Result<ScenarioTable> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    let grid: ScenarioGrid = serde_json::from_str(grid_json)
        .map_err(|e| Error::Validation(format!("invalid scenario grid JSON: {e}")))?;
    scenario_table(&deal, tranche_id, market, as_of, &grid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    fn invalid_structured_credit_json() -> String {
        let mut deal = StructuredCredit::example();
        deal.cleanup_call_pct = Some(-0.5);
        serde_json::to_string(&crate::instruments::InstrumentJson::StructuredCredit(
            Box::new(deal),
        ))
        .expect("serialize invalid structured credit")
    }

    #[test]
    fn non_finite_discount_margin_target_is_a_typed_error() {
        for target in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(discount_margin_target_money(target, Currency::USD).is_err());
        }
    }

    #[test]
    fn structured_credit_json_routes_validate_before_other_inputs_or_market_access() {
        let instrument_json = invalid_structured_credit_json();
        let market = MarketContext::new();

        let errors = [
            structured_credit_tranche_discount_margin_json(
                &instrument_json,
                "missing",
                &market,
                "not-a-date",
                f64::NAN,
            )
            .expect_err("instrument validation must win")
            .to_string(),
            structured_credit_tranche_breakeven_cdr_json(
                &instrument_json,
                "missing",
                &market,
                "not-a-date",
            )
            .expect_err("instrument validation must win")
            .to_string(),
            structured_credit_tranche_oas_json(
                &instrument_json,
                "missing",
                f64::NAN,
                &market,
                "not-a-date",
                Some("not-json"),
            )
            .expect_err("instrument validation must win")
            .to_string(),
            structured_credit_tranche_metrics_json(
                &instrument_json,
                "missing",
                &market,
                "not-a-date",
                Some(f64::NAN),
            )
            .expect_err("instrument validation must win")
            .to_string(),
            structured_credit_tranche_scenario_table_json(
                &instrument_json,
                "missing",
                &market,
                "not-a-date",
                "not-json",
            )
            .expect_err("instrument validation must win")
            .to_string(),
        ];

        for message in errors {
            assert!(
                message.contains("cleanup_call_pct"),
                "unexpected error ordering: {message}"
            );
        }
    }
}
