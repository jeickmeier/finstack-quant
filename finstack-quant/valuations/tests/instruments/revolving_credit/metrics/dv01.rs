//! DV01 (interest rate sensitivity) tests for revolving credit.

use finstack_quant_cashflows::builder::FloatingRateSpec;
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

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};

#[test]
fn test_dv01_sensitivity() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let facility = RevolvingCredit::builder()
        .id("RC-DV01-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .commitment_date(as_of)
        .maturity(date!(2028 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).unwrap())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
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

/// Floating-rate DV01 must capture forward-curve re-projection, not just
/// re-discounting of frozen projected cashflows.
///
/// A fully drawn quarterly floater at zero spread over its own funding curve
/// is (near) a par floater: the higher coupons after a parallel up-bump almost
/// exactly offset the heavier discounting, so its DV01 is a small fraction of
/// the equivalent fixed-rate facility's DV01. If the bump only re-discounted
/// fixed cashflows, the two DV01s would be nearly equal.
#[test]
fn floating_dv01_reflects_forward_reprojection() {
    let as_of = date!(2025 - 01 - 01);
    let rate = 0.04;
    let commitment = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");

    let build = |id: &str, base_rate_spec: BaseRateSpec| -> RevolvingCredit {
        RevolvingCredit::builder()
            .id(id.into())
            .commitment_amount(commitment)
            .drawn_amount(commitment) // fully drawn: pure interest-rate exposure
            .commitment_date(as_of)
            .maturity(date!(2030 - 01 - 01))
            .base_rate_spec(base_rate_spec)
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.0)
            .build()
            .unwrap()
    };

    let floating = build(
        "RC-DV01-FLOAT",
        BaseRateSpec::Floating(FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            gearing: rust_decimal::Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }),
    );
    let fixed = build("RC-DV01-FIXED", BaseRateSpec::Fixed { rate });

    let market = MarketContext::new()
        .insert(flat_discount_curve(rate, as_of, "USD-OIS"))
        .insert(flat_forward_curve(rate, as_of, "USD-SOFR-3M"));

    let dv01_of = |facility: &RevolvingCredit| -> f64 {
        let result = facility
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Dv01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();
        *result.measures.get("dv01").unwrap()
    };

    let dv01_fixed = dv01_of(&fixed);
    let dv01_float = dv01_of(&floating);

    assert!(
        dv01_fixed < -1_000.0,
        "5y fully drawn fixed facility should carry large negative DV01, got {dv01_fixed}"
    );
    assert!(
        dv01_float.abs() < 0.25 * dv01_fixed.abs(),
        "par floater DV01 ({dv01_float}) should be a small fraction of the fixed-rate \
         DV01 ({dv01_fixed}); near-equal magnitudes mean the bump re-discounted frozen \
         cashflows instead of re-projecting the floating leg"
    );
}
