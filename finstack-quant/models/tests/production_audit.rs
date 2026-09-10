//! Production audit regressions against explicit quote conventions and formulas.
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface};
use finstack_quant_models::volatility::{
    sabr::{SabrCalibrator, SabrModel, SabrParameters},
    VolSource,
};
use std::sync::Arc;

#[test]
fn m1_hull_white_node_bond_uses_centered_state() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_models::trees::{HullWhiteTree, HullWhiteTreeConfig};
    let curve = DiscountCurve::builder("HW")
        .base_date(Date::from_calendar_date(2025, time::Month::January, 1).expect("date"))
        .knots([(0.0, 1.0), (1.0, 0.98), (3.0, 0.85), (10.0, 0.55)])
        .build()
        .expect("curve");
    let kappa: f64 = 0.07;
    let sigma: f64 = 0.025;
    for steps in [20, 80] {
        let tree = HullWhiteTree::calibrate_with_times(
            HullWhiteTreeConfig::new(kappa, sigma, steps),
            &curve,
            5.0,
            &[1.0],
        )
        .expect("tree");
        let step = tree.step_at_time(1.0).expect("step");
        let middle = tree.num_nodes(step) / 2;
        // QuantLib HullWhite::A and analytical FittingParameter, evaluated
        // at the centered OU state x=0, independently of lattice alpha.
        let b = (1.0 - (-kappa * 2.0).exp()) / kappa;
        let shift = 0.5 * (sigma * (1.0 - (-kappa).exp()) / kappa).powi(2);
        let variance = sigma.powi(2) * (1.0 - (-2.0 * kappa).exp()) / (2.0 * kappa);
        let expected = (0.85 / 0.98) * (-b * shift - 0.5 * b * b * variance).exp();
        let actual = tree.bond_price(step, middle, 3.0, &curve);
        assert!(
            (actual - expected).abs() < 1e-12,
            "steps={steps}, actual={actual}, expected={expected}"
        );
    }
}

#[test]
fn m1_scheduled_hw_bond_matches_integrated_ou_moments() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
    use finstack_quant_models::trees::{HullWhiteTree, HullWhiteTreeConfig};
    let curve = DiscountCurve::flat(
        "HW",
        Date::from_calendar_date(2025, time::Month::January, 1).expect("date"),
        0.03,
    )
    .expect("curve");
    let t = 2.0_f64;
    for kappa in [1e-12_f64, 0.07] {
        let schedule =
            PiecewiseConstantCurve::new(vec![0.0, 1.0], vec![0.01, 0.025]).expect("schedule");
        let tree = HullWhiteTree::calibrate_with_times_and_volatility(
            HullWhiteTreeConfig::new(kappa, 0.01, 40),
            &curve,
            5.0,
            &[t],
            Some(schedule),
        )
        .expect("tree");
        let step = tree.step_at_time(t).expect("step");
        let b = |duration: f64| {
            if kappa < 1e-10 {
                duration
            } else {
                -(-kappa * duration).exp_m1() / kappa
            }
        };
        let n = 20_000;
        let du = t / f64::from(n);
        let (mut variance, mut covariance) = (0.0, 0.0);
        for i in 0..n {
            let u = (f64::from(i) + 0.5) * du;
            let sigma = if u < 1.0 { 0.01_f64 } else { 0.025_f64 };
            let state_loading = sigma * (-kappa * (t - u)).exp();
            variance += state_loading.powi(2) * du;
            covariance += state_loading * sigma * b(t - u) * du;
        }
        let expected =
            (-0.03_f64).exp() * (-b(1.0) * covariance - 0.5 * b(1.0).powi(2) * variance).exp();
        let actual = tree.bond_price(step, tree.num_nodes(step) / 2, 3.0, &curve);
        assert!(
            (actual - expected).abs() < 1e-11,
            "kappa={kappa}: actual={actual}, expected={expected}"
        );
    }
}

#[test]
fn b2_black_lookup_rejects_normal_surface() {
    let surface = VolSurface::builder("NORMAL")
        .quote_type(VolQuoteType::Normal)
        .expiries(&[1.0])
        .strikes(&[0.01])
        .row(&[0.008])
        .build()
        .expect("surface");
    let source = VolSource::Surface(Arc::new(surface));
    assert!(source.get_vol(1.0, 0.0, 0.01).is_err());
    assert_eq!(
        source.get_normal_vol(1.0, 0.0, 0.01).expect("normal"),
        0.008
    );
}

#[test]
fn b4_normal_sabr_smile_is_translation_invariant_across_zero() {
    // Hagan beta=0, as independently implemented by Strata's
    // SabrHaganNormalVolatilityFormula.volatilityBeta0 (B.70a):
    // alpha * z / log((sqrt(1-2*rho*z+z*z)+z-rho)/(1-rho)) * time correction.
    let alpha: f64 = 0.008;
    let nu: f64 = 0.6;
    let rho: f64 = -0.35;
    let t: f64 = 2.0;
    let z = nu / alpha * -0.005;
    let x = (((1.0 - 2.0 * rho * z + z * z).sqrt() + z - rho) / (1.0 - rho)).ln();
    let expected = alpha * z / x * (1.0 + (2.0 - 3.0 * rho * rho) / 24.0 * nu * nu * t);
    let params = SabrParameters::new(alpha, 0.0, nu, rho).expect("params");
    for (f, k) in [(-0.02, -0.015), (-0.002, 0.003), (0.01, 0.015)] {
        let model = SabrModel::new(params.clone())
            .implied_volatility(f, k, t)
            .expect("model");
        let expansion = params.implied_vol_normal(f, k, t).expect("expansion");
        assert!(
            (model - expected).abs() < 1e-12,
            "F={f}, model={model}, expected={expected}"
        );
        assert!(
            (expansion - expected).abs() < 1e-12,
            "F={f}, expansion={expansion}, expected={expected}"
        );
    }
}

#[test]
fn b5_default_sabr_calibration_controls_relative_quote_error() {
    for scale in [1.0, 10.0] {
        let forward = 0.02 * scale;
        let strikes: Vec<_> = [0.005, 0.0125, 0.02, 0.0275, 0.035]
            .map(|x| x * scale)
            .to_vec();
        let params = SabrParameters::new(0.004 * scale, 0.0, 0.7, -0.4).expect("params");
        let model = SabrModel::new(params);
        let quotes: Vec<_> = strikes
            .iter()
            .map(|&k| model.implied_volatility(forward, k, 2.0).expect("quote"))
            .collect();
        let fitted = SabrCalibrator::new()
            .calibrate(forward, &strikes, &quotes, 2.0, 0.0)
            .expect("fit");
        let fitted = SabrModel::new(fitted);
        let max_relative = strikes
            .iter()
            .zip(&quotes)
            .map(|(&k, &v)| {
                (fitted
                    .implied_volatility(forward, k, 2.0)
                    .expect("fit quote")
                    - v)
                    .abs()
                    / v
            })
            .fold(0.0_f64, f64::max);
        assert!(
            max_relative <= 1e-4,
            "scale={scale}, max relative quote error={max_relative}"
        );
    }
}

#[test]
fn b2_black_lookup_rejects_normal_cube() {
    use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
    let p = SabrParameterData::new(0.008, 0.0, 0.0, 0.3).expect("params");
    let cube = VolCube::from_grid("NORMAL", &[1.0], &[5.0], &[p], &[0.02]).expect("cube");
    let source = VolSource::Cube(Arc::new(cube));
    assert!(source.get_vol(1.0, 5.0, 0.02).is_err());
}

#[test]
fn b2_clamped_cube_preserves_small_valid_quote() {
    use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
    use finstack_quant_models::volatility::{get_cube_vol, get_cube_vol_clamped};
    let p = SabrParameterData::new(0.0001, 1.0, 0.0, 0.3).expect("params");
    let cube = VolCube::from_grid("LOW", &[1.0], &[5.0], &[p], &[0.02]).expect("cube");
    let checked = get_cube_vol(&cube, 1.0, 5.0, 0.02).expect("quote");
    let clamped = get_cube_vol_clamped(&cube, 1.0, 5.0, 0.02);
    assert!(
        (checked - clamped).abs() < 1e-14,
        "checked {checked}, clamped {clamped}"
    );
}

#[test]
fn b2_clamped_cube_never_fabricates_volatility_for_invalid_black_rates() {
    use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
    use finstack_quant_models::volatility::get_cube_vol_clamped;
    let p = SabrParameterData::new(0.008, 0.5, 0.0, 0.3).expect("params");
    let cube = VolCube::from_grid("INVALID", &[1.0], &[5.0], &[p], &[-0.01]).expect("cube");
    assert!(get_cube_vol_clamped(&cube, 1.0, 5.0, -0.01).is_nan());
}

#[test]
fn b4_normal_cube_uses_canonical_beta_snap() {
    use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};
    use finstack_quant_models::volatility::VolatilityConvention;
    let p = SabrParameterData::new(0.008, 0.00005, -0.2, 0.3).expect("params");
    let cube = VolCube::from_grid("NORMAL-SNAP", &[1.0], &[5.0], &[p], &[-0.01]).expect("cube");
    let source = VolSource::Cube(Arc::new(cube));
    assert_eq!(
        source.get_convention(1.0, 5.0).expect("convention"),
        VolatilityConvention::Normal
    );
    let model = SabrModel::new(SabrParameters::new(0.008, 0.00005, 0.3, -0.2).expect("parameters"));
    let expected = model
        .implied_volatility(-0.01, 0.001, 1.0)
        .expect("normal quote");
    assert!(
        (source
            .get_normal_vol_clamped(1.0, 5.0, 0.001)
            .expect("cube quote")
            - expected)
            .abs()
            < 1e-14
    );
}

#[test]
fn m1_hw_bond_option_converges_to_independent_affine_price() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::special_functions::norm_cdf;
    use finstack_quant_models::trees::{HullWhiteTree, HullWhiteTreeConfig};
    let rate: f64 = 0.04;
    let kappa: f64 = 0.07;
    let sigma: f64 = 0.02;
    let expiry: f64 = 1.0;
    let maturity: f64 = 5.0;
    let strike: f64 = 0.83;
    let curve = DiscountCurve::builder("HW-OPTION")
        .base_date(Date::from_calendar_date(2025, time::Month::January, 1).expect("date"))
        .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
        .build()
        .expect("curve");
    // Independent Hull-White discount-bond option formula (QuantLib's
    // HullWhite::discountBondOption), with unit bond face and expiry strike.
    let bond_vol = sigma * (1.0 - (-kappa * (maturity - expiry)).exp()) / kappa
        * ((1.0 - (-2.0 * kappa * expiry).exp()) / (2.0 * kappa)).sqrt();
    let p_expiry = (-rate * expiry).exp();
    let p_maturity = (-rate * maturity).exp();
    let d1 = (p_maturity / (strike * p_expiry)).ln() / bond_vol + 0.5 * bond_vol;
    let expected = p_maturity * norm_cdf(d1) - strike * p_expiry * norm_cdf(d1 - bond_vol);
    let mut errors = Vec::new();
    for steps in [25, 100, 400] {
        let tree = HullWhiteTree::calibrate_with_times(
            HullWhiteTreeConfig::new(kappa, sigma, steps),
            &curve,
            expiry,
            &[expiry],
        )
        .expect("tree");
        let terminal: Vec<_> = (0..tree.num_nodes(tree.num_steps()))
            .map(|j| (tree.bond_price(tree.num_steps(), j, maturity, &curve) - strike).max(0.0))
            .collect();
        let actual = tree
            .backward_induction(&terminal, |_, _, value| value)
            .expect("rollback");
        errors.push((actual - expected).abs());
    }
    assert!(
        errors[2] < errors[0] && errors[2] < 2e-5,
        "convergence errors {errors:?}"
    );
}
