use super::engine::*;
use crate::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::cashflow::{CashFlow, CashFlowAccrual, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn zero_accrual_factor_is_a_literal_zero_length_accrual() {
    // Record CORE-CONVENTIONS-015 #9: zero is never "missing, derive it".
    let start = make_date(2025, 1, 1);
    let end = make_date(2025, 7, 1);
    let coupon = CashFlowParams::new(
        end,
        None,
        Money::from((50_i64, Currency::USD)),
        CFKind::Fixed,
        0.0,
        Some(0.1),
    )
    .with_accrual(
        CashFlowAccrual::new(
            start,
            end,
            DayCount::Act360,
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
    let schedule = CashFlowSchedule::from_flows(
        vec![coupon],
        Notional::par(1_000.0, Currency::USD).expect("notional"),
        DayCount::Act360,
        CashFlowMeta {
            issue_date: Some(start),
            ..Default::default()
        },
    );
    let accrued =
        accrued_interest_amount(&schedule, make_date(2025, 4, 1), &AccrualConfig::default())
            .expect("accrued");
    assert_eq!(accrued.amount(), 0.0);
}

/// Create a minimal schedule with coupon flows for testing issue date derivation.
fn make_test_schedule(
    coupon_dates: &[(Date, f64)], // (date, accrual_factor)
    day_count: DayCount,
) -> CashFlowSchedule {
    let flows: Vec<CashFlow> = coupon_dates
        .iter()
        .map(|(date, af)| {
            CashFlowParams::new(
                *date,
                None,
                Money::from((25000_i64, Currency::USD)),
                CFKind::Fixed,
                *af,
                Some(0.05),
            )
            .build()
            .expect("valid cashflow")
        })
        .collect();

    CashFlowSchedule {
        flows,
        notional: Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        day_count,
        meta: Default::default(),
    }
}

#[test]
fn accrual_window_bounds_preserve_full_scan_order_and_values() {
    let jan1 = make_date(2025, 1, 1);
    let may1 = make_date(2025, 5, 1);
    let jun1 = make_date(2025, 6, 1);
    let jun28 = make_date(2025, 6, 28);
    let jul1 = make_date(2025, 7, 1);
    let aug30 = make_date(2025, 8, 30);
    let sep1 = make_date(2025, 9, 1);
    let oct1 = make_date(2025, 10, 1);
    let dec1 = make_date(2025, 12, 1);
    let flows = [
        (may1, jun1, jun1, (may1, jun1), 25_000_i64),
        (jan1, jul1, jul1, (jan1, jul1), 25_000),
        (jun28, jul1, jul1, (jan1, jul1), 25_000),
        // A zero prefix must still set the later segment's whole-coupon start.
        (jan1, aug30, sep1, (jan1, sep1), 0),
        (aug30, sep1, sep1, (jan1, sep1), 25_000),
        (oct1, dec1, dec1, (oct1, dec1), 25_000),
    ]
    .into_iter()
    .map(|(start, end, payment, reference, amount)| {
        CashFlowParams::new(
            payment,
            None,
            Money::from((amount, Currency::USD)),
            CFKind::Fixed,
            (end - start).whole_days() as f64 / 365.0,
            None,
        )
        .with_accrual(
            CashFlowAccrual::new(
                start,
                end,
                DayCount::Act365F,
                None,
                finstack_quant_core::dates::DayCountContextState::try_new(
                    None,
                    None,
                    None,
                    Some(reference),
                    Some(false),
                )
                .expect("valid context"),
            )
            .expect("valid accrual"),
        )
        .build()
        .expect("valid cashflow")
    })
    .collect();
    let schedule = CashFlowSchedule::from_flows(
        flows,
        Notional::par(1_000_000.0, Currency::USD).unwrap(),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(jan1),
            maturity: Some(dec1),
            ..Default::default()
        },
    );

    for cfg in [
        AccrualConfig::default(),
        AccrualConfig {
            ex_coupon: Some(ExCouponRule {
                days_before_coupon: 7,
                calendar_id: None,
            }),
            ..Default::default()
        },
    ] {
        let index = AccrualIndex::build(&schedule, &cfg).unwrap();
        assert_eq!(index.period_inputs.len(), 5);
        assert_eq!(
            index.suffix_min_coupon_start.len(),
            index.period_inputs.len()
        );
        assert!(
            index
                .period_inputs
                .windows(2)
                .all(|pair| { pair[0].payment_date <= pair[1].payment_date })
        );
        assert!(
            index
                .period_inputs
                .windows(2)
                .any(|pair| pair[0].start > pair[1].start)
        );
        assert!(
            index
                .suffix_min_coupon_start
                .windows(2)
                .all(|pair| pair[0] <= pair[1])
        );
        for (i, &minimum) in index.suffix_min_coupon_start.iter().enumerate() {
            assert_eq!(
                minimum,
                index.period_inputs[i..]
                    .iter()
                    .map(|inputs| inputs.coupon_start)
                    .min()
                    .unwrap()
            );
        }
        let future = index
            .period_inputs
            .iter()
            .find(|inputs| inputs.start == aug30)
            .unwrap();
        assert_eq!(future.coupon_start, jan1);

        // Independent queries include repeats, descending order, boundaries and extrema.
        for as_of in [
            make_date(2025, 6, 29),
            jan1,
            make_date(2025, 8, 26),
            jun1,
            jul1,
            make_date(2025, 5, 15),
            sep1,
            make_date(2025, 10, 15),
            make_date(2025, 6, 29),
            Date::MIN,
            Date::MAX,
        ] {
            let full = find_active_periods_and_elapsed(&index.period_inputs, as_of, &cfg).unwrap();
            let first = index
                .period_inputs
                .partition_point(|inputs| inputs.payment_date <= as_of);
            let upper = index
                .suffix_min_coupon_start
                .partition_point(|&start| start <= as_of);
            let bounded = if first < upper {
                find_active_periods_and_elapsed(&index.period_inputs[first..upper], as_of, &cfg)
                    .unwrap()
            } else {
                Vec::new()
            };
            assert_eq!(full.len(), bounded.len());
            for ((left, left_elapsed), (right, right_elapsed)) in full.iter().zip(&bounded) {
                assert!(std::ptr::eq(*left, *right));
                assert_eq!(left_elapsed.to_bits(), right_elapsed.to_bits());
            }
            let mut expected = finstack_quant_core::math::summation::NeumaierAccumulator::default();
            for (inputs, elapsed) in full {
                expected
                    .add(accrue_in_period(inputs, elapsed, &cfg.method).unwrap())
                    .unwrap();
            }
            assert_eq!(
                index.accrued_at(as_of).unwrap(),
                Money::from_model_value(expected.get_total().unwrap(), index.get_currency())
                    .unwrap()
            );
        }
    }

    let empty = CashFlowSchedule::from_flows(
        Vec::new(),
        schedule.get_notional().clone(),
        DayCount::Act365F,
        schedule.get_meta().clone(),
    );
    let index = AccrualIndex::build(&empty, &AccrualConfig::default()).unwrap();
    assert!(index.period_inputs.is_empty());
    assert!(index.suffix_min_coupon_start.is_empty());
    assert_eq!(
        index.accrued_at(Date::MIN).unwrap().amount().to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        index.accrued_at(Date::MAX).unwrap().amount().to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn test_missing_issue_date_errors() {
    let schedule = make_test_schedule(
        &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
        DayCount::Thirty360,
    );

    let cfg = AccrualConfig::default();
    let err = build_coupon_periods(&schedule, &cfg).expect_err("missing issue date errors");

    assert!(err.to_string().contains("issue_date"));
}

#[test]
fn test_pre_coupon_flow_does_not_substitute_for_issue_date() {
    // A flow dated before the first coupon (e.g. a pre-issue fee or a
    // funding leg) must NOT silently become the accrual start;
    // meta.issue_date is required unconditionally.
    let mut schedule = make_test_schedule(
        &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
        DayCount::Thirty360,
    );

    schedule.flows.insert(
        0,
        CashFlowParams::new(
            make_date(2025, 1, 15),
            None,
            Money::from((-1_000_000_i64, Currency::USD)),
            CFKind::Notional,
            0.0,
            None,
        )
        .with_principal_delta(Money::from((1_000_000_i64, Currency::USD)))
        .with_principal_date(make_date(2025, 1, 15))
        .build()
        .expect("valid cashflow"),
    );

    let cfg = AccrualConfig::default();
    let err = build_coupon_periods(&schedule, &cfg)
        .expect_err("issue_date must be required even when a pre-coupon flow exists");
    assert!(err.to_string().contains("issue_date"));

    // With the explicit issue date set, the first period starts there.
    schedule.meta.issue_date = Some(make_date(2025, 1, 15));
    let periods = build_coupon_periods(&schedule, &cfg).expect("periods");
    assert!(!periods.is_empty());
    assert_eq!(
        periods[0].start,
        make_date(2025, 1, 15),
        "Should use explicit meta.issue_date"
    );
}

#[test]
fn accrual_index_rejects_unsorted_schedule() {
    let mut schedule = make_test_schedule(
        &[(make_date(2026, 1, 1), 0.5), (make_date(2025, 7, 1), 0.5)],
        DayCount::Thirty360,
    );
    schedule.meta.issue_date = Some(make_date(2025, 1, 1));
    AccrualIndex::build(&schedule, &AccrualConfig::default())
        .expect_err("public accrual entry validates schedule order");
}

#[test]
fn same_date_distinct_accrual_identities_sum_independently() {
    let issue = make_date(2025, 1, 1);
    let second_start = make_date(2025, 2, 1);
    let as_of = make_date(2025, 4, 1);
    let end = make_date(2025, 7, 1);
    let flows = vec![
        CashFlowParams::new(
            end,
            None,
            Money::from((30_000_i64, Currency::USD)),
            CFKind::Fixed,
            0.5,
            Some(0.06),
        )
        .with_accrual(
            CashFlowAccrual::new(
                issue,
                end,
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
        CashFlowParams::new(
            end,
            None,
            Money::from((20_000_i64, Currency::USD)),
            CFKind::Fixed,
            150.0 / 365.0,
            Some(0.048666666666666664),
        )
        .with_accrual(
            CashFlowAccrual::new(
                second_start,
                end,
                DayCount::Act365F,
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
    ];
    let schedule = CashFlowSchedule {
        flows,
        notional: Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        day_count: DayCount::Thirty360,
        meta: crate::builder::CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    };

    let accrued = accrued_interest_amount(&schedule, as_of, &AccrualConfig::default())
        .expect("both legs accrue")
        .amount();
    let expected = 30_000.0 * 90.0 / 180.0 + 20_000.0 * 59.0 / 150.0;
    assert!((accrued - expected).abs() < 1e-9, "{accrued} != {expected}");
}

/// Two slices of one coupon listed out of accrual order must not pull
/// the next, unannotated coupon's start back to the earlier slice's end.
#[test]
fn an_unannotated_coupon_starts_after_the_latest_slice_of_the_prior_coupon() {
    let issue = make_date(2025, 1, 1);
    let split = make_date(2025, 2, 15);
    let first_payment = make_date(2025, 4, 1);
    let slice = |start: Date, end: Date, amount: f64| {
        CashFlowParams::new(
            first_payment,
            None,
            Money::new(amount, Currency::USD).expect("amount"),
            CFKind::Fixed,
            45.0 / 360.0,
            Some(0.08),
        )
        .with_accrual(
            CashFlowAccrual::new(
                start,
                end,
                DayCount::Act360,
                None,
                finstack_quant_core::dates::DayCountContextState::try_new(
                    None, None, None, None, None,
                )
                .expect("valid context"),
            )
            .expect("valid accrual"),
        )
        .build()
        .expect("valid cashflow")
    };
    let schedule = CashFlowSchedule {
        // The later slice is listed first, as an amount-ordered sort does.
        flows: vec![
            slice(split, first_payment, 0.6),
            slice(issue, split, 1.0),
            CashFlowParams::new(
                make_date(2025, 7, 1),
                None,
                Money::new(1.2, Currency::USD).expect("amount"),
                CFKind::Fixed,
                91.0 / 360.0,
                Some(0.08),
            )
            .build()
            .expect("valid cashflow"),
        ],
        notional: Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        day_count: DayCount::Act360,
        meta: crate::builder::CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    };
    let accrued =
        accrued_interest_amount(&schedule, make_date(2025, 3, 2), &AccrualConfig::default())
            .expect("accrued interest")
            .amount();
    // The first slice in full and 15 of the second slice's 45 days;
    // nothing of the coupon that starts on 1 April.
    let expected = 1.0 + 0.6 * 15.0 / 45.0;
    assert!((accrued - expected).abs() < 1e-9, "{accrued} != {expected}");
}

#[test]
fn coincident_payments_keep_distinct_coupon_record_window_validation() {
    let issue = make_date(2025, 1, 1);
    let payment = make_date(2025, 7, 1);
    let later_start = make_date(2025, 6, 28);
    let mut schedule =
        make_test_schedule(&[(payment, 0.5), (payment, 3.0 / 360.0)], DayCount::Act360);
    schedule.meta.issue_date = Some(issue);
    for (flow, start) in schedule.flows.iter_mut().zip([issue, later_start]) {
        *flow = flow
            .with_accrual(
                CashFlowAccrual::new(
                    start,
                    payment,
                    DayCount::Act360,
                    None,
                    finstack_quant_core::dates::DayCountContextState::try_new(
                        None,
                        None,
                        None,
                        Some((start, payment)),
                        Some(false),
                    )
                    .expect("valid context"),
                )
                .expect("valid accrual"),
            )
            .expect("valid cashflow");
    }
    let error = accrued_interest_amount(&schedule, make_date(2025, 6, 29), &ex_coupon_cfg(7))
        .expect_err("the later coupon's record date precedes its own accrual start");
    assert!(error.to_string().contains("coupon start 2025-06-28"));
}

#[test]
fn test_accrued_interest_uses_explicit_issue_date() {
    // Integration test: accrued interest requires explicit issue metadata
    // when outstanding balances are computed from the schedule.
    let mut schedule = make_test_schedule(
        &[
            (make_date(2025, 7, 1), 0.5), // First coupon July 1
            (make_date(2026, 1, 1), 0.5), // Second coupon
        ],
        DayCount::Thirty360,
    );
    schedule.meta.issue_date = Some(make_date(2025, 1, 1));

    // Calculate accrued at April 1 (halfway through first period)
    let as_of = make_date(2025, 4, 1);
    let accrued = accrued_interest_amount(&schedule, as_of, &AccrualConfig::default())
        .unwrap()
        .amount();

    // With explicit issue date Jan 1 and first coupon July 1:
    // - Period length: 180 days (30/360)
    // - Elapsed: 90 days (Jan 1 to Apr 1)
    // - Fraction: 90/180 = 0.5
    // - Accrued: $25,000 × 0.5 = $12,500
    assert!(
        accrued > 12_000.0 && accrued < 13_000.0,
        "Accrued should be approximately $12,500, got {}",
        accrued
    );
}

// Ex-coupon window tests (negative accrued interest, UK gilt convention)

/// Semiannual 30/360 schedule: issue 2025-01-01, coupons 2025-07-01 and
/// 2026-01-01, $25k per coupon, builder accrual factor 0.5.
fn ex_coupon_test_schedule() -> CashFlowSchedule {
    let mut schedule = make_test_schedule(
        &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
        DayCount::Thirty360,
    );
    schedule.meta.issue_date = Some(make_date(2025, 1, 1));
    schedule
}

fn ex_coupon_cfg(days: u32) -> AccrualConfig {
    AccrualConfig {
        ex_coupon: Some(ExCouponRule {
            days_before_coupon: days,
            calendar_id: None,
        }),
        ..Default::default()
    }
}

#[test]
fn ex_coupon_negative_ai_linear_golden() {
    // The record date is June 24; settlement June 25 trades ex-coupon.
    // AI = −C × (total − elapsed) / total = −25_000 × 6/180.
    let schedule = ex_coupon_test_schedule();
    let cfg = ex_coupon_cfg(7);

    let accrued = accrued_interest_amount(&schedule, make_date(2025, 6, 25), &cfg)
        .expect("accrued")
        .amount();
    let expected = -25_000.0 * 6.0 / 180.0;
    assert!(
        (accrued - expected).abs() < 1e-9,
        "ex-coupon AI: expected {expected}, got {accrued}"
    );
}

#[test]
fn ex_date_trades_ex_coupon_and_prior_day_retains_coupon() {
    let schedule = ex_coupon_test_schedule();
    for method in [AccrualMethod::Linear, AccrualMethod::Compounded] {
        let cfg = AccrualConfig {
            method: method.clone(),
            ..ex_coupon_cfg(7)
        };
        let cum_cfg = AccrualConfig {
            ex_coupon: None,
            ..cfg.clone()
        };
        let prior = make_date(2025, 6, 23);
        let ex_date = make_date(2025, 6, 24);
        let actual = accrued_interest_amount(&schedule, prior, &cfg)
            .expect("cum-coupon AI")
            .amount();
        let cum = accrued_interest_amount(&schedule, prior, &cum_cfg)
            .expect("unadjusted cum-coupon AI")
            .amount();
        assert!((actual - cum).abs() < 1e-9);
        assert!(actual > 0.0);
        assert!(
            accrued_interest_amount(&schedule, ex_date, &cfg)
                .expect("ex-date AI")
                .amount()
                < 0.0
        );
    }
}

#[test]
fn gilt_ex_date_counts_business_days_and_trades_ex_inclusive() {
    let issue = make_date(2025, 1, 1);
    let payment = make_date(2025, 7, 1);
    let rule = ExCouponRule {
        days_before_coupon: 7,
        calendar_id: Some("gblo".into()),
    };
    let ex_date = rule.ex_date(payment).expect("ex-coupon date");
    assert_eq!(ex_date, make_date(2025, 6, 20));
    assert!(
        !rule
            .is_ex_coupon(payment, make_date(2025, 6, 19))
            .expect("prior business day cum-coupon")
    );
    assert!(
        rule.is_ex_coupon(payment, ex_date)
            .expect("ex-date trades ex-coupon")
    );
    assert!(
        rule.is_ex_coupon(payment, make_date(2025, 6, 23))
            .expect("next business day")
    );
    assert!(
        !rule
            .is_ex_coupon(payment, payment)
            .expect("payment boundary")
    );

    let schedule = CashFlowSchedule::from_flows(
        vec![
            CashFlowParams::new(
                payment,
                None,
                Money::from((25_000_i64, Currency::GBP)),
                CFKind::Fixed,
                0.5,
                Some(0.05),
            )
            .with_accrual(
                CashFlowAccrual::new(
                    issue,
                    payment,
                    DayCount::ActActIsma,
                    None,
                    finstack_quant_core::dates::DayCountContextState::try_new(
                        None,
                        Some(Tenor::semi_annual()),
                        None,
                        Some((issue, payment)),
                        Some(false),
                    )
                    .expect("valid context"),
                )
                .expect("valid accrual"),
            )
            .build()
            .expect("valid cashflow"),
        ],
        Notional::par(1_000_000.0, Currency::GBP).expect("notional"),
        DayCount::ActActIsma,
        crate::builder::CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    let cfg = AccrualConfig {
        ex_coupon: Some(rule),
        frequency: Some(Tenor::semi_annual()),
        ..Default::default()
    };
    let prior = accrued_interest_amount(&schedule, make_date(2025, 6, 19), &cfg)
        .expect("AI")
        .amount();
    assert!((prior - 25_000.0 * 169.0 / 181.0).abs() < 1e-9);
    let on_ex = accrued_interest_amount(&schedule, ex_date, &cfg)
        .expect("ex-date AI")
        .amount();
    assert!((on_ex - (-25_000.0 * 11.0 / 181.0)).abs() < 1e-9);
}

#[test]
fn ex_coupon_boundary_day_before_ex_date_accrues_positively() {
    // 2025-06-23 is one day before the ex-date: normal positive accrual.
    // elapsed = 172/360 (30/360), AI = 25_000 × 172/180.
    let schedule = ex_coupon_test_schedule();
    let cfg = ex_coupon_cfg(7);

    let accrued = accrued_interest_amount(&schedule, make_date(2025, 6, 23), &cfg)
        .expect("accrued")
        .amount();
    let expected = 25_000.0 * 172.0 / 180.0;
    assert!(
        (accrued - expected).abs() < 1e-9,
        "cum-coupon AI: expected {expected}, got {accrued}"
    );
}

#[test]
fn ex_coupon_boundary_day_before_period_end() {
    // 2025-06-30 = period end − 1 day, deep in the ex window:
    // AI = −25_000 × 1/180 (one 30/360 day of remaining stub).
    let schedule = ex_coupon_test_schedule();
    let cfg = ex_coupon_cfg(7);

    let accrued = accrued_interest_amount(&schedule, make_date(2025, 6, 30), &cfg)
        .expect("accrued")
        .amount();
    let expected = -25_000.0 / 180.0;
    assert!(
        (accrued - expected).abs() < 1e-9,
        "ex-window stub AI: expected {expected}, got {accrued}"
    );
}

// Year-fraction basis consistency (payment lag / BDC-shifted period ends)

/// Period reconstructed from payment dates (end = 2025-07-05, e.g. a
/// payment lag) while the builder accrual factor 0.5 covers the true
/// accrual period [2025-01-01, 2025-07-01).
fn lagged_period_inputs() -> PeriodInputs {
    PeriodInputs {
        multi_period: None,
        calendar_id: None,
        coupon_period: None,
        end_is_termination_date: Some(false),
        payment_date: make_date(2025, 7, 5),
        coupon_start: make_date(2025, 1, 1),
        coupon_end: make_date(2025, 7, 5),
        start: make_date(2025, 1, 1),
        end: make_date(2025, 7, 5),
        day_count: DayCount::Thirty360,
        notional_start: 1_000_000.0,
        coupon_total: 25_000.0,
        total_yf: 0.5,
    }
}

#[test]
fn elapsed_rescaled_and_clamped_when_exceeding_builder_accrual_factor() {
    let inputs = lagged_period_inputs();
    let periods = [inputs.clone()];
    let cfg = AccrualConfig::default();

    // 2025-07-03: 30/360 elapsed = 182/360 > total_yf = 0.5.
    // dc_total = 184/360, so elapsed is rescaled to 0.5 × 182/184.
    let (active, elapsed) = find_active_periods_and_elapsed(&periods, make_date(2025, 7, 3), &cfg)
        .expect("ok")
        .into_iter()
        .next()
        .expect("active period");

    let expected = 0.5 * (182.0 / 360.0) / (184.0 / 360.0);
    assert!(
        (elapsed - expected).abs() < 1e-12,
        "rescaled elapsed: expected {expected}, got {elapsed}"
    );
    assert!(
        elapsed <= inputs.total_yf,
        "elapsed must not exceed total_yf"
    );

    let accrued = accrue_in_period(active, elapsed, &AccrualMethod::Linear).expect("accrued");
    assert!(
        accrued <= inputs.coupon_total,
        "accrued {accrued} must not exceed full coupon {}",
        inputs.coupon_total
    );
}

#[test]
fn elapsed_rescaled_even_when_raw_day_count_is_below_builder_accrual_factor() {
    let inputs = PeriodInputs {
        multi_period: None,
        calendar_id: None,
        coupon_period: None,
        end_is_termination_date: Some(false),
        payment_date: make_date(2025, 7, 1),
        start: make_date(2025, 1, 1),
        coupon_start: make_date(2025, 1, 1),
        coupon_end: make_date(2025, 7, 1),
        end: make_date(2025, 7, 1),
        day_count: DayCount::Act365F,
        notional_start: 1_000_000.0,
        coupon_total: 37_500.0,
        total_yf: 0.75,
    };
    let periods = [inputs.clone()];
    let cfg = AccrualConfig::default();

    let (_active, elapsed) = find_active_periods_and_elapsed(&periods, make_date(2025, 4, 1), &cfg)
        .expect("ok")
        .into_iter()
        .next()
        .expect("active period");
    let expected = inputs.total_yf * 90.0 / 181.0;

    assert!(
        (elapsed - expected).abs() < 1e-12,
        "elapsed should be scaled from raw day count to builder accrual factor, got {elapsed}"
    );
}

#[test]
fn ex_window_stub_never_positive_under_payment_lag() {
    let inputs = lagged_period_inputs();
    let periods = [inputs];
    let cfg = ex_coupon_cfg(7); // ex-date 2025-06-28

    let (active, elapsed_yf) =
        find_active_periods_and_elapsed(&periods, make_date(2025, 7, 3), &cfg)
            .expect("ok")
            .into_iter()
            .next()
            .expect("active period");

    assert!(
        elapsed_yf <= 0.0,
        "ex-window elapsed_yf must be ≤ 0, got {elapsed_yf}"
    );
    let accrued = accrue_in_period(active, elapsed_yf, &AccrualMethod::Linear).expect("accrued");
    assert!(accrued <= 0.0, "ex-window AI must be ≤ 0, got {accrued}");
}

// Compounded accrual method

fn compounded_inputs() -> PeriodInputs {
    PeriodInputs {
        multi_period: None,
        calendar_id: None,
        coupon_period: None,
        end_is_termination_date: Some(false),
        payment_date: make_date(2025, 7, 1),
        start: make_date(2025, 1, 1),
        coupon_start: make_date(2025, 1, 1),
        coupon_end: make_date(2025, 7, 1),
        end: make_date(2025, 7, 1),
        day_count: DayCount::Thirty360,
        notional_start: 1_000_000.0,
        coupon_total: 25_000.0,
        total_yf: 0.5,
    }
}

#[test]
fn compounded_accrual_positive_golden() {
    // Halfway through the period: f = 0.25/0.5 = 0.5, r = 0.025.
    // AI = N × ((1 + r)^f − 1) = 1e6 × (1.025^0.5 − 1).
    let inputs = compounded_inputs();
    let accrued = accrue_in_period(&inputs, 0.25, &AccrualMethod::Compounded).expect("accrued");
    let expected = 1_000_000.0 * (1.025f64.powf(0.5) - 1.0);
    assert!(
        (accrued - expected).abs() < 1e-6,
        "compounded AI: expected {expected}, got {accrued}"
    );
}

#[test]
fn compounded_accrual_near_zero_rate_falls_back_to_linear() {
    // r = 1e-7 / 1e6 = 1e-13 < 1e-12 threshold → linear fallback.
    let inputs = PeriodInputs {
        coupon_total: 1e-7,
        ..compounded_inputs()
    };
    let accrued = accrue_in_period(&inputs, 0.25, &AccrualMethod::Compounded).expect("accrued");
    let expected = 1e-7 * (0.25 / 0.5);
    assert!(
        (accrued - expected).abs() < 1e-15,
        "near-zero-rate fallback: expected {expected}, got {accrued}"
    );
}

#[test]
fn compounded_accrual_ex_window_negative_golden() {
    // elapsed_yf = elapsed − total = 0.4 − 0.5 = −0.1, so 1 − f = 0.2.
    // AI = −N × ((1 + r)^(1−f) − 1) = −1e6 × (1.025^0.2 − 1).
    let inputs = compounded_inputs();
    let accrued = accrue_in_period(&inputs, -0.1, &AccrualMethod::Compounded).expect("accrued");
    let expected = -1_000_000.0 * (1.025f64.powf(0.2) - 1.0);
    assert!(
        (accrued - expected).abs() < 1e-6,
        "compounded ex-window AI: expected {expected}, got {accrued}"
    );
    assert!(accrued < 0.0);
}

#[test]
fn compounded_accrual_rejects_invalid_logarithm_domain() {
    for period_rate in [-1.0, -2.0, f64::NEG_INFINITY, f64::INFINITY, f64::NAN] {
        let inputs = PeriodInputs {
            coupon_total: period_rate * 1_000_000.0,
            ..compounded_inputs()
        };
        for elapsed_yf in [0.0, 0.25, -0.1] {
            let error = accrue_in_period(&inputs, elapsed_yf, &AccrualMethod::Compounded)
                .expect_err("invalid compound rate must fail before logarithm evaluation");
            assert!(error.to_string().contains("period rate"));
        }
    }
}

#[test]
fn compounded_accrual_preserves_valid_negative_rates() {
    let inputs = PeriodInputs {
        coupon_total: -200_000.0,
        ..compounded_inputs()
    };
    let accrued = accrue_in_period(&inputs, 0.25, &AccrualMethod::Compounded)
        .expect("period rates above -1 remain valid");
    let expected = 1_000_000.0 * (0.8_f64.sqrt() - 1.0);
    assert!((accrued - expected).abs() < 1e-8);

    let rebate = accrue_in_period(&inputs, -0.25, &AccrualMethod::Compounded)
        .expect("negative-rate ex-coupon rebate remains valid");
    assert!((rebate + expected).abs() < 1e-8);
}

#[test]
fn accrual_rejects_non_finite_formula_results() {
    let inputs = PeriodInputs {
        coupon_total: f64::MAX,
        ..compounded_inputs()
    };
    assert!(accrue_in_period(&inputs, 1.0, &AccrualMethod::Linear).is_err());
    assert!(accrue_in_period(&inputs, 1.0, &AccrualMethod::Compounded).is_err());
}

#[test]
fn accrual_index_rejects_non_finite_aggregate() {
    let inputs = PeriodInputs {
        coupon_total: f64::MAX,
        ..compounded_inputs()
    };
    let coupon_start = inputs.coupon_start;
    let index = AccrualIndex {
        period_inputs: vec![inputs; 3],
        suffix_min_coupon_start: vec![coupon_start; 3],
        currency: Currency::USD,
        cfg: AccrualConfig::default(),
    };
    let error = index
        .accrued_at(make_date(2025, 6, 30))
        .expect_err("finite individual accruals must not produce a non-finite total");
    assert!(
        error.to_string().contains("compensated sum overflowed"),
        "unexpected error: {error}"
    );
}

#[test]
fn act_act_isma_without_frequency_errors() {
    let mut schedule = make_test_schedule(
        &[(make_date(2025, 7, 1), 0.5), (make_date(2026, 1, 1), 0.5)],
        DayCount::ActActIsma,
    );
    schedule.meta.issue_date = Some(make_date(2025, 1, 1));

    let err = accrued_interest_amount(&schedule, make_date(2025, 4, 1), &AccrualConfig::default())
        .expect_err("ICMA without frequency must error");
    assert!(err.to_string().to_lowercase().contains("frequency"));

    let cfg = AccrualConfig {
        frequency: Some(Tenor::semi_annual()),
        ..Default::default()
    };
    let accrued = accrued_interest_amount(&schedule, make_date(2025, 4, 1), &cfg)
        .expect("ICMA with frequency succeeds")
        .amount();
    assert!(accrued > 0.0);
}

#[test]
fn nan_coupon_total_is_rejected() {
    let schedule = make_test_schedule(&[(make_date(2025, 7, 1), 0.5)], DayCount::Thirty360);
    let periods = [CouponPeriod {
        start: make_date(2025, 1, 1),
        end: make_date(2025, 7, 1),
        day_count: DayCount::Thirty360,
        bucket: CouponBucket {
            multi_period: None,
            calendar_id: None,
            coupon_period: None,
            end_is_termination_date: Some(false),
            date: make_date(2025, 7, 1),
            accrual_start: None,
            accrual_end: None,
            accrual_day_count: None,
            cash_amount: f64::NAN,
            pik_amount: 0.0,
            accrual_factor: 0.5,
        },
    }];

    let err = build_period_inputs(&schedule, &periods, &[])
        .expect_err("NaN coupon total must be rejected");
    assert!(err.to_string().contains("non-finite"));
}

#[test]
fn ex_coupon_window_spanning_full_period_is_rejected() {
    let inputs = PeriodInputs {
        multi_period: None,
        calendar_id: None,
        coupon_period: None,
        end_is_termination_date: Some(false),
        payment_date: make_date(2025, 2, 1),
        coupon_start: make_date(2025, 1, 1),
        coupon_end: make_date(2025, 2, 1),
        start: make_date(2025, 1, 1),
        end: make_date(2025, 2, 1),
        day_count: DayCount::Act365F,
        notional_start: 1_000_000.0,
        coupon_total: 4_166.67,
        total_yf: 1.0 / 12.0,
    };
    let periods = [inputs];
    let cfg = ex_coupon_cfg(45);

    let err = find_active_periods_and_elapsed(&periods, make_date(2025, 1, 1), &cfg)
        .expect_err("ex-coupon window before period start must be rejected");
    assert!(err.to_string().contains("ex-coupon"));
}

#[test]
fn accrual_config_rejects_unknown_json_fields() {
    let err = serde_json::from_str::<AccrualConfig>(
        r#"{"method":"linear","include_pik":true,"frequency":null,"strict_issue_date":true}"#,
    )
    .expect_err("unknown top-level accrual field must be rejected");
    assert!(err.to_string().contains("strict_issue_date"));

    let nested = serde_json::from_str::<AccrualConfig>(
        r#"{"method":"linear","include_pik":true,"frequency":null,"ex_coupon":{"days_before_coupon":7,"bogus":1}}"#,
    )
    .expect_err("unknown nested ex-coupon field must be rejected");
    assert!(nested.to_string().contains("bogus"));
}

#[test]
fn ex_coupon_rule_rejects_days_above_366() {
    let rule = ExCouponRule {
        days_before_coupon: 367,
        calendar_id: None,
    };
    let err = rule
        .ex_date(make_date(2025, 7, 1))
        .expect_err("days_before_coupon > 366 must be rejected");
    assert!(err.to_string().contains("366"));

    // 366 itself is accepted (one leap year).
    let rule_ok = ExCouponRule {
        days_before_coupon: 366,
        calendar_id: None,
    };
    assert!(rule_ok.ex_date(make_date(2025, 7, 1)).is_ok());
}
