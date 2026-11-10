//! Manual numerical records for the frozen five-knot calibration benchmark.
//!
//! This ignored exporter uses the target's canonical quote preparation. It
//! supplies the seven deterministic contracts consumed by the credit-side
//! error-budget exporter, and records local sensitivities at two fixed steps.

use super::*;
use crate::api::{engine, schema::StepParams};
use finstack_quant_core::canonical::content_hash;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_credit::cds_tranche::CdsTranche;
use serde::Serialize;
use serde_json::{Value, json};

const BENCHMARK: &str = "calibration_base_correlation_full_plan_5_knots";
const CORRELATION_STEPS: [f64; 2] = [1e-4, 5e-5];

#[derive(Serialize)]
struct ControlCase {
    id: String,
    role: &'static str,
    quote_id: Option<String>,
    instrument: CdsTranche,
}

fn values(cases: &[ControlCase], market: &MarketContext, as_of: Date) -> Vec<f64> {
    let pricer = CdsTranchePricer::new();
    cases
        .iter()
        .map(|case| {
            let notional = case.instrument.notional.amount();
            assert!(notional.is_finite() && notional > 0.0);
            let value = pricer
                .price_tranche(&case.instrument, market, as_of)
                .expect("calibration control price");
            assert_eq!(value.currency(), case.instrument.notional.currency());
            let normalized = value.amount() / notional;
            assert!(normalized.is_finite());
            normalized
        })
        .collect()
}

fn bumped_market(
    market: &MarketContext,
    params: &BaseCorrelationParams,
    knot: usize,
    offset: f64,
) -> MarketContext {
    let index = market
        .get_credit_index(&params.index_id)
        .expect("calibrated index");
    let original = index.base_correlation_curve();
    let mut correlations = original.correlations().to_vec();
    correlations[knot] += offset;
    assert!((0.0..0.999).contains(&correlations[knot]));
    let curve = Arc::new(
        BaseCorrelationCurve::builder(original.id().as_str())
            .knots(
                original
                    .detachment_points()
                    .iter()
                    .copied()
                    .zip(correlations),
            )
            .build()
            .expect("single-knot correlation stencil"),
    );
    let updated_index = index
        .to_builder()
        .base_correlation_curve(Arc::clone(&curve))
        .build()
        .expect("updated stencil index");
    market
        .clone()
        .insert(curve)
        .insert_credit_index(&params.index_id, updated_index)
        .expect("stencil market")
}

#[test]
#[ignore = "manual B4/BC6 calibration control export; root archives its JSON"]
fn base_correlation_performance_control_export() {
    let envelope = crate::api::validate::parse_envelope(include_str!(
        "../../examples/market_bootstrap/05_cdx_base_correlation.json"
    ))
    .expect("unchanged benchmark envelope");
    let step = envelope
        .plan
        .steps
        .iter()
        .find(|step| matches!(step.params, StepParams::BaseCorrelation(_)))
        .expect("base correlation step");
    let StepParams::BaseCorrelation(params) = &step.params else {
        unreachable!("step selected by variant");
    };
    let result = engine::calibrate(&envelope).expect("complete benchmark calibration");
    assert!(result.result.report.success);
    assert!(result.result.report.validation_passed);
    assert_eq!(result.result.step_reports.len(), 3);
    for report in result.result.step_reports.values() {
        assert!(report.success && report.validation_passed);
        assert!(report.validation_error.is_none());
        assert!(!report.metadata.contains_key("approximate_knots"));
        assert!(
            report
                .residuals
                .values()
                .all(|residual| residual.is_finite())
        );
    }
    let report = result
        .result
        .step_reports
        .get(&step.id)
        .expect("fit report");
    let solver_tolerance = envelope.plan.settings.solver.tolerance();
    assert!(report.max_residual <= solver_tolerance);
    let market =
        MarketContext::try_from(result.result.final_market.clone()).expect("calibrated market");
    let quotes: Vec<_> = envelope
        .market_data
        .iter()
        .filter_map(|datum| match datum.as_quote() {
            Some(MarketQuote::CdsTranche(quote)) => Some(quote),
            _ => None,
        })
        .collect();
    let target = BaseCorrelationTarget::new(params.clone(), market.clone());
    let mut prepared = target
        .prepare_quotes(quotes)
        .expect("canonical quote trades");
    prepared.sort_by(|left, right| left.detachment_pct.total_cmp(&right.detachment_pct));
    assert_eq!(prepared.len(), 5);
    // Rebuild each canonical trade exactly as quote preparation does; the
    // prepared quote retains only the pricing snapshot.
    let build_ctx = target.build_ctx();
    let overrides = target.build_overrides();
    let mut cases: Vec<_> = prepared
        .into_iter()
        .map(|prepared| ControlCase {
            id: prepared.quote.id.as_str().to_string(),
            role: "quoted",
            quote_id: Some(prepared.quote.id.as_str().to_string()),
            instrument: build_cds_tranche_instrument(&prepared.quote, &build_ctx, &overrides)
                .expect("canonical quote trade"),
        })
        .collect();
    for (id, attach_pct, detach_pct) in [
        ("calibration_off_grid_0_5", 0.0, 5.0),
        ("calibration_off_grid_5_12", 5.0, 12.0),
    ] {
        // These additional PV controls keep the first canonical trade's
        // dates, calendar, coupon and notional. They are not quoted trades,
        // so they have no copied market upfront.
        let mut instrument = cases[0].instrument.clone();
        instrument.id = InstrumentId::new(id);
        instrument.attach_pct = attach_pct;
        instrument.detach_pct = detach_pct;
        instrument.upfront = None;
        instrument.validate().expect("off-grid control contract");
        cases.push(ControlCase {
            id: id.to_string(),
            role: "off_grid",
            quote_id: None,
            instrument,
        });
    }
    let baseline_values = values(&cases, &market, params.base_date);
    for (case, value) in cases.iter().zip(&baseline_values).take(5) {
        let residual = report
            .residuals
            .get(&case.id)
            .expect("canonical quote residual");
        assert_eq!(value.to_bits(), residual.to_bits());
    }
    let index = market
        .get_credit_index(&params.index_id)
        .expect("calibrated index");
    let curve = index.base_correlation_curve();
    assert_eq!(curve.len(), 5);
    assert!(curve.validate_shape().is_monotonic);
    let stencils: Vec<Value> = CORRELATION_STEPS
        .into_iter()
        .map(|h| {
            let mut jacobian = vec![vec![0.0; curve.len()]; cases.len()];
            let mut columns = Vec::with_capacity(curve.len());
            for (knot, detachment_percent) in curve.detachment_points().iter().enumerate() {
                let up = values(
                    &cases,
                    &bumped_market(&market, params, knot, h),
                    params.base_date,
                );
                let down = values(
                    &cases,
                    &bumped_market(&market, params, knot, -h),
                    params.base_date,
                );
                for ((row, up), down) in jacobian.iter_mut().zip(&up).zip(&down) {
                    row[knot] = (up - down) / (2.0 * h);
                    assert!(row[knot].is_finite());
                }
                columns.push(json!({
                    "knot": knot,
                    "detachment_percent": detachment_percent,
                    "up_normalized_pvs": up,
                    "down_normalized_pvs": down,
                }));
            }
            json!({
                "step_absolute_correlation": h,
                "jacobian_normalized_pv_per_absolute_correlation": jacobian,
                "columns": columns,
            })
        })
        .collect();
    let fixture = json!({
        "schema_version": 1,
        "benchmark": BENCHMARK,
        "envelope_hash": envelope.content_hash().expect("envelope hash"),
        "as_of": params.base_date,
        "market": market,
        "cases": cases,
    });
    let record = json!({
        "benchmark": BENCHMARK,
        "fixture_hash": content_hash(&fixture).expect("prepared fixture hash"),
        "fixture": fixture,
        "calibration_result": result,
        "knots": {
            "detachment_percent": curve.detachment_points(),
            "correlation_decimal": curve.correlations(),
        },
        "prerequisite_curve_hashes": {
            "discount": content_hash(market.get_discount(&params.discount_curve_id).expect("discount").as_ref()).expect("discount hash"),
            "hazard": content_hash(index.index_credit_curve().as_ref()).expect("hazard hash"),
        },
        "case_order": cases.iter().map(|case| &case.id).collect::<Vec<_>>(),
        "normalized_pvs": baseline_values,
        "normalized_pv_bits": baseline_values.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
        "solver_tolerance": solver_tolerance,
        "sensitivity_convention": "central difference of PV/tranche notional per 1.0 absolute correlation; local diagnostic only, not a certified derivative envelope",
        "stencils": stencils,
    });
    eprintln!("FQ_CALIBRATION_DIAGNOSTIC {record}");
}
