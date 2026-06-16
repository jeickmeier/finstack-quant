//! Market validation tests for option moneyness behavior (ITM/ATM/OTM).

use super::common::*;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

#[test]
fn test_itm_call_value() {
    // In-the-money call (strike below forward) should have substantial value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let itm_option = CDSOptionBuilder::new()
        .call()
        .strike(50.0) // Well below forward (~200bp)
        .build(as_of);

    let pv = itm_option.value(&market, as_of).unwrap().amount();

    assert_positive(pv, "ITM call value");

    // ITM option should be more valuable than ATM
    let atm_option = CDSOptionBuilder::new().call().strike(200.0).build(as_of);
    let atm_pv = atm_option.value(&market, as_of).unwrap().amount();

    assert!(
        pv > atm_pv,
        "ITM call {} should be more valuable than ATM {}",
        pv,
        atm_pv
    );
}

#[test]
fn test_otm_call_value() {
    // Out-of-the-money call (strike above forward) should have lower value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let otm_option = CDSOptionBuilder::new()
        .call()
        .strike(500.0) // Well above forward (~200bp)
        .build(as_of);

    let pv = otm_option.value(&market, as_of).unwrap().amount();

    // OTM still has time value
    assert_positive(pv, "OTM call value (time value)");

    // OTM option should be less valuable than ATM
    let atm_option = CDSOptionBuilder::new().call().strike(200.0).build(as_of);
    let atm_pv = atm_option.value(&market, as_of).unwrap().amount();

    assert!(
        pv < atm_pv,
        "OTM call {} should be less valuable than ATM {}",
        pv,
        atm_pv
    );
}

#[test]
fn test_atm_call_delta_positive() {
    // ATM call delta should be positive and reasonable
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let option = CDSOptionBuilder::new()
        .call()
        .strike(200.0) // Near forward
        .build(as_of);

    let delta = option.delta(&market, as_of).unwrap();

    // Delta should be positive for ATM call
    assert_positive(delta, "ATM call delta");
    // And should be finite
    assert_finite(delta, "ATM call delta");
}

#[test]
fn test_itm_call_delta_higher() {
    // Deep ITM call should have delta closer to 1.0
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let itm_option = CDSOptionBuilder::new().call().strike(50.0).build(as_of);

    let atm_option = CDSOptionBuilder::new().call().strike(200.0).build(as_of);

    let itm_delta = itm_option.delta(&market, as_of).unwrap();
    let atm_delta = atm_option.delta(&market, as_of).unwrap();

    assert!(
        itm_delta > atm_delta,
        "ITM delta {} should be > ATM delta {}",
        itm_delta,
        atm_delta
    );
}

#[test]
fn test_otm_call_delta_lower() {
    // Deep OTM call should have delta closer to 0
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let otm_option = CDSOptionBuilder::new().call().strike(500.0).build(as_of);

    let atm_option = CDSOptionBuilder::new().call().strike(200.0).build(as_of);

    let otm_delta = otm_option.delta(&market, as_of).unwrap();
    let atm_delta = atm_option.delta(&market, as_of).unwrap();

    assert!(
        otm_delta < atm_delta,
        "OTM delta {} should be < ATM delta {}",
        otm_delta,
        atm_delta
    );
}

#[test]
fn test_itm_put_value() {
    // In-the-money put (strike above forward) should have substantial value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let itm_option = CDSOptionBuilder::new()
        .put()
        .strike(400.0) // Well above forward (~200bp)
        .build(as_of);

    let pv = itm_option.value(&market, as_of).unwrap().amount();

    assert_positive(pv, "ITM put value");

    // ITM put should be more valuable than ATM
    let atm_option = CDSOptionBuilder::new().put().strike(200.0).build(as_of);
    let atm_pv = atm_option.value(&market, as_of).unwrap().amount();

    assert!(
        pv > atm_pv,
        "ITM put {} should be more valuable than ATM {}",
        pv,
        atm_pv
    );
}

#[test]
fn test_otm_put_value() {
    // Out-of-the-money put (strike below forward) should have lower value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let otm_option = CDSOptionBuilder::new()
        .put()
        .strike(100.0) // Well below forward (~200bp)
        .build(as_of);

    let pv = otm_option.value(&market, as_of).unwrap().amount();

    // OTM still has time value
    assert_positive(pv, "OTM put value (time value)");

    // OTM put should be less valuable than ATM
    let atm_option = CDSOptionBuilder::new().put().strike(200.0).build(as_of);
    let atm_pv = atm_option.value(&market, as_of).unwrap().amount();

    assert!(
        pv < atm_pv,
        "OTM put {} should be less valuable than ATM {}",
        pv,
        atm_pv
    );
}

#[test]
fn test_moneyness_value_ordering_calls() {
    // For calls: ITM > ATM > OTM in value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let itm = CDSOptionBuilder::new().call().strike(100.0).build(as_of);
    let atm = CDSOptionBuilder::new().call().strike(200.0).build(as_of);
    let otm = CDSOptionBuilder::new().call().strike(400.0).build(as_of);

    let itm_pv = itm.value(&market, as_of).unwrap().amount();
    let atm_pv = atm.value(&market, as_of).unwrap().amount();
    let otm_pv = otm.value(&market, as_of).unwrap().amount();

    assert!(
        itm_pv > atm_pv && atm_pv > otm_pv,
        "Call values ITM={} > ATM={} > OTM={} not satisfied",
        itm_pv,
        atm_pv,
        otm_pv
    );
}

#[test]
fn test_moneyness_value_ordering_puts() {
    // For puts: ITM > ATM > OTM in value
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    let itm = CDSOptionBuilder::new().put().strike(400.0).build(as_of);
    let atm = CDSOptionBuilder::new().put().strike(200.0).build(as_of);
    let otm = CDSOptionBuilder::new().put().strike(100.0).build(as_of);

    let itm_pv = itm.value(&market, as_of).unwrap().amount();
    let atm_pv = atm.value(&market, as_of).unwrap().amount();
    let otm_pv = otm.value(&market, as_of).unwrap().amount();

    assert!(
        itm_pv > atm_pv && atm_pv > otm_pv,
        "Put values ITM={} > ATM={} > OTM={} not satisfied",
        itm_pv,
        atm_pv,
        otm_pv
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_gamma_positive_all_strikes() {
    // Gamma should be positive for all strikes (convexity)
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [100.0, 150.0, 200.0, 250.0, 400.0] {
        let gamma = CDSOptionBuilder::new()
            .strike(strike)
            .build(as_of)
            .gamma(&market, as_of)
            .unwrap();

        assert_non_negative(gamma, &format!("Gamma for strike {}", strike));
        assert_finite(gamma, &format!("Gamma for strike {}", strike));
    }
}

#[test]
fn test_vega_positive_all_strikes() {
    // Vega should be positive for all strikes
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [100.0, 150.0, 200.0, 250.0, 400.0] {
        let vega = CDSOptionBuilder::new()
            .strike(strike)
            .build(as_of)
            .vega(&market, as_of)
            .unwrap();

        assert_positive(vega, &format!("Vega for strike {}", strike));
        assert_finite(vega, &format!("Vega for strike {}", strike));
    }
}
