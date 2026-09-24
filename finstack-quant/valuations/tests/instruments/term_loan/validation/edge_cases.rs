//! Edge case and boundary condition tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, CommitmentFeeBase, DdtlSpec, DrawEvent, OidPolicy, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_zero_coupon_loan() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-ZERO".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 0 }) // Zero coupon
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    // Should be deeply discounted
    assert!(pv.amount() < 10_000_000.0);
}

#[test]
fn test_very_short_maturity() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-SHORT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2025 - 04 - 01)) // 3 months
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
}

#[test]
fn test_very_long_maturity() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-LONG".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2055 - 01 - 01)) // 30 years
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
}

#[test]
fn test_negative_rate_environment() {
    // Arrange
    let as_of = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-NEGRATE".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Negative discount rate
    let disc_curve = flat_discount_curve(-0.01, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Act
    let pv = loan.value(&market, as_of);

    // Assert
    assert!(pv.is_ok());
    let pv = pv.unwrap();
    // Should be valued above par in negative rate environment
    assert!(pv.amount() > 10_000_000.0);
}

// DDTL draw capacity validation (m4) -- `TermLoan::build()` validates the contract

/// Build a 5Y fixed-rate loan carrying `ddtl`, returning the validation result.
fn build_ddtl_loan(ddtl: DdtlSpec) -> finstack_quant_core::Result<TermLoan> {
    TermLoan::builder()
        .id("TL-DDTL".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .ddtl_opt(Some(ddtl))
        .attributes(Default::default())
        .build()
}

#[test]
fn test_ddtl_draws_exceeding_commitment_rejected() {
    // Cumulative draws ($6M + $6M = $12M) exceed $10M commitment
    let result = build_ddtl_loan(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![
            DrawEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::new(6_000_000.0, Currency::USD).expect("valid money fixture"),
            },
            DrawEvent {
                date: date!(2026 - 01 - 01),
                amount: Money::new(6_000_000.0, Currency::USD).expect("valid money fixture"),
            },
        ],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: None,
    });
    let err = result.expect_err("should reject draws exceeding commitment");
    assert!(
        err.to_string()
            .contains("cumulative DDTL draws exceed the effective commitment"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_ddtl_draws_within_commitment_accepted() {
    // Cumulative draws ($4M + $4M = $8M) within $10M
    let result = build_ddtl_loan(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![
            DrawEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::new(4_000_000.0, Currency::USD).expect("valid money fixture"),
            },
            DrawEvent {
                date: date!(2026 - 01 - 01),
                amount: Money::new(4_000_000.0, Currency::USD).expect("valid money fixture"),
            },
        ],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: None,
    });
    assert!(result.is_ok(), "draws within commitment should be accepted");
}

// Negative OID percentage validation (n3)

#[test]
fn test_negative_oid_pct_rejected() {
    let result = build_ddtl_loan(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: Some(OidPolicy::WithheldPct(-100)),
    });
    let err = result.expect_err("negative OID percentage should be rejected");
    assert!(
        err.to_string().contains("OID percentage"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_zero_oid_pct_accepted() {
    let result = build_ddtl_loan(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: Some(OidPolicy::WithheldPct(0)),
    });
    assert!(result.is_ok(), "zero OID percentage should be valid");
}
