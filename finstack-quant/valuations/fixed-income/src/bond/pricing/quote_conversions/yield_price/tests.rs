use super::super::types::YieldCompounding;
use super::workout::{price_workout_path_from_yield, workout_cashflow_paths};
use super::*;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::pricing::ytm_solver::{YtmPricingSpec, solve_bond_ytm};
use crate::bond::{
    Bond, CallPut, CallPutSchedule, MakeWholeSpec, ProtectionWindow, ReturnFloorSpec,
};
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use time::macros::date;

#[test]
fn icma_ytm_is_invariant_to_weekend_payment_roll() {
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind};
    let as_of = date!(2025 - 04 - 01);
    let mut rolled = Bond::fixed(
        "WEEKEND",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.05).unwrap(),
        date!(2024 - 12 - 15),
        date!(2026 - 12 - 15),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    if let crate::bond::CashflowSpec::Fixed(spec) = &mut rolled.cashflow_spec {
        spec.schedule.day_count = DayCount::ActActIsma;
    }
    let mut unadjusted = rolled.clone();
    if let crate::bond::CashflowSpec::Fixed(spec) = &mut unadjusted.cashflow_spec {
        spec.schedule.business_day_convention = BusinessDayConvention::Unadjusted;
    }
    let market = MarketContext::new();
    let rolled_flows = rolled.pricing_dated_cashflows(&market, as_of).unwrap();
    let unadjusted_flows = unadjusted.pricing_dated_cashflows(&market, as_of).unwrap();
    assert_ne!(rolled_flows[0].0, unadjusted_flows[0].0);
    let price = price_from_ytm(&unadjusted, &unadjusted_flows, as_of, 0.04).unwrap();
    let rolled_yield = solve_bond_ytm(
        &rolled,
        &rolled_flows,
        as_of,
        Money::new(price, Currency::USD).unwrap(),
        YtmPricingSpec {
            day_count: DayCount::ActActIsma,
            notional: rolled.notional,
            coupon_rate: 0.05,
            compounding: YieldCompounding::Street,
            frequency: rolled.cashflow_spec.frequency(),
        },
    )
    .unwrap();
    assert!((rolled_yield - 0.04).abs() < 1e-10, "{rolled_yield}");
}

#[test]
fn street_yield_uses_simple_discounting_in_the_final_contractual_period() {
    let as_of = date!(2025 - 11 - 15);
    let bond = Bond::fixed(
        "STREET-FINAL",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.05).unwrap(),
        date!(2024 - 01 - 15),
        date!(2026 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    let flows = bond
        .pricing_dated_cashflows(&MarketContext::new(), as_of)
        .unwrap();
    let terminal = flows
        .iter()
        .filter(|&&(date, _)| date > as_of)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    let expected = terminal / (1.0 + 0.12 * 60.0 / 360.0);
    let actual = price_from_ytm(&bond, &flows, as_of, 0.12).unwrap();
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn time_based_moosmuller_counts_whole_periods_after_the_first() {
    let (ytm, w) = (0.04, 0.125);
    let simple = 1.0 / (1.0 + ytm * w);
    let df = |t: f64| {
        df_from_yield(
            ytm,
            t,
            YieldCompounding::Moosmuller,
            finstack_quant_core::dates::Tenor::semi_annual(),
        )
        .expect("df")
    };
    // Inside the first period only simple interest applies.
    assert_eq!(df(w), simple);
    // Two whole semi-annual periods after the first.
    assert_eq!(df(w + 1.0), simple * (1.0 + ytm / 2.0).powf(-2.0));
}

#[test]
fn exit_windows_keep_exact_contractual_boundaries() {
    let mut bond = Bond::example().expect("example bond");
    let start = date!(2027 - 01 - 10);
    let flow_date = date!(2027 - 01 - 15);
    let end = date!(2027 - 01 - 20);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start,
            end,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let flows = vec![(flow_date, Money::from((5_i64, Currency::USD)))];

    let dates: Vec<_> = enumerate_exit_paths(&bond, &flows, date!(2026 - 01 - 01))
        .into_iter()
        .map(|candidate| candidate.date)
        .collect();
    assert_eq!(dates, vec![start, flow_date, end]);
}

#[test]
fn same_date_calls_retain_distinct_make_whole_terms_while_puts_collapse() {
    let mut bond = Bond::example().expect("example bond");
    let exercise_date = date!(2027 - 01 - 15);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![
            CallPut {
                start: date!(2027 - 01 - 10),
                end: exercise_date,
                price_pct_of_par: 100.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "REF-A".into(),
                    spread_bp: 25.0,
                }),
            },
            CallPut {
                start: exercise_date,
                end: date!(2027 - 01 - 20),
                price_pct_of_par: 99.0,
                make_whole: Some(MakeWholeSpec {
                    reference_curve_id: "REF-B".into(),
                    spread_bp: 50.0,
                }),
            },
        ],
        puts: vec![
            CallPut {
                start: exercise_date,
                end: exercise_date,
                price_pct_of_par: 98.0,
                make_whole: None,
            },
            CallPut {
                start: exercise_date,
                end: exercise_date,
                price_pct_of_par: 101.0,
                make_whole: None,
            },
        ],
    });

    let candidates = enumerate_exit_paths(&bond, &[], date!(2026 - 01 - 01));
    let calls: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.date == exercise_date)
        .filter_map(|candidate| candidate.make_whole.as_ref())
        .collect();
    let puts: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.date == exercise_date && candidate.make_whole.is_none())
        .collect();

    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .any(|spec| spec.reference_curve_id.as_str() == "REF-A")
    );
    assert!(
        calls
            .iter()
            .any(|spec| spec.reference_curve_id.as_str() == "REF-B")
    );
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].price_pct_of_par, 101.0);
}

#[test]
fn make_whole_redemption_uses_reference_value_above_fixed_floor() {
    let as_of = date!(2025 - 01 - 01);
    let exercise_date = date!(2026 - 01 - 01);
    let mut bond = Bond::fixed(
        "MW-WORKOUT",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.10).expect("valid rate fixture"),
        as_of,
        date!(2027 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: exercise_date,
            end: exercise_date,
            price_pct_of_par: 50.0,
            make_whole: Some(MakeWholeSpec {
                reference_curve_id: "USD-REF".into(),
                spread_bp: 0.0,
            }),
        }],
        puts: Vec::new(),
    });
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 0.90)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("USD-REF")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 1.0)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("reference curve"),
        )
        .expect("insert market curve");
    let flows = bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("pricing flows");
    let schedule = bond
        .full_cashflow_schedule(&market)
        .expect("cashflow schedule");
    let basis = RedemptionBasis::new(&schedule, &bond.accrual_config()).expect("redemption basis");
    let candidate = enumerate_exit_paths(&bond, &flows, as_of)
        .into_iter()
        .find(|candidate| candidate.date == exercise_date)
        .expect("make-whole candidate");

    let redemption = exercise_redemption_amount(&bond, &market, &flows, &basis, &candidate)
        .expect("make-whole redemption");
    let remaining_reference_value: f64 = flows
        .iter()
        .filter(|(date, _)| *date > exercise_date)
        .map(|(_, amount)| amount.amount())
        .sum();

    assert!((redemption - remaining_reference_value).abs() < 1e-10);
    assert!(redemption > 100.0);
}

#[test]
fn quoted_workout_uses_make_whole_redemption_when_selecting_ytw_path() {
    let as_of = date!(2025 - 01 - 15);
    let exercise_date = date!(2025 - 12 - 15);
    let mut make_whole_bond = Bond::fixed(
        "MW-QUOTED-WORKOUT",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.10).expect("valid rate fixture"),
        as_of,
        date!(2027 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    make_whole_bond.settlement_convention = None;
    make_whole_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(100.0);
    make_whole_bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: exercise_date,
            end: exercise_date,
            price_pct_of_par: 50.0,
            make_whole: Some(MakeWholeSpec {
                reference_curve_id: "USD-REF".into(),
                spread_bp: 0.0,
            }),
        }],
        puts: Vec::new(),
    });
    let mut fixed_strike_bond = make_whole_bond.clone();
    fixed_strike_bond
        .call_put
        .as_mut()
        .expect("call schedule")
        .calls[0]
        .make_whole = None;
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 0.90)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("USD-REF")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 1.0)])
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("reference curve"),
        )
        .expect("insert market curve");

    let make_whole_flows = make_whole_bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("pricing flows");
    let make_whole_path = crate::bond::pricing::quote_conversions::quoted_workout_path(
        &make_whole_bond,
        &market,
        as_of,
        &make_whole_flows,
    )
    .expect("make-whole workout")
    .expect("callable workout path")
    .flows;
    let fixed_flows = fixed_strike_bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("pricing flows");
    let fixed_path = crate::bond::pricing::quote_conversions::quoted_workout_path(
        &fixed_strike_bond,
        &market,
        as_of,
        &fixed_flows,
    )
    .expect("fixed-strike workout")
    .expect("callable workout path")
    .flows;

    let make_whole_exit = make_whole_path
        .iter()
        .map(|(date, _)| *date)
        .max()
        .expect("make-whole exit date");
    let fixed_exit = fixed_path
        .iter()
        .map(|(date, _)| *date)
        .max()
        .expect("fixed-strike exit date");
    assert!(make_whole_exit > exercise_date);
    assert_eq!(fixed_exit, exercise_date);

    let target_ytw = 0.10;
    let maturity_price = price_from_ytm(&make_whole_bond, &make_whole_flows, as_of, target_ytw)
        .expect("maturity price");
    let make_whole_price =
        price_from_ytw(&make_whole_bond, &market, as_of, target_ytw).expect("make-whole YTW price");
    let fixed_price = price_from_ytw(&fixed_strike_bond, &market, as_of, target_ytw)
        .expect("fixed-strike YTW price");
    assert!((make_whole_price - maturity_price).abs() < 1e-10);
    assert!(fixed_price + 25.0 < make_whole_price);
}

#[test]
fn price_from_ytw_matches_ytm_for_bullet_with_rolled_final_payment() {
    let as_of = date!(2025 - 01 - 03);
    let mut bond = Bond::fixed(
        "YTW-ROLLED-MATURITY",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2026 - 01 - 03),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    let market = MarketContext::new();
    let quote_ctx = QuoteDateContext::new(&bond, &market, as_of).expect("quote context");
    let flows = quote_ctx
        .entitled_flows(&bond, &market, as_of)
        .expect("entitled flows");
    assert!(flows.iter().any(|(date, _)| *date > bond.maturity));

    let target_yield = 0.04;
    let expected =
        price_from_ytm(&bond, &flows, quote_ctx.quote_date, target_yield).expect("YTM price");
    let actual = price_from_ytw(&bond, &market, as_of, target_yield).expect("YTW inverse price");

    assert!((actual - expected).abs() < 1e-12);
}

#[test]
fn same_day_coupon_precedes_exercise_in_ytw_price_and_round_trip() {
    let issue = date!(2024 - 01 - 15);
    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        "YTW-SAME-DAY-EXERCISE",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.10).expect("valid rate fixture"),
        issue,
        date!(2026 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: as_of,
            end: as_of,
            price_pct_of_par: 101.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let market = MarketContext::new();
    let schedule = bond
        .full_cashflow_schedule(&market)
        .expect("cashflow schedule");
    let inclusive_flows = bond
        .pricing_dated_cashflows_from_schedule_inclusive(&schedule, as_of, as_of)
        .expect("inclusive exercise flows");
    let same_day_coupon: f64 = inclusive_flows
        .iter()
        .filter(|(date, _)| *date == as_of)
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!((same_day_coupon - 5.0).abs() < 1e-10);

    let target_ytw = 0.10;
    let maturity_price = same_day_coupon
        + price_from_ytm(&bond, &inclusive_flows, as_of, target_ytw).expect("maturity path price");
    let immediate_call_cash = same_day_coupon + 101.0;
    assert!(immediate_call_cash > maturity_price);

    let inverted = price_from_ytw(&bond, &market, as_of, target_ytw).expect("same-day YTW price");
    assert!((inverted - maturity_price).abs() < 1e-10);

    let strict_flows = bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("strict quote flows");
    let (solved, selected_path) = solve_ytw_from_flows(
        &bond,
        &market,
        &strict_flows,
        as_of,
        Money::new(inverted, Currency::USD).expect("valid money fixture"),
        &schedule,
    )
    .expect("same-day YTW solve");
    assert!((solved - target_ytw).abs() < 1e-10);
    assert!(
        selected_path
            .iter()
            .any(|(date, amount)| *date == as_of && amount.amount() > 0.0)
    );
}

#[test]
fn price_from_ytw_lowers_return_floor_only_bond_into_workout_paths() {
    let as_of = date!(2025 - 01 - 15);
    let exercise_date = date!(2025 - 12 - 15);
    let mut bullet = Bond::fixed(
        "YTW-RETURN-FLOOR",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.10).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bullet.settlement_convention = None;
    let floored = bullet
        .clone()
        .with_return_floor(
            ReturnFloorSpec::moic(0.50).window(ProtectionWindow::Between {
                start: exercise_date,
                end: exercise_date.next_day().expect("following date"),
            }),
        );
    assert!(floored.call_put.is_none());

    let market = MarketContext::new();
    let target_ytw = 0.0;
    let bullet_price =
        price_from_ytw(&bullet, &market, as_of, target_ytw).expect("bullet YTW price");
    let floored_price =
        price_from_ytw(&floored, &market, as_of, target_ytw).expect("floor YTW price");

    let effective = floored
        .effective_for_pricing(&market, as_of)
        .expect("lowered floor");
    assert!(effective.return_floor.is_none());
    assert!(
        effective
            .call_put
            .as_ref()
            .is_some_and(CallPutSchedule::has_options)
    );
    let quote_ctx = QuoteDateContext::new(&effective, &market, as_of).expect("quote context");
    let flows = quote_ctx
        .entitled_flows(&effective, &market, as_of)
        .expect("entitled flows");
    let schedule = effective
        .full_cashflow_schedule(&market)
        .expect("cashflow schedule");
    let paths =
        workout_cashflow_paths(&effective, &market, &flows, quote_ctx.quote_date, &schedule)
            .expect("lowered workout paths");
    assert!(paths.iter().any(|path| {
        path.flows
            .iter()
            .map(|(date, _)| *date)
            .max()
            .is_some_and(|date| date <= exercise_date.next_day().expect("following date"))
    }));
    let manual_min = paths
        .iter()
        .map(|path| {
            price_workout_path_from_yield(&effective, &path.flows, quote_ctx.quote_date, target_ytw)
                .expect("path price")
        })
        .fold(f64::INFINITY, f64::min);

    assert!((floored_price - manual_min).abs() < 1e-12);
    assert!(floored_price + 25.0 < bullet_price);
}

#[test]
fn terminal_exercise_replaces_contractual_redemption() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let schedule = CashFlowSchedule::from_flows(
        vec![
            {
                let cash = Money::from((-100_i64, Currency::USD));
                CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(finstack_quant_core::money::Money::zero(cash.currency()))
                    .with_principal_date(issue)
                    .build()
                    .expect("valid cashflow")
            },
            CashFlowParams::new(
                maturity,
                None,
                Money::from((5_i64, Currency::USD)),
                CFKind::Fixed,
                0.0,
                None,
            )
            .build()
            .expect("valid cashflow"),
            {
                let cash = Money::from((100_i64, Currency::USD));
                CashFlowParams::new(maturity, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(maturity)
                    .build()
                    .expect("valid cashflow")
            },
        ],
        Notional::par(100.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            maturity: Some(maturity),
            ..CashFlowMeta::default()
        },
    );

    let (principal, replaced) = RedemptionBasis::new(
        &schedule,
        &finstack_quant_cashflows::accrual::AccrualConfig::default(),
    )
    .expect("redemption basis")
    .exercise_principal_and_replaced_redemption(maturity, maturity);
    assert_eq!(principal, 100.0);
    assert_eq!(replaced, 100.0);
}
