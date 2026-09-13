use super::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::InterpStyle;

fn targets_from_curves(
    disc: &dyn Discounting,
    hazard: &HazardCurve,
    steps: usize,
    horizon: f64,
) -> RatesCreditCalibrationTargets {
    let discount_at_origin = disc.df(0.0);
    let survival_at_origin = hazard.sp(0.0);
    let times: Vec<f64> = (0..=steps)
        .map(|step| step as f64 * horizon / steps as f64)
        .collect();
    RatesCreditCalibrationTargets {
        discount_factors: times
            .iter()
            .map(|&time| disc.df(time) / discount_at_origin)
            .collect(),
        survival_probabilities: times
            .iter()
            .map(|&time| hazard.sp(time) / survival_at_origin)
            .collect(),
        times,
        recovery_rate: hazard.recovery_rate(),
    }
}

fn calibrate_for_test(
    tree: &mut RatesCreditTree,
    disc: &dyn Discounting,
    hazard: &HazardCurve,
    horizon: f64,
) -> Result<()> {
    let targets = targets_from_curves(disc, hazard, tree.config.steps, horizon);
    tree.calibrate(&targets)
}

/// The default config is deterministic in both factors, so
/// `..Default::default()` construction can never silently price
/// optionality the caller did not request.
#[test]
fn default_config_is_deterministic_in_both_factors() {
    let cfg = RatesCreditConfig::default();
    assert_eq!(cfg.rate_vol, 0.0);
    assert_eq!(cfg.hazard_vol, 0.0);
    assert_eq!(cfg.correlation, 0.0);
    assert_eq!(cfg.rate_mean_reversion, 0.0);
    assert_eq!(cfg.hazard_mean_reversion, 0.0);
}

#[test]
fn calibrated_factors_retain_one_affine_descriptor_per_step() {
    let steps = 64;
    let horizon = 10.0;
    let times: Vec<f64> = (0..=steps)
        .map(|step| step as f64 * horizon / steps as f64)
        .collect();
    let targets = RatesCreditCalibrationTargets {
        discount_factors: times.iter().map(|&time| (-0.03 * time).exp()).collect(),
        survival_probabilities: times.iter().map(|&time| (-0.02 * time).exp()).collect(),
        times,
        recovery_rate: 0.4,
    };
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.01,
        hazard_vol: 0.005,
        ..Default::default()
    });
    tree.calibrate(&targets).expect("calibrate affine rows");

    assert_eq!(tree.calibrated_rates.len(), steps + 1);
    assert_eq!(tree.calibrated_hazards.len(), steps + 1);
    for step in 0..=steps {
        let rate_row = tree.calibrated_rates[step];
        let hazard_row = tree.calibrated_hazards[step];
        assert_eq!(rate_row.nodes, step + 1);
        assert_eq!(hazard_row.nodes, step + 1);
        assert_eq!(
            tree.rate_at_node(step, 0).expect("first rate"),
            rate_row.base
        );
        assert_eq!(
            tree.rate_at_node(step, step).expect("last rate"),
            rate_row.base + step as f64 * rate_row.shift
        );
        assert_eq!(
            tree.hazard_at_node(step, step).expect("last hazard"),
            hazard_row.base + step as f64 * hazard_row.shift
        );
    }
}

#[test]
fn explicit_conditional_targets_define_levels_and_horizon() {
    let steps = 8;
    let horizon = 2.0;
    let times: Vec<f64> = (0..=steps)
        .map(|step| step as f64 * horizon / steps as f64)
        .collect();
    let targets = RatesCreditCalibrationTargets {
        discount_factors: times.iter().map(|&time| (-0.03 * time).exp()).collect(),
        survival_probabilities: times.iter().map(|&time| (-0.02 * time).exp()).collect(),
        times: times.clone(),
        recovery_rate: 0.35,
    };
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        ..Default::default()
    });
    tree.calibrate(&targets)
        .expect("explicit targets calibrate");

    assert_eq!(tree.time_grid().expect("grid"), times);
    assert!((tree.rate_at_node(0, 0).expect("rate") - 0.03).abs() < 1e-12);
    assert!((tree.hazard_at_node(0, 0).expect("hazard") - 0.02).abs() < 1e-12);
    assert_eq!(tree.recovery_rate(), 0.35);

    let err = tree
        .conditional_discount_factors(0, 1, horizon + 0.25)
        .expect_err("pricing with a different horizon must fail");
    assert!(err.to_string().contains("does not match"));
}

#[test]
fn calibration_rejects_origin_and_grid_ambiguity() {
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 2,
        ..Default::default()
    });
    let valid = RatesCreditCalibrationTargets {
        times: vec![0.0, 0.5, 1.0],
        discount_factors: vec![1.0, 0.98, 0.95],
        survival_probabilities: vec![1.0, 0.99, 0.97],
        recovery_rate: 0.4,
    };

    let mut shifted_origin = valid.clone();
    shifted_origin.discount_factors[0] = 0.99;
    assert!(tree.calibrate(&shifted_origin).is_err());

    let mut uneven = valid.clone();
    uneven.times[1] = 0.4;
    assert!(tree.calibrate(&uneven).is_err());

    let mut increasing_survival = valid;
    increasing_survival.survival_probabilities[2] = 1.01;
    assert!(tree.calibrate(&increasing_survival).is_err());
}

#[test]
fn extrema_correlation_scan_matches_exhaustive_node_pairs() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 16;
    let horizon = 5.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.012,
        hazard_vol: 0.05,
        rate_mean_reversion: 0.10,
        hazard_mean_reversion: 0.08,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, horizon).expect("calibrate");
    let dt = tree.calibrated_dt().expect("dt");
    let (extrema_lo, extrema_hi, _) = tree.scan_correlation_feasibility(dt, None);

    let mut exhaustive_lo = -1.0_f64;
    let mut exhaustive_hi = 1.0_f64;
    for step in 0..steps {
        for rate in tree.calibrated_rates[step].values() {
            let p_rate = RatesCreditTree::mean_reverting_up_prob(
                rate,
                tree.rate_ref,
                tree.config.rate_mean_reversion,
                tree.config.rate_vol,
                dt,
            );
            for hazard in tree.calibrated_hazards[step].values() {
                let p_hazard = RatesCreditTree::mean_reverting_up_prob(
                    hazard,
                    tree.hazard_ref,
                    tree.config.hazard_mean_reversion,
                    tree.config.hazard_vol,
                    dt,
                );
                if let Some((lo, hi)) = RatesCreditTree::node_correlation_range(p_rate, p_hazard) {
                    exhaustive_lo = exhaustive_lo.max(lo);
                    exhaustive_hi = exhaustive_hi.min(hi);
                }
            }
        }
    }
    assert!((extrema_lo - exhaustive_lo).abs() < 1e-14);
    assert!((extrema_hi - exhaustive_hi).abs() < 1e-14);
}

#[test]
fn sampled_paths_are_seeded_weighted_and_antithetic() {
    let steps = 12;
    let horizon = 3.0;
    let times: Vec<f64> = (0..=steps)
        .map(|step| step as f64 * horizon / steps as f64)
        .collect();
    let targets = RatesCreditCalibrationTargets {
        discount_factors: times.iter().map(|&time| (-0.025 * time).exp()).collect(),
        survival_probabilities: times.iter().map(|&time| (-0.015 * time).exp()).collect(),
        times,
        recovery_rate: 0.4,
    };
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.01,
        hazard_vol: 0.01,
        correlation: 1.0,
        ..Default::default()
    });
    tree.calibrate(&targets).expect("calibrate");

    let path = tree.sample_path(42, 7, false).expect("path");
    let replay = tree.sample_path(42, 7, false).expect("replay");
    let antithetic = tree.sample_path(42, 7, true).expect("antithetic");
    assert_eq!(path, replay);
    assert_eq!(path.len(), steps + 1);
    assert_eq!(antithetic.len(), steps + 1);

    for (left, right) in path.iter().zip(&antithetic) {
        assert_eq!(left.step, right.step);
        assert_eq!(left.rate_node + right.rate_node, left.step);
        assert_eq!(left.hazard_node + right.hazard_node, left.step);
        assert!(left.discount_to_next.is_finite() && left.discount_to_next > 0.0);
        assert!((left.survival_to_next + left.default_to_next - 1.0).abs() < 1e-14);
    }
    let terminal = path.last().expect("terminal");
    assert_eq!(terminal.discount_to_next, 1.0);
    assert_eq!(terminal.survival_to_next, 1.0);
    assert_eq!(terminal.default_to_next, 0.0);

    let mut reused = vec![RatesCreditPathState {
        step: usize::MAX,
        time: f64::NAN,
        rate_node: 0,
        hazard_node: 0,
        short_rate: 0.0,
        hazard_rate: 0.0,
        discount_to_next: 0.0,
        survival_to_next: 0.0,
        default_to_next: 0.0,
    }];
    tree.sample_path_into(42, 7, false, &mut reused)
        .expect("reused path");
    assert_eq!(reused, path);

    for (source, mirrored) in [(&path, false), (&antithetic, true)] {
        for (start, end) in [(0, 3), (1, 7), (5, steps), (steps, steps)] {
            let checkpoint = RatesCreditPathCheckpoint::from(&source[start]);
            tree.sample_path_segment_into(42, 7, mirrored, checkpoint, end, &mut reused)
                .expect("resumed segment");
            assert_eq!(reused, source[start..=end]);
        }
    }
}

#[test]
fn sampled_joint_moves_preserve_skewed_correlated_marginals() {
    let steps = 8;
    let horizon = 2.0;
    let disc = sloped_discount_curve();
    let hazard = test_hazard_curve();
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.01,
        hazard_vol: 0.02,
        correlation: 0.20,
        rate_mean_reversion: 0.10,
        hazard_mean_reversion: 0.08,
    });
    calibrate_for_test(&mut tree, &disc, &hazard, horizon).expect("calibrate");

    let transition = tree
        .transition_probabilities(4, 0, 4)
        .expect("skewed interior transition");
    let expected_rate_up = transition.up_up + transition.up_down;
    let expected_hazard_up = transition.up_up + transition.down_up;
    assert!(
        (expected_rate_up - 0.5).abs() > 0.01 || (expected_hazard_up - 0.5).abs() > 0.01,
        "test transition must have a nontrivial skew: {transition:?}"
    );
    assert!(
        (transition.up_up - expected_rate_up * expected_hazard_up).abs() > 1e-3,
        "test transition must carry nonzero correlation: {transition:?}"
    );

    let draws = 50_000_usize;
    let mut rng = PhiloxRng::with_stream(0x5eed, 17);
    let mut uniform = [0.0_f64; 1];
    let mut rate_ups = 0_usize;
    let mut hazard_ups = 0_usize;
    let mut joint_ups = 0_usize;
    for _ in 0..draws {
        rng.fill_u01(&mut uniform);
        let (rate_up, hazard_up) = RatesCreditTree::sampled_moves(transition, uniform[0]);
        rate_ups += usize::from(rate_up);
        hazard_ups += usize::from(hazard_up);
        joint_ups += usize::from(rate_up && hazard_up);
    }
    let empirical_rate_up = rate_ups as f64 / draws as f64;
    let empirical_hazard_up = hazard_ups as f64 / draws as f64;
    let empirical_joint_up = joint_ups as f64 / draws as f64;
    assert!((empirical_rate_up - expected_rate_up).abs() < 0.01);
    assert!((empirical_hazard_up - expected_hazard_up).abs() < 0.01);
    assert!((empirical_joint_up - transition.up_up).abs() < 0.01);
}

// Lattice safety: marginals, correlation feasibility, floor saturation

/// The Fréchet construction preserves both marginals exactly and realizes
/// the requested correlation, including at skewed marginals where the old
/// clamp-and-renormalise formulation silently distorted them.
#[test]
fn joint_probabilities_preserve_marginals_at_skewed_nodes() {
    for &(p_r, p_h) in &[
        (0.5, 0.5),
        (0.2, 0.8),
        (0.12, 0.5),
        (0.875, 0.125),
        (0.35, 0.4),
    ] {
        let (lo, hi) =
            RatesCreditTree::node_correlation_range(p_r, p_h).expect("non-degenerate marginals");
        // Sample inside the feasible interval, including both endpoints.
        for &rho in &[lo, lo * 0.5, 0.0, hi * 0.5, hi] {
            let tree = RatesCreditTree::new(RatesCreditConfig {
                rate_vol: 0.01,
                hazard_vol: 0.20,
                correlation: rho,
                ..RatesCreditConfig::default()
            });
            let (p_uu, p_ud, p_du, p_dd) = tree.joint_probabilities(p_r, p_h);

            for (label, cell) in [("uu", p_uu), ("ud", p_ud), ("du", p_du), ("dd", p_dd)] {
                assert!(
                    cell >= -1e-15,
                    "p_{label} negative at p_r={p_r}, p_h={p_h}, rho={rho}: {cell}"
                );
            }
            let sum = p_uu + p_ud + p_du + p_dd;
            assert!(
                (sum - 1.0).abs() < 1e-14,
                "probabilities must sum to 1: {sum}"
            );
            assert!(
                (p_uu + p_ud - p_r).abs() < 1e-14,
                "rate marginal distorted at p_r={p_r}, p_h={p_h}, rho={rho}: {}",
                p_uu + p_ud
            );
            assert!(
                (p_uu + p_du - p_h).abs() < 1e-14,
                "hazard marginal distorted at p_r={p_r}, p_h={p_h}, rho={rho}: {}",
                p_uu + p_du
            );

            // Realized correlation with ±1 coding.
            let var_r = p_r * (1.0 - p_r);
            let var_h = p_h * (1.0 - p_h);
            let realized = (p_uu - p_r * p_h) / (var_r * var_h).sqrt();
            assert!(
                (realized - rho).abs() < 1e-12,
                "realized correlation {realized} != requested {rho} at \
                 p_r={p_r}, p_h={p_h}"
            );
        }
    }
}

/// Without mean reversion every marginal is ½, so the whole `[-1, 1]`
/// range stays feasible and calibration accepts extreme correlations.
#[test]
fn correlation_is_unconstrained_without_mean_reversion() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    for &rho in &[-1.0, -0.99, 0.0, 0.99, 1.0] {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 30,
            rate_vol: 0.012,
            hazard_vol: 0.05,
            correlation: rho,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, 5.0)
            .unwrap_or_else(|e| panic!("rho={rho} must calibrate without mean reversion: {e}"));
    }
}

/// An infeasible correlation fails at `calibrate()` — before any pricing —
/// and the message carries the offending node, its marginals, and the
/// largest usable |ρ|. A request at that bound then calibrates and prices.
#[test]
fn infeasible_correlation_fails_at_calibration_with_bound() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let ttm = 5.0;

    let strong_reversion = |rho: f64| RatesCreditConfig {
        steps: 40,
        rate_vol: 0.012,
        hazard_vol: 0.05,
        correlation: rho,
        rate_mean_reversion: KAPPA_MAX,
        hazard_mean_reversion: KAPPA_MAX,
    };

    let mut tree = RatesCreditTree::new(strong_reversion(0.9));
    let err = calibrate_for_test(&mut tree, &disc, &haz, ttm)
        .expect_err("rho = 0.9 must be infeasible under strong mean reversion");
    let msg = err.to_string();
    for needle in [
        "rate_credit_correlation",
        "not attainable",
        "step",
        "largest usable",
        "Fréchet",
    ] {
        assert!(msg.contains(needle), "message must mention {needle}: {msg}");
    }

    // The same bound is available programmatically, so a caller can pick a
    // workable correlation without scraping the message. A tree calibrated
    // at zero correlation exposes it; a request inside it then prices.
    let mut probe = RatesCreditTree::new(strong_reversion(0.0));
    calibrate_for_test(&mut probe, &disc, &haz, ttm).expect("probe calibrates");
    let bound = probe.max_feasible_correlation();
    assert!(
        (0.0..1.0).contains(&bound),
        "reported bound {bound} must be a proper fraction"
    );
    assert!(
        msg.contains(&format!("{bound:.4}")),
        "the error must quote the same bound the accessor reports \
         ({bound:.4}): {msg}"
    );

    let mut feasible = RatesCreditTree::new(strong_reversion(bound * 0.95));
    calibrate_for_test(&mut feasible, &disc, &haz, ttm)
        .expect("a correlation inside the reported bound must calibrate");
    feasible
        .price(
            HashMap::<&'static str, f64>::default(),
            ttm,
            &MarketContext::new(),
            &DummyValuator,
        )
        .expect("and must price");
}

/// The floor-saturation diagnostic is silent when the hazard lattice never
/// floors and positive when it does — the signal that a *relative* spread
/// vol was supplied where an *absolute* hazard vol belongs.
#[test]
fn hazard_floor_saturation_reports_binding_floor() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let ttm = 5.0;

    // Hazard ~2-3.5% with a small absolute vol: the lattice stays positive.
    let mut calm = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        hazard_vol: 0.001,
        ..RatesCreditConfig::default()
    });
    calibrate_for_test(&mut calm, &disc, &haz, ttm).expect("calibrate calm");
    let calm_saturation = calm.hazard_floor_saturation();
    assert!(
        !calm_saturation.is_saturated(),
        "a small absolute hazard vol must not floor: {calm_saturation:?}"
    );
    assert_eq!(calm_saturation.unreachable_steps, 0);

    // 0.20 absolute hazard vol against a ~3% hazard is roughly a 600%
    // relative vol: the floor must bind hard and say so.
    let mut violent = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        hazard_vol: 0.20,
        ..RatesCreditConfig::default()
    });
    calibrate_for_test(&mut violent, &disc, &haz, ttm).expect("calibrate violent");
    let violent_saturation = violent.hazard_floor_saturation();
    assert!(
        violent_saturation.is_saturated() && violent_saturation.max_mass_at_floor > 0.25,
        "an absurd absolute hazard vol must report heavy floor saturation: \
         {violent_saturation:?}"
    );
}

/// `1 − (κ·T)²` is the lattice-edge variance-retention ceiling, and `κ·T`
/// — not `κ` alone — is what binds. Realized retention sits at or below the
/// ceiling because the calibrated theta pushes rows further from the
/// reversion reference than the symmetric geometry alone would.
#[test]
fn variance_retention_is_bounded_by_one_minus_kappa_t_squared() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();

    for (kappa, ttm) in [(0.05_f64, 5.0_f64), (0.10, 5.0), (0.15, 5.0), (0.10, 8.0)] {
        let ceiling = 1.0 - (kappa * ttm).powi(2);
        // The bound holds across volatilities and step counts, and the gap
        // to it closes as the lattice widens relative to the theta drift.
        let mut previous_gap = f64::INFINITY;
        for (steps, rate_vol) in [(40usize, 0.012), (100, 0.012), (200, 0.03)] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                rate_vol,
                rate_mean_reversion: kappa,
                ..RatesCreditConfig::default()
            });
            calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");
            let retention = tree.rate_variance_retention();
            let label = format!("kappa={kappa}, T={ttm}, steps={steps}, sigma={rate_vol}");

            assert_eq!(
                retention.total_nodes,
                steps * (steps + 1) / 2,
                "{label}: the scan must visit exactly the nodes backward \
                 induction does"
            );
            assert!(
                !retention.has_clamped_nodes(),
                "{label}: kappa*T = {:.2} < 1 must not clamp: {retention:?}",
                kappa * ttm
            );
            assert!(
                retention.min_retention <= ceiling + 1e-12,
                "{label}: min_retention {} must not exceed the \
                 1 - (kappa*T)^2 ceiling {ceiling}",
                retention.min_retention
            );
            // Non-trivial: the ceiling would be vacuous if retention sat
            // near zero regardless.
            assert!(
                retention.min_retention > 0.5 * ceiling,
                "{label}: min_retention {} should track the ceiling \
                 {ceiling}, not collapse",
                retention.min_retention
            );

            let gap = ceiling - retention.min_retention;
            assert!(
                gap <= previous_gap + 1e-9,
                "{label}: the gap to the ceiling ({gap}) should not widen \
                 as the lattice widens (previous {previous_gap})"
            );
            previous_gap = gap;
        }
    }
}

/// Past `κ·T = 1` the wing marginals clamp: those nodes carry zero
/// conditional variance and, being degenerate Bernoullis, express no
/// correlation at all. `KAPPA_MAX` does not bound this — only the
/// diagnostic reports it.
#[test]
fn long_horizon_clamping_is_reported_not_silent() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();

    // kappa*T = 0.15 * 5 = 0.75 < 1: no clamping.
    let mut short = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        rate_vol: 0.012,
        rate_mean_reversion: KAPPA_MAX,
        ..RatesCreditConfig::default()
    });
    calibrate_for_test(&mut short, &disc, &haz, 5.0).expect("calibrate short");
    assert!(
        !short.rate_variance_retention().has_clamped_nodes(),
        "kappa*T = 0.75 must not clamp: {:?}",
        short.rate_variance_retention()
    );

    // kappa*T = 0.15 * 10 = 1.5 >= 1: the wings clamp, at a kappa the
    // KAPPA_MAX guard accepts. This is the gap the diagnostic closes.
    let mut long = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        rate_vol: 0.012,
        rate_mean_reversion: KAPPA_MAX,
        ..RatesCreditConfig::default()
    });
    calibrate_for_test(&mut long, &disc, &haz, 10.0).expect("calibrate long");
    let retention = long.rate_variance_retention();
    assert!(
        retention.has_clamped_nodes(),
        "kappa*T = 1.5 must clamp the lattice wings: {retention:?}"
    );
    assert_eq!(
        retention.min_retention, 0.0,
        "a clamped marginal has zero conditional variance"
    );
    assert!(
        retention.clamped_share() > 0.0 && retention.clamped_share() < 1.0,
        "clamping must hit the wings, not the whole lattice: {}",
        retention.clamped_share()
    );
}

/// Without mean reversion every marginal is exactly one half, so the
/// diagnostic reports the undistorted limit — and does so for the hazard
/// factor independently of the rate factor.
#[test]
fn variance_retention_is_undistorted_without_mean_reversion() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();

    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        rate_vol: 0.012,
        hazard_vol: 0.01,
        hazard_mean_reversion: 0.08,
        ..RatesCreditConfig::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, 5.0).expect("calibrate");

    assert_eq!(tree.rate_variance_retention(), VarianceRetention::default());
    let hazard = tree.hazard_variance_retention();
    assert!(hazard.total_nodes > 0, "hazard factor must be scanned");
    assert!(
        hazard.min_retention < 1.0 && hazard.min_retention > 0.0,
        "kappa*T = 0.4 must distort without clamping: {hazard:?}"
    );

    let fresh = RatesCreditTree::new(RatesCreditConfig::default());
    assert_eq!(
        fresh.rate_variance_retention(),
        VarianceRetention::default()
    );
    assert_eq!(
        fresh.hazard_variance_retention(),
        VarianceRetention::default()
    );
}

/// Deterministic limits: the zero-vol lattice is the deterministic
/// backward induction, and both vols tending to zero converge to it.
/// There is no separate rate-only or hazard-only tree.
#[test]
fn zero_volatility_factors_are_deterministic_limits() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let ttm = 5.0;
    let ctx = MarketContext::new();

    let price_at = |rate_vol: f64, hazard_vol: f64| -> f64 {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol,
            hazard_vol,
            ..RatesCreditConfig::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");
        tree.price(
            HashMap::<&'static str, f64>::default(),
            ttm,
            &ctx,
            &DummyValuator,
        )
        .expect("price")
    };

    // zero/zero reprices the discount curve (DummyValuator pays 1 and
    // passes continuation through, so the tree price is the ZCB price).
    let deterministic = price_at(0.0, 0.0);
    let market_df = disc.df(ttm);
    assert!(
        (deterministic - market_df).abs() * 10_000.0 < 1.0,
        "zero/zero must reprice the discount curve: tree={deterministic:.8}, \
         market={market_df:.8}"
    );

    // Each single-factor regime reprices the same curve.
    for (label, rate_vol, hazard_vol) in [("nonzero/zero", 0.01, 0.0), ("zero/nonzero", 0.0, 0.02)]
    {
        let price = price_at(rate_vol, hazard_vol);
        assert!(
            price.is_finite() && (price - market_df).abs() * 10_000.0 < 1.0,
            "{label} must reprice the discount curve: {price:.8} vs {market_df:.8}"
        );
    }

    // Both vols tending to zero converge to the zero/zero result.
    let mut previous = f64::INFINITY;
    for scale in [1e-2, 1e-3, 1e-4, 1e-5] {
        let gap = (price_at(0.01 * scale, 0.02 * scale) - deterministic).abs();
        assert!(
            gap <= previous + 1e-12,
            "convergence must be monotone as vols shrink: gap={gap}, previous={previous}"
        );
        previous = gap;
    }
    assert!(
        previous < 1e-9,
        "the vanishing-vol limit must reach the deterministic price, gap={previous}"
    );
}

/// The joint transition probabilities must realize the configured
/// correlation exactly in the moderate regime (no clamping active).
/// With balanced marginals `p_r = p_h = 0.5` and ±1 coding of the
/// up/down moves, E[X] = E[Y] = 0 and Var[X] = Var[Y] = 1, so
/// realized ρ = p_uu + p_dd − p_ud − p_du, and no cell can clamp for
/// any |ρ| ≤ 1 (each cell is 0.25·(1 ± ρ) ∈ [0, 0.5]).
///
/// Note the deliberate limitation this test does NOT cover: at skewed
/// marginals a large |ρ| can exceed the Fréchet bound for two
/// Bernoullis; the cell clamp + renormalisation then reduces the
/// realized correlation (and shifts the marginals) with no diagnostic.
/// See the doc comment on `joint_probabilities`.
#[test]
fn joint_probabilities_realize_configured_correlation_when_unclamped() {
    for &target_rho in &[-0.9, -0.5, 0.0, 0.5, 0.9] {
        let tree = RatesCreditTree::new(RatesCreditConfig {
            rate_vol: 0.01,
            hazard_vol: 0.20,
            correlation: target_rho,
            ..RatesCreditConfig::default()
        });
        let (p_uu, p_ud, p_du, p_dd) = tree.joint_probabilities(0.5, 0.5);

        let sum = p_uu + p_ud + p_du + p_dd;
        assert!((sum - 1.0).abs() < 1e-14, "probs must sum to 1, got {sum}");
        assert!(
            (p_uu + p_ud - 0.5).abs() < 1e-14 && (p_uu + p_du - 0.5).abs() < 1e-14,
            "marginals distorted at rho={target_rho}: pr={}, ph={}",
            p_uu + p_ud,
            p_uu + p_du
        );
        let realized = p_uu + p_dd - p_ud - p_du;
        assert!(
            (realized - target_rho).abs() < 1e-14,
            "realized correlation {realized} != configured {target_rho}"
        );
    }
}

struct DummyValuator;

impl TreeValuator for DummyValuator {
    fn value_at_maturity(&self, _state: &NodeState) -> Result<f64> {
        Ok(1.0)
    }
    fn value_at_node(&self, _state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        Ok(continuation_value)
    }
}

fn test_base_date() -> finstack_quant_core::dates::Date {
    finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
        .expect("valid date")
}

fn sloped_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(test_base_date())
        .knots([
            (0.0, 1.0),
            (1.0, 0.96),
            (2.0, 0.91),
            (3.0, 0.86),
            (5.0, 0.78),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("curve should build")
}

fn test_hazard_curve() -> HazardCurve {
    use finstack_quant_core::market_data::term_structures::ParInterp;
    HazardCurve::builder("TEST-HAZ")
        .base_date(test_base_date())
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (2.0, 0.025), (5.0, 0.03), (10.0, 0.035)])
        .par_interp(ParInterp::Linear)
        .build()
        .expect("hazard curve should build")
}

fn near_zero_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(test_base_date())
        .knots([
            (0.0, 1.0),
            (1.0, (-0.000001_f64).exp()),
            (2.0, (-0.000002_f64).exp()),
            (5.0, (-0.000005_f64).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("curve should build")
}

fn near_zero_hazard_curve() -> HazardCurve {
    use finstack_quant_core::market_data::term_structures::ParInterp;
    HazardCurve::builder("LOW-HAZ")
        .base_date(test_base_date())
        .recovery_rate(0.4)
        .knots([(0.0, 1e-8), (2.0, 1e-8), (5.0, 1e-8)])
        .par_interp(ParInterp::Linear)
        .build()
        .expect("hazard curve should build")
}

/// Valuator that pays step-indexed cashflows under the same survival
/// convention the bond/term-loan valuators use (no recovery, no
/// exercise). Used to probe the node-coupon fold in isolation.
struct SurvivalCashflowValuator {
    cashflows: Vec<f64>,
}

impl TreeValuator for SurvivalCashflowValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        Ok(self.cashflows.get(state.step).copied().unwrap_or(0.0))
    }
    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64> {
        let cash = self.cashflows.get(state.step).copied().unwrap_or(0.0);
        let risky_continuation = state.hazard_rate.map_or(continuation_value, |hazard| {
            (-hazard.max(0.0) * dt).exp() * continuation_value
        });
        Ok(cash + risky_continuation)
    }
}

/// With zero rate volatility every rate node collapses onto the
/// calibrated deterministic path, so the tree-conditional discount
/// factor must equal the market forward discount factor over the same
/// slice interval.
#[test]
fn conditional_discount_factors_match_curve_at_zero_rate_vol() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 40;
    let ttm = 5.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.0,
        hazard_vol: 0.0,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

    let dt = ttm / steps as f64;
    let (n, m) = (16usize, 20usize);
    let market_fwd_df = disc.df(m as f64 * dt) / disc.df(n as f64 * dt);
    let conditional = tree
        .conditional_discount_factors(n, m, ttm)
        .expect("conditional discounting");
    assert_eq!(conditional.len(), n + 1);
    for (i, p) in conditional.iter().enumerate() {
        assert!(
            (p - market_fwd_df).abs() < 1e-9,
            "node {i}: conditional DF {p} must equal market forward DF {market_fwd_df} \
             when the rate factor is deterministic"
        );
    }
}

/// Higher rate nodes must produce smaller conditional discount factors,
/// and the derivation must ignore the OAS entirely (it takes no OAS
/// input — asserted here by construction through the API shape).
#[test]
fn conditional_discount_factors_decrease_in_rate_node() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 40;
    let ttm = 5.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.015,
        hazard_vol: 0.0,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

    let conditional = tree
        .conditional_discount_factors(16, 24, ttm)
        .expect("conditional discounting");
    for pair in conditional.windows(2) {
        assert!(
            pair[1] < pair[0],
            "conditional DF must strictly decrease in the rate node: {conditional:?}"
        );
    }
    assert!(conditional.iter().all(|p| *p > 0.0 && p.is_finite()));
}

/// The razor identity behind the whole floating-reset design: for an
/// increment defined off the slice-interval market forward with identity
/// composition, the fold prices to **zero** when rates and credit are
/// uncorrelated — for any rate vol, hazard vol, and OAS. The increment
/// `N·((1/P − 1) − f·τ)` satisfies
/// `E[D·(1/P − 1 − f·τ)·P] = (DF(n) − DF(m)) − f·τ·DF(m) = 0`, and with
/// `ρ = 0` the survival and OAS factors multiply out node-independently.
/// A non-zero correlation breaks the factorization and must move the
/// value — that is precisely the coupon/discount-survival covariance the
/// milestone exists to capture.
#[test]
fn pure_forward_increment_folds_to_zero_without_correlation() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 40;
    let ttm = 5.0;
    let dt = ttm / steps as f64;
    let (n, m) = (16usize, 20usize);
    let tau = (m - n) as f64 * dt;
    let slice_fwd = (disc.df(n as f64 * dt) / disc.df(m as f64 * dt) - 1.0) / tau;

    let coupon = NodeCoupon {
        reset_step: n,
        payment_step: m,
        accrual: tau,
        notional: 1_000_000.0,
        base_index_rate: slice_fwd,
        base_discount_forward: slice_fwd,
        timing_scale: 1.0,
        params: FloatingRateParams::default(),
    };
    let valuator = SurvivalCashflowValuator {
        cashflows: vec![0.0; steps + 1],
    };

    let price_with_rho = |rho: f64| -> f64 {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.015,
            hazard_vol: 0.01,
            correlation: rho,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
        let mut vars = HashMap::<&'static str, f64>::default();
        vars.insert(short_rate_keys::OAS, 175.0);
        tree.price_with_node_coupons(
            vars,
            ttm,
            &MarketContext::new(),
            &valuator,
            std::slice::from_ref(&coupon),
        )
        .expect("pricing")
    };

    let uncorrelated = price_with_rho(0.0);
    assert!(
        uncorrelated.abs() < 1e-2,
        "pure forward increment must fold to ~zero at rho = 0 (got {uncorrelated})"
    );

    // Positive rate/credit correlation puts the high-coupon states on
    // the low-survival paths, so the coupon-specific covariance is
    // strictly negative — this is the isolated sign of the channel the
    // floating-reset milestone exists to capture. (At the instrument
    // level the total correlation response also carries the opposing
    // risky-discount covariance `E[S·D]` on every booked cashflow,
    // which exists for fixed bonds too.)
    let correlated = price_with_rho(0.5);
    assert!(
        correlated < -1.0,
        "positive correlation must make the folded increment strictly \
         negative (high coupons on low-survival paths), got {correlated}"
    );
}

/// A node-coupon increment paid on an interior slice must have the same
/// value as an otherwise identical deterministic cashflow booked on that
/// slice. In particular, positive hazard applies only through the interval
/// ending at the payment slice; seeding the fold with survival from the
/// payment slice to the next one would default-discount the coupon twice.
#[test]
fn interior_node_coupon_payment_matches_cashflow_under_positive_hazard() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 12;
    let ttm = 3.0;
    let dt = ttm / steps as f64;
    let (n, m) = (2usize, 7usize);
    let tau = (m - n) as f64 * dt;
    let notional = 1_000_000.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.0,
        hazard_vol: 0.0,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
    assert!(tree.hazard_at_node(m, 0).expect("hazard") > 0.0);

    let reset_df = tree
        .conditional_discount_factors(n, m, ttm)
        .expect("conditional discount")[0];
    let node_forward = (1.0 / reset_df - 1.0) / tau;
    let increment = notional * tau * node_forward;
    let coupon = NodeCoupon {
        reset_step: n,
        payment_step: m,
        accrual: tau,
        notional,
        base_index_rate: 0.0,
        base_discount_forward: 0.0,
        timing_scale: 1.0,
        params: FloatingRateParams::default(),
    };

    let mut vars = HashMap::<&'static str, f64>::default();
    vars.insert(short_rate_keys::OAS, 125.0);
    let folded = tree
        .price_with_node_coupons(
            vars.clone(),
            ttm,
            &MarketContext::new(),
            &SurvivalCashflowValuator {
                cashflows: vec![0.0; steps + 1],
            },
            std::slice::from_ref(&coupon),
        )
        .expect("node-coupon price");

    let mut cashflows = vec![0.0; steps + 1];
    cashflows[m] = increment;
    let direct = tree
        .price(
            vars,
            ttm,
            &MarketContext::new(),
            &SurvivalCashflowValuator { cashflows },
        )
        .expect("direct cashflow price");

    assert!(
        (folded - direct).abs() < 1.0e-8,
        "interior payment must carry exactly the same discount and survival as current cash: folded={folded}, direct={direct}"
    );
}

/// The node-coupon path with an empty descriptor slice is exactly the
/// plain pricing path.
#[test]
fn empty_node_coupons_match_plain_price() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 30;
    let ttm = 5.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.012,
        hazard_vol: 0.015,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");

    let mut cashflows = vec![0.0; steps + 1];
    cashflows[steps] = 1_000_000.0;
    let valuator = SurvivalCashflowValuator { cashflows };

    let plain = tree
        .price(
            HashMap::<&'static str, f64>::default(),
            ttm,
            &MarketContext::new(),
            &valuator,
        )
        .expect("plain price");
    let with_empty = tree
        .price_with_node_coupons(
            HashMap::<&'static str, f64>::default(),
            ttm,
            &MarketContext::new(),
            &valuator,
            &[],
        )
        .expect("empty-coupon price");
    assert_eq!(plain, with_empty);
}

/// Descriptor invariants are enforced before any folding happens.
#[test]
fn node_coupon_descriptor_validation_rejects_bad_geometry() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 20;
    let ttm = 5.0;
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.01,
        hazard_vol: 0.0,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibration");
    let valuator = SurvivalCashflowValuator {
        cashflows: vec![0.0; steps + 1],
    };

    let base = NodeCoupon {
        reset_step: 4,
        payment_step: 8,
        accrual: 1.0,
        notional: 100.0,
        base_index_rate: 0.03,
        base_discount_forward: 0.03,
        timing_scale: 1.0,
        params: FloatingRateParams::default(),
    };

    let collapsed = NodeCoupon {
        payment_step: 4,
        ..base.clone()
    };
    let err = tree
        .price_with_node_coupons(
            HashMap::default(),
            ttm,
            &MarketContext::new(),
            &valuator,
            std::slice::from_ref(&collapsed),
        )
        .expect_err("collapsed reset/payment must be rejected");
    assert!(
        err.to_string().contains("too coarse"),
        "unexpected error: {err}"
    );

    let beyond = NodeCoupon {
        payment_step: steps + 1,
        ..base.clone()
    };
    assert!(tree
        .price_with_node_coupons(
            HashMap::default(),
            ttm,
            &MarketContext::new(),
            &valuator,
            std::slice::from_ref(&beyond),
        )
        .is_err());

    let bad_accrual = NodeCoupon {
        accrual: 0.0,
        ..base
    };
    assert!(tree
        .price_with_node_coupons(
            HashMap::default(),
            ttm,
            &MarketContext::new(),
            &valuator,
            std::slice::from_ref(&bad_accrual),
        )
        .is_err());
}

#[test]
fn rates_credit_calibrated_prices_positive() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    // Both factors stochastic: stated explicitly now that the default is
    // deterministic.
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 40,
        rate_vol: 0.01,
        hazard_vol: 0.20,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, 5.0).expect("calibration");

    let ctx = MarketContext::new();
    let vars = HashMap::<&'static str, f64>::default();
    let val = DummyValuator;
    let price = tree.price(vars, 5.0, &ctx, &val).expect("should succeed");
    assert!(price.is_finite() && price > 0.0);
}

#[test]
fn uncalibrated_tree_returns_error() {
    let tree = RatesCreditTree::new(RatesCreditConfig::default());
    let ctx = MarketContext::new();
    let vars = HashMap::<&'static str, f64>::default();
    let val = DummyValuator;
    let result = tree.price(vars, 1.0, &ctx, &val);
    assert!(result.is_err(), "price() without calibrate() must fail");
}

/// Verify that tree-implied ZCB prices at each step match `disc.df(t)` within 1e-6.
///
/// The DummyValuator passes continuation through unchanged and pays 1.0 at
/// maturity, so tree price = ZCB price ≈ disc.df(T) for any number of steps.
#[test]
fn calibration_quality_zcb_repricing() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 60;
    let ttm = 5.0;

    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: 0.01,
        hazard_vol: 0.0, // no hazard vol → pure rate test
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

    let ctx = MarketContext::new();
    let vars = HashMap::<&'static str, f64>::default();
    let val = DummyValuator;
    let tree_price = tree.price(vars, ttm, &ctx, &val).expect("price");
    let market_df = disc.df(ttm);

    let error_bp = (tree_price - market_df).abs() * 10_000.0;
    assert!(
        error_bp < 1.0, // within 1 bp
        "ZCB repricing error = {:.4} bp (tree={:.8}, market={:.8})",
        error_bp,
        tree_price,
        market_df
    );
}

/// Verify that calibrated hazard rates reproduce the hazard curve's survival
/// probabilities at each step, using Arrow-Debreu forward induction on the
/// 1D hazard lattice.
#[test]
fn calibration_quality_survival_matching() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 50;
    let ttm = 5.0;
    let dt = ttm / steps as f64;

    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps,
        hazard_vol: 0.20,
        ..Default::default()
    });
    calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

    // Forward-propagate Arrow-Debreu state prices through the calibrated
    // hazard lattice to compute model survival probability at each step.
    // The non-negative transform is applied here exactly as calibration
    // and `price()` apply it: a negative additive-normal node is not a
    // credit state, and all three passes must agree or the tree stops
    // reproducing the survival curve the valuator actually sees.
    let mut state_prices = vec![1.0_f64]; // Q_h[0] = 1.0

    for k in 0..steps {
        let next_nodes = k + 2;
        let mut next_sp = vec![0.0_f64; next_nodes];
        for j in 0..=k {
            let h_j =
                RatesCreditTree::effective_hazard(tree.calibrated_hazards[k].value_unchecked(j));
            let surv_df = (-h_j * dt).exp();
            let q = state_prices[j];
            // Up move to j+1, down move to j — p = 0.5 each (no mean reversion)
            if j + 1 < next_nodes {
                next_sp[j + 1] += q * surv_df * 0.5;
            }
            next_sp[j] += q * surv_df * 0.5;
        }
        state_prices = next_sp;

        // Model survival probability at step k+1 = sum of state prices
        let model_sp: f64 = state_prices.iter().sum();
        let t = (k + 1) as f64 * dt;
        let market_sp = haz.sp(t);

        let error = (model_sp - market_sp).abs();
        assert!(
            error < 1e-6,
            "Survival mismatch at step {} (t={:.3}): model={:.8}, market={:.8}, err={:.2e}",
            k + 1,
            t,
            model_sp,
            market_sp,
            error
        );
    }
}

#[test]
fn near_zero_rates_with_mean_reversion_price_finitely() {
    let disc = near_zero_discount_curve();
    let haz = near_zero_hazard_curve();
    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 20,
        rate_vol: 0.20,
        hazard_vol: 0.20,
        rate_mean_reversion: 0.001,
        hazard_mean_reversion: 0.001,
        ..Default::default()
    });

    calibrate_for_test(&mut tree, &disc, &haz, 2.0).expect("calibration");

    let price = tree
        .price(
            HashMap::<&'static str, f64>::default(),
            2.0,
            &MarketContext::new(),
            &DummyValuator,
        )
        .expect("pricing should succeed");

    assert!(price.is_finite() && price > 0.0, "price={price}");
}

/// The tree must reprice the input discount curve **with mean reversion
/// active**. The `DummyValuator` pays 1.0 at maturity and passes
/// continuation through unchanged, so the tree price equals the implied
/// ZCB price, which must match `disc.df(T)`.
///
/// On the parent (`e7dd696da`) this test fails by hundreds of bp: the
/// calibration assumed `p = 0.5` while pricing used a different
/// mean-reversion-dependent probability, so the tree no longer repriced
/// the curve once `rate_mean_reversion != 0`.
///
/// κ values are capped at `KAPPA_MAX` (= 0.15) because above that threshold
/// `calibrate()` returns a `Validation` error. The fix is still demonstrated
/// by these values — the parent was off by > 1000 bp even at κ = 0.05.
#[test]
fn calibration_reprices_disc_curve_with_rate_mean_reversion() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let ttm = 5.0;

    for &kappa in &[0.05_f64, 0.10, 0.15] {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 60,
            rate_vol: 0.012,
            hazard_vol: 0.0, // isolate the rate factor
            rate_mean_reversion: kappa,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

        let ctx = MarketContext::new();
        let price = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                ttm,
                &ctx,
                &DummyValuator,
            )
            .expect("price");
        let market_df = disc.df(ttm);
        let error_bp = (price - market_df).abs() * 10_000.0;
        assert!(
            error_bp < 1.0,
            "kappa={kappa}: ZCB repricing error {error_bp:.4} bp \
             (tree={price:.8}, market={market_df:.8})",
        );
    }
}

/// The hazard factor must likewise reprice the survival curve when its own
/// mean reversion is active.
#[test]
fn calibration_reprices_survival_with_hazard_mean_reversion() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 60;
    let ttm = 5.0;
    let dt = ttm / steps as f64;

    // κ values capped at KAPPA_MAX (0.15); above that calibrate() returns
    // a Validation error (see `mean_reversion_above_kappa_max_returns_validation_error`).
    for &kappa in &[0.05_f64, 0.10, 0.15] {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            hazard_vol: 0.20,
            hazard_mean_reversion: kappa,
            ..Default::default()
        });
        calibrate_for_test(&mut tree, &disc, &haz, ttm).expect("calibrate");

        // Forward-propagate Arrow-Debreu state prices through the
        // calibrated hazard lattice using the SAME mean-reversion
        // probability the calibration applied. The summed state prices at
        // step k must equal the market survival probability at t_k.
        let mut state_prices = vec![1.0_f64];
        for k in 0..steps {
            let next_nodes = k + 2;
            let mut next_sp = vec![0.0_f64; next_nodes];
            for j in 0..=k {
                // The raw additive-normal level drives the lattice
                // dynamics (it is what recombines with uniform spacing, so
                // the mean-reversion drift is measured on it); the
                // non-negative transform applies only where the level is
                // read as a credit intensity. Calibration and `price()`
                // make exactly this split.
                let raw_j = tree.hazard_at_node(k, j).expect("node");
                let surv_df = (-RatesCreditTree::effective_hazard(raw_j) * dt).exp();
                let q = state_prices[j];
                let p_up = RatesCreditTree::mean_reverting_up_prob(
                    raw_j,
                    tree.hazard_ref,
                    kappa,
                    0.20,
                    dt,
                );
                if j + 1 < next_nodes {
                    next_sp[j + 1] += q * surv_df * p_up;
                }
                next_sp[j] += q * surv_df * (1.0 - p_up);
            }
            state_prices = next_sp;

            let model_sp: f64 = state_prices.iter().sum();
            let t = (k + 1) as f64 * dt;
            let market_sp = haz.sp(t);
            let error = (model_sp - market_sp).abs();
            assert!(
                error < 1e-6,
                "kappa={kappa}: survival mismatch at step {} (t={t:.3}): \
                 model={model_sp:.8}, market={market_sp:.8}, err={error:.2e}",
                k + 1,
            );
        }
    }
}

/// At **zero** mean reversion the two-factor tree's rate factor must
/// reproduce a standalone Ho-Lee `ShortRateTree` node-for-node: both use
/// the identical additive lattice (`r ± σ√Δt`, `p = ½`, theta calibration).
#[test]
fn rate_factor_matches_short_rate_tree_at_zero_mean_reversion() {
    use super::super::short_rate_tree::{ShortRateTree, ShortRateTreeConfig};

    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();
    let steps = 50;
    let ttm = 5.0;
    let vol = 0.011;

    let mut two_factor = RatesCreditTree::new(RatesCreditConfig {
        steps,
        rate_vol: vol,
        hazard_vol: 0.0,
        rate_mean_reversion: 0.0,
        ..Default::default()
    });
    calibrate_for_test(&mut two_factor, &disc, &haz, ttm).expect("calibrate 2F");

    let mut short_rate = ShortRateTree::new(ShortRateTreeConfig::ho_lee(steps, vol));
    short_rate.calibrate(&disc, ttm).expect("calibrate SR");

    // Compare every calibrated rate node. The terminal row (step == steps)
    // is geometry-only and excluded from both trees' discounting, so the
    // comparison covers the discounting rows 0..steps.
    for step in 0..steps {
        for node in 0..=step {
            let r_2f = two_factor.rate_at_node(step, node).expect("2F node");
            let r_sr = short_rate.rate_at_node(step, node).expect("SR node");
            assert!(
                (r_2f - r_sr).abs() < 1e-10,
                "rate mismatch at (step={step}, node={node}): \
                 two_factor={r_2f:.12}, short_rate={r_sr:.12}",
            );
        }
    }
}

/// Calibration must reject mean-reversion speeds above `KAPPA_MAX` with a
/// `Validation` error that names the offending value, the threshold, and
/// the variance-degradation reason. Values at or below the threshold pass.
#[test]
fn mean_reversion_above_kappa_max_returns_validation_error() {
    let disc = sloped_discount_curve();
    let haz = test_hazard_curve();

    let mut tree_at_limit = RatesCreditTree::new(RatesCreditConfig {
        steps: 20,
        rate_vol: 0.01,
        hazard_vol: 0.20,
        rate_mean_reversion: KAPPA_MAX,
        ..Default::default()
    });
    calibrate_for_test(&mut tree_at_limit, &disc, &haz, 5.0)
        .expect("kappa == KAPPA_MAX must succeed");

    let over_rate = KAPPA_MAX + 0.01;
    let mut tree_over_rate = RatesCreditTree::new(RatesCreditConfig {
        steps: 20,
        rate_vol: 0.01,
        hazard_vol: 0.20,
        rate_mean_reversion: over_rate,
        ..Default::default()
    });
    match calibrate_for_test(&mut tree_over_rate, &disc, &haz, 5.0) {
        Err(Error::Validation(msg)) => {
            assert!(
                msg.contains("rate_mean_reversion"),
                "error message must name the field; got: {msg}"
            );
            assert!(
                msg.contains("HullWhiteTree"),
                "error message must point to HullWhiteTree; got: {msg}"
            );
        }
        other => panic!("expected Validation error for rate κ={over_rate}, got: {other:?}"),
    }

    let over_hazard = KAPPA_MAX + 0.01;
    let mut tree_over_hazard = RatesCreditTree::new(RatesCreditConfig {
        steps: 20,
        rate_vol: 0.01,
        hazard_vol: 0.20,
        hazard_mean_reversion: over_hazard,
        ..Default::default()
    });
    match calibrate_for_test(&mut tree_over_hazard, &disc, &haz, 5.0) {
        Err(Error::Validation(msg)) => {
            assert!(
                msg.contains("hazard_mean_reversion"),
                "error message must name the field; got: {msg}"
            );
            assert!(
                msg.contains("HullWhiteTree"),
                "error message must point to HullWhiteTree; got: {msg}"
            );
        }
        other => panic!("expected Validation error for hazard κ={over_hazard}, got: {other:?}"),
    }
}

/// The moment-matched up-probability is dimensionally coherent and reduces
/// to `½` at zero mean reversion / at the reference level.
#[test]
fn mean_reverting_up_prob_is_coherent() {
    let sigma = 0.01_f64;
    let dt = 0.05_f64;
    let kappa = 0.10_f64;
    let r_ref = 0.03_f64;

    // At the reference level the drift is zero -> p = 1/2.
    let p_at_ref = RatesCreditTree::mean_reverting_up_prob(r_ref, r_ref, kappa, sigma, dt);
    assert!((p_at_ref - 0.5).abs() < 1e-15, "p at ref should be 1/2");

    // Zero mean reversion -> p = 1/2 everywhere.
    let p_no_mr = RatesCreditTree::mean_reverting_up_prob(0.07, r_ref, 0.0, sigma, dt);
    assert!((p_no_mr - 0.5).abs() < 1e-15, "p without MR should be 1/2");

    // Above the reference level the drift is negative -> p < 1/2 (pull
    // down); below -> p > 1/2 (pull up). Symmetric about 1/2.
    let p_high = RatesCreditTree::mean_reverting_up_prob(r_ref + 0.02, r_ref, kappa, sigma, dt);
    let p_low = RatesCreditTree::mean_reverting_up_prob(r_ref - 0.02, r_ref, kappa, sigma, dt);
    assert!(
        p_high < 0.5 && p_low > 0.5,
        "mean reversion must pull toward ref"
    );
    assert!(
        ((p_high - 0.5) + (p_low - 0.5)).abs() < 1e-15,
        "probability must be symmetric about the reference level",
    );

    // Magnitude check: p = 1/2 + mu*sqrt(dt)/(2*sigma), mu = -kappa*(r-r_ref),
    // all in absolute rate units. With the inputs above:
    //   mu = -0.10 * 0.02 = -0.002 (rate/yr)
    //   p  = 0.5 + (-0.002)*sqrt(0.05)/(2*0.01) = 0.5 - 0.0223607...
    let expected = 0.5 + (-kappa * 0.02) * dt.sqrt() / (2.0 * sigma);
    assert!(
        (p_high - expected).abs() < 1e-14,
        "p_high={p_high}, expected={expected}"
    );

    let p_extreme = RatesCreditTree::mean_reverting_up_prob(r_ref + 100.0, r_ref, kappa, sigma, dt);
    assert!(
        (0.0..=1.0).contains(&p_extreme),
        "probability must stay in [0,1]"
    );
}
