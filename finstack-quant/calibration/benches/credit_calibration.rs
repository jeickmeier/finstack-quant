//! Validated full-plan and isolated credit calibration workloads.
//! Parsing and prerequisite solves are outside isolated timings; market restoration,
//! preparation, solving, validation and report construction remain inside.
#[path = "support/checked_calibration.rs"]
mod checked;
use checked::checked_calibrate;

use criterion::{Criterion, criterion_group, criterion_main};
use finstack_quant_calibration::api::{
    engine, prior_market::PriorMarketObject, schema::CalibrationEnvelope,
};
use finstack_quant_calibration::recalibration::bump_hazard_spreads;
use finstack_quant_core::{
    contract::LoadLimits,
    market_data::context::{CurveState, MarketContext},
    types::CurveId,
};
use finstack_quant_valuations_core::{
    market::conventions::ids::CdsDocClause, recalibration::QuoteBump,
};
use std::hint::black_box;

fn load_example(json: &str) -> CalibrationEnvelope {
    CalibrationEnvelope::from_slice_strict(json.as_bytes(), &LoadLimits::default())
        .expect("valid benchmark envelope")
        .0
}

// Optional untimed diagnostics. Save the full output, including per-quote
// residuals and fitted knots, alongside Criterion's raw samples for each slice.
fn record_evidence(envelope: &CalibrationEnvelope) {
    if let Ok(directory) = std::env::var("FQ_CREDIT_EVIDENCE_DIR") {
        let mut explained = envelope.clone();
        explained.plan.settings.explain.enabled = true;
        let output = checked_calibrate(&explained);
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).expect("evidence directory");
        std::fs::write(
            directory.join(format!("{}.json", envelope.plan.id)),
            serde_json::to_vec_pretty(&output).expect("serialize evidence"),
        )
        .expect("save evidence");
    }
}

fn prior_curve(curve: CurveState) -> PriorMarketObject {
    match curve {
        CurveState::Discount(c) => PriorMarketObject::DiscountCurve(c),
        CurveState::Forward(c) => PriorMarketObject::ForwardCurve(c),
        CurveState::Hazard(c) => PriorMarketObject::HazardCurve(c),
        CurveState::Inflation(c) => PriorMarketObject::InflationCurve(c),
        CurveState::BaseCorrelation(c) => PriorMarketObject::BaseCorrelationCurve(c),
        CurveState::Price(c) => PriorMarketObject::PriceCurve(c),
        CurveState::BasisSpread(c) => PriorMarketObject::BasisSpreadCurve(c),
        CurveState::Parametric(c) => PriorMarketObject::ParametricCurve(c),
    }
}

fn isolate_last(envelope: &CalibrationEnvelope) -> Result<CalibrationEnvelope, String> {
    let mut prefix = envelope.clone();
    let last = prefix.plan.steps.pop().ok_or("no steps")?;
    let output = engine::calibrate(&prefix).map_err(|e| format!("prefix failed: {e}"))?;
    assert!(output.result.report.success, "prerequisites must fit");
    let mut isolated = envelope.clone();
    isolated.plan.id.push_str("_isolated_last_step");
    isolated.plan.steps = vec![last.clone()];
    isolated
        .plan
        .quote_sets
        .retain(|key, _| key == &last.quote_set);
    let quote_ids = isolated
        .plan
        .quote_sets
        .get(&last.quote_set)
        .ok_or("missing quote set")?;
    isolated.market_data.retain(|datum| {
        datum.as_quote().is_none() || quote_ids.iter().any(|id| id.as_str() == datum.id())
    });
    isolated.prior_market = output
        .result
        .final_market
        .curves
        .into_iter()
        .map(prior_curve)
        .collect();
    isolated.prior_market.extend(
        output
            .result
            .final_market
            .surfaces
            .into_iter()
            .map(PriorMarketObject::VolSurface),
    );
    Ok(isolated)
}

fn bench_hazard_bootstrap(c: &mut Criterion) {
    let envelope = load_example(include_str!(
        "../examples/market_bootstrap/03_single_name_hazard.json"
    ));

    let isolated = isolate_last(&envelope).expect("isolate calibrated prerequisites");
    record_evidence(&isolated);
    c.bench_function("calibration_hazard_isolated_5_pillars", |b| {
        b.iter(|| checked_calibrate(black_box(&isolated)))
    });

    c.bench_function("calibration_hazard_full_plan_5_pillars", |b| {
        b.iter(|| checked_calibrate(black_box(&envelope)))
    });

    let result = checked_calibrate(&envelope);
    let market =
        MarketContext::try_from(result.result.final_market).expect("market state should rebuild");
    let hazard = market
        .get_hazard("ISSUER-A-CDS")
        .expect("calibrated hazard curve should exist");
    let discount_id = CurveId::new("USD-OIS");

    c.bench_function("calibration_hazard_rebootstrap_parallel_5_pillars", |b| {
        b.iter(|| {
            bump_hazard_spreads(
                black_box(hazard.as_ref()),
                black_box(&market),
                black_box(&QuoteBump::ParallelBp(1.0)),
                black_box(&discount_id),
                Some(CdsDocClause::IsdaNa),
                None,
            )
            .expect("hazard rebootstrap should succeed")
        })
    });
}

fn bench_base_correlation_bootstrap(c: &mut Criterion) {
    let envelope = load_example(include_str!(
        "../examples/market_bootstrap/05_cdx_base_correlation.json"
    ));

    let isolated = isolate_last(&envelope).expect("isolate calibrated prerequisites");
    record_evidence(&isolated);
    c.bench_function("calibration_base_correlation_isolated_5_knots", |b| {
        b.iter(|| checked_calibrate(black_box(&isolated)))
    });

    c.bench_function("calibration_base_correlation_full_plan_5_knots", |b| {
        b.iter(|| checked_calibrate(black_box(&envelope)))
    });
}

fn bench_hazard_variants(c: &mut Criterion) {
    let source = include_str!("../examples/market_bootstrap/03_single_name_hazard.json");
    for variant in ["distressed", "upfront", "long_tenor"] {
        let mut json: serde_json::Value = serde_json::from_str(source).expect("source");
        json["plan"]["id"] = format!("hazard_{variant}").into();
        let mut i = 0;
        for datum in json["market_data"].as_array_mut().expect("market data") {
            if datum["type"] != "cds_par_spread" {
                continue;
            }
            match variant {
                "distressed" => {
                    datum["spread_bp"] = (datum["spread_bp"].as_f64().expect("spread") * 8.0).into()
                }
                "upfront" => {
                    let object = datum.as_object_mut().expect("quote");
                    object.remove("spread_bp");
                    object.insert("type".into(), "cds_upfront".into());
                    object.insert("coupon_bp".into(), 100.0.into());
                    object.insert("upfront_pct".into(), (0.005 + 0.003 * i as f64).into());
                }
                "long_tenor" => {
                    datum["pillar"] = serde_json::json!({"tenor": {"count": ([5,10,15,20,30][i]), "unit": "years"}})
                }
                _ => unreachable!(),
            }
            i += 1;
        }
        assert_eq!(i, 5, "must vary five hazard quotes");
        let envelope = load_example(&serde_json::to_string(&json).expect("envelope"));
        let isolated = isolate_last(&envelope).expect("isolate hazard prerequisites");
        record_evidence(&isolated);
        c.bench_function(
            &format!("calibration_hazard_{variant}_isolated_5_pillars"),
            |b| b.iter(|| checked_calibrate(black_box(&isolated))),
        );
    }
}

criterion_group!(
    benches,
    bench_hazard_bootstrap,
    bench_hazard_variants,
    bench_base_correlation_bootstrap
);
criterion_main!(benches);
