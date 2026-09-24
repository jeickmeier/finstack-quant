//! Finite-output guards for the tranche analytics calculators.
//!
//! [`calculate_tranche_metrics`](super::calculate_tranche_metrics),
//! [`calculate_tranche_oas`](super::calculate_tranche_oas) and
//! [`scenario_table`](super::scenario_table) reject a non-finite field
//! rather than return it: `serde_json` serializes NaN and infinity as JSON
//! `null`, which downstream consumers coerce to 0, so a failed computation
//! would otherwise read as a valid result.

use super::{OasResult, ScenarioCell, ScenarioTable, TrancheMetrics};
use finstack_quant_core::Result;

pub(crate) fn ensure_oas_finite(result: &OasResult) -> Result<()> {
    ensure_finite(
        &[
            ("oas", result.oas),
            ("model_price", result.model_price),
            ("market_price", result.market_price),
            ("price_std_error", result.price_std_error),
        ],
        "structured-credit tranche OAS",
    )
}

pub(crate) fn ensure_tranche_metrics_finite(metrics: &TrancheMetrics) -> Result<()> {
    ensure_finite(
        &[
            ("pv", metrics.pv),
            ("price_pct", metrics.price_pct),
            ("factor", metrics.factor),
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

pub(crate) fn ensure_scenario_table_finite(table: &ScenarioTable) -> Result<()> {
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
                factor: 1.0,
                wal: 3.0,
                z_spread_bp: 150.0,
                cs01: bad,
                spread_duration: 4.0,
                spread_convexity: 12.0,
                modified_duration: 3.5,
                convexity: 12.0,
                target_price_pct: 98.0,
                wal_to_call: None,
                z_spread_to_call_bp: None,
                dm_to_call_bp: None,
                dm_bp: None,
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
}
