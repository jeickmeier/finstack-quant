//! Behavioral tests for portfolio credit-loss simulation.

use finstack_quant_valuations::correlation::{
    simulate_portfolio_loss, simulate_portfolio_loss_serial, simulate_portfolio_loss_with_recovery,
    CopulaSpec, CreditExposure, PortfolioLossConfig, PortfolioLossResult, RecoverySpec,
    MAX_PORTFOLIO_LOSS_PATHS,
};

fn exposure(
    id: &str,
    notional: f64,
    default_probability: f64,
    lgd: f64,
    factor_loadings: Vec<f64>,
) -> CreditExposure {
    CreditExposure {
        id: id.to_string(),
        notional,
        default_probability,
        lgd,
        factor_loadings,
    }
}

fn config(copula: CopulaSpec) -> PortfolioLossConfig {
    PortfolioLossConfig {
        num_paths: 4_096,
        seed: 42,
        confidence: 0.99,
        copula,
    }
}

#[test]
fn one_name_probability_limits_are_exact_for_gaussian_and_student_t() {
    for copula in [
        CopulaSpec::gaussian(),
        CopulaSpec::student_t(5.0).expect("valid t copula"),
    ] {
        let never = simulate_portfolio_loss(
            &[exposure("never", 100.0, 0.0, 0.6, vec![0.4])],
            &config(copula.clone()),
        )
        .expect("valid simulation");
        assert!(never.losses.iter().all(|loss| *loss == 0.0));
        assert_eq!(never.expected_loss, 0.0);
        assert_eq!(never.var, 0.0);
        assert_eq!(never.expected_shortfall, 0.0);

        let certain = simulate_portfolio_loss(
            &[exposure("certain", 100.0, 1.0, 0.6, vec![0.4])],
            &config(copula),
        )
        .expect("valid simulation");
        assert!(certain.losses.iter().all(|loss| *loss == 60.0));
        assert_eq!(certain.expected_loss, 60.0);
        assert_eq!(certain.var, 60.0);
        assert_eq!(certain.expected_shortfall, 60.0);
    }
}

#[test]
fn path_indexed_streams_make_repeated_and_serial_parallel_results_identical() {
    let exposures = vec![
        exposure("a", 100.0, 0.02, 0.6, vec![0.35, 0.10]),
        exposure("b", 80.0, 0.08, 0.5, vec![0.20, 0.25]),
        exposure("c", 120.0, 0.04, 0.7, vec![0.30, -0.15]),
    ];
    let cfg = config(CopulaSpec::student_t(6.0).expect("valid t copula"));

    let first = simulate_portfolio_loss(&exposures, &cfg).expect("parallel simulation");
    let second = simulate_portfolio_loss(&exposures, &cfg).expect("repeat simulation");
    let serial = simulate_portfolio_loss_serial(&exposures, &cfg).expect("serial simulation");

    assert_eq!(first, second);
    assert_eq!(first, serial);
}

#[test]
fn constant_recovery_model_matches_equivalent_exposure_lgd_bit_for_bit() {
    let exposures = vec![
        exposure("a", 100.0, 0.10, 0.6, vec![0.3]),
        exposure("b", 100.0, 0.10, 0.6, vec![0.3]),
    ];
    let cfg = config(CopulaSpec::gaussian());
    let direct = simulate_portfolio_loss(&exposures, &cfg).expect("direct LGD simulation");
    let recovery = RecoverySpec::constant(0.4).expect("valid recovery");
    let modeled = simulate_portfolio_loss_with_recovery(&exposures, &cfg, &recovery)
        .expect("recovery-model simulation");

    assert_eq!(direct, modeled);
}

#[test]
fn loss_positive_var_uses_nearest_rank_and_es_includes_the_var_observation() {
    let result = PortfolioLossResult::from_losses(vec![20.0, 0.0, 10.0, 0.0], 0.75)
        .expect("valid loss distribution");

    assert_eq!(result.expected_loss, 7.5);
    assert_eq!(result.var, 10.0);
    assert_eq!(result.expected_shortfall, 15.0);
    assert!(result.expected_shortfall >= result.var);
}

#[test]
fn finite_extreme_losses_produce_finite_mean_and_expected_shortfall() {
    let result = PortfolioLossResult::from_losses(vec![1.0e308, 1.0e308], 0.5)
        .expect("scaled aggregation should not overflow");

    assert_eq!(result.expected_loss, 1.0e308);
    assert_eq!(result.var, 1.0e308);
    assert_eq!(result.expected_shortfall, 1.0e308);
    assert!(result.expected_loss.is_finite());
    assert!(result.expected_shortfall.is_finite());
}

#[test]
fn simulation_rejects_path_loss_overflow() {
    let exposures = vec![
        exposure("huge_a", 1.0e308, 1.0, 1.0, vec![0.0]),
        exposure("huge_b", 1.0e308, 1.0, 1.0, vec![0.0]),
    ];
    let mut cfg = config(CopulaSpec::gaussian());
    cfg.num_paths = 1;

    let error = simulate_portfolio_loss(&exposures, &cfg).expect_err("path sum must overflow");
    assert!(error.to_string().contains("path loss overflow"));
}

#[test]
fn rounded_unit_norm_loadings_are_accepted_but_material_excess_is_rejected() {
    let loading = 0.5_f64.sqrt();
    let mut cfg = config(CopulaSpec::gaussian());
    cfg.num_paths = 8;

    let accepted = exposure("unit", 100.0, 0.05, 0.6, vec![loading, loading]);
    simulate_portfolio_loss(&[accepted], &cfg).expect("rounded unit norm should be accepted");

    let rejected = exposure("over", 100.0, 0.05, 0.6, vec![0.8, 0.8]);
    assert!(simulate_portfolio_loss(&[rejected], &cfg).is_err());
}

#[test]
fn simulation_rejects_duplicate_trimmed_exposure_ids() {
    let exposures = vec![
        exposure("duplicate", 100.0, 0.05, 0.6, vec![0.3]),
        exposure(" duplicate ", 100.0, 0.05, 0.6, vec![0.3]),
    ];
    let mut cfg = config(CopulaSpec::gaussian());
    cfg.num_paths = 1;

    let error = simulate_portfolio_loss(&exposures, &cfg).expect_err("duplicate ids must fail");
    assert_eq!(
        error.to_string(),
        "Validation error: duplicate credit exposure id after trimming: 'duplicate'"
    );
}

#[test]
fn simulation_rejects_invalid_paths_confidence_exposures_and_loadings() {
    let valid = exposure("ok", 100.0, 0.05, 0.6, vec![0.3]);
    let mut cfg = config(CopulaSpec::gaussian());

    cfg.num_paths = 0;
    assert!(simulate_portfolio_loss(std::slice::from_ref(&valid), &cfg).is_err());
    cfg.num_paths = MAX_PORTFOLIO_LOSS_PATHS + 1;
    let error = simulate_portfolio_loss(std::slice::from_ref(&valid), &cfg)
        .expect_err("path maximum must be enforced before allocation");
    assert!(error
        .to_string()
        .contains("portfolio loss num_paths must not exceed"));
    cfg.num_paths = 10;
    for confidence in [0.0, 1.0, f64::NAN, f64::INFINITY] {
        cfg.confidence = confidence;
        assert!(simulate_portfolio_loss(std::slice::from_ref(&valid), &cfg).is_err());
    }

    let invalid_exposures = [
        exposure("", 100.0, 0.05, 0.6, vec![0.3]),
        exposure("notional", -1.0, 0.05, 0.6, vec![0.3]),
        exposure("notional_nan", f64::NAN, 0.05, 0.6, vec![0.3]),
        exposure("pd", 100.0, 1.01, 0.6, vec![0.3]),
        exposure("lgd", 100.0, 0.05, -0.1, vec![0.3]),
        exposure("empty_loadings", 100.0, 0.05, 0.6, vec![]),
        exposure("loading_nan", 100.0, 0.05, 0.6, vec![f64::NAN]),
        exposure("loading_norm", 100.0, 0.05, 0.6, vec![0.9, 0.9]),
    ];
    cfg.confidence = 0.99;
    for invalid in invalid_exposures {
        assert!(simulate_portfolio_loss(&[invalid], &cfg).is_err());
    }

    let mismatched = vec![
        exposure("one", 100.0, 0.05, 0.6, vec![0.3]),
        exposure("two", 100.0, 0.05, 0.6, vec![0.2, 0.1]),
    ];
    assert!(simulate_portfolio_loss(&mismatched, &cfg).is_err());

    cfg.copula = CopulaSpec::random_factor_loading(0.1);
    assert!(simulate_portfolio_loss(&[valid], &cfg).is_err());
}
