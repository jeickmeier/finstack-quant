//! Integration tests for CDS Option pricing workflows.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_call_option_positive_value() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().call().strike(100.0).build(as_of);
    let market = standard_market(as_of);

    let pv = option.value(&market, as_of).unwrap();

    assert_non_negative(pv.amount(), "Call option PV");
    assert_finite(pv.amount(), "Call option PV");
}

#[test]
fn test_put_option_positive_value() {
    let as_of = date!(2025 - 01 - 01);
    let option = CDSOptionBuilder::new().put().strike(100.0).build(as_of);
    let market = standard_market(as_of);

    let pv = option.value(&market, as_of).unwrap();

    assert_non_negative(pv.amount(), "Put option PV");
    assert_finite(pv.amount(), "Put option PV");
}

#[test]
fn test_atm_option_value() {
    let as_of = date!(2025 - 01 - 01);
    // Strike near forward spread (200bp based on 2% hazard * 10000 * (1-0.4))
    let option = CDSOptionBuilder::new().strike(200.0).build(as_of);
    let market = standard_market(as_of);

    let pv = option.value(&market, as_of).unwrap();

    assert_positive(pv.amount(), "ATM option should have positive time value");
}

#[test]
fn test_deep_itm_call() {
    let as_of = date!(2025 - 01 - 01);
    // Strike well below forward
    let option = CDSOptionBuilder::new().call().strike(50.0).build(as_of);
    let market = standard_market(as_of);

    let pv = option.value(&market, as_of).unwrap();

    assert_positive(pv.amount(), "Deep ITM call should have substantial value");
}

#[test]
fn test_deep_otm_call() {
    let as_of = date!(2025 - 01 - 01);
    // Strike well above forward
    let option = CDSOptionBuilder::new().call().strike(500.0).build(as_of);
    let market = standard_market(as_of);

    let pv = option.value(&market, as_of).unwrap();

    // OTM options still have time value
    assert_non_negative(pv.amount(), "OTM call should be non-negative");
}

#[test]
fn test_notional_scaling() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let option1 = CDSOptionBuilder::new()
        .notional(10_000_000.0, finstack_quant_core::currency::Currency::USD)
        .build(as_of);
    let option2 = CDSOptionBuilder::new()
        .notional(20_000_000.0, finstack_quant_core::currency::Currency::USD)
        .build(as_of);

    let pv1 = option1.value(&market, as_of).unwrap().amount();
    let pv2 = option2.value(&market, as_of).unwrap().amount();

    // Double notional should approximately double PV
    assert_approx_eq(pv2 / pv1, 2.0, 0.001, "Notional scaling");
}

#[test]
fn test_time_to_expiry_effect() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let mut values = Vec::new();
    for months in [3, 6, 12, 18, 24] {
        let option = CDSOptionBuilder::new()
            .expiry_months(months)
            .cds_maturity_months(months + 48)
            .build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();
        values.push((months as f64, pv));
    }

    // Longer time to expiry should increase option value
    assert_increasing(&values, "Time to expiry (months)", "Option value");
}

#[test]
fn test_volatility_effect() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let mut values = Vec::new();
    for vol in [0.10, 0.20, 0.30, 0.40, 0.50] {
        let option = CDSOptionBuilder::new().implied_vol(vol).build(as_of);
        let pv = option.value(&market, as_of).unwrap().amount();
        values.push((vol, pv));
    }

    // Higher volatility should increase option value
    assert_increasing(&values, "Volatility", "Option value");
}

#[test]
fn test_very_short_dated_option() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    // Option with very short time to expiry (1 week via 0 months)
    let option = CDSOptionBuilder::new()
        .expiry_months(1) // 1 month is shortest practical period
        .cds_maturity_months(13)
        .build(as_of);

    let result = option.value(&market, as_of);
    assert!(
        result.is_ok(),
        "Very short-dated option should price successfully"
    );

    // Short-dated options have value
    let pv = result.unwrap().amount();
    assert_positive(pv, "Short-dated option value");
}

#[test]
fn test_forward_spread_calculation() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let underlying = synthetic_underlying_cds(&option, as_of).expect("synthetic underlying CDS");
    let forward = underlying
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("par spread should compute")
        .measures[&MetricId::ParSpread];

    assert_positive(forward, "Forward spread");
    assert_in_range(forward, 50.0, 500.0, "Forward spread reasonableness");
}

#[test]
fn test_price_with_metrics() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[
                finstack_quant_valuations::metrics::MetricId::Delta,
                finstack_quant_valuations::metrics::MetricId::Vega,
            ],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert_non_negative(result.value.amount(), "PV in result");
    assert_eq!(result.measures.len(), 2, "Should have 2 metrics");
    assert!(result.measures.contains_key("delta"));
    assert!(result.measures.contains_key("vega"));
}

#[test]
fn test_realized_index_loss_changes_payer_and_receiver_values() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let loss = 0.01;

    let payer_base = CDSOptionBuilder::new()
        .call()
        .with_index(1.0)
        .underlying_cds_coupon_bp(100.0)
        .build(as_of);
    let mut payer_loss = payer_base.clone();
    payer_loss.realized_index_loss = Some(loss);

    let receiver_base = CDSOptionBuilder::new()
        .put()
        .with_index(1.0)
        .underlying_cds_coupon_bp(100.0)
        .build(as_of);
    let mut receiver_loss = receiver_base.clone();
    receiver_loss.realized_index_loss = Some(loss);

    let payer_base_pv = payer_base.value(&market, as_of).unwrap().amount();
    let payer_loss_pv = payer_loss.value(&market, as_of).unwrap().amount();
    let receiver_base_pv = receiver_base.value(&market, as_of).unwrap().amount();
    let receiver_loss_pv = receiver_loss.value(&market, as_of).unwrap().amount();

    assert!(
        payer_loss_pv > payer_base_pv,
        "payer no-knockout option should gain from realized index loss: base={payer_base_pv}, loss={payer_loss_pv}",
    );
    assert!(
        receiver_loss_pv < receiver_base_pv,
        "receiver no-knockout option should lose from realized index loss: base={receiver_base_pv}, loss={receiver_loss_pv}",
    );
}

#[test]
fn test_single_name_option_is_knockout_weighted_by_survival() {
    let as_of = date!(2025 - 01 - 01);
    let discount = flat_discount("USD-OIS", as_of, 0.03);
    let hazard = flat_hazard("HZ-SN", as_of, 0.4, 0.10);
    let market = MarketContext::new().insert(discount).insert(hazard);

    let single_name = CDSOptionBuilder::new()
        .call()
        .strike(100.0)
        .notional(10_000_000.0, Currency::USD)
        .build(as_of);
    let index_no_knockout = CDSOptionBuilder::new()
        .call()
        .strike(100.0)
        .notional(10_000_000.0, Currency::USD)
        .with_index(1.0)
        .underlying_cds_coupon_bp(100.0)
        .build(as_of);

    let single_pv = single_name.value(&market, as_of).unwrap().amount();
    let index_pv = index_no_knockout.value(&market, as_of).unwrap().amount();

    assert!(
        single_pv < index_pv,
        "single-name CDS options knock out on pre-expiry default and should be worth less than the no-knockout index option under material default risk: single={single_pv}, index={index_pv}",
    );
}
