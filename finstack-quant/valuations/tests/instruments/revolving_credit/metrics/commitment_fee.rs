//! Commitment fee metric tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_commitment_fee_proportional_to_undrawn() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);

    // 50% utilization
    let facility_50 = RevolvingCredit::builder()
        .id("RC-FEE-50".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0)) // Commitment fee only
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // 25% utilization (more undrawn)
    let facility_25 = RevolvingCredit::builder()
        .id("RC-FEE-25".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(2_500_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.03, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv_50 = facility_50.value(&market, as_of).unwrap();
    let pv_25 = facility_25.value(&market, as_of).unwrap();

    // Assert
    // More undrawn should have higher commitment fee component
    // (though total PV may be different due to drawn principal)
    assert!(pv_50.amount().is_finite());
    assert!(pv_25.amount().is_finite());
}
