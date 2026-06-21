//! Carry decomposition over an inter-coupon window (no coupon payment inside).
//!
//! Regression: `coupon_income` must report the coupon EARNED (≈ Δaccrued), not 0,
//! and `roll_down` must be small (not the old millions); the components must
//! partition `carry_total`.

use finstack_quant_attribution::{attribute_pnl_parallel, ExecutionPolicy};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::macros::date;

fn flat_disc(id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

#[test]
fn inter_coupon_window_coupon_income_is_accrued_and_roll_down_small() {
    let t0 = date!(2025 - 01 - 15);
    let t1 = date!(2025 - 02 - 18); // no coupon pays between (bond pays Mar/Sep 15)

    let bond = Bond::fixed(
        "ICW-BOND",
        Money::new(10_000_000.0, Currency::USD),
        0.0425,
        date!(2024 - 03 - 15),
        date!(2034 - 03 - 15),
        "USD-OIS",
    )
    .unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    // Same market at both dates → total P&L is pure carry (no MtM).
    let market = MarketContext::new().insert(flat_disc("USD-OIS", t0, 0.04));

    // Expected coupon earned = Δaccrued (accrued is curve-independent).
    let accrued = |as_of: time::Date| -> f64 {
        instrument
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Accrued],
                PricingOptions::default(),
            )
            .unwrap()
            .measures
            .get(MetricId::Accrued.as_str())
            .copied()
            .unwrap()
    };
    let delta_accrued = accrued(t1) - accrued(t0);
    assert!(
        delta_accrued > 30_000.0,
        "sanity: Δaccrued should be ~a month of 4.25% on $10M, got {delta_accrued}"
    );

    let attribution = attribute_pnl_parallel(
        &instrument,
        &market,
        &market,
        t0,
        t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("parallel attribution should succeed");

    let cd = attribution
        .carry_detail
        .as_ref()
        .expect("carry detail present");
    let coupon = cd
        .coupon_income
        .as_ref()
        .map(|l| l.total.amount())
        .unwrap_or(0.0);
    let roll = cd
        .roll_down
        .as_ref()
        .map(|l| l.total.amount())
        .unwrap_or(0.0);
    let ptp = cd.pull_to_par.map(|m| m.amount()).unwrap_or(0.0);
    let carry_total = cd.total.amount();

    // coupon income ≈ Δaccrued (was 0 before the fix)
    assert!(
        (coupon - delta_accrued).abs() < 0.02 * delta_accrued.abs() + 1.0,
        "coupon_income ({coupon:.2}) should ≈ Δaccrued ({delta_accrued:.2})"
    );
    // roll_down small, NOT the old millions
    assert!(
        roll.abs() < carry_total.abs(),
        "roll_down ({roll:.2}) should be small vs carry_total ({carry_total:.2})"
    );
    // partition: coupon_income + pull_to_par + roll_down ≈ carry_total
    let sum = coupon + ptp + roll;
    assert!(
        (sum - carry_total).abs() < 1.0,
        "partition coupon+ptp+roll ({sum:.4}) should ≈ carry_total ({carry_total:.4})"
    );
}
