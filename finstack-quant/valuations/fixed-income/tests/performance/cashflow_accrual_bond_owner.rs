//! Owner-local, untimed capture of the actual prepared bond grid and vectors.

use crate as fi;
use crate::cashflow_accrual_support as support;

use super::{BondValuator, PreparedTree, TreePricer};
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_valuations_core::pricer::ModelKey;
use serde_json::{Value, json};
use time::macros::date;

fn actual_windowed_bond_record() -> Value {
    let as_of = date!(2025 - 03 - 17);
    let bond = support::windowed_bond();
    let market = support::bond_market();
    let config = super::super::config::bond_tree_config(&bond, &market).expect("bond tree config");
    let pricer = TreePricer::with_config(config);
    let prepared = pricer
        .prepare(&bond, &market, as_of)
        .expect("actual public pricing preparation");
    let PreparedTree::HullWhite(tree, valuator) = &prepared.model else {
        panic!("windowed bond must retain its Hull-White production route");
    };
    let schedule = bond
        .raw_cashflow_schedule(&market, as_of)
        .expect("full signed schedule");
    let events = BondValuator::exercise_event_dates(&schedule);
    let right = &bond.call_put.as_ref().expect("call schedule").calls[0];
    let dates = BondValuator::exercise_candidates(
        right.start.max(as_of),
        right.end.min(bond.maturity),
        &events,
    );
    assert!(
        dates.len() > 20,
        "windowed consumer must issue repeated queries"
    );
    assert!(dates.windows(2).all(|pair| pair[0] < pair[1]));
    let cfg = bond.accrual_config();
    let query_record = support::queries(&schedule, &cfg, &dates);
    let index = finstack_quant_cashflows::accrual::AccrualIndex::build(&schedule, &cfg)
        .expect("actual claim index");
    let discount = market
        .get_discount(&bond.discount_curve_id)
        .expect("discount");
    let claims: Vec<_> = dates.iter().enumerate().map(|(ordinal, &date)| {
        let ai = index
            .accrued_at(date)
            .expect("selector-date accrued claim")
            .amount();
        let expected_ai = support::expected_linear(&schedule, date, cfg.include_pik);
        support::close(ai, expected_ai, support::AI_BOUND_USD);
        let time = discount.day_count().year_fraction(as_of, date, DayCountContext::default()).expect("exercise time");
        json!({
            "ordinal": ordinal, "date": date.to_string(), "curve_time": support::scalar(time),
            "cash_accrued": support::scalar(ai), "independent_cash_accrued": support::scalar(expected_ai),
            "clean_call_principal": support::scalar(100.0), "dirty_call_before_grid_mapping": support::scalar(100.0 + ai),
        })
    }).collect();
    let grid: Vec<_> = tree
        .time_grid()
        .iter()
        .copied()
        .map(support::scalar)
        .collect();
    assert_eq!(grid.len(), valuator.call_vec.len());
    let vectors: Vec<_> = valuator
        .call_vec
        .iter()
        .enumerate()
        .map(|(step, call)| {
            json!({"step": step, "time": support::scalar(tree.time_grid()[step]),
            "call": call.map(support::scalar), "put": valuator.put_vec[step].map(support::scalar),
            "cashflow": support::scalar(valuator.cashflow_vec[step]),
            "outstanding_principal": support::scalar(valuator.outstanding_principal_vec[step])})
        })
        .collect();
    assert!(
        valuator
            .call_vec
            .iter()
            .filter(|call| call.is_some())
            .count()
            > 20
    );
    let prepared_price = prepared.price(0.0).expect("prepared zero-OAS price").amount;
    let public_price = fi::bond::pricing::quote_conversions::price_from_oas(
        &bond,
        &market,
        as_of,
        ModelKey::Tree,
        0.0,
    )
    .expect("complete public zero-OAS price");
    assert_eq!(prepared_price.to_bits(), public_price.to_bits());
    json!({
        "fixture_id": support::WINDOWED_BOND_ID, "panel": "actual_prepared_owner_fields",
        "source": "bond::pricing::tree::tree_pricer::TreePricer::prepare",
        "as_of": as_of.to_string(), "instrument": bond, "market": market,
        "schedule": support::schedule(&schedule), "public_raw_price": support::scalar(public_price),
        "prepared_raw_price": support::scalar(prepared_price), "prepared_public_bits_equal": true,
        "selector_provenance": "production exercise_event_dates/exercise_candidates helpers; not an observed exercise policy",
        "selector_queries": query_record, "selector_claims": claims,
        "actual_grid_provenance": "time_grid and stored call/put/cashflow/outstanding vectors from the actual prepared HullWhite variant",
        "actual_grid": grid, "actual_stored_vectors": vectors,
        "mapping_note": "selector dates and grid entries are separate; no one-to-one mapping or actual optimal exercise is claimed",
        "bound_usd": support::AI_BOUND_USD,
    })
}

#[test]
fn windowed_bond_actual_grid_and_claims() {
    actual_windowed_bond_record();
}

#[test]
#[ignore = "opt-in untimed CCM-I3 actual bond consumer capture"]
fn export_actual_bond_consumer() {
    let _destination = support::export_destination();
    support::emit(actual_windowed_bond_record());
}
