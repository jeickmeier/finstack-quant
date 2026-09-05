use super::*;
use crate::trees::hull_white_tree::HullWhiteTree;
use crate::trees::tree_framework::{NodeState, TreeModel, TreeValuator};
use crate::volatility::{convert_atm_volatility, VolatilityConvention};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::{Error, HashMap, Result};
use time::Month;

const TEST_CURVE_ID: &str = "USD-OIS";

fn create_test_curve() -> DiscountCurve {
    DiscountCurve::builder(TEST_CURVE_ID)
        .base_date(
            finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                .expect("should succeed"),
        )
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (5.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("should succeed")
}

fn create_flat_curve(rate: f64) -> DiscountCurve {
    let knots = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0]
        .into_iter()
        .map(|t| (t, (-rate * t).exp()));
    DiscountCurve::builder(TEST_CURVE_ID)
        .base_date(
            finstack_quant_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                .expect("should succeed"),
        )
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("should succeed")
}

struct ConstantValuator;

impl TreeValuator for ConstantValuator {
    fn value_at_maturity(&self, _state: &NodeState) -> Result<f64> {
        Ok(1.0)
    }

    fn value_at_node(&self, _state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        Ok(continuation_value)
    }
}

struct RateCallValuator {
    strike: f64,
}

impl TreeValuator for RateCallValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        let rate = state
            .interest_rate()
            .ok_or_else(|| Error::internal("rate-call node missing interest rate"))?;
        Ok((rate - self.strike).max(0.0))
    }

    fn value_at_node(&self, _state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        Ok(continuation_value)
    }
}

#[test]
fn test_ho_lee_tree_creation() {
    let tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(50, 0.01));
    assert_eq!(tree.config.steps, 50);
    assert_eq!(tree.config.model, ShortRateModel::HoLee);
    assert_eq!(tree.config.volatility, 0.01);
}

#[test]
fn test_tree_calibration() {
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(10, 0.015));
    let curve = create_test_curve();

    let result = tree.calibrate(&curve, 2.0);
    assert!(result.is_ok());

    assert_eq!(tree.rates.len(), 11); // 0 to 10 steps
    assert_eq!(tree.rates[0].len(), 1); // First step has one node
    assert_eq!(tree.rates[10].len(), 11); // Last step has 11 nodes
}

#[test]
fn ho_lee_stored_lattice_prices_zero_coupon_to_calibration_curve() {
    let steps = 12;
    let maturity = 2.0;
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(steps, 0.015));
    let curve = create_test_curve();
    tree.calibrate(&curve, maturity)
        .expect("Ho-Lee calibration");

    let market = MarketContext::new();
    let actual = tree
        .price(
            HashMap::<&'static str, f64>::default(),
            maturity,
            &market,
            &ConstantValuator,
        )
        .expect("Ho-Lee zero-coupon price");
    let expected = curve.df(maturity);

    assert!(
        (actual - expected).abs() < 1e-8,
        "Ho-Lee stored lattice should price a zero coupon to the calibration curve: actual={actual}, expected={expected}"
    );
}

/// calibration must honor `config.compounding`. A
/// Ho-Lee tree configured with non-continuous compounding must reprice
/// the calibration curve to <0.1 bp, because `price()` discounts with the
/// same convention.
#[test]
fn ho_lee_noncontinuous_compounding_reprices_curve() {
    for compounding in [
        TreeCompounding::Simple,
        TreeCompounding::SemiAnnual,
        TreeCompounding::Quarterly,
        TreeCompounding::Monthly,
    ] {
        let steps = 24;
        let maturity = 2.0;
        let config = ShortRateTreeConfig::ho_lee(steps, 0.012).with_compounding(compounding);
        let mut tree = ShortRateTree::new(config);
        let curve = create_test_curve();
        tree.calibrate(&curve, maturity)
            .expect("Ho-Lee calibration under non-continuous compounding");

        let quality = tree.calibration_result().expect("quality");
        assert!(
            quality.converged && quality.max_error_bp < 0.1,
            "{compounding:?}: calibration must reprice the curve to <0.1bp, \
             got {quality:?}"
        );

        let market = MarketContext::new();
        let actual = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                maturity,
                &market,
                &ConstantValuator,
            )
            .expect("zero-coupon price");
        let expected = curve.df(maturity);
        assert!(
            ((actual - expected) / expected).abs() * 10_000.0 < 0.1,
            "{compounding:?}: zero coupon must reprice to <0.1bp: \
             actual={actual}, expected={expected}"
        );
    }
}

/// `rate_from_df` inverts `df` for every convention.
#[test]
fn tree_compounding_rate_from_df_inverts_df() {
    for compounding in [
        TreeCompounding::Continuous,
        TreeCompounding::Simple,
        TreeCompounding::SemiAnnual,
        TreeCompounding::Quarterly,
        TreeCompounding::Monthly,
    ] {
        for rate in [-0.01, 0.0, 0.025, 0.10] {
            let dt = 0.25;
            let df = compounding.df(rate, dt);
            let recovered = compounding.rate_from_df(df, dt);
            assert!(
                (recovered - rate).abs() < 1e-12,
                "{compounding:?}: rate_from_df(df({rate})) = {recovered}"
            );
        }
    }
}

#[test]
fn ho_lee_calibration_flags_pathologically_extreme_node_discount_factors() {
    // P0/item-8: Ho-Lee correctly admits negative rates, but with an
    // extreme normal volatility the lattice produces wildly negative node
    // rates whose per-step discount factor `exp(-r*dt)` explodes far above
    // 1. That is a numerical-breakdown signal: the calibration must emit a
    // diagnostic error rather than silently returning an unusable tree.
    //
    // sigma = 8.0 (800%/yr normal vol), 60 steps, T = 30 => dt = 0.5,
    // sigma*sqrt(dt) ~ 5.66 per step; the lowest node after 60 steps sits
    // near -300, so its node DF is exp(150) — astronomically extreme.
    let curve = create_flat_curve(0.03);
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(60, 8.0));

    let result = tree.calibrate(&curve, 30.0);
    assert!(
        result.is_err(),
        "Ho-Lee calibration that yields pathologically extreme node \
         discount factors must report a diagnostic error"
    );
    let msg = result.expect_err("must error").to_string().to_lowercase();
    assert!(
        msg.contains("discount") || msg.contains("rate") || msg.contains("extreme"),
        "error should explain the extreme-node diagnostic, got: {msg}"
    );
}

#[test]
fn ho_lee_calibration_succeeds_for_a_normal_volatility_tree() {
    // The extreme-node guard must not reject ordinary trees: a normal
    // volatility (1%) Ho-Lee tree must still calibrate cleanly even with
    // many steps and a long horizon.
    let curve = create_test_curve();
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(60, 0.01));
    tree.calibrate(&curve, 5.0)
        .expect("a normal-volatility Ho-Lee tree must calibrate");
    let quality = tree.calibration_result().expect("quality");
    assert!(quality.converged, "quality={quality:?}");
}

#[test]
fn test_rate_access() {
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(5, 0.01));
    let curve = create_test_curve();
    tree.calibrate(&curve, 1.0).expect("should succeed");

    let r0 = tree.rate_at_node(0, 0).expect("should succeed");
    assert!(r0 > 0.0);

    let r_final = tree.rate_at_node(5, 2).expect("should succeed");
    assert!(r_final.is_finite());

    assert!(tree.rate_at_node(10, 0).is_err());
    assert!(tree.rate_at_node(0, 5).is_err());
}

#[test]
fn test_bdt_tree_creation() {
    // BDT with realistic 20% lognormal volatility
    let tree = ShortRateTree::new(ShortRateTreeConfig::bdt(25, 0.20, 0.03));
    assert_eq!(tree.config.model, ShortRateModel::BlackDermanToy);
    assert_eq!(tree.config.volatility, 0.20);
    assert_eq!(tree.config.mean_reversion, 0.03);
}

#[test]
fn test_bdt_calibration_populates_quality_metrics() {
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(6, 0.20, 0.0));
    let curve = create_test_curve();

    tree.calibrate(&curve, 2.0).expect("should succeed");

    assert_eq!(tree.rates.len(), 7);
    let quality = tree.calibration_result().expect("calibration result");
    assert!(quality.converged);
    assert!(quality.max_error_bp.is_finite());
}

#[test]
fn test_bdt_stored_lattice_prices_zero_coupon_to_calibration_curve() {
    let steps = 8;
    let maturity = 2.0;
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.0));
    let curve = create_test_curve();
    tree.calibrate(&curve, maturity).expect("BDT calibration");

    let mut vars = HashMap::<&'static str, f64>::default();
    vars.insert(
        short_rate_keys::SHORT_RATE,
        tree.rate_at_node(0, 0).expect("root rate"),
    );
    let market = MarketContext::new();
    let actual = tree
        .price(vars, maturity, &market, &ConstantValuator)
        .expect("BDT zero coupon price");
    let expected = curve.df(maturity);

    assert!(
        (actual - expected).abs() < 1e-8,
        "BDT stored lattice should price a zero coupon to the calibration curve: actual={actual}, expected={expected}"
    );
}

#[test]
fn test_bdt_config_uses_binomial_branching_matching_calibration_geometry() {
    let config = ShortRateTreeConfig::bdt(6, 0.20, 0.0);
    let mut tree = ShortRateTree::new(config);
    let curve = create_test_curve();
    tree.calibrate(&curve, 2.0).expect("BDT calibration");

    for step in 0..=6 {
        assert_eq!(
            tree.rates[step].len(),
            step + 1,
            "BDT calibration is binomial-width at step {step}"
        );
    }
}

/// Terminal probability distribution over the BK trinomial lattice
/// (transition-probability measure, no discounting). Returns the node
/// probabilities and the terminal x-values `x = ln r − a_N`.
fn bk_terminal_x_distribution(tree: &ShortRateTree) -> (Vec<f64>, Vec<f64>) {
    let lattice = tree.bk_trinomial.as_ref().expect("BK trinomial lattice");
    let steps = tree.config.steps;
    let j_max = lattice.j_max;
    let dt = tree.time_steps[1] - tree.time_steps[0];
    let dx = tree.config.volatility * (3.0 * dt).sqrt();

    let mut dist = vec![1.0];
    for step in 0..steps {
        let curr_j_max = step.min(j_max);
        let next_j_max = (step + 1).min(j_max);
        let boundary = if curr_j_max == next_j_max {
            curr_j_max
        } else {
            usize::MAX
        };
        let mut next = vec![0.0; 2 * next_j_max + 1];
        for (j, &pj) in dist.iter().enumerate() {
            let j_signed = j as i32 - curr_j_max as i32;
            for (offset, p) in
                HullWhiteTree::transition_offsets(j_signed, boundary, lattice.probs[step][j])
            {
                if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max) {
                    next[idx] += pj * p;
                }
            }
        }
        dist = next;
    }

    let term_j_max = steps.min(j_max);
    let xs: Vec<f64> = (0..dist.len())
        .map(|j| (j as i32 - term_j_max as i32) as f64 * dx)
        .collect();
    (xs, dist)
}

fn weighted_std(values: &[f64], weights: &[f64]) -> f64 {
    let total: f64 = weights.iter().sum();
    let mean: f64 = values.iter().zip(weights).map(|(v, w)| v * w).sum::<f64>() / total;
    let var: f64 = values
        .iter()
        .zip(weights)
        .map(|(v, w)| w * (v - mean) * (v - mean))
        .sum::<f64>()
        / total;
    var.sqrt()
}

/// with κ ≠ 0 the BDT model routes to a genuine
/// trinomial Black-Karasinski lattice that still reprices the curve and
/// tightens the (probability-weighted) terminal log-rate dispersion
/// relative to κ = 0.
#[test]
fn test_bdt_mean_reversion_calibrates_and_tightens_rate_dispersion() {
    let steps = 50;
    let mut tree_no_mr = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.0));
    let mut tree_mr = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.05));
    let curve = create_test_curve();

    tree_no_mr.calibrate(&curve, 2.0).expect("BDT(κ=0)");
    tree_mr.calibrate(&curve, 2.0).expect("BK(κ=0.05)");

    let quality = tree_mr.calibration_result().expect("quality");
    assert!(
        quality.is_acceptable(),
        "BK(κ=0.05) calibration: max_error={:.2}bp",
        quality.max_error_bp
    );

    // Probability-weighted terminal ln-rate dispersion: κ > 0 tightens it.
    // Binomial κ=0 tree: terminal distribution is Binomial(steps, 1/2).
    let ln_rates_no_mr: Vec<f64> = tree_no_mr.rates[steps].iter().map(|r| r.ln()).collect();
    let mut binom_weights = vec![0.0_f64; steps + 1];
    let mut c = 1.0_f64;
    for (k, w) in binom_weights.iter_mut().enumerate() {
        *w = c * 0.5_f64.powi(steps as i32);
        c = c * (steps - k) as f64 / (k + 1) as f64;
    }
    let std_no_mr = weighted_std(&ln_rates_no_mr, &binom_weights);

    let (xs_mr, dist_mr) = bk_terminal_x_distribution(&tree_mr);
    let std_mr = weighted_std(&xs_mr, &dist_mr);

    assert!(
        std_mr < std_no_mr,
        "mean reversion should tighten terminal log-rate dispersion: \
         no_mr={std_no_mr:.6}, mr={std_mr:.6}"
    );

    let market = MarketContext::new();
    let mut vars = HashMap::<&'static str, f64>::default();
    vars.insert(
        short_rate_keys::SHORT_RATE,
        tree_mr.rate_at_node(0, 0).expect("root"),
    );
    let zcb = tree_mr
        .price(vars, 2.0, &market, &ConstantValuator)
        .expect("ZCB price");
    let target = curve.df(2.0);
    assert!(
        (zcb - target).abs() < 1e-6,
        "BK(κ=0.05) should still price ZCBs to curve: got={zcb:.8}, target={target:.8}"
    );
}

/// the BK trinomial lattice reprices the calibration
/// curve to <0.1 bp, both via Arrow-Debreu state prices and via the
/// dedicated backward induction in `price()`.
#[test]
fn bk_trinomial_reprices_curve_to_a_tenth_bp() {
    let steps = 200;
    let maturity = 5.0;
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, 0.20, 0.03));
    let curve = create_test_curve();
    tree.calibrate(&curve, maturity).expect("BK calibration");

    let quality = tree.calibration_result().expect("quality");
    assert!(
        quality.converged && quality.max_error_bp < 0.1,
        "BK calibration must reprice the curve to <0.1bp, got {quality:?}"
    );

    let market = MarketContext::new();
    let zcb = tree
        .price(
            HashMap::<&'static str, f64>::default(),
            maturity,
            &market,
            &ConstantValuator,
        )
        .expect("ZCB price");
    let target = curve.df(maturity);
    let error_bp = ((zcb - target) / target).abs() * 10_000.0;
    assert!(
        error_bp < 0.1,
        "BK backward induction must reprice ZCB to <0.1bp: \
         got={zcb:.8}, target={target:.8} ({error_bp:.4}bp)"
    );
}

/// as Δt → 0 the terminal log-rate dispersion of the
/// BK lattice approaches the OU limit `σ√((1−e^{−2κT})/(2κ))` — about
/// 13% below σ√T at κ = 0.03, T = 10y — instead of growing like σ√T.
#[test]
fn bk_terminal_log_rate_dispersion_matches_ou_limit() {
    let steps = 400;
    let maturity = 10.0;
    let sigma = 0.20;
    let kappa = 0.03;
    let curve = create_flat_curve(0.04);

    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, kappa));
    tree.calibrate(&curve, maturity).expect("BK calibration");

    let (xs, dist) = bk_terminal_x_distribution(&tree);
    let std_x = weighted_std(&xs, &dist);

    let target = sigma * ((1.0 - (-2.0 * kappa * maturity).exp()) / (2.0 * kappa)).sqrt();
    let no_mr = sigma * maturity.sqrt();

    assert!(
        ((std_x - target) / target).abs() < 0.02,
        "terminal log-rate dispersion should match the OU limit: \
         got {std_x:.6}, target {target:.6} (σ√T = {no_mr:.6})"
    );
    assert!(
        std_x < 0.95 * no_mr,
        "dispersion must be materially below the κ=0 value σ√T: \
         got {std_x:.6} vs σ√T = {no_mr:.6}"
    );
}

/// as κ → 0 the trinomial BK lattice converges to the
/// binomial BDT lattice (same continuous model).
#[test]
fn bk_kappa_to_zero_converges_to_bdt() {
    let steps = 200;
    let maturity = 5.0;
    let sigma = 0.20;
    let curve = create_flat_curve(0.04);
    let market = MarketContext::new().insert(curve.clone());
    let valuator = RateCallValuator { strike: 0.04 };
    let vars = HashMap::<&'static str, f64>::default();

    let mut bdt = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, 0.0));
    bdt.calibrate(&curve, maturity).expect("BDT(κ=0)");
    let price_bdt = bdt
        .price(vars.clone(), maturity, &market, &valuator)
        .expect("BDT price");

    let mut bk = ShortRateTree::new(ShortRateTreeConfig::bdt(steps, sigma, 1e-4));
    bk.calibrate(&curve, maturity).expect("BK(κ→0)");
    assert!(bk.bk_trinomial.is_some(), "κ=1e-4 must route to BK lattice");
    let price_bk = bk
        .price(vars, maturity, &market, &valuator)
        .expect("BK price");

    // Tiny terminal dispersion check: at κ→0 the OU limit is σ√T.
    let (xs, dist) = bk_terminal_x_distribution(&bk);
    let std_x = weighted_std(&xs, &dist);
    let target = sigma * maturity.sqrt();
    assert!(
        ((std_x - target) / target).abs() < 0.01,
        "κ→0 dispersion should approach σ√T: got {std_x:.6}, target {target:.6}"
    );

    assert!(
        price_bdt > 0.0 && price_bk > 0.0,
        "rate-call prices must be positive: bdt={price_bdt}, bk={price_bk}"
    );
    let rel = ((price_bk - price_bdt) / price_bdt).abs();
    assert!(
        rel < 0.05,
        "κ→0 BK lattice should converge to BDT: bdt={price_bdt:.8}, \
         bk={price_bk:.8} (rel diff {rel:.4})"
    );
}

#[test]
fn test_normal_to_lognormal_vol_conversion() {
    let normal_vol = 0.01; // 100 bp
    let rate_level = 0.05; // 5%

    let lognormal = convert_atm_volatility(
        normal_vol,
        VolatilityConvention::Normal,
        VolatilityConvention::Lognormal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");

    // Roughly normal_vol / rate_level.
    assert!(
        lognormal > 0.15 && lognormal < 0.25,
        "lognormal vol {lognormal} out of range"
    );

    let recovered = convert_atm_volatility(
        lognormal,
        VolatilityConvention::Lognormal,
        VolatilityConvention::Normal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");
    assert!(
        (recovered - normal_vol).abs() < 1e-10,
        "Round-trip failed: got {recovered}, expected {normal_vol}"
    );
}

#[test]
fn test_lognormal_to_normal_vol_conversion() {
    let lognormal_vol = 0.20; // 20%
    let rate_level = 0.05; // 5%

    let normal = convert_atm_volatility(
        lognormal_vol,
        VolatilityConvention::Lognormal,
        VolatilityConvention::Normal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");

    // Roughly lognormal_vol * rate_level.
    assert!(
        normal > 0.005 && normal < 0.015,
        "normal vol {normal} out of range"
    );

    let recovered = convert_atm_volatility(
        normal,
        VolatilityConvention::Normal,
        VolatilityConvention::Lognormal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");
    assert!(
        (recovered - lognormal_vol).abs() < 1e-10,
        "Round-trip failed: got {recovered}, expected {lognormal_vol}"
    );
}

#[test]
fn test_vol_conversion_roundtrip() {
    let original_normal = 0.012; // 120 bp
    let rate_level = 0.045; // 4.5%

    let lognormal = convert_atm_volatility(
        original_normal,
        VolatilityConvention::Normal,
        VolatilityConvention::Lognormal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");
    let back_to_normal = convert_atm_volatility(
        lognormal,
        VolatilityConvention::Lognormal,
        VolatilityConvention::Normal,
        rate_level,
        1.0,
    )
    .expect("valid conversion");

    assert!(
        (back_to_normal - original_normal).abs() < 1e-6,
        "Roundtrip conversion should be exact"
    );
}

#[test]
fn test_normal_to_lognormal_errors_on_zero_rate() {
    let err = convert_atm_volatility(
        0.01,
        VolatilityConvention::Normal,
        VolatilityConvention::Lognormal,
        0.0,
        1.0,
    )
    .expect_err("should error");
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_normal_to_lognormal_errors_on_negative_rate() {
    let err = convert_atm_volatility(
        0.01,
        VolatilityConvention::Normal,
        VolatilityConvention::Lognormal,
        -0.01,
        1.0,
    )
    .expect_err("should error");
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_calibration_result_quality_helpers_cover_thresholds() {
    let good = TreeCalibrationResult {
        max_error_bp: 0.05,
        max_error_step: 2,
        fallback_count: 0,
        converged: true,
    };
    assert!(good.is_acceptable());

    let acceptable_only = TreeCalibrationResult {
        max_error_bp: 0.5,
        max_error_step: 3,
        fallback_count: 0,
        converged: true,
    };
    assert!(acceptable_only.is_acceptable());

    let poor = TreeCalibrationResult {
        max_error_bp: 2.0,
        max_error_step: 1,
        fallback_count: 1,
        converged: true,
    };
    assert!(!poor.is_acceptable());
}

#[test]
fn compounding_conventions_stay_finite_for_deeply_negative_rates() {
    for compounding in [
        TreeCompounding::Simple,
        TreeCompounding::SemiAnnual,
        TreeCompounding::Quarterly,
        TreeCompounding::Monthly,
    ] {
        let df = compounding.df(-100.0, 0.5);
        let continuous = compounding.to_continuous(-100.0, 0.5);
        assert!(
            df.is_finite() && df > 0.0,
            "{compounding:?} discount factor should stay positive and finite, got {df}"
        );
        assert!(
            continuous.is_finite(),
            "{compounding:?} continuous equivalent should stay finite, got {continuous}"
        );
    }
}

#[test]
fn bdt_calibrates_near_zero_flat_curve_without_fallbacks() {
    let curve = create_flat_curve(0.0001);
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(12, 0.20, 0.0));

    tree.calibrate(&curve, 2.0)
        .expect("near-zero BDT calibration");

    let quality = tree.calibration_result().expect("quality");
    assert_eq!(quality.fallback_count, 0);
    assert!(quality.is_acceptable(), "quality={quality:?}");
    for step in 0..=12 {
        for node in 0..=step {
            let rate = tree.rate_at_node(step, node).expect("rate");
            assert!(rate.is_finite() && rate > 0.0, "rate={rate}");
        }
    }
}

#[test]
fn bdt_calibrates_high_rate_flat_curve_with_finite_rates() {
    let curve = create_flat_curve(0.75);
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(12, 0.20, 0.0));

    tree.calibrate(&curve, 2.0)
        .expect("high-rate BDT calibration");

    let quality = tree.calibration_result().expect("quality");
    assert_eq!(quality.fallback_count, 0);
    assert!(quality.is_acceptable(), "quality={quality:?}");
    for step in 0..=12 {
        for node in 0..=step {
            let rate = tree.rate_at_node(step, node).expect("rate");
            assert!(rate.is_finite() && rate > 0.0, "rate={rate}");
        }
    }
}

#[test]
fn bdt_calibration_fails_when_node_rate_clamp_engages_materially() {
    // P0-6: BDT clamps every node rate to `[1e-8, 5.0]` inside the Brent
    // objective. For a tree that is too wide (high vol, many steps, long
    // horizon) the clamp saturates nodes with material Arrow-Debreu
    // weight, the objective stops responding to `alpha`, and the
    // calibrated tree silently *fails* to reprice the curve (here by many
    // thousands of basis points). Calibration must NOT report success in
    // that case — it must return an explicit error rather than a
    // quietly-mispriced tree.
    //
    // sigma = 1.50, 120 steps, T = 60 => step_vol ~ 1.50*sqrt(0.5) ~ 1.06,
    // u ~ 2.89; the lattice is so wide the clamp wrecks repricing.
    let curve = create_flat_curve(0.05);
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(120, 1.50, 0.0));

    let result = tree.calibrate(&curve, 60.0);
    assert!(
        result.is_err(),
        "a BDT calibration whose node-rate clamp engages materially must \
         fail explicitly, not return a silently-mispriced tree"
    );
    let msg = result.expect_err("must error").to_string().to_lowercase();
    assert!(
        msg.contains("clamp") || msg.contains("reprice") || msg.contains("calibrat"),
        "error should explain the calibration / clamp failure, got: {msg}"
    );
}

#[test]
fn bdt_calibration_succeeds_for_a_normal_well_posed_tree() {
    // The clamp-engagement guard must not be over-eager: an ordinary
    // BDT tree (moderate vol, moderate horizon) whose node rates stay
    // comfortably inside `[1e-8, 5.0]` must still calibrate cleanly.
    let curve = create_test_curve();
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(40, 0.20, 0.0));
    tree.calibrate(&curve, 5.0)
        .expect("a well-posed BDT tree must calibrate");
    let quality = tree.calibration_result().expect("quality");
    assert!(quality.converged, "quality={quality:?}");
    assert!(quality.is_acceptable(), "quality={quality:?}");
}

#[test]
fn test_config_ho_lee_factory() {
    let config = ShortRateTreeConfig::ho_lee(100, 0.008);
    assert_eq!(config.steps, 100);
    assert_eq!(config.model, ShortRateModel::HoLee);
    assert_eq!(config.volatility, 0.008);
    assert_eq!(config.mean_reversion, 0.0);
}

#[test]
fn test_config_bdt_factory() {
    let config = ShortRateTreeConfig::bdt(100, 0.20, 0.03);
    assert_eq!(config.steps, 100);
    assert_eq!(config.model, ShortRateModel::BlackDermanToy);
    assert_eq!(config.volatility, 0.20);
    assert_eq!(config.mean_reversion, 0.03);
}

#[test]
fn test_time_accessor_validates_bounds() {
    let mut tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(5, 0.01));
    let curve = create_test_curve();
    tree.calibrate(&curve, 1.0).expect("should succeed");

    assert_eq!(tree.time_at_step(0).expect("time"), 0.0);
    assert!(tree.time_at_step(5).expect("time").is_finite());
    assert!(tree.time_at_step(10).is_err());
}

#[test]
fn test_calibrate_rejects_zero_steps_and_non_positive_horizon() {
    let curve = create_test_curve();

    let mut zero_steps = ShortRateTree::new(ShortRateTreeConfig::ho_lee(0, 0.01));
    let err = zero_steps
        .calibrate(&curve, 1.0)
        .expect_err("zero steps must be rejected");
    assert!(err.to_string().contains("at least one step"), "{err}");

    let mut tree = ShortRateTree::new(ShortRateTreeConfig::bdt(5, 0.20, 0.0));
    for ttm in [0.0, -1.0, f64::NAN] {
        let err = tree
            .calibrate(&curve, ttm)
            .expect_err("non-positive horizon must be rejected");
        assert!(err.to_string().contains("time to maturity"), "{err}");
    }
}

#[test]
fn test_price_rejects_uncalibrated_tree() {
    let tree = ShortRateTree::new(ShortRateTreeConfig::ho_lee(5, 0.01));
    let err = tree
        .price(
            HashMap::<&'static str, f64>::default(),
            1.0,
            &MarketContext::new(),
            &ConstantValuator,
        )
        .expect_err("uncalibrated tree should error");
    assert!(err.to_string().contains("must be calibrated"));
}

#[test]
fn test_ho_lee_rejects_nonzero_mean_reversion() {
    let config = ShortRateTreeConfig {
        steps: 10,
        model: ShortRateModel::HoLee,
        volatility: 0.01,
        mean_reversion: 0.05,
        compounding: TreeCompounding::default(),
        curve_fit_tolerance_bp: DEFAULT_CURVE_FIT_TOLERANCE_BP,
    };
    let mut tree = ShortRateTree::new(config);
    let curve = create_test_curve();
    let err = tree
        .calibrate(&curve, 2.0)
        .expect_err("Ho-Lee with mean reversion must be rejected");
    assert!(
        err.to_string().contains("mean reversion"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_ho_lee_allows_zero_mean_reversion() {
    let config = ShortRateTreeConfig {
        steps: 10,
        model: ShortRateModel::HoLee,
        volatility: 0.01,
        mean_reversion: 0.0,
        compounding: TreeCompounding::default(),
        curve_fit_tolerance_bp: DEFAULT_CURVE_FIT_TOLERANCE_BP,
    };
    let mut tree = ShortRateTree::new(config);
    let curve = create_test_curve();
    tree.calibrate(&curve, 2.0)
        .expect("Ho-Lee with κ=0 should succeed");
}
