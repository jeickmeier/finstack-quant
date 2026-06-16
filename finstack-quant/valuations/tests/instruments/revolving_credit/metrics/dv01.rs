//! DV01 (interest rate sensitivity) tests for revolving credit.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_dv01_sensitivity() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let facility = RevolvingCredit::builder()
        .id("RC-DV01-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of)
        .maturity(date!(2028 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.04, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let result = facility.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    // Assert
    assert!(result.is_ok());
    let result = result.unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 should be finite and non-zero
    assert!(dv01.is_finite());
    assert!(dv01.abs() > 0.0, "DV01 should not be zero");
}
