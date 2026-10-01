//! Loan cashflow and call-exercise regressions from the finance review.

use finstack_quant_cashflows::builder::{FloatingLegCompounding, FloatingRateSpec};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, MakeWholeSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use time::macros::date;
use time::{Duration, Weekday};

fn base_loan() -> TermLoan {
    let mut loan = TermLoan::example().expect("loan");
    loan.notional_limit = Money::new(1_000_000.0, Currency::USD).expect("notional");
    loan.issue_date = date!(2025 - 01 - 01);
    loan.maturity = date!(2026 - 01 - 01);
    loan.settlement_days = 0;
    loan.frequency = Tenor::semi_annual();
    loan.day_count = DayCount::Thirty360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.calendar_id = Some("weekends_only".into());
    loan.stub = StubKind::None;
    loan.ddtl = None;
    loan.covenants = None;
    loan.upfront_fee = None;
    loan.amortization = AmortizationSpec::None;
    loan.call_schedule = None;
    loan.credit_curve_id = None;
    loan.discount_curve_id = "D".into();
    loan.rate = RateSpec::Fixed { rate: 0.1 };
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.0);
    loan.instrument_pricing_overrides.model_config.tree_steps = Some(365);
    loan
}

fn discount_market(as_of: Date) -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("D")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 1.0)])
            .build()
            .expect("discount"),
    )
}

fn overnight_loan(mode: FloatingLegCompounding) -> (TermLoan, MarketContext, Date) {
    let mut loan = base_loan();
    loan.maturity = date!(2025 - 04 - 01);
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act360;
    let mut spec = FloatingRateSpec::sofr(0.into());
    spec.forward_curve_id = "TEST-ON".into();
    spec.reset_frequency = Tenor::quarterly();
    spec.compounding = Some(mode);
    spec.fixing_calendar_id = Some("weekends_only".into());
    loan.rate = RateSpec::Floating(spec);
    let as_of = date!(2025 - 01 - 15);
    let forward = ForwardCurve::builder("TEST-ON", 1.0 / 365.0)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.1), (1.0, 0.1)])
        .build()
        .expect("forward");
    // Include the lookback before issue; only the first accrual observation is low.
    let observations = (-7..14)
        .map(|day| {
            let d = loan.issue_date + Duration::days(day);
            (d, if day == 0 { 0.01 } else { 0.1 })
        })
        .collect();
    let history = ScalarTimeSeries::new("FIXING:TEST-ON", observations, None).expect("history");
    (
        loan,
        discount_market(as_of)
            .insert(forward)
            .insert_series(history),
        as_of,
    )
}

#[test]
fn seasoned_overnight_coupon_retains_observation_aggregation() {
    for mode in [
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        FloatingLegCompounding::SimpleAverage,
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 2 },
        FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
    ] {
        let (loan, market, as_of) = overnight_loan(mode);
        let schedule = loan.cashflow_schedule(&market, as_of).expect("schedule");
        let expected: f64 = schedule
            .get_flows()
            .iter()
            .filter(|f| f.date > as_of)
            .map(|f| f.amount.amount())
            .sum();
        let pv = loan.value(&market, as_of).expect("value").amount();
        assert!((pv - expected).abs() < 1e-6, "{mode:?}: {pv} != {expected}");
        assert!(
            pv > 1_024_000.0,
            "single low fixing must not replace the period: {pv}"
        );
    }
}

#[test]
fn seasoned_compounded_coupon_matches_daily_product_and_tree() {
    let (mut loan, market, as_of) =
        overnight_loan(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 });
    let mut product = 1.0;
    let mut start = loan.issue_date;
    while start < loan.maturity {
        let mut end = start + Duration::DAY;
        while matches!(end.weekday(), Weekday::Saturday | Weekday::Sunday) {
            end += Duration::DAY;
        }
        end = end.min(loan.maturity);
        let rate = if start == loan.issue_date { 0.01 } else { 0.1 };
        product *= 1.0 + rate * (end - start).whole_days() as f64 / 360.0;
        start = end;
    }
    let expected = 1_000_000.0 * product;
    assert!((loan.value(&market, as_of).expect("value").amount() - expected).abs() < 1e-6);
    // An out-of-the-money call routes the same overnight schedule through the tree.
    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2025 - 02 - 01),
            price_pct_of_par: 200.0,
            call_type: LoanCallType::Hard,
        }],
    });
    assert!((loan.value(&market, as_of).expect("tree").amount() - expected).abs() < 1e-4);
}

#[test]
fn seasoned_term_reset_still_uses_the_single_contractual_fixing() {
    let (loan, market, as_of) = overnight_loan(FloatingLegCompounding::Simple);
    let pv = loan.value(&market, as_of).expect("term value").amount();
    assert!((pv - 1_002_500.0).abs() < 1e-6, "{pv}");
}

#[test]
fn seasoned_overnight_preserves_an_intraperiod_balance_change() {
    use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::{
        CashSweepEvent, TermLoanCovenantEvents,
    };
    let (mut loan, market, as_of) =
        overnight_loan(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 });
    loan.covenants = Some(TermLoanCovenantEvents {
        cash_sweeps: vec![CashSweepEvent {
            date: date!(2025 - 02 - 14),
            amount: Money::new(400_000.0, Currency::USD).expect("sweep"),
        }],
        ..Default::default()
    });
    let schedule = loan
        .cashflow_schedule(&market, as_of)
        .expect("segmented schedule");
    let expected: f64 = schedule
        .get_flows()
        .iter()
        .filter(|f| f.date > as_of)
        .map(|f| f.amount.amount())
        .sum();
    let value = loan
        .value(&market, as_of)
        .expect("segmented value")
        .amount();
    assert!((value - expected).abs() < 1e-6, "{value} != {expected}");
    assert!(value > 1_019_000.0 && value < 1_021_000.0, "{value}");
}

fn make_whole_loan(spread_bp: f64) -> TermLoan {
    let mut loan = base_loan();
    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2025 - 07 - 01),
            price_pct_of_par: 100.0,
            call_type: LoanCallType::MakeWhole(MakeWholeSpec {
                reference_curve_id: "REF".into(),
                spread_bp,
            }),
        }],
    });
    loan
}

fn make_whole_market(as_of: Date, reference_rate: f64) -> MarketContext {
    discount_market(as_of).insert(
        DiscountCurve::builder("REF")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, (-10.0 * reference_rate).exp())])
            .build()
            .expect("reference"),
    )
}

#[test]
fn make_whole_calls_obey_first_exercise_bound_at_market_and_stress_spreads() {
    for spread in [25.0, 500.0] {
        let loan = make_whole_loan(spread);
        assert_eq!(loan.default_model(), ModelKey::Tree);
        let market = make_whole_market(loan.issue_date, 0.0);
        let pv = loan
            .value(&market, loan.issue_date)
            .expect("callable value")
            .amount();
        let bound = 50_000.0 + 1_050_000.0 * (-spread / 10_000.0 * 184.0 / 365.0).exp();
        assert!(
            pv <= bound + 1e-5,
            "{spread}bp: {pv} > first-call bound {bound}"
        );
        assert!(pv < 1_099_000.0, "make-whole option was stripped: {pv}");
    }
}

#[test]
fn make_whole_zero_spread_and_floor_binding_controls() {
    let loan = make_whole_loan(0.0);
    let flat = make_whole_market(loan.issue_date, 0.0);
    assert!(
        (loan
            .value(&flat, loan.issue_date)
            .expect("zero spread")
            .amount()
            - 1_100_000.0)
            .abs()
            < 1e-5
    );
    let high_reference = make_whole_market(loan.issue_date, 0.5);
    assert!(
        (loan
            .value(&high_reference, loan.issue_date)
            .expect("floor")
            .amount()
            - 1_050_000.0)
            .abs()
            < 1e-5
    );
}

#[test]
fn stochastic_rate_make_whole_is_explicitly_unsupported() {
    let mut loan = make_whole_loan(25.0);
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    let error = loan
        .value(&make_whole_market(loan.issue_date, 0.0), loan.issue_date)
        .expect_err("unsupported");
    assert!(
        error.to_string().contains("exercise-date reference curve"),
        "{error}"
    );
}

#[test]
fn make_whole_yields_use_reference_redemption() {
    let mut loan = make_whole_loan(25.0);
    loan.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(100.0);
    let value = loan
        .price_with_metrics(
            &make_whole_market(loan.issue_date, 0.0),
            loan.issue_date,
            &[MetricId::custom("ytc"), MetricId::Ytw],
            PricingOptions::default(),
        )
        .expect("yield metrics");
    let first_receipt = 50_000.0 + 1_050_000.0 * (-0.0025_f64 * 184.0 / 365.0).exp();
    let expected = (first_receipt / 1_000_000.0).powi(2) - 1.0;
    assert!(
        (value.measures["ytc"] - expected).abs() < 1e-8,
        "{:?}",
        value.measures
    );
    assert!(value.measures["ytw"] <= value.measures["ytc"]);
}

#[test]
fn mid_coupon_make_whole_yield_includes_accrued_exactly_once() {
    let mut loan = make_whole_loan(25.0);
    let call = &mut loan.call_schedule.as_mut().expect("calls").calls[0];
    call.date = date!(2025 - 04 - 01);
    call.price_pct_of_par = 120.0; // clean floor binds against the reference PV.
    loan.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(100.0);
    let value = loan
        .price_with_metrics(
            &make_whole_market(loan.issue_date, 0.0),
            loan.issue_date,
            &[MetricId::custom("ytc")],
            PricingOptions::default(),
        )
        .expect("mid-coupon yield");
    let expected = (1_225_000.0_f64 / 1_000_000.0).powi(4) - 1.0;
    assert!(
        (value.measures["ytc"] - expected).abs() < 1e-8,
        "{:?}",
        value.measures
    );
}

#[test]
fn replaced_make_whole_does_not_reject_a_live_hard_call() {
    let mut loan = make_whole_loan(25.0);
    loan.call_schedule
        .as_mut()
        .expect("calls")
        .calls
        .push(LoanCall {
            date: date!(2025 - 10 - 01),
            price_pct_of_par: 100.0,
            call_type: LoanCallType::Hard,
        });
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);
    let as_of = date!(2025 - 10 - 01);
    assert!(loan.value(&make_whole_market(as_of, 0.0), as_of).is_ok());
}
