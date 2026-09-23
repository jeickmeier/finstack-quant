//! Invalid inputs surface as errors instead of silently pricing a default:
//! a non-finite utilization in a fee-tier lookup, and an unknown
//! `pricing_model` override.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn fee_tier_lookup_rejects_non_finite_utilization() {
    let fees = RevolvingCreditFees::flat(25.0, 10.0, 0.0).expect("fees");
    assert!(fees.commitment_fee_bp(f64::NAN).is_err());
    assert!(fees.usage_fee_bp(f64::INFINITY).is_err());
    assert!(fees
        .commitment_fee_bp_at(f64::NAN, date!(2025 - 06 - 01))
        .is_err());
    assert!(fees
        .usage_fee_bp_at(f64::NAN, date!(2025 - 06 - 01))
        .is_err());
    // A finite utilization selects the tier.
    assert_eq!(fees.commitment_fee_bp(0.4).expect("tier"), 25.0);
    assert_eq!(fees.usage_fee_bp(0.4).expect("tier"), 10.0);
}

#[test]
fn unknown_pricing_model_override_is_an_error() {
    let as_of = date!(2025 - 01 - 01);
    let usd = |amount: f64| Money::new(amount, Currency::USD).expect("money");
    let mut facility = RevolvingCredit::builder()
        .id("RC-MODEL".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(5_000_000.0))
        .commitment_date(as_of)
        .maturity(date!(2027 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("facility");
    let market = MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"));
    assert!(facility.value(&market, as_of).is_ok());

    facility.attributes = Attributes::new().with_meta("pricing_model", "not_a_model");
    let err = facility
        .value(&market, as_of)
        .expect_err("unknown model must not fall back to discounting");
    assert!(err.to_string().contains("not_a_model"), "{err}");
}
