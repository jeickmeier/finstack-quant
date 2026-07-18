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

// ---------------------------------------------------------------------------
// Tranche loss statistics
// ---------------------------------------------------------------------------

/// Five-path pool distribution used by the tranche cases below.
///
/// Pool notional is 100, so the path loss fractions are
/// `[0.00, 0.01, 0.02, 0.05, 0.10]`. At confidence 0.75 the nearest-rank VaR
/// index is `ceil(0.75 * 5) - 1 = 3`, i.e. the fourth-worst observation.
fn tranche_pool() -> PortfolioLossResult {
    PortfolioLossResult::from_losses(vec![0.0, 1.0, 2.0, 5.0, 10.0], 0.75)
        .expect("valid loss distribution")
}

const TRANCHE_POOL_NOTIONAL: f64 = 100.0;
const TOL: f64 = 1e-12;

#[test]
fn from_losses_records_the_confidence_used_for_tail_statistics() {
    assert_eq!(tranche_pool().confidence, 0.75);
}

#[test]
fn equity_tranche_absorbs_the_first_losses_and_writes_down_fully() {
    let stats = tranche_pool()
        .tranche_loss_statistics(0.0, 0.03, TRANCHE_POOL_NOTIONAL)
        .expect("valid 0-3% equity tranche");

    assert_eq!(stats.attachment, 0.0);
    assert_eq!(stats.detachment, 0.03);
    assert!((stats.tranche_notional - 3.0).abs() < TOL);

    // Per-path tranche fractions: [0, 1/3, 2/3, 1, 1].
    assert!((stats.expected_loss_fraction - 0.6).abs() < 1e-9);
    assert!((stats.expected_loss_amount - 1.8).abs() < 1e-9);
    // VaR at rank 3 of the sorted fractions is a full write-down, so ES is too.
    assert!((stats.var_fraction - 1.0).abs() < TOL);
    assert!((stats.var_amount - 3.0).abs() < TOL);
    assert!((stats.expected_shortfall_fraction - 1.0).abs() < TOL);
    assert!((stats.expected_shortfall_amount - 3.0).abs() < TOL);
    // Four of five paths lose something; two reach the 3% detachment.
    assert!((stats.prob_attachment_breached - 0.8).abs() < TOL);
    assert!((stats.prob_full_writedown - 0.4).abs() < TOL);
}

#[test]
fn mezzanine_tranche_is_protected_by_subordination() {
    let pool = tranche_pool();
    let equity = pool
        .tranche_loss_statistics(0.0, 0.03, TRANCHE_POOL_NOTIONAL)
        .expect("valid equity tranche");
    let mezzanine = pool
        .tranche_loss_statistics(0.03, 0.10, TRANCHE_POOL_NOTIONAL)
        .expect("valid 3-10% mezzanine tranche");

    // Only the 5% and 10% paths pierce 3%; only the 10% path exhausts it.
    let expected = (0.02 / 0.07 + 1.0) / 5.0;
    assert!((mezzanine.expected_loss_fraction - expected).abs() < 1e-12);
    assert!((mezzanine.tranche_notional - 7.0).abs() < 1e-12);
    assert!((mezzanine.expected_loss_amount - expected * 7.0).abs() < 1e-12);
    assert!((mezzanine.prob_attachment_breached - 0.4).abs() < TOL);
    assert!((mezzanine.prob_full_writedown - 0.2).abs() < TOL);

    // Subordination: the junior tranche can never lose a smaller share.
    assert!(equity.expected_loss_fraction > mezzanine.expected_loss_fraction);
}

#[test]
fn senior_tranche_above_the_worst_loss_is_untouched() {
    // Boundary case L <= A for every path: the worst path loses exactly 10%.
    let stats = tranche_pool()
        .tranche_loss_statistics(0.10, 0.30, TRANCHE_POOL_NOTIONAL)
        .expect("valid 10-30% senior tranche");

    assert_eq!(stats.expected_loss_fraction, 0.0);
    assert_eq!(stats.expected_loss_amount, 0.0);
    assert_eq!(stats.var_fraction, 0.0);
    assert_eq!(stats.expected_shortfall_fraction, 0.0);
    assert_eq!(stats.prob_attachment_breached, 0.0);
    assert_eq!(stats.prob_full_writedown, 0.0);
}

#[test]
fn thin_equity_tranche_below_every_positive_loss_is_fully_written_down() {
    // Boundary case L >= D on every path that loses anything at all.
    let stats = tranche_pool()
        .tranche_loss_statistics(0.0, 0.005, TRANCHE_POOL_NOTIONAL)
        .expect("valid 0-0.5% first-loss tranche");

    assert!((stats.expected_loss_fraction - 0.8).abs() < TOL);
    assert!((stats.var_fraction - 1.0).abs() < TOL);
    assert!((stats.expected_shortfall_fraction - 1.0).abs() < TOL);
    assert!((stats.prob_attachment_breached - 0.8).abs() < TOL);
    assert!((stats.prob_full_writedown - 0.8).abs() < TOL);
}

#[test]
fn tranche_statistics_reject_invalid_boundaries_and_pool_notional() {
    let pool = tranche_pool();

    // attachment >= detachment
    assert!(pool.tranche_loss_statistics(0.10, 0.10, 100.0).is_err());
    assert!(pool.tranche_loss_statistics(0.20, 0.10, 100.0).is_err());
    // outside [0, 1]
    for (attachment, detachment) in [(-0.01, 0.10), (0.10, 1.01), (f64::NAN, 0.10)] {
        assert!(pool
            .tranche_loss_statistics(attachment, detachment, 100.0)
            .is_err());
    }
    // non-positive or non-finite pool notional
    for notional in [0.0, -100.0, f64::NAN, f64::INFINITY] {
        assert!(pool.tranche_loss_statistics(0.0, 0.03, notional).is_err());
    }
}
