//! Owner-local capture of actual loan call vectors, query dates and PIK claims.

use crate::cashflow_accrual_support as support;

use super::{PreparedTree, TermLoanTreePricer, dates_on_time_grid};
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::dates::DayCountContext;
use serde_json::{Value, json};
use time::macros::date;

fn actual_callable_loan_record() -> Value {
    let as_of = date!(2025 - 01 - 01);
    let loan = support::callable_loan();
    let market = support::loan_market(as_of);
    let pricer = TermLoanTreePricer;
    let prepared = pricer
        .prepare(&loan, &market, as_of)
        .expect("actual loan preparation")
        .expect("live loan");
    let PreparedTree::ShortRate { valuator, .. } = &prepared else {
        panic!("risk-free fixed loan must retain its short-rate route");
    };
    let discount = market
        .get_discount(&loan.discount_curve_id)
        .expect("discount");
    let dates = dates_on_time_grid(
        valuator.origin,
        loan.maturity,
        discount.day_count(),
        &valuator.time_steps,
    )
    .expect("production date mapping of the stored actual grid");
    assert_eq!(dates.len(), 101);
    let claims = super::super::call::ExerciseClaims::new(&loan, &valuator.schedule)
        .expect("actual exercise claims");
    let query_dates: Vec<_> = dates
        .iter()
        .zip(&valuator.call_vec)
        .filter_map(|(&date, call)| call.is_some().then_some(date))
        .collect();
    assert!(
        query_dates.len() > 20,
        "effective-dated call must query subsequent steps"
    );
    let queries = support::queries(&valuator.schedule, &loan.accrual_config(), &query_dates);
    let actual_calls: Vec<_> = valuator.call_vec.iter().enumerate().filter_map(|(step, call)| {
        let stored = (*call)?;
        let date = dates[step];
        assert!(date < loan.maturity);
        let (outstanding, accrued) = claims.at(date).expect("actual ExerciseClaims::at");
        let expected_ai = support::expected_linear(&valuator.schedule, date, false);
        support::close(accrued, expected_ai, support::AI_BOUND_USD);
        support::close(outstanding, 10_000_000.0, support::AI_BOUND_USD);
        assert_eq!(outstanding.to_bits(), valuator.call_outstanding_vec[step].expect("stored call outstanding").to_bits());
        let event_time = discount.day_count().year_fraction(valuator.origin, date, DayCountContext::default()).expect("event time");
        let dirty = 1.01 * outstanding + accrued;
        let expected_stored = dirty * discount.df(event_time) / discount.df(valuator.time_steps[step]);
        support::close(stored, expected_stored, support::AI_BOUND_USD);
        Some(json!({
            "step": step, "query_date": date.to_string(), "actual_step_time": support::scalar(valuator.time_steps[step]),
            "event_time": support::scalar(event_time), "stored_time_offset": valuator.call_time_offset_vec[step].map(support::scalar),
            "actual_claim_principal": support::scalar(outstanding), "actual_cash_ai": support::scalar(accrued),
            "independent_cash_ai": support::scalar(expected_ai), "dirty_call_before_grid_timing": support::scalar(dirty),
            "actual_stored_call": support::scalar(stored), "independent_timed_call": support::scalar(expected_stored),
        }))
    }).collect();
    let stored_vectors: Vec<_> = valuator
        .time_steps
        .iter()
        .enumerate()
        .map(|(step, &time)| {
            json!({
                "step": step, "time": support::scalar(time), "mapped_date": dates[step].to_string(),
                "call": valuator.call_vec[step].map(support::scalar),
                "call_principal": valuator.call_outstanding_vec[step].map(support::scalar),
                "coupon_fee": support::scalar(valuator.coupon_fee_vec[step]),
                "principal_cash": support::scalar(valuator.principal_vec[step]),
                "outstanding": support::scalar(valuator.outstanding_vec[step]),
                "recovery_outstanding": support::scalar(valuator.recovery_outstanding_vec[step]),
            })
        })
        .collect();
    let public_value = pricer
        .price_callable(&loan, &market, as_of)
        .expect("complete public price");
    let prepared_settlement_value = TermLoanTreePricer::price_on_tree(&prepared, 0.0)
        .expect("actual prepared settlement value");
    json!({
        "fixture_id": support::CALLABLE_LOAN_ID, "panel": "actual_prepared_owner_fields",
        "source": "term_loan::pricing::tree_engine::TermLoanTreePricer::prepare",
        "instrument": loan, "market": market, "as_of": as_of.to_string(), "origin": valuator.origin.to_string(),
        "schedule": support::schedule(&valuator.schedule), "public_value": support::money(public_value),
        "prepared_settlement_value": support::scalar(prepared_settlement_value),
        "actual_call_claims": actual_calls, "actual_stored_vectors": stored_vectors,
        "query_records": queries,
        "date_provenance": "existing production dates_on_time_grid helper applied to the actual stored origin/time grid; only Some call-vector steps are queries",
        "mapping_note": "duplicate mapped dates are retained in tree-step order; these are candidate claims, not observed optimal exercise",
        "bound_usd": support::AI_BOUND_USD,
    })
}

fn actual_pik_claims_record() -> Value {
    let market = finstack_quant_core::market_data::context::MarketContext::new();
    let mut records = Vec::new();
    for amortizing in [false, true] {
        let loan = support::pik_loan(amortizing);
        let schedule = loan
            .raw_cashflow_schedule(&market, loan.issue_date)
            .expect("full PIK schedule");
        let claims =
            super::super::call::ExerciseClaims::new(&loan, &schedule).expect("actual PIK claims");
        let mut queries = Vec::new();
        for (date, expected_principal) in [
            (date!(2025 - 07 - 01), 105.0),
            (date!(2026 - 01 - 01), if amortizing { 55.0 } else { 110.0 }),
        ] {
            let (principal, cash) = claims.at(date).expect("actual PIK claim");
            support::close(principal, expected_principal, support::AI_BOUND_USD);
            support::close(cash, 0.0, 0.0);
            queries.push(
                json!({"date": date.to_string(), "principal": support::scalar(principal),
                "cash": support::scalar(cash), "independent_principal": expected_principal,
                "source": "actual ExerciseClaims::at; not a public reconstruction"}),
            );
        }
        records.push(json!({"amortizing": amortizing, "instrument": loan,
            "schedule": support::schedule(&schedule), "actual_claims": queries}));
    }
    json!({"fixture_id": support::PIK_LOAN_ID, "panel": "actual_owner_claims", "records": records,
        "source": "term_loan/pricing/call.rs::call_claim_includes_pik_once_after_scheduled_amortization",
        "bound_usd": support::AI_BOUND_USD})
}

#[test]
fn callable_loan_actual_grid_and_claims() {
    actual_callable_loan_record();
}

#[test]
fn pik_claims_include_capitalization_once() {
    actual_pik_claims_record();
}

#[test]
#[ignore = "opt-in untimed CCM-I3 actual loan consumer capture"]
fn export_actual_loan_consumer() {
    let _destination = support::export_destination();
    support::emit(actual_callable_loan_record());
    support::emit(actual_pik_claims_record());
}
