use super::*;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::bond::{CallPutSchedule, CashflowSpec};
use crate::instruments::InstrumentPricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::trees::two_factor_rates_credit::{
    RatesCreditCalibrationTargets, RatesCreditConfig,
};
use rust_decimal::Decimal;

fn synthetic_daily_template(days: usize, with_daily_make_whole: bool) -> ReplayTemplate {
    let states = days + 1;
    let make_whole_claims = if with_daily_make_whole {
        (0..days)
            .map(|decision_index| MakeWholeClaim {
                exercise_step: decision_index,
                decision_index,
                basis_index: 0,
            })
            .collect()
    } else {
        Vec::new()
    };
    let make_whole_bases = if with_daily_make_whole {
        vec![MakeWholeBasis {
            interval_adjustments: vec![1.0; days],
        }]
    } else {
        Vec::new()
    };
    ReplayTemplate {
        times: (0..=days).map(|day| day as f64 / 365.0).collect(),
        step_dates: vec![None; states],
        static_cash: vec![Vec::new(); states],
        balance_events: vec![Vec::new(); states],
        floating: Vec::new(),
        floating_reset_ids: vec![Vec::new(); states],
        floating_accrual_start_ids: vec![Vec::new(); states],
        floating_payment_ids: vec![Vec::new(); states],
        static_accruals: Vec::new(),
        static_distributions: vec![Vec::new(); states],
        exercise: vec![Vec::new(); states],
        decision_steps: (0..=days).collect(),
        initial_outstanding: 100.0,
        redemption_step: Some(days),
        call_friction_cents: 0.0,
        recovery_rate: 0.4,
        return_floor: None,
        historical_distribution_cash: 0.0,
        historical_distribution_target_pv: 0.0,
        make_whole_claims,
        make_whole_bases,
        max_rate_history_steps: 0,
    }
}

fn stochastic_test_market(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.03_f64).exp()),
            (2.0, (-0.06_f64).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("test discount curve");
    let term_forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .knots([(0.0, 0.04), (2.0, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("term forward curve");
    let overnight_forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
        .base_date(as_of)
        .knots([(0.0, 0.04), (2.0, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("overnight forward curve");
    MarketContext::new()
        .insert(discount)
        .insert(term_forward)
        .insert(overnight_forward)
}

fn stochastic_test_bond(as_of: Date, maturity: Date) -> Bond {
    Bond::builder()
        .id("LSMC_OPTION_ORDERING".into())
        .notional(Money::from((100_i64, Currency::USD)))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.08, Tenor::quarterly(), DayCount::Act365F)
                .expect("finite coupon"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("test bond")
}

fn term_pik_test_bond(as_of: Date, maturity: Date) -> Bond {
    let mut bond = stochastic_test_bond(as_of, maturity);
    bond.id = "LSMC_TERM_PIK".into();
    bond.cashflow_spec = CashflowSpec::floating_with_reset_lag(
        CurveId::new("USD-SOFR-3M"),
        200.0,
        Tenor::semi_annual(),
        DayCount::Act360,
        2,
    )
    .expect("term floating specification");
    let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec else {
        unreachable!("floating constructor returned a non-floating specification");
    };
    spec.coupon_type = CouponType::Pik;
    spec.rate_spec.index_floor_bp = Some(Decimal::from(100));
    spec.rate_spec.all_in_cap_bp = Some(Decimal::from(800));
    spec.rate_spec.fallback = FloatingRateFallback::FixedRate(Decimal::new(4, 2));
    bond
}

fn overnight_pik_test_bond(as_of: Date, maturity: Date) -> Bond {
    let mut bond = stochastic_test_bond(as_of, maturity);
    bond.id = "LSMC_OVERNIGHT_PIK".into();
    bond.cashflow_spec = CashflowSpec::floating_with_reset_lag(
        CurveId::new("USD-SOFR"),
        200.0,
        Tenor::quarterly(),
        DayCount::Act360,
        0,
    )
    .expect("overnight floating specification");
    let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec else {
        unreachable!("floating constructor returned a non-floating specification");
    };
    spec.coupon_type = CouponType::Pik;
    spec.rate_spec.overnight_compounding =
        Some(crate::cashflow::builder::OvernightCompoundingMethod::CompoundedInArrears);
    spec.rate_spec.overnight_basis = Some(DayCount::Act360);
    spec.rate_spec.fixing_calendar_id = Some("weekends_only".to_string());
    spec.rate_spec.index_floor_bp = Some(Decimal::from(100));
    spec.rate_spec.index_cap_bp = Some(Decimal::from(300));
    spec.rate_spec.fallback = FloatingRateFallback::FixedRate(Decimal::new(4, 2));
    bond
}

fn stochastic_test_tree(days: usize) -> RatesCreditTree {
    let times = (0..=days).map(|day| day as f64 / 365.0).collect::<Vec<_>>();
    let discount_factors = times
        .iter()
        .map(|time| (-0.03 * time).exp())
        .collect::<Vec<_>>();
    let survival_probabilities = times
        .iter()
        .map(|time| (-0.02 * time).exp())
        .collect::<Vec<_>>();
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: days,
        rate_vol: 0.02,
        hazard_vol: 0.03,
        correlation: -0.25,
        rate_mean_reversion: 0.05,
        hazard_mean_reversion: 0.03,
    });
    tree.calibrate(&RatesCreditCalibrationTargets {
        times,
        discount_factors,
        survival_probabilities,
        recovery_rate: 0.4,
    })
    .expect("rates-credit calibration");
    tree
}

fn path_state(
    step: usize,
    short_rate: f64,
    hazard_rate: f64,
    discount_to_next: f64,
) -> RatesCreditPathState {
    RatesCreditPathState {
        step,
        time: step as f64,
        rate_node: 0,
        hazard_node: 0,
        short_rate,
        hazard_rate,
        discount_to_next,
        survival_to_next: (-hazard_rate).exp(),
        default_to_next: 1.0 - (-hazard_rate).exp(),
    }
}

#[test]
fn quadratic_interaction_policy_recovers_surface() {
    let mut features = Vec::new();
    let mut responses = Vec::new();
    for left in -3..=3 {
        for right in -3..=3 {
            let x = left as f64 / 2.0;
            let y = right as f64 / 3.0;
            let mut row = [0.0; FEATURE_COUNT];
            row[0] = x;
            row[1] = y;
            features.push(row);
            responses.push(4.0 + 2.0 * x - 3.0 * y + 0.5 * x * x + 1.25 * x * y);
        }
    }
    let policy = fit_policy(&features, &responses).expect("quadratic policy");
    let mut probe = [0.0; FEATURE_COUNT];
    probe[0] = 0.7;
    probe[1] = -0.4;
    let expected = 4.0 + 2.0 * 0.7 - 3.0 * -0.4 + 0.5 * 0.7_f64.powi(2) + 1.25 * 0.7 * -0.4;
    let actual = policy.predict(&probe).expect("finite prediction");
    assert!((actual - expected).abs() < 1.0e-10);
}

#[test]
fn bond_policy_cubic_refinement_recovers_nonquadratic_continuation() {
    let mut features = Vec::new();
    let mut responses = Vec::new();
    for left in -3..=3 {
        for right in -3..=3 {
            let x = left as f64 / 2.0;
            let y = right as f64 / 2.5;
            let mut row = [0.0; FEATURE_COUNT];
            row[0] = x;
            row[1] = y;
            features.push(row);
            responses.push(10.0 + 0.7 * x.powi(3) - 0.4 * x * y.powi(2) + 0.2 * y);
        }
    }
    let quadratic = RegressionPolicy::fit(&features, &responses, PolynomialDegree::Quadratic)
        .expect("quadratic bond policy");
    let cubic = RegressionPolicy::fit(&features, &responses, PolynomialDegree::Cubic)
        .expect("cubic bond policy refinement");
    let mut quadratic_error = 0.0;
    let mut cubic_error = 0.0;
    for (x, y) in [
        (0.35_f64, -0.55_f64),
        (0.9, 0.45),
        (-1.1, 0.2),
        (0.15, 1.05),
    ] {
        let mut row = [0.0; FEATURE_COUNT];
        row[0] = x;
        row[1] = y;
        let expected = 10.0 + 0.7 * x.powi(3) - 0.4 * x * y.powi(2) + 0.2 * y;
        quadratic_error +=
            (quadratic.predict(&row).expect("quadratic prediction") - expected).powi(2);
        cubic_error += (cubic.predict(&row).expect("cubic prediction") - expected).powi(2);
    }
    assert!(cubic_error < 1.0e-20, "cubic error={cubic_error}");
    assert!(
        cubic_error < quadratic_error * 1.0e-8,
        "cubic refinement must improve the same bond continuation sample: cubic={cubic_error}, quadratic={quadratic_error}"
    );
}

#[test]
fn constant_features_drop_to_intercept_without_realized_fallback() {
    let features = vec![[1.0; FEATURE_COUNT]; 8];
    let responses = vec![2.0, 4.0, 3.0, 5.0, 1.0, 7.0, 6.0, 4.0];
    let policy = fit_policy(&features, &responses).expect("intercept policy");
    assert_eq!(policy.num_terms(), 1);
    assert!((policy.predict(&[1.0; FEATURE_COUNT]).expect("prediction") - 4.0).abs() < 1e-12);
}

#[test]
fn put_precedes_call_and_incompatible_barriers_are_rejected_upstream() {
    let snapshot = DecisionSnapshot {
        step: 1,
        features: [0.0; FEATURE_COUNT],
        current_cash: 5.0,
        hold_redemption: 0.0,
        call: Some(105.0),
        put: Some(100.0),
        friction: 0.0,
        a_to_next: 1.0,
        b_to_next: 0.0,
    };
    assert_eq!(snapshot.exercise_value(90.0, 90.0), 100.0);
    assert_eq!(snapshot.exercise_value(110.0, 110.0), 105.0);
    assert_eq!(snapshot.exercise_value(102.0, 102.0), 102.0);
}

#[test]
fn antithetic_budget_counts_independent_estimators() {
    assert_eq!(simulated_path_count(20_000, true).expect("count"), 40_000);
    assert_eq!(simulated_path_count(20_000, false).expect("count"), 20_000);
}

#[test]
fn default_daily_training_budget_is_time_blocked_without_path_truncation() {
    let template = synthetic_daily_template(3_650, true);
    let physical_paths = simulated_path_count(DEFAULT_BOND_LSMC_PATHS, true).expect("count");
    let block_len = training_block_len(&template, physical_paths, 3_650)
        .expect("ten-year daily training plan must fit its memory bound");
    let boundaries = training_boundaries(&template, physical_paths).expect("boundaries");
    assert!(block_len > 0 && block_len < 3_650);
    assert_eq!(boundaries.first(), Some(&0));
    assert_eq!(boundaries.last(), Some(&3_650));
    assert_eq!(physical_paths, 40_000);
}

#[test]
fn stochastic_option_ordering_is_reproducible_with_exact_stage_counts() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 04 - 01);
    let exercise = time::macros::date!(2025 - 02 - 15);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(90);
    let bullet = stochastic_test_bond(as_of, maturity);
    let mut callable = bullet.clone();
    callable.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: exercise,
            end_date: exercise,
            price_pct_of_par: 80.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let mut puttable = bullet.clone();
    puttable.call_put = Some(CallPutSchedule {
        calls: Vec::new(),
        puts: vec![CallPut {
            start_date: exercise,
            end_date: exercise,
            price_pct_of_par: 120.0,
            make_whole: None,
        }],
    });
    let config = BondLsmcConfig {
        paths: 128,
        antithetic: true,
        seed: 0x5eed_cafe,
        oas_bp: 17.0,
        target_ci_half_width: None,
    };
    let bullet_result =
        price_bond_lsmc(&tree, &bullet, &market, as_of, &config, None).expect("bullet");
    let call_result =
        price_bond_lsmc(&tree, &callable, &market, as_of, &config, None).expect("callable");
    let repeated_call = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
        .expect("repeated callable");
    let put_result =
        price_bond_lsmc(&tree, &puttable, &market, as_of, &config, None).expect("puttable");
    let bullet_value = bullet_result.estimate.mean.amount();
    let call_value = call_result.estimate.mean.amount();
    let put_value = put_result.estimate.mean.amount();
    assert!(
        call_value < bullet_value,
        "issuer call must lower value: call={call_value}, bullet={bullet_value}"
    );
    assert!(
        bullet_value < put_value,
        "holder put must raise value: bullet={bullet_value}, put={put_value}"
    );
    assert_eq!(
        call_value,
        repeated_call.estimate.mean.amount(),
        "fixed seed and grid must reproduce the fitted-policy estimate"
    );
    assert_eq!(bullet_result.training_paths, 0);
    assert_eq!(call_result.training_paths, 128);
    assert_eq!(call_result.training_simulated_paths, 256);
    assert_eq!(call_result.pricing_simulated_paths, 256);
    assert_eq!(put_result.training_paths, 128);
}

#[test]
fn issue_date_initial_exchange_does_not_double_replay_outstanding() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 04 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(90);
    let bond = stochastic_test_bond(as_of, maturity);
    let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
    assert_eq!(template.initial_outstanding, 100.0);
    let config = BondLsmcConfig {
        paths: 1,
        antithetic: false,
        seed: 1,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let mut path = Vec::new();
    tree.sample_path_into(1, 0, false, &mut path)
        .expect("factor path");
    let replay = template
        .replay(&tree, &bond, &path, &config, None, None)
        .expect("product replay");
    assert_eq!(replay.snapshots[0].features[2], 100.0);
}

#[test]
fn term_reset_base_forward_uses_discount_curve_date_axis() {
    let curve_base = time::macros::date!(2024 - 01 - 01);
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2026 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(curve_base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.02_f64).exp()),
            (2.0, (-0.08_f64).exp()),
            (3.0, (-0.17_f64).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("shifted-base test discount curve");
    let term_forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .knots([(0.0, 0.04), (2.0, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("term forward curve");
    let market = MarketContext::new()
        .insert(discount.clone())
        .insert(term_forward);
    let tree = stochastic_test_tree(366);
    let bond = term_pik_test_bond(as_of, maturity);
    let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
    let coupon = template
        .floating
        .iter()
        .find(|coupon| coupon.reset_step > 0)
        .expect("future term coupon");
    let FloatingRateModel::Term(ObservedRateSource::Conditional(rate)) = &coupon.rate_model else {
        panic!("future reset should carry a conditional rate");
    };
    let FloatingRateObservation::Term { tenor_years, .. } = coupon.compiled.observation() else {
        panic!("expected term observation");
    };
    let observation_end_step = nearest_step(
        &template.times,
        template.times[coupon.reset_step] + tenor_years,
    );
    let reset_date = template.step_dates[coupon.reset_step].expect("reset grid date");
    let observation_end_date =
        template.step_dates[observation_end_step].expect("observation-end grid date");
    let expected_df = discount
        .df_between_dates(reset_date, observation_end_date)
        .expect("date-based forward discount factor");
    let expected_forward = (1.0 / expected_df - 1.0) / rate.accrual;
    assert!(
        (rate.base_discount_forward - expected_forward).abs() < 1.0e-14,
        "base forward must use discount-curve dates: actual={}, expected={expected_forward}",
        rate.base_discount_forward
    );

    let old_time_axis_df = discount.df(template.times[observation_end_step])
        / discount.df(template.times[coupon.reset_step]);
    let old_time_axis_forward = (1.0 / old_time_axis_df - 1.0) / rate.accrual;
    assert!(
        (rate.base_discount_forward - old_time_axis_forward).abs() > 0.02,
        "fixture must distinguish curve-relative time from the valuation grid"
    );
}

#[test]
fn lagged_term_reset_uses_curve_tenor_and_captures_post_pik_start_balance() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2026 - 01 - 01);
    let market = stochastic_test_market(as_of);
    // New Year's Day adjusts the final payment to 2026-01-02, so the
    // calibrated daily grid must extend one day past contractual maturity.
    let tree = stochastic_test_tree(366);
    let bullet = term_pik_test_bond(as_of, maturity);
    let template = ReplayTemplate::new(&tree, &bullet, &market, as_of).expect("template");
    let (coupon_id, coupon) = template
        .floating
        .iter()
        .enumerate()
        .find(|(_, coupon)| coupon.accrual_start_step > 0)
        .expect("future term coupon");
    assert!(coupon.reset_step < coupon.accrual_start_step);
    let FloatingRateObservation::Term { tenor_years, .. } = coupon.compiled.observation() else {
        panic!("expected term observation");
    };
    assert!((*tenor_years - 0.25).abs() < 1.0e-12);
    let times = tree.time_grid().expect("tree times");
    let observation_end = nearest_step(times, times[coupon.reset_step] + tenor_years);
    assert!(
        observation_end < coupon.payment_step,
        "3M curve tenor must not be replaced by the 6M coupon/payment interval"
    );
    let FloatingRateModel::Term(ObservedRateSource::Conditional(rate)) = &coupon.rate_model else {
        panic!("future reset should carry a conditional rate");
    };
    assert!((rate.accrual - (times[observation_end] - times[coupon.reset_step])).abs() < 1e-14);
    assert_eq!(
        rate.conditional_discount_factors,
        tree.conditional_discount_factors(
            coupon.reset_step,
            observation_end,
            *times.last().expect("terminal time")
        )
        .expect("conditional discount factors")
    );

    let schedule = bullet
        .full_cashflow_schedule(&market)
        .expect("deterministic schedule");
    let first_pik = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Pik)
        .min_by_key(|flow| flow.date)
        .expect("first PIK coupon")
        .amount
        .amount();
    let expected_start_balance = 100.0 + first_pik;
    let mut sampled = Vec::new();
    tree.sample_path_into(23, 0, false, &mut sampled)
        .expect("factor path");
    let config = BondLsmcConfig {
        paths: 64,
        antithetic: true,
        seed: 23,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let mut cursor = ReplayCursor::new(&template).expect("cursor");
    for step in 0..=coupon.accrual_start_step {
        cursor
            .advance_step(&template, &bullet, &sampled, &config, step)
            .expect("forward replay");
    }
    let captured = coupon
        .compiled
        .captured_notional(&cursor.floating[coupon_id])
        .expect("second coupon notional");
    assert!(
        (captured - expected_start_balance).abs() < 1.0e-10,
        "the prior PIK coupon must capitalize before the next coupon captures notional: captured={captured}, expected={expected_start_balance}"
    );

    let mut callable = bullet.clone();
    callable.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: time::macros::date!(2025 - 08 - 15),
            end_date: time::macros::date!(2025 - 08 - 15),
            price_pct_of_par: 80.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let bullet_value = price_bond_lsmc(&tree, &bullet, &market, as_of, &config, None)
        .expect("term PIK bullet")
        .estimate
        .mean
        .amount();
    let call_value = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
        .expect("callable term PIK bond")
        .estimate
        .mean
        .amount();
    assert!(
        call_value < bullet_value,
        "mid-period call must reduce stochastic term PIK value: call={call_value}, bullet={bullet_value}"
    );
}

#[test]
fn overnight_pik_replays_daily_caps_and_mid_accrual_exercise_state() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 07 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(181);
    let capped = overnight_pik_test_bond(as_of, maturity);
    let mut uncapped = capped.clone();
    let CashflowSpec::Floating(uncapped_spec) = &mut uncapped.cashflow_spec else {
        unreachable!("expected floating coupon");
    };
    uncapped_spec.rate_spec.index_cap_bp = None;
    let config = BondLsmcConfig {
        paths: 64,
        antithetic: true,
        seed: 0x0a11_ce55,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let capped_template =
        ReplayTemplate::new(&tree, &capped, &market, as_of).expect("capped template");
    let uncapped_template =
        ReplayTemplate::new(&tree, &uncapped, &market, as_of).expect("uncapped template");
    let mut sampled = Vec::new();
    tree.sample_path_into(41, 0, false, &mut sampled)
        .expect("factor path");
    let capped_path = capped_template
        .replay(&tree, &capped, &sampled, &config, None, None)
        .expect("capped overnight replay");
    let uncapped_path = uncapped_template
        .replay(&tree, &uncapped, &sampled, &config, None, None)
        .expect("uncapped overnight replay");
    assert!(
        capped_path
            .snapshots
            .last()
            .expect("terminal")
            .hold_redemption
            < uncapped_path
                .snapshots
                .last()
                .expect("terminal")
                .hold_redemption,
        "daily index cap must reduce PIK capitalization on the same overnight path"
    );

    let mut callable = capped.clone();
    let exercise = time::macros::date!(2025 - 02 - 14);
    callable.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: exercise,
            end_date: exercise,
            price_pct_of_par: 90.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let callable_template =
        ReplayTemplate::new(&tree, &callable, &market, as_of).expect("callable template");
    let callable_path = callable_template
        .replay(&tree, &callable, &sampled, &config, None, None)
        .expect("callable overnight replay");
    let exercise_step =
        exact_grid_step(tree.time_grid().expect("times"), as_of, exercise).expect("exercise step");
    let exercise_snapshot = callable_path
        .snapshots
        .iter()
        .find(|snapshot| snapshot.step == exercise_step)
        .expect("mid-accrual snapshot");
    assert!(
        exercise_snapshot.features[5] > 0.0,
        "overnight PIK must carry partial daily accrual into the exercise state"
    );
    let capped_value = price_bond_lsmc(&tree, &capped, &market, as_of, &config, None)
        .expect("overnight PIK bullet")
        .estimate
        .mean
        .amount();
    let call_value = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
        .expect("callable overnight PIK")
        .estimate
        .mean
        .amount();
    assert!(
        call_value < capped_value,
        "mid-accrual call must reduce stochastic overnight PIK value: call={call_value}, bullet={capped_value}"
    );
}

#[test]
fn fixed_budget_rejects_unmet_final_confidence_target() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 04 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(90);
    let bond = stochastic_test_bond(as_of, maturity);
    let config = BondLsmcConfig {
        paths: 32,
        antithetic: true,
        seed: 11,
        oas_bp: 0.0,
        target_ci_half_width: Some(f64::MIN_POSITIVE),
    };
    let error = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
        .expect_err("a finite stochastic sample cannot satisfy a near-zero CI target");
    assert!(
        error.to_string().contains("exhausted its fixed budget"),
        "unexpected error: {error}"
    );
}

#[test]
fn state_dependent_right_on_nonfirst_path_still_requires_a_policy() {
    let snapshot = |step, call| DecisionSnapshot {
        step,
        features: [0.0; FEATURE_COUNT],
        current_cash: 0.0,
        hold_redemption: 0.0,
        call,
        put: None,
        friction: 0.0,
        a_to_next: 1.0,
        b_to_next: 0.0,
    };
    // Path-major layout: path 0 has no right at either local decision;
    // path 1 has a state-dependent call only at local decision 0.
    let snapshots = vec![
        snapshot(1, None),
        snapshot(2, None),
        snapshot(1, Some(95.0)),
        snapshot(2, None),
    ];
    assert!(
        block_requires_policy(&snapshots, 2, 2, 0).expect("policy scan"),
        "a right on any training path must trigger regression"
    );
    assert!(!block_requires_policy(&snapshots, 2, 2, 1).expect("policy scan"));
}

#[test]
fn stochastic_maturity_make_whole_uses_contractual_floor_without_training_target() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 04 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(90);
    let mut bond = stochastic_test_bond(as_of, maturity);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: maturity,
            end_date: maturity,
            price_pct_of_par: 100.0,
            make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
                reference_curve_id: CurveId::new("USD-OIS"),
                spread_bp: 25.0,
            }),
        }],
        puts: Vec::new(),
    });
    let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
    assert!(
        template.make_whole_claims.is_empty(),
        "terminal make-whole has no later reference cashflows to regress"
    );
    let config = BondLsmcConfig {
        paths: 32,
        antithetic: true,
        seed: 91,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let result = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
        .expect("maturity make-whole price");
    assert!(result.estimate.mean.amount().is_finite());
    assert_eq!(result.make_whole_training_paths, 0);
}

#[test]
fn rolled_maturity_make_whole_retains_later_reference_cash() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    // Saturday contractual maturity rolls the holder payment to Monday.
    let maturity = time::macros::date!(2025 - 03 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(61);
    let mut bond = stochastic_test_bond(as_of, maturity);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: maturity,
            end_date: maturity,
            price_pct_of_par: 100.0,
            make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
                reference_curve_id: CurveId::new("USD-OIS"),
                spread_bp: 25.0,
            }),
        }],
        puts: Vec::new(),
    });
    let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
    assert_eq!(template.make_whole_claims.len(), 1);
    let redemption_step = template.redemption_step.expect("rolled redemption step");
    assert!(template.make_whole_claims[0].exercise_step < redemption_step);

    let config = BondLsmcConfig {
        paths: 16,
        antithetic: true,
        seed: 92,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let result = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
        .expect("rolled-maturity make-whole price");
    assert!(result.estimate.mean.amount().is_finite());
    assert_eq!(result.make_whole_training_paths, 16);
    assert_eq!(result.make_whole_training_simulated_paths, 32);
}

#[test]
fn option_bearing_custom_cashflows_are_rejected_before_replay() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 04 - 01);
    let market = stochastic_test_market(as_of);
    let tree = stochastic_test_tree(90);
    let mut bond = stochastic_test_bond(as_of, maturity);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: time::macros::date!(2025 - 02 - 15),
            end_date: time::macros::date!(2025 - 02 - 15),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    bond.custom_cashflows = Some(
        bond.full_cashflow_schedule(&market)
            .expect("canonical schedule"),
    );
    let config = BondLsmcConfig {
        paths: 8,
        antithetic: true,
        seed: 5,
        oas_bp: 0.0,
        target_ci_half_width: None,
    };
    let error = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
        .expect_err("custom optional schedule must be rejected");
    assert!(
        error.to_string().contains("custom cashflows"),
        "unexpected error: {error}"
    );
}

#[test]
fn two_time_blocks_match_full_replay_at_cash_amortization_exercise_boundary() {
    let as_of = time::macros::date!(2025 - 01 - 01);
    let maturity = time::macros::date!(2025 - 01 - 05);
    let bond = stochastic_test_bond(as_of, maturity);
    let tree = stochastic_test_tree(4);
    let mut template = synthetic_daily_template(4, false);
    template.step_dates = (0_i64..=4)
        .map(|offset| as_of.checked_add(Duration::days(offset)))
        .collect();
    template.decision_steps = vec![0, 2, 4];
    template.static_cash[2].push(CashEvent {
        amount_at_step: 15.0,
        event_minus_step: 0.0,
    });
    template.static_cash[4].push(CashEvent {
        amount_at_step: 4.0,
        event_minus_step: 0.0,
    });
    template.balance_events[2].push(BalanceEvent { delta: -10.0 });
    template.exercise[2].push(ExerciseDate {
        date: time::macros::date!(2025 - 01 - 03),
        calls: vec![ExerciseCall {
            price_pct_of_par: 90.0,
            make_whole: None,
        }],
        puts: Vec::new(),
        return_floor: false,
    });
    let config = BondLsmcConfig {
        paths: 1,
        antithetic: false,
        seed: 77,
        oas_bp: 125.0,
        target_ci_half_width: None,
    };
    let seed = 0x1234_5678;
    let mut full_path = Vec::new();
    tree.sample_path_into(seed, 0, false, &mut full_path)
        .expect("full factor path");
    let full = template
        .replay(&tree, &bond, &full_path, &config, None, None)
        .expect("full product replay");

    let initial_cursor = ReplayCursor::new(&template).expect("initial cursor");
    let checkpoint_zero = TrainingCheckpoint {
        factor: RatesCreditPathCheckpoint::from(&full_path[0]),
        product: initial_cursor.checkpoint(&template),
    };
    let mut boundary_cursor = ReplayCursor::new(&template).expect("boundary cursor");
    for step in 0..2 {
        boundary_cursor
            .advance_step(&template, &bond, &full_path, &config, step)
            .expect("prefix replay");
    }
    let checkpoint_two = TrainingCheckpoint {
        factor: RatesCreditPathCheckpoint::from(&full_path[2]),
        product: boundary_cursor.checkpoint(&template),
    };
    let mut segment = Vec::new();
    let low = template
        .replay_block(
            &tree,
            &bond,
            &config,
            seed,
            0,
            false,
            &checkpoint_zero,
            0,
            1,
            None,
            None,
            &mut segment,
        )
        .expect("low block");
    let high = template
        .replay_block(
            &tree,
            &bond,
            &config,
            seed,
            0,
            false,
            &checkpoint_two,
            1,
            2,
            None,
            None,
            &mut segment,
        )
        .expect("high block");
    let low_snapshot = &low.snapshots[0];
    let boundary_snapshot = &high.snapshots[0];
    let terminal = high.terminal.as_ref().expect("terminal snapshot");
    for (blocked, complete) in [
        (low_snapshot, &full.snapshots[0]),
        (boundary_snapshot, &full.snapshots[1]),
        (terminal, &full.snapshots[2]),
    ] {
        assert!((blocked.current_cash - complete.current_cash).abs() < 1.0e-14);
        assert!((blocked.hold_redemption - complete.hold_redemption).abs() < 1.0e-14);
        assert!((blocked.a_to_next - complete.a_to_next).abs() < 1.0e-14);
        assert!((blocked.b_to_next - complete.b_to_next).abs() < 1.0e-14);
        assert_eq!(blocked.call, complete.call);
    }
    assert_eq!(boundary_snapshot.current_cash, 15.0);
    assert_eq!(boundary_snapshot.features[2], 90.0);
    assert_eq!(boundary_snapshot.call, Some(81.0));

    let policy = fit_policy(
        &[full.snapshots[1].features, full.snapshots[1].features],
        &[100.0, 100.0],
    )
    .expect("boundary policy");
    let full_value =
        value_with_policy(&full, &[None, Some(policy.clone()), None]).expect("full replay value");
    let mut blocked_value = terminal.current_cash
        + terminal.exercise_value(terminal.hold_redemption, terminal.hold_redemption);
    let boundary_continuation =
        boundary_snapshot.a_to_next * blocked_value + boundary_snapshot.b_to_next;
    blocked_value = boundary_snapshot.current_cash
        + boundary_snapshot.exercise_value(
            policy
                .predict(&boundary_snapshot.features)
                .expect("policy prediction"),
            boundary_continuation,
        );
    let low_continuation = low_snapshot.a_to_next * blocked_value + low_snapshot.b_to_next;
    blocked_value =
        low_snapshot.current_cash + low_snapshot.exercise_value(low_continuation, low_continuation);
    assert!(
        (blocked_value - full_value).abs() < 1.0e-12,
        "blocked={blocked_value}, full={full_value}"
    );
}

#[test]
fn return_floor_subtracts_partial_accrual_before_adding_it_once() {
    let floor = ReturnFloorTemplate {
        kind: ReturnFloorKind::Moic(2.0),
        issue_price: 100.0,
        issue_date: time::macros::date!(2025 - 01 - 01),
        day_count: DayCount::Act365F,
    };
    let clean = floor
        .redemption(time::macros::date!(2025 - 07 - 01), 100.0, 50.0, 0.0, 10.0)
        .expect("floor redemption");
    assert_eq!(clean, 140.0);
    assert_eq!(clean + 10.0, 150.0);
}

#[test]
fn make_whole_reference_target_uses_rates_but_not_hazard_or_oas() {
    let basis = MakeWholeBasis {
        interval_adjustments: vec![1.0, 1.0],
    };
    let cash = [0.0, 10.0, 100.0];
    let low_hazard = [
        path_state(0, 0.02, 0.01, 0.9),
        path_state(1, 0.02, 0.01, 0.8),
        path_state(2, 0.02, 0.01, 1.0),
    ];
    let high_hazard = [
        path_state(0, 0.02, 0.50, 0.9),
        path_state(1, 0.02, 0.50, 0.8),
        path_state(2, 0.02, 0.50, 1.0),
    ];
    let lower_rates = [
        path_state(0, 0.01, 0.01, 0.95),
        path_state(1, 0.01, 0.01, 0.9),
        path_state(2, 0.01, 0.01, 1.0),
    ];
    let base = basis
        .realized_reference_value(0, &low_hazard, &cash)
        .expect("reference target");
    let hazard_changed = basis
        .realized_reference_value(0, &high_hazard, &cash)
        .expect("reference target");
    let rate_changed = basis
        .realized_reference_value(0, &lower_rates, &cash)
        .expect("reference target");
    assert!((base - 81.0).abs() < 1.0e-12);
    assert_eq!(base, hazard_changed);
    assert!(rate_changed > base);
}

#[test]
fn same_day_terminal_claim_has_no_next_day_credit_or_rate_exposure() {
    let origin = time::macros::date!(2025 - 01 - 15);
    assert_eq!(
        exact_grid_step(&[0.0, 1.0 / 365.0], origin, origin).expect("same-day grid step"),
        0
    );
    let path = PathRecord {
        snapshots: vec![
            DecisionSnapshot {
                step: 0,
                features: [0.0; FEATURE_COUNT],
                current_cash: 5.0,
                hold_redemption: 100.0,
                call: None,
                put: None,
                friction: 0.0,
                a_to_next: 0.01,
                b_to_next: 1_000.0,
            },
            DecisionSnapshot {
                step: 1,
                features: [0.0; FEATURE_COUNT],
                current_cash: 0.0,
                hold_redemption: 0.0,
                call: None,
                put: None,
                friction: 0.0,
                a_to_next: 0.0,
                b_to_next: 0.0,
            },
        ],
    };
    let value = value_with_policy(&path, &[None, None]).expect("same-day terminal value");
    assert_eq!(value, 105.0);

    let terminal = &path.snapshots[0];
    let holder_put = DecisionSnapshot {
        put: Some(110.0),
        ..terminal.clone()
    };
    assert_eq!(holder_put.exercise_value(-999.0, 999.0), 110.0);
    let issuer_call = DecisionSnapshot {
        call: Some(90.0),
        ..terminal.clone()
    };
    assert_eq!(issuer_call.exercise_value(-999.0, 999.0), 90.0);
}
