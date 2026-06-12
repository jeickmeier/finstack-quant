//! Integration tests for implied volatility solver.

use super::common::*;
use finstack_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_implied_vol_round_trip() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for vol in [0.15, 0.25, 0.35, 0.45] {
        let option = CDSOptionBuilder::new().implied_vol(vol).build(as_of);

        // Price with known vol
        let pv = option.value(&market, as_of).unwrap().amount();

        // Solve for implied vol
        let mut option_for_solve = option.clone();
        option_for_solve
            .pricing_overrides
            .market_quotes
            .implied_volatility = None;

        let solved_vol = option_for_solve
            .implied_vol(&market, as_of, pv, None)
            .unwrap();

        assert_approx_eq(
            solved_vol,
            vol,
            1e-6,
            &format!("IV round-trip for vol={}", vol),
        );
    }
}

#[test]
fn test_implied_vol_metric() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let target_vol = 0.28;

    let option = CDSOptionBuilder::new().implied_vol(target_vol).build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ImpliedVol],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let iv_metric = *result.measures.get("implied_vol").unwrap();

    assert_approx_eq(iv_metric, target_vol, 1e-6, "IV from metric");
}

#[test]
fn test_implied_vol_call_vs_put() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let vol = 0.30;

    let call = CDSOptionBuilder::new().call().implied_vol(vol).build(as_of);
    let put = CDSOptionBuilder::new().put().implied_vol(vol).build(as_of);

    let call_pv = call.value(&market, as_of).unwrap().amount();
    let put_pv = put.value(&market, as_of).unwrap().amount();

    // Solve for IV
    let mut call_solve = call;
    call_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;
    let call_iv = call_solve
        .implied_vol(&market, as_of, call_pv, None)
        .unwrap();

    let mut put_solve = put;
    put_solve.pricing_overrides.market_quotes.implied_volatility = None;
    let put_iv = put_solve.implied_vol(&market, as_of, put_pv, None).unwrap();

    assert_approx_eq(call_iv, vol, 1e-6, "Call IV");
    assert_approx_eq(put_iv, vol, 1e-6, "Put IV");
}

#[test]
fn test_implied_vol_moneyness_independence() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let vol = 0.25;

    for strike in [100.0, 150.0, 200.0, 220.0, 250.0] {
        let option = CDSOptionBuilder::new()
            .strike(strike)
            .implied_vol(vol)
            .build(as_of);

        let pv = option.value(&market, as_of).unwrap().amount();

        let mut option_solve = option.clone();
        option_solve
            .pricing_overrides
            .market_quotes
            .implied_volatility = None;
        let solved_iv = option_solve.implied_vol(&market, as_of, pv, None).unwrap();

        // For far OTM/ITM strikes, tolerance is slightly relaxed
        // Note: statrs provides more accurate implementations than custom approximations,
        // causing slight differences in numerical precision for edge cases
        // Updated tolerance to account for small differences introduced by ISDA Standard Model
        let tolerance = if !(150.0..=220.0).contains(&strike) {
            1.5e-5
        } else {
            3e-6
        };
        assert_approx_eq(
            solved_iv,
            vol,
            tolerance,
            &format!("IV for strike {}", strike),
        );
    }
}

#[test]
fn test_implied_vol_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().implied_vol(0.35).build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;
    let iv = option_solve.implied_vol(&market, as_of, pv, None).unwrap();

    assert_positive(iv, "Implied volatility");
    assert_in_range(iv, 0.01, 2.0, "Implied volatility reasonableness");
}

#[test]
fn test_implied_vol_with_initial_guess() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let true_vol = 0.40;

    let option = CDSOptionBuilder::new().implied_vol(true_vol).build(as_of);
    let pv = option.value(&market, as_of).unwrap().amount();

    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;

    // Try with different initial guesses
    for guess in [0.02, 0.10, 0.25, 0.50, 0.75, 4.0] {
        let iv = option_solve
            .implied_vol(&market, as_of, pv, Some(guess))
            .unwrap();

        assert_approx_eq(iv, true_vol, 1e-6, &format!("IV with guess {}", guess));
    }
}

#[test]
fn test_implied_vol_distressed_credit() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let true_vol = 1.50;
    let option = CDSOptionBuilder::new().implied_vol(true_vol).build(as_of);
    let pv = option.value(&market, as_of).unwrap().amount();
    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;

    let iv = option_solve
        .implied_vol(&market, as_of, pv, Some(0.30))
        .unwrap();

    assert_approx_eq(iv, true_vol, 1e-6, "distressed-credit IV");
}

#[test]
fn test_max_implied_vol_hard_error() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let err = CDSOption::example()
        .unwrap()
        .with_implied_vol(5.5)
        .unwrap_err();
    assert!(err.to_string().contains("exceeds maximum"));

    let invalid_option = CDSOptionBuilder::new().implied_vol(5.5).build(as_of);
    let pricing_err = invalid_option.value(&market, as_of).unwrap_err();
    assert!(pricing_err.to_string().contains("exceeds maximum"));
}

#[test]
fn test_implied_vol_convergence_atm() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let vol = 0.30;

    // ATM option (strike near forward ~200bp)
    let option = CDSOptionBuilder::new()
        .strike(200.0)
        .implied_vol(vol)
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;
    let iv = option_solve.implied_vol(&market, as_of, pv, None).unwrap();

    assert_approx_eq(iv, vol, 1e-6, "ATM IV convergence");
}

#[test]
fn test_implied_vol_convergence_itm() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let vol = 0.30;

    // ITM call (low strike)
    let option = CDSOptionBuilder::new()
        .call()
        .strike(100.0)
        .implied_vol(vol)
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;
    let iv = option_solve.implied_vol(&market, as_of, pv, None).unwrap();

    assert_approx_eq(iv, vol, 1e-6, "ITM IV convergence");
}

#[test]
fn test_implied_vol_convergence_otm() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let vol = 0.30;

    // OTM call (high strike)
    let option = CDSOptionBuilder::new()
        .call()
        .strike(400.0)
        .implied_vol(vol)
        .build(as_of);

    let pv = option.value(&market, as_of).unwrap().amount();

    let mut option_solve = option;
    option_solve
        .pricing_overrides
        .market_quotes
        .implied_volatility = None;
    let iv = option_solve.implied_vol(&market, as_of, pv, None).unwrap();

    assert_approx_eq(
        iv,
        vol,
        2.5e-4,
        "OTM IV convergence (relaxed tolerance for deep OTM)",
    );
}
