//! Public-engine regressions for resolved calibration dependencies.

use super::execute;
use crate::api::errors::EnvelopeError;
use crate::api::market_datum::MarketDatum;
use crate::api::prior_market::PriorMarketObject;
use crate::api::schema::{CalibrationEnvelope, StepParams};
use crate::api::validate::{dry_run, validate, CalibrationValidationReport};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use time::macros::date;

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn swaption_fixture() -> TestResult<CalibrationEnvelope> {
    Ok(serde_json::from_str(include_str!(
        "../../examples/market_bootstrap/07_swaption_vol_surface.json"
    ))?)
}

fn assert_parallel_matches_sequential(mut envelope: CalibrationEnvelope) -> TestResult {
    envelope.plan.settings.use_parallel = false;
    let sequential = execute(&envelope)?;
    assert!(sequential.result.report.success);
    envelope.plan.settings.use_parallel = true;
    let parallel = execute(&envelope)?;
    assert!(parallel.result.report.success);

    let sequential = MarketContext::try_from(sequential.result.final_market)?;
    let parallel = MarketContext::try_from(parallel.result.final_market)?;
    for time in [0.25, 1.0, 2.0, 5.0, 10.0] {
        assert!(
            (sequential.get_discount("USD-OIS")?.df(time)
                - parallel.get_discount("USD-OIS")?.df(time))
            .abs()
                <= 1e-12,
            "parallel discount factor differs at t={time}"
        );
        assert!(
            (sequential.get_forward("USD-SOFR-3M")?.rate(time)
                - parallel.get_forward("USD-SOFR-3M")?.rate(time))
            .abs()
                <= 1e-12,
            "parallel projection differs at t={time}"
        );
    }

    // Checking only the final forward curve misses a stale dependency read:
    // both paths can install the new curve while the parallel cube retains
    // forwards and smile parameters calibrated against the previous curve.
    let sequential_cube = sequential.get_vol_cube("USD-SWAPTION-NORMAL-VOL")?;
    let parallel_cube = parallel.get_vol_cube("USD-SWAPTION-NORMAL-VOL")?;
    assert_eq!(
        serde_json::to_value(sequential_cube.as_ref())?,
        serde_json::to_value(parallel_cube.as_ref())?,
        "parallel swaption calibration must consume the same rebuilt dependencies"
    );
    Ok(())
}

#[test]
fn swaption_example_has_identical_sequential_and_parallel_outputs() -> TestResult {
    let envelope = swaption_fixture()?;
    let validation = validate(&envelope);
    assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    let swaption_node = &validation.dependency_graph.nodes[2];
    assert!(swaption_node.reads.iter().any(|id| id == "USD-OIS"));
    assert!(swaption_node.reads.iter().any(|id| id == "USD-SOFR-3M"));
    assert_parallel_matches_sequential(envelope)
}

#[test]
fn prior_projection_does_not_bypass_an_earlier_recalibration_writer() -> TestResult {
    let mut envelope = swaption_fixture()?;
    let stale_forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(date!(2026 - 05 - 08))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.01), (30.0, 0.01)])
        .build()?;
    envelope
        .prior_market
        .push(PriorMarketObject::ForwardCurve(stale_forward));

    // The forward ID is already available before execution, but its earlier
    // plan step must finish before the swaption step reads that same ID.
    let validation = validate(&envelope);
    assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    assert!(validation
        .dependency_graph
        .initial_ids
        .iter()
        .any(|id| id == "USD-SOFR-3M"));
    assert_parallel_matches_sequential(envelope)
}

#[test]
fn xccy_dependencies_include_both_convention_projection_curves() -> TestResult {
    let base_date = date!(2025 - 01 - 02);
    let mut envelope: CalibrationEnvelope = serde_json::from_value(serde_json::json!({
        "schema": "finstack_quant.calibration/1",
        "plan": {
            "id": "xccy-dependencies",
            "quote_sets": {"xccy-quotes": ["EURUSD-XCCY-5Y"]},
            "steps": [{
                "id": "eur-xccy",
                "quote_set": "xccy-quotes",
                "kind": "xccy_basis",
                "curve_id": "EUR-OIS",
                "currency": "EUR",
                "base_date": "2025-01-02",
                "fx_spot": 1.1,
                "domestic_discount_id": "USD-OIS"
            }],
            "settings": {}
        },
        "market_data": [{
            "kind": "xccy_quote",
            "id": "EURUSD-XCCY-5Y",
            "convention": "EUR/USD-XCCY",
            "far_pillar": {"tenor": {"count": 5, "unit": "years"}},
            "basis_spread_bp": -10.0,
            "spot_fx": 1.1
        }]
    }))?;
    envelope.prior_market.push(PriorMarketObject::DiscountCurve(
        DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (10.0, 0.8)])
            .build()?,
    ));
    for id in ["EUR-ESTR-OIS", "USD-SOFR-OIS"] {
        envelope.prior_market.push(PriorMarketObject::ForwardCurve(
            ForwardCurve::builder(id, 0.25)
                .base_date(base_date)
                .knots([(0.0, 0.02), (10.0, 0.02)])
                .build()?,
        ));
    }

    let complete = validate(&envelope);
    assert!(complete.errors.is_empty(), "{:?}", complete.errors);
    let mut reads = complete.dependency_graph.nodes[0].reads.clone();
    reads.sort();
    assert_eq!(reads, ["EUR-ESTR-OIS", "USD-OIS", "USD-SOFR-OIS"]);

    for missing_id in ["EUR-ESTR-OIS", "USD-SOFR-OIS"] {
        let mut incomplete = envelope.clone();
        incomplete
            .prior_market
            .retain(|object| object.id() != missing_id);
        let report = validate(&incomplete);
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(matches!(
            &report.errors[0],
            EnvelopeError::MissingDependency {
                step_index: 0,
                step_id,
                missing_id: actual,
                ..
            } if step_id == "eur-xccy" && actual == missing_id
        ));

        // The JSON/bindings preflight must expose the same missing dependency
        // before any XCCY instrument is priced or any solver is started.
        let wire_report: CalibrationValidationReport =
            serde_json::from_str(&dry_run(&serde_json::to_string(&incomplete)?)?)?;
        assert_eq!(wire_report.errors, report.errors);
    }
    Ok(())
}

fn equity_dependency_fixture(kind: &str) -> TestResult<CalibrationEnvelope> {
    let mut step = serde_json::json!({
        "id": "equity-vol",
        "quote_set": "equity-quotes",
        "kind": kind,
        "vol_surface_id": "AAPL-VOL",
        "base_date": "2025-01-02",
        "underlying_ticker": "AAPL",
        "discount_curve_id": "USD-OIS",
        "target_expiries": [1.0],
        "target_strikes": [80.0, 90.0, 100.0, 110.0, 120.0]
    });
    if kind == "vol_surface" {
        step["model"] = serde_json::json!("sabr");
    }
    let mut envelope: CalibrationEnvelope = serde_json::from_value(serde_json::json!({
        "schema": "finstack_quant.calibration/1",
        "plan": {
            "id": "equity-dependencies",
            "quote_sets": {"equity-quotes": ["AAPL-1Y-100"]},
            "steps": [step],
            "settings": {}
        },
        "market_data": [
            {
                "kind": "vol_quote",
                "option_vol": {
                    "id": "AAPL-1Y-100",
                    "underlying": "AAPL",
                    "expiry": "2026-01-02",
                    "strike": 100.0,
                    "vol": 0.20,
                    "option_type": "call"
                }
            },
            {"kind": "price", "id": "AAPL", "scalar": {"unitless": 100.0}},
            {"kind": "price", "id": "AAPL-DIVYIELD", "scalar": {"unitless": 0.01}}
        ]
    }))?;
    envelope.prior_market.push(PriorMarketObject::DiscountCurve(
        DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 02))
            .knots([(0.0, 1.0), (10.0, 0.8)])
            .build()?,
    ));
    Ok(envelope)
}

fn assert_only_missing_dependency(envelope: &CalibrationEnvelope, expected: &str) {
    let report = validate(envelope);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert!(matches!(
        &report.errors[0],
        EnvelopeError::MissingDependency { missing_id, .. } if missing_id == expected
    ));
}

fn equity_cash_dividends() -> TestResult<MarketDatum> {
    // The provider's schedule identifier need not contain the equity ticker.
    Ok(serde_json::from_value(serde_json::json!({
        "kind": "dividend_schedule",
        "schedule": {
            "id": "PAYMENTS-42",
            "underlying": "AAPL",
            "currency": "USD",
            "events": [{
                "date": "2025-07-02",
                "kind": {"cash": {"amount": "0.25", "currency": "USD"}}
            }]
        }
    }))?)
}

#[test]
fn equity_dependencies_require_market_spot_and_carry_unless_overridden() -> TestResult {
    for kind in ["vol_surface", "svi_surface"] {
        let envelope = equity_dependency_fixture(kind)?;
        let complete = validate(&envelope);
        assert!(complete.errors.is_empty(), "{:?}", complete.errors);
        assert_eq!(
            complete.dependency_graph.nodes[0].reads,
            ["AAPL", "AAPL-DIVYIELD", "USD-OIS"]
        );
        for missing_id in ["AAPL", "AAPL-DIVYIELD"] {
            let mut incomplete = envelope.clone();
            incomplete
                .market_data
                .retain(|datum| datum.id() != missing_id);
            assert_only_missing_dependency(&incomplete, missing_id);
        }

        let mut overridden = envelope;
        overridden
            .market_data
            .retain(|datum| datum.id() != "AAPL" && datum.id() != "AAPL-DIVYIELD");
        match &mut overridden.plan.steps[0].params {
            StepParams::VolSurface(params) => {
                params.spot_override = Some(100.0);
                params.dividend_yield_override = Some(0.01);
            }
            StepParams::SviSurface(params) => {
                params.spot_override = Some(100.0);
                params.dividend_yield_override = Some(0.01);
            }
            _ => return Err("fixture must construct an equity volatility step".into()),
        }
        let report = validate(&overridden);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.dependency_graph.nodes[0].reads, ["USD-OIS"]);
    }
    Ok(())
}

#[test]
fn equity_carry_dependency_resolves_schedule_id_and_prefers_existing_yield() -> TestResult {
    for kind in ["vol_surface", "svi_surface"] {
        let mut envelope = equity_dependency_fixture(kind)?;
        envelope.market_data.push(equity_cash_dividends()?);
        let scalar_preferred = validate(&envelope);
        assert!(scalar_preferred.errors.is_empty());
        assert_eq!(
            scalar_preferred.dependency_graph.nodes[0].reads,
            ["AAPL", "AAPL-DIVYIELD", "USD-OIS"]
        );

        envelope
            .market_data
            .retain(|datum| datum.id() != "AAPL-DIVYIELD");
        let schedule_fallback = validate(&envelope);
        assert!(
            schedule_fallback.errors.is_empty(),
            "{:?}",
            schedule_fallback.errors
        );
        assert_eq!(
            schedule_fallback.dependency_graph.nodes[0].reads,
            ["AAPL", "PAYMENTS-42", "USD-OIS"]
        );
        let wire_report: CalibrationValidationReport =
            serde_json::from_str(&dry_run(&serde_json::to_string(&envelope)?)?)?;
        assert!(wire_report.errors.is_empty());
        assert_eq!(
            wire_report.dependency_graph.nodes[0].reads,
            schedule_fallback.dependency_graph.nodes[0].reads
        );
    }
    Ok(())
}

#[test]
fn svi_without_discount_requires_yield_even_when_cash_dividends_exist() -> TestResult {
    let mut envelope = equity_dependency_fixture("svi_surface")?;
    if let StepParams::SviSurface(params) = &mut envelope.plan.steps[0].params {
        params.discount_curve_id = None;
    }
    envelope.market_data.push(equity_cash_dividends()?);
    envelope
        .market_data
        .retain(|datum| datum.id() != "AAPL-DIVYIELD");
    assert_only_missing_dependency(&envelope, "AAPL-DIVYIELD");
    assert_eq!(
        validate(&envelope).dependency_graph.nodes[0].reads,
        ["AAPL", "AAPL-DIVYIELD"]
    );
    Ok(())
}

#[test]
fn hull_white_dependency_outputs_use_canonical_parameter_keys() -> TestResult {
    let swaption_params: StepParams = serde_json::from_value(serde_json::json!({
        "kind": "hull_white",
        "curve_id": "USD-OIS",
        "currency": "USD",
        "base_date": "2025-01-02",
        "fit_tolerance": 1e-4
    }))?;
    assert_eq!(
        swaption_params.io().writes,
        ["USD-OIS_HW1F_KAPPA", "USD-OIS_HW1F_SIGMA"]
    );

    for (mode, volatility_key) in [
        ("scalar", "USD-OIS_CAPFLOOR_HW1F_SIGMA"),
        ("piecewise", "USD-OIS_CAPFLOOR_HW1F_SIGMA_SCHEDULE"),
    ] {
        let params: StepParams = serde_json::from_value(serde_json::json!({
            "kind": "cap_floor_hull_white",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
            "currency": "USD",
            "base_date": "2025-01-02",
            "fit_tolerance": 1e-4,
            "index_id": "USD-SOFR-3M",
            "volatility_mode": mode
        }))?;
        assert_eq!(
            params.io().writes,
            ["USD-OIS_CAPFLOOR_HW1F_KAPPA", volatility_key]
        );
    }
    Ok(())
}
