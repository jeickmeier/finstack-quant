use super::cap_floor::solve_cap_floor_sigma_for_fixed_kappa;
use super::pricing::{cap_floor_periods, forward_rate_from_df, normal_caplet_price};
use super::swaption::infer_hw_initial_guess;
use super::targets::{HullWhiteSwaptionTarget, PreparedSwaption, KAPPA_MAX, KAPPA_MIN};
use super::*;
use finstack_quant_models::rates::hull_white::{
    hw1f_caplet_forward_rate_normal_vol, hw1f_convexity_adjustment, hw_b, hw_bond_vol,
    hw_bond_vol_with_model,
};

#[test]
fn swap_frequency_display_matches_serde_contract() {
    assert_eq!(SwapFrequency::Annual.to_string(), "annual");
    assert_eq!(SwapFrequency::SemiAnnual.to_string(), "semi_annual");
    assert_eq!(SwapFrequency::Quarterly.to_string(), "quarterly");
}

/// Flat discount curve at a given continuously compounded rate.
fn flat_df(rate: f64) -> impl Fn(f64) -> f64 {
    move |t: f64| (-rate * t).exp()
}

#[test]
fn cap_floor_conflicting_quotes_keep_all_residuals_in_any_order() {
    let df = flat_df(0.03);
    let strike = ((0.03_f64 * 0.25).exp() - 1.0) / 0.25;
    for vols in [[0.005, 0.015, 0.010], [0.005, 0.010, 0.015]] {
        let quotes: Vec<_> = vols
            .into_iter()
            .map(|volatility| CapFloorQuote {
                maturity: 5.0,
                strike,
                volatility,
                is_cap: true,
                is_normal_vol: true,
            })
            .collect();
        let (_, report) = calibrate_hull_white_to_cap_floors(
            &df,
            &df,
            &quotes,
            CapFloorCalibrationConfig {
                fit_tolerance: 1e-6,
                frequency: SwapFrequency::Quarterly,
                fixed_kappa: Some(0.0342),
                initial_guess: None,
            },
        )
        .expect("least squares fit returns diagnostics");
        assert_eq!(report.residuals.len(), quotes.len());
        assert!(
            !report.success,
            "contradictory observations must fail acceptance"
        );
        let max_residual = report
            .residuals
            .values()
            .map(|r| r.abs())
            .fold(0.0_f64, f64::max);
        assert!((max_residual - 0.005).abs() < 1e-8);
    }
}

#[test]
fn hw_params_validation() {
    assert!(HullWhiteCalibrationParams::new(0.05, 0.01).is_ok());
    assert!(HullWhiteCalibrationParams::new(0.0, 0.01).is_err()); // kappa = 0
    assert!(HullWhiteCalibrationParams::new(-0.1, 0.01).is_err()); // kappa < 0
    assert!(HullWhiteCalibrationParams::new(0.05, 0.0).is_err()); // sigma = 0
    assert!(HullWhiteCalibrationParams::new(0.05, -0.01).is_err()); // sigma < 0
}

#[test]
fn constant_model_params_match_scalar_state_variance() {
    let scalar = HullWhiteCalibrationParams::new(0.05, 0.01).expect("scalar parameters");
    let model = HullWhiteParams::try_from(scalar).expect("constant model");
    let expected = 0.01_f64.powi(2) * (1.0 - (-0.10_f64).exp()) / 0.10;

    assert!((model.state_variance(1.0).expect("variance") - expected).abs() < 1.0e-15);
}

#[test]
fn constant_model_bond_vol_matches_scalar_formula() {
    let scalar = HullWhiteCalibrationParams::new(0.05, 0.01).expect("scalar parameters");
    let model = HullWhiteParams::try_from(scalar).expect("constant model");

    let model_vol = hw_bond_vol_with_model(&model, 0.0, 1.0, 2.0).expect("model vol");
    let scalar_vol = hw_bond_vol(0.05, 0.01, 0.0, 1.0, 2.0);

    assert!((model_vol - scalar_vol).abs() < 1.0e-15);
}

#[test]
fn piecewise_cap_floor_bootstrap_recovers_synthetic_segments() {
    let discount = flat_df(0.03);
    let forward = flat_df(0.03);
    let kappa = 0.05;
    let generated = HullWhiteParams::new(
        kappa,
        PiecewiseConstantCurve::new(vec![0.0, 1.0], vec![0.01, 0.02]).expect("schedule"),
    )
    .expect("model");
    let maturities = [1.0, 2.0];
    let quotes: Vec<CapFloorQuote> = maturities
        .into_iter()
        .map(|maturity| {
            let spec = CapFloorPriceSpec::new(maturity, 0.03, true, SwapFrequency::Quarterly);
            let target = hw1f_cap_floor_price_with_model(&generated, &discount, &forward, spec)
                .expect("model price");
            let normal_vol = BrentSolver::new()
                .tolerance(1.0e-12)
                .solve_in_bracket(
                    |vol| {
                        bachelier_cap_floor_price(
                            &discount,
                            &forward,
                            maturity,
                            0.03,
                            vol,
                            true,
                            SwapFrequency::Quarterly,
                        ) - target
                    },
                    1.0e-8,
                    1.0,
                )
                .expect("implied normal vol");
            CapFloorQuote {
                maturity,
                strike: 0.03,
                volatility: normal_vol,
                is_cap: true,
                is_normal_vol: true,
            }
        })
        .collect();

    let (bootstrapped, _report) = bootstrap_hull_white_sigma_schedule_to_cap_floors(
        &discount,
        &forward,
        &quotes,
        PiecewiseSigmaCalibrationConfig {
            fit_tolerance: 1e-6,
            fixed_kappa: kappa,
            sigma_min: 1.0e-6,
            sigma_max: 0.1,
            frequency: SwapFrequency::Quarterly,
        },
    )
    .expect("bootstrap");

    assert_eq!(bootstrapped.volatility.times(), &[0.0, 1.0]);
    assert!((bootstrapped.volatility.values()[0] - 0.01).abs() < 1.0e-10);
    assert!((bootstrapped.volatility.values()[1] - 0.02).abs() < 1.0e-10);
}

#[test]
fn b_function_properties() {
    let p = HullWhiteCalibrationParams::new(0.1, 0.01).expect("valid");
    let b = hw_b(p.kappa, 0.0, 1.0);
    // B(0, 1) = (1 − e^{−0.1}) / 0.1 ≈ 0.9516
    assert!((b - 0.9516).abs() < 0.001);

    // B should be positive and increasing in (t2 − t1)
    let b_short = hw_b(p.kappa, 0.0, 0.5);
    let b_long = hw_b(p.kappa, 0.0, 2.0);
    assert!(b_short < b);
    assert!(b < b_long);
}

#[test]
fn bond_option_vol_positive() {
    let p = HullWhiteCalibrationParams::new(0.05, 0.01).expect("valid");
    let vol = hw_bond_vol(p.kappa, p.sigma, 0.0, 1.0, 2.0);
    assert!(vol > 0.0, "Bond option vol should be positive: {vol}");
}

#[test]
fn swaption_price_positive() {
    let df_fn = flat_df(0.03);
    let price = hw1f_swaption_price(0.05, 0.01, &df_fn, 1.0, 5.0, 0.03, 2);
    assert!(price > 0.0, "Swaption price should be positive: {price:.6}");
}

#[test]
fn swaption_price_monotone_in_sigma() {
    let df_fn = flat_df(0.03);
    let fwd = {
        let (_, r) = compute_swap_annuity_and_rate(&df_fn, 1.0, 5.0, 2);
        r
    };
    let p_low = hw1f_swaption_price(0.05, 0.005, &df_fn, 1.0, 5.0, fwd, 2);
    let p_high = hw1f_swaption_price(0.05, 0.015, &df_fn, 1.0, 5.0, fwd, 2);
    assert!(
        p_high > p_low,
        "Higher sigma should give higher swaption price: {p_high:.6} vs {p_low:.6}"
    );
}

/// Item 5: under an extreme mean-reversion `kappa` the HW1F r* objective `g(r)`
/// becomes near-flat — every `B(t0,t_i) ≈ 1/kappa` shrinks, so `g'(r) ≈ -Σ c_i/κ`
/// is tiny (~1e-8 at κ=1e8, ~1e-10 at κ=1e10). The pre-fix Newton guard only
/// rejected `|g'| < 1e-15`, so such a derivative passed through and `step = g/g'`
/// exploded to a ~1e8–1e10-scale jump, throwing `r*` to a non-physical value that
/// then poisoned the bond-option strikes and the swaption price.
///
/// Post-fix the safeguarded step bound rejects the explosive Newton step and hands
/// off to the bracketed Brent fallback, so the price stays finite and in the valid
/// `[0, annuity]`-bounded range (a payer swaption can never be worth more than its
/// fixed-leg annuity).
#[test]
fn item5_hw1f_r_star_extreme_kappa_does_not_explode() {
    let df_fn = flat_df(0.03);
    let (annuity, fwd) = compute_swap_annuity_and_rate(&df_fn, 1.0, 5.0, 2);

    for kappa in [1.0e6_f64, 1.0e8, 1.0e10] {
        let price = hw1f_swaption_price(kappa, 0.01, &df_fn, 1.0, 5.0, fwd, 2);
        assert!(
            price.is_finite(),
            "swaption price must stay finite under extreme kappa={kappa:e}; \
             the r* Newton step must not explode (got {price})"
        );
        assert!(
            price >= 0.0,
            "swaption price must be non-negative under extreme kappa={kappa:e}; got {price}"
        );
        // A payer swaption is a portfolio of bond puts; its value cannot exceed the
        // fixed-leg annuity. An exploded r* would blow this bound.
        assert!(
            price <= annuity * 1.0001,
            "swaption price {price} exceeds the annuity bound {annuity} \
             under extreme kappa={kappa:e} — r* likely exploded"
        );
    }
}

#[test]
fn calibrate_hw1f_round_trip() {
    let true_kappa = 0.05;
    let true_sigma = 0.01;
    let rate = 0.03;
    let df_fn = flat_df(rate);
    let ppy = SwapFrequency::SemiAnnual.periods_per_year();

    // Repeated observations still require separate residuals.
    let swaption_specs: Vec<(f64, f64)> = vec![
        (1.0, 5.0),
        (2.0, 5.0),
        (5.0, 5.0),
        (1.0, 10.0),
        (5.0, 10.0),
        (1.0, 5.0),
    ];

    let quotes: Vec<SwaptionQuote> = swaption_specs
        .iter()
        .map(|&(expiry, tenor)| {
            let (annuity, fwd_rate) = compute_swap_annuity_and_rate(&df_fn, expiry, tenor, ppy);
            let model_price =
                hw1f_swaption_price(true_kappa, true_sigma, &df_fn, expiry, tenor, fwd_rate, ppy);

            let normal_vol = if annuity > 1e-15 && expiry > 0.0 {
                let approx_vol =
                    model_price / (annuity * (expiry / (2.0 * std::f64::consts::PI)).sqrt());
                approx_vol.max(1e-6)
            } else {
                0.005
            };

            SwaptionQuote {
                expiry,
                tenor,
                volatility: normal_vol,
                is_normal_vol: true,
            }
        })
        .collect();

    let (params, report) = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::default(),
        None,
        None,
        1e-6,
    )
    .expect("Calibration should succeed");

    assert_eq!(report.residuals.len(), quotes.len());
    assert!(
        report.success,
        "Calibration should succeed: {}",
        report.convergence_reason
    );
    assert!(
        params.kappa > 0.0 && params.kappa < 1.0,
        "kappa should be reasonable: {:.4}",
        params.kappa
    );
    assert!(
        params.sigma > 0.0 && params.sigma < 0.1,
        "sigma should be reasonable: {:.4}",
        params.sigma
    );
}

#[test]
fn calibrate_hw1f_annual_vs_semiannual_produces_different_params() {
    let df_fn = flat_df(0.03);
    let quotes = vec![
        SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.005,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 5.0,
            tenor: 5.0,
            volatility: 0.006,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 10.0,
            tenor: 5.0,
            volatility: 0.005,
            is_normal_vol: true,
        },
    ];

    let (params_semi, _) = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::SemiAnnual,
        None,
        None,
        1e-6,
    )
    .expect("semi-annual");
    let (params_ann, _) =
        calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::Annual, None, None, 1e-6)
            .expect("annual");

    assert!(
        (params_semi.kappa - params_ann.kappa).abs() > 1e-6
            || (params_semi.sigma - params_ann.sigma).abs() > 1e-6,
        "Different frequencies should produce different params: semi={:?} ann={:?}",
        params_semi,
        params_ann
    );
}

#[test]
fn test_hw1f_brent_fallback_extreme_params() {
    let kappa = 5.0;
    let sigma = 0.03;
    let df = flat_df(0.03);

    let price = hw1f_swaption_price(kappa, sigma, &df, 1.0, 5.0, 0.03, 2);
    assert!(
        price.is_finite(),
        "Swaption price should be finite with Brent fallback"
    );
    assert!(price >= 0.0, "Swaption price must be non-negative");
}

#[test]
fn calibrate_hw1f_rejects_insufficient_quotes() {
    let quotes = vec![SwaptionQuote {
        expiry: 1.0,
        tenor: 5.0,
        volatility: 0.005,
        is_normal_vol: true,
    }];
    let df_fn = flat_df(0.03);
    let result = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::default(),
        None,
        None,
        1e-6,
    );
    assert!(result.is_err(), "Should reject < 2 quotes");
}

// HW1F vega-weighted calibration + multi-start

/// Wide-grid round-trip: generate ATM normal vols from a known
/// `(κ*, σ*) = (0.08, 0.012)` on a 10-swaption co-terminal-style
/// grid spanning 1Y to 10Y expiries × 5Y and 10Y tenors, then verify
/// the calibrator recovers κ in a tight neighbourhood of κ*.
///
/// Pre-fix: the **unweighted** price residual let the 10Y×10Y quote
/// (largest annuity → largest price) dominate the objective; the LM
/// solver minimised overall price error by pushing κ toward zero
/// (which widens the long-dated bond-option vol and soaks up most of
/// the residual) at the cost of a 20–30 bp vol error on the 1Y
/// quotes. The vega-weighted residual (post-fix) puts every quote
/// on an implied-vol scale and multi-start escapes the flat κ→0
/// region of the objective surface.
#[test]
fn hw1f_calibration_recovers_kappa_on_wide_round_trip_grid() {
    let true_kappa = 0.08_f64;
    let true_sigma = 0.012_f64;
    let df_fn = flat_df(0.03);
    let ppy = SwapFrequency::SemiAnnual.periods_per_year();

    // 10-swaption co-terminal grid.
    let specs: &[(f64, f64)] = &[
        (1.0, 5.0),
        (2.0, 5.0),
        (3.0, 5.0),
        (5.0, 5.0),
        (7.0, 5.0),
        (10.0, 5.0),
        (1.0, 10.0),
        (3.0, 10.0),
        (5.0, 10.0),
        (10.0, 10.0),
    ];

    // Back out the implied normal vol from the model price so the
    // resulting quotes are internally consistent with (κ*, σ*). Use
    // the Bachelier ATM relation: price ≈ annuity · σ_n · √T / √(2π).
    let quotes: Vec<SwaptionQuote> = specs
        .iter()
        .map(|&(expiry, tenor)| {
            let (annuity, fwd_rate) = compute_swap_annuity_and_rate(&df_fn, expiry, tenor, ppy);
            let model_price =
                hw1f_swaption_price(true_kappa, true_sigma, &df_fn, expiry, tenor, fwd_rate, ppy);
            let vol = model_price / (annuity * (expiry / (2.0 * std::f64::consts::PI)).sqrt());
            SwaptionQuote {
                expiry,
                tenor,
                volatility: vol.max(1e-6),
                is_normal_vol: true,
            }
        })
        .collect();

    let (params, report) = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::SemiAnnual,
        None,
        None,
        1e-6,
    )
    .expect("calibration should succeed");

    assert!(
        report.success,
        "calibration should converge, got: {}",
        report.convergence_reason
    );

    // Recovery tolerance: κ within 20% of the true value — tight
    // enough to fail pre-fix (where the unweighted residual pulled κ
    // into the single-digit-bp range) but permissive enough to
    // accommodate the LM convergence tolerance and multi-start
    // noise.
    assert!(
        (true_kappa * 0.8..=true_kappa * 1.2).contains(&params.kappa),
        "κ = {:.6} not within 20% of κ* = {true_kappa:.6}; \
         pre-fix C8 behaviour was to push κ toward zero on wide \
         expiry grids because the unweighted price residual let \
         long-dated quotes dominate",
        params.kappa
    );
    assert!(
        (true_sigma * 0.5..=true_sigma * 1.5).contains(&params.sigma),
        "σ = {:.6} not within 50% of σ* = {true_sigma:.6}",
        params.sigma
    );
}

/// κ out of bounds `[0.001, 1.0]` must return `Err` rather than a
/// `tracing::warn!`-and-succeed. Use synthetic quotes with
/// inconsistent rate/tenor structure to push the calibration to a
/// pathological κ if it converges at all.
#[test]
fn hw1f_calibration_errors_when_kappa_drives_out_of_bounds() {
    // Construct pathological quotes: essentially flat very low vol
    // across a wide expiry grid. The LM will tend toward κ → 0 +
    // σ → 0; the post-fix implementation should either (a) find a
    // feasible κ in-bounds thanks to multi-start or (b) return an
    // OutOfBounds error. Both outcomes are acceptable; a silent
    // warn-and-return path is NOT.
    let df_fn = flat_df(0.03);
    let quotes: Vec<SwaptionQuote> = (1..=10)
        .map(|i| SwaptionQuote {
            expiry: i as f64,
            tenor: 5.0,
            volatility: 1e-6, // ~0 bp
            is_normal_vol: true,
        })
        .collect();

    let result = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::SemiAnnual,
        None,
        None,
        1e-6,
    );

    match result {
        Ok((params, _)) => {
            assert!(
                (0.001..=1.0).contains(&params.kappa),
                "κ = {:.6} outside hard bounds [0.001, 1.0]; Err expected \
                 rather than a warn-and-succeed path",
                params.kappa
            );
        }
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.contains("κ") || msg.contains("kappa") || msg.contains("bounded"),
                "error message must identify κ-bounds violation: {msg}"
            );
        }
    }
}

#[test]
fn cap_floor_hw1f_calibration_rejects_one_quote_without_fixed_kappa() {
    let df_fn = flat_df(0.03);
    let quotes = vec![CapFloorQuote {
        maturity: 5.0,
        strike: 0.03,
        volatility: 0.0075,
        is_cap: true,
        is_normal_vol: true,
    }];

    let result = calibrate_hull_white_to_cap_floors(
        &df_fn,
        &df_fn,
        &quotes,
        CapFloorCalibrationConfig {
            fit_tolerance: 1e-6,
            frequency: SwapFrequency::SemiAnnual,
            fixed_kappa: None,
            initial_guess: None,
        },
    );

    assert!(
        result.is_err(),
        "one cap/floor quote cannot calibrate both kappa and sigma"
    );
}

/// Item 7: fixed-kappa cap/floor sigma calibration must minimise a residual NORM,
/// not a signed sum. With an inconsistent basket (no single sigma fits every cap),
/// the signed-sum root lets opposite errors cancel and lands on a sigma that is not
/// the least-squares optimum.
///
/// Construct two caps of differing maturity (hence differing vega) and feed market
/// prices generated at *different* sigmas — `0.004` for the short cap, `0.020` for
/// the long cap — so no single sigma reprices both. The calibrated sigma must be the
/// SSE minimiser: `SSE(sigma*)` must be no worse than `SSE` a small step either side,
/// and strictly better than the SSE at the signed-sum root.
#[test]
fn item7_cap_floor_fixed_kappa_minimises_norm_not_signed_sum() {
    let kappa = 0.03_f64;
    let df_fn = flat_df(0.035);
    let frequency = SwapFrequency::Quarterly;

    // Two caps, very different maturities -> very different vega.
    let q_short = CapFloorQuote {
        maturity: 2.0,
        strike: 0.035,
        volatility: 0.0, // unused for price-basket construction below
        is_cap: true,
        is_normal_vol: true,
    };
    let q_long = CapFloorQuote {
        maturity: 10.0,
        strike: 0.035,
        volatility: 0.0,
        is_cap: true,
        is_normal_vol: true,
    };
    let quotes = [q_short, q_long];

    // Inconsistent market prices: short cap priced at sigma=0.004, long at 0.020.
    let spec_short = CapFloorPriceSpec::from_quote(&q_short, frequency);
    let spec_long = CapFloorPriceSpec::from_quote(&q_long, frequency);
    let market = [
        hw1f_cap_floor_price(kappa, 0.004, &df_fn, &df_fn, spec_short),
        hw1f_cap_floor_price(kappa, 0.020, &df_fn, &df_fn, spec_long),
    ];

    let sigma =
        solve_cap_floor_sigma_for_fixed_kappa(kappa, &df_fn, &df_fn, &quotes, &market, frequency)
            .expect("fixed-kappa sigma calibration should succeed");

    // SSE objective replicated locally.
    let sse = |s: f64| -> f64 {
        let r0 = hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_short) - market[0];
        let r1 = hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_long) - market[1];
        r0 * r0 + r1 * r1
    };
    // Signed-sum objective (the pre-fix root-find target).
    let signed_sum = |s: f64| -> f64 {
        (hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_short) - market[0])
            + (hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_long) - market[1])
    };

    // 1. The returned sigma is a genuine SSE minimum (no better point nearby).
    let delta = 1e-4;
    let f_star = sse(sigma);
    assert!(
        f_star <= sse(sigma + delta) && f_star <= sse(sigma - delta),
        "calibrated sigma={sigma} is not an SSE minimum: \
         SSE(sigma)={f_star:.3e}, SSE(+d)={:.3e}, SSE(-d)={:.3e}",
        sse(sigma + delta),
        sse(sigma - delta),
    );

    // 2. Bracket the signed-sum root and confirm it is a DIFFERENT, worse point.
    //    signed_sum is monotone increasing in sigma; bisect for its zero.
    let (mut lo, mut hi) = (1e-8_f64, 1.0_f64);
    if signed_sum(lo) < 0.0 && signed_sum(hi) > 0.0 {
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if signed_sum(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let signed_root = 0.5 * (lo + hi);
        // The signed-sum root cancels opposite errors; its SSE is strictly worse.
        assert!(
            sse(signed_root) > f_star,
            "the signed-sum root sigma={signed_root} has SSE {:.3e} which is not \
             worse than the norm-minimising SSE {f_star:.3e} — the fix did not \
             change behaviour",
            sse(signed_root),
        );
    }
}

#[test]
fn cap_floor_hw1f_calibration_solves_sigma_with_fixed_kappa() {
    let true_kappa = 0.0342;
    let true_sigma = 0.0095;
    let df_fn = flat_df(0.037);
    let quotes = vec![CapFloorQuote {
        maturity: 5.0,
        strike: 0.0365,
        volatility: hw1f_cap_floor_implied_normal_vol(
            true_kappa,
            true_sigma,
            &df_fn,
            &df_fn,
            CapFloorPriceSpec::new(5.0, 0.0365, true, SwapFrequency::Quarterly),
        )
        .expect("implied normal quote"),
        is_cap: true,
        is_normal_vol: true,
    }];

    let (params, report) = calibrate_hull_white_to_cap_floors(
        &df_fn,
        &df_fn,
        &quotes,
        CapFloorCalibrationConfig {
            fit_tolerance: 1e-6,
            fixed_kappa: Some(true_kappa),
            frequency: SwapFrequency::SemiAnnual,
            initial_guess: None,
        },
    )
    .expect("fixed-kappa cap/floor calibration succeeds");

    assert!(report.success, "report should be successful: {report:?}");
    assert!((params.kappa - true_kappa).abs() < 1e-12);
    assert!(
        (params.sigma - true_sigma).abs() < 1e-4,
        "sigma {} should recover true sigma {true_sigma}",
        params.sigma
    );
}

#[test]
fn cap_floor_hw1f_calibration_recovers_two_parameters_on_synthetic_grid() {
    let true_kappa = 0.05;
    let true_sigma = 0.011;
    let df_fn = flat_df(0.035);
    let specs = [(2.0, 0.034), (5.0, 0.036), (7.0, 0.037), (5.0, 0.036)];
    let quotes: Vec<CapFloorQuote> = specs
        .iter()
        .map(|(maturity, strike)| CapFloorQuote {
            maturity: *maturity,
            strike: *strike,
            volatility: hw1f_cap_floor_implied_normal_vol(
                true_kappa,
                true_sigma,
                &df_fn,
                &df_fn,
                CapFloorPriceSpec::new(*maturity, *strike, true, SwapFrequency::Quarterly),
            )
            .expect("implied normal quote"),
            is_cap: true,
            is_normal_vol: true,
        })
        .collect();

    let (params, report) = calibrate_hull_white_to_cap_floors(
        &df_fn,
        &df_fn,
        &quotes,
        CapFloorCalibrationConfig {
            fit_tolerance: 1e-6,
            frequency: SwapFrequency::Quarterly,
            initial_guess: Some(HullWhiteCalibrationParams::new(0.04, 0.01).expect("guess")),
            fixed_kappa: None,
        },
    )
    .expect("two-parameter cap/floor calibration succeeds");

    assert_eq!(report.residuals.len(), quotes.len());
    assert!(report.success, "report should be successful: {report:?}");
    assert!(
        (true_kappa * 0.8..=true_kappa * 1.2).contains(&params.kappa),
        "kappa {} should recover true kappa {true_kappa}",
        params.kappa
    );
    assert!(
        (true_sigma * 0.8..=true_sigma * 1.2).contains(&params.sigma),
        "sigma {} should recover true sigma {true_sigma}",
        params.sigma
    );
}

/// Regression: a non-finite model price for one quote must cause
/// `calculate_residuals` to return `Err` (which the global LM solver
/// converts into a bounded penalty via `fill_penalty`) rather than
/// injecting a magic `1e6` literal directly into the residual buffer.
///
/// Pre-fix, a single bad quote contributed a hard-coded `1e6` as a
/// genuine residual; scaled by `1/vega` it dominated the Gauss-Newton
/// step. Post-fix the buffer is left untouched and the solver applies
/// proper infeasibility handling.
#[test]
fn hw1f_residuals_signal_err_on_non_finite_price_no_magic_literal() {
    // A discount factor closure that returns NaN forces the swaption
    // pricer to produce a non-finite price deterministically.
    let nan_df = |_t: f64| f64::NAN;

    let quotes = vec![
        SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.005,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 5.0,
            tenor: 5.0,
            volatility: 0.006,
            is_normal_vol: true,
        },
    ];

    let prepared: Vec<PreparedSwaption> = quotes
        .iter()
        .map(|_| PreparedSwaption {
            market_price: 0.01,
            fwd_swap_rate: 0.03,
            vega: 0.5,
            schedule: None,
        })
        .collect();

    let target = HullWhiteSwaptionTarget {
        df: &nan_df,
        ppy: SwapFrequency::SemiAnnual.periods_per_year(),
        initial_x0: [(-2.5_f64), (-4.0_f64)],
        prepared,
    };
    let curve = HullWhiteCalibrationParams {
        kappa: 0.08,
        sigma: 0.012,
    };

    // Sentinel buffer: if the bug regressed, the implementation would
    // overwrite an entry with a `1e6`-style literal. We pre-fill with a
    // recognisable marker and assert it is never replaced by a magic
    // residual on the infeasible path.
    let mut residuals = vec![-7.0_f64; quotes.len()];
    let result = target.calculate_residuals(&curve, &quotes, &mut residuals);

    let err = result.expect_err("non-finite price must yield Err, not a 1e6 residual");
    let msg = format!("{err}");
    assert!(
        msg.contains("non-finite") && msg.contains("1Yx5Y"),
        "error must name the offending quote and the failure mode: {msg}"
    );
    // No entry was overwritten with a magic penalty literal: the marker
    // survives, proving `1e6` is no longer treated as a real residual.
    assert!(
        residuals.iter().all(|&r| r == -7.0),
        "residual buffer must not contain an injected magic literal: {residuals:?}"
    );

    // End-to-end: the full calibration with the same NaN curve must
    // fail cleanly rather than silently converge to a poisoned minimum.
    let calib = calibrate_hull_white_to_swaptions(
        &nan_df,
        &quotes,
        SwapFrequency::SemiAnnual,
        None,
        None,
        1e-6,
    );
    assert!(
        calib.is_err() || calib.as_ref().is_ok_and(|(_, report)| !report.success),
        "calibration on a degenerate (NaN-priced) curve must report \
         non-convergence rather than accept a 1e6-dominated minimum"
    );
}

/// W-39: the σ seed must not conflate Bachelier (normal) and Black
/// (lognormal) vol regimes. For NORMAL swaption quotes the quoted vol
/// is already an absolute short-rate-scale vol; multiplying it by the
/// forward swap rate (`avg_fwd ≈ 0.03`) wrongly shrinks it by ~30×,
/// and the `clamp(0.001, …)` floor then masks the bug while still
/// leaving the seed an order of magnitude too small.
#[test]
fn infer_hw_initial_guess_normal_vol_seed_is_right_order_of_magnitude() {
    // A normal-vol swaption set with ~80 bp absolute-rate vol.
    let quotes = vec![
        SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.0080,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 5.0,
            tenor: 5.0,
            volatility: 0.0085,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 10.0,
            tenor: 5.0,
            volatility: 0.0075,
            is_normal_vol: true,
        },
    ];
    let fwd_swap_rates = vec![0.03, 0.032, 0.031];
    let (_kappa, sigma) = infer_hw_initial_guess(&quotes, &fwd_swap_rates);

    // The HW1F σ is an absolute short-rate vol; for normal quotes the
    // seed should track the quoted absolute vol (~80 bp), i.e. land in
    // roughly [3e-3, 3e-2]. The buggy `avg_vol·avg_fwd` product yields
    // ~2.4e-4, clamped up to the 1e-3 floor — still ~8× too small.
    assert!(
        (3e-3..=3e-2).contains(&sigma),
        "normal-vol σ seed out of order of magnitude: {sigma}"
    );
}

/// W-39 companion: for LOGNORMAL quotes the σ seed *should* still
/// multiply by the forward rate, since a Black vol is dimensionless
/// and `vol·fwd` recovers an absolute-rate scale.
#[test]
fn infer_hw_initial_guess_lognormal_vol_seed_uses_forward_rate() {
    // 25% Black vol at a 3% forward → absolute vol ≈ 0.75%.
    let quotes = vec![
        SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.25,
            is_normal_vol: false,
        },
        SwaptionQuote {
            expiry: 5.0,
            tenor: 5.0,
            volatility: 0.25,
            is_normal_vol: false,
        },
    ];
    let fwd_swap_rates = vec![0.03, 0.03];
    let (_kappa, sigma) = infer_hw_initial_guess(&quotes, &fwd_swap_rates);
    // 0.25 · 0.03 = 0.0075 — within the valid σ band.
    assert!(
        (3e-3..=3e-2).contains(&sigma),
        "lognormal-vol σ seed out of order of magnitude: {sigma}"
    );
}

/// M2.17: the HW futures convexity adjustment must reduce to the Ho-Lee
/// limit `½σ²T₁T₂` as κ → 0 — both via the explicit small-κ branch and
/// continuously through it.
#[test]
fn convexity_adjustment_ho_lee_limit() {
    let sigma = 0.01;
    let t1 = 5.0;
    let t2 = 5.25;
    let ho_lee = 0.5 * sigma * sigma * t1 * t2;

    // Explicit small-κ branch.
    let ca_branch = hw1f_convexity_adjustment(1e-12, sigma, t1, t2);
    assert!(
        (ca_branch - ho_lee).abs() < 1e-15,
        "κ→0 branch must equal ½σ²T₁T₂: got {ca_branch}, want {ho_lee}"
    );

    // Continuity across the branch threshold.
    let ca_small = hw1f_convexity_adjustment(1e-6, sigma, t1, t2);
    assert!(
        (ca_small - ho_lee).abs() / ho_lee < 1e-4,
        "full formula at κ=1e-6 must approach the Ho-Lee limit: \
         got {ca_small}, want {ho_lee}"
    );
}

/// M2.17: at realistic parameters (κ=0.03, σ=0.01, T₁=5y eurodollar) the
/// adjustment is ~11–13bp; the previous formula `½σ²B(0,T₁)B(T₁,T₂)`
/// gave ~0.58bp (≈20× understated) because it dropped the `½σ²T₁²` term.
#[test]
fn convexity_adjustment_magnitude_at_realistic_params() {
    let kappa = 0.03;
    let sigma = 0.01;
    let t1 = 5.0;
    let t2 = 5.25;

    let ca = hw1f_convexity_adjustment(kappa, sigma, t1, t2);
    // Below the Ho-Lee bound (mean reversion damps the adjustment)…
    let ho_lee = 0.5 * sigma * sigma * t1 * t2;
    assert!(
        ca < ho_lee,
        "κ>0 must damp the adjustment: {ca} vs {ho_lee}"
    );
    // …but on the same order, not 20× smaller.
    assert!(
        (1.0e-3..1.35e-3).contains(&ca),
        "expected ~11–13bp adjustment, got {ca}"
    );

    // The dropped-term formula for reference: it must NOT match.
    let old = 0.5 * sigma * sigma * hw_b(kappa, 0.0, t1) * hw_b(kappa, t1, t2);
    assert!(
        ca > 10.0 * old,
        "fixed adjustment {ca} should dwarf the old understated value {old}"
    );
}

/// Degenerate inputs return zero adjustment.
#[test]
fn convexity_adjustment_degenerate_inputs() {
    assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 0.0, 0.25), 0.0);
    assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 5.0, 5.0), 0.0);
    assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 5.0, 4.0), 0.0);
}

/// M2.19: non-finite or non-positive discount factors must propagate as
/// NaN — `df.max(1e-12)` silently absorbed NaN (f64::max semantics) and
/// produced a finite forward, defeating the non-finite-price error
/// contract in the calibration residuals.
#[test]
fn forward_rate_from_df_propagates_bad_dfs() {
    let nan_df = |_: f64| f64::NAN;
    assert!(forward_rate_from_df(&nan_df, 0.25, 0.5).is_nan());

    let neg_df = |t: f64| if t > 0.3 { -1.0 } else { 1.0 };
    assert!(forward_rate_from_df(&neg_df, 0.25, 0.5).is_nan());

    let zero_df = |t: f64| if t > 0.3 { 0.0 } else { 1.0 };
    assert!(forward_rate_from_df(&zero_df, 0.25, 0.5).is_nan());

    // Sane curve still produces a sane forward.
    let df_fn = flat_df(0.03);
    let fwd = forward_rate_from_df(&df_fn, 0.25, 0.5);
    assert!((fwd - 0.03).abs() < 1e-3, "flat 3% curve forward: {fwd}");
}

/// M2.18: the spot-start caplet (fixing at t=0, no optionality) is
/// excluded from cap decomposition, and caplet expiry is the fixing time
/// `t_start`, not the payment time `t_end`.
#[test]
fn cap_floor_periods_exclude_spot_caplet_and_expiry_is_fixing_time() {
    let periods: Vec<(f64, f64, f64)> = cap_floor_periods(1.0, SwapFrequency::Quarterly).collect();
    assert_eq!(
        periods.len(),
        3,
        "1y quarterly cap: 3 caplets, spot excluded"
    );
    assert!(
        (periods[0].0 - 0.25).abs() < 1e-12,
        "first included caplet fixes at 0.25, got {}",
        periods[0].0
    );

    // Expiry convention: a cap priced with vol accruing to t_start must be
    // strictly cheaper than the same legs priced to t_end (more variance).
    let df_fn = flat_df(0.03);
    let price = bachelier_cap_floor_price(
        &df_fn,
        &df_fn,
        2.0,
        0.03,
        0.008,
        true,
        SwapFrequency::Quarterly,
    );
    let price_t_end: f64 = cap_floor_periods(2.0, SwapFrequency::Quarterly)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(&df_fn, t_start, t_end);
            normal_caplet_price(forward, 0.03, 0.008, t_end, accrual, df_fn(t_end), true)
        })
        .sum();
    assert!(
        price < price_t_end,
        "fixing-time expiry must price below payment-time expiry: \
         {price} vs {price_t_end}"
    );
    assert!(price > 0.0);
}

/// M2.18: a quote spanning only the (excluded) spot-start caplet is
/// rejected at validation rather than calibrated against a zero price.
#[test]
fn cap_floor_single_period_quote_rejected() {
    let df_fn = flat_df(0.03);
    let quote = CapFloorQuote {
        maturity: 0.25,
        strike: 0.03,
        volatility: 0.008,
        is_cap: true,
        is_normal_vol: true,
    };
    let config = CapFloorCalibrationConfig {
        fit_tolerance: 1e-6,
        frequency: SwapFrequency::Quarterly,
        fixed_kappa: Some(0.05),
        initial_guess: None,
    };
    let result = calibrate_hull_white_to_cap_floors(&df_fn, &df_fn, &[quote], config);
    assert!(
        result.is_err(),
        "single-period cap quote must be rejected, got {:?}",
        result.map(|(p, _)| p)
    );
}

/// ZCB-option caplet pricing satisfies exact cap/floor parity:
/// cap − floor = Σ P_d(0, S_i) · τ_i · (F_i − K).
#[test]
fn hw1f_cap_floor_zcb_option_parity() {
    let df_fn = flat_df(0.03);
    let (kappa, sigma, maturity, strike) = (0.05, 0.012, 5.0, 0.035);
    let frequency = SwapFrequency::Quarterly;
    let cap = hw1f_cap_floor_price(
        kappa,
        sigma,
        &df_fn,
        &df_fn,
        CapFloorPriceSpec::new(maturity, strike, true, frequency),
    );
    let floor = hw1f_cap_floor_price(
        kappa,
        sigma,
        &df_fn,
        &df_fn,
        CapFloorPriceSpec::new(maturity, strike, false, frequency),
    );
    let forward_leg: f64 = cap_floor_periods(maturity, frequency)
        .map(|(t_start, t_end, accrual)| {
            let fwd = forward_rate_from_df(&df_fn, t_start, t_end);
            df_fn(t_end) * accrual * (fwd - strike)
        })
        .sum();
    assert!(
        (cap - floor - forward_leg).abs() < 1e-12,
        "cap/floor parity violated: cap={cap}, floor={floor}, fwd_leg={forward_leg}"
    );
    assert!(cap > 0.0 && floor > 0.0);
}

/// The exact ZCB-put caplet price exceeds the old forward-rate-normal-vol
/// approximation (which understated the caplet vol by ~(1+τF)) and the
/// gap matches the (1+τF) vol scaling to first order.
#[test]
fn hw1f_zcb_option_caplet_prices_above_old_approximation() {
    let df_fn = flat_df(0.04);
    let (kappa, sigma) = (0.05, 0.012);
    let spec = CapFloorPriceSpec::new(5.0, 0.04, true, SwapFrequency::Annual);
    let exact = hw1f_cap_floor_price(kappa, sigma, &df_fn, &df_fn, spec);
    let approx: f64 = cap_floor_periods(spec.maturity, spec.frequency)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(&df_fn, t_start, t_end);
            let hw_vol = hw1f_caplet_forward_rate_normal_vol(kappa, sigma, t_start, accrual);
            normal_caplet_price(
                forward,
                spec.strike,
                hw_vol,
                t_start,
                accrual,
                df_fn(t_end),
                spec.is_cap,
            )
        })
        .sum();
    assert!(
        exact > approx,
        "exact ZCB-option price must exceed the understated approximation: \
         {exact} vs {approx}"
    );
    // The relative gap is on the order of τF ≈ 4% for annual caplets at
    // a 4% forward (ATM vega is linear in vol).
    let rel_gap = (exact - approx) / approx;
    assert!(
        rel_gap > 0.01 && rel_gap < 0.10,
        "expected ~τF vol understatement, got relative price gap {rel_gap}"
    );
}

/// A malformed contractual schedule is rejected rather than silently replaced
/// by a synthetic constant-period schedule.
#[test]
fn swaption_malformed_schedule_is_rejected() {
    let df_fn = flat_df(0.03);
    let quotes = vec![
        SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.007,
            is_normal_vol: true,
        },
        SwaptionQuote {
            expiry: 5.0,
            tenor: 5.0,
            volatility: 0.005,
            is_normal_vol: true,
        },
    ];
    let schedules = vec![
        SwaptionSchedule {
            swap_start_time: 1.0,
            payment_times: (1..=10).map(|index| 1.0 + index as f64 * 0.5).collect(),
            accruals: vec![0.5; 10],
            maturity_time: 6.0,
        },
        SwaptionSchedule {
            swap_start_time: 5.0,
            payment_times: vec![5.5, 6.0],
            accruals: vec![0.5],
            maturity_time: 10.0,
        },
    ];
    let err = calibrate_hull_white_to_swaptions(
        &df_fn,
        &quotes,
        SwapFrequency::Annual,
        Some(&schedules),
        None,
        1e-6,
    )
    .expect_err("a malformed per-quote schedule must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("5Yx5Y") && msg.contains("malformed"),
        "error must name the offending quote and the cause: {msg}"
    );
}

/// Fixed-κ guardrail parity: κ outside the LM box-constraint band is
/// rejected up front.
#[test]
fn cap_floor_fixed_kappa_out_of_band_rejected() {
    let df_fn = flat_df(0.03);
    let quote = CapFloorQuote {
        maturity: 5.0,
        strike: 0.03,
        volatility: 0.008,
        is_cap: true,
        is_normal_vol: true,
    };
    for bad_kappa in [KAPPA_MAX * 2.0, KAPPA_MIN / 2.0] {
        let config = CapFloorCalibrationConfig {
            fit_tolerance: 1e-6,
            frequency: SwapFrequency::Quarterly,
            fixed_kappa: Some(bad_kappa),
            initial_guess: None,
        };
        let result = calibrate_hull_white_to_cap_floors(&df_fn, &df_fn, &[quote], config);
        assert!(
            result.is_err(),
            "fixed_kappa={bad_kappa} outside [{KAPPA_MIN}, {KAPPA_MAX}] must be rejected"
        );
    }
}

/// Quote deserialization rejects unknown fields and invalid values.
#[test]
fn quote_deserialization_validates() {
    // Valid quotes round-trip.
    let q: SwaptionQuote = serde_json::from_str(
        r#"{"expiry": 1.0, "tenor": 5.0, "volatility": 0.006, "is_normal_vol": true}"#,
    )
    .expect("valid swaption quote");
    assert!((q.expiry - 1.0).abs() < 1e-15);
    let c: CapFloorQuote = serde_json::from_str(
        r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.008,
            "is_cap": true, "is_normal_vol": true}"#,
    )
    .expect("valid cap quote");
    assert!((c.maturity - 5.0).abs() < 1e-15);

    // Unknown fields are rejected.
    assert!(serde_json::from_str::<SwaptionQuote>(
        r#"{"expiry": 1.0, "tenor": 5.0, "volatility": 0.006,
            "is_normal_vol": true, "strike": 0.03}"#,
    )
    .is_err());
    assert!(serde_json::from_str::<CapFloorQuote>(
        r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.008,
            "is_cap": true, "is_normal_vol": true, "extra": 1}"#,
    )
    .is_err());

    // Invalid values are rejected at deserialization time.
    assert!(serde_json::from_str::<SwaptionQuote>(
        r#"{"expiry": -1.0, "tenor": 5.0, "volatility": 0.006, "is_normal_vol": true}"#,
    )
    .is_err());
    assert!(serde_json::from_str::<CapFloorQuote>(
        r#"{"maturity": 5.0, "strike": 0.03, "volatility": -0.008,
            "is_cap": true, "is_normal_vol": true}"#,
    )
    .is_err());
    // Lognormal cap/floor quotes are not accepted yet.
    assert!(serde_json::from_str::<CapFloorQuote>(
        r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.2,
            "is_cap": true, "is_normal_vol": false}"#,
    )
    .is_err());
}

#[test]
fn m11_quote_fit_budget_controls_acceptance_without_changing_parameters() {
    let df = flat_df(0.03);
    let strike = ((0.03_f64 * 0.25).exp() - 1.0) / 0.25;
    let quotes: Vec<_> = [0.009, 0.010, 0.011]
        .into_iter()
        .map(|volatility| CapFloorQuote {
            maturity: 5.0,
            strike,
            volatility,
            is_cap: true,
            is_normal_vol: true,
        })
        .collect();
    let fit = |fit_tolerance| {
        calibrate_hull_white_to_cap_floors(
            &df,
            &df,
            &quotes,
            CapFloorCalibrationConfig {
                fit_tolerance,
                frequency: SwapFrequency::Quarterly,
                fixed_kappa: Some(0.05),
                initial_guess: None,
            },
        )
        .expect("fit diagnostics")
    };
    let (strict_params, strict) = fit(0.0005);
    let (accepted_params, accepted) = fit(0.0011);
    assert_eq!(strict_params, accepted_params);
    assert!(!strict.success);
    assert!(accepted.success);
    assert!((accepted.max_residual - 0.001).abs() < 1e-8);
    assert_eq!(accepted.success_tolerance, Some(0.0011));
}
