use super::*;
use finstack_quant_models::credit::toggle_exercise::ThresholdDirection;
use finstack_quant_models::credit::{
    AssetDynamics, BarrierType, CreditStateVariable, DynamicRecoverySpec, EndogenousHazardSpec,
    MertonModel, ToggleExerciseModel,
};

fn test_merton() -> MertonModel {
    MertonModel::new_with_dynamics(
        200.0,
        0.25,
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton")
}

#[test]
fn config_accepts_recovery_boundaries_and_rejects_invalid_values() {
    for recovery in [0.0, 1.0] {
        let config = MertonMcConfig::new(test_merton(), recovery)
            .expect("boundary recovery should be valid");
        assert_eq!(config.default_recovery_rate, recovery);
    }

    for recovery in [-f64::EPSILON, 1.0 + f64::EPSILON, f64::NAN] {
        assert!(MertonMcConfig::new(test_merton(), recovery).is_err());
    }
}

#[test]
fn cash_bond_produces_positive_price() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
        "Price should be reasonable: got {}",
        result.clean_price_pct
    );
}

#[test]
fn pik_bond_produces_positive_price() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(5000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
        "Price should be reasonable: got {}",
        result.clean_price_pct
    );
}

#[test]
fn endogenous_hazard_lowers_pik_price() {
    let endo = EndogenousHazardSpec::power_law(0.06, 0.5, 2.5).expect("valid");
    let config_no = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let config_yes = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .endogenous_hazard(endo);
    let result_no = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_no, 0.04).expect("ok");
    let result_yes = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_yes, 0.04).expect("ok");
    assert!(
        result_yes.clean_price_pct <= result_no.clean_price_pct,
        "Endogenous hazard should lower or maintain PIK price: no={}, yes={}",
        result_no.clean_price_pct,
        result_yes.clean_price_pct
    );
}

#[test]
fn dynamic_recovery_lowers_pik_price() {
    let dyn_rec = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.10).expect("valid");
    let config_no = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let config_yes = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42)
        .dynamic_recovery(dyn_rec);
    let result_no = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_no, 0.04).expect("ok");
    let result_yes = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_yes, 0.04).expect("ok");
    assert!(
        result_yes.clean_price_pct <= result_no.clean_price_pct,
        "Dynamic recovery should lower or maintain PIK price: no={}, yes={}",
        result_no.clean_price_pct,
        result_yes.clean_price_pct
    );
}

#[test]
fn toggle_price_between_cash_and_pik() {
    let toggle = ToggleExerciseModel::threshold(
        CreditStateVariable::HazardRate,
        0.10,
        ThresholdDirection::Above,
    );
    let config_cash = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(10_000)
        .seed(42);
    let config_pik = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(10_000)
        .seed(42);
    let config_toggle = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Toggle))
        .num_paths(10_000)
        .seed(42)
        .toggle_model(toggle);
    let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
    let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
    let toggle_result =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_toggle, 0.04).expect("ok");
    let min_price = pik.clean_price_pct.min(cash.clean_price_pct) - 5.0;
    let max_price = pik.clean_price_pct.max(cash.clean_price_pct) + 5.0;
    assert!(
        toggle_result.clean_price_pct >= min_price && toggle_result.clean_price_pct <= max_price,
        "Toggle should be between cash and PIK: cash={}, pik={}, toggle={}",
        cash.clean_price_pct,
        pik.clean_price_pct,
        toggle_result.clean_price_pct
    );
}

/// The synthetic coupon schedule is anchored backward from maturity; the
/// valuation date sits inside the first period, whose coupon is still paid
/// in full (dirty valuation), and aligned maturities stay regular.
#[test]
fn coupon_schedule_handles_stub_and_aligned_maturities() {
    // 4.6y semi-annual: 10 coupons, the first 0.1y away and paying a full
    // half-year coupon; the last lands exactly at maturity.
    let sched = MertonMcEngine::coupon_schedule(4.6, 2);
    assert_eq!(sched.len(), 10);
    let (t_first, accrual_first) = sched[0];
    assert!((t_first - 0.1).abs() < 1e-9, "first coupon time: {t_first}");
    assert!(
        (accrual_first - 0.5).abs() < 1e-12,
        "first coupon accrual: {accrual_first}"
    );
    let (t_last, accrual_last) = sched[sched.len() - 1];
    assert!(
        (t_last - 4.6).abs() < 1e-12,
        "final coupon at maturity: {t_last}"
    );
    assert!((accrual_last - 0.5).abs() < 1e-12);

    // Aligned 5.0y semi-annual: 10 full coupons at 0.5, 1.0, …, 5.0.
    let aligned = MertonMcEngine::coupon_schedule(5.0, 2);
    assert_eq!(aligned.len(), 10);
    for (i, &(t, accrual)) in aligned.iter().enumerate() {
        assert!((t - 0.5 * (i + 1) as f64).abs() < 1e-9);
        assert!((accrual - 0.5).abs() < 1e-12);
    }
}

/// Dirty minus clean equals the accrued interest of the elapsed part of the
/// current period: 4.6y semi-annual 8% on 100 has 0.4y elapsed, so accrued
/// = 100 × 0.08 × 0.4 = 3.2 per 100.
#[test]
fn synthetic_schedule_clean_price_subtracts_accrued() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("valid config")
        .num_paths(200)
        .seed(3);
    let stub = MertonMcEngine::price(100.0, 0.08, 4.6, 2, &config, 0.04).expect("price");
    assert!((stub.dirty_price_pct - stub.clean_price_pct - 3.2).abs() < 1e-9);
    let aligned = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("price");
    assert!((aligned.dirty_price_pct - aligned.clean_price_pct).abs() < 1e-12);
}

/// M2.12: with default risk switched off (asset value far above the
/// barrier, negligible vol) the MC price of a stub-maturity bond must
/// reproduce the risk-free PV — i.e. every coupon in the risk-free leg is
/// reachable on the simulation grid and the grid ends exactly at
/// maturity. Before the fix, `round(maturity·frequency)` and a fixed
/// `dt = 1/steps_per_year` let the legs disagree on stub maturities.
#[test]
fn stub_maturity_mc_matches_risk_free_pv_without_default_risk() {
    let merton = MertonModel::new_with_dynamics(
        1.0e9, // asset value far above the barrier: no defaults
        1e-8,  // negligible asset vol: deterministic paths
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton");
    let config = MertonMcConfig::new(merton, 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(64)
        .seed(42);

    for maturity in [4.6, 4.8, 5.0] {
        let result =
            MertonMcEngine::price(100.0, 0.08, maturity, 2, &config, 0.04).expect("price succeeds");
        assert_eq!(
            result.path_statistics.default_rate, 0.0,
            "no defaults expected at maturity {maturity}"
        );
        // expected_loss = 1 − mean_mc_pv / risk_free_pv: with default
        // risk off, the two legs must agree (shared coupon schedule,
        // grid ending exactly at maturity).
        assert!(
            result.expected_loss.abs() < 1e-6,
            "default-free MC price must equal the risk-free PV at maturity \
             {maturity}: expected_loss = {}",
            result.expected_loss
        );
    }
}

#[test]
fn mc_is_deterministic_with_seed() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(1000)
        .seed(42);
    let r1 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    let r2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        (r1.clean_price_pct - r2.clean_price_pct).abs() < 1e-10,
        "Same seed should give same result"
    );
}

#[test]
fn path_statistics_reasonable() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(5000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.path_statistics.default_rate >= 0.0 && result.path_statistics.default_rate <= 1.0
    );
    assert!(
        result.path_statistics.avg_terminal_notional >= 100.0,
        "PIK should accrete notional, got {}",
        result.path_statistics.avg_terminal_notional
    );
    assert!(result.standard_error > 0.0);
}

// PikSchedule tests

#[test]
fn pik_schedule_mode_at_uniform() {
    let s = PikSchedule::Uniform(PikMode::Pik);
    assert_eq!(s.mode_at(0.0), PikMode::Pik);
    assert_eq!(s.mode_at(5.0), PikMode::Pik);
}

#[test]
fn pik_schedule_mode_at_stepped() {
    let s = PikSchedule::Stepped(vec![(0.0, PikMode::Pik), (2.0, PikMode::Cash)]);
    assert_eq!(s.mode_at(0.5), PikMode::Pik);
    assert_eq!(s.mode_at(1.9), PikMode::Pik);
    assert_eq!(s.mode_at(2.0), PikMode::Cash);
    assert_eq!(s.mode_at(5.0), PikMode::Cash);
}

#[test]
fn pik_schedule_stepped_toggle_then_cash() {
    let s = PikSchedule::Stepped(vec![(0.0, PikMode::Toggle), (3.0, PikMode::Cash)]);
    assert_eq!(s.mode_at(1.0), PikMode::Toggle);
    assert_eq!(s.mode_at(2.9), PikMode::Toggle);
    assert_eq!(s.mode_at(3.0), PikMode::Cash);
}

#[test]
fn split_schedule_prices_between_cash_and_pik() {
    let config_cash = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    let config_pik = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(5000)
        .seed(42);
    let config_split = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Split {
            cash_fraction: 0.5,
            pik_fraction: 0.5,
        }))
        .num_paths(5000)
        .seed(42);

    let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
    let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
    let split = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_split, 0.04).expect("ok");

    let lo = cash.clean_price_pct.min(pik.clean_price_pct) - 2.0;
    let hi = cash.clean_price_pct.max(pik.clean_price_pct) + 2.0;
    assert!(
        split.clean_price_pct >= lo && split.clean_price_pct <= hi,
        "50/50 split should be between cash ({}) and PIK ({}), got {}",
        cash.clean_price_pct,
        pik.clean_price_pct,
        split.clean_price_pct
    );
}

#[test]
fn stepped_schedule_pik_then_cash() {
    // PIK for first 2 years, then cash for remaining 3 years.
    // Should be between full cash and full PIK.
    let config_cash = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    let config_pik = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
        .num_paths(5000)
        .seed(42);
    let config_step = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Stepped(vec![
            (0.0, PikMode::Pik),
            (2.0, PikMode::Cash),
        ]))
        .num_paths(5000)
        .seed(42);

    let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
    let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
    let step = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_step, 0.04).expect("ok");

    let lo = cash.clean_price_pct.min(pik.clean_price_pct) - 2.0;
    let hi = cash.clean_price_pct.max(pik.clean_price_pct) + 2.0;
    assert!(
        step.clean_price_pct >= lo && step.clean_price_pct <= hi,
        "Stepped PIK→cash should be between full cash ({}) and full PIK ({}), got {}",
        cash.clean_price_pct,
        pik.clean_price_pct,
        step.clean_price_pct
    );
    assert!(
        step.average_pik_fraction > 0.0 && step.average_pik_fraction < 1.0,
        "Stepped schedule should have partial PIK fraction, got {}",
        step.average_pik_fraction
    );
}

#[test]
fn toggle_window_then_cash() {
    // Toggle for first 3 years, mandatory cash after.
    let toggle = ToggleExerciseModel::threshold(
        CreditStateVariable::HazardRate,
        0.10,
        ThresholdDirection::Above,
    );
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Stepped(vec![
            (0.0, PikMode::Toggle),
            (3.0, PikMode::Cash),
        ]))
        .toggle_model(toggle)
        .num_paths(5000)
        .seed(42);

    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
        "Toggle window price should be reasonable: {}",
        result.clean_price_pct
    );
}

#[test]
fn toggle_without_model_falls_back_to_cash() {
    // PikMode::Toggle but no toggle_model → should behave like cash
    let config_toggle_no_model = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Toggle))
        .num_paths(5000)
        .seed(42);
    let config_cash = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);

    let toggle_result =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_toggle_no_model, 0.04).expect("ok");
    let cash_result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");

    assert!(
        (toggle_result.clean_price_pct - cash_result.clean_price_pct).abs() < 1e-10,
        "Toggle without model should equal cash: toggle={}, cash={}",
        toggle_result.clean_price_pct,
        cash_result.clean_price_pct,
    );
}

#[test]
fn default_pik_schedule_is_cash() {
    let config = MertonMcConfig::new(test_merton(), 0.40).expect("0.40 recovery should be valid");
    assert!(
        matches!(config.pik_schedule, PikSchedule::Uniform(PikMode::Cash)),
        "Default pik_schedule should be Uniform(Cash)"
    );
}

// Brownian-bridge crossing tests

#[test]
fn brownian_bridge_increases_default_rate() {
    let config_discrete = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(10_000)
        .seed(42)
        .barrier_crossing(BarrierCrossing::Discrete);
    let config_bridge = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(10_000)
        .seed(42)
        .barrier_crossing(BarrierCrossing::BrownianBridge);

    let result_discrete =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_discrete, 0.04).expect("ok");
    let result_bridge =
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_bridge, 0.04).expect("ok");

    assert!(
        result_bridge.path_statistics.default_rate >= result_discrete.path_statistics.default_rate,
        "Brownian-bridge should detect at least as many defaults as discrete: bb={}, discrete={}",
        result_bridge.path_statistics.default_rate,
        result_discrete.path_statistics.default_rate
    );
}

#[test]
fn brownian_bridge_is_deterministic() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(2000)
        .seed(99)
        .barrier_crossing(BarrierCrossing::BrownianBridge);
    let r1 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    let r2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        (r1.clean_price_pct - r2.clean_price_pct).abs() < 1e-10,
        "Same seed + bridge should give same result"
    );
}

#[test]
fn terminal_barrier_only_defaults_at_maturity() {
    let merton_terminal = MertonModel::new(200.0, 0.25, 100.0, 0.04).expect("valid");
    let config = MertonMcConfig::new(merton_terminal, 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    assert_eq!(config.barrier_crossing, BarrierCrossing::Discrete);

    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    if result.path_statistics.default_rate > 0.0 {
        let expected_default_time = 5.0;
        assert!(
            (result.path_statistics.avg_default_time - expected_default_time).abs() < 0.5,
            "Terminal barrier defaults should only occur near maturity, got avg_default_time={}",
            result.path_statistics.avg_default_time
        );
    }
}

#[test]
fn first_passage_default_config_uses_brownian_bridge() {
    let config = MertonMcConfig::new(test_merton(), 0.40).expect("0.40 recovery should be valid");
    assert_eq!(
        config.barrier_crossing,
        BarrierCrossing::BrownianBridge,
        "FirstPassage should default to BrownianBridge"
    );
}

// Validation tests

#[test]
fn non_gbm_dynamics_rejected() {
    let merton_jd = MertonModel::new_with_dynamics(
        200.0,
        0.25,
        100.0,
        0.04,
        0.0,
        BarrierType::Terminal,
        AssetDynamics::JumpDiffusion {
            jump_intensity: 0.5,
            jump_mean: -0.05,
            jump_vol: 0.10,
        },
    )
    .expect("valid");
    let config = MertonMcConfig::new(merton_jd, 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(100)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
    assert!(result.is_err(), "JumpDiffusion should be rejected");
}

#[test]
fn invalid_split_fractions_rejected() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Split {
            cash_fraction: 0.6,
            pik_fraction: 0.6,
        }))
        .num_paths(100)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
    assert!(result.is_err(), "Split fractions > 1.0 should be rejected");
}

#[test]
fn negative_split_fractions_rejected() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Split {
            cash_fraction: -0.1,
            pik_fraction: 1.1,
        }))
        .num_paths(100)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
    assert!(
        result.is_err(),
        "Negative split fractions should be rejected"
    );
}

#[test]
fn unsorted_stepped_schedule_rejected() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Stepped(vec![
            (2.0, PikMode::Cash),
            (0.0, PikMode::Pik),
        ]))
        .num_paths(100)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
    assert!(
        result.is_err(),
        "Out-of-order Stepped times must be rejected"
    );

    let dup = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Stepped(vec![
            (1.0, PikMode::Pik),
            (1.0, PikMode::Cash),
        ]))
        .num_paths(100)
        .seed(42);
    assert!(
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &dup, 0.04).is_err(),
        "Duplicate Stepped times must be rejected"
    );
}

#[test]
fn single_path_rejected() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(1)
        .seed(42);
    assert!(
        MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).is_err(),
        "num_paths < 2 must be rejected"
    );
}

/// Antithetic SE must be computed over pair averages, not individual
/// legs: pairs are negatively correlated, so the pair-based SE differs
/// from the naive 2N-independent-legs SE (and is typically smaller).
#[test]
fn antithetic_se_uses_pair_averages() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(10_000)
        .antithetic(true)
        .seed(7);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");

    // Naive SE treating all legs as i.i.d.
    let naive_se = result.unexpected_loss * 100.0 / (result.num_paths as f64).sqrt();
    assert!(
        result.standard_error > 0.0 && (result.standard_error - naive_se).abs() > 1e-12,
        "pair-aware SE ({}) should differ from naive per-leg SE ({naive_se})",
        result.standard_error
    );
}

/// The effective spread must be solved on the same discount basis as the
/// MC PV: with term-structure DFs set, a default-free bond must imply a
/// ~zero spread even when the curve shape differs from the flat rate.
#[test]
fn effective_spread_zero_for_default_free_bond_on_term_structure_basis() {
    let merton = MertonModel::new_with_dynamics(
        1.0e9,
        1e-8,
        100.0,
        0.04,
        0.0,
        BarrierType::FirstPassage {
            barrier_growth_rate: 0.0,
        },
        AssetDynamics::GeometricBrownian,
    )
    .expect("valid merton");
    let steep_dfs: Vec<(f64, f64)> = (1..=60)
        .map(|i| {
            let t = i as f64 / 12.0;
            let r = 0.05 - 0.002 * t;
            (t, (-r * t).exp())
        })
        .collect();
    let config = MertonMcConfig::new(merton, 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(64)
        .seed(42)
        .cashflow_dfs(steep_dfs);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.effective_spread_bp.abs() < 0.1,
        "default-free bond must imply ~zero spread on the curve basis, got {} bp",
        result.effective_spread_bp
    );
}

#[test]
fn valid_split_fractions_accepted() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .pik_schedule(PikSchedule::Uniform(PikMode::Split {
            cash_fraction: 0.3,
            pik_fraction: 0.7,
        }))
        .num_paths(1000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
    assert!(result.is_ok(), "Valid 30/70 split should be accepted");
}

// Term-structure discounting tests

#[test]
fn cashflow_dfs_overrides_flat_rate() {
    let flat_config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(2000)
        .seed(42);
    let flat = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &flat_config, 0.04).expect("ok");

    // Build steeper curve DFs (higher short rates, lower long rates)
    let steep_dfs: Vec<(f64, f64)> = (1..=60)
        .map(|i| {
            let t = i as f64 / 12.0;
            let r = 0.05 - 0.002 * t; // inverted for visible difference
            (t, (-r * t).exp())
        })
        .collect();
    let ts_config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(2000)
        .seed(42)
        .cashflow_dfs(steep_dfs);
    let ts = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &ts_config, 0.04).expect("ok");

    assert!(
        (flat.clean_price_pct - ts.clean_price_pct).abs() > 0.01,
        "Term-structure DFs should produce a different price: flat={}, ts={}",
        flat.clean_price_pct,
        ts.clean_price_pct
    );
}

// Spread solver test

#[test]
fn effective_spread_positive_for_risky_bond() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.effective_spread_bp > 0.0,
        "Risky bond should have positive effective spread, got {}",
        result.effective_spread_bp
    );
}

#[test]
fn standard_error_in_pct_of_par() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(5000)
        .seed(42);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        result.standard_error > 0.001 && result.standard_error < 10.0,
        "SE in pct-of-par should be small but positive: got {}",
        result.standard_error
    );
}

// Antithetic complementary-uniform regression guard (W15)
//
// The antithetic path uses sign-flipped normals.  Its Brownian-bridge
// barrier-crossing test must use the *complementary* uniform `1 - u`
// (where the base path uses `u`) so that the two legs remain negatively
// correlated in their default decisions.  Before this fix both legs used
// the same raw `uniforms[step]`, which partially defeated variance
// reduction and produced a subtly biased `default_rate`.
//
// This test pins the *corrected* `default_rate` and `clean_price_pct` for
// a fixed seed.  It is a deterministic value-pinning guard: if the
// complementary-uniform logic is regressed the output changes measurably
// and this test will fail.
#[test]
fn antithetic_bridge_uses_complementary_uniform() {
    let config = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(10_000)
        .antithetic(true)
        .seed(777)
        .barrier_crossing(BarrierCrossing::BrownianBridge);
    let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");

    // Pinned post-fix values (seed=777, 10_000 paths, antithetic=true).
    // The pre-fix (biased) values were default_rate=0.1889 and
    // clean_price_pct≈106.197, confirming the fix has an observable effect.
    let expected_default_rate = 0.1882_f64;
    let expected_price = 106.109_28_f64;

    assert!(
        (result.path_statistics.default_rate - expected_default_rate).abs() < 1e-9,
        "Antithetic default_rate should be {expected_default_rate} (complementary uniform), \
         got {}",
        result.path_statistics.default_rate
    );
    assert!(
        (result.clean_price_pct - expected_price).abs() < 1e-3,
        "clean_price_pct should be ≈{expected_price} (complementary uniform), got {}",
        result.clean_price_pct
    );

    // Also verify determinism is preserved with the fixed seed.
    let result2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
    assert!(
        (result.clean_price_pct - result2.clean_price_pct).abs() < 1e-10,
        "Same seed must give identical results after fix"
    );
}

#[test]
fn merton_mc_config_roundtrips_via_pricing_overrides_json() {
    // Ensures the canonical nested model configuration can deserialize.
    use crate::instruments::InstrumentPricingOverrides;
    let cfg = MertonMcConfig::new(test_merton(), 0.40)
        .expect("0.40 recovery should be valid")
        .num_paths(64)
        .seed(7)
        .pik_schedule(PikSchedule::Uniform(PikMode::Cash));
    let mut ov = InstrumentPricingOverrides::default();
    ov = ov.with_merton_mc(cfg);
    let json = serde_json::to_string(&ov).expect("ser");
    let back: InstrumentPricingOverrides = serde_json::from_str(&json).expect("de");
    assert!(back.model_config.merton_mc_config.is_some());
    // The inner config should roundtrip key fields
    let restored = &back
        .model_config
        .merton_mc_config
        .as_ref()
        .expect("merton_mc_config should be populated")
        .0;
    assert_eq!(restored.num_paths, 64);
    assert_eq!(restored.seed, 7);

    let notebook_shape = serde_json::json!({
        "model_config": {
            "merton_mc_config": {
                "merton": {
                    "asset_value": 200.0,
                    "asset_vol": 0.25,
                    "debt_barrier": 100.0,
                    "risk_free_rate": 0.04,
                    "payout_rate": 0.0,
                    "barrier_type": {"first_passage": {"barrier_growth_rate": 0.0}},
                    "dynamics": "geometric_brownian"
                },
                "pik_schedule": {"uniform": "pik"},
                "num_paths": 2000,
                "seed": 42,
                "antithetic": true,
                "time_steps_per_year": 50,
                "default_recovery_rate": 0.40,
                "barrier_crossing": "brownian_bridge"
            }
        }
    });
    let from_notebook: InstrumentPricingOverrides =
        serde_json::from_value(notebook_shape).expect("notebook merton_mc_config shape");
    let restored = &from_notebook
        .model_config
        .merton_mc_config
        .as_ref()
        .expect("merton_mc_config should be populated")
        .0;
    assert_eq!(restored.num_paths, 2000);
    assert_eq!(restored.seed, 42);
    assert!(matches!(
        restored.pik_schedule,
        PikSchedule::Uniform(PikMode::Pik)
    ));
}
