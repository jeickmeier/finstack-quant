//! Shared pricing runner helpers for instrument-level golden fixtures.

use crate::golden::schema::{GoldenFixture, Market};
use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::pricer::{parse_boxed_instrument_from_json, price_instrument};
use std::collections::BTreeMap;
use std::sync::Arc;

fn price_instrument_from_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    instrument_pricing_overrides_json: Option<&str>,
    market_history_json: Option<&str>,
) -> finstack_quant_core::Result<finstack_quant_valuations::results::ValuationResult> {
    let instrument =
        parse_boxed_instrument_from_json(instrument_json, instrument_pricing_overrides_json)?;
    price_instrument(
        &instrument,
        market,
        as_of,
        model,
        metrics,
        market_history_json,
        PricingOptions::default()
            .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::new())),
    )
}

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
            let bytes = serde_json::to_vec(envelope)
                .map_err(|error| format!("encode market envelope: {error}"))?;
            let (env, _load_report) =
                CalibrationEnvelope::from_slice_strict(&bytes, &LoadLimits::default())
                    .map_err(|error| format!("strictly load market envelope: {error}"))?;
            let result = engine::execute(&env).map_err(|error| {
                let plan_id = &env.plan.id;
                let details = error.details();
                format!(
                    "calibrate market envelope for plan '{plan_id}' failed \
                     (stage={}, category={}, step={:?}): {}",
                    details.stage.as_str(),
                    details.category,
                    details.step_id,
                    details.cause,
                )
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

    let result = price_instrument_from_json(
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
    use crate::golden::schema::SCHEMA;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope, InstrumentJson};

    fn pricing_fixture(market: serde_json::Value) -> GoldenFixture {
        let json = serde_json::json!({
            "schema": SCHEMA,
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
            "schema_version": 1,
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
            "collateral": {},
            "hierarchy": null
        })
    }

    fn minimal_envelope() -> serde_json::Value {
        serde_json::json!({
            "schema": "finstack_quant.calibration/1",
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
        let result = price_instrument_from_json(
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
            "schema": SCHEMA,
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
        let envelope: InstrumentEnvelope = serde_json::from_value(pricing.instrument.clone())
            .expect("parse structured-credit instrument envelope");
        let instrument = envelope.instrument;
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
        assert!(dependencies.market_scalar_ids.is_empty());
        assert!(dependencies.volatility_dependencies.is_empty());
        assert!(dependencies.fx_pairs.is_empty());
    }

    #[test]
    fn structured_credit_default_claim_inputs_reconcile() {
        use time::macros::date;

        for raw in [
            include_str!(
                "data/pricing/regression_goldens/structured_credit/abs_credit_card_senior.json"
            ),
            include_str!(
                "data/pricing/regression_goldens/structured_credit/clo_mezzanine_base_case.json"
            ),
        ] {
            let fixture: GoldenFixture = serde_json::from_str(raw).unwrap();
            let pricing = fixture.pricing().unwrap();
            let instrument_json = serde_json::to_string(&pricing.instrument).unwrap();
            let instrument = parse_boxed_instrument_from_json(&instrument_json, None)
                .expect("default claim and tranche state must be valid");
            let deal = instrument
                .as_instrument()
                .as_any()
                .downcast_ref::<finstack_quant_valuations::instruments::StructuredCredit>()
                .unwrap();
            let asset = deal
                .pool
                .assets
                .iter()
                .find(|asset| asset.id.as_str() == "BOND1")
                .unwrap();
            assert!(asset.is_defaulted);
            assert_eq!(asset.default_date, Some(date!(2026 - 04 - 01)));
            assert_eq!(asset.balance.amount(), 8_000_000.0);
            let claim = asset.recovery_amount.unwrap();
            assert_eq!(claim.currency(), asset.balance.currency());
            assert_eq!(claim.amount(), 1_000_000.0);
            assert_eq!(
                deal.pool.performing_balance().unwrap().amount(),
                12_000_000.0
            );
            assert_eq!(deal.pool.cumulative_defaults, asset.balance);
            assert_eq!(deal.pool.cumulative_recoveries.amount(), 0.0);
            assert_eq!(deal.behavior_overrides.recovery_lag_months, Some(9));
            let historical_loss = asset.balance.amount() - claim.amount();
            let written_down: f64 = deal
                .tranches
                .tranches
                .iter()
                .map(|tranche| tranche.original_balance.amount() - tranche.current_balance.amount())
                .sum();
            assert_eq!(written_down, historical_loss);
            for tranche in &deal.tranches.tranches {
                match tranche.id.as_str() {
                    "EQUITY" => assert_eq!(tranche.current_balance.amount(), 0.0),
                    "SENIOR" => assert_eq!(tranche.current_balance.amount(), 43_000_000.0),
                    id => panic!("unexpected tranche {id}"),
                }
            }
        }
    }

    #[test]
    fn structured_credit_default_claim_is_paid_once_without_repeating_historical_loss() {
        use finstack_quant_core::{currency::Currency, money::Money};
        use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCoupon;
        use time::macros::date;

        let fixture = structured_credit_fixture();
        let pricing = fixture.pricing().unwrap();
        let envelope: InstrumentEnvelope =
            serde_json::from_value(pricing.instrument.clone()).unwrap();
        let InstrumentJson::StructuredCredit(mut deal) = envelope.instrument else {
            panic!("expected structured credit");
        };
        deal.pool.assets.retain(|asset| asset.is_defaulted);
        // The empty market cannot project the fixture swap; this test is
        // about the default claim, not the hedge.
        deal.hedge_swaps.clear();
        let zero = Money::from((0_i64, Currency::USD));
        deal.pool.collection_account = zero;
        deal.pool.reserve_account = zero;
        deal.pool.excess_spread_account = zero;
        for tranche in &mut deal.tranches.tranches {
            tranche.coupon = TrancheCoupon::Fixed { rate: 0.0 };
        }
        let flows = deal
            .get_tranche_cashflows("SENIOR", &MarketContext::new(), date!(2026 - 04 - 30))
            .expect("the only future collateral receipt is the outstanding default claim");
        let principal: Vec<_> = flows
            .principal_flows
            .iter()
            .filter(|(_, amount)| amount.amount() > 0.0)
            .copied()
            .collect();
        assert_eq!(
            principal,
            vec![(
                date!(2027 - 01 - 04),
                Money::from((1_000_000_i64, Currency::USD))
            )]
        );
        assert_eq!(flows.total_principal.amount(), 1_000_000.0);
        assert_eq!(flows.total_interest.amount(), 0.0);
        // The claim is the deal's only remaining collateral, so the 42M the
        // senior never receives is realized once, as a principal shortfall on
        // the final settlement date — not as a repeat of the historical loss.
        assert_eq!(
            flows.writedown_flows,
            vec![(
                date!(2027 - 01 - 04),
                Money::from((42_000_000_i64, Currency::USD))
            )]
        );
        assert_eq!(flows.total_writedown.amount(), 42_000_000.0);
        assert_eq!(flows.final_balance.amount(), 0.0);
    }

    #[test]
    #[ignore = "slow: covered by mise goldens-test or mise rust-test-slow"]
    fn structured_credit_spread_quote_matches_discounted_cashflow_reference() {
        use finstack_quant_core::dates::{DayCount, DayCountContext};
        use time::macros::date;

        for raw in [
            include_str!(
                "data/pricing/regression_goldens/structured_credit/abs_credit_card_senior.json"
            ),
            include_str!(
                "data/pricing/regression_goldens/structured_credit/clo_mezzanine_base_case.json"
            ),
        ] {
            let fixture: GoldenFixture = serde_json::from_str(raw).unwrap();
            let pricing = fixture.pricing().unwrap();
            let market = resolve_market(&pricing.market).unwrap();
            let envelope: InstrumentEnvelope =
                serde_json::from_value(pricing.instrument.clone()).unwrap();
            let InstrumentJson::StructuredCredit(deal) = envelope.instrument else {
                panic!("expected structured credit");
            };
            let as_of = date!(2026 - 04 - 30);
            let discount = market
                .get_discount(deal.discount_curve_id.as_str())
                .unwrap();
            let mut quote = 0.0;
            let mut npv = 0.0;
            let mut cs01 = 0.0;
            for tranche in &deal.tranches.tranches {
                let flows = deal
                    .get_tranche_cashflows(tranche.id.as_str(), &market, as_of)
                    .unwrap();
                for (date, amount) in flows.cashflows.iter().filter(|(date, _)| *date > as_of) {
                    let t = DayCount::Act365F
                        .year_fraction(as_of, *date, DayCountContext::default())
                        .unwrap();
                    let pv = amount.amount() * discount.df_between_dates(as_of, *date).unwrap();
                    let quoted_pv = pv * (-0.10 * t).exp();
                    npv += pv;
                    quote += quoted_pv;
                    cs01 += quoted_pv * (-0.0001 * t).exp_m1();
                }
            }
            let result = price_instrument_from_json(
                &serde_json::to_string(&pricing.instrument).unwrap(),
                &market,
                &fixture.metadata.valuation_date,
                &pricing.model,
                &["cs01".to_string(), "z_spread".to_string()],
                None,
                None,
            )
            .unwrap();
            assert_close(result.value.amount(), npv, 1e-6);
            assert_close(result.measures["z_spread"], 0.10, 1e-10);
            assert_close(result.measures["cs01"], cs01, 1e-6);
            assert_close(
                deal.instrument_pricing_overrides
                    .market_quotes
                    .quoted_dirty_price_currency
                    .unwrap(),
                quote,
                1e-6,
            );
        }
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

        let registry_result = price_instrument_from_json(
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

        assert_close(discount, -3_082.891_017_534_77, 1e-6);
        assert_close(sofr_3m, 2_836.106_479_169_801, 1e-6);
        // Take the combined target from the fixture rather than repeating the
        // literal here, so a re-blessed fixture cannot leave this test stale.
        assert_close(combined, fixture.expected["dv01"], 1e-6);
        assert_close(combined, registry_dv01, 1e-8);
        // Bumping both curves together is not exactly the sum of the two
        // single-curve bumps: the OC/IC triggers divert cash discontinuously in
        // rates, so the legs carry a small cross-term. Bound it relative to the
        // leg size instead of in absolute currency.
        let cross_term = (combined - (discount + sofr_3m)).abs();
        assert!(
            cross_term < 1e-5 * discount.abs(),
            "curve-leg cross-term {cross_term:.15} exceeds 1e-5 of the discount leg"
        );
        assert!((combined - discount).abs() > 2_000.0);
    }
}
