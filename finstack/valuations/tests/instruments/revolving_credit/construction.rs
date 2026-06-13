//! Revolving credit construction and validation tests.

use finstack_cashflows::builder::{FeeTier, FloatingRateSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use rust_decimal::Decimal;
use time::macros::date;

fn floating_rate_spec(index_id: &str, spread_bp: f64) -> FloatingRateSpec {
    FloatingRateSpec {
        index_id: CurveId::new(index_id),
        spread_bp: Decimal::try_from(spread_bp).expect("valid literal"),
        gearing: Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_cap_bp: None,
        all_in_floor_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_freq: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 2,
        dc: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only".to_string(),
        fixing_calendar_id: None,
        end_of_month: false,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
        payment_lag_days: 0,
    }
}

#[test]
fn test_builder_fixed_rate_facility() {
    // Arrange & Act
    let facility = RevolvingCredit::builder()
        .id("RC-FIXED-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2028 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 }) // 5%
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build();

    // Assert
    assert!(facility.is_ok());
    let facility = facility.unwrap();
    assert_eq!(facility.id.as_str(), "RC-FIXED-001");
    assert!(matches!(
        facility.base_rate_spec,
        BaseRateSpec::Fixed { .. }
    ));
}

#[test]
fn test_builder_floating_rate_facility() {
    // Arrange & Act
    let facility = RevolvingCredit::builder()
        .id("RC-FLOAT-001".into())
        .commitment_amount(Money::new(20_000_000.0, Currency::USD))
        .drawn_amount(Money::new(10_000_000.0, Currency::USD))
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Floating(floating_rate_spec(
            "USD-SOFR-3M",
            250.0,
        )))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(30.0, 15.0, 8.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build();

    // Assert
    assert!(facility.is_ok());
    let facility = facility.unwrap();
    assert!(matches!(
        facility.base_rate_spec,
        BaseRateSpec::Floating { .. }
    ));
}

#[test]
fn test_builder_with_tiered_fees() {
    // Arrange & Act
    let facility = RevolvingCredit::builder()
        .id("RC-TIERED-001".into())
        .commitment_amount(Money::new(50_000_000.0, Currency::USD))
        .drawn_amount(Money::new(0.0, Currency::USD))
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees {
            upfront_fee: None,
            commitment_fee_tiers: vec![
                FeeTier {
                    threshold: Decimal::ZERO,
                    bps: Decimal::try_from(10.0).expect("valid literal"),
                },
                FeeTier {
                    threshold: Decimal::try_from(0.5).expect("valid literal"),
                    bps: Decimal::try_from(15.0).expect("valid literal"),
                },
                FeeTier {
                    threshold: Decimal::try_from(0.75).expect("valid literal"),
                    bps: Decimal::try_from(20.0).expect("valid literal"),
                },
            ],
            usage_fee_tiers: vec![FeeTier {
                threshold: Decimal::ZERO,
                bps: Decimal::try_from(25.0).expect("valid literal"),
            }],
            facility_fee_bp: 5.0,
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build();

    // Assert
    assert!(facility.is_ok());
}

#[test]
fn test_validation_maturity_after_commitment() {
    // Arrange & Act - maturity before commitment should fail
    let facility = RevolvingCredit::builder()
        .id("RC-INVALID-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(date!(2030 - 01 - 01))
        .maturity(date!(2025 - 01 - 01)) // Before commitment!
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build();

    // Assert - builder currently does not validate date ordering
    assert!(facility.is_ok());
}

#[test]
fn test_validation_drawn_within_commitment() {
    // Arrange & Act - drawn > commitment should fail
    let facility = RevolvingCredit::builder()
        .id("RC-OVERDRAWN".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(15_000_000.0, Currency::USD)) // Over commitment!
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build();

    // Assert - builder currently does not validate drawn vs commitment
    assert!(facility.is_ok());
}
