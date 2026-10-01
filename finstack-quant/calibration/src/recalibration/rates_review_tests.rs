//! Public-provider regressions for quote replay economics and composition.

#![allow(clippy::expect_used)]

use super::*;
use crate::api::engine::execute_json;
use crate::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::dates::{adjust, calendar_by_id_strict, DateExt, Tenor};
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationMethod, RateCalibrationPillar, ValidationMode,
};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{ConventionSwapParams, InterestRateSwap};
use finstack_quant_valuations::instruments::{Instrument, PayReceive};
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use finstack_quant_valuations::recalibration::{
    RateMarketRecalibrationRequest, RecalibrationProvider,
};
use serde_json::{json, Value};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn calibrate(input: &str) -> TestResult<Arc<MarketContext>> {
    let result = execute_json(input)?;
    for (step, report) in &result.result.step_reports {
        assert!(report.success, "fixture step {step} failed: {report:?}");
    }
    Ok(Arc::new(MarketContext::try_from(
        result.result.final_market,
    )?))
}

fn joint_swap_fixture() -> TestResult<Value> {
    let mut input: Value = serde_json::from_str(include_str!(
        "../../examples/market_bootstrap/02_usd_3m_forward_curve.json"
    ))?;
    input["plan"]["quote_sets"]["sofr_3m_quotes"] = json!(["FWD-1Y", "FWD-2Y", "FWD-5Y"]);
    let quotes = input["market_data"]
        .as_array_mut()
        .expect("example market data is an array");
    quotes.retain(|quote| quote["type"] != "fra");
    for (years, rate) in [(1, 0.02), (2, 0.04), (5, 0.06)] {
        quotes.push(json!({
            "kind": "rate_quote",
            "type": "swap",
            "id": format!("FWD-{years}Y"),
            "index": "USD-SOFR-3M",
            "pillar": {"tenor": {"count": years, "unit": "years"}},
            "rate": rate
        }));
    }
    Ok(input)
}

#[test]
fn linked_rate_replay_reprices_bumped_swaps_with_bumped_discounting() -> TestResult {
    let mut input = joint_swap_fixture()?;
    let source = calibrate(&input.to_string())?;
    for quote in input["market_data"]
        .as_array_mut()
        .expect("fixture market data is an array")
    {
        quote["rate"] = json!(quote["rate"].as_f64().expect("fixture rate") + 1e-4);
    }
    let direct = calibrate(&input.to_string())?;
    let provider = CachedRecalibrationProvider::new();
    let replay =
        provider.rebuild_rate_market(&RateMarketRecalibrationRequest::LinkedDiscountForward {
            market: Arc::clone(&source),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            bump: QuoteBump::ParallelBp(1.0),
        })?;

    // These are the same spot-starting swaps as the calibration quotes. The
    // independent full calibration establishes their expected zero PV before
    // testing the delivered replay market, including its projection curve.
    let as_of = source.get_discount("USD-OIS")?.base_date();
    let registry = ConventionRegistry::try_global()?;
    let convention = registry.require_rate_index(&"USD-SOFR-3M".into())?;
    let calendar = calendar_by_id_strict(&convention.market_calendar_id)?;
    let start = adjust(
        as_of.add_business_days(convention.market_settlement_days, calendar)?,
        convention.market_business_day_convention,
        calendar,
    )?;
    const NOTIONAL: f64 = 1_000_000.0;
    const FIT_TOLERANCE: f64 = 1e-8;
    for (years, fixed_rate) in [(1, 0.0201), (2, 0.0401), (5, 0.0601)] {
        let tenor: Tenor = format!("{years}Y").parse()?;
        let maturity = tenor.add_to_date(
            start,
            Some(calendar),
            convention.market_business_day_convention,
        )?;
        let mut swap = InterestRateSwap::from_conventions(ConventionSwapParams {
            id: format!("FWD-{years}Y").into(),
            notional: Money::new(NOTIONAL, convention.currency)?,
            side: PayReceive::Pay,
            fixed_rate,
            start_date: start,
            maturity,
            index_id: "USD-SOFR-3M",
            discount_curve_id: "USD-OIS",
            forward_curve_id: "USD-SOFR-3M",
        })?;
        swap.fixed_leg.business_day_convention = convention.market_business_day_convention;
        swap.float_leg.business_day_convention = convention.market_business_day_convention;
        swap.fixed_leg.end_of_month = start == start.end_of_month();
        swap.float_leg.end_of_month = start == start.end_of_month();

        let direct_residual = swap.base_value_raw(&direct, as_of)? / NOTIONAL;
        assert!(
            direct_residual.abs() <= FIT_TOLERANCE,
            "direct {years}Y calibration residual: {direct_residual:.12e}"
        );
        let replay_residual = swap.base_value_raw(&replay, as_of)? / NOTIONAL;
        assert!(
            replay_residual.abs() <= FIT_TOLERANCE,
            "replayed {years}Y bumped quote residual: {replay_residual:.12e}; \
             direct calibration residual: {direct_residual:.12e}"
        );
    }
    Ok(())
}

#[test]
fn single_ois_quote_replay_composes_and_preserves_zero_shock_identity() -> TestResult {
    let source = calibrate(include_str!(
        "../../examples/market_bootstrap/01_usd_discount.json"
    ))?;
    let provider = CachedRecalibrationProvider::new();
    let bump = |market: Arc<MarketContext>, bp: f64| {
        provider.rebuild_rate_market(&RateMarketRecalibrationRequest::SingleOis {
            market,
            curve_id: "USD-OIS".into(),
            bump: QuoteBump::ParallelBp(bp),
        })
    };
    let plus_one = bump(Arc::clone(&source), 1.0)?;
    let twice = bump(Arc::clone(&plus_one), 1.0)?;
    let direct_two = bump(Arc::clone(&source), 2.0)?;
    let zero_after = bump(Arc::clone(&plus_one), 0.0)?;
    let restored = bump(Arc::clone(&plus_one), -1.0)?;

    assert!(
        (source.get_discount("USD-OIS")?.df(5.0) - plus_one.get_discount("USD-OIS")?.df(5.0)).abs()
            > 1e-6,
        "fixture must have a material response to the first quote shock"
    );
    for time in [0.25, 1.0, 2.0, 5.0, 10.0] {
        for (label, actual, expected) in [
            ("+1bp then +1bp equals +2bp", &twice, &direct_two),
            (
                "zero shock preserves the bumped market",
                &zero_after,
                &plus_one,
            ),
            (
                "inverse quote shock restores the source",
                &restored,
                &source,
            ),
        ] {
            let actual_df = actual.get_discount("USD-OIS")?.df(time);
            let expected_df = expected.get_discount("USD-OIS")?.df(time);
            assert!(
                (actual_df - expected_df).abs() <= 1e-10,
                "{label} at t={time}: actual={actual_df:.15}, expected={expected_df:.15}"
            );
        }
    }
    Ok(())
}

#[test]
fn linked_rate_quote_replay_composes_both_curve_recipes() -> TestResult {
    let source = calibrate(&joint_swap_fixture()?.to_string())?;
    let provider = CachedRecalibrationProvider::new();
    let bump = |market: Arc<MarketContext>, bp: f64| {
        provider.rebuild_rate_market(&RateMarketRecalibrationRequest::LinkedDiscountForward {
            market,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            bump: QuoteBump::ParallelBp(bp),
        })
    };
    let plus_one = bump(Arc::clone(&source), 1.0)?;
    let twice = bump(Arc::clone(&plus_one), 1.0)?;
    let direct_two = bump(Arc::clone(&source), 2.0)?;
    let zero_after = bump(Arc::clone(&plus_one), 0.0)?;
    let restored = bump(Arc::clone(&plus_one), -1.0)?;

    for time in [0.25, 1.0, 2.0, 3.0, 5.0] {
        for (label, actual, expected) in [
            ("+1bp then +1bp equals +2bp", &twice, &direct_two),
            (
                "zero shock preserves the bumped market",
                &zero_after,
                &plus_one,
            ),
            (
                "inverse quote shock restores the source",
                &restored,
                &source,
            ),
        ] {
            let actual_df = actual.get_discount("USD-OIS")?.df(time);
            let expected_df = expected.get_discount("USD-OIS")?.df(time);
            assert!(
                (actual_df - expected_df).abs() <= 1e-10,
                "{label}, discount at t={time}: actual={actual_df:.15}, expected={expected_df:.15}"
            );
            let actual_forward = actual.get_forward("USD-SOFR-3M")?.rate(time);
            let expected_forward = expected.get_forward("USD-SOFR-3M")?.rate(time);
            assert!(
                (actual_forward - expected_forward).abs() <= 1e-9,
                "{label}, forward at t={time}: actual={actual_forward:.15}, \
                 expected={expected_forward:.15}"
            );
        }
    }
    Ok(())
}

fn discount_floor_fixture(
    rate: f64,
    forward_floor: Option<f64>,
) -> TestResult<(DiscountCurve, RateCalibrationRecipe)> {
    let mut quotes = Vec::new();
    for tenor in ["1Y", "2Y"] {
        quotes.push(RateCalibrationQuote::Deposit {
            index_id: IndexId::new("USD-SOFR-OIS"),
            pillar: RateCalibrationPillar::Tenor(tenor.parse()?),
            rate,
        });
    }
    let recipe = RateCalibrationRecipe {
        currency: Currency::USD,
        method: RateCalibrationMethod::Bootstrap,
        curve_day_count: DayCount::Act365F,
        ois_compounding: None,
        role: RateCalibrationCurveRole::Discount {
            projection_curve_id: CurveId::new("USD-OIS"),
        },
        quotes,
    };
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(Date::from_calendar_date(2025, time::Month::January, 2)?)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-rate).exp()), (2.0, (-2.0 * rate).exp())])
        .rate_calibration(recipe.clone())
        .validation(ValidationMode::Raw {
            allow_non_monotonic: true,
            forward_floor,
        })
        .build()?;
    Ok((curve, recipe))
}

#[test]
fn discount_replay_preserves_absent_forward_floor_in_negative_rate_market() -> TestResult {
    let (source, recipe) = discount_floor_fixture(-0.015, None)?;
    let replay = bump_discount_curve_from_rate_calibration(
        &source,
        &recipe,
        &MarketContext::new(),
        &QuoteBump::ParallelBp(0.0),
    )?;

    assert!(replay.allows_non_monotonic());
    assert_eq!(replay.min_forward_rate(), None);
    assert!(replay.df(2.0) > 1.0);
    for time in [0.0, 0.25, 0.5, 1.0, 1.5, 2.0] {
        assert!(
            (replay.df(time) - source.df(time)).abs() < 1e-14,
            "zero replay changed the negative-rate curve at t={time}"
        );
    }
    Ok(())
}

#[test]
fn discount_replay_preserves_and_enforces_positive_forward_floor() -> TestResult {
    let (source, recipe) = discount_floor_fixture(0.03, Some(0.01))?;
    let replay = bump_discount_curve_from_rate_calibration(
        &source,
        &recipe,
        &MarketContext::new(),
        &QuoteBump::ParallelBp(0.0),
    )?;

    assert!(replay.allows_non_monotonic());
    assert_eq!(replay.min_forward_rate(), Some(0.01));
    for time in [0.0, 0.25, 0.5, 1.0, 1.5, 2.0] {
        assert!(
            (replay.df(time) - source.df(time)).abs() < 1e-14,
            "zero replay changed the positive-rate curve at t={time}"
        );
    }

    let error = bump_discount_curve_from_rate_calibration(
        &source,
        &recipe,
        &MarketContext::new(),
        &QuoteBump::ParallelBp(-300.0),
    )
    .expect_err("the delivered curve must reject forwards below its native 1% floor");
    assert!(
        matches!(
            &error,
            finstack_quant_core::Error::Validation(message)
                if message.contains("below minimum 1.0000%")
        ),
        "expected native curve floor validation, received: {error}"
    );
    Ok(())
}
