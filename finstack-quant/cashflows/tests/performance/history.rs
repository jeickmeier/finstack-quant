//! Independent CCM-I2 numerical records, separate from clean benchmark timing.

#[path = "../../benches/support/history_workloads.rs"]
mod history_workloads;

use finstack_quant_cashflows::builder::{
    CashFlowBuilder, CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingLegCompounding,
    FloatingRateFallback, FloatingRateSpec, OvernightObservationSchedule, ScheduleParams,
};
use finstack_quant_cashflows::fixings::materialize_fixings;
use finstack_quant_core::canonical::{CANONICAL_VERSION, content_hash};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, StubKind, calendar_by_id_strict};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use history_workloads::{Case, Fixture, Leg};
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use time::Month;

fn d(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

fn flow_record(schedule: &CashFlowSchedule) -> Value {
    let bits: Vec<_> = schedule.get_flows().iter().map(|flow| {
        json!({
            "date": flow.get_date().to_string(), "kind": flow.get_kind().to_string(),
            "currency": flow.get_amount().currency().to_string(),
            "amount_decimal_bytes": flow.get_amount().amount_decimal().serialize(),
            "amount_bits": flow.get_amount().amount().to_bits(),
            "reset_date": flow.get_reset_date().map(|date| date.to_string()),
            "principal_date": flow.get_balance_date().map(|date| date.to_string()),
            "principal_delta_bytes": flow.get_principal_delta().map(|value| value.amount_decimal().serialize()),
            "accrual_factor_bits": flow.get_accrual_factor().to_bits(),
            "rate_bits": flow.get_rate().map(f64::to_bits),
            "accrual": flow.get_accrual(),
            "projected_index_rate_bits": flow.get_accrual().as_ref().and_then(|a| a.projected_index_rate).map(f64::to_bits),
        })
    }).collect();
    let fixings: Vec<_> = schedule
        .get_meta()
        .projected_fixings
        .iter()
        .map(|fixing| {
            json!({
                "series_id": fixing.series_id, "date": fixing.date.to_string(),
                "value_bits": fixing.value.map(f64::to_bits),
            })
        })
        .collect();
    json!({"schedule": schedule, "flow_bits": bits, "normalized_fixing_bits": fixings})
}

fn history_bits(history: &[(Date, f64)]) -> Value {
    Value::Array(
        history
            .iter()
            .map(|(date, value)| json!({"date":date.to_string(), "value_bits":value.to_bits()}))
            .collect(),
    )
}

fn matrix_record(fixture: &Fixture, schedule: &CashFlowSchedule) -> Value {
    let reference = history_workloads::check(fixture, schedule);
    let coupon_reference: Vec<_> = reference
        .coupons
        .iter()
        .map(|coupon| {
            json!({
                "start":coupon.start.to_string(), "end":coupon.end.to_string(),
                "payment":coupon.payment.to_string(), "reset":coupon.reset.to_string(),
                "accrual_factor":coupon.accrual_factor, "index_rate":coupon.index_rate,
                "all_in_rate":coupon.all_in_rate, "amount":coupon.amount,
            })
        })
        .collect();
    let raw_reference: Vec<_> = reference
        .raw_fixings
        .iter()
        .map(|(date, value)| {
            json!({
                "date":date.to_string(), "raw_rate":value, "raw_rate_bits":value.to_bits(),
            })
        })
        .collect();
    let descriptor = json!({
        "id": format!("{}/{}", fixture.case.leg.group(), fixture.case.suffix()),
        "leg": fixture.case.leg.index(), "issue":"2025-01-15", "maturity":"2041-01-15",
        "curve_base":"2024-12-01", "as_of":fixture.as_of.to_string(),
        "notional_usd":1_000_000, "coupons":64, "pieces":fixture.case.pieces,
        "extra_history":fixture.case.extra_history,
        "required_history_count":fixture.required_history.len(), "total_series_count":fixture.history.len(),
        "frequency_months":3, "day_count":"act_360", "calendar":"weekends_only",
        "payment_convention":"modified_following", "adjust_accrual_dates":false,
        "payment_lag_days":0, "term_reset_lag_business_days":2,
        "overnight_reset_lag":0, "spread_bp":200, "gearing":1,
        "compounding":"in_arrears_lookback0", "fallback":"error",
        "curve_knots":[[0.0,0.03],[40.0,0.05]], "curve_interpolation":"linear",
        "series_currency":"USD", "series_interpolation":"linear",
    });
    json!({
        "fixture":descriptor,
        "fixture_hash":content_hash(&descriptor).unwrap(), "market":fixture.market,
        "required_history":history_bits(&fixture.required_history),
        "required_history_hash":content_hash(&history_bits(&fixture.required_history)).unwrap(),
        "full_history":history_bits(&fixture.history),
        "full_history_hash":content_hash(&history_bits(&fixture.history)).unwrap(),
        "independent_coupon_reference":coupon_reference, "independent_raw_reference":raw_reference,
        "bounds": {
            "term_amount_usd":history_workloads::TERM_AMOUNT_BOUND,
            "overnight_amount_usd":history_workloads::OVERNIGHT_AMOUNT_BOUND,
            "term_annualized_rate":history_workloads::TERM_RATE_BOUND,
            "overnight_annualized_rate":history_workloads::RATE_BOUND, "raw_fixing_rate":1e-11,
            "accrual_factor":1e-12,
            "same_build_candidate_float_bits_and_decimal_metadata":"exact",
        },
        "output":flow_record(schedule),
        "counts": {"flows":schedule.get_flows().len(), "coupons":schedule.coupons().count(),
            "normalized_fixings":schedule.get_meta().projected_fixings.len()},
        "counter_kind":"structural counts from a separate untimed operation; no production counters",
    })
}

#[test]
fn ccm_history_axes_preserve_complete_economics() {
    for leg in [Leg::Term, Leg::Overnight] {
        let mut anchor = None;
        let mut required_hash = None;
        for case in history_workloads::cases(leg) {
            let fixture = history_workloads::fixture(case, history_workloads::mixed_cutoff());
            let market_before = content_hash(&fixture.market).unwrap();
            let schedule = fixture
                .builder
                .build_as_of(&fixture.market, fixture.as_of)
                .unwrap();
            history_workloads::check(&fixture, &schedule);
            assert_eq!(content_hash(&fixture.market).unwrap(), market_before);
            let output = flow_record(&schedule);
            if let Some(expected) = &anchor {
                assert_eq!(&output, expected);
            } else {
                anchor = Some(output);
            }
            let required = content_hash(&history_bits(&fixture.required_history)).unwrap();
            if let Some(expected) = &required_hash {
                assert_eq!(&required, expected);
            } else {
                required_hash = Some(required);
            }
        }
    }
}

#[test]
fn ccm_history_cutoffs_and_irrelevant_history_preserve_selection() {
    for leg in [Leg::Term, Leg::Overnight] {
        let fresh = history_workloads::fixture(
            Case {
                leg,
                pieces: 16,
                extra_history: 0,
            },
            history_workloads::curve_base(),
        );
        let populated = history_workloads::fixture(
            Case {
                leg,
                pieces: 16,
                extra_history: 10_000,
            },
            history_workloads::curve_base(),
        );
        let empty_output = fresh
            .builder
            .build_as_of(&fresh.market, fresh.as_of)
            .unwrap();
        let populated_output = populated
            .builder
            .build_as_of(&populated.market, populated.as_of)
            .unwrap();
        history_workloads::check(&fresh, &empty_output);
        history_workloads::check(&populated, &populated_output);
        assert_eq!(flow_record(&empty_output), flow_record(&populated_output));
        let mixed = history_workloads::fixture(
            Case {
                leg,
                pieces: 16,
                extra_history: 250,
            },
            history_workloads::mixed_cutoff(),
        );
        let implicit = mixed.builder.build(Some(&mixed.market)).unwrap();
        assert_eq!(flow_record(&implicit), flow_record(&empty_output));
        let explicit = mixed
            .builder
            .build_as_of(&mixed.market, mixed.as_of)
            .unwrap();
        history_workloads::check(&mixed, &explicit);
        assert_ne!(
            explicit.coupons().next().unwrap().get_amount(),
            implicit.coupons().next().unwrap().get_amount()
        );
        let historical = history_workloads::fixture(
            Case {
                leg,
                pieces: 16,
                extra_history: 250,
            },
            d(2041, Month::January, 16),
        );
        let output = historical
            .builder
            .build_as_of(&historical.market, historical.as_of)
            .unwrap();
        history_workloads::check(&historical, &output);
    }
}

fn short_builder(
    method: FloatingLegCompounding,
    fallback: FloatingRateFallback,
) -> CashFlowBuilder {
    let mut rate_spec = FloatingRateSpec::sofr(dec!(0));
    rate_spec.forward_curve_id = "CCM-SHORT".into();
    rate_spec.fixing_calendar_id = Some("weekends_only".into());
    rate_spec.compounding = Some(method);
    rate_spec.fallback = fallback;
    let mut params = ScheduleParams::quarterly_act360();
    params.stub = StubKind::ShortBack;
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(
            Money::new(1_000_000.0, Currency::USD).unwrap(),
            d(2025, Month::June, 2),
            d(2025, Month::June, 16),
        )
        .floating_cf(FloatingCouponSpec {
            rate_spec,
            coupon_type: CouponType::Cash,
            schedule: params,
        });
    builder
}

fn short_market(observations: Vec<(Date, f64)>) -> MarketContext {
    let forward = ForwardCurve::builder("CCM-SHORT", 1.0 / 360.0)
        .base_date(d(2025, Month::June, 9))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.09), (40.0, 0.09)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(forward);
    if observations.is_empty() {
        market
    } else {
        market.insert_series(
            ScalarTimeSeries::new("FIXING:CCM-SHORT", observations, Some(Currency::USD))
                .unwrap()
                .with_interpolation(SeriesInterpolation::Linear),
        )
    }
}

fn short_past() -> Vec<(Date, f64)> {
    (2u8..=6)
        .map(|day| {
            (
                d(2025, Month::June, day),
                0.020 + f64::from(day - 2) * 0.001,
            )
        })
        .collect()
}

fn short_record() -> Value {
    let method = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let builder = short_builder(method, FloatingRateFallback::Error);
    let cutoff = d(2025, Month::June, 9);
    let mut history = short_past();
    history.push((cutoff, 0.032));
    let market = short_market(history.clone());
    let output = builder.build_as_of(&market, cutoff).unwrap();
    let mut future_history = history;
    future_history.push((d(2025, Month::June, 10), 0.99));
    let future_output = builder
        .build_as_of(&short_market(future_history), cutoff)
        .unwrap();
    assert_eq!(flow_record(&output), flow_record(&future_output));
    let weights = [
        (0.020, 1.0),
        (0.021, 1.0),
        (0.022, 1.0),
        (0.023, 1.0),
        (0.024, 3.0),
        (0.032, 1.0),
        (0.09, 1.0),
        (0.09, 1.0),
        (0.09, 1.0),
        (0.09, 3.0),
    ];
    let expected = 1_000_000.0
        * (weights.iter().fold(1.0, |product, (rate, days)| {
            product * (1.0 + rate * days / 360.0)
        }) - 1.0);
    let amount = output.coupons().next().unwrap().get_amount().amount();
    assert!((amount - expected).abs() <= 1e-8);
    let mut missing = short_past();
    missing.retain(|(date, _)| *date != d(2025, Month::June, 4));
    let mut errors = Vec::new();
    for fallback in [
        FloatingRateFallback::Error,
        FloatingRateFallback::SpreadOnly,
        FloatingRateFallback::FixedRate(dec!(0.01)),
    ] {
        let error = short_builder(method, fallback.clone())
            .build_as_of(&short_market(missing.clone()), cutoff)
            .unwrap_err();
        assert!(error.to_string().contains("2025-06-04"));
        errors.push(json!({"fallback":fallback, "kind":error.kind().as_str(), "debug":format!("{error:?}"), "message":error.to_string()}));
    }
    let missing_error = short_builder(FloatingLegCompounding::Simple, FloatingRateFallback::Error)
        .build_as_of(&short_market(vec![]), cutoff)
        .unwrap_err();
    let fallback_output = short_builder(
        FloatingLegCompounding::Simple,
        FloatingRateFallback::FixedRate(dec!(0.01)),
    )
    .build_as_of(&short_market(vec![]), cutoff)
    .unwrap();
    let fallback_amount = fallback_output
        .coupons()
        .next()
        .unwrap()
        .get_amount()
        .amount();
    assert!((fallback_amount - 1_000_000.0 * 0.01 * 14.0 / 360.0).abs() <= 1e-8);
    json!({"id":"same_day_future_and_missing_history", "market":market,
        "observations_and_weights":weights, "expected_amount":expected, "amount_bound_usd":1e-8,
        "output":flow_record(&output), "future_junk_output":flow_record(&future_output),
        "missing_supplied_history_errors":errors,
        "absent_series_error":{"kind":missing_error.kind().as_str(),"debug":format!("{missing_error:?}"),"message":missing_error.to_string()},
        "absent_series_fixed_fallback_output":flow_record(&fallback_output)})
}

#[test]
fn ccm_history_same_day_future_and_missing_controls() {
    short_record();
}

fn holiday_record() -> Value {
    let start = d(2025, Month::April, 17);
    let end = d(2025, Month::April, 21);
    let forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
        .base_date(end)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.07), (40.0, 0.07)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(forward).insert_series(
        ScalarTimeSeries::new("FIXING:USD-SOFR", vec![(start, 0.05)], None).unwrap(),
    );
    let mut builder = CashFlowSchedule::builder();
    let output = builder
        .principal(Money::new(1_000_000.0, Currency::USD).unwrap(), start, end)
        .floating_cf(FloatingCouponSpec {
            rate_spec: FloatingRateSpec::sofr(dec!(0)),
            coupon_type: CouponType::Cash,
            schedule: ScheduleParams::quarterly_act360(),
        })
        .build(Some(&market))
        .unwrap();
    let expected = 1_000_000.0 * 0.05 * 4.0 / 360.0;
    assert_eq!(output.coupons().count(), 1);
    assert_eq!(output.get_meta().projected_fixings.len(), 1);
    assert_eq!(output.get_meta().projected_fixings[0].date, start);
    assert!((output.coupons().next().unwrap().get_amount().amount() - expected).abs() <= 1e-8);
    json!({"id":"sofr_good_friday_four_day_carry", "market":market,"expected_amount":expected,"amount_bound_usd":1e-8,"output":flow_record(&output)})
}

#[test]
fn ccm_history_sofr_holiday_carry() {
    holiday_record();
}

fn convention_records() -> Vec<Value> {
    let may = Month::May;
    let june = Month::June;
    let ordinary = vec![
        (d(2025, june, 2), 1),
        (d(2025, june, 3), 1),
        (d(2025, june, 4), 1),
        (d(2025, june, 5), 1),
        (d(2025, june, 6), 3),
        (d(2025, june, 9), 1),
        (d(2025, june, 10), 1),
        (d(2025, june, 11), 1),
        (d(2025, june, 12), 1),
        (d(2025, june, 13), 3),
    ];
    let lookback = vec![
        (d(2025, may, 29), 1),
        (d(2025, may, 30), 1),
        (d(2025, june, 2), 1),
        (d(2025, june, 3), 1),
        (d(2025, june, 4), 3),
        (d(2025, june, 5), 1),
        (d(2025, june, 6), 1),
        (d(2025, june, 9), 1),
        (d(2025, june, 10), 1),
        (d(2025, june, 11), 3),
    ];
    let shifted = vec![
        (d(2025, may, 29), 1),
        (d(2025, may, 30), 3),
        (d(2025, june, 2), 1),
        (d(2025, june, 3), 1),
        (d(2025, june, 4), 1),
        (d(2025, june, 5), 1),
        (d(2025, june, 6), 3),
        (d(2025, june, 9), 1),
        (d(2025, june, 10), 1),
        (d(2025, june, 11), 1),
    ];
    let cutoff = vec![
        (d(2025, june, 2), 1),
        (d(2025, june, 3), 1),
        (d(2025, june, 4), 1),
        (d(2025, june, 5), 1),
        (d(2025, june, 6), 3),
        (d(2025, june, 9), 1),
        (d(2025, june, 10), 1),
        (d(2025, june, 11), 1),
        (d(2025, june, 11), 1),
        (d(2025, june, 11), 3),
    ];
    [
        ("historical_lookback2",FloatingLegCompounding::CompoundedInArrears { lookback_days:2 },lookback),
        ("historical_shift2",FloatingLegCompounding::CompoundedWithObservationShift { shift_days:2 },shifted),
        ("historical_cutoff2",FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days:2 },cutoff),
        ("historical_simple_average",FloatingLegCompounding::SimpleAverage,ordinary),
    ].into_iter().map(|(id,method,expected_slices)| {
        let mut observed = std::collections::BTreeMap::new();
        for (date,_) in &expected_slices { observed.insert(*date,history_workloads::historical_rate(*date)); }
        let market = short_market(observed.iter().map(|(date,rate)| (*date,*rate)).collect());
        let output = short_builder(method,FloatingRateFallback::Error)
            .build_as_of(&market,d(2025,june,16)).unwrap();
        let compiled = OvernightObservationSchedule::compile(d(2025,june,2),d(2025,june,16),method,
            calendar_by_id_strict("weekends_only").unwrap()).unwrap();
        assert_eq!(compiled.observations().len(),expected_slices.len());
        for (actual,(date,days)) in compiled.observations().iter().zip(&expected_slices) {
            assert_eq!(actual.observation_date,*date);
            assert_eq!(i64::from(actual.weight_days),*days);
        }
        let growth = if method == FloatingLegCompounding::SimpleAverage {
            expected_slices.iter().map(|(date,days)| observed[date]*(*days as f64)/360.0).sum()
        } else {
            expected_slices.iter().fold(1.0,|product,(date,days)| product*(1.0+observed[date]*(*days as f64)/360.0))-1.0
        };
        let expected = 1_000_000.0*growth;
        assert_eq!(output.coupons().count(),1);
        assert_eq!(output.get_meta().projected_fixings.len(),observed.len());
        assert!((output.coupons().next().unwrap().get_amount().amount()-expected).abs() <= 1e-8);
        let reference:Vec<_> = expected_slices.iter().map(|(date,days)| json!({"date":date.to_string(),"weight_days":days,"raw_rate":observed[date]})).collect();
        json!({"id":id,"method":method,"market":market,"explicit_observation_reference":reference,
            "expected_amount":expected,"amount_bound_usd":1e-8,"output":flow_record(&output)})
    }).collect()
}

#[test]
fn ccm_history_lookback_shift_cutoff_and_average() {
    convention_records();
}

fn roll_record(leg: Leg) -> Value {
    let fixture = history_workloads::fixture(
        Case {
            leg,
            pieces: 16,
            extra_history: 250,
        },
        history_workloads::mixed_cutoff(),
    );
    let before = content_hash(&fixture.market).unwrap();
    let initial = fixture
        .builder
        .build_as_of(&fixture.market, fixture.as_of)
        .unwrap();
    history_workloads::check(&fixture, &initial);
    let horizon = d(2025, Month::October, 15);
    let realized =
        materialize_fixings(&fixture.market, [&initial], fixture.as_of, horizon).unwrap();
    assert_eq!(content_hash(&fixture.market).unwrap(), before);
    let curve_base_before = realized.get_forward(leg.index()).unwrap().base_date();
    assert_eq!(curve_base_before, history_workloads::curve_base());
    let roll_days = (horizon - curve_base_before).whole_days();
    let rolled = realized.roll_forward(roll_days).unwrap();
    let curve_base_after = rolled.get_forward(leg.index()).unwrap().base_date();
    assert_eq!(curve_base_after, horizon);
    let direct = fixture.builder.build_as_of(&realized, horizon).unwrap();
    let advanced = fixture.builder.build_as_of(&rolled, horizon).unwrap();
    assert_eq!(direct.get_flows().len(), advanced.get_flows().len());
    for (left, right) in direct.get_flows().iter().zip(advanced.get_flows()) {
        assert_eq!(
            (left.get_date(), left.get_kind(), left.get_reset_date()),
            (right.get_date(), right.get_kind(), right.get_reset_date())
        );
        assert_eq!(left.get_amount().currency(), right.get_amount().currency());
        assert!(
            (left.get_amount().amount() - right.get_amount().amount()).abs()
                <= history_workloads::OVERNIGHT_AMOUNT_BOUND
        );
    }
    let series_id = format!("FIXING:{}", leg.index());
    let preserved = realized.get_series(&series_id).unwrap();
    for (date, value) in &fixture.history {
        assert_eq!(
            preserved.value_on_exact(*date).unwrap().to_bits(),
            value.to_bits()
        );
    }
    let origin_fixings:Vec<_> = initial.get_meta().projected_fixings.iter().filter(|fixing| fixture.as_of <= fixing.date && fixing.date <= horizon)
        .map(|fixing| json!({"date":fixing.date.to_string(),"value_bits":fixing.value.map(f64::to_bits)})).collect();
    assert!(!origin_fixings.is_empty());
    json!({"id":format!("{}_materialize_roll_rebuild",leg.index()),"origin":fixture.as_of.to_string(),"horizon":horizon.to_string(),
        "curve_base_before":curve_base_before.to_string(),"curve_base_after":curve_base_after.to_string(),"roll_days":roll_days,
        "crossed_raw_observations":origin_fixings,"input_market_hash":before,
        "realized_market":realized,"rolled_market":rolled,"initial":flow_record(&initial),
        "unrolled_forward_rebuild":flow_record(&direct),"rolled_forward_rebuild":flow_record(&advanced),
        "roll_comparison_amount_bound_usd":history_workloads::OVERNIGHT_AMOUNT_BOUND,
        "candidate_against_C0":"exact per separate output; roll comparison is not general PV invariance"})
}

#[test]
fn ccm_history_materialize_roll_rebuild() {
    roll_record(Leg::Term);
    roll_record(Leg::Overnight);
}

#[test]
#[ignore = "CCM-I2 full numerical/metadata export; set CCM_CASHFLOW_HISTORY_EXPORT"]
fn ccm_history_accuracy_export() {
    let mut records = Vec::new();
    for leg in [Leg::Term, Leg::Overnight] {
        for case in history_workloads::cases(leg) {
            let fixture = history_workloads::fixture(case, history_workloads::mixed_cutoff());
            let before = content_hash(&fixture.market).unwrap();
            let output = fixture
                .builder
                .build_as_of(&fixture.market, fixture.as_of)
                .unwrap();
            let mut record = matrix_record(&fixture, &output);
            assert_eq!(content_hash(&fixture.market).unwrap(), before);
            record["input_market_unchanged"] = json!(true);
            records.push(record);
        }
    }
    records.push(short_record());
    records.push(holiday_record());
    records.extend(convention_records());
    records.push(roll_record(Leg::Term));
    records.push(roll_record(Leg::Overnight));
    let document = json!({"schema_version":1,"canonical_version":CANONICAL_VERSION,
        "capture_kind":"untimed deterministic numerical records, no latency/allocation claim",
        "candidate_contract":"all C0 output bits, Decimal representations, ordered metadata and error signatures must remain exact",
        "records":records});
    let path = std::env::var_os("CCM_CASHFLOW_HISTORY_EXPORT")
        .expect("explicit history export destination");
    std::fs::write(path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
}
