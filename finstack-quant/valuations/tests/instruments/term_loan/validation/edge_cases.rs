//! Edge case and boundary condition tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, CommitmentFeeBase, DdtlSpec, DrawEvent, OidPolicy, RateSpec, TermLoan,
    TermLoanSpec,
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 0 }) // Zero coupon
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2025 - 04 - 01)) // 3 months
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2055 - 01 - 01)) // 30 years
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
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
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
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

// ---------------------------------------------------------------------------
// DDTL draw capacity validation (m4) -- via TermLoanSpec TryFrom path
// ---------------------------------------------------------------------------

fn ddtl_spec_template() -> TermLoanSpec {
    TermLoanSpec {
        id: "TL-DDTL".into(),
        discount_curve_id: CurveId::from("USD-OIS"),
        credit_curve_id: None,
        currency: Currency::USD,
        notional_limit: Some(Money::new(1_000_000.0, Currency::USD)),
        issue: date!(2025 - 01 - 01),
        maturity: date!(2030 - 01 - 01),
        rate: RateSpec::Fixed { rate_bp: 500 },
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        amortization: AmortizationSpec::None,
        coupon_type: CouponType::Cash,
        upfront_fee: None,
        ddtl: None,
        covenants: None,
        pricing_overrides: Default::default(),
        oid_eir: None,
        call_schedule: None,
        settlement_days: 2,
    }
}

#[test]
fn test_ddtl_draws_exceeding_commitment_rejected() {
    // Cumulative draws ($6M + $6M = $12M) exceed $10M commitment
    let mut spec = ddtl_spec_template();
    spec.ddtl = Some(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![
            DrawEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::new(6_000_000.0, Currency::USD),
            },
            DrawEvent {
                date: date!(2026 - 01 - 01),
                amount: Money::new(6_000_000.0, Currency::USD),
            },
        ],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: None,
    });
    let result: Result<TermLoan, _> = spec.try_into();
    assert!(result.is_err(), "should reject draws exceeding commitment");
}

#[test]
fn test_ddtl_draws_within_commitment_accepted() {
    // Cumulative draws ($4M + $4M = $8M) within $10M
    let mut spec = ddtl_spec_template();
    spec.ddtl = Some(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![
            DrawEvent {
                date: date!(2025 - 06 - 01),
                amount: Money::new(4_000_000.0, Currency::USD),
            },
            DrawEvent {
                date: date!(2026 - 01 - 01),
                amount: Money::new(4_000_000.0, Currency::USD),
            },
        ],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: None,
    });
    let result: Result<TermLoan, _> = spec.try_into();
    assert!(result.is_ok(), "draws within commitment should be accepted");
}

// ---------------------------------------------------------------------------
// Negative OID percentage validation (n3) -- via TermLoanSpec TryFrom path
// ---------------------------------------------------------------------------

#[test]
fn test_negative_oid_pct_rejected() {
    let mut spec = ddtl_spec_template();
    spec.ddtl = Some(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: Some(OidPolicy::WithheldPct(-100)),
    });
    let result: Result<TermLoan, _> = spec.try_into();
    assert!(
        result.is_err(),
        "negative OID percentage should be rejected"
    );
}

#[test]
fn test_zero_oid_pct_accepted() {
    let mut spec = ddtl_spec_template();
    spec.ddtl = Some(DdtlSpec {
        commitment_limit: Money::new(10_000_000.0, Currency::USD),
        availability_start: date!(2025 - 01 - 01),
        availability_end: date!(2027 - 01 - 01),
        draws: vec![],
        commitment_step_downs: vec![],
        usage_fee_bp: 0,
        commitment_fee_bp: 0,
        fee_base: CommitmentFeeBase::Undrawn,
        oid_policy: Some(OidPolicy::WithheldPct(0)),
    });
    let result: Result<TermLoan, _> = spec.try_into();
    assert!(result.is_ok(), "zero OID percentage should be valid");
}
