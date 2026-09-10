//! Host-facing orchestration for structured-credit tranche analytics.
//!
//! The standalone structured-credit tranche metrics — discount margin, OAS,
//! break-even CDR and the scenario table — take a tranche id and metric-specific
//! configuration that the scalar generic metric registry does not carry. These
//! functions accept an already validated [`StructuredCredit`], normalize the
//! remaining host inputs, and dispatch to the typed domain calculators.
//!
//! # Examples
//! ```no_run
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
//!     structured_credit_tranche_breakeven_cdr, StructuredCredit,
//! };
//!
//! # fn run(deal: &StructuredCredit, market: &MarketContext) -> finstack_quant_core::Result<()> {
//! let cdr = structured_credit_tranche_breakeven_cdr(
//!     deal, "SR", market, "2024-01-01",
//! )?;
//! assert!(cdr >= 0.0);
//! # Ok(())
//! # }
//! ```

use crate::instruments::fixed_income::structured_credit::{
    calculate_tranche_breakeven_cdr, calculate_tranche_discount_margin, calculate_tranche_metrics,
    calculate_tranche_oas, scenario_table, OasConfig, OasResult, ScenarioCell, ScenarioGrid,
    ScenarioTable, StructuredCredit, TrancheMetrics,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::parse_iso_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};

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

/// Solve a tranche discount margin for an already parsed structured-credit
/// deal.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal.
/// * `tranche_id` - Identifier of the floating-rate tranche to solve.
/// * `market` - Market context supplying discounting and projection inputs.
/// * `as_of` - ISO-8601 valuation date.
/// * `target_pv` - Positive dirty settlement amount in the tranche's currency,
///   including accrued interest once. The deal's `quote_settlement_date`
///   excludes seller-owned payments; absence means valuation-date settlement.
///
/// # Errors
///
/// Returns an error for an invalid date or target, a missing or fixed-rate
/// tranche, unavailable market data, or a failed spread solve.
pub fn structured_credit_tranche_discount_margin(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    target_pv: f64,
) -> Result<f64> {
    let as_of = parse_iso_date(as_of)?;
    let currency = tranche_currency(deal, tranche_id)?;
    calculate_tranche_discount_margin(
        deal,
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
    Money::new(target_pv, currency)
}

/// Solve the break-even CDR for an already parsed structured-credit deal.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal.
/// * `tranche_id` - Identifier of the tranche to solve.
/// * `market` - Market context used to project tranche cashflows.
/// * `as_of` - ISO-8601 valuation date.
///
/// # Errors
///
/// Returns an error for an invalid date, missing tranche or market data, or a
/// failed break-even calculation.
pub fn structured_credit_tranche_breakeven_cdr(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
) -> Result<f64> {
    let as_of = parse_iso_date(as_of)?;
    calculate_tranche_breakeven_cdr(deal, tranche_id, market, as_of)
}

/// Solve tranche OAS for an already parsed structured-credit deal.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal.
/// * `tranche_id` - Identifier of the tranche to solve.
/// * `market_price_pct` - Clean settlement price as a percentage of original
///   balance. Accrued interest is added once at `quote_settlement_date`,
///   which defaults to the valuation date.
/// * `market` - Market context supplying discounting and stochastic inputs.
/// * `as_of` - ISO-8601 valuation date.
/// * `config_json` - Optional serialized [`OasConfig`]; `None` uses defaults.
///
/// # Errors
///
/// Returns an error for invalid date or configuration input, missing tranche or
/// market data, a failed solve, or non-finite output.
pub fn structured_credit_tranche_oas(
    deal: &StructuredCredit,
    tranche_id: &str,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: &str,
    config_json: Option<&str>,
) -> Result<OasResult> {
    let as_of = parse_iso_date(as_of)?;
    let config = match config_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Error::Validation(format!("invalid OAS config JSON: {e}")))?,
        None => OasConfig::default(),
    };
    let result = calculate_tranche_oas(deal, tranche_id, market_price_pct, market, as_of, &config)?;
    ensure_finite(
        &[
            ("oas", result.oas),
            ("model_price", result.model_price),
            ("market_price", result.market_price),
            ("price_std_error", result.price_std_error),
        ],
        "structured-credit tranche OAS",
    )?;
    Ok(result)
}

/// Compute tranche metrics for an already parsed structured-credit deal.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal.
/// * `tranche_id` - Identifier of the tranche to measure.
/// * `market` - Market context supplying discounting and projection inputs.
/// * `as_of` - ISO-8601 valuation date.
/// * `market_price_pct` - Optional clean settlement price as a percentage of original
///   balance; `None` uses the model price.
///
/// # Errors
///
/// Returns an error for an invalid date, missing tranche or market data, a
/// failed calculation, or non-finite output.
pub fn structured_credit_tranche_metrics(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    market_price_pct: Option<f64>,
) -> Result<TrancheMetrics> {
    let as_of = parse_iso_date(as_of)?;
    let metrics = calculate_tranche_metrics(deal, tranche_id, market, as_of, market_price_pct)?;
    ensure_tranche_metrics_finite(&metrics)?;
    Ok(metrics)
}

fn ensure_tranche_metrics_finite(metrics: &TrancheMetrics) -> Result<()> {
    ensure_finite(
        &[
            ("pv", metrics.pv),
            ("price_pct", metrics.price_pct),
            ("wal", metrics.wal),
            ("z_spread_bp", metrics.z_spread_bp),
            ("cs01", metrics.cs01),
            ("spread_duration", metrics.spread_duration),
            ("modified_duration", metrics.modified_duration),
            ("convexity", metrics.convexity),
            ("target_price_pct", metrics.target_price_pct),
        ],
        "structured-credit tranche metrics",
    )
}

/// Reject non-finite metrics before `serde_json` converts them to JSON `null`.
fn ensure_finite(fields: &[(&str, f64)], metric: &str) -> Result<()> {
    for (name, value) in fields {
        if !value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{metric} produced a non-finite `{name}` ({value}). Serializing \
                 it would emit JSON `null`, which downstream consumers coerce to \
                 0 — a failed computation must not read as a valid result."
            )));
        }
    }
    Ok(())
}

/// Build a tranche scenario table for an already parsed structured-credit
/// deal.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal.
/// * `tranche_id` - Identifier of the tranche evaluated at each grid point.
/// * `market` - Market context used for projection and discounting.
/// * `as_of` - ISO-8601 valuation date.
/// * `grid_json` - Serialized [`ScenarioGrid`] containing CPR, CDR, and
///   severity axes.
///
/// # Errors
///
/// Returns an error for invalid date or grid input, missing tranche or market
/// data, a failed scenario, or non-finite output.
pub fn structured_credit_tranche_scenario_table(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    grid_json: &str,
) -> Result<ScenarioTable> {
    let as_of = parse_iso_date(as_of)?;
    let grid: ScenarioGrid = serde_json::from_str(grid_json)
        .map_err(|e| Error::Validation(format!("invalid scenario grid JSON: {e}")))?;
    let table = scenario_table(deal, tranche_id, market, as_of, &grid)?;
    ensure_scenario_table_finite(&table)?;
    Ok(table)
}

fn ensure_scenario_table_finite(table: &ScenarioTable) -> Result<()> {
    for (index, cell) in table.cells.iter().enumerate() {
        ensure_scenario_cell_finite(cell, index)?;
    }
    Ok(())
}

fn ensure_scenario_cell_finite(cell: &ScenarioCell, index: usize) -> Result<()> {
    ensure_finite(
        &[
            ("cpr", cell.cpr),
            ("cdr", cell.cdr),
            ("severity", cell.severity),
            ("price", cell.price),
            ("wal", cell.wal),
            ("writedown", cell.writedown),
        ],
        &format!("structured-credit scenario cell {index}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    /// Non-finite metrics are errors rather than JSON `null`.
    #[test]
    fn non_finite_metric_is_rejected_at_the_boundary() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = ensure_finite(&[("pv", bad)], "test metric")
                .expect_err("a non-finite metric must be rejected");
            let msg = err.to_string();
            assert!(
                msg.contains("pv") && msg.contains("non-finite"),
                "the error must name the offending field; got: {msg}"
            );
        }
    }

    /// Finite metrics include zero and legitimate negative spreads.
    #[test]
    fn finite_metrics_pass_the_boundary_check() {
        assert!(
            ensure_finite(
                &[("oas", 0.0), ("spread_duration", -1.5), ("pv", 1.0e9)],
                "test metric"
            )
            .is_ok(),
            "zero, negative and large finite values must all be accepted"
        );
    }

    #[test]
    fn non_finite_tranche_metrics_cs01_is_rejected_at_the_boundary() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let metrics = TrancheMetrics {
                tranche_id: "A".to_string(),
                currency: "USD".to_string(),
                pv: 100.0,
                price_pct: 100.0,
                wal: 3.0,
                z_spread_bp: 150.0,
                cs01: bad,
                spread_duration: 4.0,
                modified_duration: 3.5,
                convexity: 12.0,
                target_price_pct: 98.0,
            };
            let err = ensure_tranche_metrics_finite(&metrics)
                .expect_err("a non-finite cs01 must be rejected");
            let message = err.to_string();
            assert!(
                message.contains("cs01") && message.contains("non-finite"),
                "the error must identify cs01; got: {message}"
            );
        }
    }

    #[test]
    fn non_finite_scenario_cell_fields_are_rejected_at_the_boundary() {
        let finite = ScenarioCell {
            cpr: 0.05,
            cdr: 0.02,
            severity: 0.4,
            price: 98.0,
            wal: 3.0,
            writedown: 0.0,
        };
        let cases = [
            (
                "cpr",
                ScenarioCell {
                    cpr: f64::NAN,
                    ..finite.clone()
                },
            ),
            (
                "cdr",
                ScenarioCell {
                    cdr: f64::NAN,
                    ..finite.clone()
                },
            ),
            (
                "severity",
                ScenarioCell {
                    severity: f64::NAN,
                    ..finite.clone()
                },
            ),
            (
                "price",
                ScenarioCell {
                    price: f64::NAN,
                    ..finite.clone()
                },
            ),
            (
                "wal",
                ScenarioCell {
                    wal: f64::NAN,
                    ..finite.clone()
                },
            ),
            (
                "writedown",
                ScenarioCell {
                    writedown: f64::NAN,
                    ..finite
                },
            ),
        ];

        for (field, cell) in cases {
            let table = ScenarioTable {
                tranche_id: "A".to_string(),
                cells: vec![cell],
            };
            let err = ensure_scenario_table_finite(&table)
                .expect_err("a non-finite scenario cell field must be rejected");
            let message = err.to_string();
            assert!(
                message.contains(field) && message.contains("non-finite"),
                "the error must identify {field}; got: {message}"
            );
        }
    }

    /// `serde_json` emits `null` for non-finite floats.
    #[test]
    fn serde_json_emits_null_for_non_finite_which_is_why_we_guard() {
        let encoded = serde_json::to_string(&f64::NAN).expect("serde_json does not error on NaN");
        assert_eq!(
            encoded, "null",
            "if this ever starts erroring instead, the ensure_finite guard \
             becomes belt-and-braces rather than load-bearing"
        );
    }

    #[test]
    fn non_finite_discount_margin_target_is_a_typed_error() {
        for target in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(discount_margin_target_money(target, Currency::USD).is_err());
        }
    }
}
