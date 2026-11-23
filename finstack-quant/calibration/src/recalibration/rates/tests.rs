#![allow(clippy::expect_used, clippy::panic)]

use super::discount::*;
use super::forward_ois::*;
use crate::CalibrationConfig;
use crate::api::schema::{DiscountCurveParams, ForwardCurveParams, StepParams};
use crate::config::CalibrationMethod;
use crate::config::RatesStepConventions;
use crate::quotes::MarketQuote;
use crate::quotes::Pillar;
use crate::quotes::QuoteId;
use crate::quotes::RateQuote;
use crate::step_runtime;
use crate::targets::rate_recipe::{original_execution, rate_quotes_from_recipe};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::RateCalibrationQuote;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, RateCalibrationCurveRole, RateCalibrationRecipe,
};
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations_core::recalibration::QuoteBump;
use std::cell::Cell;
use std::sync::Arc;
use time::Duration;

use finstack_quant_core::market_data::term_structures::{
    RateCalibrationMethod, RateCalibrationPillar,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_valuations_core::market::conventions::ids::IrFutureContractId;

fn linked_single_curve_ois_market(
    forward_discount_curve_id: CurveId,
) -> (MarketContext, CurveId, CurveId, Vec<f64>) {
    let base_date = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
    let discount_curve_id = CurveId::new("USD-OIS");
    let forward_curve_id = CurveId::new("USD-SOFR-OIS");
    let index_id = IndexId::new("USD-SOFR-OIS");
    let quotes: Vec<_> = [("6M", 0.0430), ("1Y", 0.0410), ("2Y", 0.0390)]
        .into_iter()
        .map(|(tenor, rate)| RateQuote::Deposit {
            id: QuoteId::new(format!("USD-DEP-{tenor}")),
            index: index_id.clone(),
            pillar: Pillar::Tenor(tenor.parse().expect("valid tenor")),
            rate,
        })
        .collect();
    let params = DiscountCurveParams {
        curve_id: discount_curve_id.clone(),
        currency: Currency::USD,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::LogLinear,
        extrapolation: ExtrapolationPolicy::Extend,
        pricing_discount_id: Some(discount_curve_id.clone()),
        pricing_forward_id: Some(forward_curve_id.clone()),
        conventions: RatesStepConventions {
            ois_compounding: Some(
                finstack_quant_cashflows::builder::FloatingLegCompounding::Simple,
            ),
            curve_day_count: Some(DayCount::Act365F),
        },
    };
    let discount = solve_discount_curve(
        &quotes,
        &params,
        &MarketContext::new(),
        &CalibrationConfig::default(),
    )
    .expect("fit source deposit quotes")
    .to_builder_with_id(discount_curve_id.clone())
    .fx_policy("single_curve_ois::USD")
    .build()
    .expect("discount representation");

    let projection_grid = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    let forward_knots = projection_grid.iter().map(|&time: &f64| {
        let date = base_date + Duration::days((time * 360.0).round() as i64);
        (time, discount.df_on_date(date).expect("projection factor"))
    });

    let mut forward_recipe = discount
        .rate_calibration()
        .expect("executed deposit recipe")
        .clone();
    forward_recipe.method = RateCalibrationMethod::GlobalSolve {
        use_analytical_jacobian: false,
    };
    forward_recipe.role = RateCalibrationCurveRole::Projection {
        discount_curve_id: forward_discount_curve_id,
    };
    let forward = ForwardCurve::builder(forward_curve_id.clone())
        .base_date(base_date)
        .index_tenor(finstack_quant_core::dates::Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .knots(forward_knots)
        .interp(InterpStyle::CubicHermite)
        .extrapolation(ExtrapolationPolicy::Extend)
        .rate_calibration(forward_recipe)
        .fx_policy("single_curve_ois::USD")
        .build()
        .expect("projection representation");

    (
        MarketContext::new()
            .try_insert(discount)
            .expect("insert market curve")
            .try_insert(forward)
            .expect("insert market curve"),
        discount_curve_id,
        forward_curve_id,
        projection_grid,
    )
}

#[test]
fn exact_discount_rate_replay_moves_only_the_matched_quote() {
    let base = time::macros::date!(2025 - 01 - 02);
    let short = time::macros::date!(2025 - 07 - 02);
    let long = time::macros::date!(2026 - 01 - 02);
    let params = DiscountCurveParams {
        curve_id: CurveId::new("GBP-SONIA"),
        currency: Currency::GBP,
        base_date: base,
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::LogLinear,
        extrapolation: ExtrapolationPolicy::Extend,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            curve_day_count: Some(DayCount::Act365F),
            ois_compounding: None,
        },
    };
    let quotes: Vec<_> = [("SHORT", short, 0.041), ("LONG", long, 0.057)]
        .into_iter()
        .map(|(id, end, rate)| RateQuote::Deposit {
            id: QuoteId::new(id),
            index: IndexId::new("GBP-SONIA-OIS"),
            pillar: Pillar::Date(end),
            rate,
        })
        .collect();
    let context = MarketContext::new();
    let config = CalibrationConfig::default();
    let center = solve_discount_curve(&quotes, &params, &context, &config).unwrap();
    let target = CalibrationQuoteCoordinate {
        quote_id: "LONG".into(),
        pillar_date: long,
    };
    let raised = bump_discount_rate_quote(&center, &context, &target, 1.0).unwrap();
    let expected = [quotes[0].clone(), quotes[1].bump_rate_bp(1.0)];
    let rebuilt = solve_discount_curve(&expected, &params, &context, &config).unwrap();
    assert_eq!(raised.dfs(), rebuilt.dfs());
    assert_eq!(
        raised.df_on_date(short).unwrap(),
        center.df_on_date(short).unwrap()
    );
    let actual = raised.df_on_date(long).unwrap() - center.df_on_date(long).unwrap();
    let expected = (1.0_f64 / 1.0571) - (1.0_f64 / 1.057);
    assert!((actual - expected).abs() < 1e-10);
    let wrong_date = CalibrationQuoteCoordinate {
        pillar_date: short,
        ..target
    };
    assert!(
        bump_discount_rate_quote(&center, &context, &wrong_date, 1.0)
            .unwrap_err()
            .to_string()
            .contains("resolved pillar")
    );
}

#[test]
fn exact_shared_ois_rate_reprices_one_quote_in_both_views() {
    let (market, discount_id, forward_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let discount = market
        .get_discount(discount_id.as_str())
        .expect("center discount");
    let recipe = required_discount_rate_calibration(&discount).expect("discount recipe");
    let row = original_execution(recipe)
        .expect("execution")
        .bindings
        .iter()
        .find(|binding| binding.quote_id == "USD-DEP-1Y")
        .expect("qualified 1Y binding");
    let quote = CalibrationQuoteCoordinate {
        quote_id: row.quote_id.clone(),
        pillar_date: row.pillar_date,
    };

    let shocked = bump_linked_ois_rate_quote(&market, &discount_id, &forward_id, &quote, 1.0)
        .expect("one selected OIS quote shock");
    let raised = shocked
        .get_discount(discount_id.as_str())
        .expect("shocked discount");
    let projected = shocked
        .get_forward(forward_id.as_str())
        .expect("shocked projection");
    assert!(
        raised.df_on_date(quote.pillar_date).expect("raised DF")
            < discount.df_on_date(quote.pillar_date).expect("original DF")
    );
    for (time, factor) in projected.knots().iter().zip(projected.factors()) {
        let date = date_from_forward_time(&projected, *time).expect("projection date");
        assert_eq!(
            *factor,
            raised.df_on_date(date).expect("projected discount factor")
        );
    }
    let wrong = CalibrationQuoteCoordinate {
        quote_id: "USD-DEP-2Y".into(),
        ..quote
    };
    assert!(bump_linked_ois_rate_quote(&market, &discount_id, &forward_id, &wrong, 1.0).is_err());
}

#[test]
fn linked_single_curve_ois_quote_shock_derives_projection_from_discount() {
    let (market, discount_curve_id, forward_curve_id, projection_grid) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));

    let shocked = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(5.0),
    )
    .expect("linked OIS quote shock");
    let shocked_discount = shocked
        .get_discount(discount_curve_id.as_str())
        .expect("shocked discount representation");
    let shocked_forward = shocked
        .get_forward(forward_curve_id.as_str())
        .expect("shocked projection representation");
    let source_discount = market
        .get_discount(discount_curve_id.as_str())
        .expect("source discount representation");
    let source_forward = market
        .get_forward(forward_curve_id.as_str())
        .expect("source projection representation");

    for period in projection_grid.windows(2) {
        let (start, end) = (period[0], period[1]);
        let start_date =
            shocked_forward.base_date() + Duration::days((start * 360.0).round() as i64);
        let end_date = shocked_forward.base_date() + Duration::days((end * 360.0).round() as i64);
        let discount_implied = (shocked_discount
            .df_on_date(start_date)
            .expect("discount factor on reset date")
            / shocked_discount
                .df_on_date(end_date)
                .expect("discount factor on payment date")
            - 1.0)
            / (end - start);
        let projected = shocked_forward
            .rate_between(start, end, end - start)
            .expect("projection-grid forward");
        assert!(
            (projected - discount_implied).abs() < 1e-12,
            "linked OIS representations diverged over [{start}, {end}]: \
             projection={projected:.12}, discount-implied={discount_implied:.12}"
        );
    }
    assert!(
        (shocked_discount.df(1.0).expect("valid") - source_discount.df(1.0).expect("valid")).abs()
            > 1e-8,
        "non-zero shared quote shock must move the linked discount curve"
    );

    assert_eq!(shocked_forward.id(), source_forward.id());
    assert_eq!(shocked_forward.knots(), source_forward.knots());
    assert_eq!(shocked_forward.index_tenor(), source_forward.index_tenor());
    assert_eq!(shocked_forward.day_count(), source_forward.day_count());
    assert_eq!(
        shocked_forward.interp_style(),
        source_forward.interp_style()
    );
    assert_eq!(
        shocked_forward.extrapolation(),
        source_forward.extrapolation()
    );
    assert_eq!(shocked_forward.fx_policy(), source_forward.fx_policy());
    let mut expected_recipe = source_forward
        .rate_calibration()
        .expect("source recipe")
        .clone();
    expected_recipe.quotes = shocked_discount
        .rate_calibration()
        .expect("shocked discount recipe")
        .quotes
        .clone();
    assert_ne!(
        expected_recipe.quotes,
        source_forward
            .rate_calibration()
            .expect("source recipe")
            .quotes
    );
    assert_eq!(shocked_forward.rate_calibration(), Some(&expected_recipe));
}

#[test]
fn linked_ois_projection_uses_forward_grid_dates_across_day_counts() {
    let (market, discount_curve_id, forward_curve_id, projection_grid) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));

    let shocked = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(5.0),
    )
    .expect("linked OIS quote shock");
    let shocked_discount = shocked
        .get_discount(discount_curve_id.as_str())
        .expect("shocked discount representation");
    let shocked_forward = shocked
        .get_forward(forward_curve_id.as_str())
        .expect("shocked projection representation");

    assert_eq!(shocked_forward.day_count(), DayCount::Act360);
    assert_eq!(shocked_discount.day_count(), DayCount::Act365F);
    for period in projection_grid.windows(2) {
        let (start, end) = (period[0], period[1]);
        let start_date =
            shocked_forward.base_date() + Duration::days((start * 360.0).round() as i64);
        let end_date = shocked_forward.base_date() + Duration::days((end * 360.0).round() as i64);
        let date_implied = (shocked_discount
            .df_on_date(start_date)
            .expect("discount factor on reset date")
            / shocked_discount
                .df_on_date(end_date)
                .expect("discount factor on payment date")
            - 1.0)
            / (end - start);
        let projected = shocked_forward
            .rate_between(start, end, end - start)
            .expect("projection-grid forward");
        assert!(
            (projected - date_implied).abs() < 1e-12,
            "mixed-day-count linked projection diverged over {start_date}..{end_date}: \
             projection={projected:.12}, date-implied={date_implied:.12}"
        );
    }
}

#[test]
fn linked_replay_rejects_a_projection_inconsistent_with_its_source_discount() {
    let (market, discount_id, forward_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let source = market.get_forward(forward_id.as_str()).unwrap();
    let altered = source
        .rebuild_with_knots(
            source
                .knots()
                .iter()
                .zip(source.factors())
                .map(|(&time, &df)| (time, if time > 0.0 { df * 0.95 } else { df })),
        )
        .unwrap()
        .to_builder_with_id(source.id().clone())
        .rate_calibration(source.rate_calibration().unwrap().clone())
        .build()
        .unwrap();
    let error = bump_market_via_rate_quote_shock(
        &market.try_insert(altered).expect("insert market curve"),
        &discount_id,
        &forward_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .unwrap_err();
    assert!(error.to_string().contains("source factor"), "{error}");
}

#[test]
fn linked_ois_quote_shock_verifies_source_before_calibrating_target() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(0));

    let shocked = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(5.0),
    )
    .expect("linked OIS quote shock");

    assert_eq!(
        DISCOUNT_CALIBRATION_RUNS.with(Cell::get),
        2,
        "one solve verifies the source, one fits the shocked quotes"
    );
    assert_eq!(
        shocked
            .get_discount(discount_curve_id.as_str())
            .expect("shocked discount curve")
            .knots(),
        market
            .get_discount(discount_curve_id.as_str())
            .expect("source discount curve")
            .knots(),
        "single calibration must still be sampled on the source discount grid"
    );
}

#[test]
fn rate_recalibration_cache_reuses_identical_market_shock() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let cache = RateRecalibrationCache::default();
    DISCOUNT_CALIBRATION_RUNS.with(|runs| runs.set(0));

    let first = bump_market_via_rate_quote_shock_cached(
        &cache,
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(5.0),
    )
    .expect("first cached quote shock");
    let second = bump_market_via_rate_quote_shock_cached(
        &cache,
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(5.0),
    )
    .expect("reused cached quote shock");

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(
        DISCOUNT_CALIBRATION_RUNS.with(Cell::get),
        2,
        "identical requests share both verification and shocked calibration"
    );
}

#[test]
fn linked_rate_cache_preserves_each_source_market() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let (market, discount_id, forward_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let first_market = market.clone().insert_price(
        "EQUITY",
        MarketScalar::unitless(100.0).expect("valid scalar"),
    );
    let second_market = market.insert_price(
        "EQUITY",
        MarketScalar::unitless(200.0).expect("valid scalar"),
    );
    let cache = RateRecalibrationCache::default();
    let bump = QuoteBump::ParallelBp(1.0);
    let first = bump_market_via_rate_quote_shock_cached(
        &cache,
        &first_market,
        &discount_id,
        &forward_id,
        &bump,
    )
    .expect("first market replay");
    let second = bump_market_via_rate_quote_shock_cached(
        &cache,
        &second_market,
        &discount_id,
        &forward_id,
        &bump,
    )
    .expect("second market replay");
    assert!(
        first
            .get_price("EQUITY")
            .is_ok_and(|s| s.get_unitless() == Some(100.0))
    );
    assert!(
        second
            .get_price("EQUITY")
            .is_ok_and(|s| s.get_unitless() == Some(200.0))
    );
    assert!(!Arc::ptr_eq(&first, &second));
}

fn single_ois_example_market(quote_shift: f64) -> MarketContext {
    let mut envelope: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/market_bootstrap/01_usd_discount.json"
    ))
    .expect("example envelope");
    for quote in envelope["market_data"].as_array_mut().expect("quotes") {
        quote["rate"] = serde_json::json!(quote["rate"].as_f64().expect("rate") + quote_shift);
    }
    let result = crate::api::engine::calibrate_from_json(&envelope.to_string())
        .expect("calibrated single-OIS market");
    MarketContext::try_from(result.result.final_market).expect("materialized market")
}

#[test]
fn single_ois_cache_uses_current_source_quotes() {
    let first_market = single_ois_example_market(0.0);
    let second_market = single_ois_example_market(0.01);
    let cache = RateRecalibrationCache::default();
    let id = CurveId::new("USD-OIS");
    let bump = QuoteBump::ParallelBp(1.0);
    let first =
        bump_single_ois_market_via_rate_quote_shock_cached(&cache, &first_market, &id, &bump)
            .expect("first market replay");
    let second =
        bump_single_ois_market_via_rate_quote_shock_cached(&cache, &second_market, &id, &bump)
            .expect("second market replay");
    let expected = bump_single_ois_market_via_rate_quote_shock(&second_market, &id, &bump)
        .expect("independent second market replay");
    let first_df = first
        .get_discount(&id)
        .expect("first curve")
        .df(1.0)
        .expect("df");
    let second_df = second
        .get_discount(&id)
        .expect("second curve")
        .df(1.0)
        .expect("df");
    let expected_df = expected
        .get_discount(&id)
        .expect("expected curve")
        .df(1.0)
        .expect("df");
    assert!((second_df - expected_df).abs() < 1e-12);
    assert!((second_df - first_df).abs() > 1e-3);
}

#[test]
fn discount_cache_uses_source_curve_and_explicit_recipe() {
    let market = single_ois_example_market(0.0);
    let source = market.get_discount("USD-OIS").expect("source curve");
    let recipe = source.rate_calibration().expect("recipe");
    let different_curve = source
        .rebuild_with_knots(
            source
                .knots()
                .iter()
                .zip(source.dfs())
                .map(|(&t, &df)| (t, df * (-0.01 * t).exp())),
        )
        .expect("different stored source curve");
    let cache = RateRecalibrationCache::default();
    let bump = QuoteBump::ParallelBp(1.0);
    let first = bump_discount_curve_from_rate_calibration_cached(
        &cache,
        source.as_ref(),
        recipe,
        &market,
        &bump,
    )
    .expect("first source curve replay");
    let different = bump_discount_curve_from_rate_calibration_cached(
        &cache,
        &different_curve,
        recipe,
        &market,
        &bump,
    )
    .expect_err("stored curve differs from quote fit");
    assert!(different.to_string().contains("source factor"));
    assert!(first.df(1.0).expect("valid").is_finite());

    let mut different_recipe = recipe.clone();
    for quote in &mut different_recipe.quotes {
        if let RateCalibrationQuote::Deposit { rate, .. }
        | RateCalibrationQuote::Swap { rate, .. } = quote
        {
            *rate += 0.01;
        }
    }
    let mismatch = bump_discount_curve_from_rate_calibration_cached(
        &cache,
        source.as_ref(),
        &different_recipe,
        &market,
        &bump,
    )
    .expect_err("a different quote record cannot describe this source");
    assert!(mismatch.to_string().contains("source factor"));
}

#[test]
fn rate_replay_preserves_observed_fixings_and_does_not_publish_seeds() {
    let (market, discount_id, forward_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let base_date = market
        .get_discount(&discount_id)
        .expect("curve")
        .base_date();
    let observed = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-OIS",
        vec![
            (base_date - Duration::days(30), 0.066),
            (base_date - Duration::days(1), 0.067),
        ],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("observed fixing history");
    let with_history = market.clone().insert_series(observed.clone());
    for bump in [QuoteBump::ParallelBp(0.0), QuoteBump::ParallelBp(1.0)] {
        let replayed =
            bump_market_via_rate_quote_shock(&with_history, &discount_id, &forward_id, &bump)
                .expect("linked market replay");
        assert_eq!(
            replayed
                .get_series(observed.id())
                .expect("preserved history")
                .observations(),
            observed.observations()
        );
        assert!(replayed.get_series("FIXING:USD-OIS").is_err());
    }
    let no_history = bump_market_via_rate_quote_shock(
        &market,
        &discount_id,
        &forward_id,
        &QuoteBump::ParallelBp(0.0),
    )
    .expect("replay with private calibration seeds");
    assert!(no_history.get_series(observed.id()).is_err());

    let single_market = single_ois_example_market(0.0);
    let single_base = single_market
        .get_discount("USD-OIS")
        .expect("curve")
        .base_date();
    let single_history = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-OIS",
        vec![
            (single_base - Duration::days(30), 0.066),
            (single_base - Duration::days(1), 0.067),
        ],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("single-OIS history");
    let single_market = single_market.insert_series(single_history.clone());
    let replayed = bump_single_ois_market_via_rate_quote_shock(
        &single_market,
        &CurveId::new("USD-OIS"),
        &QuoteBump::ParallelBp(0.0),
    )
    .expect("single-OIS replay");
    assert_eq!(
        replayed
            .get_series(single_history.id())
            .expect("preserved single-OIS history")
            .observations(),
        single_history.observations()
    );
    assert!(replayed.get_series("FIXING:USD-OIS").is_err());
}

#[test]
fn malformed_linked_single_curve_ois_recipe_fails_explicitly() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OTHER"));

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("non-reciprocal linked OIS roles must fail");

    assert!(
        error.to_string().contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
}

#[test]
fn forward_declared_ois_link_without_discount_recipe_fails_explicitly() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let discount = market
        .get_discount(discount_curve_id.as_str())
        .expect("discount representation");
    let discount_without_recipe = discount
        .to_builder_with_id(discount_curve_id.clone())
        .rate_calibration_opt(None)
        .build()
        .expect("discount representation without recipe");
    let market = market
        .try_insert(discount_without_recipe)
        .expect("insert market curve");

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("one-sided forward OIS link must fail");

    assert!(
        error.to_string().contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
}

#[test]
fn linkage_validation_precedes_missing_discount_calibration_error() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let discount = market
        .get_discount(discount_curve_id.as_str())
        .expect("discount representation");
    let discount_without_metadata = discount
        .to_builder_with_id(discount_curve_id.clone())
        .rate_calibration_opt(None)
        .build()
        .expect("discount representation without calibration metadata");
    let market = market
        .try_insert(discount_without_metadata)
        .expect("insert market curve");

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("linkage validation must run before calibration validation");
    let message = error.to_string();

    assert!(
        message.contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
    assert!(
        !message.contains("no rate calibration metadata"),
        "generic metadata error escaped before link validation: {error}"
    );
}

#[test]
fn forward_declared_ois_link_with_reverse_mismatch_fails_explicitly() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let discount = market
        .get_discount(discount_curve_id.as_str())
        .expect("discount representation");
    let mut mismatched_recipe = discount
        .rate_calibration()
        .expect("discount recipe")
        .clone();
    mismatched_recipe.role = RateCalibrationCurveRole::Discount {
        projection_curve_id: CurveId::new("USD-OTHER-PROJECTION"),
    };
    let mismatched_discount = discount
        .to_builder_with_id(discount_curve_id.clone())
        .rate_calibration(mismatched_recipe)
        .build()
        .expect("mismatched discount representation");
    let market = market
        .try_insert(mismatched_discount)
        .expect("insert market curve");

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("reverse-mismatched OIS link must fail");

    assert!(
        error.to_string().contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
}

#[test]
fn discount_declared_ois_link_without_forward_recipe_fails_explicitly() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let forward = market
        .get_forward(forward_curve_id.as_str())
        .expect("projection representation");
    let forward_without_recipe = forward
        .to_builder_with_id(forward_curve_id.clone())
        .rate_calibration_opt(None)
        .build()
        .expect("projection representation without recipe");
    let market = market
        .try_insert(forward_without_recipe)
        .expect("insert market curve");

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("one-sided discount OIS link must fail");

    assert!(
        error.to_string().contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
}

#[test]
fn reciprocal_roles_with_one_sided_ois_metadata_fail_explicitly() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let forward = market
        .get_forward(forward_curve_id.as_str())
        .expect("projection representation");
    let mut partial_recipe = forward
        .rate_calibration()
        .expect("projection recipe")
        .clone();
    partial_recipe.ois_compounding = None;
    let partial_forward = forward
        .to_builder_with_id(forward_curve_id.clone())
        .rate_calibration(partial_recipe)
        .build()
        .expect("projection representation with partial OIS metadata");
    let market = market
        .try_insert(partial_forward)
        .expect("insert market curve");

    let error = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("one-sided OIS convention metadata must fail");

    assert!(
        error.to_string().contains("linked single-curve OIS recipe"),
        "unexpected error: {error}"
    );
}

#[test]
fn term_index_relabel_cannot_reuse_an_ois_derived_projection() {
    let (market, discount_curve_id, forward_curve_id, _) =
        linked_single_curve_ois_market(CurveId::new("USD-OIS"));
    let discount = market
        .get_discount(discount_curve_id.as_str())
        .expect("discount representation");
    let mut discount_recipe = discount
        .rate_calibration()
        .expect("discount recipe")
        .clone();
    discount_recipe.role = RateCalibrationCurveRole::Discount {
        projection_curve_id: discount_curve_id.clone(),
    };
    let discount = discount
        .to_builder_with_id(discount_curve_id.clone())
        .rate_calibration(discount_recipe)
        .build()
        .expect("self-projected discount representation");

    let forward = market
        .get_forward(forward_curve_id.as_str())
        .expect("projection representation");
    let mut forward_recipe = forward
        .rate_calibration()
        .expect("projection recipe")
        .clone();
    forward_recipe.ois_compounding = None;
    let forward = forward
        .to_builder_with_id(forward_curve_id.clone())
        .rate_calibration(forward_recipe)
        .build()
        .expect("term-index projection representation");
    let market = market
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(forward)
        .expect("insert market curve");

    let mismatch = bump_market_via_rate_quote_shock(
        &market,
        &discount_curve_id,
        &forward_curve_id,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("independent term-index replay requires an independently fitted source");
    assert!(
        mismatch.to_string().contains("projection")
            || mismatch.to_string().contains("source factor")
    );
}

#[test]
fn parallel_bump_shifts_futures_implied_rate_up() {
    let quotes = vec![
        RateQuote::Deposit {
            id: QuoteId::new("USD-DEP-3M"),
            index: IndexId::new("USD-SOFR-3M"),
            pillar: Pillar::Tenor("3M".parse().expect("valid tenor")),
            rate: 0.05,
        },
        RateQuote::Futures {
            id: QuoteId::new("USD-FUT-SEP26"),
            contract: IrFutureContractId::new("CME:SR3"),
            expiry: Date::from_calendar_date(2026, time::Month::September, 16).expect("valid date"),
            price: 96.00, // implied rate 4%
            convexity_adjustment: 0.0,
        },
        RateQuote::Swap {
            id: QuoteId::new("USD-SWAP-2Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
            rate: 0.045,
            spread_decimal: None,
        },
    ];

    let implied_rate = |q: &RateQuote| -> f64 {
        match q {
            RateQuote::Deposit { rate, .. }
            | RateQuote::Fra { rate, .. }
            | RateQuote::Swap { rate, .. } => *rate,
            RateQuote::Futures { price, .. } => (100.0 - price) / 100.0,
        }
    };
    let base_rates: Vec<f64> = quotes.iter().map(implied_rate).collect();

    let bumped =
        apply_bump_to_rate_quotes(quotes, &QuoteBump::ParallelBp(1.0), &[]).expect("parallel bump");

    for (q, base) in bumped.iter().zip(base_rates.iter()) {
        let moved = implied_rate(q) - base;
        assert!(
            (moved - 1e-4).abs() < 1e-12,
            "{}: implied rate must move +1bp, moved {moved:.8}",
            q.id().as_str()
        );
    }
}

#[test]
fn projection_replay_returns_the_accepted_solve_and_rejects_a_shifted_source() {
    let base_date = time::macros::date!(2025 - 01 - 02);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .expect("discount curve");
    let context = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve");
    let params = ForwardCurveParams {
        curve_id: CurveId::new("USD-TERM"),
        base_date,
        currency: Currency::USD,
        index_tenor: Some(finstack_quant_core::dates::Tenor::quarterly()),
        discount_curve_id: CurveId::new("USD-OIS"),
        method: CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        interpolation: InterpStyle::LogLinear,
        extrapolation: ExtrapolationPolicy::Extend,
        conventions: RatesStepConventions {
            curve_day_count: Some(DayCount::Act360),
            ois_compounding: None,
        },
    };
    let quotes: Vec<_> = [("3M", 0.04), ("6M", 0.042)]
        .into_iter()
        .map(|(tenor, rate)| RateQuote::Deposit {
            id: QuoteId::new(format!("SOFR-{tenor}")),
            index: IndexId::new("USD-SOFR-3M"),
            pillar: Pillar::Tenor(tenor.parse().unwrap()),
            rate,
        })
        .collect();
    let config = CalibrationConfig::default();
    let source = rebootstrap_forward_curve(quotes.clone(), &params, &context, &config).unwrap();
    let source = source
        .to_builder_with_id(source.id().clone())
        .fx_policy("projection-policy")
        .build()
        .unwrap();
    let record = required_forward_rate_calibration(&source).unwrap();
    let bump = QuoteBump::ParallelBp(1.0);
    let shifted = source
        .rebuild_with_knots(
            source
                .knots()
                .iter()
                .zip(source.factors())
                .map(|(&time, &factor)| (time, if time == 0.0 { factor } else { 0.95 * factor })),
        )
        .unwrap();
    assert!(
        bump_forward_curve_from_rate_calibration(&shifted, record, &context, &context, &bump)
            .is_err()
    );
    let actual =
        bump_forward_curve_from_rate_calibration(&source, record, &context, &context, &bump)
            .unwrap();
    let fitted = rebootstrap_forward_curve(
        quotes.iter().map(|quote| quote.bump_rate_bp(1.0)).collect(),
        &params,
        &context,
        &config,
    )
    .unwrap();
    assert_eq!(actual.knots(), fitted.knots());
    assert_eq!(actual.factors(), fitted.factors());
    assert_eq!(actual.rate_calibration(), fitted.rate_calibration());
    assert_eq!(actual.fx_policy(), source.fx_policy());
    let row = original_execution(record)
        .expect("executed quote")
        .bindings
        .iter()
        .find(|binding| binding.quote_id == "SOFR-6M")
        .expect("quoted 6M");
    let target = CalibrationQuoteCoordinate {
        quote_id: row.quote_id.clone(),
        pillar_date: row.pillar_date,
    };
    let single =
        bump_forward_rate_quote(&source, &context, &target, 1.0).expect("admitted 6M quote");
    let expected = rebootstrap_forward_curve(
        vec![quotes[0].clone(), quotes[1].bump_rate_bp(1.0)],
        &params,
        &context,
        &config,
    )
    .expect("independent 6M shock");
    assert_eq!(single.factors(), expected.factors());
    assert!(bump_forward_rate_quote(&shifted, &context, &target, 1.0).is_err());
}

#[test]
fn replay_checks_source_unit_cashflow_before_publishing_target() {
    let base = time::macros::date!(2025 - 01 - 02);
    let maturity = time::macros::date!(2026 - 01 - 02);
    let params = DiscountCurveParams {
        curve_id: CurveId::new("GBP-SONIA"),
        currency: Currency::GBP,
        base_date: base,
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::LogLinear,
        extrapolation: ExtrapolationPolicy::Extend,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            curve_day_count: Some(DayCount::Act365F),
            ois_compounding: None,
        },
    };
    // SONIA deposits start on trade date. The one-year unit payment has
    // discount factor 1/(1+r) since its ACT/365F accrual is exactly one.
    let rate = 1.0 / 0.95 - 1.0;
    let quotes = [time::macros::date!(2025 - 07 - 02), maturity].map(|end| RateQuote::Deposit {
        id: QuoteId::new(format!("SONIA-{end}")),
        index: IndexId::new("GBP-SONIA-OIS"),
        pillar: Pillar::Date(end),
        rate,
    });
    let context = MarketContext::new();
    let source =
        solve_discount_curve(&quotes, &params, &context, &CalibrationConfig::default()).unwrap();
    let recipe = required_discount_rate_calibration(&source).unwrap();
    let source_factor = source.df_on_date(maturity).unwrap();
    assert!((source_factor - 0.95).abs() < 1e-10);
    let bump = QuoteBump::ParallelBp((1.0 / 0.9499 - 1.0 / 0.95) * 10_000.0);
    let altered = source
        .rebuild_with_knots(
            source
                .knots()
                .iter()
                .zip(source.dfs())
                .map(|(&time, &factor)| {
                    (
                        time,
                        if (time - 1.0).abs() < 1e-12 {
                            0.90
                        } else {
                            factor
                        },
                    )
                }),
        )
        .unwrap();
    let err =
        bump_discount_curve_from_rate_calibration(&altered, recipe, &context, &bump).unwrap_err();
    assert!(err.to_string().contains("source factor"));
    let replayed =
        bump_discount_curve_from_rate_calibration(&source, recipe, &context, &bump).unwrap();
    let target = replayed.df_on_date(maturity).unwrap();
    assert!((target - 0.9499).abs() < 1e-10);
    assert!((target - source_factor + 0.0001).abs() < 1e-10);
    assert!((target * (1.0 + rate + (1.0 / 0.9499 - 1.0 / 0.95)) - 1.0).abs() < 1e-10);
}

#[test]
fn sofr_cutoff_recipe_replays_zero_and_symmetric_quote_shocks() {
    let base_date = Date::from_calendar_date(2025, time::Month::January, 2).expect("valid date");
    let index = IndexId::new("USD-SOFR-OIS");
    let quotes = vec![
        RateQuote::Deposit {
            id: QuoteId::new("SOFR-DEP-1M"),
            index: index.clone(),
            pillar: Pillar::Tenor("1M".parse().expect("valid tenor")),
            rate: 0.0430,
        },
        RateQuote::Swap {
            id: QuoteId::new("SOFR-OIS-1Y"),
            index: index.clone(),
            pillar: Pillar::Tenor("1Y".parse().expect("valid tenor")),
            rate: 0.0410,
            spread_decimal: None,
        },
        RateQuote::Swap {
            id: QuoteId::new("SOFR-OIS-2Y"),
            index: index.clone(),
            pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
            rate: 0.0390,
            spread_decimal: None,
        },
    ];
    let params = DiscountCurveParams {
        curve_id: CurveId::new("USD-OIS"),
        currency: Currency::USD,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::Linear,
        extrapolation: ExtrapolationPolicy::Extend,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            curve_day_count: Some(DayCount::Act365F),
            ois_compounding: Some(
                finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedWithRateCutoff {
                    cutoff_days: 1,
                },
            ),
        },
    };
    let context = MarketContext::new()
        .insert_series(fixing_seed(index.as_str(), base_date, 0.0430).expect("SOFR fixing seed"));
    let cfg = CalibrationConfig {
        ..CalibrationConfig::default()
    };
    let source =
        solve_discount_curve(&quotes, &params, &context, &cfg).expect("source SOFR calibration");
    let calibration = source
        .rate_calibration()
        .cloned()
        .expect("calibrated curve recipe metadata");
    let altered = source
        .rebuild_with_knots(
            source
                .knots()
                .iter()
                .zip(source.dfs())
                .map(|(&time, &factor)| {
                    (
                        time,
                        if time > 0.0 {
                            factor * 0.90 / 0.95
                        } else {
                            factor
                        },
                    )
                }),
        )
        .expect("valid curve with mismatched prices");
    let error = bump_discount_curve_from_rate_calibration(
        &altered,
        &calibration,
        &context,
        &QuoteBump::ParallelBp(1.0),
    )
    .expect_err("reject source level before publishing bumped quotes");
    assert!(error.to_string().contains("source factor"));
    let recipe = source
        .rate_calibration()
        .expect("calibration target must stamp replay recipe");
    assert!(matches!(
        recipe.ois_compounding,
        Some(
            finstack_quant_core::market_data::term_structures::RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                cutoff_days: 1
            }
        )
    ));

    let zero = bump_discount_curve_from_rate_calibration(
        &source,
        &calibration,
        &context,
        &QuoteBump::ParallelBp(0.0),
    )
    .expect("zero quote shock");
    for (&time, &source_df) in source.knots().iter().zip(source.dfs()) {
        assert!(
            (zero.df(time).expect("valid") - source_df).abs() < 1e-12,
            "zero shock changed DF at {time}"
        );
    }

    for bump_bp in [-1.0, 1.0] {
        let replayed = bump_discount_curve_from_rate_calibration(
            &source,
            &calibration,
            &context,
            &QuoteBump::ParallelBp(bump_bp),
        )
        .expect("stored-recipe quote shock");
        let direct = solve_discount_curve(
            &quotes
                .iter()
                .map(|quote| quote.bump_rate_bp(bump_bp))
                .collect::<Vec<_>>(),
            &params,
            &context,
            &cfg,
        )
        .expect("explicit-recipe quote shock");
        for &time in source.knots() {
            assert!(
                (replayed.df(time).expect("valid") - direct.df(time).expect("valid")).abs() < 1e-12,
                "{bump_bp:+}bp replay mismatch at {time}: replayed={}, direct={}",
                replayed.df(time).expect("valid"),
                direct.df(time).expect("valid")
            );
        }
    }
}

#[test]
fn discount_replay_rejects_a_failed_fit_before_returning_curve() {
    let params: DiscountCurveParams = serde_json::from_value(serde_json::json!({
        "curve_id": "USD-DISC", "currency": "USD", "base_date": "2025-01-02",
        "method": {"global_solve": {"use_analytical_jacobian": false}},
        "interpolation": "log_linear"
    }))
    .expect("params");
    let quotes = vec![RateQuote::Deposit {
        id: QuoteId::new("DEP"),
        index: IndexId::new("USD-Deposit"),
        pillar: Pillar::Tenor("5Y".parse().expect("tenor")),
        rate: 0.20,
    }];
    let mut config = CalibrationConfig::default();
    config.solver = config.solver.with_max_iterations(1);
    config.discount_curve.bootstrap_seed_global_solve = false;
    let market_quotes = quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect::<Vec<_>>();
    let (_, report) = step_runtime::execute_params_and_apply(
        &StepParams::Discount(params.clone()),
        &market_quotes,
        &MarketContext::new(),
        &config,
    )
    .expect("optimizer returns diagnostics");
    assert!(
        !report.success,
        "fixture must fail the fit gate: {report:?}"
    );
    let error = solve_discount_curve(&quotes, &params, &MarketContext::new(), &config)
        .expect_err("strict replay");
    assert!(error.to_string().contains("failed fit acceptance"));
    config.fail_on_bad_fit = false;
    solve_discount_curve(&quotes, &params, &MarketContext::new(), &config)
        .expect("explicit diagnostic policy");
}

#[test]
fn typed_recipe_replay_restores_mixed_quote_fields() {
    let date = Date::from_calendar_date(2025, time::Month::September, 17).expect("valid date");
    let recipe = RateCalibrationRecipe {
        execution: Some(finstack_quant_core::market_data::term_structures::RateCalibrationExecution {
            bindings: ["deposit", "fra", "futures", "swap"].map(|id| finstack_quant_core::market_data::term_structures::RateQuoteBinding {
                quote_id: id.to_owned(), pillar_date: date, pillar_time: 1.0,
            }).to_vec(), accepted: true, step_params: serde_json::json!({}), calibration_config: serde_json::json!({}),
        }),
        currency: Currency::USD,
        method: RateCalibrationMethod::Bootstrap,
        curve_day_count: DayCount::Act365F,
        ois_compounding: None,
        role: RateCalibrationCurveRole::Discount {
            projection_curve_id: CurveId::new("USD-OIS"),
        },
        quotes: vec![
            RateCalibrationQuote::Deposit {
                index_id: IndexId::new("USD-SOFR-OIS"),
                pillar: RateCalibrationPillar::Date(date),
                rate: 0.043,
            },
            RateCalibrationQuote::Fra {
                index_id: IndexId::new("USD-SOFR-3M"),
                start: RateCalibrationPillar::Tenor(
                    "3M".parse().expect("valid start tenor"),
                ),
                end: RateCalibrationPillar::Date(date),
                rate: 0.041,
            },
            RateCalibrationQuote::Futures {
                contract: finstack_quant_core::market_data::term_structures::RateCalibrationFutureContractId::new("CME:SR3"),
                expiry: date,
                price: 95.75,
                convexity_adjustment: Some(0.0001),
            },
            RateCalibrationQuote::Swap {
                index_id: IndexId::new("USD-SOFR-OIS"),
                pillar: RateCalibrationPillar::Tenor(
                    "5Y".parse().expect("valid swap tenor"),
                ),
                rate: 0.039,
                spread_decimal: Some(0.00025),
            },
        ],
    };

    let restored =
        rate_quotes_from_recipe(&recipe, &CurveId::new("USD-OIS")).expect("typed replay");

    assert!(matches!(
        &restored[0],
        RateQuote::Deposit {
            pillar: Pillar::Date(value),
            ..
        } if *value == date
    ));
    assert!(matches!(
        &restored[1],
        RateQuote::Fra {
            start: Pillar::Tenor(_),
            end: Pillar::Date(value),
            ..
        } if *value == date
    ));
    assert!(matches!(
        &restored[2],
        RateQuote::Futures {
            contract,
            convexity_adjustment: value,
            ..
        } if contract.as_str() == "CME:SR3" && (*value - 0.0001).abs() < f64::EPSILON
    ));
    assert!(matches!(
        &restored[3],
        RateQuote::Swap {
            spread_decimal: Some(value),
            ..
        } if (*value - 0.00025).abs() < f64::EPSILON
    ));
}
