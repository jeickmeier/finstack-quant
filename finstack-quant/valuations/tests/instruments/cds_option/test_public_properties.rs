//! Public-surface CDS option properties that do not need private payoff
//! diagnostics from the Bloomberg quadrature module.

use super::common::*;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

#[test]
fn test_vol_monotonicity_and_high_vol_finiteness() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let mut values = Vec::new();

    for vol in [0.05, 0.30, 1.0, 2.0, 5.0] {
        let option = CDSOptionBuilder::new().call().implied_vol(vol).build(as_of);
        let pv = option.value(&market, as_of).expect("option value").amount();
        assert_finite(pv, "high-vol CDS option PV");
        values.push((vol, pv));
    }

    for pair in values.windows(2) {
        let (vol_lo, pv_lo) = pair[0];
        let (vol_hi, pv_hi) = pair[1];
        assert!(
            pv_hi + 1e-8 >= pv_lo,
            "option value should be non-decreasing in volatility: vol {vol_lo} -> {vol_hi}, pv {pv_lo} -> {pv_hi}",
        );
    }
}

#[test]
fn test_put_delta_sign_negative() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);

    for strike in [75.0, 100.0, 150.0, 250.0] {
        let put = CDSOptionBuilder::new().put().strike(strike).build(as_of);
        let delta = put.delta(&market, as_of).expect("put delta");
        assert!(
            delta < 0.0,
            "receiver/put CDS option delta should be negative at strike {strike}: delta={delta}",
        );
    }
}
