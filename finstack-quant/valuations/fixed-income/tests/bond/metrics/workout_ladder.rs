//! Workout ladder tests: every exit of a bond, and agreement with the `ytw` metric.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_core::{Error, InputError};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::bond::pricing::quote_conversions::{
    bond_workout_ladder, BondWorkoutLadder, BondWorkoutRow, WorkoutExitKind,
};
use finstack_quant_valuations_fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, MakeWholeSpec, ProtectionWindow, ReturnFloorSpec,
};
use time::macros::date;
use time::Date;

const AS_OF: Date = date!(2025 - 01 - 15);

fn market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .knots([(0.0, 1.0), (10.0, 0.70)])
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

fn bullet(id: &str) -> Bond {
    let mut bond = Bond::fixed(
        id,
        Money::new(1_000_000.0, Currency::USD).expect("money"),
        Rate::from_decimal(0.06).expect("rate"),
        date!(2025 - 01 - 15),
        date!(2032 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond
}

fn call(on: Date, price_pct_of_par: f64) -> CallPut {
    CallPut {
        start: on,
        end: on,
        price_pct_of_par,
        make_whole: None,
    }
}

fn quoted(mut bond: Bond, clean_price_pct: f64) -> Bond {
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price_pct(clean_price_pct);
    bond
}

fn callable(clean_price_pct: f64) -> Bond {
    let mut bond = bullet("LADDER-CALL");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![
            call(date!(2027 - 01 - 15), 103.0),
            call(date!(2029 - 01 - 15), 101.0),
        ],
        puts: vec![],
    });
    quoted(bond, clean_price_pct)
}

fn worst(ladder: &BondWorkoutLadder) -> &BondWorkoutRow {
    let marked: Vec<_> = ladder.rows.iter().filter(|row| row.is_worst).collect();
    assert_eq!(marked.len(), 1, "exactly one worst row: {ladder:?}");
    marked[0]
}

fn measure(bond: &Bond, id: MetricId, name: &str) -> f64 {
    let result = bond
        .price_with_metrics(&market(), AS_OF, &[id], PricingOptions::default())
        .expect("metric");
    *result.measures.get(name).expect("measure")
}

#[test]
fn worst_row_is_the_ytw_metric_and_the_i_spread_metric() {
    // A premium price makes the early call worst; a discount makes maturity worst.
    for (clean, worst_date) in [
        (112.0, date!(2027 - 01 - 15)),
        (88.0, date!(2032 - 01 - 15)),
    ] {
        let bond = callable(clean);
        let ladder = bond_workout_ladder(&bond, &market(), AS_OF).expect("ladder");
        let row = worst(&ladder);
        assert_eq!(row.date, worst_date, "worst date at {clean}");
        assert_eq!(
            row.yield_rate.expect("finite worst yield"),
            measure(&bond, MetricId::Ytw, "ytw"),
            "ladder and metric run one walk, so they are bit-identical at {clean}"
        );
        assert_eq!(
            row.i_spread.expect("worst spread"),
            measure(&bond, MetricId::ISpread, "i_spread"),
            "worst I-spread at {clean}"
        );
        let lowest = ladder
            .rows
            .iter()
            .filter_map(|r| r.yield_rate)
            .fold(f64::INFINITY, f64::min);
        assert_eq!(row.yield_rate, Some(lowest));
    }
}

#[test]
fn every_exit_has_a_row_in_date_order_with_its_redemption() {
    let ladder = bond_workout_ladder(&callable(101.0), &market(), AS_OF).expect("ladder");
    let summary: Vec<_> = ladder
        .rows
        .iter()
        .map(|row| (row.date, row.kind, row.redemption_price_pct))
        .collect();
    assert_eq!(
        summary,
        vec![
            (date!(2027 - 01 - 15), WorkoutExitKind::Call, 103.0),
            (date!(2029 - 01 - 15), WorkoutExitKind::Call, 101.0),
            (date!(2032 - 01 - 15), WorkoutExitKind::Maturity, 100.0),
        ]
    );
    assert_eq!(ladder.rows[0].redemption_amount.amount(), 1_030_000.0);
    assert_eq!(ladder.rows[2].redemption_amount.amount(), 1_000_000.0);
    assert_eq!(ladder.clean_price_pct, 101.0);
    assert_eq!(ladder.dirty_price.amount(), 1_010_000.0);
    assert_eq!(ladder.quote_date, AS_OF);
}

#[test]
fn a_put_gets_a_row_and_can_be_the_worst_exit() {
    let mut bond = bullet("LADDER-PUT");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![],
        puts: vec![call(date!(2027 - 01 - 15), 100.0)],
    });
    let bond = quoted(bond, 110.0);
    let ladder = bond_workout_ladder(&bond, &market(), AS_OF).expect("ladder");
    assert_eq!(ladder.rows.len(), 2);
    assert_eq!(ladder.rows[0].kind, WorkoutExitKind::Put);
    assert_eq!(
        worst(&ladder).yield_rate,
        Some(measure(&bond, MetricId::Ytw, "ytw"))
    );
}

#[test]
fn a_bond_with_no_exercise_rights_has_one_maturity_row() {
    // With no quote, the target is the model price, exactly as the metric's.
    let plain = bullet("LADDER-BULLET");
    let ladder = bond_workout_ladder(&plain, &market(), AS_OF).expect("ladder");
    assert_eq!(ladder.rows.len(), 1);
    assert_eq!(ladder.rows[0].kind, WorkoutExitKind::Maturity);
    assert_eq!(worst(&ladder).date, plain.maturity);
    assert_eq!(
        ladder.rows[0].yield_rate,
        Some(measure(&plain, MetricId::Ytw, "ytw"))
    );

    let at_price = quoted(bullet("LADDER-QUOTED"), 97.5);
    let ladder = bond_workout_ladder(&at_price, &market(), AS_OF).expect("ladder");
    assert_eq!(ladder.rows.len(), 1);
    assert_eq!(ladder.clean_price_pct, 97.5);
    assert_eq!(
        ladder.rows[0].yield_rate,
        Some(measure(&at_price, MetricId::Ytw, "ytw"))
    );
    assert_eq!(
        ladder.rows[0].i_spread,
        Some(measure(&at_price, MetricId::ISpread, "i_spread"))
    );
}

#[test]
fn exercise_rights_without_a_quote_fail_as_the_metric_does() {
    let mut bond = callable(100.0);
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default();
    let error = bond_workout_ladder(&bond, &market(), AS_OF).expect_err("needs a quote");
    assert!(
        matches!(&error, Error::Input(InputError::NotFound { id }) if id.ends_with("quoted_clean_price_pct")),
        "{error:?}"
    );
    assert!(bond
        .price_with_metrics(
            &market(),
            AS_OF,
            &[MetricId::Ytw],
            PricingOptions::default()
        )
        .is_err());
}

#[test]
fn a_return_floor_is_lowered_into_call_rows() {
    let exercise = date!(2026 - 07 - 15);
    let floored = quoted(bullet("LADDER-FLOOR"), 104.0).with_return_floor(
        ReturnFloorSpec::moic(1.20).window(ProtectionWindow::Between {
            start: exercise,
            end: exercise.next_day().expect("following date"),
        }),
    );
    assert!(floored.call_put.is_none());
    let ladder = bond_workout_ladder(&floored, &market(), AS_OF).expect("ladder");
    assert!(
        ladder
            .rows
            .iter()
            .any(|row| row.kind == WorkoutExitKind::Call),
        "{ladder:?}"
    );
    assert_eq!(
        ladder.rows.last().map(|r| r.kind),
        Some(WorkoutExitKind::Maturity)
    );
    assert_eq!(
        worst(&ladder).yield_rate,
        Some(measure(&floored, MetricId::Ytw, "ytw"))
    );
}

#[test]
fn a_make_whole_call_redeems_above_its_strike() {
    let mut bond = bullet("LADDER-MW");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            make_whole: Some(MakeWholeSpec {
                reference_curve_id: "USD-OIS".into(),
                spread_bp: 25.0,
            }),
            ..call(date!(2027 - 01 - 15), 100.0)
        }],
        puts: vec![],
    });
    let bond = quoted(bond, 105.0);
    let ladder = bond_workout_ladder(&bond, &market(), AS_OF).expect("ladder");
    let row = &ladder.rows[0];
    assert!(row.make_whole);
    assert!(row.redemption_price_pct > 100.0, "{row:?}");
    let from_pct = row.redemption_price_pct / 100.0 * 1_000_000.0;
    assert!((row.redemption_amount.amount() - from_pct).abs() < 1e-6);
    assert!(!ladder.rows[1].make_whole);
}

#[test]
fn an_exit_on_the_quote_date_has_no_yield() {
    // Callable today above the price: no future cash on that path, so no yield.
    let mut bond = bullet("LADDER-TODAY");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![call(AS_OF, 105.0)],
        puts: vec![],
    });
    let bond = quoted(bond, 100.0);
    let ladder = bond_workout_ladder(&bond, &market(), AS_OF).expect("ladder");
    let today = &ladder.rows[0];
    assert_eq!(today.date, AS_OF);
    assert_eq!(today.yield_rate, None);
    assert_eq!(today.i_spread, None);
    assert!(!today.is_worst);
    assert_eq!(
        worst(&ladder).yield_rate,
        Some(measure(&bond, MetricId::Ytw, "ytw"))
    );

    // Callable today below the price: the limiting worst path, also the metric's.
    let mut below = bullet("LADDER-TODAY-BELOW");
    below.call_put = Some(CallPutSchedule {
        calls: vec![call(AS_OF, 95.0)],
        puts: vec![],
    });
    let below = quoted(below, 100.0);
    let ladder = bond_workout_ladder(&below, &market(), AS_OF).expect("ladder");
    let today = worst(&ladder);
    assert_eq!(today.date, AS_OF);
    assert_eq!(today.yield_rate, None);
    assert_eq!(measure(&below, MetricId::Ytw, "ytw"), f64::NEG_INFINITY);
}

#[test]
fn the_result_round_trips_and_rejects_unknown_fields() {
    let ladder = bond_workout_ladder(&callable(101.0), &market(), AS_OF).expect("ladder");
    let json = serde_json::to_value(&ladder).expect("serialize");
    assert_eq!(json["rows"][0]["kind"], "call");
    assert_eq!(json["rows"][2]["kind"], "maturity");
    let back: BondWorkoutLadder = serde_json::from_value(json.clone()).expect("deserialize");
    assert_eq!(back, ladder);

    let mut unknown = json;
    unknown["surprise"] = serde_json::json!(1);
    assert!(serde_json::from_value::<BondWorkoutLadder>(unknown).is_err());
}
