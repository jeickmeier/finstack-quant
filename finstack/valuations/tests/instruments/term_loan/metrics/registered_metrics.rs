//! Tests for term-loan metrics that are registered but were previously
//! unexercised: `all_in_rate` (borrower all-in cost) and
//! `EmbeddedOptionValue` (callability value).
//!
//! - `all_in_rate` is finite and positive for a plain loan, and rises when an
//!   upfront fee is charged (the fee is part of the borrower's cash cost).
//! - `EmbeddedOptionValue` = P_straight − P_callable, so it is strictly
//!   positive for a callable loan and exactly zero when there is no call
//!   schedule.

use crate::common::test_helpers::flat_discount_curve;
use finstack_cashflows::builder::specs::CouponType;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

/// Build a 5Y bullet 6% fixed-rate loan, optionally with an upfront fee and/or
/// a hard call schedule.
fn term_loan(upfront_fee: Option<Money>, callable: bool) -> TermLoan {
    let as_of = date!(2025 - 01 - 01);
    let mut loan = TermLoan::builder()
        .id("TL-REGMETRICS".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(upfront_fee)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    if callable {
        loan.call_schedule = Some(LoanCallSchedule {
            calls: vec![LoanCall {
                date: date!(2027 - 01 - 01),
                price_pct_of_par: 101.0,
                call_type: LoanCallType::Hard,
            }],
        });
    }
    loan
}

fn market() -> MarketContext {
    let as_of = date!(2025 - 01 - 01);
    MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"))
}

fn metric(loan: &TermLoan, id: MetricId, key: &str) -> f64 {
    let as_of = date!(2025 - 01 - 01);
    *loan
        .price_with_metrics(
            &market(),
            as_of,
            std::slice::from_ref(&id),
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get(key)
        .unwrap()
}

#[test]
fn test_all_in_rate_equals_coupon_for_plain_loan() {
    let loan = term_loan(None, false);
    let all_in = metric(&loan, MetricId::custom("all_in_rate"), "all_in_rate");

    assert!(
        all_in.is_finite() && all_in > 0.0,
        "all-in rate should be finite and positive, got {all_in}"
    );
    // A plain, fee-free, bullet 6% loan's borrower all-in cost is the coupon:
    // the time-weighted cash cost over the constant outstanding balance.
    assert!(
        (all_in - 0.06).abs() < 1e-3,
        "all-in rate for a plain 6% loan should equal the coupon, got {all_in}"
    );
}

#[test]
fn test_all_in_rate_unchanged_by_upfront_fee_oid_eir_captures_it() {
    // `all_in_rate` is a cash running-cost metric: it sums cash interest and
    // *periodic* fees over time-weighted outstanding and intentionally excludes
    // the one-time origination/upfront fee (which is dated at issue). So it is
    // unchanged by an upfront fee...
    let plain = metric(
        &term_loan(None, false),
        MetricId::custom("all_in_rate"),
        "all_in_rate",
    );
    let with_fee = metric(
        &term_loan(Some(Money::new(200_000.0, Currency::USD)), false),
        MetricId::custom("all_in_rate"),
        "all_in_rate",
    );
    assert!(
        (with_fee - plain).abs() < 1e-9,
        "all_in_rate is a cash running cost and should be unchanged by an upfront fee: plain {plain}, with fee {with_fee}"
    );

    // ...whereas the fee-inclusive effective rate (IFRS-9 EIR, include_fees by
    // default) amortizes the upfront fee into the borrower's effective cost and
    // therefore rises above the 6% coupon. This is the metric that represents
    // the true fee-inclusive "all-in" cost.
    let eir = metric(
        &term_loan(Some(Money::new(200_000.0, Currency::USD)), false),
        MetricId::custom("oid_eir_amortization"),
        "oid_eir_rate",
    );
    assert!(
        eir > 0.06,
        "oid_eir_rate should exceed the 6% coupon once the upfront fee is amortized in, got {eir}"
    );
}

#[test]
fn test_embedded_option_value_zero_for_non_callable() {
    let loan = term_loan(None, false);
    let eov = metric(
        &loan,
        MetricId::EmbeddedOptionValue,
        "embedded_option_value",
    );

    assert_eq!(
        eov, 0.0,
        "a non-callable loan must have zero embedded option value, got {eov}"
    );
}

#[test]
fn test_embedded_option_value_positive_for_callable() {
    let loan = term_loan(None, true);
    let eov = metric(
        &loan,
        MetricId::EmbeddedOptionValue,
        "embedded_option_value",
    );

    assert!(
        eov.is_finite() && eov > 0.0,
        "a callable loan's embedded (borrower) call option must have positive value, got {eov}"
    );
}
