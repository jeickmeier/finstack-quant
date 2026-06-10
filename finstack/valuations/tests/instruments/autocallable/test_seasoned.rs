//! Seasoned (mid-life) autocallable pricing tests.
//!
//! A seasoned autocallable has observation dates on or before the valuation
//! date. Those dates must be evaluated against *observed fixings*, never
//! against simulated spot: the outcomes (autocall, missed memory coupons,
//! discrete knock-in monitoring) are already known at pricing time.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::helpers::*;
use finstack_core::dates::Date;
use finstack_valuations::instruments::equity::autocallable::{Autocallable, FinalPayoffType};
use finstack_valuations::instruments::Instrument;
use time::macros::date;

const SPOT: f64 = 100.0;
const VOL: f64 = 0.20;
const RATE: f64 = 0.03;
const DIV: f64 = 0.01;

fn quarterly_dates() -> Vec<Date> {
    vec![
        date!(2024 - 03 - 29),
        date!(2024 - 06 - 28),
        date!(2024 - 09 - 30),
        date!(2024 - 12 - 31),
    ]
}

fn seasoned_note(past_fixings: Vec<(Date, f64)>, initial_level: Option<f64>) -> Autocallable {
    let mut inst = create_quarterly_autocallable(
        quarterly_dates(),
        finstack_core::dates::DayCount::Act365F,
        None,
    );
    inst.initial_level = initial_level;
    inst.past_fixings = past_fixings;
    inst
}

/// Mid-life note with past observation dates but no fixings must error,
/// not silently evaluate past dates against simulated spot.
#[test]
fn past_observation_dates_without_fixings_error() {
    let as_of = date!(2024 - 07 - 15); // two observation dates already past
    let market = build_market_with_dc(
        as_of,
        SPOT,
        VOL,
        RATE,
        DIV,
        finstack_core::dates::DayCount::Act365F,
    );

    // No initial_level: must error on the strike-set level first.
    let inst = seasoned_note(vec![], None);
    let err = inst
        .value(&market, as_of)
        .expect_err("missing initial_level");
    assert!(
        err.to_string().contains("initial_level"),
        "expected initial_level error, got: {err}"
    );

    // initial_level provided but fixings missing: must error on fixings.
    let inst = seasoned_note(vec![(date!(2024 - 03 - 29), 95.0)], Some(100.0));
    let err = inst.value(&market, as_of).expect_err("missing fixing");
    assert!(
        err.to_string().contains("past_fixings"),
        "expected past_fixings error, got: {err}"
    );
}

/// A past fixing at or above its autocall barrier means the note already
/// redeemed before as_of: nothing remains to value.
#[test]
fn already_autocalled_note_values_to_zero() {
    let as_of = date!(2024 - 07 - 15);
    let market = build_market_with_dc(
        as_of,
        SPOT,
        VOL,
        RATE,
        DIV,
        finstack_core::dates::DayCount::Act365F,
    );

    // Barrier is 100% of initial_level=100; the June fixing breached it.
    let inst = seasoned_note(
        vec![
            (date!(2024 - 03 - 29), 95.0),
            (date!(2024 - 06 - 28), 104.0),
        ],
        Some(100.0),
    );
    let pv = inst.value(&market, as_of).expect("pv");
    assert_eq!(pv.amount(), 0.0);
}

/// A seasoned note that survived its past observations must price exactly
/// like a fresh note over the remaining observation dates with the same
/// strike-set level (non-memory; min-spot monitoring irrelevant for
/// Participation final payoff).
#[test]
fn surviving_seasoned_note_equals_remaining_note() {
    let as_of = date!(2024 - 07 - 15);
    let market = build_market_with_dc(
        as_of,
        SPOT,
        VOL,
        RATE,
        DIV,
        finstack_core::dates::DayCount::Act365F,
    );

    // Past fixings below the 100% barrier: note survived.
    let seasoned = seasoned_note(
        vec![(date!(2024 - 03 - 29), 95.0), (date!(2024 - 06 - 28), 97.0)],
        Some(100.0),
    );

    // Same id (same MC seed), only the remaining observation dates.
    let mut remaining = create_quarterly_autocallable(
        vec![date!(2024 - 09 - 30), date!(2024 - 12 - 31)],
        finstack_core::dates::DayCount::Act365F,
        None,
    );
    remaining.initial_level = Some(100.0);

    let pv_seasoned = seasoned.value(&market, as_of).expect("seasoned pv");
    let pv_remaining = remaining.value(&market, as_of).expect("remaining pv");
    assert!(
        (pv_seasoned.amount() - pv_remaining.amount()).abs() < 1e-9,
        "seasoned note must reduce to the remaining-dates note: seasoned={} remaining={}",
        pv_seasoned.amount(),
        pv_remaining.amount()
    );
}

/// Memory ("Phoenix") coupons missed at past observation dates must accrue
/// and be released on a future autocall — a memory seasoned note is strictly
/// more valuable than the identical non-memory note.
#[test]
fn missed_past_memory_coupons_accrue_to_future_autocall() {
    let as_of = date!(2024 - 07 - 15);
    let market = build_market_with_dc(
        as_of,
        SPOT,
        VOL,
        RATE,
        DIV,
        finstack_core::dates::DayCount::Act365F,
    );

    let fixings = vec![(date!(2024 - 03 - 29), 95.0), (date!(2024 - 06 - 28), 97.0)];

    let mut memory = seasoned_note(fixings.clone(), Some(100.0));
    memory.memory_coupons = true;
    let non_memory = seasoned_note(fixings, Some(100.0));

    let pv_memory = memory.value(&market, as_of).expect("memory pv");
    let pv_non_memory = non_memory.value(&market, as_of).expect("non-memory pv");
    assert!(
        pv_memory.amount() > pv_non_memory.amount(),
        "memory note must carry the missed past coupons: memory={} non_memory={}",
        pv_memory.amount(),
        pv_non_memory.amount()
    );
}

/// All observation dates past, never autocalled: the final payoff is fully
/// determined by the observed fixings and only needs discounting to the
/// settlement date.
#[test]
fn all_past_observations_give_deterministic_payoff() {
    let as_of = date!(2025 - 01 - 15);
    let market = build_market_with_dc(
        as_of,
        SPOT,
        VOL,
        RATE,
        DIV,
        finstack_core::dates::DayCount::Act365F,
    );

    let mut inst = seasoned_note(
        vec![
            (date!(2024 - 03 - 29), 95.0),
            (date!(2024 - 06 - 28), 90.0),
            (date!(2024 - 09 - 30), 85.0),
            (date!(2024 - 12 - 31), 92.0),
        ],
        Some(100.0),
    );
    // Settlement strictly after the last observation so the note is still alive.
    inst.expiry = date!(2025 - 01 - 31);
    inst.final_payoff_type = FinalPayoffType::Participation { rate: 1.0 };

    let pv = inst.value(&market, as_of).expect("pv");

    // Participation payoff: final fixing 92 < initial 100 => ratio 1.0
    // (principal back), discounted ~16 days at 3%.
    let df = (-RATE * 16.0 / 365.0).exp();
    let expected = 100_000.0 * df;
    assert!(
        (pv.amount() - expected).abs() / expected < 1e-3,
        "deterministic payoff expected ~{expected}, got {}",
        pv.amount()
    );

    // Knock-in put variant: min past fixing 85 > 60 barrier => not knocked
    // in => principal back. Same expected value.
    let mut ki = inst.clone();
    ki.final_payoff_type = FinalPayoffType::KnockInPut { strike: 100.0 };
    let pv_ki = ki.value(&market, as_of).expect("ki pv");
    assert!(
        (pv_ki.amount() - expected).abs() / expected < 1e-3,
        "non-knocked-in put note must return principal: expected ~{expected}, got {}",
        pv_ki.amount()
    );

    // Knocked-in variant: barrier at 90% catches the 85 fixing; final 92
    // against strike 100 => put loss 8% of initial.
    let mut ki_hit = inst.clone();
    ki_hit.final_barrier = 0.9;
    ki_hit.final_payoff_type = FinalPayoffType::KnockInPut { strike: 100.0 };
    let pv_hit = ki_hit.value(&market, as_of).expect("ki hit pv");
    let expected_hit = 100_000.0 * (1.0 - 0.08) * df;
    assert!(
        (pv_hit.amount() - expected_hit).abs() / expected_hit < 1e-3,
        "knocked-in note must pay principal minus put loss: expected ~{expected_hit}, got {}",
        pv_hit.amount()
    );
}
