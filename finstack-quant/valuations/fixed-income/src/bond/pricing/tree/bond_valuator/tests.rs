use super::*;
use crate::bond::{Bond, CallPut, CallPutSchedule, CashflowSpec, MakeWholeSpec};
use crate::rates_credit::continuous_frp_weight;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::time_grid::map_date_to_step;
use finstack_quant_core::money::Money;
use finstack_quant_models::trees::tree_framework::{NodeState, TreeValuator};
use finstack_quant_models::trees::two_factor_rates_credit::{
    RatesCreditCalibrationTargets, RatesCreditConfig, RatesCreditTree,
};
use time::macros::date;

fn node_test_valuator(
    current_payment: f64,
    terminal_redemption: f64,
    call: [Option<f64>; 2],
    put: [Option<f64>; 2],
    outstanding: f64,
    recovery_rate: Option<f64>,
    call_friction_cents: f64,
) -> BondValuator {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let bond = Bond::fixed(
        "NODE-TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    BondValuator {
        bond,
        cashflow_vec: vec![current_payment, current_payment + terminal_redemption],
        cashflow_components: vec![Vec::new(), Vec::new()],
        redemption_vec: vec![0.0, terminal_redemption],
        redemption_components: vec![Vec::new(), Vec::new()],
        call_vec: call.to_vec(),
        put_vec: put.to_vec(),
        outstanding_principal_vec: vec![outstanding, 0.0],
        time_steps: vec![0.0, 1.0],
        step_discount_factors: vec![1.0, 1.0],
        recovery_rate,
        call_friction_cents,
    }
}

#[test]
fn risky_hold_is_survival_weighted_before_exercise_and_current_payment_is_not() {
    let valuator = node_test_valuator(5.0, 100.0, [None; 2], [None; 2], 100.0, Some(0.4), 0.0);
    let rate: f64 = 0.03;
    let hazard: f64 = 0.10;
    let dt: f64 = 1.0;
    let interval_df = (-rate * dt).exp();
    let state = NodeState {
        step: 0,
        interest_rate: Some(rate),
        hazard_rate: Some(hazard),
        df: Some(interval_df),
        ..NodeState::default()
    };
    let discounted_continuation = 100.0 * interval_df;

    let actual = valuator
        .value_at_node(&state, discounted_continuation, dt)
        .expect("node value");
    let survival = (-hazard * dt).exp();
    let recovery = 0.4 * 100.0 * hazard * (1.0 - (-(rate + hazard) * dt).exp()) / (rate + hazard);
    let expected = 5.0 + survival * discounted_continuation + recovery;
    assert!(
        (actual - expected).abs() < 1e-12,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn immediate_put_is_not_survival_discounted_and_ends_recovery_exposure() {
    let valuator = node_test_valuator(
        5.0,
        100.0,
        [None; 2],
        [Some(100.0), None],
        100.0,
        Some(0.4),
        0.0,
    );
    let state = NodeState {
        step: 0,
        interest_rate: Some(0.0),
        hazard_rate: Some(1.0),
        df: Some(1.0),
        ..NodeState::default()
    };

    let actual = valuator
        .value_at_node(&state, 0.0, 1.0)
        .expect("node value");
    assert_eq!(actual, 105.0);
}

#[test]
fn within_step_recovery_has_stable_zero_denominator_limit() {
    let survival = (-0.10_f64).exp();
    let weight = continuous_frp_weight(0.10_f64.exp(), survival).expect("recovery weight");
    assert!((weight - 0.10).abs() < 1e-12, "weight={weight}");
}

#[test]
fn deterministic_rates_credit_scalar_pass_matches_one_step_frp() {
    let valuator = node_test_valuator(0.0, 100.0, [None; 2], [None; 2], 100.0, Some(0.4), 0.0);
    let rate = 0.03_f64;
    let hazard = 0.10_f64;
    let discount = (-rate).exp();
    let survival = (-hazard).exp();
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 1,
        ..RatesCreditConfig::default()
    });
    tree.calibrate(&RatesCreditCalibrationTargets {
        times: vec![0.0, 1.0],
        discount_factors: vec![1.0, discount],
        survival_probabilities: vec![1.0, survival],
        recovery_rate: 0.4,
    })
    .expect("calibration");

    let actual = valuator
        .price_deterministic_rates_credit(&tree, 0.0, Compounding::Continuous)
        .expect("scalar price");
    let recovery =
        0.4 * 100.0 * continuous_frp_weight(discount, survival).expect("recovery weight");
    let expected = discount * survival * 100.0 + recovery;
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn terminal_put_replaces_redemption_and_preserves_coupon() {
    let valuator = node_test_valuator(
        50.0,
        1_000.0,
        [None; 2],
        [None, Some(1_100.0)],
        1_000.0,
        None,
        0.0,
    );
    let state = NodeState {
        step: 1,
        ..NodeState::default()
    };

    let actual = valuator.value_at_maturity(&state).expect("terminal value");
    assert_eq!(actual, 1_150.0);
}

#[test]
fn call_friction_changes_threshold_but_not_holder_proceeds() {
    let valuator = node_test_valuator(
        0.0,
        100.0,
        [Some(100.0), None],
        [None; 2],
        100.0,
        None,
        100.0,
    );
    assert_eq!(valuator.apply_exercise(0, 100.5), 100.5);
    assert_eq!(valuator.apply_exercise(0, 101.5), 100.0);
}

#[test]
fn exercise_candidates_are_window_ends_interior_events_and_month_ends() {
    let dates = BondValuator::exercise_candidates(
        date!(2025 - 01 - 15),
        date!(2025 - 04 - 10),
        &[
            date!(2024 - 07 - 15),
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 17),
            date!(2025 - 07 - 15),
        ],
    );
    assert_eq!(
        dates,
        vec![
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 31),
            date!(2025 - 02 - 17),
            date!(2025 - 02 - 28),
            date!(2025 - 03 - 31),
            date!(2025 - 04 - 10)
        ]
    );
    assert!(
        BondValuator::exercise_candidates(date!(2025 - 02 - 01), date!(2025 - 01 - 01), &[])
            .is_empty()
    );
}

#[test]
fn explicit_grid_day_count_controls_event_mapping_when_curve_basis_differs() {
    let as_of = date!(2024 - 01 - 01);
    let exercise = date!(2024 - 06 - 29);
    let maturity = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ACT365F-GRID",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: exercise,
            end: exercise,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.1, 0.94)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let horizon = DayCount::Act365F
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("ACT/365F horizon");
    // The exercise time is 180/365 = 0.49315. On the curve's ACT/360
    // basis it would be 0.5, which would snap to step 2 on this grid.
    let grid = vec![0.0, 0.49, 0.497, horizon];

    let valuator = BondValuator::new_with_time_steps_and_day_count(
        bond,
        &market,
        as_of,
        grid,
        DayCount::Act365F,
    )
    .expect("valuator");

    assert!(valuator.call_vec[1].is_some());
    assert!(valuator.call_vec[2].is_none());
}

#[test]
fn canonical_balance_replay_carries_historical_amortization_and_pik() {
    let issue = date!(2024 - 01 - 01);
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let money = |amount| Money::new(amount, Currency::USD).expect("valid money fixture");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            {
                let cash = money(-1_000.0);
                CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(finstack_quant_core::money::Money::zero(cash.currency()))
                    .with_principal_date(issue)
                    .build()
                    .expect("valid cashflow")
            },
            {
                let cash = money(200.0);
                CashFlowParams::new(
                    date!(2024 - 06 - 01),
                    None,
                    cash,
                    CFKind::Amortization,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(date!(2024 - 06 - 01))
                .build()
                .expect("valid cashflow")
            },
            CashFlowParams::new(
                date!(2024 - 12 - 01),
                None,
                money(0.0),
                CFKind::Pik,
                0.0,
                None,
            )
            .with_principal_delta(money(50.0))
            .with_principal_date(date!(2024 - 12 - 01))
            .build()
            .expect("valid cashflow"),
            {
                let cash = money(850.0);
                CashFlowParams::new(maturity, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(maturity)
                    .build()
                    .expect("valid cashflow")
            },
        ],
        Notional::par(1_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            maturity: Some(maturity),
            ..CashFlowMeta::default()
        },
    );
    let mut bond = Bond::from_cashflows(
        "SEASONED-PIK",
        schedule,
        finstack_quant_core::math::Compounding::SEMI_ANNUAL,
        "USD-OIS",
        None,
    )
    .expect("custom bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: as_of,
            end: as_of,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![CallPut {
            start: maturity,
            end: maturity,
            price_pct_of_par: 110.0,
            make_whole: None,
        }],
    });
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");

    let valuator = BondValuator::new(bond, &market, as_of, 1.0, 4).expect("valuator");
    assert!((valuator.outstanding_principal_vec[0] - 850.0).abs() < 1e-12);
    assert_eq!(valuator.call_vec[0], Some(850.0));
    assert!((valuator.put_vec[4].expect("terminal put") - 935.0).abs() < 1e-12);
}

#[test]
fn as_of_coupon_and_amortization_are_paid_before_immediate_call() {
    let issue = date!(2024 - 01 - 01);
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let money = |amount| Money::new(amount, Currency::USD).expect("valid money fixture");
    let schedule = CashFlowSchedule::from_flows(
        vec![
            {
                let cash = money(-1_000.0);
                CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(finstack_quant_core::money::Money::zero(cash.currency()))
                    .with_principal_date(issue)
                    .build()
                    .expect("valid cashflow")
            },
            CashFlowParams::new(as_of, None, money(50.0), CFKind::Fixed, 0.0, None)
                .build()
                .expect("valid cashflow"),
            {
                let cash = money(200.0);
                CashFlowParams::new(as_of, None, cash, CFKind::Amortization, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(as_of)
                    .build()
                    .expect("valid cashflow")
            },
            {
                let cash = money(800.0);
                CashFlowParams::new(maturity, None, cash, CFKind::Notional, 0.0, None)
                    .with_principal_delta(cash.checked_neg())
                    .with_principal_date(maturity)
                    .build()
                    .expect("valid cashflow")
            },
        ],
        Notional::par(1_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            maturity: Some(maturity),
            ..CashFlowMeta::default()
        },
    );
    let mut bond = Bond::from_cashflows(
        "SAME-DAY-EXERCISE",
        schedule,
        finstack_quant_core::math::Compounding::SEMI_ANNUAL,
        "USD-OIS",
        None,
    )
    .expect("bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: as_of,
            end: as_of,
            price_pct_of_par: 90.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 1.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");

    let valuator = BondValuator::new(bond, &market, as_of, 1.0, 1).expect("valuator");
    assert_eq!(valuator.cashflow_vec[0], 250.0);
    assert_eq!(valuator.outstanding_principal_vec[0], 800.0);
    assert_eq!(valuator.call_vec[0], Some(720.0));
    let price = valuator
        .price_deterministic_discount_curve(0.0, Compounding::Continuous)
        .expect("price");
    assert_eq!(price, 970.0);
}

#[test]
fn make_whole_reference_value_adds_near_coupon_accrued_once() {
    let issue = date!(2025 - 01 - 01);
    let exercise = date!(2025 - 06 - 30);
    let maturity = date!(2026 - 01 - 01);
    let mut bond = Bond::fixed(
        "MAKE-WHOLE-ACCRUED",
        Money::from((1_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.10).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: exercise,
            end: exercise,
            price_pct_of_par: 50.0,
            make_whole: Some(MakeWholeSpec {
                reference_curve_id: "USD-OIS".into(),
                spread_bp: 0.0,
            }),
        }],
        puts: Vec::new(),
    });
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 1.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let flows = bond
        .pricing_dated_cashflows(&market, issue)
        .expect("cashflows");
    let first_future_date = flows
        .iter()
        .find_map(|(date, _)| (*date > exercise).then_some(*date))
        .expect("future coupon");
    assert_eq!(first_future_date, date!(2025 - 07 - 01));
    let reference_dirty = flows
        .iter()
        .filter(|(date, _)| *date > exercise)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    let day_count = market
        .get_discount("USD-OIS")
        .expect("discount curve")
        .day_count();
    let exercise_time = day_count
        .year_fraction(issue, exercise, DayCountContext::default())
        .expect("exercise time");
    let maturity_time = day_count
        .year_fraction(issue, maturity, DayCountContext::default())
        .expect("maturity time");

    let valuator = BondValuator::new_with_time_steps(
        bond,
        &market,
        issue,
        vec![0.0, exercise_time, maturity_time],
    )
    .expect("valuator");
    let dirty_call = valuator.call_vec[1].expect("make-whole call");

    assert!(reference_dirty > 1_000.0);
    assert!(
        (dirty_call - reference_dirty).abs() < 1.0e-10,
        "the dirty reference PV already contains accrued cash: call={dirty_call}, reference={reference_dirty}"
    );
}

#[test]
fn incompatible_same_date_put_above_call_is_rejected() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let exercise = date!(2025 - 07 - 01);
    let mut bond = Bond::fixed(
        "BAD-BARRIERS",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: exercise,
            end: exercise,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![CallPut {
            start: exercise,
            end: exercise,
            price_pct_of_par: 101.0,
            make_whole: None,
        }],
    });
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");

    let error = BondValuator::new(bond, &market, as_of, 1.0, 12)
        .err()
        .expect("crossed barriers must fail");
    assert!(error.to_string().contains("incompatible call/put barriers"));
}

/// Item 8 regression: off-grid coupons on the **non-exercise** path are
/// distributed across the two bracketing tree steps, and each distributed
/// piece must carry the discount-factor correction that moves it from the
/// coupon's true time to its destination step time — exactly as the
/// exercise-coincident path does via `value_at_step_time`.
///
/// Aggregate PV-preservation invariant: discounting every step's mapped
/// cashflow at that step's time must reproduce the total present value of
/// the bond's raw cashflows discounted at their true times:
/// `Σ_step cashflow_vec[s] · DF(step_time_s) == Σ_flow amount · DF(true_t)`.
/// Without the DF correction on the distributed path the raw linear split
/// mis-times discounting and this identity fails.
#[test]
fn off_grid_nonexercise_cashflows_carry_df_correction() {
    let as_of = date!(2025 - 01 - 01);
    // Call snapped to its own step; the annual coupons below land off-grid
    // and take the distributed (non-exercise) mapping path.
    let call_date = date!(2031 - 06 - 15);
    let maturity = date!(2032 - 01 - 01);
    let tree_steps = 9;
    let mut bond = Bond::fixed(
        "OFF-GRID-NONEX-CF",
        Money::from((1_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.cashflow_spec =
        CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    bond.quote_compounding =
        finstack_quant_core::math::Compounding::Periodic(std::num::NonZeroU32::MIN);
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: call_date,
            end: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    // Strongly sloped curve so a DF-correction error would be material.
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.50)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
    let day_count = discount_curve.day_count();
    let time_to_maturity = day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("time to maturity");
    let dt = time_to_maturity / tree_steps as f64;

    let flows = bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("cashflows");
    // Confirm the test premise: at least one coupon is genuinely off-grid.
    let has_off_grid = flows.iter().any(|(date, _)| {
        if *date <= as_of {
            return false;
        }
        let tf = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .unwrap_or(0.0);
        let raw = (tf / time_to_maturity) * tree_steps as f64;
        let frac = raw - raw.floor();
        frac > 1e-6 && frac < 1.0 - 1e-6
    });
    assert!(has_off_grid, "test premise: an off-grid coupon must exist");

    let valuator =
        BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");

    // PV of every step's mapped cashflow, discounted at its step time.
    let mut pv_mapped = 0.0;
    for (step, amount) in valuator.cashflow_vec.iter().enumerate() {
        pv_mapped += amount * discount_curve.df(step as f64 * dt).expect("df");
    }
    // PV of the raw cashflows at their true times.
    let mut pv_true = 0.0;
    for (date, amount) in &flows {
        if *date <= as_of {
            continue;
        }
        let tf = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .expect("year fraction");
        pv_true += amount.amount() * discount_curve.df(tf).expect("df");
    }

    assert!(
        (pv_mapped - pv_true).abs() < 1e-9,
        "tree cashflow mapping must preserve present value under distributed \
         (non-exercise) off-grid coupons: pv_mapped={pv_mapped}, pv_true={pv_true}, \
         diff={}",
        (pv_mapped - pv_true).abs()
    );
}

/// Regression: a coupon whose raw step index lands in (0, 1)
/// (i.e. inside the first time step) must keep its `(1 - weight)` share at
/// step 0. The old guard `step_idx > 0` silently dropped that share,
/// leaking up to a full coupon of PV for any callable bond valued within
/// one tree step of a coupon date.
#[test]
fn coupon_inside_first_time_step_books_share_at_step_zero() {
    let as_of = date!(2025 - 01 - 01);
    let call_date = date!(2031 - 06 - 15);
    let maturity = date!(2035 - 01 - 01);
    // 10y with 9 steps -> dt ≈ 1.11y, so the first annual coupon (t ≈ 1.0)
    // has raw index ≈ 0.9 ∈ (0, 1) and splits across steps 0 and 1.
    let tree_steps = 9;
    let mut bond = Bond::fixed(
        "FIRST-STEP-COUPON",
        Money::from((1_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.cashflow_spec =
        CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    bond.quote_compounding =
        finstack_quant_core::math::Compounding::Periodic(std::num::NonZeroU32::MIN);
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: call_date,
            end: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.60)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
    let day_count = discount_curve.day_count();
    let time_to_maturity = day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("time to maturity");
    let dt = time_to_maturity / tree_steps as f64;

    let flows = bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("cashflows");
    // Test premise: at least one cashflow lands strictly inside the first step.
    let has_first_step_coupon = flows.iter().any(|(date, _)| {
        if *date <= as_of {
            return false;
        }
        let tf = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .unwrap_or(0.0);
        let raw = (tf / time_to_maturity) * tree_steps as f64;
        raw > 1e-6 && raw < 1.0 - 1e-6
    });
    assert!(
        has_first_step_coupon,
        "test premise: a coupon must fall inside the first time step"
    );

    let valuator =
        BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");

    // The (1 - weight) share of the first coupon must be booked at step 0.
    assert!(
        valuator.cashflow_vec[0] > 0.0,
        "step 0 must receive the lower-step share of a first-step coupon, got {}",
        valuator.cashflow_vec[0]
    );

    // PV-preservation identity: mapped cashflows discounted at step times
    // must reproduce the raw cashflows discounted at their true times.
    // Under the old `step_idx > 0` guard the first coupon's step-0 share
    // vanished and this identity failed by ~0.1 coupon.
    let mut pv_mapped = 0.0;
    for (step, amount) in valuator.cashflow_vec.iter().enumerate() {
        pv_mapped += amount * discount_curve.df(step as f64 * dt).expect("df");
    }
    let mut pv_true = 0.0;
    for (date, amount) in &flows {
        if *date <= as_of {
            continue;
        }
        let tf = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .expect("year fraction");
        pv_true += amount.amount() * discount_curve.df(tf).expect("df");
    }
    assert!(
        (pv_mapped - pv_true).abs() < 1e-9,
        "PV must be preserved when a coupon falls inside the first time step: \
         pv_mapped={pv_mapped}, pv_true={pv_true}, diff={}",
        (pv_mapped - pv_true).abs()
    );
}

#[test]
fn exercise_date_cashflows_are_adjusted_to_snapped_step_time() {
    let as_of = date!(2025 - 01 - 01);
    let call_date = date!(2027 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let tree_steps = 7;
    let mut bond = Bond::fixed(
        "OFF-GRID-CALL-CF",
        Money::from((1_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.cashflow_spec =
        CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    bond.quote_compounding =
        finstack_quant_core::math::Compounding::Periodic(std::num::NonZeroU32::MIN);
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: call_date,
            end: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.55)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    let discount_curve = market.get_discount("USD-OIS").expect("discount curve");
    let day_count = discount_curve.day_count();
    let time_to_maturity = day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("time to maturity");
    let event_time = day_count
        .year_fraction(as_of, call_date, DayCountContext::default())
        .expect("call time");
    let step = map_date_to_step(
        as_of,
        call_date,
        maturity,
        tree_steps,
        day_count,
        DayCountContext::default(),
    )
    .expect("map call date")
    .clamp(1, tree_steps);
    let step_time = time_to_maturity / tree_steps as f64 * step as f64;
    assert!(
        (event_time - step_time).abs() > 1e-4,
        "test requires an off-grid exercise date"
    );

    let raw_exercise_date_cashflow = bond
        .pricing_dated_cashflows(&market, as_of)
        .expect("cashflows")
        .into_iter()
        .filter(|(date, _)| *date == call_date)
        .map(|(_, amount)| amount.amount())
        .sum::<f64>();
    assert!(
        raw_exercise_date_cashflow > 0.0,
        "test requires a coupon on the exercise date"
    );

    let valuator =
        BondValuator::new(bond, &market, as_of, time_to_maturity, tree_steps).expect("tree");
    let grid = (0..=tree_steps)
        .map(|index| index as f64 * time_to_maturity / tree_steps as f64)
        .collect::<Vec<_>>();
    let step_dfs = BondValuator::conditional_discount_factors_on_grid(
        as_of,
        maturity,
        day_count,
        &grid,
        discount_curve.as_ref(),
    )
    .expect("step discount factors");
    let expected = BondValuator::value_at_step_time(
        raw_exercise_date_cashflow,
        call_date,
        step,
        &step_dfs,
        discount_curve.as_ref(),
        as_of,
    )
    .expect("timing-adjusted cashflow");
    let actual = valuator.cashflow_vec[step];

    assert!(
        (actual - expected).abs() < 1e-10,
        "exercise-date cashflow should be valued consistently with call redemption timing: actual={actual}, expected={expected}, raw={raw_exercise_date_cashflow}"
    );
}
