//! Payer vs Receiver swaption tests

use crate::swaption::common::*;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolSurface, VolSurfaceAxis,
};
use finstack_core::money::Money;
use finstack_valuations::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use finstack_valuations::instruments::rates::swaption::VolatilityModel;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

#[test]
fn test_payer_receiver_symmetry() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let market = create_flat_market(as_of, 0.05, 0.30);
    let forward = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05)
        .forward_swap_rate(&market, as_of)
        .unwrap();

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, forward);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, forward);

    let pv_payer = payer.value(&market, as_of).unwrap().amount();
    let pv_receiver = receiver.value(&market, as_of).unwrap().amount();

    // At ATM (strike = forward), payer and receiver should have similar values
    assert_approx_eq(pv_payer, pv_receiver, 0.05, "ATM payer-receiver symmetry");
}

#[test]
fn test_forward_swap_rate_includes_first_multicurve_float_period() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 12 - 31);
    let swap_start = date!(2025 - 01 - 02);
    let swap_end = date!(2027 - 01 - 02);
    let market = create_flat_market(as_of, 0.02, 0.30).insert(build_flat_forward_curve(
        0.06,
        as_of,
        "USD_LIBOR_3M",
    ));

    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.04);
    let actual = swaption.forward_swap_rate(&market, as_of).unwrap();

    let fixed = |rate| FixedLegSpec {
        discount_curve_id: "USD_OIS".into(),
        rate: Decimal::try_from(rate).unwrap(),
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        start: swap_start,
        end: swap_end,
        par_method: None,
        compounding_simple: true,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let float = FloatLegSpec {
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start: swap_start,
        end: swap_end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let value_at_fixed_rate = |rate| {
        InterestRateSwap::builder()
            .id(format!("IRS-{rate:.0}").into())
            .notional(Money::new(
                1_000_000.0,
                finstack_core::currency::Currency::USD,
            ))
            .side(PayReceive::Receive)
            .fixed(fixed(rate))
            .float(float.clone())
            .build()
            .unwrap()
            .value(&market, as_of)
            .unwrap()
            .amount()
    };

    let value_zero = value_at_fixed_rate(0.0);
    let value_unit = value_at_fixed_rate(1.0);
    let expected = -value_zero / (value_unit - value_zero);

    assert_approx_eq(
        actual,
        expected,
        1e-10,
        "swaption forward should match equivalent IRS par rate",
    );
}

#[test]
fn test_normal_vol_surface_uses_underlying_tenor_axis() {
    let (as_of, expiry, swap_start, swap_end_5y) = standard_dates();
    let tenor_surface = VolSurface::from_grid_opts(
        "USD_SWAPTION_VOL",
        &[1.0],
        &[5.0, 10.0],
        &[0.01, 0.02],
        VolGridOpts::new(VolSurfaceAxis::Tenor, VolInterpolationMode::Vol),
    )
    .unwrap();
    let market = create_flat_market(as_of, 0.05, 0.30).insert_surface(tenor_surface);

    let mut five_year = create_standard_payer_swaption(expiry, swap_start, swap_end_5y, 0.05);
    five_year.vol_model = VolatilityModel::Normal;
    let five_year_sigma = five_year.resolve_volatility(&market, 0.05, 1.0).unwrap();

    let mut ten_year =
        create_standard_payer_swaption(expiry, swap_start, date!(2035 - 01 - 01), 0.05);
    ten_year.vol_model = VolatilityModel::Normal;
    let ten_year_sigma = ten_year.resolve_volatility(&market, 0.05, 1.0).unwrap();

    assert_approx_eq(five_year_sigma, 0.01, 1e-12, "5Y tenor normal vol");
    assert_approx_eq(ten_year_sigma, 0.02, 1e-12, "10Y tenor normal vol");
}

#[test]
fn test_payer_benefits_from_rate_increase() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;

    // Low rate environment
    let market_low = create_flat_market(as_of, 0.03, 0.30);
    // High rate environment
    let market_high = create_flat_market(as_of, 0.07, 0.30);

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);

    let pv_low = payer.value(&market_low, as_of).unwrap().amount();
    let pv_high = payer.value(&market_high, as_of).unwrap().amount();

    // Payer swaption is more valuable when rates are higher
    assert!(pv_high > pv_low, "Payer should benefit from rate increase");
}

#[test]
fn test_receiver_benefits_from_rate_decrease() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;

    // Low rate environment
    let market_low = create_flat_market(as_of, 0.03, 0.30);
    // High rate environment
    let market_high = create_flat_market(as_of, 0.07, 0.30);

    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

    let pv_low = receiver.value(&market_low, as_of).unwrap().amount();
    let pv_high = receiver.value(&market_high, as_of).unwrap().amount();

    // Receiver swaption is more valuable when rates are lower
    assert!(
        pv_low > pv_high,
        "Receiver should benefit from rate decrease"
    );
}

#[test]
fn test_delta_signs_opposite() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;
    let market = create_flat_market(as_of, 0.05, 0.30);

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

    let delta_payer = payer
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    let delta_receiver = receiver
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .copied()
        .unwrap();

    // Deltas should have opposite signs
    assert!(delta_payer > 0.0, "Payer delta should be positive");
    assert!(delta_receiver < 0.0, "Receiver delta should be negative");
}

#[test]
fn test_vega_same_sign() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;
    let market = create_flat_market(as_of, 0.05, 0.30);

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

    let vega_payer = payer
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    let vega_receiver = receiver
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .copied()
        .unwrap();

    // Both long options → both have positive vega
    assert!(vega_payer > 0.0, "Payer vega should be positive");
    assert!(vega_receiver > 0.0, "Receiver vega should be positive");

    // ATM vegas should be similar
    assert_approx_eq(
        vega_payer,
        vega_receiver,
        0.10,
        "ATM vegas should be similar",
    );
}

#[test]
fn test_gamma_same_sign() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let strike = 0.05;
    let market = create_flat_market(as_of, 0.05, 0.30);

    let payer = create_standard_payer_swaption(expiry, swap_start, swap_end, strike);
    let receiver = create_standard_receiver_swaption(expiry, swap_start, swap_end, strike);

    let gamma_payer = payer
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .copied()
        .unwrap();

    let gamma_receiver = receiver
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .copied()
        .unwrap();

    // Both long options → both have positive gamma
    assert!(gamma_payer >= 0.0, "Payer gamma should be non-negative");
    assert!(
        gamma_receiver >= 0.0,
        "Receiver gamma should be non-negative"
    );
}
