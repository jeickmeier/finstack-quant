//! Validated full-plan and isolated credit calibration workloads.
//! Parsing and prerequisite solves are outside isolated timings; market restoration,
//! preparation, solving, validation and report construction remain inside.
#[path = "../../valuations/benches/support/mod.rs"]
mod bench_support;
#[path = "support/checked_calibration.rs"]
mod checked;
#[path = "../../valuations/benches/support/record.rs"]
mod record;
use checked::checked_calibrate;

use criterion::{Criterion, criterion_group, criterion_main};
use finstack_quant_calibration::api::{
    engine,
    prior_market::PriorMarketObject,
    schema::{CalibrationEnvelope, StepParams},
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

    c.bench_function("calibration_hazard_rebootstrap_parallel_5_pillars", |b| {
        let result = checked_calibrate(&envelope);
        let market = MarketContext::try_from(result.result.final_market)
            .expect("market state should rebuild");
        let hazard = market
            .get_hazard("ISSUER-A-CDS")
            .expect("calibrated hazard curve should exist");
        let discount_id = CurveId::new("USD-OIS");
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
        assert_eq!(envelope.plan.steps.len(), 3);
        bench_support::bench(
            b,
            || checked_calibrate(black_box(&envelope)),
            |output| validate_base_correlation_output(&envelope, output),
        );
    });
}

fn check_calibration_report(report: &finstack_quant_calibration::CalibrationReport) {
    assert!(report.success, "{}", report.convergence_reason);
    assert!(report.validation_passed);
    assert!(report.validation_error.is_none());
    assert!(!report.residuals.is_empty());
    assert!(report.residuals.values().all(|value| value.is_finite()));
    assert!(report.objective_value.is_finite());
    assert!(report.max_residual.is_finite());
    assert!(report.rmse.is_finite());
    assert!(report.max_residual_ratio.is_none_or(f64::is_finite));
    assert!(report.rmse_ratio.is_none_or(f64::is_finite));
    if let Some(tolerance) = report.success_tolerance {
        assert!(tolerance.is_finite() && tolerance > 0.0);
        assert!(report.max_residual <= tolerance);
    }
}

/// Untimed structural checks on the full-plan base-correlation output.
fn validate_base_correlation_output(
    envelope: &CalibrationEnvelope,
    output: &finstack_quant_calibration::api::schema::CalibrationResultEnvelope,
) {
    let result = &output.result;
    check_calibration_report(&result.report);
    assert_eq!(result.step_reports.len(), envelope.plan.steps.len());
    let mut quote_count = 0;
    for step in &envelope.plan.steps {
        let report = result.step_reports.get(&step.id).expect("step report");
        check_calibration_report(report);
        let quotes = envelope
            .plan
            .quote_sets
            .get(&step.quote_set)
            .expect("step quote set");
        assert_eq!(report.residuals.len(), quotes.len());
        assert!(
            quotes
                .iter()
                .all(|id| report.residuals.contains_key(id.as_str()))
        );
        quote_count += quotes.len();
    }
    assert_eq!(quote_count, 16);
    assert_eq!(result.report.residuals.len(), quote_count);
    let params = envelope
        .plan
        .steps
        .iter()
        .find_map(|step| match &step.params {
            StepParams::BaseCorrelation(params) => Some(params),
            _ => None,
        })
        .expect("base-correlation step");
    assert_eq!(params.detachment_points.len(), 5);
    let market = MarketContext::try_from(result.final_market.clone())
        .expect("calibrated market should rebuild");
    let index = market
        .get_credit_index(&params.index_id)
        .expect("calibrated credit index");
    let curve = market
        .get_base_correlation(index.base_correlation_curve().id().as_str())
        .expect("calibrated correlation curve");
    assert_eq!(curve.len(), params.detachment_points.len());
    assert!(curve.validate_shape().is_monotonic);
    for (&actual, &expected) in curve
        .detachment_points()
        .iter()
        .zip(&params.detachment_points)
    {
        assert!((actual - expected * 100.0).abs() < 1e-12);
    }
    assert!(
        curve
            .correlations()
            .iter()
            .all(|rho| rho.is_finite() && (0.0..=1.0).contains(rho))
    );
    assert_eq!(
        index.base_correlation_curve().correlations(),
        curve.correlations()
    );
    assert_eq!(
        index.base_correlation_curve().detachment_points(),
        curve.detachment_points()
    );
    record::capture("calibration_base_correlation_full_plan_5_knots", || {
        serde_json::json!({
            "fixture": {
                "envelope": envelope,
                "timing_boundary": "complete canonical calibration engine, including rate/hazard prerequisite calibration and all final repricing",
                "expected_steps": 3,
                "expected_quotes": 16,
                "expected_correlation_knots": 5,
            },
            "output": output,
        })
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

fn bench_hazard_bucket_batches(c: &mut Criterion) {
    use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
    use finstack_quant_valuations_core::recalibration::{
        HazardRecalibrationAction, HazardRecalibrationRequest, RecalibrationProvider,
    };
    use std::sync::Arc;
    let envelope = load_example(include_str!(
        "../examples/market_bootstrap/03_single_name_hazard.json"
    ));
    let output = checked_calibrate(&envelope);
    let market = Arc::new(MarketContext::try_from(output.result.final_market).expect("market"));
    let hazard = market.get_hazard("ISSUER-A-CDS").expect("hazard");
    let requests: Vec<_> = (0..5)
        .map(|quote_index| HazardRecalibrationRequest {
            hazard: Arc::clone(&hazard),
            source_market: Arc::clone(&market),
            target_market: Arc::clone(&market),
            discount_curve_id: CurveId::new("USD-OIS"),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::ExactQuoteIndexBump {
                quote_index,
                bump_bp: 1.0,
            },
        })
        .collect();
    let mut group = c.benchmark_group("hazard_bucket_batch");
    group.bench_function("one_bump", |b| {
        b.iter(|| {
            let provider = CachedRecalibrationProvider::default();
            black_box(
                provider
                    .rebuild_hazard_curve(black_box(&requests[0]))
                    .expect("one bump"),
            );
        })
    });
    for reversed in [false, true] {
        let order: Vec<_> = if reversed {
            (0..5).rev().collect()
        } else {
            (0..5).collect()
        };
        group.bench_function(
            if reversed {
                "five_reverse"
            } else {
                "five_ascending"
            },
            |b| {
                b.iter(|| {
                    let provider = CachedRecalibrationProvider::default();
                    for &index in &order {
                        black_box(
                            provider
                                .rebuild_hazard_curve(black_box(&requests[index]))
                                .expect("bucket"),
                        );
                    }
                })
            },
        );
    }
    group.bench_function("five_concurrent", |b| {
        b.iter(|| {
            let provider = CachedRecalibrationProvider::default();
            std::thread::scope(|scope| {
                let workers: Vec<_> = requests
                    .iter()
                    .map(|request| {
                        let provider = &provider;
                        scope.spawn(move || {
                            provider
                                .rebuild_hazard_curve(black_box(request))
                                .expect("concurrent bucket")
                        })
                    })
                    .collect();
                for worker in workers {
                    black_box(worker.join().expect("worker"));
                }
            });
        })
    });
    group.finish();
}

fn bench_credit_vol(c: &mut Criterion) {
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_models::volatility::sabr::{SabrCalibrator, SabrParameters};
    let small = isolate_last(&load_example(include_str!(
        "../examples/market_bootstrap/06_cdx_index_vol.json"
    )))
    .expect("isolate vol prerequisites");
    record_evidence(&small);
    c.bench_function("credit_vol/small_3x3", |b| {
        b.iter(|| checked_calibrate(black_box(&small)))
    });
    let mut json = serde_json::to_value(&small).expect("small envelope");
    json["plan"]["id"] = "credit_vol_large_10x21".into();
    let base = Date::from_calendar_date(2026, time::Month::May, 8).expect("base");
    let strikes: Vec<f64> = (0..21).map(|i| 0.005 + i as f64 * 0.0005).collect();
    let expiries: Vec<_> = (1..=10)
        .map(|i| base + time::Duration::days(i * 180))
        .collect();
    let times: Vec<_> = expiries
        .iter()
        .map(|&expiry| {
            DayCount::Act365F
                .year_fraction(base, expiry, DayCountContext::default())
                .expect("time")
        })
        .collect();
    let step = &mut json["plan"]["steps"][0];
    step["discount_curve_id"] = "CREDIT-FLAT".into();
    step["target_expiries"] = serde_json::json!(times);
    step["target_strikes"] = serde_json::json!(strikes);
    let quote_set = step["quote_set"].as_str().expect("set").to_owned();
    let underlying = step["underlying_ticker"]
        .as_str()
        .expect("underlying")
        .to_owned();
    let mut quotes = Vec::new();
    let mut ids = Vec::new();
    for (i, (&expiry, &t)) in expiries.iter().zip(&times).enumerate() {
        let parameters =
            SabrParameters::new(0.28 + 0.005 * i as f64, 1.0, 0.5, -0.25).expect("parameters");
        for (j, &strike) in strikes.iter().enumerate() {
            let id = format!("CREDIT-VOL-{i}-{j}");
            ids.push(id.clone());
            quotes.push(serde_json::json!({"kind":"vol_quote","option_vol":{
                "id":id,"underlying":underlying,"expiry":expiry,"strike":strike,
                "vol":parameters.implied_vol(0.01,strike,t).expect("quote"),"option_type":"call"
            }}));
        }
    }
    json["market_data"] = serde_json::json!(quotes);
    json["plan"]["quote_sets"][&quote_set] = serde_json::json!(ids);
    let discount =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("CREDIT-FLAT")
            .base_date(base)
            .knots([(0.0, 1.0), (10.0, 1.0)])
            .build()
            .expect("flat discount");
    json["prior_market"] =
        serde_json::to_value(vec![PriorMarketObject::DiscountCurve(discount)]).expect("prior");
    let large = load_example(&serde_json::to_string(&json).expect("large JSON"));
    record_evidence(&large);
    c.bench_function("credit_vol/large_10x21", |b| {
        b.iter(|| checked_calibrate(black_box(&large)))
    });
    let truth = SabrParameters::new(0.3, 1.0, 0.5, -0.25).expect("truth");
    let vols: Vec<_> = strikes
        .iter()
        .map(|&k| truth.implied_vol(0.01, k, 2.0).expect("vol"))
        .collect();
    for pinned in [false, true] {
        let calibrator = SabrCalibrator::new().with_atm_pinning(pinned);
        if let Ok(directory) = std::env::var("FQ_CREDIT_EVIDENCE_DIR") {
            let outcome = calibrator
                .calibrate(0.01, &strikes, &vols, 2.0, 1.0)
                .expect("fit");
            let off_grid: Vec<_> = (0..41)
                .map(|i| {
                    let strike = 0.0049 + i as f64 * 0.00026;
                    (
                        strike,
                        outcome
                            .parameters
                            .implied_vol(0.01, strike, 2.0)
                            .expect("off grid"),
                    )
                })
                .collect();
            std::fs::write(
                std::path::Path::new(&directory).join(format!("sabr_pinned_{pinned}.json")),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"outcome":outcome,"off_grid":off_grid}),
                )
                .expect("serialize"),
            )
            .expect("write model evidence");
        }
        c.bench_function(
            if pinned {
                "credit_vol/pinned_21"
            } else {
                "credit_vol/free_21"
            },
            |b| {
                b.iter(|| {
                    calibrator
                        .calibrate(
                            black_box(0.01),
                            black_box(&strikes),
                            black_box(&vols),
                            black_box(2.0),
                            black_box(1.0),
                        )
                        .expect("fit")
                })
            },
        );
    }
}

fn bench_sabr_formula(c: &mut Criterion) {
    use finstack_quant_models::volatility::sabr::SabrParameters;
    for (label, alpha, beta) in [
        ("normal", 0.01, 0.0),
        ("cev", 0.05, 0.5),
        ("lognormal", 0.3, 1.0),
    ] {
        let parameters = SabrParameters::new(alpha, beta, 0.5, -0.25).expect("parameters");
        let strikes: Vec<_> = (0..21).map(|i| 0.005 + i as f64 * 0.0005).collect();
        c.bench_function(&format!("sabr_formula/{label}_21"), |b| {
            b.iter(|| {
                for &strike in &strikes {
                    black_box(
                        parameters
                            .implied_vol(black_box(0.01), black_box(strike), black_box(2.0))
                            .expect("valid vol"),
                    );
                }
            })
        });
    }
}

criterion_group!(
    benches,
    bench_hazard_bootstrap,
    bench_hazard_variants,
    bench_hazard_bucket_batches,
    bench_base_correlation_bootstrap,
    bench_credit_vol,
    bench_sabr_formula
);
criterion_main!(benches);
