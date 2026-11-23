use super::engine::*;
use super::pricer::*;
use super::types::*;
use crate::monte_carlo::pricer::basis::BasisFunctions;
use crate::monte_carlo::process::gbm::GbmProcess;
use crate::types::{OptionMarketParams, OptionType};
use finstack_quant_core::currency::Currency;

use crate::monte_carlo::pricer::basis::{LaguerreBasis, PolynomialBasis};
use crate::monte_carlo::process::gbm::GbmParams;

#[test]
fn lsmc_rejects_oversized_workload_before_allocating_paths_or_schedule() {
    assert!(LsmcConfig::new(usize::MAX / 4, vec![1], 1).is_err());
    assert!(LsmcConfig::every_step(2, usize::MAX).is_err());
    assert!(checked_path_values(400_000, 100, false).is_ok());
    assert!(checked_path_values(400_000, 100, true).is_err());

    let mut config = LsmcConfig::new(16, vec![1], 1).expect("small config");
    config.num_paths = usize::MAX;
    let pricer = LsmcEngine::new(config);
    let process = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
        .map(GbmProcess::new)
        .expect("process");
    assert!(
        pricer
            .generate_paths_with_seed(&process, 100.0, 1.0, 1, pricer.config.seed)
            .is_err()
    );
}

#[test]
fn lsmc_requires_two_independent_estimators_with_or_without_antithetics() {
    let process = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
        .map(GbmProcess::new)
        .expect("process");
    for antithetic in [false, true] {
        for num_paths in [0, 1] {
            assert!(LsmcConfig::new(num_paths, vec![1], 1).is_err());
            assert!(LsmcEngine::gbm_american(num_paths, 1, 42, false, antithetic).is_err());
            // Public fields can be changed after construction; pricing
            // must revalidate before simulating either paths or pairs.
            let mut config = LsmcConfig::new(2, vec![1], 1)
                .expect("two independent estimators")
                .with_antithetic(antithetic);
            config.num_paths = num_paths;
            let seed = config.seed;
            assert!(
                LsmcEngine::new(config)
                    .generate_paths_with_seed(&process, 100.0, 1.0, 1, seed)
                    .is_err()
            );
        }
        assert!(checked_path_values(2, 1, antithetic).is_ok());
    }
}

#[test]
fn test_polynomial_basis() {
    let basis = PolynomialBasis::new(2).expect("valid regression basis");
    let mut out = vec![0.0; 3];

    basis.evaluate(100.0, &mut out);

    assert_eq!(out[0], 1.0);
    assert_eq!(out[1], 100.0);
    assert_eq!(out[2], 10000.0);
}

#[test]
fn test_laguerre_basis() {
    let basis = LaguerreBasis::new(2, 100.0).expect("valid regression basis");
    let mut out = vec![0.0; 3];

    basis.evaluate(100.0, &mut out);

    assert_eq!(out[0], 1.0);
    // L_1(1) = 1 - 1 = 0
    assert_eq!(out[1], 0.0);
}

#[test]
fn test_laguerre_basis_non_standard_strikes() {
    let basis_low = LaguerreBasis::new(2, 1.0).expect("valid regression basis");
    let basis_high = LaguerreBasis::new(2, 1000.0).expect("valid regression basis");
    let mut out_low = vec![0.0; 3];
    let mut out_high = vec![0.0; 3];

    basis_low.evaluate(1.0, &mut out_low);
    basis_high.evaluate(1000.0, &mut out_high);

    // L_1(1) = 0 for both
    assert_eq!(out_low[1], 0.0);
    assert_eq!(out_high[1], 0.0);

    assert_eq!(basis_low.strike(), 1.0);
    assert_eq!(basis_high.strike(), 1000.0);
}

#[test]
fn test_american_put_exercise() {
    let put = AmericanExercise {
        option_type: OptionType::Put,
        strike: 100.0,
    };

    assert_eq!(put.exercise_value(90.0), 10.0);
    assert_eq!(put.exercise_value(110.0), 0.0);
}

#[test]
fn test_american_call_exercise() {
    let call = AmericanExercise {
        option_type: OptionType::Call,
        strike: 100.0,
    };

    assert_eq!(call.exercise_value(110.0), 10.0);
    assert_eq!(call.exercise_value(90.0), 0.0);
}

#[test]
fn american_put_and_call_reject_non_finite_strike() {
    for strike in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
        let put_err = AmericanExercise::new(OptionType::Put, strike).expect_err("put strike");
        assert!(
            put_err.to_string().contains("finite and positive"),
            "unexpected put error for {strike}: {put_err}"
        );
        let call_err = AmericanExercise::new(OptionType::Call, strike).expect_err("call strike");
        assert!(
            call_err.to_string().contains("finite and positive"),
            "unexpected call error for {strike}: {call_err}"
        );
    }
}

#[test]
fn test_lsmc_basic() {
    let exercise_dates = vec![50, 100];
    let config = LsmcConfig::new(1_000, exercise_dates, 100)
        .unwrap()
        .with_seed(42);
    let pricer = LsmcEngine::new(config);

    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
    let put = AmericanExercise {
        option_type: OptionType::Put,
        strike: 100.0,
    };
    let basis = PolynomialBasis::new(2).expect("valid regression basis");

    let result = pricer
        .price(
            &LsmcInputs {
                process: &gbm,
                initial_spot: 100.0,
                time_to_maturity: 1.0,
                num_steps: 100,
                exercise: &put,
                basis: &basis,
                discount_rate: 0.05,
            },
            Currency::USD,
        )
        .expect("LSMC pricing should succeed in test");

    assert!(result.mean.amount() > 0.0);
    assert!(result.mean.amount() < 50.0);
}

#[test]
fn test_lsmc_high_degree_polynomial() {
    // Test with degree-5 polynomial (can be ill-conditioned)
    // Exercises the SVD solver's relative singular-value truncation
    let exercise_dates = vec![25, 50, 75, 100];
    let config = LsmcConfig::new(5_000, exercise_dates, 100)
        .unwrap()
        .with_seed(42);
    let pricer = LsmcEngine::new(config);

    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
    let put = AmericanExercise {
        option_type: OptionType::Put,
        strike: 100.0,
    };

    let basis = PolynomialBasis::new(5).expect("valid regression basis");

    let result = pricer.price(
        &LsmcInputs {
            process: &gbm,
            initial_spot: 80.0,
            time_to_maturity: 1.0,
            num_steps: 100,
            exercise: &put,
            basis: &basis,
            discount_rate: 0.05,
        },
        Currency::USD,
    );

    assert!(result.is_ok());
    let price = result.expect("LSMC pricing should succeed in test");
    assert!(price.mean.amount().is_finite());
    assert!(price.mean.amount() > 0.0);

    println!("High-degree poly LSMC (deep ITM): {}", price.mean);
}

#[test]
fn test_lsmc_extreme_spot_ranges() {
    // Test with paths spanning wide spot range (10 to 1000)
    // This can cause numerical issues with polynomial basis
    let exercise_dates = vec![50, 100];
    let config = LsmcConfig::new(5_000, exercise_dates, 100)
        .unwrap()
        .with_seed(123);
    let pricer = LsmcEngine::new(config);

    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 1.0).unwrap());
    let put = AmericanExercise {
        option_type: OptionType::Put,
        strike: 100.0,
    };
    let basis = PolynomialBasis::new(3).expect("valid regression basis");

    let result = pricer.price(
        &LsmcInputs {
            process: &gbm,
            initial_spot: 100.0,
            time_to_maturity: 1.0,
            num_steps: 100,
            exercise: &put,
            basis: &basis,
            discount_rate: 0.05,
        },
        Currency::USD,
    );

    assert!(result.is_ok());
    let price = result.expect("LSMC pricing should succeed in test");
    assert!(price.mean.amount().is_finite());
    assert!(price.mean.amount() >= 0.0);

    println!("Extreme spot ranges LSMC: {}", price.mean);
}

#[test]
fn test_lsmc_few_itm_paths() {
    // Deep OTM put with few ITM paths
    // Tests regression fallback when insufficient data
    let exercise_dates = vec![50, 100];
    let config = LsmcConfig::new(1_000, exercise_dates, 100)
        .unwrap()
        .with_seed(456);
    let pricer = LsmcEngine::new(config);

    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.05).unwrap());
    let put = AmericanExercise {
        option_type: OptionType::Put,
        strike: 50.0,
    };
    let basis = PolynomialBasis::new(2).expect("valid regression basis");

    let result = pricer.price(
        &LsmcInputs {
            process: &gbm,
            initial_spot: 150.0,
            time_to_maturity: 0.5,
            num_steps: 100,
            exercise: &put,
            basis: &basis,
            discount_rate: 0.05,
        },
        Currency::USD,
    );

    assert!(result.is_ok());
    let price = result.expect("LSMC pricing should succeed in test");
    assert!(price.mean.amount().is_finite());
    assert!(price.mean.amount() >= 0.0);
    assert!(price.mean.amount() < 0.1);

    println!("Few ITM paths LSMC: {}", price.mean);
}

#[test]
fn test_lsmc_insufficient_itm_paths_preserves_continuation() {
    let config = LsmcConfig::new(2, vec![1], 2).unwrap();
    let pricer = LsmcEngine::new(config);
    let exercise = AmericanExercise {
        option_type: OptionType::Call,
        strike: 100.0,
    };
    let basis = PolynomialBasis::new(2).expect("valid regression basis");
    let paths = PathMatrix::from_rows(&[vec![100.0, 110.0, 130.0]]);

    let process =
        GbmProcess::new(crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2).unwrap());
    let inputs = LsmcInputs {
        process: &process,
        initial_spot: 100.0,
        time_to_maturity: 1.0,
        num_steps: 2,
        exercise: &exercise,
        basis: &basis,
        discount_rate: 0.05,
    };
    let TrainingResult {
        values: present_values,
        exercise_boundary,
        ..
    } = pricer
        .train_exercise_policy(&paths, &inputs, true)
        .expect("backward induction should succeed");
    assert_eq!(exercise_boundary.len(), 1);
    assert_eq!(exercise_boundary[0].itm_paths, 1);
    assert_eq!(exercise_boundary[0].exercised_paths, 0);
    assert!(exercise_boundary[0].critical_spot.is_none());
    assert!(exercise_boundary[0].continuation_coeffs.is_empty());

    let expected = 30.0 * (-0.05_f64).exp();
    assert!((present_values[0] - expected).abs() < 1e-12);
}

#[test]
fn test_lsmc_config_accepts_explicit_valuation_exercise() {
    let config =
        LsmcConfig::new(100, vec![0, 10, 20], 100).expect("zero denotes valuation-date exercise");
    assert_eq!(config.exercise_dates, vec![0, 10, 20]);
    assert!(LsmcConfig::new(100, vec![0], 0).is_err());
}

#[test]
fn test_lsmc_config_rejects_date_beyond_num_steps() {
    let err = LsmcConfig::new(100, vec![5, 15, 42], 20)
        .expect_err("should reject date > num_steps")
        .to_string();
    assert!(
        err.contains("42") && err.contains("num_steps=20"),
        "unexpected error message: {err}"
    );
}

#[test]
fn test_lsmc_config_accepts_terminal_date() {
    let cfg = LsmcConfig::new(100, vec![5, 10, 20], 20).expect("terminal date should be accepted");
    assert_eq!(cfg.exercise_dates, vec![5, 10, 20]);
}

#[test]
fn test_two_pass_lsmc_produces_finite_unbiased_price() {
    let exercise_dates = vec![25, 50, 75, 100];
    let config = LsmcConfig::new(2_000, exercise_dates, 100)
        .unwrap()
        .with_seed(42);
    let pricer = LsmcEngine::new(config);
    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
    let put = AmericanExercise::new(OptionType::Put, 100.0).unwrap();
    let basis = PolynomialBasis::new(2).expect("valid regression basis");

    let unbiased = pricer
        .price_unbiased(
            &LsmcInputs {
                process: &gbm,
                initial_spot: 100.0,
                time_to_maturity: 1.0,
                num_steps: 100,
                exercise: &put,
                basis: &basis,
                discount_rate: 0.05,
            },
            Currency::USD,
            /* pricing_seed = */ 4243,
        )
        .expect("two-pass LSMC should succeed");

    assert!(unbiased.mean.amount().is_finite());
    assert!(unbiased.mean.amount() > 0.0);
    assert!(unbiased.mean.amount() < 50.0);
}

#[test]
fn test_price_unbiased_rejects_matching_seeds() {
    let cfg = LsmcConfig::new(100, vec![10], 20).unwrap().with_seed(7);
    let pricer = LsmcEngine::new(cfg);
    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
    let put = AmericanExercise::new(OptionType::Put, 100.0).unwrap();
    let basis = PolynomialBasis::new(2).expect("valid regression basis");

    let result = pricer.price_unbiased(
        &LsmcInputs {
            process: &gbm,
            initial_spot: 100.0,
            time_to_maturity: 1.0,
            num_steps: 20,
            exercise: &put,
            basis: &basis,
            discount_rate: 0.05,
        },
        Currency::USD,
        /* pricing_seed = */ 7,
    );
    assert!(result.is_err());
}

#[test]
fn frozen_policy_replays_time_zero_decision_on_independent_paths() {
    let config = LsmcConfig::new(2, vec![0, 1], 1)
        .unwrap()
        .with_antithetic(false);
    let pricer = LsmcEngine::new(config);
    let exercise = AmericanExercise::new(OptionType::Put, 100.0).unwrap();
    let basis = PolynomialBasis::new(1).expect("valid regression basis");
    let zero_continuation = PathMatrix::from_rows(&vec![vec![90.0, 100.0]; 2]);
    let high_continuation = PathMatrix::from_rows(&vec![vec![90.0, 50.0]; 2]);

    let process = GbmProcess::new(GbmParams::new(0.0, 0.0, 0.2).unwrap());
    let inputs = LsmcInputs {
        process: &process,
        initial_spot: 90.0,
        time_to_maturity: 1.0,
        num_steps: 1,
        exercise: &exercise,
        basis: &basis,
        discount_rate: 0.0,
    };

    for (training, pricing, exercise_at_start, expected) in [
        (&zero_continuation, &high_continuation, true, 10.0),
        (&high_continuation, &zero_continuation, false, 0.0),
    ] {
        let TrainingResult { policy, .. } = pricer
            .train_exercise_policy(training, &inputs, false)
            .unwrap();
        assert_eq!(policy.exercise_at_start, exercise_at_start);
        let values = pricer.apply_policy_to_paths(
            pricing,
            &exercise,
            &basis,
            &policy,
            PolicyTiming {
                discount_rate: 0.0,
                time_to_maturity: 1.0,
                num_steps: 1,
            },
        );
        let result = pricer
            .summarize_present_values(&values, None, Currency::USD)
            .unwrap();
        assert_eq!(result.mean.amount(), expected);
        assert_eq!(result.stderr, 0.0);
    }
}

#[test]
fn test_price_with_policy_rejects_basis_mismatch() {
    let cfg = LsmcConfig::new(500, vec![10], 20).unwrap().with_seed(1);
    let pricer = LsmcEngine::new(cfg);
    let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
    let put = AmericanExercise::new(OptionType::Put, 100.0).unwrap();
    let basis_train = PolynomialBasis::new(2).expect("valid regression basis");
    let basis_price = PolynomialBasis::new(3).expect("valid regression basis");

    let policy = pricer
        .fit_exercise_policy(&LsmcInputs {
            process: &gbm,
            initial_spot: 100.0,
            time_to_maturity: 1.0,
            num_steps: 20,
            exercise: &put,
            basis: &basis_train,
            discount_rate: 0.05,
        })
        .unwrap();

    let err = pricer
        .price_with_policy(
            &LsmcInputs {
                process: &gbm,
                initial_spot: 100.0,
                time_to_maturity: 1.0,
                num_steps: 20,
                exercise: &put,
                basis: &basis_price,
                discount_rate: 0.05,
            },
            &policy,
            Currency::USD,
            999,
        )
        .expect_err("basis mismatch should be rejected");
    assert!(err.to_string().contains("num_basis"));
}

#[test]
fn test_lsmc_tiny_positive_intrinsic_values_are_treated_as_itm() {
    let config = LsmcConfig::new(16, vec![1], 2).unwrap();
    let pricer = LsmcEngine::new(config);
    let exercise = AmericanExercise {
        option_type: OptionType::Call,
        strike: 100.0,
    };
    let basis = PolynomialBasis::new(1).expect("valid regression basis");
    let paths = PathMatrix::from_rows(&vec![vec![100.0, 100.0 + 1.0e-8, 100.0]; 16]);

    let process =
        GbmProcess::new(crate::monte_carlo::process::gbm::GbmParams::new(0.0, 0.0, 0.2).unwrap());
    let inputs = LsmcInputs {
        process: &process,
        initial_spot: 100.0,
        time_to_maturity: 1.0,
        num_steps: 2,
        exercise: &exercise,
        basis: &basis,
        discount_rate: 0.0,
    };
    let TrainingResult {
        values: present_values,
        ..
    } = pricer
        .train_exercise_policy(&paths, &inputs, false)
        .expect("backward induction should succeed");

    for value in present_values {
        assert!(
            (value - 1.0e-8).abs() < 1.0e-14,
            "tiny intrinsic value should trigger exercise instead of being dropped: {value}"
        );
    }
}

#[test]
fn price_gbm_american_put_atm_is_positive() {
    let pricer = LsmcPricer::new(
        Some(1_000),
        Some(42),
        Some(false),
        Some(8),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("GBM American pricer should construct");
    let estimate = pricer
        .price_american(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                dividend_yield: 0.0,
                volatility: 0.2,
                time_to_expiry: 1.0,
            },
            Some(8),
            Some(Currency::USD),
            Some("laguerre"),
            Some(3),
        )
        .expect("American put pricing should succeed");
    assert_eq!(estimate.exercise_boundary.len(), 7);
    assert!(
        estimate
            .exercise_boundary
            .iter()
            .all(|p| p.itm_paths >= p.exercised_paths)
    );
    let estimate = estimate.estimate;
    assert!(estimate.mean.amount() > 0.0);
}

#[test]
fn lsmc_deep_itm_put_respects_intrinsic_floor() {
    let pricer = LsmcPricer::new(
        Some(4_000),
        Some(7),
        Some(false),
        Some(8),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("GBM American pricer should construct");
    let estimate = pricer
        .price_american(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot: 50.0,
                strike: 100.0,
                rate: 0.05,
                dividend_yield: 0.0,
                volatility: 0.2,
                time_to_expiry: 1.0,
            },
            Some(8),
            Some(Currency::USD),
            Some("laguerre"),
            Some(3),
        )
        .expect("deep ITM put should price")
        .estimate;
    let intrinsic = 50.0;
    assert!(
        estimate.mean.amount() + 1e-12 >= intrinsic,
        "price {} below intrinsic {intrinsic}",
        estimate.mean.amount()
    );
}

#[test]
fn lsmc_deep_itm_put_floor_keeps_stderr() {
    let pricer = LsmcPricer::new(
        Some(2_000),
        Some(17),
        Some(false),
        Some(2),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("GBM American pricer should construct");
    let estimate = pricer
        .price_american(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot: 50.0,
                strike: 100.0,
                rate: 0.05,
                dividend_yield: 0.0,
                volatility: 0.2,
                time_to_expiry: 1.0 / 252.0,
            },
            Some(2),
            Some(Currency::USD),
            Some("laguerre"),
            Some(3),
        )
        .expect("deep ITM short-horizon put should price")
        .estimate;
    let intrinsic = 50.0;
    assert_eq!(
        estimate.mean.amount(),
        intrinsic,
        "short-horizon deep ITM put should bind the t=0 intrinsic floor"
    );
    assert!(
        estimate.stderr > 0.0,
        "floored mean should keep MC stderr, got {}",
        estimate.stderr
    );
    assert!(
        estimate.ci_95.0.amount() + 1e-12 >= intrinsic,
        "CI lower {} should be clamped to intrinsic {intrinsic}",
        estimate.ci_95.0.amount()
    );
    assert!(
        estimate.ci_95.0.amount() <= estimate.mean.amount()
            && estimate.mean.amount() <= estimate.ci_95.1.amount(),
        "CI [{}, {}] should contain floored mean {intrinsic}",
        estimate.ci_95.0.amount(),
        estimate.ci_95.1.amount()
    );
}

#[test]
fn lsmc_american_put_is_at_least_european() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let div_yield = 0.0;
    let vol = 0.2;
    let expiry = 1.0;
    let european = crate::closed_form::bs_price(&crate::types::OptionMarketParams {
        spot,
        strike,
        rate,
        dividend_yield: div_yield,
        volatility: vol,
        time_to_expiry: expiry,
        option_type: crate::types::OptionType::Put,
    })
    .expect("valid Black-Scholes inputs");
    let pricer = LsmcPricer::new(
        Some(8_000),
        Some(11),
        Some(false),
        Some(8),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("GBM American pricer should construct");
    let estimate = pricer
        .price_american(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(8),
            Some(Currency::USD),
            Some("laguerre"),
            Some(3),
        )
        .expect("ATM American put should price")
        .estimate;
    let price = estimate.mean.amount();
    let tol = (4.0 * estimate.stderr).max(0.05);
    assert!(
        price + tol >= european,
        "American put {price} below European {european} (stderr={}, tol={tol})",
        estimate.stderr
    );
}

#[test]
fn lsmc_american_call_matches_european_when_q_is_zero() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let div_yield = 0.0;
    let vol = 0.2;
    let expiry = 1.0;
    let european = crate::closed_form::bs_price(&crate::types::OptionMarketParams {
        spot,
        strike,
        rate,
        dividend_yield: div_yield,
        volatility: vol,
        time_to_expiry: expiry,
        option_type: crate::types::OptionType::Call,
    })
    .expect("valid Black-Scholes inputs");
    let pricer = LsmcPricer::new(
        Some(8_000),
        Some(13),
        Some(false),
        Some(8),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("GBM American pricer should construct");
    let estimate = pricer
        .price_american(
            &OptionMarketParams {
                option_type: OptionType::Call,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(8),
            Some(Currency::USD),
            Some("laguerre"),
            Some(3),
        )
        .expect("ATM American call should price")
        .estimate;
    let price = estimate.mean.amount();
    let tol = (4.0 * estimate.stderr).max(0.15);
    assert!(
        (price - european).abs() < tol,
        "q=0 American call {price} vs European {european} (stderr={}, tol={tol})",
        estimate.stderr
    );
}

/// A separate Philox and path implementation with an SVD least-squares fit
/// on centered monomials supplies the seeded reference prices and errors.
/// Their polynomial spans match the trained Laguerre and polynomial bases.
#[test]
fn gbm_american_prices_match_independent_lsmc() {
    let expected = [
        (11.898687341034666, 0.21784542389407813),
        (8.419769043337007, 0.42760430950110856),
        (11.049993079057634, 0.21553979940717297),
        (8.481169729641454, 0.4095871389934612),
    ];
    let pricer = LsmcPricer::new(
        Some(400),
        Some(42),
        Some(false),
        Some(10),
        Some("laguerre"),
        Some(3),
        Some(true),
    )
    .expect("pricer");
    let (spot, strike, rate, div_yield, vol, expiry, num_steps, currency) =
        (100.0, 105.0, 0.05, 0.03, 0.25, 1.0, 10, Currency::USD);
    let estimates = [
        pricer.price_american(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(num_steps),
            Some(currency),
            Some("laguerre"),
            Some(3),
        ),
        pricer.price_american(
            &OptionMarketParams {
                option_type: OptionType::Call,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(num_steps),
            Some(currency),
            Some("polynomial"),
            Some(2),
        ),
        pricer.price_american_unbiased(
            &OptionMarketParams {
                option_type: OptionType::Put,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(num_steps),
            Some(currency),
            Some("laguerre"),
            Some(3),
            99,
        ),
        pricer.price_american_unbiased(
            &OptionMarketParams {
                option_type: OptionType::Call,
                spot,
                strike,
                rate,
                dividend_yield: div_yield,
                volatility: vol,
                time_to_expiry: expiry,
            },
            Some(num_steps),
            Some(currency),
            Some("polynomial"),
            Some(2),
            99,
        ),
    ];
    for (estimate, (price, stderr)) in estimates.into_iter().zip(expected) {
        let estimate = estimate.expect("price").estimate;
        assert!(
            (estimate.mean.amount() - price).abs() <= 1e-12,
            "American price {} vs {price}",
            estimate.mean.amount()
        );
        assert!(
            (estimate.stderr - stderr).abs() <= 1e-13,
            "American stderr {} vs {stderr}",
            estimate.stderr
        );
    }
}
#[test]
fn lsmc_call_config_layers_call_overrides_over_construction() {
    let lsmc = LsmcPricer::new(
        Some(500),
        Some(3),
        Some(false),
        Some(10),
        None,
        Some(2),
        None,
    )
    .expect("valid");
    assert_eq!(lsmc.num_steps(), 10);
    assert_eq!(lsmc.basis_degree(), 2);

    let (pricer, steps, basis, degree) = lsmc.call_config(None, None, None).expect("defaults");
    assert_eq!(pricer.config().num_paths, 500);
    assert_eq!((steps, basis, degree), (10, lsmc.basis(), 2));

    let (_, steps, _, degree) = lsmc
        .call_config(Some(20), None, Some(4))
        .expect("overrides");
    assert_eq!((steps, degree), (20, 4));

    assert!(lsmc.call_config(None, Some("not-a-basis"), None).is_err());
    assert!(LsmcPricer::new(None, None, None, None, Some("nope"), None, None).is_err());
}
