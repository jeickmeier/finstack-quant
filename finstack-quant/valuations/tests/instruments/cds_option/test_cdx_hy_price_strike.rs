//! CDX HY clean-price-strike CDS option tests.
//!
//! Covers the price-strike risk metrics (curve-reprice delta/gamma,
//! implied-vol round trips, vega) and end-to-end pricing behaviour that is
//! specific to the clean-price strike convention. The factor/loss payoff
//! algebra unit tests live next to the quadrature in
//! `bloomberg_quadrature.rs`.

#![allow(clippy::expect_used)]

use super::common::*;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

const VOL: f64 = 0.40;
const HY_COUPON_BP: f64 = 500.0;
const NEAR_ATM_PRICE_PCT: f64 = 105.0;
const STRIKE_FACTOR: f64 = 1.0;
const CURRENT_FACTOR: f64 = 0.99;
const REALIZED_LOSS: f64 = 0.004;

fn hy_option(option_type: OptionType, strike_price_pct: f64, as_of: Date) -> CDSOption {
    let mut builder = CDSOptionBuilder::new()
        .id("CDX-HY-PRICE-STRIKE")
        .clean_price_strike(strike_price_pct)
        .with_index(CURRENT_FACTOR)
        .strike_index_factor(STRIKE_FACTOR)
        .realized_index_loss(REALIZED_LOSS)
        .underlying_cds_coupon_bp(HY_COUPON_BP)
        .implied_vol(VOL);
    builder = match option_type {
        OptionType::Call => builder.call(),
        OptionType::Put => builder.put(),
    };
    builder.build(as_of)
}

fn price_delta(option: &CDSOption, market: &MarketContext, as_of: Date) -> f64 {
    option
        .price_with_metrics(
            market,
            as_of,
            &[MetricId::Delta],
            crate::instruments::test_support::credit::pricing_options(),
        )
        .expect("provider-backed price-strike delta")
        .measures[&MetricId::Delta]
}

#[test]
fn price_delta_signs_payer_positive_receiver_negative() {
    let as_of = date!(2025 - 01 - 01);
    let market = replayable_standard_market(as_of);

    let payer_delta = price_delta(
        &hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT, as_of),
        &market,
        as_of,
    );
    let receiver_delta = price_delta(
        &hy_option(OptionType::Put, NEAR_ATM_PRICE_PCT, as_of),
        &market,
        as_of,
    );

    assert_positive(payer_delta, "near-ATM payer price-strike delta");
    assert!(
        receiver_delta < 0.0,
        "near-ATM receiver price-strike delta must be negative, got {receiver_delta}"
    );
}

#[test]
fn price_delta_moves_monotonically_with_moneyness() {
    let as_of = date!(2025 - 01 - 01);
    let market = replayable_standard_market(as_of);

    // Payers gain value as the price strike rises: deep ITM above the
    // representative near-ATM strike, deep OTM below it. Receivers mirror.
    let payer_itm = price_delta(
        &hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT + 15.0, as_of),
        &market,
        as_of,
    );
    let payer_otm = price_delta(
        &hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT - 15.0, as_of),
        &market,
        as_of,
    );
    assert_positive(payer_itm, "ITM payer hedge ratio");
    assert_positive(payer_otm, "OTM payer hedge ratio");
    assert!(
        payer_itm > payer_otm,
        "payer hedge ratio must increase from OTM to ITM: OTM={payer_otm}, ITM={payer_itm}"
    );

    let receiver_itm = price_delta(
        &hy_option(OptionType::Put, NEAR_ATM_PRICE_PCT - 15.0, as_of),
        &market,
        as_of,
    );
    let receiver_otm = price_delta(
        &hy_option(OptionType::Put, NEAR_ATM_PRICE_PCT + 15.0, as_of),
        &market,
        as_of,
    );
    assert!(
        receiver_itm < 0.0,
        "ITM receiver hedge ratio must be negative, got {receiver_itm}"
    );
    assert!(
        receiver_otm <= 0.0,
        "OTM receiver hedge ratio must be non-positive, got {receiver_otm}"
    );
    assert!(
        receiver_itm < receiver_otm,
        "receiver hedge-ratio magnitude must increase from OTM to ITM: \
         OTM={receiver_otm}, ITM={receiver_itm}"
    );
}

#[test]
fn price_delta_requires_replay_recipe() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT, as_of);

    let error = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            crate::instruments::test_support::credit::pricing_options(),
        )
        .expect_err("standard price delta requires quote-space replay");
    assert!(error.to_string().contains("calibration recipe"));
}

#[test]
fn price_gamma_requires_replay_recipe() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for option_type in [OptionType::Call, OptionType::Put] {
        let option = hy_option(option_type, NEAR_ATM_PRICE_PCT, as_of);
        let error = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Gamma],
                crate::instruments::test_support::credit::pricing_options(),
            )
            .expect_err("standard price gamma requires quote-space replay");
        assert!(error.to_string().contains("calibration recipe"));
    }
}

#[test]
fn price_strike_implied_vol_round_trips() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for option_type in [OptionType::Call, OptionType::Put] {
        let option = hy_option(option_type, NEAR_ATM_PRICE_PCT, as_of);
        let target = option.value(&market, as_of).expect("npv").amount();
        assert_positive(target, "near-ATM option premium");
        let recovered = option
            .implied_vol(&market, as_of, target, None)
            .expect("implied vol");
        relative_eq(recovered, VOL, 1e-6, "implied-vol round trip");
    }
}

#[test]
fn price_strike_implied_vol_rejects_unattainable_targets() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT, as_of);

    // A premium approaching full notional is far above anything the model
    // can produce at the vol ceiling: must fail with a bracket error, not
    // return a plausible-looking volatility.
    let err = option
        .implied_vol(&market, as_of, 0.9 * option.notional.amount(), None)
        .expect_err("unattainable premium must fail");
    assert!(
        err.to_string().contains("outside model bounds"),
        "expected bracket error, got: {err}"
    );
}

#[test]
fn price_strike_vega_is_positive() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for option_type in [OptionType::Call, OptionType::Put] {
        let vega = hy_option(option_type, NEAR_ATM_PRICE_PCT, as_of)
            .vega(&market, as_of)
            .expect("price-strike vega");
        assert_positive(vega, "near-ATM vega");
    }
}

/// Expiry-boundary and missing-curve behaviour.
#[test]
fn expiry_boundary_and_missing_curves_fail_or_degrade_explicitly() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = hy_option(OptionType::Call, NEAR_ATM_PRICE_PCT, as_of);

    // Cash-settled valuation AT expiry degenerates to the discounted
    // intrinsic (t = 0) and stays finite and non-negative.
    let at_expiry = option
        .value(&market, option.expiry)
        .expect("cash valuation at expiry")
        .amount();
    assert!(
        at_expiry.is_finite() && at_expiry >= 0.0,
        "expiry-boundary value must be finite and non-negative, got {at_expiry}"
    );

    // Missing hazard curve: explicit error.
    let no_hazard = MarketContext::new().insert(flat_discount("USD-OIS", as_of, 0.03));
    option
        .value(&no_hazard, as_of)
        .expect_err("missing hazard curve must fail");

    // Missing discount curve: explicit error.
    let no_discount = MarketContext::new().insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02));
    option
        .value(&no_discount, as_of)
        .expect_err("missing discount curve must fail");
}
