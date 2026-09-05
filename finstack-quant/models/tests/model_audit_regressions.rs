//! Financial invariants and invalid-input regressions from the models audit.

use finstack_quant_core::{currency::Currency, market_data::surfaces::VolSurface};
use finstack_quant_models::correlation::PortfolioLossResult;
use finstack_quant_models::fourier::cos::{bs_cos_price, BlackScholesCosParams};
use finstack_quant_models::monte_carlo::{
    pricer::{
        basis::PolynomialBasis,
        lsmc::{AmericanPut, LsmcConfig, LsmcPricer},
    },
    process::gbm::GbmProcess,
};
use finstack_quant_models::pde::{BlackScholesPde, Grid1D, PdeProblem1D, Solver1D};
use finstack_quant_models::volatility::arbitrage::{
    check_local_vol_density_grid, check_surface, check_surface_grid, ArbitrageCheckConfig,
};
use finstack_quant_models::{bs_price, OptionType};

#[test]
fn checked_black_scholes_rejects_invalid_inputs_without_masking_nan() {
    let valid = [100.0, 100.0, 0.05, 0.0, 0.2, 1.0];
    for i in 0..valid.len() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut p = valid;
            p[i] = invalid;
            assert!(bs_price(p[0], p[1], p[2], p[3], p[4], p[5], OptionType::Call).is_err());
        }
    }
    for i in [0, 1, 4, 5] {
        let mut p = valid;
        p[i] = -1.0;
        assert!(bs_price(p[0], p[1], p[2], p[3], p[4], p[5], OptionType::Call).is_err());
    }
    assert!(bs_price(100.0, 100.0, -1000.0, -1000.0, 0.0, 1.0, OptionType::Call).is_err());
    assert_eq!(
        bs_price(120.0, 100.0, 0.05, 0.0, 0.2, 0.0, OptionType::Call).unwrap(),
        20.0
    );
    let deterministic = bs_price(50.0, 100.0, 0.2, 0.0, 0.0, 1.0, OptionType::Put).unwrap();
    assert!((deterministic - (100.0 * (-0.2_f64).exp() - 50.0)).abs() < 1e-12);
}

#[test]
fn cos_requires_a_nonempty_expansion_and_reconciles_to_black_scholes() {
    let mut params = BlackScholesCosParams {
        spot: 100.0,
        strike: 100.0,
        rate: 0.05,
        div_yield: 0.0,
        vol: 0.2,
        expiry: 1.0,
        is_call: true,
        n_terms: Some(0),
    };
    assert!(bs_cos_price(params).is_err());
    params.n_terms = Some(128);
    let exact = bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, OptionType::Call).unwrap();
    assert!((bs_cos_price(params).unwrap() - exact).abs() < 1e-8);
}

#[test]
fn lsmc_only_applies_immediate_exercise_when_scheduled_in_both_pricing_modes() {
    let process = GbmProcess::with_params(0.2, 0.0, 0.0).unwrap();
    let payoff = AmericanPut::new(100.0).unwrap();
    let basis = PolynomialBasis::new(1);
    for (dates, expected) in [
        (vec![1], 100.0 * (-0.2_f64).exp() - 50.0),
        (vec![0, 1], 50.0),
    ] {
        let config = LsmcConfig::new(32, dates, 1)
            .unwrap()
            .with_parallel(false)
            .with_antithetic(false)
            .with_seed(7);
        let pricer = LsmcPricer::new(config);
        let estimate = pricer
            .price(&process, 50.0, 1.0, 1, &payoff, &basis, Currency::USD, 0.2)
            .unwrap();
        let independent = pricer
            .price_unbiased(
                &process,
                50.0,
                1.0,
                1,
                &payoff,
                &basis,
                Currency::USD,
                0.2,
                8,
            )
            .unwrap();
        assert!((estimate.mean.amount() - expected).abs() < 1e-10);
        assert!((independent.mean.amount() - expected).abs() < 1e-10);
    }
    assert_eq!(
        LsmcConfig::every_step(32, 1).unwrap().exercise_dates,
        vec![0, 1]
    );
}

#[test]
fn empirical_es_has_exact_tail_mass_and_is_invariant_to_replication() {
    let losses = vec![0.0, 0.0, 10.0, 20.0];
    for (confidence, expected) in [(0.75, 20.0), (0.625, 50.0 / 3.0), (0.99, 20.0)] {
        let result = PortfolioLossResult::from_losses(losses.clone(), confidence).unwrap();
        let repeated = PortfolioLossResult::from_losses(losses.repeat(10), confidence).unwrap();
        assert!((result.expected_shortfall - expected).abs() < 1e-12);
        assert!((result.expected_shortfall - repeated.expected_shortfall).abs() < 1e-12);
    }
    let tied = PortfolioLossResult::from_losses(vec![0.0, 10.0, 10.0, 20.0], 0.5).unwrap();
    assert_eq!(tied.expected_shortfall, 15.0);
    let tranche = PortfolioLossResult::from_losses(losses, 0.75)
        .unwrap()
        .tranche_loss_statistics(0.0, 1.0, 100.0)
        .unwrap();
    assert!((tranche.expected_shortfall_amount - 20.0).abs() < 1e-12);
}

#[test]
fn local_density_uses_log_moneyness_derivatives_and_is_price_scale_invariant() {
    let vols = vec![vec![0.024_f64.sqrt(), 0.04_f64.sqrt(), 0.056_f64.sqrt()]; 2];
    let mut magnitudes = Vec::new();
    for scale in [1.0, 0.01, 100.0] {
        let violations = check_local_vol_density_grid(
            &[98.0 * scale, 100.0 * scale, 102.0 * scale],
            &[1.0, 2.0],
            &vols,
            vec![100.0 * scale; 2],
        )
        .unwrap();
        assert_eq!(violations.len(), 6);
        let atm = violations
            .iter()
            .find(|v| v.location.strike == 100.0 * scale && v.location.expiry == 1.0)
            .unwrap();
        // w(K)=0.04+0.008(K-100), so w_k=.8, w_kk=.8 and g(0)=-2.64.
        assert!((atm.magnitude - 2.64).abs() < 1e-10);
        magnitudes.push(violations.iter().map(|v| v.magnitude).collect::<Vec<_>>());
    }
    for other in &magnitudes[1..] {
        for (a, b) in magnitudes[0].iter().zip(other) {
            assert!((a - b).abs() < 1e-10);
        }
    }
}

#[test]
fn arbitrage_checks_reject_missing_or_invalid_forwards_and_tolerance() {
    let strikes = [90.0, 100.0, 110.0];
    let expiries = [1.0, 2.0];
    let vols = [vec![0.3; 3], vec![0.1; 3]];
    let surface = VolSurface::from_rows("bad-calendar", &expiries, &strikes, &vols).unwrap();
    assert!(check_surface(&surface, &ArbitrageCheckConfig::default()).is_err());
    for forward in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(check_surface_grid(&strikes, &expiries, &vols, vec![forward], 1e-10).is_err());
        assert!(
            check_local_vol_density_grid(&strikes, &expiries, &vols, vec![forward; 2]).is_err()
        );
    }
    for tolerance in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(check_surface_grid(&strikes, &expiries, &vols, vec![100.0], tolerance).is_err());
    }
    assert!(
        !check_surface_grid(&strikes, &expiries, &vols, vec![100.0], 1e-10)
            .unwrap()
            .passed
    );
    assert!(check_local_vol_density_grid(&strikes, &expiries, &vols, vec![100.0]).is_err());
}

#[test]
fn pde_rejects_exercise_dates_that_would_be_silently_dropped() {
    let problem = BlackScholesPde {
        sigma: 0.2,
        rate: 0.1,
        dividend: 0.0,
        strike: 100.0,
        maturity: 1.0,
        is_call: false,
    };
    let grid = Grid1D::uniform(1.0, 6.0, 401).unwrap();
    let payoffs: Vec<_> = grid.points()[1..grid.n() - 1]
        .iter()
        .map(|&x| problem.terminal_condition(x))
        .collect();
    for time in [0.51, -0.1, 1.1, f64::NAN, f64::INFINITY] {
        let solver = Solver1D::builder()
            .grid(grid.clone())
            .crank_nicolson(10)
            .bermudan(payoffs.clone(), vec![time])
            .build()
            .unwrap();
        assert!(solver.solve(&problem, 1.0).is_err());
    }
    let european = Solver1D::builder()
        .grid(grid.clone())
        .crank_nicolson(10)
        .build()
        .unwrap()
        .solve(&problem, 1.0)
        .unwrap()
        .interpolate(50.0_f64.ln());
    let bermudan = Solver1D::builder()
        .grid(grid)
        .crank_nicolson(10)
        .bermudan(payoffs, vec![0.5])
        .build()
        .unwrap()
        .solve(&problem, 1.0)
        .unwrap();
    assert!(bermudan.interpolate(50.0_f64.ln()) > european + 4.0);
}
