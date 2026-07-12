//! Bond pricing helper function tests
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    df_from_yield, fixed_leg_annuity, par_rate_and_annuity_from_discount, periods_per_year,
    price_from_ytm_compounded_params, YieldCompounding,
};

#[test]
fn test_periods_per_year_monthly() {
    // 1 month = 12 periods per year
    assert_eq!(periods_per_year(Tenor::monthly()).unwrap(), 12.0);

    // 3 months = 4 periods per year (quarterly)
    assert_eq!(periods_per_year(Tenor::quarterly()).unwrap(), 4.0);

    // 6 months = 2 periods per year (semi-annual)
    assert_eq!(periods_per_year(Tenor::semi_annual()).unwrap(), 2.0);

    // 12 months = 1 period per year (annual)
    assert_eq!(periods_per_year(Tenor::annual()).unwrap(), 1.0);
}

#[test]
fn test_periods_per_year_days() {
    // 30 days = 365/30 ≈ 12.17 periods per year
    let result = periods_per_year(Tenor::new(30, TenorUnit::Days)).unwrap();
    assert!((result - 12.166666).abs() < 0.01);

    // 90 days ≈ 4.06 periods per year
    let result = periods_per_year(Tenor::new(90, TenorUnit::Days)).unwrap();
    assert!((result - 4.0555).abs() < 0.01);

    // 365 days = 1 period per year
    assert_eq!(
        periods_per_year(Tenor::new(365, TenorUnit::Days)).unwrap(),
        1.0
    );
}

#[test]
fn test_periods_per_year_zero_months_error() {
    // 0 months should be an error
    assert!(Tenor::try_new(0, TenorUnit::Months).is_err());
}

#[test]
fn test_periods_per_year_zero_days_error() {
    // 0 days should be an error
    assert!(Tenor::try_new(0, TenorUnit::Days).is_err());
}

#[test]
fn test_periods_per_year_years_and_weeks() {
    assert_eq!(
        periods_per_year(Tenor::new(2, TenorUnit::Years)).unwrap(),
        0.5
    );
    assert_eq!(
        periods_per_year(Tenor::new(1, TenorUnit::Weeks)).unwrap(),
        52.0
    );
    assert_eq!(
        periods_per_year(Tenor::new(2, TenorUnit::Weeks)).unwrap(),
        26.0
    );
}

#[test]
fn test_periods_per_year_zero_years_and_weeks_error() {
    assert!(Tenor::try_new(0, TenorUnit::Years).is_err());
    assert!(Tenor::try_new(0, TenorUnit::Weeks).is_err());
}

#[test]
fn test_yield_compounding_variants() {
    // Test that all compounding variants are available
    let _simple = YieldCompounding::Simple;
    let _annual = YieldCompounding::Annual;
    let _periodic = YieldCompounding::Periodic(2);
    let _continuous = YieldCompounding::Continuous;
    let _street = YieldCompounding::Street;
}

#[test]
fn test_df_from_yield_simple() {
    // Simple compounding: DF = 1 / (1 + y*t)
    let ytm = 0.05; // 5%
    let t = 1.0; // 1 year
    let df = df_from_yield(ytm, t, YieldCompounding::Simple, Tenor::semi_annual()).unwrap();

    let expected = 1.0 / (1.0 + 0.05 * 1.0); // 1 / 1.05 = 0.9524
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_annual() {
    // Annual compounding: DF = (1 + y)^(-t)
    let ytm = 0.05;
    let t = 2.0;
    let df = df_from_yield(ytm, t, YieldCompounding::Annual, Tenor::annual()).unwrap();

    let expected = (1.05_f64).powf(-2.0); // 0.9070
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_periodic_semi_annual() {
    // Semi-annual compounding: DF = (1 + y/2)^(-2*t)
    let ytm = 0.06;
    let t = 1.0;
    let df = df_from_yield(ytm, t, YieldCompounding::Periodic(2), Tenor::semi_annual()).unwrap();

    let expected = (1.0_f64 + 0.06 / 2.0).powf(-2.0 * 1.0); // (1.03)^(-2) = 0.9426
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_periodic_quarterly() {
    // Quarterly compounding: DF = (1 + y/4)^(-4*t)
    let ytm = 0.08;
    let t = 1.0;
    let df = df_from_yield(ytm, t, YieldCompounding::Periodic(4), Tenor::quarterly()).unwrap();

    let expected = (1.0_f64 + 0.08 / 4.0).powf(-4.0 * 1.0); // (1.02)^(-4) = 0.9238
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_continuous() {
    // Continuous compounding: DF = exp(-y*t)
    let ytm = 0.05;
    let t = 1.0;
    let df = df_from_yield(ytm, t, YieldCompounding::Continuous, Tenor::annual()).unwrap();

    let expected = (-0.05_f64 * 1.0).exp(); // exp(-0.05) = 0.9512
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_street() {
    // Street convention uses bond's frequency
    let ytm = 0.06;
    let t = 1.0;
    let freq = Tenor::semi_annual(); // 2 periods per year
    let df = df_from_yield(ytm, t, YieldCompounding::Street, freq).unwrap();

    // Should be same as Periodic(2)
    let expected = (1.0_f64 + 0.06 / 2.0).powf(-2.0 * 1.0);
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_treasury_actual_short_and_long_paths() {
    let freq = Tenor::semi_annual();

    let short_df = df_from_yield(0.06, 0.25, YieldCompounding::TreasuryActual, freq).unwrap();
    let short_expected = 1.0 / (1.0 + 0.06 * 0.25);
    assert!((short_df - short_expected).abs() < 1e-12);

    let stub_df = df_from_yield(0.06, 1.3, YieldCompounding::TreasuryActual, freq).unwrap();
    let m = 2.0_f64;
    let n_full_periods = (1.3_f64 * m).floor();
    let stub_time = 1.3 - n_full_periods / m;
    let stub_expected = (1.0_f64 / (1.0_f64 + 0.06_f64 * stub_time))
        * (1.0_f64 + 0.06_f64 / m).powf(-n_full_periods);
    assert!((stub_df - stub_expected).abs() < 1e-12);

    let no_stub_df = df_from_yield(0.06, 1.5, YieldCompounding::TreasuryActual, freq).unwrap();
    let no_stub_expected = (1.0_f64 + 0.06_f64 / m).powf(-m * 1.5_f64);
    assert!((no_stub_df - no_stub_expected).abs() < 1e-12);
}

#[test]
fn test_df_from_yield_validation_errors_cover_multiple_compounding_modes() {
    assert!(df_from_yield(-2.0, 1.0, YieldCompounding::Simple, Tenor::annual()).is_err());
    assert!(df_from_yield(-1.5, 1.0, YieldCompounding::Annual, Tenor::annual()).is_err());
    assert!(df_from_yield(-5.0, 1.0, YieldCompounding::Street, Tenor::semi_annual()).is_err());
    assert!(df_from_yield(
        -5.0,
        0.75,
        YieldCompounding::TreasuryActual,
        Tenor::semi_annual()
    )
    .is_err());
}

#[test]
fn test_df_from_yield_zero_time() {
    // t = 0 should return DF = 1.0
    let ytm = 0.05;
    let df = df_from_yield(ytm, 0.0, YieldCompounding::Simple, Tenor::annual()).unwrap();
    assert_eq!(df, 1.0);
}

#[test]
fn test_df_from_yield_negative_time() {
    // Negative time should return DF = 1.0
    let ytm = 0.05;
    let df = df_from_yield(ytm, -1.0, YieldCompounding::Simple, Tenor::annual()).unwrap();
    assert_eq!(df, 1.0);
}

#[test]
fn test_df_from_yield_zero_ytm() {
    // Zero yield should give DF = 1.0 for all compounding
    let t = 1.0;

    let df_simple = df_from_yield(0.0, t, YieldCompounding::Simple, Tenor::annual()).unwrap();
    assert_eq!(df_simple, 1.0);

    let df_annual = df_from_yield(0.0, t, YieldCompounding::Annual, Tenor::annual()).unwrap();
    assert_eq!(df_annual, 1.0);

    let df_continuous =
        df_from_yield(0.0, t, YieldCompounding::Continuous, Tenor::annual()).unwrap();
    assert_eq!(df_continuous, 1.0);
}

#[test]
fn test_df_from_yield_high_ytm() {
    // High yield should give small DF
    let ytm = 0.20; // 20%
    let t = 5.0;
    let df = df_from_yield(ytm, t, YieldCompounding::Annual, Tenor::annual()).unwrap();

    let expected = (1.20_f64).powf(-5.0); // 0.4019
    assert!((df - expected).abs() < 0.0001);
    assert!(df < 0.5); // Should be significantly discounted
}

#[test]
fn test_price_from_ytm_compounded_params_single_flow() {
    // Single cashflow in 1 year. Use non-leap year dates so Act365F gives year_fraction = 1.0 exactly.
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let payment_date = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
    let flows = vec![(payment_date, Money::new(100.0, Currency::USD))];

    let ytm = 0.05;
    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    // PV = 100 / 1.05 = 95.238... (exact: year_fraction = 365/365 = 1.0)
    let expected = 100.0 / 1.05;
    assert!((price - expected).abs() < 1e-10);
}

#[test]
fn test_price_from_ytm_compounded_params_act_act_isma() {
    // ACT/ACT (ICMA) requires the coupon frequency in the day-count context.
    // Regression for the YTM path erroring with MissingFrequencyForActActIsma.
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let date1 = Date::from_calendar_date(2025, time::Month::July, 1).unwrap();
    let date2 = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();

    let flows = vec![
        (date1, Money::new(2.5, Currency::USD)),
        (date2, Money::new(102.5, Currency::USD)),
    ];

    let ytm = 0.05;
    let price = price_from_ytm_compounded_params(
        DayCount::ActActIsma,
        Tenor::semi_annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Street,
    )
    .expect("ACT/ACT (ICMA) pricing from yield must succeed");

    // Regular semi-annual periods => t = 0.5 and 1.0 exactly under ISMA.
    let expected = 2.5 / 1.025 + 102.5 / (1.025_f64 * 1.025);
    assert!(
        (price - expected).abs() < 1e-10,
        "price {price} vs expected {expected}"
    );
}

#[test]
fn test_price_from_ytm_compounded_params_multiple_flows() {
    // Multiple cashflows: $5 in 6 months, $105 in 1 year
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let date1 = Date::from_calendar_date(2024, time::Month::July, 1).unwrap();
    let date2 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    let flows = vec![
        (date1, Money::new(5.0, Currency::USD)),
        (date2, Money::new(105.0, Currency::USD)),
    ];

    let ytm = 0.06;
    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::semi_annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Periodic(2),
    )
    .unwrap();

    // PV ≈ 5/(1.03)^0.5 + 105/(1.03)^2 = 4.929 + 99.006 = 103.935
    assert!(price > 103.0 && price < 105.0);
}

#[test]
fn test_price_from_ytm_compounded_params_past_flows_ignored() {
    // Past flows should be ignored. Use non-leap year dates so Act365F gives year_fraction = 1.0 exactly.
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let past_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let future_date = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();

    let flows = vec![
        (past_date, Money::new(100.0, Currency::USD)), // Should be ignored
        (future_date, Money::new(100.0, Currency::USD)),
    ];

    let ytm = 0.05;
    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    // Should only value the future flow: PV = 100 / 1.05 = 95.238... (year_fraction = 365/365 = 1.0)
    let expected = 100.0 / 1.05;
    assert!((price - expected).abs() < 1e-10);
}

#[test]
fn test_price_from_ytm_compounded_params_zero_ytm() {
    // Zero yield means flows at par
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let date1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let date2 = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();

    let flows = vec![
        (date1, Money::new(50.0, Currency::USD)),
        (date2, Money::new(50.0, Currency::USD)),
    ];

    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        0.0,
        YieldCompounding::Annual,
    )
    .unwrap();

    // With zero yield, PV = sum of cashflows
    assert!((price - 100.0).abs() < 0.01);
}

#[test]
fn test_price_from_ytm_compounded_params_empty_flows() {
    // Empty flows should give PV = 0
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let flows: Vec<(Date, Money)> = vec![];

    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        0.05,
        YieldCompounding::Annual,
    )
    .unwrap();

    assert_eq!(price, 0.0);
}

#[test]
fn test_price_from_ytm_compounded_params_different_day_counts() {
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let payment_date = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let flows = vec![(payment_date, Money::new(100.0, Currency::USD))];
    let ytm = 0.05;

    // Test different day count conventions
    let price_365f = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    let price_360 = price_from_ytm_compounded_params(
        DayCount::Act360,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    // Prices should be slightly different due to day count
    assert!(price_365f > 94.0 && price_365f < 96.0);
    assert!(price_360 > 94.0 && price_360 < 96.0);
    // Act360 has fewer days in year, so year fraction is slightly higher, DF slightly lower
    assert!(price_360 < price_365f);
}

#[test]
fn test_price_from_ytm_compounded_params_long_maturity() {
    // Test long maturity bond (30 years)
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2054, time::Month::January, 1).unwrap();

    let flows = vec![(maturity, Money::new(100.0, Currency::USD))];

    let ytm = 0.04; // 4% yield
    let price = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    // Act365F from 2024-01-01 to 2054-01-01: 30 years with 8 leap years = 10958 days / 365 = 30.0219 yrs.
    // expected uses the actual year fraction, not 30.0 exactly.
    let year_fraction = 10958.0 / 365.0;
    let expected = 100.0 / (1.04_f64).powf(year_fraction);
    assert!((price - expected).abs() < 1e-10);
    assert!(price < 35.0); // Should be heavily discounted
}

#[test]
fn test_price_from_ytm_compounded_params_continuous_vs_annual() {
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let payment_date = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let flows = vec![(payment_date, Money::new(100.0, Currency::USD))];
    let ytm = 0.05;

    let price_annual = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Annual,
    )
    .unwrap();

    let price_continuous = price_from_ytm_compounded_params(
        DayCount::Act365F,
        Tenor::annual(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Continuous,
    )
    .unwrap();

    // Continuous compounding should give slightly lower price (higher discount).
    // For 5% yield over ~1 year, the difference is ~0.12 (100bp annualized effect).
    assert!(price_continuous < price_annual);
    assert!((price_annual - price_continuous).abs() < 0.2);
}

#[test]
fn test_yield_compounding_equality() {
    // Test PartialEq implementation
    assert_eq!(YieldCompounding::Simple, YieldCompounding::Simple);
    assert_eq!(YieldCompounding::Annual, YieldCompounding::Annual);
    assert_eq!(YieldCompounding::Periodic(2), YieldCompounding::Periodic(2));
    assert_eq!(YieldCompounding::Continuous, YieldCompounding::Continuous);
    assert_eq!(YieldCompounding::Street, YieldCompounding::Street);

    assert_ne!(YieldCompounding::Simple, YieldCompounding::Annual);
    assert_ne!(YieldCompounding::Periodic(2), YieldCompounding::Periodic(4));
}

#[test]
fn test_df_from_yield_street_with_monthly() {
    // Street convention with monthly frequency
    let ytm = 0.12; // 12% annual
    let t = 0.5; // 6 months
    let freq = Tenor::monthly(); // 12 periods per year
    let df = df_from_yield(ytm, t, YieldCompounding::Street, freq).unwrap();

    // Should use monthly compounding
    let expected = (1.0_f64 + 0.12 / 12.0).powf(-12.0 * 0.5);
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_df_from_yield_street_with_quarterly() {
    let ytm = 0.08;
    let t = 1.0;
    let freq = Tenor::quarterly();
    let df = df_from_yield(ytm, t, YieldCompounding::Street, freq).unwrap();

    // Should use quarterly compounding (4 periods per year)
    let expected = (1.0_f64 + 0.08 / 4.0).powf(-4.0 * 1.0);
    assert!((df - expected).abs() < 0.0001);
}

#[test]
fn test_fixed_leg_annuity_and_par_rate_discount_helpers_cover_edge_cases() {
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .build()
        .unwrap();

    let short_schedule = vec![as_of];
    assert_eq!(
        fixed_leg_annuity(&curve, DayCount::Act365F, None, &short_schedule).unwrap(),
        0.0
    );
    assert_eq!(
        par_rate_and_annuity_from_discount(&curve, DayCount::Act365F, None, &short_schedule)
            .unwrap(),
        (0.0, 0.0)
    );

    let end = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let schedule = vec![as_of, end];
    let annuity = fixed_leg_annuity(&curve, DayCount::Act365F, None, &schedule).unwrap();
    assert!((annuity - 1.002739726).abs() < 1e-9);

    let (par_rate, par_annuity) =
        par_rate_and_annuity_from_discount(&curve, DayCount::Act365F, None, &schedule).unwrap();
    assert_eq!(par_rate, 0.0);
    assert!((par_annuity - annuity).abs() < 1e-12);
}
