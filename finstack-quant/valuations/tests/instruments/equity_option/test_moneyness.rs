//! Tests for option behavior across different moneyness levels (ITM/ATM/OTM).

use super::helpers::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

// ==================== VALUE TESTS ====================

#[test]
fn test_moneyness_ordering_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let itm_call = create_call(as_of, expiry, 90.0);
    let atm_call = create_call(as_of, expiry, 100.0);
    let otm_call = create_call(as_of, expiry, 110.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let itm_pv = itm_call.value(&market, as_of).unwrap().amount();
    let atm_pv = atm_call.value(&market, as_of).unwrap().amount();
    let otm_pv = otm_call.value(&market, as_of).unwrap().amount();

    // ITM > ATM > OTM
    assert!(
        itm_pv > atm_pv,
        "ITM call ({}) > ATM call ({})",
        itm_pv,
        atm_pv
    );
    assert!(
        atm_pv > otm_pv,
        "ATM call ({}) > OTM call ({})",
        atm_pv,
        otm_pv
    );
}

#[test]
fn test_moneyness_ordering_put() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let itm_put = create_put(as_of, expiry, 110.0);
    let atm_put = create_put(as_of, expiry, 100.0);
    let otm_put = create_put(as_of, expiry, 90.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let itm_pv = itm_put.value(&market, as_of).unwrap().amount();
    let atm_pv = atm_put.value(&market, as_of).unwrap().amount();
    let otm_pv = otm_put.value(&market, as_of).unwrap().amount();

    // ITM > ATM > OTM
    assert!(
        itm_pv > atm_pv,
        "ITM put ({}) > ATM put ({})",
        itm_pv,
        atm_pv
    );
    assert!(
        atm_pv > otm_pv,
        "ATM put ({}) > OTM put ({})",
        atm_pv,
        otm_pv
    );
}

// ==================== DELTA TESTS ====================

#[test]
fn test_delta_increases_with_moneyness_call() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0]; // ITM to OTM
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let mut deltas = Vec::new();
    for strike in strikes {
        let call = create_call(as_of, expiry, strike);
        let result = call
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        deltas.push(*result.measures.get("delta").unwrap());
    }

    // Delta should decrease as strike increases (moving from ITM to OTM)
    for i in 1..deltas.len() {
        assert!(
            deltas[i] < deltas[i - 1],
            "Delta should decrease from ITM to OTM: strike[{}]={} has delta {}, strike[{}]={} has delta {}",
            i-1, strikes[i-1], deltas[i-1],
            i, strikes[i], deltas[i]
        );
    }
}

#[test]
fn test_delta_behavior_put() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0]; // OTM to ITM for puts
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let mut deltas = Vec::new();
    for strike in strikes {
        let put = create_put(as_of, expiry, strike);
        let result = put
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Delta],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        deltas.push(*result.measures.get("delta").unwrap());
    }

    // Put delta should become more negative as strike increases (moving from OTM to ITM)
    for i in 1..deltas.len() {
        assert!(
            deltas[i] < deltas[i - 1],
            "Put delta should become more negative from OTM to ITM"
        );
    }
}

// ==================== GAMMA PROFILE ====================

// ==================== VEGA PROFILE ====================

// ==================== INTRINSIC VS TIME VALUE ====================

#[test]
fn test_itm_call_exceeds_intrinsic() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 90.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap().amount();
    let intrinsic = (spot - strike) * call.notional.amount();

    assert!(
        pv > intrinsic,
        "ITM call PV ({}) should exceed intrinsic ({})",
        pv,
        intrinsic
    );
}

#[test]
fn test_otm_call_is_all_time_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 110.0;
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap().amount();
    let intrinsic = (spot - strike).max(0.0) * call.notional.amount();

    // OTM intrinsic is zero
    assert_approx_eq_tol(intrinsic, 0.0, TIGHT_TOL, "OTM intrinsic");
    // PV is all time value
    assert_positive(pv, "OTM call time value");
}

#[test]
fn test_atm_call_maximizes_time_value() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let itm_call = create_call(as_of, expiry, 90.0);
    let atm_call = create_call(as_of, expiry, 100.0);
    let otm_call = create_call(as_of, expiry, 110.0);

    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let itm_pv = itm_call.value(&market, as_of).unwrap().amount();
    let atm_pv = atm_call.value(&market, as_of).unwrap().amount();
    let otm_pv = otm_call.value(&market, as_of).unwrap().amount();

    // Calculate time values
    let itm_intrinsic = (spot - 90.0) * itm_call.notional.amount();
    let atm_intrinsic = (spot - 100.0).max(0.0) * atm_call.notional.amount();
    let otm_intrinsic = (spot - 110.0).max(0.0) * otm_call.notional.amount();

    let itm_time_value = itm_pv - itm_intrinsic;
    let atm_time_value = atm_pv - atm_intrinsic;
    let otm_time_value = otm_pv - otm_intrinsic;

    // ATM should have highest time value
    assert!(
        atm_time_value >= itm_time_value,
        "ATM time value ({}) >= ITM time value ({})",
        atm_time_value,
        itm_time_value
    );
    assert!(
        atm_time_value >= otm_time_value,
        "ATM time value ({}) >= OTM time value ({})",
        atm_time_value,
        otm_time_value
    );
}

// ==================== MONEYNESS TRANSITIONS ====================

#[test]
fn test_smooth_transition_across_moneyness() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Fine grid of strikes around ATM
    let strikes: Vec<f64> = (90..=110).map(|x| x as f64).collect();
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let mut prev_pv = 0.0;
    for strike in strikes {
        let call = create_call(as_of, expiry, strike);
        let pv = call.value(&market, as_of).unwrap().amount();

        if strike > 90.0 {
            // Values should decrease smoothly
            assert!(
                pv < prev_pv,
                "Call value should decrease smoothly as strike increases"
            );
        }
        prev_pv = pv;
    }
}

#[test]
fn test_deep_itm_approaches_forward() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 50.0; // Deep ITM
    let spot = 100.0;
    let rate = 0.05;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, rate, 0.0);

    let pv = call.value(&market, as_of).unwrap().amount();

    // Deep ITM call ≈ (S - K*e^(-rT)) * contract_size
    let forward_value = (spot - strike * (-rate * 1.0).exp()) * call.notional.amount();

    assert!(
        (pv - forward_value).abs() < 100.0,
        "Deep ITM call PV ({}) should be close to forward value ({})",
        pv,
        forward_value
    );
}

#[test]
fn test_deep_otm_approaches_zero() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let strike = 200.0; // Deep OTM
    let spot = 100.0;

    let call = create_call(as_of, expiry, strike);
    let market = build_standard_market(as_of, spot, 0.25, 0.05, 0.0);

    let pv = call.value(&market, as_of).unwrap().amount();

    // Strike=200, spot=100: 100% OTM. BS price is about $0.055 per share, and
    // the test option has a 100-share notional.
    assert!(pv < 10.0, "Deep OTM call should be near zero, got {}", pv);
}
