//! Recovery01 tests for CDS options.

use super::common::*;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_recovery01_recalibrates_hazard_curve_with_par_spreads() {
    let as_of = date!(2025 - 01 - 01);
    let market = standard_market(as_of);
    let option = CDSOptionBuilder::new().build(as_of);

    let result = option
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Recovery01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("recovery01 should compute");
    let recovery01 = result.measures[&MetricId::Recovery01];

    // Hand-rolled frozen-curve baseline: bump only the option LGD, keep the
    // hazard λ knots intact. The hazard curve's recovery metadata is realigned
    // with each bumped option recovery so the (trade, curve) pair stays
    // consistent under the ISDA recovery-consistency guard; λ is untouched, so
    // this remains the pure LGD-only sensitivity.
    let base_recovery = option.recovery_rate;
    let mut option_up_frozen = option.clone();
    option_up_frozen.recovery_rate += 0.01;
    let market_up = frozen_market_with_recovery(&market, "HZ-SN", base_recovery + 0.01);
    let mut option_down_frozen = option.clone();
    option_down_frozen.recovery_rate -= 0.01;
    let market_down = frozen_market_with_recovery(&market, "HZ-SN", base_recovery - 0.01);
    let frozen = (option_up_frozen.value(&market_up, as_of).unwrap().amount()
        - option_down_frozen
            .value(&market_down, as_of)
            .unwrap()
            .amount())
        / 2.0;

    assert_finite(recovery01, "par-invariant Recovery01");
    assert!(
        (recovery01 - frozen).abs() > 1.0,
        "Recovery01 should rebootstrap hazard from par spreads, not match frozen-curve bump: recovery01={recovery01}, frozen={frozen}"
    );
}
