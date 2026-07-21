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
    calculate_tranche_oas, scenario_table, OasConfig, OasResult, ScenarioCell, ScenarioGrid,
    ScenarioTable, StructuredCredit, TrancheMetrics,
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

/// Z-spread-equivalent discount margin for a floating-rate tranche from tagged
/// instrument JSON, returned in decimal units (`0.0125` = 125 bp).
///
/// Contractual cashflows are projected without changing coupon projection, then
/// a constant additive spread is applied to the discount curve. The result is
/// zero at model PV, negative for a richer (higher) target PV, and positive for
/// a cheaper (lower) target PV; it is not the contractual quoted margin.
/// `target_pv` is interpreted in the named tranche's currency.
///
/// # Errors
///
/// Returns an error if the JSON is not a structured-credit deal, the as-of date
/// is malformed, the deal fails validation, the tranche is missing or
/// fixed-rate, `target_pv` is non-finite, required discount/projection market
/// data is unavailable, or the spread solve fails or exceeds ±5000 bp.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 tagged structured-credit instrument JSON.
/// * `tranche_id` - Identifier of the floating-rate tranche whose contractual
///   cashflows are spread-discounted.
/// * `market` - Market context supplying the discount curve and any forward
///   curves or historical fixings required for contractual cashflow projection.
/// * `as_of` - ISO-8601 valuation date used for projection and discounting.
/// * `target_pv` - Observed present-value amount in the named tranche's
///   currency. Values above model PV produce a negative result; values below
///   model PV produce a positive result.
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
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 tagged structured-credit instrument JSON.
/// * `tranche_id` - Identifier of the tranche whose break-even default rate is
///   solved.
/// * `market` - Market context used to project tranche cashflows.
/// * `as_of` - ISO-8601 valuation date used for cashflow projection.
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
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 tagged structured-credit instrument JSON.
/// * `tranche_id` - Identifier of the tranche whose option-adjusted spread is
///   solved.
/// * `market_price_pct` - Observed clean price as a percentage of original
///   tranche balance.
/// * `market` - Market context supplying discounting and stochastic-pricing
///   dependencies.
/// * `as_of` - ISO-8601 valuation date used for cashflow projection.
/// * `config_json` - Optional UTF-8 serialized [`OasConfig`]; `None` selects
///   the canonical OAS simulation and solver defaults.
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
    let result =
        calculate_tranche_oas(&deal, tranche_id, market_price_pct, market, as_of, &config)?;
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
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 tagged structured-credit instrument JSON.
/// * `tranche_id` - Identifier of the tranche whose own projected flows are
///   used for all metrics.
/// * `market` - Market context supplying the discount curve and required
///   projection dependencies.
/// * `as_of` - ISO-8601 valuation date used for projection and discounting.
/// * `market_price_pct` - Optional observed price as a percentage of original
///   balance; `None` uses model price and yields a zero z-spread.
pub fn structured_credit_tranche_metrics_json(
    instrument_json: &str,
    tranche_id: &str,
    market: &MarketContext,
    as_of: &str,
    market_price_pct: Option<f64>,
) -> Result<TrancheMetrics> {
    let deal = structured_credit_from_json(instrument_json)?;
    let as_of = parse_iso_date(as_of)?;
    let metrics = calculate_tranche_metrics(&deal, tranche_id, market, as_of, market_price_pct)?;
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

/// Scenario (CPR × CDR × severity) price/WAL/writedown table for a tranche from
/// tagged instrument JSON. `grid_json` is a serialized [`ScenarioGrid`].
///
/// # Errors
///
/// Returns an error if the JSON (instrument or grid) is invalid, the as-of date
/// is malformed, or a scenario fails to evaluate.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 tagged structured-credit instrument JSON.
/// * `tranche_id` - Identifier of the tranche evaluated at every scenario grid
///   point.
/// * `market` - Market context used for projection and discounting.
/// * `as_of` - ISO-8601 valuation date used for all scenario valuations.
/// * `grid_json` - UTF-8 serialized [`ScenarioGrid`] of CPR, CDR, and
///   recovery-severity assumptions to sweep.
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
    let table = scenario_table(&deal, tranche_id, market, as_of, &grid)?;
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
