//! Integration tests for inflation calibration conventions (canonical).

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, InflationCurveParams, StepParams,
};
use finstack_quant_calibration::quotes::InflationQuote;
use finstack_quant_calibration::quotes::MarketQuote;
use finstack_quant_calibration::quotes::QuoteId;
use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, InflationObservation, SeriesInterpolation,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::market::conventions::ids::InflationSwapConventionId;
use finstack_quant_valuations_rates::inflation_swap::InflationSwap;
use rust_decimal::Decimal;

use crate::calibration::calibration_support as cal_utils;
use time::Month;

use super::tolerances::F64_ABS_TOL_LOOSE;
use finstack_quant_test_utils::assert::approx_eq;

fn create_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.96),
            (3.0, 0.88),
            (5.0, 0.82),
            (10.0, 0.68),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve")
}

fn create_us_cpi_fixings() -> InflationIndex {
    let observations = vec![
        (
            Date::from_calendar_date(2024, Month::September, 1).expect("date"),
            300.0,
        ),
        (
            Date::from_calendar_date(2024, Month::October, 1).expect("date"),
            301.0,
        ),
        (
            Date::from_calendar_date(2024, Month::November, 1).expect("date"),
            302.0,
        ),
        (
            Date::from_calendar_date(2024, Month::December, 1).expect("date"),
            303.0,
        ),
    ];

    let cutoff = time::macros::date!(2025 - 01 - 15);
    InflationIndex::new(
        "USD-CPI",
        observations
            .into_iter()
            .map(|(reference_month, value)| InflationObservation {
                reference_month,
                value,
                available_on: cutoff,
            })
            .collect(),
        Currency::USD,
        cutoff,
    )
    .expect("index")
    .with_expected_releases(
        finstack_quant_valuations_test_support::inflation::simple_index(
            "USD-CPI",
            cutoff,
            100.0,
            Currency::USD,
        )
        .get_expected_releases()
        .to_vec(),
    )
    .expect("explicit releases")
}

fn calibrated_market(index: Option<InflationIndex>, base_cpi: f64) -> (MarketContext, Date, Date) {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("maturity");
    let quotes = vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300115"),
        maturity,
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD"),
    })];
    let mut source_market = MarketContext::new().insert(create_discount_curve(base_date));
    let index = index.unwrap_or_else(|| {
        finstack_quant_valuations_test_support::inflation::simple_index(
            "USD-CPI",
            base_date,
            base_cpi,
            Currency::USD,
        )
    });
    source_market = source_market.insert_inflation_index(index);
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "infl".to_string(),
            quote_set: "infl".to_string(),
            params: StepParams::Inflation(InflationCurveParams {
                curve_id: "USD-CPI".into(),
                currency: Currency::USD,
                base_date,
                discount_curve_id: "USD-OIS".into(),
                index: "USA-CPI-U".to_string(),
                observation_lag: "3M".to_string(),
                base_cpi,
                source_base_date: time::macros::date!(2024 - 01 - 01),
                source_base_cpi: 100.0,
                day_count: DayCount::Act365F,
                calendar_id: None,
                extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::Flat,
                notional: 1.0,
                method: Default::default(),
                interpolation: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("execute");
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    (ctx, base_date, maturity)
}

#[test]
fn inflation_quote_time_uses_lagged_fixing_date() {
    let (ctx, _, maturity) = calibrated_market(None, 100.0);
    let curve = ctx.get_inflation_curve("USD-CPI").expect("inflation curve");
    let fixing_date = maturity
        .replace_day(1)
        .expect("month start")
        .add_months(-2)
        .expect("valid date shift");
    let reference_date = time::macros::date!(2024 - 01 - 01);
    let expected_t = DayCount::Act365F
        .year_fraction(reference_date, fixing_date, DayCountContext::default())
        .expect("clock");
    assert_eq!(curve.knots().first().copied(), Some(0.0));
    assert_eq!(curve.knots().len(), 2);
    approx_eq(
        curve.knots()[1],
        expected_t,
        F64_ABS_TOL_LOOSE,
        "lagged pillar time",
    );
}

#[test]
fn inflation_curve_origin_is_independent_of_contract_lag() {
    let (ctx, _, maturity) = calibrated_market(None, 100.0);
    let curve = ctx.get_inflation_curve("USD-CPI").expect("inflation curve");
    let reference_date = time::macros::date!(2024 - 01 - 01);
    assert_eq!(curve.base_date(), reference_date);
    assert_eq!(
        curve.cpi_on_date(reference_date).expect("reference CPI"),
        100.0
    );
    let anchor0 = maturity
        .replace_day(1)
        .expect("month start")
        .add_months(-3)
        .expect("valid date shift");
    let cpi0 = curve.cpi_on_date(anchor0).expect("first CPI");
    let cpi1 = curve
        .cpi_on_date(anchor0.add_months(1).expect("valid date shift"))
        .expect("second CPI");
    let weight =
        f64::from(maturity.day() - 1) / f64::from(maturity.month().length(maturity.year()));
    let reference_end = cpi0 + weight * (cpi1 - cpi0);
    let expected = 100.0 * 1.02_f64.powi(5);
    assert!(
        (reference_end - expected).abs() < 1e-7,
        "{reference_end} vs {expected}"
    );
}

#[test]
fn inflation_calibration_preserves_explicit_contract_denominator() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("maturity");

    let quotes = vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300115"),
        maturity,
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD"),
    })];
    let source_market = MarketContext::new()
        .insert(create_discount_curve(base_date))
        .insert_inflation_index(create_us_cpi_fixings());
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "infl".to_string(),
            quote_set: "infl".to_string(),
            params: StepParams::Inflation(InflationCurveParams {
                curve_id: "USD-CPI".into(),
                currency: Currency::USD,
                base_date,
                discount_curve_id: "USD-OIS".into(),
                index: "USA-CPI-U".to_string(),
                observation_lag: "3M".to_string(),
                base_cpi: 100.0,
                source_base_date: time::macros::date!(2024 - 01 - 01),
                source_base_cpi: 105.0,
                day_count: DayCount::Act365F,
                calendar_id: None,
                extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::Flat,
                notional: 1.0,
                method: Default::default(),
                interpolation: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope)
        .expect("explicit denominator is not replaced by a source observation");
    let market = MarketContext::try_from(result.result.final_market).expect("market");
    assert_eq!(
        market.get_inflation_curve("USD-CPI").unwrap().base_cpi(),
        105.0
    );
}

#[test]
fn b17_inflation_calibration_matches_observed_and_explicit_base_cpi() {
    use time::macros::date;
    let index = InflationIndex::new(
        "USD-CPI",
        vec![
            InflationObservation {
                reference_month: date!(2024 - 10 - 01),
                value: 300.0,
                available_on: date!(2024 - 11 - 13),
            },
            InflationObservation {
                reference_month: date!(2024 - 11 - 01),
                value: 303.0,
                available_on: date!(2024 - 12 - 13),
            },
        ],
        Currency::USD,
        date!(2025 - 01 - 15),
    )
    .expect("monthly CPI")
    .with_expected_releases(
        finstack_quant_valuations_test_support::inflation::simple_index(
            "USD-CPI",
            date!(2025 - 01 - 15),
            100.0,
            Currency::USD,
        )
        .get_expected_releases()
        .to_vec(),
    )
    .unwrap();
    let reference_cpi = 300.0 + 3.0 * 14.0 / 31.0;
    let (explicit, _, _) = calibrated_market(None, reference_cpi);
    let (observed, _, _) = calibrated_market(Some(index), reference_cpi);
    let left = explicit
        .get_inflation_curve("USD-CPI")
        .expect("explicit curve");
    let right = observed
        .get_inflation_curve("USD-CPI")
        .expect("observed curve");
    assert_eq!(left.base_date(), date!(2024 - 01 - 01));
    assert_eq!(left.base_date(), right.base_date());
    assert_eq!(left.knots(), right.knots());
    for t in [0.0, 0.1, 1.0, 3.0, 5.0] {
        assert!(
            (left.cpi(t).unwrap() - right.cpi(t).unwrap()).abs() < 1e-9,
            "{t}"
        );
    }
}

fn lag_calibration_params(lag: &str, method: CalibrationMethod) -> InflationCurveParams {
    InflationCurveParams {
        curve_id: "UK-RPI".into(),
        currency: Currency::GBP,
        base_date: time::macros::date!(2025 - 01 - 15),
        discount_curve_id: "GBP-SONIA".into(),
        index: "UK-RPI".to_string(),
        observation_lag: lag.to_string(),
        base_cpi: 100.0,
        source_base_date: time::macros::date!(2024 - 01 - 01),
        source_base_cpi: 100.0,
        day_count: DayCount::Act365F,
        calendar_id: None,
        extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::Flat,
        notional: 1.0,
        method,
        interpolation: InterpStyle::Linear,
    }
}

fn lag_calibration_market() -> MarketContext {
    MarketContext::new()
        .insert(
            DiscountCurve::builder("GBP-SONIA")
                .base_date(time::macros::date!(2025 - 01 - 15))
                .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("GBP discount curve"),
        )
        .insert_inflation_index(
            finstack_quant_valuations_test_support::inflation::simple_index(
                "UK-RPI",
                time::macros::date!(2025 - 01 - 15),
                100.0,
                Currency::GBP,
            ),
        )
}

fn lag_calibration_quotes() -> Vec<MarketQuote> {
    vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("UK-RPI-ZCIS-5Y"),
        maturity: time::macros::date!(2030 - 01 - 15),
        rate: 0.02,
        index: "UK-RPI".to_string(),
        convention: InflationSwapConventionId::new("UK-RPI"),
    })]
}

#[test]
fn delivered_inflation_curve_reprices_with_explicit_contract_lag() {
    for method in [
        CalibrationMethod::Bootstrap,
        CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
    ] {
        for (lag, expected_months) in [("2M", 2), ("none", 0)] {
            let params = lag_calibration_params(lag, method.clone());
            let (market, report) = cal_utils::execute_step(
                &StepParams::Inflation(params.clone()),
                &lag_calibration_quotes(),
                &lag_calibration_market(),
                &CalibrationConfig::default(),
            )
            .expect("calibrate inflation curve");
            assert!(report.success, "{method:?}, {lag}: {report:?}");
            // Residuals carry the quote id under both methods, never the
            // hard-coded calibration instrument id.
            assert_eq!(
                report.residuals.keys().collect::<Vec<_>>(),
                vec!["step-0:UK-RPI-ZCIS-5Y:tolerance_ratio"],
                "{method:?}, {lag}"
            );

            let curve = market.get_inflation_curve("UK-RPI").expect("curve");
            assert_eq!(curve.base_date(), params.source_base_date);

            // No index history and no contractual lag override: pricing must
            // inherit the same lag that calibrated the delivered curve.
            let mut swap = InflationSwap::builder()
                .id("DOWNSTREAM-RPI".into())
                .lag(InflationLag::months(expected_months))
                .notional(Money::new(1_000_000.0, Currency::GBP).expect("notional"))
                .start_date(params.base_date)
                .maturity(time::macros::date!(2030 - 01 - 15))
                .fixed_rate(Decimal::new(2, 2))
                .inflation_index_id("UK-RPI".into())
                .discount_curve_id("GBP-SONIA".into())
                .day_count(DayCount::OneOne)
                .side(PayReceive::Pay)
                .base_cpi(100.0)
                .interpolation(SeriesInterpolation::Step)
                .calendar_id_opt(Some("gblo".into()))
                .build()
                .expect("downstream swap");
            assert_eq!(swap.lag, Some(InflationLag::months(expected_months)));
            let inherited_pv = swap
                .npv_raw(&market, params.base_date)
                .expect("inherited PV");
            swap.lag = Some(lag.parse().expect("explicit lag"));
            let explicit_pv = swap
                .npv_raw(&market, params.base_date)
                .expect("explicit PV");
            assert!(
                (inherited_pv - explicit_pv).abs() < 1e-9,
                "{method:?}, {lag}: inherited={inherited_pv}, explicit={explicit_pv}"
            );
            assert!(
                inherited_pv.abs() < 0.02,
                "{method:?}, {lag}: calibrated swap PV={inherited_pv}"
            );
        }
    }
}

#[test]
fn inflation_calibration_rejects_unrepresentable_day_lags() {
    let error = cal_utils::execute_step(
        &StepParams::Inflation(lag_calibration_params("60D", CalibrationMethod::Bootstrap)),
        &lag_calibration_quotes(),
        &lag_calibration_market(),
        &CalibrationConfig::default(),
    )
    .expect_err("day lag cannot survive in the delivered curve metadata");
    assert!(
        error.to_string().contains("day-based observation lags"),
        "{error}"
    );
}

#[test]
fn inflation_month_lags_preserve_leap_day_and_monthly_anchor_contracts() {
    let base = time::macros::date!(2028 - 02 - 29);
    let maturity = time::macros::date!(2032 - 02 - 29);
    let market = MarketContext::new().insert(
        DiscountCurve::builder("GBP-SONIA")
            .base_date(base)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve"),
    );
    let quotes = [MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("LEAP-DAY"),
        maturity,
        rate: 0.02,
        index: "UK-RPI".to_string(),
        convention: InflationSwapConventionId::new("UK-RPI"),
    })];
    let market = market.insert_inflation_index(
        finstack_quant_valuations_test_support::inflation::simple_index(
            "UK-RPI",
            base,
            100.0,
            Currency::GBP,
        ),
    );
    let source_origin = time::macros::date!(2024 - 01 - 01);
    for (lag, origin, anchor) in [
        ("1M", source_origin, time::macros::date!(2032 - 01 - 01)),
        ("00M", source_origin, time::macros::date!(2032 - 02 - 01)),
        ("0M", source_origin, time::macros::date!(2032 - 02 - 01)),
        ("none", source_origin, time::macros::date!(2032 - 02 - 01)),
    ] {
        let mut params = lag_calibration_params(lag, CalibrationMethod::Bootstrap);
        params.base_date = base;
        let (calibrated, report) = cal_utils::execute_step(
            &StepParams::Inflation(params),
            &quotes,
            &market,
            &CalibrationConfig::default(),
        )
        .expect("calibrate leap-day quote");
        assert!(report.success, "{lag}: {report:?}");
        let curve = calibrated.get_inflation_curve("UK-RPI").expect("curve");
        assert_eq!(curve.base_date(), origin, "{lag}");
        let expected = DayCount::Act365F
            .year_fraction(origin, anchor, DayCountContext::default())
            .expect("pillar clock");
        assert_eq!(curve.knots().last().copied(), Some(expected), "{lag}");
    }
}

#[test]
fn inflation_params_reject_unsupported_seasonal_factors() {
    let params = lag_calibration_params("2M", CalibrationMethod::Bootstrap);
    for unsupported in [
        serde_json::Value::Null,
        serde_json::json!({
            "monthly_adjustments": [0.01, -0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        }),
    ] {
        let mut payload = serde_json::to_value(&params).expect("serialize params");
        payload["seasonal_factors"] = unsupported;
        // schema-rejection-test
        let error = serde_json::from_value::<InflationCurveParams>(payload)
            .expect_err("unsupported seasonal adjustment must not be silently ignored");
        assert!(
            error
                .to_string()
                .contains("unknown field `seasonal_factors`")
        );
    }
}
