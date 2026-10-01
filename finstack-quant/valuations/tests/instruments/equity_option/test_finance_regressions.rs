//! Independent cashflow and deterministic-limit regressions.

use super::helpers::*;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::{
    EquityOption, ExerciseStyle, Instrument, PricingOptions,
};
use finstack_quant_valuations::pricer::{standard_pricer_registry, ModelKey};
use time::{macros::date, Duration};

#[test]
fn cash_dividend_theta_matches_smooth_calendar_repricing() {
    let as_of = date!(2025 - 01 - 06);
    let expiry = date!(2026 - 01 - 06);
    for rate in [-0.04, 0.0, 0.08] {
        for is_call in [true, false] {
            for dividends in [
                vec![],
                vec![(date!(2025 - 05 - 06), 15.0)],
                vec![(date!(2025 - 05 - 06), 15.0), (date!(2025 - 09 - 06), 20.0)],
            ] {
                let mut option = if is_call {
                    create_call(as_of, expiry, 75.0)
                } else {
                    create_put(as_of, expiry, 75.0)
                };
                option.discrete_dividends = dividends;
                let market = build_standard_market(as_of, 100.0, 0.25, rate, 0.0);
                let actual = option.theta(&market, as_of).unwrap();
                let dividend_pv: f64 = option
                    .discrete_dividends
                    .iter()
                    .map(|&(date, amount)| {
                        amount * (-rate * (date - as_of).whole_days() as f64 / 365.0).exp()
                    })
                    .sum();
                let base = finstack_quant_models::closed_form::bs_greeks(
                    100.0 - dividend_pv,
                    option.strike,
                    rate,
                    0.0,
                    0.25,
                    1.0,
                    option.option_type,
                    option.metric_pricing_overrides.theta_days_per_year(),
                )
                .unwrap();
                let chain_rule = (base.theta
                    - base.delta * rate * dividend_pv
                        / option.metric_pricing_overrides.theta_days_per_year())
                    * option.quantity;
                assert!((actual - chain_rule).abs() < 1e-9);
                // Keep spot, volatility, and the flat zero yield fixed while
                // shortening every dated cashflow's remaining time by one day.
                let pv_on = |day: Date| {
                    option
                        .value(&build_standard_market(day, 100.0, 0.25, rate, 0.0), day)
                        .unwrap()
                        .amount()
                };
                let expected = (pv_on(as_of + Duration::days(1))
                    - pv_on(as_of - Duration::days(1)))
                    * 0.5
                    * 365.0
                    / option.metric_pricing_overrides.theta_days_per_year();
                assert!((actual - expected).abs() < 2e-5,
                    "rate={rate}, call={is_call}, dividends={:?}: theta={actual}, repriced={expected}", option.discrete_dividends);
            }
        }
    }
}

#[test]
fn cash_dividend_theta_on_ex_date_uses_the_post_dividend_state() {
    let ex_date = date!(2025 - 05 - 06);
    let mut option = create_call(ex_date, date!(2026 - 01 - 06), 90.0);
    option.discrete_dividends = vec![(ex_date, 10.0)];
    let mut no_dividends = option.clone();
    no_dividends.discrete_dividends.clear();
    let market = build_standard_market(ex_date, 90.0, 0.25, 0.05, 0.0);
    // The ex-date cash payment and spot jump are already reflected in the
    // observed spot. Smooth forward theta must not count the cashflow again.
    assert_eq!(
        option.theta(&market, ex_date).unwrap(),
        no_dividends.theta(&market, ex_date).unwrap()
    );
    let tomorrow = ex_date + Duration::days(1);
    let next_pv = option
        .value(
            &build_standard_market(tomorrow, 90.0, 0.25, 0.05, 0.0),
            tomorrow,
        )
        .unwrap()
        .amount();
    let pv = option.value(&market, ex_date).unwrap().amount();
    let theta = option.theta(&market, ex_date).unwrap();
    assert!((next_pv - pv - theta).abs() < 0.005);
}

fn pde_price(option: &EquityOption, market: &MarketContext, as_of: Date) -> f64 {
    standard_pricer_registry()
        .price_with_metrics(
            option,
            ModelKey::PdeCrankNicolson1D,
            market,
            as_of,
            &[],
            PricingOptions::default(),
        )
        .unwrap()
        .value
        .amount()
}

#[test]
fn zero_volatility_pde_matches_discounted_payoffs_and_optimal_exercise() {
    let as_of = date!(2025 - 01 - 06);
    let expiry = as_of + Duration::days(365 * 5);
    let maturity = 5.0;
    for (rate, dividend_yield) in [
        (0.0, 0.0),
        (0.05, 0.0),
        (-0.04, 0.0),
        (0.1, 0.05),
        (-0.08, -0.03),
    ] {
        for strike in [60.0, 100.0, 120.0] {
            for is_call in [true, false] {
                let mut option = if is_call {
                    create_call(as_of, expiry, strike)
                } else {
                    create_put(as_of, expiry, strike)
                };
                let market = build_standard_market(as_of, 100.0, 0.0, rate, dividend_yield);
                let direction = if is_call { 1.0 } else { -1.0 };
                let exercise_value = |time: f64| {
                    (direction
                        * (100.0 * (-dividend_yield * time).exp() - strike * (-rate * time).exp()))
                    .max(0.0)
                        * option.quantity
                };
                let expected_european = exercise_value(maturity);
                // Independent dense optimal-stopping search, including interior
                // exercise times when carry makes both endpoints suboptimal.
                let expected_american = (0..=20_000)
                    .map(|i| exercise_value(maturity * i as f64 / 20_000.0))
                    .fold(0.0_f64, f64::max);
                let european = pde_price(&option, &market, as_of);
                assert!((european - expected_european).abs() < 1e-9);
                option.exercise_style = ExerciseStyle::American;
                let american = pde_price(&option, &market, as_of);
                assert!((american - expected_american).abs() < 1e-6,
                    "rate={rate}, yield={dividend_yield}, strike={strike}, call={is_call}: {american} vs {expected_american}");
                assert!(american + 1e-9 >= european);
            }
        }
    }
}

#[test]
fn zero_volatility_european_pde_preserves_cash_dividend_escrow() {
    let as_of = date!(2025 - 01 - 06);
    let expiry = as_of + Duration::days(365);
    let ex_date = as_of + Duration::days(182);
    let mut option = create_call(as_of, expiry, 80.0);
    option.discrete_dividends = vec![(ex_date, 10.0)];
    let market = build_standard_market(as_of, 100.0, 0.0, 0.05, 0.03);
    let curve = market.get_discount(DISC_ID).unwrap();
    let expected = (100.0
        - 10.0 * curve.df_between_dates(as_of, ex_date).unwrap()
        - 80.0 * curve.df_between_dates(as_of, expiry).unwrap())
    .max(0.0)
        * option.quantity;
    assert!((pde_price(&option, &market, as_of) - expected).abs() < 1e-9);
}

#[test]
fn positive_volatility_pde_approaches_the_deterministic_limit() {
    let as_of = date!(2025 - 01 - 06);
    let option = create_call(as_of, as_of + Duration::days(365), 100.0);
    let limit = (100.0 - 100.0 * (-0.05_f64).exp()) * option.quantity;
    let mut previous_error = f64::INFINITY;
    for vol in [0.04, 0.02, 0.01] {
        let market = build_standard_market(as_of, 100.0, vol, 0.05, 0.0);
        let error = (pde_price(&option, &market, as_of) - limit).abs();
        assert!(
            error < previous_error,
            "vol={vol}: error={error}, previous={previous_error}"
        );
        previous_error = error;
    }
    assert!(
        previous_error < 0.1,
        "limiting price error={previous_error}"
    );
}
