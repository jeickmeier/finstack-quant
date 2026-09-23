//! A floored SOFR term loan and a fully drawn revolver on the same spec pay
//! the same coupons: both honor the spec's overnight index-floor application
//! (daily by default, or once per period), and a daily floor pays at least
//! the period floor when the overnight fixings straddle zero.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_cashflows::builder::{
    FloatingRateSpec, OvernightCompoundingMethod, OvernightIndexConstraintApplication,
};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use rust_decimal::Decimal;
use time::macros::date;

const ISSUE: Date = date!(2025 - 01 - 02);
const MATURITY: Date = date!(2025 - 07 - 02);
const NOTIONAL: f64 = 10_000_000.0;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

/// SOFR forwards at -1% for the first ~2.4 months, then +2%: the first
/// quarter's daily fixings straddle zero.
fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(ISSUE)
        .knots([(0.0, 1.0), (5.0, (-0.03_f64 * 5.0).exp())])
        .build()
        .expect("discount curve");
    let fwd = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
        .base_date(ISSUE)
        .day_count(DayCount::Act360)
        .knots([(0.0, -0.01), (0.2, -0.01), (0.21, 0.02), (2.0, 0.02)])
        .build()
        .expect("forward curve");
    MarketContext::new().insert(disc).insert(fwd)
}

fn spec(application: OvernightIndexConstraintApplication) -> FloatingRateSpec {
    FloatingRateSpec {
        index_id: "USD-SOFR-OIS".into(),
        spread_bp: Decimal::from(200),
        gearing: Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: Some(Decimal::ZERO),
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: application,
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: Some("usny".into()),
        overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
        overnight_basis: Some(DayCount::Act360),
        fallback: Default::default(),
    }
}

fn term_loan(application: OvernightIndexConstraintApplication) -> TermLoan {
    TermLoan::builder()
        .id("TL-SOFR".into())
        .currency(Currency::USD)
        .notional_limit(usd(NOTIONAL))
        .issue_date(ISSUE)
        .maturity(MATURITY)
        .rate(RateSpec::Floating(spec(application)))
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("usny".to_string()))
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .expect("term loan")
}

fn revolver(application: OvernightIndexConstraintApplication) -> RevolvingCredit {
    let mut facility = RevolvingCredit::builder()
        .id("RC-SOFR".into())
        .commitment_amount(usd(NOTIONAL))
        .drawn_amount(usd(NOTIONAL))
        .commitment_date(ISSUE)
        .maturity(MATURITY)
        .base_rate_spec(BaseRateSpec::Floating(spec(application)))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("revolver");
    facility.calendar_id = Some("usny".to_string());
    facility.business_day_convention = BusinessDayConvention::ModifiedFollowing;
    facility
}

fn coupons<T: CashflowProvider>(instrument: &T) -> Vec<(Date, f64)> {
    instrument
        .cashflow_schedule(&market(), ISSUE)
        .expect("schedule")
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| (cf.date, cf.amount.amount()))
        .collect()
}

#[test]
fn term_loan_and_revolver_pay_the_same_floored_sofr_coupons() {
    for application in [
        OvernightIndexConstraintApplication::Daily,
        OvernightIndexConstraintApplication::Period,
    ] {
        let loan = coupons(&term_loan(application));
        let facility = coupons(&revolver(application));
        assert_eq!(loan.len(), 2, "{application:?}: {loan:?}");
        assert_eq!(
            loan.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
            facility.iter().map(|(d, _)| *d).collect::<Vec<_>>(),
            "{application:?}: payment dates"
        );
        for ((date, loan_amount), (_, facility_amount)) in loan.iter().zip(&facility) {
            assert!(
                (loan_amount - facility_amount).abs() < 1e-6,
                "{application:?} {date}: term loan {loan_amount} vs revolver {facility_amount}"
            );
        }
    }
}

#[test]
fn daily_floor_pays_more_than_period_floor_when_fixings_straddle_zero() {
    let daily = coupons(&term_loan(OvernightIndexConstraintApplication::Daily));
    let period = coupons(&term_loan(OvernightIndexConstraintApplication::Period));
    // First quarter (2025-01-02 to 2025-04-02, 90 days): -1% for 72 days
    // then +2%, so the compounded period index is negative and a period
    // floor leaves only the margin: 10M × 2% × 90/360 = 50,000. A daily
    // floor keeps the positive fixings (+2% for the last ~18 days).
    assert!(
        (period[0].1 - 50_000.0).abs() < 1e-6,
        "period {}",
        period[0].1
    );
    assert!(
        daily[0].1 > period[0].1 + 5_000.0,
        "daily {} vs period {}",
        daily[0].1,
        period[0].1
    );
    // Second quarter: every fixing is +2%, so the floor never binds.
    assert!((daily[1].1 - period[1].1).abs() < 1e-6);
}

/// The revolver projects from the forward curve and has no fallback path:
/// an `index_tenor` or a non-default `fallback` would be silently ignored,
/// so validation rejects them.
#[test]
fn revolver_rejects_unsupported_floating_spec_fields() {
    use finstack_quant_cashflows::builder::FloatingRateFallback;
    use finstack_quant_valuations::instruments::Instrument;

    let with_spec = |spec: FloatingRateSpec| {
        let mut facility = revolver(OvernightIndexConstraintApplication::Daily);
        facility.base_rate_spec = BaseRateSpec::Floating(spec);
        facility
    };
    let mut tenor = spec(OvernightIndexConstraintApplication::Daily);
    tenor.index_tenor = Some(Tenor::quarterly());
    let err = with_spec(tenor)
        .validate_invariants()
        .expect_err("index_tenor");
    assert!(err.to_string().contains("index_tenor"), "{err}");

    let mut fallback = spec(OvernightIndexConstraintApplication::Daily);
    fallback.fallback = FloatingRateFallback::SpreadOnly;
    let err = with_spec(fallback)
        .validate_invariants()
        .expect_err("fallback");
    assert!(err.to_string().contains("fallback"), "{err}");

    // The supported spec validates.
    revolver(OvernightIndexConstraintApplication::Period)
        .validate_invariants()
        .expect("supported spec");
}
