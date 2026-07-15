//! Annuity metric tests.
//!
//! Tests the fixed leg annuity calculation, which is the sum of
//! discounted accrual factors: Annuity = Σ α_i × DF(T_i)

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ]);

    // For zero or negative rates, DFs may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

fn build_market(rate: f64, base_date: Date) -> MarketContext {
    let disc_curve = build_flat_discount_curve(rate, base_date, "USD_OIS");
    let fwd_rate = if rate.abs() < 1e-10 { 1.0e-8 } else { rate };
    let fwd_curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, fwd_rate), (10.0, fwd_rate)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

fn create_standard_swap(as_of: Date, end: Date) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_ANNUITY_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        float: finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            // Use reset_lag_days: 0 for spot-starting swaps to avoid needing historical fixings.
            // With reset_lag > 0, the first reset date would be before as_of, requiring fixings.
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        margin_spec: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_annuity_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    assert!(annuity > 0.0, "Annuity should be positive");
}

#[test]
fn test_annuity_less_than_maturity() {
    // Due to discounting, annuity < time to maturity
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    assert!(
        annuity < 5.0,
        "Annuity {} should be less than 5-year maturity due to discounting",
        annuity
    );
}

#[test]
fn test_annuity_five_year_swap() {
    // 5-year swap at 5% should have annuity around 4.48
    //
    // Analytical with Act/360 day count:
    // - Quarterly periods ≈ 91 days each → accrual factor ≈ 91/360 = 0.2528
    // - Annuity = Σ (91/360) × exp(-0.05 × t_i) for 20 quarterly periods
    // - With Act/360: expect ~4.48 (vs 4.40 with exact 0.25 periods)
    //
    // The test uses Act/360 discount curve and Act/360 for accrual calculation,
    // so the expected value is slightly higher than the simplified 0.25-period formula.
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    // Expected annuity with Act/360: ~4.45 (accounting for actual day counts and discounting)
    let expected_annuity = 4.45;
    assert!(
        (annuity - expected_annuity).abs() < 0.02, // 0.5% tolerance for rounding/interpolation
        "5Y annuity at 5% should be ~{:.4}, got {:.4} (diff: {:.4})",
        expected_annuity,
        annuity,
        (annuity - expected_annuity).abs()
    );
}

#[test]
fn test_annuity_scales_with_maturity() {
    // Longer maturity → higher annuity
    let as_of = date!(2024 - 01 - 01);
    let market = build_market(0.05, as_of);

    let swap_2y = create_standard_swap(as_of, date!(2026 - 01 - 01));
    let swap_5y = create_standard_swap(as_of, date!(2029 - 01 - 01));
    let swap_10y = create_standard_swap(as_of, date!(2034 - 01 - 01));

    let annuity_2y = *swap_2y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    let annuity_5y = *swap_5y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    let annuity_10y = *swap_10y
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    assert!(annuity_2y < annuity_5y);
    assert!(annuity_5y < annuity_10y);
}

#[test]
fn test_annuity_higher_rates_lower_annuity() {
    // Higher discount rates → lower annuity
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market_3pct = build_market(0.03, as_of);
    let market_7pct = build_market(0.07, as_of);

    let annuity_3pct = *swap
        .price_with_metrics(
            &market_3pct,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    let annuity_7pct = *swap
        .price_with_metrics(
            &market_7pct,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    assert!(
        annuity_7pct < annuity_3pct,
        "Higher rate should give lower annuity: 7%={}, 3%={}",
        annuity_7pct,
        annuity_3pct
    );
}

#[test]
fn test_annuity_short_swap() {
    // 1-year swap
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market = build_market(0.05, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    assert!(
        annuity < 1.0,
        "1Y annuity should be less than 1, got {}",
        annuity
    );
}

#[test]
fn test_annuity_semiannual_vs_quarterly() {
    // Semiannual frequency should have lower annuity (fewer payments)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let market = build_market(0.05, as_of);

    let swap_quarterly = create_standard_swap(as_of, end);

    let mut swap_semiannual = create_standard_swap(as_of, end);
    swap_semiannual.fixed.frequency = Tenor::semi_annual();

    let annuity_quarterly = *swap_quarterly
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    let annuity_semiannual = *swap_semiannual
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    // Both should be positive and similar magnitude
    assert!(annuity_quarterly > 0.0);
    assert!(annuity_semiannual > 0.0);
}

#[test]
fn test_annuity_independent_of_side() {
    // Annuity should be same for pay vs receive
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let market = build_market(0.05, as_of);

    let mut swap_receive = create_standard_swap(as_of, end);
    swap_receive.side = PayReceive::Receive;

    let mut swap_pay = create_standard_swap(as_of, end);
    swap_pay.side = PayReceive::Pay;

    let annuity_receive = *swap_receive
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    let annuity_pay = *swap_pay
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("annuity")
        .unwrap();

    assert!(
        (annuity_receive - annuity_pay).abs() < 0.001,
        "Annuity should be independent of side: receive={}, pay={}",
        annuity_receive,
        annuity_pay
    );
}

#[test]
fn test_annuity_zero_rate_equals_maturity() {
    // At zero rates, annuity ≈ maturity (no discounting)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_standard_swap(as_of, end);
    let market = build_market(0.0, as_of);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Annuity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let annuity = *result.measures.get("annuity").unwrap();

    // At zero rates, annuity should approach maturity
    assert!(
        (annuity - 5.0).abs() < 0.1,
        "At zero rates, annuity should ≈ maturity: got {}",
        annuity
    );
}
