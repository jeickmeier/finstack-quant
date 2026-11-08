//! Numerical and error contracts for the CCM accrued-interest investigation.
//!
//! The opt-in export is untimed. Scalar and outer-sum bits freeze the C0 result;
//! independent formula tolerances establish fixture accuracy, not candidate drift.

#[path = "support/accrual_fixture.rs"]
mod fixture;

use finstack_quant_cashflows::CashflowScheduleBuildSpec;
use finstack_quant_cashflows::accrual::{
    AccrualConfig, AccrualIndex, AccrualMethod, ExCouponRule, accrued_interest_amount,
};
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::Error;
use finstack_quant_core::canonical::{CANONICAL_VERSION, content_hash};
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor, parse_iso_date};
use finstack_quant_core::money::Money;
use serde_json::{Value, json};

fn date(value: &str) -> Date {
    parse_iso_date(value).expect("fixture date")
}

fn row(
    payment: &str,
    start: &str,
    end: &str,
    amount: f64,
    day_count: DayCount,
    factor: f64,
) -> CashFlow {
    let start = date(start);
    let end = date(end);
    CashFlow::new(
        date(payment),
        None,
        Money::new(amount, Currency::USD).expect("row amount"),
        CFKind::Fixed,
        factor,
        None,
    )
    .with_accrual(CashFlowAccrual {
        start,
        end,
        day_count,
        calendar_id: None,
        coupon_period: Some((start, end)),
        end_is_termination_date: false,
        projected_index_rate: None,
    })
}

fn hand_schedule(
    flows: Vec<CashFlow>,
    issue: &str,
    principal: f64,
    day_count: DayCount,
) -> CashFlowSchedule {
    let result = CashFlowSchedule::from_flows(
        flows,
        Notional::par(principal, Currency::USD).expect("fixture principal"),
        day_count,
        CashFlowMeta {
            issue_date: Some(date(issue)),
            ..Default::default()
        },
    )
    .expect("valid public hand rows");
    result.validate().expect("valid hand schedule");
    result
}

fn schedule_record(schedule: &CashFlowSchedule) -> Value {
    let rows: Vec<_> = schedule.get_flows().iter().enumerate().map(|(order,flow)| {
        json!({
            "order":order,"row":flow,
            "amount_bits":flow.amount.amount().to_bits(),
            "amount_decimal_bytes":flow.amount.amount_decimal().serialize(),
            "accrual_factor_bits":flow.accrual_factor.to_bits(),
            "rate_bits":flow.rate.map(f64::to_bits),
            "projected_index_rate_bits":flow.accrual.as_ref().and_then(|a|a.projected_index_rate).map(f64::to_bits),
            "principal_delta_decimal_bytes":flow.principal_delta.map(|m|m.amount_decimal().serialize()),
            "principal_delta_bits":flow.principal_delta.map(|m|m.amount().to_bits())
        })
    }).collect();
    json!({
        "schedule":schedule,"ordered_rows":rows,
        "initial_notional_decimal_bytes":schedule.get_notional().initial.amount_decimal().serialize(),
        "initial_notional_bits":schedule.get_notional().initial.amount().to_bits(),
        "flow_count":schedule.get_flows().len(),"cash_coupon_count":schedule.coupons().count()
    })
}

fn success_record(
    id: &str,
    schedule: &CashFlowSchedule,
    config: &AccrualConfig,
    queries: &[(Date, f64)],
    bound: f64,
) -> Value {
    let index = AccrualIndex::build(schedule, config).expect("semantic index build");
    let mut values = Vec::with_capacity(queries.len());
    let mut total = 0.0;
    let mut reference_total = 0.0;
    for (order, &(as_of, expected)) in queries.iter().enumerate() {
        let direct =
            accrued_interest_amount(schedule, as_of, config).expect("direct semantic accrual");
        let indexed = index.accrued_at(as_of).expect("indexed semantic accrual");
        assert_eq!(
            direct.to_bits(),
            indexed.to_bits(),
            "direct/index query {id}/{order}"
        );
        fixture::close(indexed, expected, bound);
        total += indexed;
        reference_total += expected;
        values.push(json!({
            "order":order,"as_of":as_of,"actual":indexed,"bits":indexed.to_bits(),
            "direct_bits":direct.to_bits(),"expected":expected,"expected_bits":expected.to_bits(),
            "absolute_error":(indexed-expected).abs(),"absolute_bound":bound
        }));
    }
    fixture::close(total, reference_total, bound * queries.len().max(1) as f64);
    let input = json!({"schedule":schedule_record(schedule),"config":config,"query_dates":queries.iter().map(|(d,_)|*d).collect::<Vec<_>>()});
    json!({
        "id":id,"input":input,"input_hash":content_hash(&input).expect("semantic input hash"),
        "operation":"public AccrualIndex::build/accrued_at and direct accrued_interest_amount","units":"USD",
        "query_count":queries.len(),"values":values,"ordered_outer_sum":total,"ordered_outer_sum_bits":total.to_bits(),
        "reference_outer_sum":reference_total,"reference_outer_sum_bits":reference_total.to_bits()
    })
}

fn error_kind(error: &Error) -> &'static str {
    match error {
        Error::Validation(_) => "Validation",
        Error::Input(_) => "Input",
        Error::CurrencyMismatch { .. } => "CurrencyMismatch",
        Error::Internal(_) => "Internal",
        _ => "Other",
    }
}

fn error_record(
    id: &str,
    schedule: &CashFlowSchedule,
    config: &AccrualConfig,
    as_of: Date,
    required_message: &str,
) -> Value {
    // These recipes deliberately keep preparation valid: failure belongs to the query.
    let index = AccrualIndex::build(schedule, config).expect("query-error fixture builds lazily");
    let direct =
        accrued_interest_amount(schedule, as_of, config).expect_err("direct required failure");
    let indexed = index
        .accrued_at(as_of)
        .expect_err("indexed required failure");
    assert_eq!(error_kind(&direct), error_kind(&indexed));
    assert_eq!(direct.to_string(), indexed.to_string());
    assert!(
        indexed.to_string().contains(required_message),
        "{id}: {indexed}"
    );
    let input = json!({"schedule":schedule_record(schedule),"config":config,"as_of":as_of});
    json!({
        "id":id,"input":input,"input_hash":content_hash(&input).expect("error input hash"),
        "operation":"public index/direct query; index build must succeed","required_substring":required_message,"category":error_kind(&indexed),
        "message":indexed.to_string(),"debug":format!("{indexed:?}")
    })
}

fn base_spec(issue: &str, maturity: &str) -> Value {
    json!({
        "notional":{"initial":{"amount":"1000000","currency":"USD"},"amort":"none"},
        "issue_date":issue,"maturity":maturity,
        "coupon_program":[{"kind":"fixed","spec":{
            "coupon_type":"cash","rate":"0.1","frequency":{"count":3,"unit":"months"},
            "day_count":"act_360","business_day_convention":"unadjusted",
            "calendar_id":"weekends_only","stub":"short_back","payment_lag_days":0
        }}]
    })
}

fn build(value: Value) -> CashFlowSchedule {
    serde_json::from_value::<CashflowScheduleBuildSpec>(value)
        .expect("public build recipe")
        .build(None)
        .expect("public semantic schedule")
}

fn overlap_records() -> Vec<Value> {
    let identity = hand_schedule(
        vec![
            row(
                "2025-07-01",
                "2025-01-01",
                "2025-07-01",
                30_000.0,
                DayCount::Thirty360,
                0.5,
            ),
            row(
                "2025-07-01",
                "2025-02-01",
                "2025-07-01",
                20_000.0,
                DayCount::Act365F,
                150.0 / 365.0,
            ),
        ],
        "2025-01-01",
        1_000_000.0,
        DayCount::Thirty360,
    );
    let descending = hand_schedule(
        vec![
            row(
                "2025-06-01",
                "2025-05-01",
                "2025-06-01",
                31_000.0,
                DayCount::Act365F,
                31.0 / 365.0,
            ),
            row(
                "2025-07-01",
                "2025-01-01",
                "2025-07-01",
                181_000.0,
                DayCount::Act365F,
                181.0 / 365.0,
            ),
        ],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act365F,
    );
    let lagged = hand_schedule(
        vec![
            row(
                "2025-02-05",
                "2025-01-01",
                "2025-02-01",
                31_000.0,
                DayCount::Act365F,
                31.0 / 365.0,
            ),
            row(
                "2025-03-05",
                "2025-02-01",
                "2025-03-01",
                28_000.0,
                DayCount::Act365F,
                28.0 / 365.0,
            ),
        ],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act365F,
    );
    vec![
        success_record(
            "same_payment_distinct_identity",
            &identity,
            &AccrualConfig::default(),
            &[(
                date("2025-04-01"),
                30_000.0 * 90.0 / 180.0 + 20_000.0 * 59.0 / 150.0,
            )],
            1e-9,
        ),
        success_record(
            "descending_accrual_starts",
            &descending,
            &AccrualConfig::default(),
            &[(date("2025-05-15"), 148_000.0)],
            1e-7,
        ),
        success_record(
            "lag_overlap_and_boundaries",
            &lagged,
            &AccrualConfig::default(),
            &[
                (date("2024-12-31"), 0.0),
                (date("2025-01-01"), 0.0),
                (date("2025-02-01"), 31_000.0),
                (date("2025-02-03"), 33_000.0),
                (date("2025-02-05"), 4_000.0),
                (date("2025-03-01"), 28_000.0),
                (date("2025-03-05"), 0.0),
                (date("2025-03-06"), 0.0),
            ],
            1e-7,
        ),
    ]
}

fn pik_records() -> Vec<Value> {
    let cash = row(
        "2025-07-01",
        "2025-01-01",
        "2025-07-01",
        30_000.0,
        DayCount::Act365F,
        181.0 / 365.0,
    );
    let mut pik = row(
        "2025-07-01",
        "2025-01-01",
        "2025-07-01",
        20_000.0,
        DayCount::Act365F,
        181.0 / 365.0,
    );
    pik.kind = CFKind::Pik;
    let schedule = hand_schedule(
        vec![cash, pik],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act365F,
    );
    [false, true]
        .into_iter()
        .map(|include_pik| {
            let config = AccrualConfig {
                include_pik,
                ..Default::default()
            };
            let total = if include_pik { 50_000.0 } else { 30_000.0 };
            success_record(
                if include_pik {
                    "pik_included"
                } else {
                    "pik_excluded"
                },
                &schedule,
                &config,
                &[(date("2025-04-01"), total * 90.0 / 181.0)],
                1e-7,
            )
        })
        .collect()
}

fn ex_coupon_records() -> Vec<Value> {
    let lagged = hand_schedule(
        vec![row(
            "2025-07-03",
            "2025-01-01",
            "2025-07-01",
            100_000.0 * 181.0 / 360.0,
            DayCount::Act360,
            181.0 / 360.0,
        )],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act360,
    );
    let ex = AccrualConfig {
        ex_coupon: Some(ExCouponRule {
            days_before_coupon: 7,
            calendar_id: None,
        }),
        ..Default::default()
    };
    let mut records = vec![
        success_record(
            "ex_record_and_lag",
            &lagged,
            &ex,
            &[
                (date("2025-06-26"), 100_000.0 * 176.0 / 360.0),
                (date("2025-06-27"), -100_000.0 * 4.0 / 360.0),
                (date("2025-07-02"), 0.0),
                (date("2025-07-03"), 0.0),
            ],
            1e-8,
        ),
        success_record(
            "earned_pending_payment",
            &lagged,
            &AccrualConfig::default(),
            &[
                (date("2025-07-02"), 100_000.0 * 181.0 / 360.0),
                (date("2025-07-03"), 0.0),
            ],
            1e-8,
        ),
    ];
    for initial in [100, 0] {
        let mut value = base_spec("2025-01-01", "2025-07-01");
        value["notional"]["initial"]["amount"] = json!(initial.to_string());
        value["coupon_program"][0]["spec"]["rate"] = json!("0.12");
        value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
        value["principal_events"] = json!([{
            "date":"2025-06-28","payment_date":"2025-06-28","kind":"notional",
            "delta":{"amount":"100","currency":"USD"}
        }]);
        let schedule = build(value);
        if initial == 0 {
            records.push(success_record(
                "ex_zero_balance_prefix",
                &schedule,
                &ex,
                &[(date("2025-06-25"), -0.1)],
                1e-8,
            ));
            continue;
        }
        for method in [AccrualMethod::Linear, AccrualMethod::Compounded] {
            let config = AccrualConfig {
                method: method.clone(),
                ..ex.clone()
            };
            let (before, after, id) = if method == AccrualMethod::Linear {
                (
                    -0.3,
                    -0.12 * 200.0 * 2.0 / 360.0,
                    "ex_future_segments_linear",
                )
            } else {
                (
                    -100.0 * ((1.0_f64 + 0.12 * 178.0 / 360.0).powf(3.0 / 178.0) - 1.0) - 0.2,
                    -200.0 * ((1.0_f64 + 0.12 * 3.0 / 360.0).powf(2.0 / 3.0) - 1.0),
                    "ex_future_segments_compounded",
                )
            };
            records.push(success_record(
                id,
                &schedule,
                &config,
                &[(date("2025-06-25"), before), (date("2025-06-29"), after)],
                1e-8,
            ));
        }
    }
    records
}

fn day_count_records() -> Vec<Value> {
    let mut records = Vec::new();
    let mut value = base_spec("2023-03-01", "2024-03-01");
    value["notional"]["initial"]["amount"] = json!("100");
    value["coupon_program"][0]["spec"]["rate"] = json!("0.12");
    value["coupon_program"][0]["spec"]["day_count"] = json!("act_365l");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(12);
    records.push(success_record(
        "act365l_future_leap_denominator",
        &build(value),
        &AccrualConfig::default(),
        &[(date("2023-04-01"), 100.0 * 0.12 * 31.0 / 366.0)],
        1e-8,
    ));
    for (stub, query, expected) in [
        ("long_back", "2025-07-15", 50_000.0),
        ("long_front", "2025-04-15", 100_000.0 * 90.0 / 182.0 / 2.0),
    ] {
        let mut value = base_spec("2025-01-15", "2025-10-15");
        value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
        value["coupon_program"][0]["spec"]["day_count"] = json!("act_act_isma");
        value["coupon_program"][0]["spec"]["stub"] = json!(stub);
        let schedule = build(value);
        let config = AccrualConfig {
            frequency: Some(Tenor::semi_annual()),
            ..Default::default()
        };
        records.push(success_record(
            &format!("icma_{stub}_reference"),
            &schedule,
            &config,
            &[(date(query), expected)],
            1e-8,
        ));
    }
    let mut value = base_spec("2024-08-31", "2025-02-28");
    value["coupon_program"][0]["spec"]["frequency"]["count"] = json!(6);
    value["coupon_program"][0]["spec"]["day_count"] = json!("30e_360_isda");
    value["coupon_program"][0]["spec"]["payment_lag_days"] = json!(2);
    records.push(success_record(
        "february_termination_and_lag",
        &build(value),
        &AccrualConfig::default(),
        &[
            (date("2025-01-31"), 100_000.0 * 150.0 / 360.0),
            (date("2025-02-28"), 100_000.0 * 178.0 / 360.0),
            (date("2025-03-04"), 0.0),
        ],
        1e-8,
    ));
    let icma = hand_schedule(
        vec![row(
            "2025-07-01",
            "2025-01-01",
            "2025-07-01",
            25_000.0,
            DayCount::ActActIsma,
            0.5,
        )],
        "2025-01-01",
        1_000_000.0,
        DayCount::ActActIsma,
    );
    records.push(error_record(
        "icma_missing_frequency",
        &icma,
        &AccrualConfig::default(),
        date("2025-04-01"),
        "frequency",
    ));
    records
}

fn lazy_calendar_records() -> Vec<Value> {
    let active = row(
        "2025-07-01",
        "2025-01-01",
        "2025-07-01",
        18_100.0,
        DayCount::Act365F,
        181.0 / 365.0,
    );
    let mut future = row(
        "2025-12-01",
        "2025-09-01",
        "2025-12-01",
        9_100.0,
        DayCount::Act365F,
        91.0 / 365.0,
    );
    future.accrual.as_mut().expect("future accrual").calendar_id =
        Some("CCM-NO-SUCH-CALENDAR".into());
    let schedule = hand_schedule(
        vec![active, future],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act365F,
    );
    let cfg = AccrualConfig::default();
    vec![
        success_record(
            "inactive_calendar_is_lazy",
            &schedule,
            &cfg,
            &[(date("2025-04-01"), 9_000.0)],
            1e-7,
        ),
        error_record(
            "active_calendar_still_errors",
            &schedule,
            &cfg,
            date("2025-10-01"),
            "CCM-NO-SUCH-CALENDAR",
        ),
    ]
}

fn phase_order_records() -> Vec<Value> {
    let first = row(
        "2025-07-01",
        "2025-01-01",
        "2025-07-01",
        -2_000_000.0,
        DayCount::Act360,
        181.0 / 360.0,
    );
    let second = row(
        "2025-07-01",
        "2025-06-28",
        "2025-07-01",
        25_000.0,
        DayCount::Act360,
        3.0 / 360.0,
    );
    let schedule = hand_schedule(
        vec![first.clone(), second],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act360,
    );
    assert_eq!(schedule.get_flows()[0].amount.amount(), -2_000_000.0);
    let config = AccrualConfig {
        method: AccrualMethod::Compounded,
        ex_coupon: Some(ExCouponRule {
            days_before_coupon: 7,
            calendar_id: None,
        }),
        ..Default::default()
    };
    let winner = error_record(
        "window_error_precedes_earlier_formula_error",
        &schedule,
        &config,
        date("2025-06-29"),
        "coupon start 2025-06-28",
    );
    assert_eq!(winner["category"], "Validation");
    assert!(
        !winner["message"]
            .as_str()
            .expect("captured error")
            .contains("period rate")
    );
    let repaired = hand_schedule(
        vec![
            first,
            row(
                "2025-07-01",
                "2025-06-01",
                "2025-07-01",
                25_000.0,
                DayCount::Act360,
                30.0 / 360.0,
            ),
        ],
        "2025-01-01",
        1_000_000.0,
        DayCount::Act360,
    );
    vec![
        winner,
        error_record(
            "repaired_window_exposes_formula_error",
            &repaired,
            &config,
            date("2025-06-29"),
            "period rate",
        ),
        error_record(
            "inactive_bad_window_does_not_hide_formula",
            &schedule,
            &config,
            date("2025-04-01"),
            "period rate",
        ),
        success_record(
            "inactive_failure_windows_are_zero",
            &schedule,
            &config,
            &[(date("2024-12-31"), 0.0), (date("2025-07-01"), 0.0)],
            0.0,
        ),
    ]
}

fn compounded_records() -> Vec<Value> {
    let cfg = AccrualConfig {
        method: AccrualMethod::Compounded,
        ..Default::default()
    };
    let mut result = Vec::new();
    for (id, amount, expected, bound) in [
        (
            "compound_positive",
            25_000.0,
            1_000_000.0 * (1.025_f64.sqrt() - 1.0),
            1e-6,
        ),
        (
            "compound_negative",
            -200_000.0,
            1_000_000.0 * (0.8_f64.sqrt() - 1.0),
            1e-8,
        ),
        ("compound_near_zero_linear", 1e-7, 5e-8, 1e-15),
    ] {
        let schedule = hand_schedule(
            vec![row(
                "2025-07-01",
                "2025-01-01",
                "2025-07-01",
                amount,
                DayCount::Thirty360,
                0.5,
            )],
            "2025-01-01",
            1_000_000.0,
            DayCount::Thirty360,
        );
        result.push(success_record(
            id,
            &schedule,
            &cfg,
            &[(date("2025-04-01"), expected)],
            bound,
        ));
    }
    for (id, amount) in [
        ("compound_domain_minus_one", -1_000_000.0),
        ("compound_domain_minus_two", -2_000_000.0),
    ] {
        let schedule = hand_schedule(
            vec![row(
                "2025-07-01",
                "2025-01-01",
                "2025-07-01",
                amount,
                DayCount::Thirty360,
                0.5,
            )],
            "2025-01-01",
            1_000_000.0,
            DayCount::Thirty360,
        );
        result.push(error_record(
            id,
            &schedule,
            &cfg,
            date("2025-04-01"),
            "period rate",
        ));
    }
    result
}

fn semantic_records() -> Vec<Value> {
    overlap_records()
        .into_iter()
        .chain(pik_records())
        .chain(ex_coupon_records())
        .chain(day_count_records())
        .chain(lazy_calendar_records())
        .chain(phase_order_records())
        .chain(compounded_records())
        .collect()
}

fn scalar_capture(case: fixture::Case) -> Value {
    let fixture = fixture::fixture(case);
    fixture::validate(case, &fixture);
    let index = AccrualIndex::build(&fixture.schedule, &fixture.config).expect("capture index");
    let mut total = 0.0;
    let mut expected_total = 0.0;
    let mut queries = Vec::with_capacity(case.periods);
    for (order, (&date, &expected)) in fixture.dates.iter().zip(&fixture.expected).enumerate() {
        let actual = if case.rebuild {
            accrued_interest_amount(&fixture.schedule, date, &fixture.config)
                .expect("direct capture")
        } else {
            index.accrued_at(date).expect("indexed capture")
        };
        fixture::close(actual, expected, fixture::SCALAR_BOUND);
        total += actual;
        expected_total += expected;
        let active_period = (0..case.periods).find(|&period| {
            let start = fixture::issue()
                .add_months(period as i32)
                .expect("reference active start");
            let end = fixture::issue()
                .add_months((period + 1) as i32)
                .expect("reference active end");
            start <= date && date < end
        });
        queries.push(json!({"order":order,"date":date,"expected_active_contractual_period":active_period,"actual":actual,"bits":actual.to_bits(),"expected":expected,"expected_bits":expected.to_bits(),"absolute_error":(actual-expected).abs()}));
    }
    fixture::close(total, expected_total, fixture::TOTAL_BOUND);
    let id = match case.shape {
        fixture::QueryShape::Synthetic => format!(
            "scaling_accrued_per_exercise_date/{}/{}",
            if case.rebuild {
                "per_call_rebuild"
            } else {
                "prebuilt_index"
            },
            case.periods
        ),
        _ => format!(
            "ccm_accrued_contractual_band/{}/{}",
            case.shape.label(),
            case.periods
        ),
    };
    let unique_dates = fixture
        .dates
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let input = json!({"schedule":schedule_record(&fixture.schedule),"config":fixture.config,"dates":fixture.dates});
    json!({
        "id":id,"periods":case.periods,"query_count":queries.len(),"unique_query_date_count":unique_dates,"queries":queries,"query_shape":case.shape.label(),"rebuild_per_query":case.rebuild,
        "operation":if case.rebuild {"N direct calls including N index builds"} else {"one index build plus N indexed queries"},"units":"USD",
        "input":input,"input_hash":content_hash(&input).expect("capture input hash"),
        "ordered_outer_sum":total,"ordered_outer_sum_bits":total.to_bits(),
        "expected_ordered_outer_sum":expected_total,"expected_ordered_outer_sum_bits":expected_total.to_bits(),
        "scalar_absolute_bound":fixture::SCALAR_BOUND,"total_absolute_bound":fixture::TOTAL_BOUND,
        "candidate_contract":"Every query value and ordered outer sum must equal C0 bits on the same build; independent bounds do not authorize drift."
    })
}

#[test]
fn ccm_accrual_monthly_matrix_matches_independent_elapsed_days() {
    let cases = fixture::cases();
    assert_eq!(cases.len(), 20);
    for case in cases {
        let fixture = fixture::fixture(case);
        fixture::validate(case, &fixture);
    }
}

#[test]
fn ccm_accrual_overlap_and_payment_boundaries() {
    assert_eq!(overlap_records().len(), 3);
}

#[test]
fn ccm_accrual_pik_selection() {
    assert_eq!(pik_records().len(), 2);
}

#[test]
fn ccm_accrual_ex_coupon_segments_and_record_equality() {
    assert_eq!(ex_coupon_records().len(), 5);
}

#[test]
fn ccm_accrual_day_count_reference_context() {
    assert_eq!(day_count_records().len(), 5);
}

#[test]
fn ccm_accrual_inactive_calendar_is_resolved_only_when_active() {
    assert_eq!(lazy_calendar_records().len(), 2);
}

#[test]
fn ccm_accrual_window_validation_precedes_formula_errors() {
    assert_eq!(phase_order_records().len(), 4);
}

#[test]
fn ccm_accrual_compounded_domain_and_small_rate() {
    assert_eq!(compounded_records().len(), 5);
}

#[test]
#[ignore = "Untimed CCM I3 numerical capture; set CCM_CASHFLOW_ACCRUAL_EXPORT and run explicitly"]
fn ccm_accrual_performance_accuracy_panel() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CCM_CASHFLOW_ACCRUAL_EXPORT")
            .expect("set CCM_CASHFLOW_ACCRUAL_EXPORT to the output JSON path"),
    );
    let cases: Vec<_> = fixture::cases().into_iter().map(scalar_capture).collect();
    let semantics = semantic_records();
    assert_eq!(cases.len(), 20);
    assert_eq!(semantics.len(), 26);
    let output = json!({
        "panel":"CCM-I3-accrual","canonical_version":CANONICAL_VERSION,
        "scope":"Untimed public build/index/query records. No performance or allocation measurement.",
        "candidate_contract":"Exact C0 scalar, ordered sum and error signatures on the same build; independent formula bounds do not authorize drift.",
        "matrix_cases":cases,"semantic_cases":semantics,
        "existing_nonfinite_guards":"Private-input formula/aggregate overflow tests remain in src/accrual.rs; public Money rows cannot encode f64::MAX."
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&output).expect("serialize CCM accrual panel"),
    )
    .expect("write CCM accrual panel");
}
