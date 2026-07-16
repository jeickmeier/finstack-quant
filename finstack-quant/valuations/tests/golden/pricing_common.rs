//! Shared pricing runner helpers for instrument-level golden fixtures.

use crate::golden::schema::{GoldenFixture, Market};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::calibration::api::engine::{self, ExecuteError};
use finstack_quant_valuations::calibration::api::schema::CalibrationEnvelope;
use finstack_quant_valuations::pricer::price_instrument_json_with_metrics_and_history;
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
            let plan_id = env.plan.id;
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
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

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
            "schema": "finstack_quant.calibration",
            "plan": {"id": "test_envelope", "quote_sets": {}, "steps": [], "settings": {}}
        })
    }

    fn structured_credit_fixture() -> GoldenFixture {
        serde_json::from_str(include_str!(
            "data/pricing/regression_goldens/structured_credit/abs_credit_card_senior.json"
        ))
        .expect("parse structured-credit golden fixture")
    }

    fn price_fixture_npv(
        fixture: &GoldenFixture,
        market: &MarketContext,
        instrument_json: &str,
    ) -> f64 {
        let pricing = fixture.pricing().expect("pricing body");
        let result = price_instrument_json_with_metrics_and_history(
            instrument_json,
            market,
            &fixture.metadata.valuation_date,
            &pricing.model,
            &[],
            None,
            None,
        )
        .expect("structured-credit fixture should price");
        result.value.amount()
    }

    fn direct_parallel_dv01(
        fixture: &GoldenFixture,
        market: &MarketContext,
        instrument_json: &str,
        curve_ids: &[CurveId],
    ) -> f64 {
        let bumped_market = |direction| {
            market
                .bump(curve_ids.iter().cloned().map(|id| MarketBump::Curve {
                    id,
                    spec: BumpSpec::parallel_bp(direction),
                }))
                .expect("declared curve should support a parallel bump")
        };
        let up = price_fixture_npv(fixture, &bumped_market(1.0), instrument_json);
        let down = price_fixture_npv(fixture, &bumped_market(-1.0), instrument_json);
        (up - down) / 2.0
    }

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() < tolerance,
            "expected {expected:.15}, got {actual:.15}"
        );
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

    #[test]
    fn structured_credit_dependencies_preserve_curve_roles_and_fixing_ids() {
        let fixture = structured_credit_fixture();
        let pricing = fixture.pricing().expect("pricing body");
        let instrument: InstrumentJson = serde_json::from_value(pricing.instrument.clone())
            .expect("parse structured-credit instrument");
        let InstrumentJson::StructuredCredit(instrument) = instrument else {
            panic!("fixture should contain structured credit");
        };

        let dependencies = instrument
            .market_dependencies()
            .expect("collect structured-credit dependencies");
        let discount_curves: Vec<_> = dependencies
            .curves
            .discount_curves
            .iter()
            .map(|id| id.as_str())
            .collect();
        let forward_curves: Vec<_> = dependencies
            .curves
            .forward_curves
            .iter()
            .map(|id| id.as_str())
            .collect();

        assert_eq!(discount_curves, ["USD-SOFR-DISC"]);
        assert_eq!(forward_curves, ["SOFR-3M"]);
        assert!(dependencies.curves.credit_curves.is_empty());
        assert!(dependencies.curves.inflation_curves.is_empty());
        assert_eq!(dependencies.series_ids, ["FIXING:SOFR-3M"]);
        assert!(dependencies.spot_ids.is_empty());
        assert!(dependencies.volatility_dependencies.is_empty());
        assert!(dependencies.fx_pairs.is_empty());
    }

    #[test]
    #[ignore = "slow: covered by mise goldens-test or mise rust-test-slow"]
    fn structured_credit_dv01_matches_declared_curve_repricing() {
        let fixture = structured_credit_fixture();
        let pricing = fixture.pricing().expect("pricing body");
        let market = resolve_market(&pricing.market).expect("resolve fixture market");
        let instrument_json =
            serde_json::to_string(&pricing.instrument).expect("serialize instrument");

        let discount = direct_parallel_dv01(
            &fixture,
            &market,
            &instrument_json,
            &[CurveId::new("USD-SOFR-DISC")],
        );
        let sofr_3m = direct_parallel_dv01(
            &fixture,
            &market,
            &instrument_json,
            &[CurveId::new("SOFR-3M")],
        );
        let combined = direct_parallel_dv01(
            &fixture,
            &market,
            &instrument_json,
            &[CurveId::new("USD-SOFR-DISC"), CurveId::new("SOFR-3M")],
        );

        let registry_result = price_instrument_json_with_metrics_and_history(
            &instrument_json,
            &market,
            &fixture.metadata.valuation_date,
            &pricing.model,
            &["dv01".to_string()],
            None,
            None,
        )
        .expect("registry DV01 should price");
        let registry_dv01 = registry_result.measures["dv01"];

        assert_close(discount, -3_043.885_803_190_06, 1e-6);
        assert_close(sofr_3m, 2836.106479169801, 1e-6);
        assert_close(combined, -207.779270004481, 1e-6);
        assert_close(combined, registry_dv01, 1e-8);
        assert!((combined - (discount + sofr_3m)).abs() < 1e-3);
        assert!((combined - discount).abs() > 2_000.0);
    }
}
