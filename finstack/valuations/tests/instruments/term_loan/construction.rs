//! Term loan construction and validation tests.

use finstack_cashflows::builder::specs::{CouponType, FloatingRateSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
};
use time::macros::date;

#[test]
fn test_builder_fixed_rate_loan() {
    // Arrange & Act
    let loan = TermLoan::builder()
        .id("TL-FIXED-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 }) // 5%
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
        .build();

    // Assert
    assert!(loan.is_ok());
    let loan = loan.unwrap();
    assert_eq!(loan.id.as_str(), "TL-FIXED-001");
    assert_eq!(loan.currency, Currency::USD);
    assert!(matches!(loan.rate, RateSpec::Fixed { rate_bp: 500 }));
}

#[test]
fn test_builder_floating_rate_loan() {
    // Arrange & Act
    let loan = TermLoan::builder()
        .id("TL-FLOAT-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(5_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2028 - 01 - 01))
        .rate(RateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::from("USD-SOFR"),
            spread_bp: rust_decimal::Decimal::try_from(250.0).expect("valid"), // +250 bps
            gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
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
        }))
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
        .build();

    // Assert
    assert!(loan.is_ok());
    let loan = loan.unwrap();
    assert!(matches!(loan.rate, RateSpec::Floating(_)));
}

#[test]
fn test_builder_with_amortization() {
    // Arrange & Act
    let loan = TermLoan::builder()
        .id("TL-AMORT-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::Linear {
            start: date!(2025 - 01 - 01),
            end: date!(2030 - 01 - 01),
        })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build();

    // Assert
    assert!(loan.is_ok());
    let loan = loan.unwrap();
    assert!(matches!(loan.amortization, AmortizationSpec::Linear { .. }));
}

#[test]
fn test_builder_with_callability() {
    // Arrange & Act
    let call_schedule = LoanCallSchedule {
        calls: vec![
            LoanCall {
                date: date!(2027 - 01 - 01),
                price_pct_of_par: 102.0,
                call_type: LoanCallType::Hard,
            },
            LoanCall {
                date: date!(2028 - 01 - 01),
                price_pct_of_par: 101.0,
                call_type: LoanCallType::Hard,
            },
        ],
    };

    let loan = TermLoan::builder()
        .id("TL-CALLABLE-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 550 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .call_schedule_opt(Some(call_schedule))
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build();

    // Assert
    assert!(loan.is_ok());
    let loan = loan.unwrap();
    assert!(loan.call_schedule.is_some());
    let calls = &loan.call_schedule.as_ref().unwrap().calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].price_pct_of_par, 102.0);
}

#[test]
fn test_builder_validation_maturity_after_issue() {
    // Arrange & Act - maturity before issue should fail
    let loan = TermLoan::builder()
        .id("TL-INVALID-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2030 - 01 - 01))
        .maturity(date!(2025 - 01 - 01)) // Before issue!
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
        .build();

    // Assert
    assert!(loan.is_err());
}

#[test]
fn test_pik_coupon_type() {
    // Arrange & Act
    let loan = TermLoan::builder()
        .id("TL-PIK-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 800 }) // Higher rate for PIK
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::PIK) // Payment-in-kind
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build();

    // Assert
    assert!(loan.is_ok());
    let loan = loan.unwrap();
    assert!(matches!(loan.coupon_type, CouponType::PIK));
}
