//! Shared pricing runner helpers for instrument-level golden fixtures.

use crate::golden::schema::{GoldenFixture, Market};
use finstack_core::market_data::context::MarketContext;
use finstack_valuations::calibration::api::engine::{self, ExecuteError};
use finstack_valuations::calibration::api::schema::CalibrationEnvelope;
use finstack_valuations::pricer::price_instrument_json_with_metrics_and_history;
use std::collections::BTreeMap;

fn metric_base(metric: &str) -> &str {
    metric.split_once("::").map_or(metric, |(base, _)| base)
}

/// Metrics to request from the pricer, derived from the expected-output keys.
///
/// `npv` is always produced by the pricer and is therefore never requested.
pub(crate) fn requested_metrics(fixture: &GoldenFixture) -> Vec<String> {
    let mut metrics = Vec::new();
    for key in fixture.expected.keys() {
        let base = metric_base(key);
        if base != "npv" && !metrics.iter().any(|m| m == base) {
            metrics.push(base.to_string());
        }
    }
    metrics
}

fn resolve_market(market: &Market) -> Result<MarketContext, String> {
    match market {
        Market::Snapshot { data } => serde_json::from_value::<MarketContext>(data.clone())
            .map_err(|err| format!("parse market snapshot: {err}")),
        Market::Envelope { envelope } => {
            let env: CalibrationEnvelope = serde_json::from_value(envelope.clone())
                .map_err(|err| format!("parse market envelope: {err}"))?;
            // Use `execute_with_diagnostics` so envelope failures surface the
            // structured `EnvelopeError::SolverNotConverged` (worst-quote ID,
            // tolerance, etc.) instead of the lossy `Error::Calibration` form.
            let result = engine::execute_with_diagnostics(&env).map_err(|err| {
                let plan_id = &env.plan.id;
                match &err {
                    ExecuteError::Envelope(envelope_err) => format!(
                        "calibrate market envelope for plan '{plan_id}' failed \
                         ({}, step={:?}): {envelope_err}",
                        envelope_err.kind_str(),
                        envelope_err.step_id(),
                    ),
                    ExecuteError::Other(other) => {
                        format!("calibrate market envelope for plan '{plan_id}': {other}")
                    }
                }
            })?;
            let plan_id = env.plan.id.clone();
            MarketContext::try_from(result.result.final_market)
                .map_err(|err| format!("rehydrate calibrated market for plan '{plan_id}': {err}"))
        }
    }
}

/// Price an instrument fixture that follows the common pricing input contract.
pub(crate) fn run_pricing_fixture(
    fixture: &GoldenFixture,
) -> Result<BTreeMap<String, f64>, String> {
    let pricing = fixture
        .pricing()
        .ok_or("pricing runner requires a 'pricing' fixture body")?;
    let market = resolve_market(&pricing.market)?;
    let instrument_json = serde_json::to_string(&pricing.instrument)
        .map_err(|err| format!("serialize instrument: {err}"))?;
    let metrics = requested_metrics(fixture);

    let result = price_instrument_json_with_metrics_and_history(
        &instrument_json,
        &market,
        &fixture.metadata.valuation_date,
        &pricing.model,
        &metrics,
        None,
        None,
    )
    .map_err(|err| format!("price instrument JSON: {err}"))?;

    let mut actuals = BTreeMap::new();
    for metric in fixture.expected.keys() {
        let value = if metric == "npv" {
            result.value.amount()
        } else {
            *result
                .measures
                .get(metric.as_str())
                .ok_or_else(|| format!("result missing metric '{metric}'"))?
        };
        actuals.insert(metric.clone(), value);
    }
    Ok(actuals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::schema::SCHEMA_VERSION;

    fn pricing_fixture(market: serde_json::Value) -> GoldenFixture {
        let json = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "metadata": {
                "name": "market_test",
                "domain": "rates.deposit",
                "description": "market resolution test",
                "valuation_date": "2026-04-30",
                "source": "formula",
                "source_detail": "unit test",
                "captured_by": "test",
                "captured_on": "2026-04-30",
                "last_reviewed_by": "test",
                "last_reviewed_on": "2026-04-30",
                "review_interval_months": 6,
                "regen_command": ""
            },
            "kind": "pricing",
            "model": "discounting",
            "market": market,
            "instrument": {},
            "expected": {"npv": 0.0},
            "tolerances": {"npv": {"abs": 0.0}}
        });
        serde_json::from_value(json).expect("parse fixture")
    }

    fn minimal_market() -> serde_json::Value {
        serde_json::json!({
            "version": 2,
            "curves": [],
            "fx": null,
            "surfaces": [],
            "prices": {},
            "series": [],
            "inflation_indices": [],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "collateral": {}
        })
    }

    fn minimal_envelope() -> serde_json::Value {
        serde_json::json!({
            "schema": "finstack.calibration",
            "plan": {"id": "test_envelope", "quote_sets": {}, "steps": [], "settings": {}}
        })
    }

    #[test]
    fn requested_metrics_derives_from_expected_and_excludes_npv() {
        let json = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "metadata": {
                "name": "m", "domain": "rates.irs", "description": "d",
                "valuation_date": "2026-04-30", "source": "formula",
                "source_detail": "u", "captured_by": "t", "captured_on": "2026-04-30",
                "last_reviewed_by": "t", "last_reviewed_on": "2026-04-30",
                "review_interval_months": 6, "regen_command": ""
            },
            "kind": "pricing",
            "model": "discounting",
            "market": {"kind": "envelope", "envelope": minimal_envelope()},
            "instrument": {},
            "expected": {"npv": 1.0, "dv01": 1.0, "bucketed_dv01::USD-OIS::1y": 1.0},
            "tolerances": {
                "npv": {"abs": 1.0}, "dv01": {"abs": 1.0},
                "bucketed_dv01::USD-OIS::1y": {"abs": 1.0}
            }
        });
        let fixture: GoldenFixture = serde_json::from_value(json).expect("parse");
        let metrics = requested_metrics(&fixture);
        assert_eq!(
            metrics,
            vec!["bucketed_dv01".to_string(), "dv01".to_string()]
        );
    }

    #[test]
    fn resolve_market_snapshot_only() {
        let fixture =
            pricing_fixture(serde_json::json!({"kind": "snapshot", "data": minimal_market()}));
        let pricing = fixture.pricing().expect("pricing body");
        resolve_market(&pricing.market).expect("snapshot resolves");
    }

    #[test]
    fn resolve_market_envelope_only() {
        let fixture = pricing_fixture(
            serde_json::json!({"kind": "envelope", "envelope": minimal_envelope()}),
        );
        let pricing = fixture.pricing().expect("pricing body");
        resolve_market(&pricing.market).expect("envelope resolves through engine::execute");
    }
}
