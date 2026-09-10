//! Tests for swaption pricers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_valuations::instruments::OptionType;

use crate::swaption::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_models::SabrParameters;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::rates::swaption::{BermudanSchedule, BermudanSwaption};
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSwaptionPricer, BermudanSwaptionPricerConfig, PreparedHullWhiteModel,
    SimpleSwaptionBlackPricer, SimpleSwaptionNormalPricer,
};
use finstack_quant_valuations::pricer::Pricer;
use time::macros::date;

#[test]
fn test_simple_swaption_normal_pricer_uses_bachelier_formula() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.vol_model =
        finstack_quant_valuations::instruments::rates::swaption::VolatilityModel::Normal;
    swaption.instrument_pricing_overrides = swaption
        .instrument_pricing_overrides
        .clone()
        .with_implied_vol(0.25);

    let market = create_flat_market(as_of, 0.03, 0.2);
    let expected_normal = swaption.price_normal(&market, 0.25, as_of).unwrap();
    let pricer = SimpleSwaptionNormalPricer;
    let result = pricer.price_dyn(&swaption, &market, as_of).unwrap().value;

    assert_approx_eq(
        result.amount(),
        expected_normal.amount(),
        1e-10,
        "pricer should honor instrument volatility model",
    );
}

#[test]
fn test_simple_swaption_pricers_reject_volatility_model_mismatch() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let mut swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05);
    swaption.vol_model =
        finstack_quant_valuations::instruments::rates::swaption::VolatilityModel::Normal;
    swaption.instrument_pricing_overrides = swaption
        .instrument_pricing_overrides
        .clone()
        .with_implied_vol(0.35);

    let market = create_flat_market(as_of, 0.03, 0.2);
    let black_error = SimpleSwaptionBlackPricer
        .price_dyn(&swaption, &market, as_of)
        .expect_err("Black-76 route must reject a normal-volatility instrument");
    assert!(black_error.to_string().contains("incompatible"));

    swaption.vol_model =
        finstack_quant_valuations::instruments::rates::swaption::VolatilityModel::Black;
    let normal_error = SimpleSwaptionNormalPricer
        .price_dyn(&swaption, &market, as_of)
        .expect_err("normal route must reject a Black-volatility instrument");
    assert!(normal_error.to_string().contains("incompatible"));
}

#[test]
fn test_simple_swaption_black_pricer_uses_sabr_dispatch_when_present() {
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let sabr_params = SabrParameters {
        alpha: 0.20,
        beta: 0.5,
        rho: -0.3,
        nu: 0.4,
        shift: None,
    };
    let swaption =
        create_standard_payer_swaption(expiry, swap_start, swap_end, 0.05).with_sabr(sabr_params);
    let market = create_flat_market(as_of, 0.05, 0.30);

    let expected = swaption.price_sabr(&market, as_of).unwrap();
    let pricer = SimpleSwaptionBlackPricer;
    let result = pricer.price_dyn(&swaption, &market, as_of).unwrap().value;

    assert_approx_eq(
        result.amount(),
        expected.amount(),
        1e-10,
        "sabr dispatch result",
    );
}

#[test]
fn test_simple_swaption_black_pricer_prices_out_of_grid_strike_via_vol_provider() {
    // The pricer uses VolSource::get_vol_clamped, which handles all strikes
    // (SABR cubes natively, surfaces via clamped extrapolation). The old
    // Error-extrapolation path is no longer relevant at the pricer level.
    let (as_of, expiry, swap_start, swap_end) = standard_dates();
    let swaption = create_standard_payer_swaption(expiry, swap_start, swap_end, 0.15);

    let market = create_flat_market(as_of, 0.03, 0.20);
    let pricer = SimpleSwaptionBlackPricer;
    let result = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect("vol_clamped should handle any strike");

    assert!(result.value.amount().is_finite());
}

#[test]
fn test_simple_swaption_black_pricer_type_mismatch() {
    let (as_of, _, _, _) = standard_dates();
    let market = create_flat_market(as_of, 0.03, 0.20);
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD_OIS",
    )
    .unwrap();

    let pricer = SimpleSwaptionBlackPricer;
    let err = pricer
        .price_dyn(&bond, &market, as_of)
        .expect_err("wrong instrument should fail");

    assert!(!err.to_string().is_empty());
}

#[test]
fn test_bermudan_pricer_cached_model_sets_measure() {
    let as_of = date!(2025 - 01 - 01);
    let swap_start = as_of;
    let swap_end = date!(2030 - 01 - 01);
    let first_exercise = date!(2026 - 01 - 01);
    let swaption = BermudanSwaption::new(
        "BERM-CACHED",
        OptionType::Call,
        Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        0.03,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            finstack_quant_core::dates::Tenor::semi_annual(),
        )
        .expect("valid Bermudan schedule"),
        "USD_OIS",
        "USD_OIS",
        "USD-SWPNVOL",
    )
    .expect("valid literal strike");

    let market = create_flat_market(as_of, 0.03, 0.2);
    let disc = market.get_discount("USD_OIS").unwrap();
    let ttm = swaption.time_to_maturity(as_of).unwrap();
    let model = PreparedHullWhiteModel::prepare(
        HullWhiteCalibrationParams::default(),
        50,
        disc.as_ref(),
        as_of,
        ttm,
        &swaption.exercise_times(as_of).expect("exercise times"),
    )
    .unwrap();

    let pricer = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
        prepared_model: Some(model),
        ..Default::default()
    });
    let result = pricer.price_dyn(&swaption, &market, as_of).unwrap();

    let used_cached = result.measures.get("used_cached_model").copied().unwrap();
    assert_eq!(used_cached, 1.0);
    assert!(result.value.amount() >= 0.0);
}

#[test]
fn test_bermudan_pricer_expired_returns_zero() {
    let as_of = date!(2035 - 01 - 01);
    let swap_start = date!(2025 - 01 - 01);
    let swap_end = date!(2030 - 01 - 01);
    let first_exercise = date!(2026 - 01 - 01);
    let swaption = BermudanSwaption::new(
        "BERM-EXPIRED",
        OptionType::Put,
        Money::new(2_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        0.04,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            finstack_quant_core::dates::Tenor::semi_annual(),
        )
        .expect("valid Bermudan schedule"),
        "USD_OIS",
        "USD_OIS",
        "USD-SWPNVOL",
    )
    .expect("valid literal strike");

    let market = create_flat_market(as_of, 0.03, 0.2);
    let pricer = BermudanSwaptionPricer::tree();
    let result = pricer.price_dyn(&swaption, &market, as_of).unwrap();

    assert_approx_eq(result.value.amount(), 0.0, 1e-12, "expired bermudan pv");
}

#[test]
fn test_bermudan_tree_pricer_rejects_mixed_curves() {
    let as_of = date!(2025 - 01 - 01);
    let swap_start = as_of;
    let swap_end = date!(2030 - 01 - 01);
    let first_exercise = date!(2026 - 01 - 01);
    let swaption = BermudanSwaption::new(
        "BERM-MIXED",
        OptionType::Call,
        Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        0.03,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            finstack_quant_core::dates::Tenor::semi_annual(),
        )
        .expect("valid Bermudan schedule"),
        "USD_OIS",
        "USD-SOFR-3M",
        "USD-SWPNVOL",
    )
    .expect("valid literal strike");

    let market = create_flat_market(as_of, 0.03, 0.2);
    let pricer = BermudanSwaptionPricer::tree();
    let err = pricer
        .price_dyn(&swaption, &market, as_of)
        .expect_err("mixed-curve Bermudan tree pricer should reject pricing");
    let msg = err.to_string();
    assert!(
        msg.contains("single-curve"),
        "expected single-curve rejection error, got: {msg}"
    );
}
