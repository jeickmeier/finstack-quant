use super::merge::*;
use super::ordering::*;
use super::types::*;
use crate::aggregation::weighted_average_life_from_principal;
use crate::builder::Notional;
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::money::Money;
use time::Month;

/// A fixture row; a cash principal row moves the balance by minus its cash
/// unless the test states another explicit effect.
fn flow_params(date: Date, amount: f64, kind: CFKind) -> CashFlowParams {
    let cash = Money::new(amount, Currency::USD).expect("valid money fixture");
    let row = CashFlowParams::new(date, None, cash, kind, 0.0, None);
    if kind.is_principal_like() && kind.is_cash_settlement() {
        row.with_principal_delta(cash.checked_neg())
            .with_principal_date(date)
    } else {
        row
    }
}

fn flow(date: Date, amount: f64, kind: CFKind) -> CashFlow {
    flow_params(date, amount, kind)
        .build()
        .expect("valid cashflow")
}

#[test]
fn outstanding_on_reads_last_event_on_or_before_date_without_clamping() {
    let day = |d: u8| Date::from_calendar_date(2025, Month::January, d).expect("valid date");
    let usd = |amount: f64| Money::new(amount, Currency::USD).expect("valid money");
    let path = vec![(day(10), usd(60.0)), (day(20), usd(-5.0))];

    assert_eq!(outstanding_on(&path, day(5), 100.0), 100.0);
    assert_eq!(outstanding_on(&path, day(10), 100.0), 60.0);
    assert_eq!(outstanding_on(&path, day(15), 100.0), 60.0);
    assert_eq!(outstanding_on(&path, day(20), 100.0), -5.0);
    assert_eq!(outstanding_on(&[], day(20), 100.0), 100.0);
}

fn assert_every_row_order_is_canonical(rows: Vec<CashFlow>) {
    fn canonical_json(rows: &[CashFlow]) -> String {
        let schedule = CashFlowSchedule::from_flows(
            rows.to_vec(),
            Notional::par(1_000.0, Currency::USD).expect("notional"),
            DayCount::Act360,
            CashFlowMeta::default(),
        );
        serde_json::to_string(schedule.get_flows()).expect("serializable rows")
    }
    fn visit(rows: &mut [CashFlow], index: usize, expected: &str) {
        if index == rows.len() {
            assert_eq!(canonical_json(rows), expected);
            return;
        }
        for next in index..rows.len() {
            rows.swap(index, next);
            visit(rows, index + 1, expected);
            rows.swap(index, next);
        }
    }
    let expected = canonical_json(&rows);
    visit(&mut rows.clone(), 0, &expected);
    let schedules: Vec<_> = rows
        .into_iter()
        .map(|row| {
            CashFlowSchedule::from_flows(
                vec![row],
                Notional::par(1_000.0, Currency::USD).expect("notional"),
                DayCount::Act360,
                CashFlowMeta::default(),
            )
        })
        .collect();
    for order in [schedules.clone(), schedules.into_iter().rev().collect()] {
        let merged = merge_cashflow_schedules(
            order,
            Notional::par(1_000.0, Currency::USD).expect("notional"),
            DayCount::Act360,
        );
        assert_eq!(
            serde_json::to_string(merged.get_flows()).expect("serializable merge"),
            expected
        );
    }
}

#[test]
fn canonical_order_distinguishes_kinds_with_shared_economic_ranks() {
    let date = Date::from_calendar_date(2025, Month::June, 15).expect("date");
    for (first, second) in [
        (CFKind::Fixed, CFKind::Floating),
        (CFKind::Fee, CFKind::CommitmentFee),
        (CFKind::Recovery, CFKind::AccruedOnDefault),
        (CFKind::Notional, CFKind::RevolvingDraw),
        (CFKind::InitialMarginPost, CFKind::MarginInterest),
    ] {
        assert_eq!(kind_rank(first), kind_rank(second));
        assert_every_row_order_is_canonical(vec![flow(date, 1.0, first), flow(date, 1.0, second)]);
    }
}

#[test]
fn canonical_order_preserves_exact_decimal_values_and_representation() {
    let date = Date::from_calendar_date(2025, Month::June, 15).expect("date");
    let money = |value| Money::from_decimal_str(value, Currency::USD).expect("exact money");
    let low = money("1.000000000000000001");
    let high = money("1.000000000000000002");
    assert_eq!(low.amount(), high.amount(), "fixture must collide as f64");
    let with_amount = |amount| {
        CashFlowParams {
            amount,
            ..flow_params(date, 1.0, CFKind::Fixed)
        }
        .build()
        .expect("valid cashflow")
    };
    assert_eq!(
        compare_flows(&with_amount(low), &with_amount(high)),
        std::cmp::Ordering::Less
    );
    assert_every_row_order_is_canonical(vec![
        with_amount(high),
        with_amount(low),
        with_amount(money("1.0")),
        with_amount(money("1.00")),
    ]);
    let with_delta = |value| {
        flow_params(date, 1.0, CFKind::Notional)
            .with_principal_delta(value)
            .build()
            .expect("valid cashflow")
    };
    assert_eq!(
        compare_flows(&with_delta(low), &with_delta(high)),
        std::cmp::Ordering::Less
    );
    assert_every_row_order_is_canonical(vec![
        with_delta(high),
        with_delta(low),
        with_delta(money("1.0")),
        with_delta(money("1.00")),
    ]);
}

#[test]
fn canonical_order_distinguishes_all_day_count_metadata() {
    let start = Date::from_calendar_date(2025, Month::March, 15).expect("start");
    let end = Date::from_calendar_date(2025, Month::June, 15).expect("end");
    let conventions = [
        DayCount::OneOne,
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::Act365L,
        DayCount::Thirty360,
        DayCount::ThirtyE360,
        DayCount::ThirtyE360Isda,
        DayCount::Thirty360It,
        DayCount::Nl365,
        DayCount::ActAct,
        DayCount::ActActIsma,
        DayCount::ActActAfb,
        DayCount::Bus252,
    ];
    let with_day_count = |day_count| {
        flow_params(end, 1.0, CFKind::Fixed)
            .with_accrual(
                CashFlowAccrual::new(
                    start,
                    end,
                    day_count,
                    None,
                    finstack_quant_core::dates::DayCountContextState::try_new(
                        Some("weekends_only".into()),
                        Some(finstack_quant_core::dates::Tenor::quarterly()),
                        None,
                        Some((start, end)),
                        Some(false),
                    )
                    .expect("valid context"),
                )
                .expect("valid accrual"),
            )
            .build()
            .expect("valid cashflow")
    };
    for (index, first) in conventions.iter().enumerate() {
        for second in &conventions[index + 1..] {
            assert_every_row_order_is_canonical(vec![
                with_day_count(*first),
                with_day_count(*second),
            ]);
        }
    }
}

#[test]
fn invalid_rows_never_become_schedule_flows() {
    // Record CORE-CONVENTIONS-015 #10: rows validate at construction.
    let date = Date::from_calendar_date(2025, Month::June, 15).expect("date");
    let negative_accrual = CashFlowParams {
        accrual_factor: -1.0,
        ..flow_params(date, 10.0, CFKind::Fixed)
    };
    let late_reset = CashFlowParams {
        reset_date: Some(date + time::Duration::days(1)),
        ..flow_params(date, 10.0, CFKind::Fixed)
    };
    let mismatched_currency = flow_params(date, -98.0, CFKind::Notional)
        .with_principal_delta(Money::from((100_i64, Currency::EUR)));
    for bad in [negative_accrual, late_reset, mismatched_currency] {
        assert!(bad.build().is_err());
    }
    let notional = Notional::par(1_000.0, Currency::USD).expect("notional");
    let ok = CashFlowSchedule::from_flows(
        vec![flow(date, 1.0, CFKind::Fixed)],
        notional,
        DayCount::Act360,
        CashFlowMeta::default(),
    );
    assert_eq!(ok.get_flows().len(), 1);
}

#[test]
fn public_normalization_preserves_funding_and_pik_balances() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("issue");
    let pik = issue.add_months(3).expect("valid date shift");
    let maturity = issue.add_months(6).expect("valid date shift");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow_params(issue, -98.0, CFKind::Notional)
                .with_principal_delta(Money::from((100_i64, Currency::USD)))
                .build()
                .expect("valid cashflow"),
            flow_params(pik, 0.0, CFKind::Pik)
                .with_principal_delta(Money::from((5_i64, Currency::USD)))
                .with_principal_date(pik)
                .build()
                .expect("valid cashflow"),
            flow(maturity, 105.0, CFKind::Notional),
        ],
        Notional::par(0.0, Currency::USD).expect("notional"),
        DayCount::Act360,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    )
    .normalize_public(issue.add_months(1).expect("valid date shift"))
    .expect("normalize");
    schedule.validate().expect("preserved principal");
    assert_eq!(schedule.notional.initial.amount(), 100.0);
    assert_eq!(schedule.flows[0].get_kind(), CFKind::Pik);
    assert_eq!(schedule.flows[0].get_amount().amount(), 0.0);
    assert_eq!(
        schedule.flows[0]
            .get_principal_delta()
            .expect("PIK delta")
            .amount(),
        5.0
    );
    let balances = schedule.outstanding_by_date().expect("balances");
    assert_eq!(balances[0].1.amount(), 105.0);
    assert_eq!(balances[1].1.amount(), 0.0);
}

#[test]
fn normalized_pik_preserves_accrual_but_is_not_settlement_cash() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let as_of = issue.add_months(3).expect("date shift");
    let maturity = issue.add_months(6).expect("date shift");
    let pik = CashFlowParams::new(
        maturity,
        None,
        Money::zero(Currency::USD),
        CFKind::Pik,
        0.5,
        Some(0.12),
    )
    .with_principal_delta(Money::from((6_i64, Currency::USD)))
    .with_principal_date(maturity)
    .with_accrual(
        CashFlowAccrual::new(
            issue,
            maturity,
            DayCount::Thirty360,
            None,
            finstack_quant_core::dates::DayCountContextState::try_new(
                None,
                None,
                None,
                None,
                Some(true),
            )
            .expect("valid context"),
        )
        .expect("valid accrual"),
    )
    .build()
    .expect("valid cashflow");
    let raw = CashFlowSchedule::from_flows(
        vec![pik, flow(maturity, 106.0, CFKind::Notional)],
        Notional::par(100.0, Currency::USD).expect("notional"),
        DayCount::Thirty360,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    let normalized = raw.clone().normalize_public(as_of).expect("normalize");
    let wire = serde_json::to_string(&normalized).expect("serialize schedule");
    let normalized: CashFlowSchedule = serde_json::from_str(&wire).expect("roundtrip");
    for schedule in [&raw, &normalized] {
        schedule.validate().expect("valid PIK state");
        assert_eq!(
            crate::accrual::accrued_interest_amount(
                schedule,
                as_of,
                &crate::accrual::AccrualConfig {
                    include_pik: true,
                    ..Default::default()
                }
            )
            .expect("PIK accrued")
            .amount(),
            3.0
        );
        assert_eq!(
            crate::accrual::accrued_interest_amount(
                schedule,
                as_of,
                &crate::accrual::AccrualConfig::default()
            )
            .expect("default cash-only accrued")
            .amount(),
            0.0
        );
        assert_eq!(
            schedule.dated_flows().expect("settlements"),
            vec![(maturity, Money::from((106_i64, Currency::USD)))]
        );
    }
}

#[test]
fn normalized_amortizing_schedule_keeps_only_resolved_remaining_principal() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let first = issue.add_months(3).expect("date shift");
    let second = issue.add_months(4).expect("date shift");
    let as_of = issue.add_months(5).expect("date shift");
    let maturity = issue.add_months(6).expect("date shift");
    for (amort, remaining) in [
        (
            crate::builder::AmortizationSpec::CustomPrincipal {
                items: vec![
                    (first, Money::from((40_i64, Currency::USD))),
                    (maturity, Money::from((60_i64, Currency::USD))),
                ],
            },
            60_i64,
        ),
        (
            crate::builder::AmortizationSpec::StepRemaining {
                schedule: vec![
                    (first, Money::from((60_i64, Currency::USD))),
                    (second, Money::from((30_i64, Currency::USD))),
                    (maturity, Money::from((0_i64, Currency::USD))),
                ],
            },
            30_i64,
        ),
    ] {
        let raw = CashFlowSchedule::builder()
            .principal(Money::from((100_i64, Currency::USD)), issue, maturity)
            .amortization(amort)
            .build(None)
            .expect("build amortizing schedule");
        raw.validate().expect("valid original recipe");
        let normalized = raw.normalize_public(as_of).expect("normalize");
        normalized.validate().expect("valid remaining state");
        assert_eq!(
            normalized.notional.initial,
            Money::from((remaining, Currency::USD))
        );
        assert!(matches!(
            normalized.notional.amort,
            crate::builder::AmortizationSpec::None
        ));
        assert_eq!(
            normalized.dated_flows().expect("remaining principal"),
            vec![(maturity, Money::from((remaining, Currency::USD)))]
        );
    }
}

#[test]
fn scale_amounts_restores_canonical_order_under_sign_flip() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow(date, 100.0, CFKind::Amortization),
            flow(date, 200.0, CFKind::Amortization),
        ],
        Notional::par(1000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    assert_eq!(schedule.flows[0].get_amount().amount(), 100.0);
    assert_eq!(schedule.flows[1].get_amount().amount(), 200.0);

    let scaled = schedule.scale_amounts(-1.0).expect("finite scale");

    assert_eq!(scaled.flows[0].get_amount().amount(), -200.0);
    assert_eq!(scaled.flows[1].get_amount().amount(), -100.0);
    assert!(
        scaled
            .flows
            .windows(2)
            .all(|w| compare_flows(&w[0], &w[1]) != std::cmp::Ordering::Greater),
        "scale_amounts must leave flows in canonical compare_flows order"
    );
}

#[test]
fn from_flows_unchecked_sorts_by_date_then_kind_rank() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow(date, 10.0, CFKind::Recovery),
            flow(date, 12.0, CFKind::Amortization),
            flow(date, 8.0, CFKind::PrePayment),
            flow(date, 5.0, CFKind::Fixed),
        ],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

    assert_eq!(schedule.flows[0].get_kind(), CFKind::Fixed);
    assert_eq!(schedule.flows[1].get_kind(), CFKind::Amortization);
    assert_eq!(schedule.flows[2].get_kind(), CFKind::PrePayment);
    assert_eq!(schedule.flows[3].get_kind(), CFKind::Recovery);
}

#[test]
fn validate_rejects_over_amortization() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");

    let schedule = CashFlowSchedule::from_flows(
        vec![flow(date, 150.0, CFKind::Amortization)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

    assert!(
        schedule.validate().is_err(),
        "validated construction must reject over-amortization"
    );
}

#[test]
fn serde_is_structural_and_requires_explicit_economic_validation() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let unvalidated = CashFlowSchedule::from_flows(
        vec![flow(date, 150.0, CFKind::Amortization)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    let wire = serde_json::to_value(unvalidated).expect("serialize schedule");

    let decoded: CashFlowSchedule =
        serde_json::from_value(wire).expect("serde validates structure only");

    assert!(
        decoded.validate().is_err(),
        "economic validation remains explicit for direct serde consumers",
    );
}

#[test]
fn merge_cashflow_schedules_merges_meta_and_resorts() {
    let d1 = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let d2 = Date::from_calendar_date(2025, Month::February, 15).expect("valid date");
    let left = CashFlowSchedule::from_flows(
        vec![
            flow_params(d2, 4.0, CFKind::Recovery)
                .with_accrual(
                    CashFlowAccrual::new(
                        d1,
                        d2,
                        DayCount::Thirty360,
                        None,
                        finstack_quant_core::dates::DayCountContextState::try_new(
                            None,
                            None,
                            None,
                            None,
                            Some(false),
                        )
                        .expect("valid context"),
                    )
                    .expect("valid accrual"),
                )
                .build()
                .expect("valid cashflow"),
        ],
        Notional::par(50.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            projected_fixings: Vec::new(),
            representation: CashflowRepresentation::Projected,
            calendar_ids: vec!["nyc".to_string()],
            commitment: None,
            issue_date: Some(d1),
            maturity: None,
        },
    );
    let right = CashFlowSchedule::from_flows(
        vec![
            flow_params(d1, 10.0, CFKind::Amortization)
                .with_accrual(
                    CashFlowAccrual::new(
                        d1,
                        d2,
                        DayCount::Act360,
                        None,
                        finstack_quant_core::dates::DayCountContextState::try_new(
                            None,
                            None,
                            None,
                            None,
                            Some(false),
                        )
                        .expect("valid context"),
                    )
                    .expect("valid accrual"),
                )
                .build()
                .expect("valid cashflow"),
        ],
        Notional::par(50.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            projected_fixings: Vec::new(),
            representation: CashflowRepresentation::Projected,
            calendar_ids: vec!["lon".to_string(), "nyc".to_string()],
            commitment: None,
            issue_date: Some(d1),
            maturity: None,
        },
    );

    let merged = merge_cashflow_schedules(
        vec![left, right],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
    );

    assert_eq!(merged.flows.len(), 2);
    assert_eq!(merged.flows[0].get_date(), d1);
    assert_eq!(
        merged.meta.representation,
        CashflowRepresentation::Projected
    );
    assert_eq!(
        merged.meta.calendar_ids,
        vec!["lon".to_string(), "nyc".to_string()]
    );
    assert_eq!(merged.meta.issue_date, Some(d1));
    assert_eq!(
        merged.flows[0]
            .get_accrual()
            .as_ref()
            .map(|accrual| accrual.get_day_count()),
        Some(DayCount::Act360)
    );
    assert_eq!(
        merged.flows[1]
            .get_accrual()
            .as_ref()
            .map(|accrual| accrual.get_day_count()),
        Some(DayCount::Thirty360)
    );
}

#[test]
fn merge_cashflow_schedules_preserves_mixed_currency_rows() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let usd = CashFlowSchedule::from_flows(
        vec![flow(date, 100.0, CFKind::Notional)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    let eur = CashFlowSchedule::from_flows(
        vec![{
            let cash = Money::from((-90_i64, Currency::EUR));
            CashFlowParams::new(date, None, cash, CFKind::Notional, 0.0, None)
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(date)
                .build()
                .expect("valid cashflow")
        }],
        Notional::par(90.0, Currency::EUR).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

    let merged = merge_cashflow_schedules(
        [usd, eur],
        Notional::par(0.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
    );

    assert_eq!(merged.flows.len(), 2);
    assert!(
        merged
            .flows
            .iter()
            .any(|flow| flow.get_amount().currency() == Currency::USD)
    );
    assert!(
        merged
            .flows
            .iter()
            .any(|flow| flow.get_amount().currency() == Currency::EUR)
    );
}

#[test]
fn merge_cashflow_schedules_marks_mixed_projection_as_projected() {
    let date = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let contractual = CashFlowSchedule::from_flows(
        vec![flow(date, 100.0, CFKind::Fixed)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    let projected = CashFlowSchedule::from_flows(
        vec![flow(date, 1.0, CFKind::Floating)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            representation: CashflowRepresentation::Projected,
            ..Default::default()
        },
    );

    let merged = merge_cashflow_schedules(
        [contractual, projected],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
    );

    assert_eq!(
        merged.meta.representation,
        CashflowRepresentation::Projected
    );
}

#[test]
fn validation_counts_delayed_funding_before_amortization() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let repayment = Date::from_calendar_date(2025, Month::July, 15).expect("valid date");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow(issue, -100.0, CFKind::Notional),
            flow(repayment, 30.0, CFKind::Amortization),
        ],
        Notional::par(0.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );

    schedule
        .validate()
        .expect("future funding should support later amortization");
}

#[test]
fn retired_accrual_sidecars_are_rejected() {
    let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
    let schedule = CashFlowSchedule::from_flows(
        vec![flow(end, 12.5, CFKind::Fixed)],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    let mut retired = serde_json::to_value(&schedule).expect("serialize schedule");
    let meta = retired["meta"].as_object_mut().expect("meta object");
    meta.insert(
        "accrual_periods".to_string(),
        serde_json::to_value(vec![Some((end, end))]).expect("periods"),
    );

    let error = serde_json::from_value::<CashFlowSchedule>(retired)
        .expect_err("retired schedule sidecars must fail");
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn deserialization_routes_unsorted_flows_through_canonical_constructor() {
    let early = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let late = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow(early, 1.0, CFKind::Fixed),
            flow(late, 2.0, CFKind::Fixed),
        ],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    let mut wire = serde_json::to_value(schedule).expect("serialize schedule");
    wire["flows"].as_array_mut().expect("flows array").reverse();

    let decoded: CashFlowSchedule = serde_json::from_value(wire).expect("canonical schedule");
    assert_eq!(decoded.dates(), vec![early, late]);
}

#[test]
fn filtering_and_sorting_keep_owned_accrual_metadata() {
    let d1 = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let d2 = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
    let accrual = CashFlowAccrual::new(
        d1,
        d2,
        DayCount::Act360,
        Some(0.031),
        finstack_quant_core::dates::DayCountContextState::try_new(
            None,
            None,
            None,
            None,
            Some(false),
        )
        .expect("valid context"),
    )
    .expect("valid accrual");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow_params(d2, 12.5, CFKind::Fixed)
                .with_accrual(accrual.clone())
                .build()
                .expect("valid cashflow"),
            flow(d1, 100.0, CFKind::Notional),
        ],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    )
    .normalize_public(d2)
    .expect("valid balance replay");

    assert_eq!(schedule.flows.len(), 1);
    assert_eq!(schedule.flows[0].get_accrual(), &Some(accrual));
}

#[test]
fn wal_uses_act365f_regardless_of_schedule_day_count() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let d1 = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    let d2 = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");

    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow(d1, 500_000.0, CFKind::Amortization),
            flow(d2, 500_000.0, CFKind::Amortization),
        ],
        Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Thirty360, // schedule uses 30/360 but WAL should use Act/365F
        CashFlowMeta::default(),
    );

    let wal = schedule.wal(as_of).expect("WAL succeeds");

    let t1 = DayCount::Act365F
        .year_fraction(as_of, d1, DayCountContext::default())
        .unwrap();
    let t2 = DayCount::Act365F
        .year_fraction(as_of, d2, DayCountContext::default())
        .unwrap();
    let expected = (500_000.0 * t1 + 500_000.0 * t2) / 1_000_000.0;

    assert!(
        (wal - expected).abs() < 1e-10,
        "WAL should match Act/365F calculation: expected {}, got {}",
        expected,
        wal
    );

    let t30_360_1 = DayCount::Thirty360
        .year_fraction(as_of, d1, DayCountContext::default())
        .unwrap();
    let t30_360_2 = DayCount::Thirty360
        .year_fraction(as_of, d2, DayCountContext::default())
        .unwrap();
    let wal_30360 = (500_000.0 * t30_360_1 + 500_000.0 * t30_360_2) / 1_000_000.0;

    assert!(
        (wal - expected).abs() < (wal - wal_30360).abs() || (wal - expected).abs() < 1e-10,
        "WAL should be closer to Act/365F value than 30/360 value"
    );
}

#[test]
fn wal_kernel_combines_same_date_principal() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let payment = Date::from_calendar_date(2026, Month::July, 1).expect("valid date");
    let expected = DayCount::Act365F
        .year_fraction(as_of, payment, DayCountContext::default())
        .expect("year fraction");

    let wal = weighted_average_life_from_principal(
        [
            (payment, Money::from((40_i64, Currency::USD))),
            (payment, Money::from((60_i64, Currency::USD))),
        ],
        as_of,
    )
    .expect("WAL succeeds");

    assert!((wal - expected).abs() < 1e-12);
}

#[test]
fn wal_kernel_rejects_mixed_currencies() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let first = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
    let second = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");

    let error = weighted_average_life_from_principal(
        [
            (first, Money::from((40_i64, Currency::USD))),
            (second, Money::from((60_i64, Currency::EUR))),
        ],
        as_of,
    )
    .expect_err("mixed currencies must fail");

    assert!(matches!(
        error,
        finstack_quant_core::Error::CurrencyMismatch {
            expected: Currency::USD,
            actual: Currency::EUR,
        }
    ));
}

#[test]
fn wal_kernel_returns_zero_without_future_principal() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let past = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");

    let wal = weighted_average_life_from_principal(
        [
            (past, Money::from((100_i64, Currency::USD))),
            (as_of, Money::from((100_i64, Currency::USD))),
        ],
        as_of,
    )
    .expect("WAL succeeds");

    assert_eq!(wal, 0.0);
}

#[test]
fn finite_scaling_overflow_returns_an_error_for_cash_and_principal() {
    let date = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    for (amount, delta, scale) in [
        (1.0, None, 1e30),
        (1.0, None, 1e100),
        (1e28, None, 10.0),
        (0.0, Some(1.0), 1e30),
        (0.0, Some(1e28), 10.0),
    ] {
        let row = match delta {
            None => flow(date, amount, CFKind::Fixed),
            Some(value) => flow_params(date, amount, CFKind::Notional)
                .with_principal_delta(Money::new(value, Currency::USD).expect("money"))
                .build()
                .expect("valid cashflow"),
        };
        let schedule = CashFlowSchedule::from_flows(
            vec![row],
            Notional::par(0.0, Currency::USD).expect("notional"),
            DayCount::Act365F,
            CashFlowMeta::default(),
        );
        assert!(schedule.scale_amounts(scale).is_err());
    }
}

fn shifted_principal_schedule(payment: Date, economic: Date) -> CashFlowSchedule {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("date");
    CashFlowSchedule::from_flows(
        vec![
            flow_params(issue, -100.0, CFKind::Notional)
                .with_principal_delta(Money::zero(Currency::USD))
                .build()
                .expect("valid cashflow"),
            flow_params(payment, 50.0, CFKind::Amortization)
                .with_principal_date(economic)
                .build()
                .expect("valid cashflow"),
            flow(maturity, 50.0, CFKind::Notional),
        ],
        Notional::par(100.0, Currency::USD).expect("notional"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    )
}

#[test]
fn normalization_includes_economic_repayment_before_pending_cash() {
    let economic = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let as_of = Date::from_calendar_date(2025, Month::July, 2).expect("date");
    let payment = Date::from_calendar_date(2025, Month::July, 3).expect("date");
    let schedule = shifted_principal_schedule(payment, economic)
        .normalize_public(as_of)
        .expect("normalize");
    schedule.validate().expect("valid normalized state");
    assert_eq!(schedule.get_notional().initial.amount(), 50.0);
    assert_eq!(schedule.get_flows()[0].get_amount().amount(), 50.0);
    assert_eq!(
        schedule.get_flows()[0]
            .get_principal_delta()
            .map(|delta| delta.amount()),
        Some(0.0)
    );
    assert_eq!(
        schedule
            .outstanding_by_date()
            .expect("balances")
            .last()
            .map(|(_, balance)| balance.amount()),
        Some(0.0)
    );

    let wire = serde_json::to_value(&schedule).expect("serialize");
    let again = schedule
        .clone()
        .normalize_public(as_of)
        .expect("normalize again");
    assert_eq!(serde_json::to_value(again).expect("serialize"), wire);
    let after_cash = schedule
        .normalize_public(payment.add_days(1).expect("date shift"))
        .expect("normalize after cash");
    assert_eq!(after_cash.get_notional().initial.amount(), 50.0);
    after_cash.validate().expect("valid after settlement");
}

#[test]
fn normalization_preserves_future_principal_after_cash_has_settled() {
    let payment = Date::from_calendar_date(2025, Month::June, 30).expect("date");
    let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let economic = Date::from_calendar_date(2025, Month::July, 2).expect("date");
    let schedule = shifted_principal_schedule(payment, economic)
        .normalize_public(as_of)
        .expect("normalize");
    schedule.validate().expect("valid normalized state");
    assert_eq!(schedule.get_notional().initial.amount(), 100.0);
    let movement = &schedule.get_flows()[0];
    assert_eq!(movement.get_date(), economic);
    assert_eq!(movement.get_amount().amount(), 0.0);
    assert_eq!(
        movement.get_principal_delta().map(|delta| delta.amount()),
        Some(-50.0)
    );
    let balances = schedule
        .outstanding_at_dates(&[as_of, economic])
        .expect("balances");
    assert_eq!(
        balances
            .iter()
            .map(|balance| balance.amount())
            .collect::<Vec<_>>(),
        vec![100.0, 50.0]
    );
    let after_economic = schedule
        .normalize_public(economic.add_days(1).expect("date shift"))
        .expect("normalize after principal");
    assert_eq!(after_economic.get_notional().initial.amount(), 50.0);
    after_economic
        .validate()
        .expect("valid after principal movement");
}

#[test]
fn normalization_keeps_movements_on_the_snapshot_date_out_of_opening_balance() {
    let economic = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let payment = Date::from_calendar_date(2025, Month::July, 3).expect("date");
    let schedule = shifted_principal_schedule(payment, economic)
        .normalize_public(economic)
        .expect("normalize");
    assert_eq!(schedule.get_notional().initial.amount(), 100.0);
    assert_eq!(
        schedule
            .outstanding_at_dates(&[economic])
            .expect("balances")[0]
            .amount(),
        50.0
    );
    schedule.validate().expect("valid snapshot");
}

#[test]
fn bulk_balance_queries_use_economic_dates_and_preserve_query_order() {
    let economic = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let payment = Date::from_calendar_date(2025, Month::July, 3).expect("date");
    let before_issue = Date::from_calendar_date(2024, Month::December, 31).expect("date");
    let before_principal = Date::from_calendar_date(2025, Month::June, 30).expect("date");
    let schedule = shifted_principal_schedule(payment, economic);
    let balances = schedule
        .outstanding_at_dates(&[before_issue, before_principal, economic, economic, payment])
        .expect("balances");
    assert_eq!(
        balances
            .iter()
            .map(|balance| balance.amount())
            .collect::<Vec<_>>(),
        vec![100.0, 100.0, 50.0, 50.0, 50.0]
    );
    assert!(schedule.outstanding_at_dates(&[payment, economic]).is_err());
    assert!(
        schedule
            .outstanding_at_dates(&[])
            .expect("empty query")
            .is_empty()
    );
}

#[test]
fn wal_counts_revolving_repayments() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("date");
    let schedule = CashFlowSchedule::from_flows(
        vec![flow(maturity, 100.0, CFKind::RevolvingRepayment)],
        Notional::par(100.0, Currency::USD).expect("notional"),
        DayCount::Thirty360,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    schedule.validate().expect("valid revolving repayment");
    assert_eq!(schedule.wal(issue).expect("wal"), 1.0);
}

#[test]
fn calendar_year_ladder_rejects_native_amounts_in_different_currencies() {
    let date = Date::from_calendar_date(2026, Month::January, 1).expect("date");
    let usd = flow(date, 100.0, CFKind::Fixed);
    let eur = CashFlowParams {
        amount: Money::from((100_i64, Currency::EUR)),
        ..flow_params(date, 100.0, CFKind::Fixed)
    }
    .build()
    .expect("valid cashflow");
    let schedule = CashFlowSchedule::from_flows(
        vec![usd, eur],
        Notional::par(100.0, Currency::USD).expect("notional"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );
    schedule
        .validate()
        .expect("mixed-currency coupon rows allowed");
    assert!(matches!(
        schedule.calendar_year_ladder(&[90.0, 80.0]),
        Err(finstack_quant_core::Error::CurrencyMismatch { .. })
    ));
}

#[test]
fn wal_uses_par_principal_delta_and_balance_date() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let economic = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let payment = Date::from_calendar_date(2025, Month::July, 3).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("date");
    let redemption = flow_params(payment, 52.5, CFKind::Amortization)
        .with_principal_delta(Money::new(-50.0, Currency::USD).expect("money"))
        .with_principal_date(economic)
        .build()
        .expect("valid cashflow");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            flow_params(issue, -100.0, CFKind::Notional)
                .with_principal_delta(Money::zero(Currency::USD))
                .build()
                .expect("valid cashflow"),
            redemption,
            flow(maturity, 50.0, CFKind::Notional),
        ],
        Notional::par(100.0, Currency::USD).expect("notional"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    let t_econ = DayCount::Act365F
        .year_fraction(issue, economic, DayCountContext::default())
        .expect("yf");
    let t_mat = DayCount::Act365F
        .year_fraction(issue, maturity, DayCountContext::default())
        .expect("yf");
    let expected = 0.5 * t_econ + 0.5 * t_mat;
    assert!((schedule.wal(issue).expect("wal") - expected).abs() < 1e-12);
}

#[test]
fn normalization_allows_multi_currency_composite_pay_leg_rebase() {
    let start = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let as_of = Date::from_calendar_date(2025, Month::January, 3).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 2).expect("date");

    let notional_row = |date: Date, amount: f64, currency: Currency| {
        let cash = Money::new(amount, currency).expect("money");
        CashFlowParams::new(date, None, cash, CFKind::Notional, 0.0, None)
            .with_principal_delta(cash.checked_neg())
            .with_principal_date(date)
            .build()
            .expect("valid cashflow")
    };

    let eur_leg = CashFlowSchedule::from_flows(
        vec![
            notional_row(start, -90.0, Currency::EUR),
            notional_row(maturity, 90.0, Currency::EUR),
        ],
        Notional::par(0.0, Currency::EUR).expect("notional"),
        DayCount::Act360,
        CashFlowMeta::default(),
    );
    let usd_leg = CashFlowSchedule::from_flows(
        vec![
            notional_row(start, 100.0, Currency::USD),
            notional_row(maturity, -100.0, Currency::USD),
        ],
        Notional::par(0.0, Currency::USD).expect("notional"),
        DayCount::Act360,
        CashFlowMeta::default(),
    );

    let normalized = merge_cashflow_schedules(
        [eur_leg, usd_leg],
        Notional::par(0.0, Currency::USD).expect("notional"),
        DayCount::Act360,
    )
    .normalize_public(as_of)
    .expect("multi-currency composite pay leg should rebase");

    assert_eq!(normalized.get_notional().initial.currency(), Currency::USD);
    assert_eq!(normalized.get_notional().initial.amount(), 0.0);
    assert_eq!(normalized.get_flows().len(), 2);
    normalized
        .dated_flows()
        .expect("future maturity exchanges remain valid");

    // A single-currency principal schedule with a foreign coupon must still
    // enforce the non-negative rebased balance check.
    let single_principal_with_fx_coupon = CashFlowSchedule::from_flows(
        vec![
            notional_row(start, 100.0, Currency::USD),
            CashFlowParams::new(
                maturity,
                None,
                Money::from((5_i64, Currency::EUR)),
                CFKind::Fixed,
                1.0,
                Some(0.05),
            )
            .build()
            .expect("valid coupon"),
        ],
        Notional::par(0.0, Currency::USD).expect("notional"),
        DayCount::Act360,
        CashFlowMeta::default(),
    );
    let err = single_principal_with_fx_coupon
        .normalize_public(as_of)
        .expect_err("single-currency negative rebased balance must fail");
    assert!(
        err.to_string()
            .contains("principal repayments exceed rebased outstanding"),
        "unexpected error: {err}"
    );
}
