use finstack_cashflows::builder::specs::CouponType;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::fixed_income::term_loan::{
    LoanCall, LoanCallSchedule, LoanCallType, TermLoan,
};
use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

fn build_callable_loan(as_of: Date) -> TermLoan {
    let maturity = date!(2030 - 01 - 01);
    TermLoan::builder()
        .id(InstrumentId::new("TL-CALLABLE"))
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(
            finstack_valuations::instruments::fixed_income::term_loan::RateSpec::Fixed {
                rate_bp: 600,
            },
        )
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .credit_curve_id_opt(None)
        .amortization(
            finstack_valuations::instruments::fixed_income::term_loan::AmortizationSpec::None,
        )
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(PricingOverrides::default())
        .oid_eir_opt(None)
        .call_schedule_opt(Some(LoanCallSchedule {
            calls: vec![LoanCall {
                date: date!(2027 - 01 - 01),
                price_pct_of_par: 101.0,
                call_type: LoanCallType::Hard,
            }],
        }))
        .settlement_days(1)
        .attributes(Default::default())
        .build()
        .unwrap()
}

fn base_market(as_of: Date) -> MarketContext {
    let disc = flat_discount_curve(0.05, as_of, "USD-OIS");
    MarketContext::new().insert(disc)
}

#[test]
fn callable_loan_tree_pv_is_below_straight() {
    let as_of = date!(2025 - 01 - 01);
    let loan = build_callable_loan(as_of);
    let market = base_market(as_of);

    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    let pv_callable = pricer
        .price_callable(&loan, &market, as_of)
        .unwrap()
        .amount();

    let mut straight = loan;
    straight.call_schedule = None;
    let pv_straight = pricer
        .price_callable(&straight, &market, as_of)
        .unwrap()
        .amount();

    assert!(
        pv_callable < pv_straight,
        "Callable PV should be below straight PV (borrower owns the call). callable={} straight={}",
        pv_callable,
        pv_straight
    );
}

#[test]
fn friction_cost_increases_callable_value_monotonically() {
    let as_of = date!(2025 - 01 - 01);
    let market = base_market(as_of);
    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    let mut loan0 = build_callable_loan(as_of);
    loan0.pricing_overrides.model_config.call_friction_cents = Some(0.0);
    let pv0 = pricer
        .price_callable(&loan0, &market, as_of)
        .unwrap()
        .amount();

    let mut loan50 = build_callable_loan(as_of);
    loan50.pricing_overrides.model_config.call_friction_cents = Some(50.0); // 0.50 points
    let pv50 = pricer
        .price_callable(&loan50, &market, as_of)
        .unwrap()
        .amount();

    let mut loan200 = build_callable_loan(as_of);
    loan200.pricing_overrides.model_config.call_friction_cents = Some(200.0); // 2.00 points
    let pv200 = pricer
        .price_callable(&loan200, &market, as_of)
        .unwrap()
        .amount();

    assert!(
        pv50 >= pv0,
        "PV should increase with friction: pv0={} pv50={}",
        pv0,
        pv50
    );
    assert!(
        pv200 >= pv50,
        "PV should increase with friction: pv50={} pv200={}",
        pv50,
        pv200
    );
}

#[test]
fn huge_friction_matches_straight_loan() {
    let as_of = date!(2025 - 01 - 01);
    let market = base_market(as_of);
    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    let mut callable = build_callable_loan(as_of);
    callable.pricing_overrides.model_config.call_friction_cents = Some(1_000_000.0);
    let pv_callable = pricer
        .price_callable(&callable, &market, as_of)
        .unwrap()
        .amount();

    let mut straight = callable;
    straight.call_schedule = None;
    let pv_straight = pricer
        .price_callable(&straight, &market, as_of)
        .unwrap()
        .amount();

    assert!(
        (pv_callable - pv_straight).abs() / pv_straight.max(1.0) < 5e-6,
        "With huge friction, callable PV should match straight. callable={:.2} straight={:.2}",
        pv_callable,
        pv_straight
    );
}

#[test]
fn oas_round_trip_near_zero_at_model_price() {
    let as_of = date!(2025 - 01 - 01);
    let loan = build_callable_loan(as_of);
    let market = base_market(as_of);
    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    let pv = pricer
        .price_callable(&loan, &market, as_of)
        .unwrap()
        .amount();
    let clean_pct = pv / loan.notional_limit.amount() * 100.0;

    let oas_bp = pricer
        .calculate_oas(&loan, &market, as_of, clean_pct)
        .unwrap();
    assert!(
        oas_bp.abs() < 1e-5,
        "OAS should be near 0 when using model-implied price. oas_bp={}",
        oas_bp
    );
}

#[test]
fn credit_tree_higher_hazard_lowers_price() {
    use finstack_core::market_data::term_structures::HazardCurve;
    use finstack_core::market_data::term_structures::ParInterp;

    let as_of = date!(2025 - 01 - 01);

    let disc = flat_discount_curve(0.05, as_of, "USD-OIS");

    let low_hazard = HazardCurve::builder("USD-HAZ")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.01), (5.0, 0.01)])
        .par_interp(ParInterp::Linear)
        .build()
        .unwrap();

    let high_hazard = HazardCurve::builder("USD-HAZ")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.05), (5.0, 0.05)])
        .par_interp(ParInterp::Linear)
        .build()
        .unwrap();

    let mut loan = build_callable_loan(as_of);
    loan.credit_curve_id = Some(CurveId::from("USD-HAZ"));

    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    let market_low = MarketContext::new().insert(disc.clone()).insert(low_hazard);
    let market_high = MarketContext::new().insert(disc).insert(high_hazard);

    let pv_low = pricer
        .price_callable(&loan, &market_low, as_of)
        .unwrap()
        .amount();
    let pv_high = pricer
        .price_callable(&loan, &market_high, as_of)
        .unwrap()
        .amount();

    assert!(
        pv_high < pv_low,
        "Higher hazard should lower PV. pv_low={} pv_high={}",
        pv_low,
        pv_high
    );
}

/// Regression: call at first callable date (= settlement) must produce non-zero PV.
/// Previously `outstanding_before` was initialised to 0.0, returning zero when the
/// first entry in the outstanding path matched the target date.
#[test]
fn call_at_settlement_date_produces_nonzero_pv() {
    let as_of = date!(2025 - 01 - 01);
    let market = base_market(as_of);
    let pricer =
        finstack_valuations::instruments::fixed_income::term_loan::TermLoanTreePricer::new();

    // Build a loan whose first call date equals as_of (immediate call option).
    let maturity = date!(2030 - 01 - 01);
    let loan = TermLoan::builder()
        .id(InstrumentId::new("TL-CALL-AT-SETTLE"))
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(
            finstack_valuations::instruments::fixed_income::term_loan::RateSpec::Fixed {
                rate_bp: 600,
            },
        )
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .credit_curve_id_opt(None)
        .amortization(
            finstack_valuations::instruments::fixed_income::term_loan::AmortizationSpec::None,
        )
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(PricingOverrides::default())
        .oid_eir_opt(None)
        .call_schedule_opt(Some(LoanCallSchedule {
            calls: vec![LoanCall {
                date: as_of, // Call right at settlement
                price_pct_of_par: 100.0,
                call_type: LoanCallType::Hard,
            }],
        }))
        .settlement_days(0)
        .attributes(Default::default())
        .build()
        .unwrap();

    let pv = pricer
        .price_callable(&loan, &market, as_of)
        .unwrap()
        .amount();
    assert!(
        pv > 0.0,
        "PV with call at settlement must be positive, got {}",
        pv
    );
}
