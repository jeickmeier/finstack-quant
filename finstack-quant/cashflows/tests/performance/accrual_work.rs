//! Untimed private-input work records for the frozen CCM-I3 query matrix.
//!
//! The explicit test-only recipe matches the frozen benchmark helper; no
//! existing public fixture file, production counter or API is changed.

use super::{
    AccrualConfig, AccrualIndex, PeriodInputs, accrue_in_period, accrued_interest_amount,
    find_active_periods_and_elapsed,
};
use crate::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, RollRule, ScheduleParams};
use finstack_quant_core::canonical::{CANONICAL_VERSION, content_hash};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::mem::size_of;
use time::Month;

const EXPECTED_IDS: [&str; 20] = [
    "scaling_accrued_per_exercise_date/per_call_rebuild/60",
    "scaling_accrued_per_exercise_date/prebuilt_index/60",
    "ccm_accrued_contractual_band/early/60",
    "ccm_accrued_contractual_band/middle/60",
    "ccm_accrued_contractual_band/late/60",
    "scaling_accrued_per_exercise_date/per_call_rebuild/120",
    "scaling_accrued_per_exercise_date/prebuilt_index/120",
    "ccm_accrued_contractual_band/early/120",
    "ccm_accrued_contractual_band/middle/120",
    "ccm_accrued_contractual_band/late/120",
    "scaling_accrued_per_exercise_date/per_call_rebuild/240",
    "scaling_accrued_per_exercise_date/prebuilt_index/240",
    "ccm_accrued_contractual_band/early/240",
    "ccm_accrued_contractual_band/middle/240",
    "ccm_accrued_contractual_band/late/240",
    "scaling_accrued_per_exercise_date/per_call_rebuild/480",
    "scaling_accrued_per_exercise_date/prebuilt_index/480",
    "ccm_accrued_contractual_band/early/480",
    "ccm_accrued_contractual_band/middle/480",
    "ccm_accrued_contractual_band/late/480",
];

#[derive(Clone, Copy)]
enum Shape {
    Synthetic,
    Early,
    Middle,
    Late,
}

impl Shape {
    fn label(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::Early => "early",
            Self::Middle => "middle",
            Self::Late => "late",
        }
    }
}

fn issue() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

fn schedule(periods: usize) -> CashFlowSchedule {
    CashFlowSchedule::builder()
        .principal(
            Money::from((1_000_000_i64, Currency::USD)),
            issue(),
            issue().add_months(periods as i32).unwrap(),
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: RollRule::None,
            },
        })
        .build(None)
        .unwrap()
}

fn dates(periods: usize, shape: Shape) -> Vec<Date> {
    (0..periods)
        .map(|i| match shape {
            Shape::Synthetic => issue() + time::Duration::days(i as i64 * 30 + 15),
            shape => {
                let span = periods / 4;
                let offset = match shape {
                    Shape::Early | Shape::Synthetic => 0,
                    Shape::Middle => periods / 2,
                    Shape::Late => 3 * periods / 4,
                };
                issue()
                    .add_months((offset + i * span / periods) as i32)
                    .unwrap()
                    + time::Duration::days(15)
            }
        })
        .collect()
}

fn reference(periods: usize, as_of: Date) -> f64 {
    for period in 0..periods {
        let start = issue().add_months(period as i32).unwrap();
        let end = issue().add_months((period + 1) as i32).unwrap();
        if start <= as_of && as_of < end {
            return 1_000_000.0 * 0.06 * (as_of - start).whole_days() as f64 / 365.0;
        }
    }
    0.0
}

// Observe the current private search bounds; the independent linear model and
// original full collector below retain the frozen pre-I3 comparison.
fn observed_bounds(index: &AccrualIndex, as_of: Date) -> (usize, usize) {
    let first = index
        .period_inputs
        .partition_point(|inputs| inputs.payment_date <= as_of);
    let upper = index
        .suffix_min_coupon_start
        .partition_point(|&start| start <= as_of);
    (first, upper)
}

// Independent linear scans, deliberately not the proposed suffix index.
fn proposed_bounds(inputs: &[PeriodInputs], as_of: Date) -> (usize, usize) {
    let first = inputs
        .iter()
        .position(|p| p.payment_date > as_of)
        .unwrap_or(inputs.len());
    let upper = inputs
        .iter()
        .rposition(|p| p.coupon_start <= as_of)
        .map_or(0, |i| i + 1);
    (first, upper)
}

fn selected(
    index: &AccrualIndex,
    as_of: Date,
    bounds: (usize, usize),
) -> Vec<(&PeriodInputs, f64)> {
    let (first, upper) = bounds;
    assert!(first <= index.period_inputs.len() && upper <= index.period_inputs.len());
    if first >= upper {
        Vec::new()
    } else {
        find_active_periods_and_elapsed(&index.period_inputs[first..upper], as_of, &index.cfg)
            .expect("frozen matrix windows succeed")
    }
}

fn scalar(active: &[(&PeriodInputs, f64)], config: &AccrualConfig) -> f64 {
    let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::default();
    for &(inputs, elapsed) in active {
        total.add(accrue_in_period(inputs, elapsed, &config.method).unwrap());
    }
    let value = total.total();
    assert!(value.is_finite());
    value
}

fn identities(inputs: &[PeriodInputs], active: &[(&PeriodInputs, f64)]) -> Vec<(usize, u64)> {
    active
        .iter()
        .map(|&(selected, elapsed)| {
            let ordinal = inputs
                .iter()
                .position(|p| std::ptr::eq(p, selected))
                .unwrap();
            (ordinal, elapsed.to_bits())
        })
        .collect()
}

fn private_rows(inputs: &[PeriodInputs]) -> Vec<Value> {
    inputs.iter().enumerate().map(|(ordinal, p)| json!({
        "ordinal":ordinal, "payment_date":p.payment_date, "coupon_start":p.coupon_start,
        "start":p.start, "end":p.end, "day_count":p.day_count, "calendar_id":p.calendar_id,
        "coupon_period":p.coupon_period, "end_is_termination_date":p.end_is_termination_date,
        "notional_start_bits":p.notional_start.to_bits(),
        "coupon_total_bits":p.coupon_total.to_bits(), "total_yf_bits":p.total_yf.to_bits(),
    })).collect()
}

fn case_record(periods: usize, shape: Shape, rebuild: bool) -> Value {
    let id = match shape {
        Shape::Synthetic => format!(
            "scaling_accrued_per_exercise_date/{}/{periods}",
            if rebuild {
                "per_call_rebuild"
            } else {
                "prebuilt_index"
            },
        ),
        _ => format!("ccm_accrued_contractual_band/{}/{periods}", shape.label()),
    };
    let schedule = schedule(periods);
    let config = AccrualConfig::default();
    let dates = dates(periods, shape);
    schedule.validate().unwrap();
    assert_eq!(schedule.get_flows().len(), periods + 2);
    assert_eq!(schedule.coupons().count(), periods);
    assert!(schedule.get_flows().iter().all(|flow| {
        flow.get_amount().currency() == Currency::USD && flow.get_amount().amount().is_finite()
    }));
    let unique_dates = dates.iter().copied().collect::<BTreeSet<_>>().len();
    assert_eq!(dates.len(), periods);
    assert_eq!(
        unique_dates,
        if matches!(shape, Shape::Synthetic) {
            periods
        } else {
            periods / 4
        }
    );

    let index = AccrualIndex::build(&schedule, &config).unwrap();
    let inputs = &index.period_inputs;
    assert_eq!(inputs.len(), periods);
    assert!(
        inputs
            .windows(2)
            .all(|pair| pair[0].payment_date <= pair[1].payment_date)
    );
    let wire_input = json!({"schedule":&schedule,"config":&config,"dates":&dates});
    let rows = private_rows(inputs);
    let mut full_visits = 0usize;
    let mut actual_visits = 0usize;
    let mut proposed_visits = 0usize;
    let mut outer_full = 0.0;
    let mut outer_full_money = 0.0;
    let mut outer_public = 0.0;
    let mut outer_proposed = 0.0;
    let mut outer_reference = 0.0;
    let mut queries = Vec::with_capacity(dates.len());
    for (ordinal, &date) in dates.iter().enumerate() {
        let full = find_active_periods_and_elapsed(inputs, date, &config).unwrap();
        let actual_range = observed_bounds(&index, date);
        let proposed_range = proposed_bounds(inputs, date);
        let actual = selected(&index, date, actual_range);
        let proposed = selected(&index, date, proposed_range);
        let full_ids = identities(inputs, &full);
        assert_eq!(full_ids, identities(inputs, &actual));
        assert_eq!(full_ids, identities(inputs, &proposed));
        assert_eq!(full_ids.len(), 1);
        let full_value = scalar(&full, &config);
        let actual_value = scalar(&actual, &config);
        let proposed_value = scalar(&proposed, &config);
        let public = index.accrued_at(date).unwrap().amount();
        let full_money = Money::new(full_value, index.get_currency())
            .unwrap()
            .amount();
        assert_eq!(full_value.to_bits(), actual_value.to_bits());
        assert_eq!(full_value.to_bits(), proposed_value.to_bits());
        assert_eq!(full_money.to_bits(), public.to_bits());
        let direct = rebuild.then(|| {
            accrued_interest_amount(&schedule, date, &config)
                .unwrap()
                .amount()
        });
        if let Some(value) = direct {
            assert_eq!(value.to_bits(), public.to_bits());
        }
        let expected = reference(periods, date);
        assert!((full_value - expected).abs() <= 1e-7);
        let width = proposed_range.1.saturating_sub(proposed_range.0);
        assert_eq!(width, 1);
        full_visits += inputs.len();
        actual_visits += actual_range.1.saturating_sub(actual_range.0);
        proposed_visits += width;
        outer_full += full_value;
        outer_full_money += full_money;
        outer_public += public;
        outer_proposed += proposed_value;
        outer_reference += expected;
        queries.push(json!({
            "ordinal":ordinal,"date":date,"actual_collector_range":actual_range,
            "independent_proposed_range":proposed_range,"proposed_width":width,
            "actual_matches_proposed":actual_range == proposed_range,
            "full_ordered_contributors_and_elapsed_bits":full_ids,
            "actual_ordered_contributors_and_elapsed_bits":identities(inputs,&actual),
            "proposed_ordered_contributors_and_elapsed_bits":identities(inputs,&proposed),
            "full_scalar":full_value,"full_scalar_bits":full_value.to_bits(),
            "actual_range_scalar_bits":actual_value.to_bits(),
            "proposed_range_scalar_bits":proposed_value.to_bits(),
            "public_index_scalar_bits":public.to_bits(),"direct_scalar_bits":direct.map(f64::to_bits),
            "independent_scalar":expected,"scalar_absolute_bound_usd":1e-7,
        }));
    }
    assert_eq!(outer_full_money.to_bits(), outer_public.to_bits());
    assert_eq!(outer_full.to_bits(), outer_proposed.to_bits());
    assert!((outer_full - outer_reference).abs() <= 1e-6);
    json!({
        "id":id,"package":"finstack-quant-cashflows","target":"cashflow_scaling",
        "periods":periods,"actual_retained_P":inputs.len(),"Q":dates.len(),
        "unique_query_dates":unique_dates,"query_shape":shape.label(),"rebuild_per_query":rebuild,
        "benchmark_index_builds_source_model":if rebuild {dates.len()} else {1},
        "diagnostic_policy":"One private state index plus duplicate full/bounded references; direct public calls additionally checked for rebuild cases. Not a timing or diagnostic-total-operation count.",
        "wire_input":wire_input,"wire_input_hash":content_hash(&wire_input).unwrap(),
        "private_rows":rows,"private_rows_hash":content_hash(&rows).unwrap(),"queries":queries,
        "row_visits_source_model":{
            "full_scan_sum_P":full_visits,"observed_range_sum_width":actual_visits,
            "independent_proposed_sum_width":proposed_visits,
            "meaning":"Retained-row/range counts, not measured hardware operations or allocations.",
        },
        "proposed_suffix_payload_bytes":inputs.len()*size_of::<Date>(),
        "date_size_bytes":size_of::<Date>(),"storage_model_excludes":"Vec header, allocator overhead, capacity rounding and any diagnostic allocations.",
        "ordered_outer_sum_bits":outer_full.to_bits(),"public_ordered_outer_sum_bits":outer_public.to_bits(),
        "proposed_ordered_outer_sum_bits":outer_proposed.to_bits(),"independent_outer_sum":outer_reference,
        "ordered_sum_absolute_bound_usd":1e-6,
    })
}

fn panel() -> Value {
    let mut cases = Vec::with_capacity(20);
    for periods in [60, 120, 240, 480] {
        cases.push(case_record(periods, Shape::Synthetic, true));
        cases.push(case_record(periods, Shape::Synthetic, false));
        for shape in [Shape::Early, Shape::Middle, Shape::Late] {
            cases.push(case_record(periods, shape, false));
        }
    }
    assert_eq!(cases.len(), EXPECTED_IDS.len());
    for (case, id) in cases.iter().zip(EXPECTED_IDS) {
        assert_eq!(case["id"].as_str(), Some(id));
    }
    for group in cases.chunks_exact(5) {
        assert_eq!(group[0]["wire_input"], group[1]["wire_input"]);
    }
    let registration_ids: Vec<_> = cases
        .iter()
        .filter(|case| case["query_shape"] == "synthetic")
        .chain(
            cases
                .iter()
                .filter(|case| case["query_shape"] != "synthetic"),
        )
        .map(|case| case["id"].clone())
        .collect();
    json!({
        "schema":"ccm-i3-private-work-v1","canonical_version":CANONICAL_VERSION,
        "source_label":std::env::var("CCM_I3_WORK_SOURCE").ok(),
        "scope":"Untimed private retained-input work model and full-collector parity; no hardware, allocation, RSS or latency measurement.",
        "source_ids":{
            "owner":"finstack-quant/cashflows/src/accrual.rs",
            "frozen_fixture":"finstack-quant/cashflows/tests/performance/support/accrual_fixture.rs",
            "frozen_fixture_canonical_text_hash":content_hash(&include_str!("support/accrual_fixture.rs")).unwrap(),
            "synthetic_registration":"finstack-quant/cashflows/benches/cashflow_scaling.rs",
            "synthetic_registration_canonical_text_hash":content_hash(&include_str!("../../benches/cashflow_scaling.rs")).unwrap(),
            "band_registration":"finstack-quant/cashflows/benches/support/accrual_workloads.rs",
            "band_registration_canonical_text_hash":content_hash(&include_str!("../../benches/support/accrual_workloads.rs")).unwrap(),
            "workload_contract":"execution/B3/predeclared-02/workload-contract.json; SHA256 supplied by the root capture manifest",
        },
        "capture_order":"N-major: rebuild, prepared, early, middle, late",
        "criterion_registration_ids":registration_ids,"cases":cases,
        "public_panel_wire_projection":"Compare each wire_input exactly with the existing public panel's input.schedule.schedule + input.config + input.dates. Existing input_hash has a wider representation.",
        "semantic_gate":"Existing 26 semantic/error numerical records remain unchanged and separately exact; this valid-case work panel does not replace them.",
    })
}

#[test]
fn ccm_i3_work_matches_full_collector() {
    drop(panel());
}

#[test]
#[ignore = "Untimed private-input work export; set CCM_I3_WORK_EXPORT to a new file"]
fn ccm_i3_work_export() {
    let output = panel();
    let path = std::env::var_os("CCM_I3_WORK_EXPORT").expect("explicit new work-export path");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("create a new work-export file without overwriting evidence");
    serde_json::to_writer_pretty(&mut file, &output).unwrap();
    file.write_all(b"\n").unwrap();
}
