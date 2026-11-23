use super::config::*;
use super::pricer::*;
use super::sobol_bridge::*;

mod unit_tests {
    use super::{PathDependentPricer, PathDependentPricerConfig};

    #[test]
    fn bridge_keeps_standardized_increments_when_levels_round_together() {
        use finstack_quant_core::math::time_grid::TimeGrid;
        let eta = f64::from_bits(1);
        let one_step = super::SobolBridge::new(&TimeGrid::from_times(vec![0.0, eta]).unwrap());
        let mut single = [0.0];
        for shock in [eta, -eta, 0.0] {
            one_step.fill(&[shock], &mut single).unwrap();
            assert_eq!(single[0], shock);
        }

        let two_steps =
            super::SobolBridge::new(&TimeGrid::from_times(vec![0.0, eta, 2.0 * eta]).unwrap());
        let mut pair = [0.0; 2];
        for (shocks, expected) in [
            ([0.0, eta], [eta, -eta]),
            ([eta, eta], [eta, 0.0]),
            ([0.0, 0.0], [0.0, 0.0]),
        ] {
            two_steps.fill(&shocks, &mut pair).unwrap();
            assert_eq!(pair, expected);
        }

        // A unit terminal shock on this irregular grid gives a midpoint
        // level of eta/2, rounding to zero, and a standard shock of sqrt(eta)/2.
        let irregular = TimeGrid::from_times(vec![0.0, eta, 4.0]).unwrap();
        super::SobolBridge::new(&irregular)
            .fill(&[1.0, 0.0], &mut pair)
            .unwrap();
        assert_eq!(pair, [eta.sqrt() / 2.0, 1.0]);

        // The first two positive-time levels both round to 2. The second
        // interval still has a representable standardized innovation.
        let grid = TimeGrid::from_times(vec![0.0, 1.0, 1.0_f64.next_up(), 4.0]).unwrap();
        let mut increments = [0.0; 3];
        super::SobolBridge::new(&grid)
            .fill(&[1.0, 1.5 / 0.75_f64.sqrt(), 1e-10], &mut increments)
            .unwrap();
        assert!((increments[1] - 1e-10).abs() <= 5e-24);
    }

    #[test]
    fn bridge_splits_actual_intervals_in_depth_first_order() {
        use finstack_quant_core::math::time_grid::TimeGrid;
        let grid = TimeGrid::from_times(vec![0.0, 1.0, 2.0, 5.0, 6.0]).unwrap();
        let mut out = [0.0; 4];
        let bridge = super::SobolBridge::new(&grid);
        bridge.fill(&[0.0, 0.0, 1.0, 2.0], &mut out).unwrap();
        let left = 1.0 / 2.0_f64.sqrt();
        assert_eq!(out, [left, -left, 1.0, -3.0_f64.sqrt()]);
        let equal_steps = TimeGrid::from_times(vec![0.0, 1.0, 2.0]).unwrap();
        assert!(
            super::SobolBridge::new(&equal_steps)
                .fill(&[f64::MAX, f64::MAX], &mut out[..2])
                .is_err()
        );
    }

    #[test]
    fn incomplete_sobol_path_draw_preserves_the_adapter_cursor() {
        use crate::monte_carlo::rng::philox::PhiloxRng;
        use crate::monte_carlo::traits::RandomStream;

        let mut stream = super::SobolPathStream::new(&[1.0, -1.5], PhiloxRng::new(42));
        let mut output = [99.0; 3];
        assert!(stream.fill_std_normals(&mut output).is_err());
        assert_eq!(output, [99.0; 3]);
        stream.fill_std_normals(&mut output[..2]).unwrap();
        assert_eq!(&output[..2], &[1.0, -1.5]);
    }

    #[test]
    fn test_validate_rejects_sobol_plus_parallel() {
        let cfg = PathDependentPricerConfig::new(1_000)
            .with_parallel(true)
            .with_sobol(true);
        let err = cfg
            .validate()
            .expect_err("Sobol + parallel must be rejected");
        assert!(err.to_string().contains("Sobol"));
    }

    #[test]
    fn test_validate_rejects_sobol_plus_antithetic() {
        let cfg = PathDependentPricerConfig::new(1_000)
            .with_parallel(false)
            .with_sobol(true)
            .with_antithetic(true);
        let err = cfg
            .validate()
            .expect_err("Sobol + antithetic must be rejected");
        assert!(err.to_string().contains("antithetic"));
    }

    #[test]
    fn test_validate_requires_two_independent_estimators() {
        for antithetic in [false, true] {
            for num_paths in [0, 1] {
                let cfg = PathDependentPricerConfig::new(num_paths)
                    .with_parallel(false)
                    .with_sobol(false)
                    .with_antithetic(antithetic);
                let err = cfg.validate().expect_err("too few independent estimators");
                assert!(err.to_string().contains("num_paths"));
            }
            PathDependentPricerConfig::new(2)
                .with_parallel(false)
                .with_sobol(false)
                .with_antithetic(antithetic)
                .validate()
                .expect("two estimators, including two antithetic pairs");
        }
        for num_paths in [0, 1, 2] {
            let result = PathDependentPricerConfig::new(num_paths)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(true)
                .validate();
            assert_eq!(result.is_ok(), num_paths >= 2);
        }
    }

    use crate::monte_carlo::engine::PathCaptureConfig;
    use crate::monte_carlo::paths::PathSamplingMethod;
    use crate::monte_carlo::payoff::asian::{Asian, AveragingMethod};
    use crate::monte_carlo::payoff::vanilla::European;
    use crate::monte_carlo::process::gbm::{GbmParams, GbmProcess};
    use crate::types::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::math::random::MAX_SOBOL_DIMENSION;
    use finstack_quant_core::math::time_grid::TimeGrid;

    use crate::monte_carlo::payoff::lookback::Lookback;

    #[test]
    fn sobol_reuses_engine_payoff_and_runtime_validation() {
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.0, 0.0, 0.2)
            .map(GbmProcess::new)
            .unwrap();
        for (paths, maturity_step, discount_factor, message) in [
            (32, 2, 1.0, "event at step 2"),
            (32, 1, -1.0, "discount_factor"),
            (
                crate::monte_carlo::engine::MAX_NUM_PATHS + 1,
                1,
                1.0,
                "maximum",
            ),
        ] {
            let pricer = PathDependentPricer::new(
                PathDependentPricerConfig::new(paths)
                    .with_parallel(false)
                    .with_antithetic(false)
                    .with_sobol(true),
            );
            let payoff = European::new(OptionType::Call, 100.0, 1.0, maturity_step);
            let error = pricer
                .price(
                    &gbm,
                    100.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                        .expect("valid time grid"),
                    &payoff,
                    Currency::USD,
                    discount_factor,
                )
                .expect_err("specialized Sobol pricing must preserve engine validation");
            assert!(error.to_string().contains(message), "{error}");
        }
    }

    #[test]
    fn lrm_rejects_initial_spot_observations_before_simulation() {
        let pricer =
            PathDependentPricer::new(PathDependentPricerConfig::new(2).with_parallel(false));
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.0, 0.0, 0.2)
            .map(GbmProcess::new)
            .unwrap();
        let asian = Asian::new(
            crate::types::OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            vec![0],
        )
        .unwrap();
        let asian_error = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                    .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect_err("a time-zero fixing has an explicit spot derivative");
        let lookback = Lookback::new(crate::types::OptionType::Call, 90.0, 1.0, 1);
        let lookback_error = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 1)
                    .expect("valid time grid"),
                &lookback,
                Currency::USD,
                1.0,
            )
            .expect_err("the initial lookback extremum has an explicit spot derivative");
        for error in [asian_error, lookback_error] {
            assert!(error.to_string().contains("common-random-number"));
        }
    }

    #[test]
    fn test_path_dependent_pricer_asian() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).unwrap());

        // Monthly fixings
        let fixing_steps: Vec<usize> = (0..=12).map(|i| i * 21).collect();
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 252)
                    .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("should succeed");

        assert!(result.mean.amount() > 0.0);
        assert!(result.mean.amount() < 20.0);
    }

    #[test]
    fn test_fully_observed_asian_call_and_put() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(16)
                .with_seed(42)
                .with_parallel(false),
        );
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
            .map(GbmProcess::new)
            .expect("valid GBM parameters");

        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let call = Asian::with_history(
                crate::types::OptionType::Call,
                100.0,
                2.0,
                averaging,
                Vec::new(),
                crate::monte_carlo::payoff::asian::AsianFixingHistory {
                    sum: 220.0,
                    log_sum: 2.0 * 110.0_f64.ln(),
                    count: 2,
                },
            )
            .expect("fully observed call");
            let put = Asian::with_history(
                crate::types::OptionType::Put,
                120.0,
                2.0,
                averaging,
                Vec::new(),
                crate::monte_carlo::payoff::asian::AsianFixingHistory {
                    sum: 220.0,
                    log_sum: 2.0 * 110.0_f64.ln(),
                    count: 2,
                },
            )
            .expect("fully observed put");

            let call_value = pricer
                .price(
                    &gbm,
                    150.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 2)
                        .expect("valid time grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("price from historical call fixings");
            let put_value = pricer
                .price(
                    &gbm,
                    150.0,
                    finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 2)
                        .expect("valid time grid"),
                    &put,
                    Currency::USD,
                    0.95,
                )
                .expect("price from historical put fixings");

            // Every path retains the historical average of 110 through reset;
            // simulated spot levels do not enter an already fixed contract.
            assert!((call_value.mean.amount() - 19.0).abs() < 1e-10);
            assert!((put_value.mean.amount() - 19.0).abs() < 1e-10);
        }
    }

    #[test]
    fn grid_entry_points_preserve_estimates_and_capture_contracts() {
        let gbm = crate::monte_carlo::process::gbm::GbmParams::new(0.05, 0.0, 0.2)
            .map(GbmProcess::new)
            .expect("valid GBM");
        let call = European::new(OptionType::Call, 100.0, 1.0, 4);
        for sobol in [false, true] {
            let config = PathDependentPricerConfig::new(32)
                .with_seed(17)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(sobol);
            let pricer = PathDependentPricer::new(config);
            let estimate = pricer
                .price(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("estimate");
            let result = pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95,
                )
                .expect("result");
            assert_eq!(
                serde_json::to_value(estimate).expect("serialize"),
                serde_json::to_value(result.estimate).expect("serialize")
            );
            assert_eq!(result.run.expect("metadata").seed, Some(17));
        }
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(32)
                .with_parallel(false)
                .with_antithetic(true),
        );
        let estimate = pricer
            .price(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &call,
                Currency::USD,
                0.95,
            )
            .expect("antithetic estimate");
        assert_eq!(estimate.num_simulated_paths, 64);
        assert!(
            pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("grid"),
                    &call,
                    Currency::USD,
                    0.95
                )
                .is_err()
        );
    }

    #[test]
    fn test_sampled_path_capture_prices_and_records_subset() {
        let mut config = PathDependentPricerConfig::new(256)
            .with_seed(42)
            .with_parallel(false)
            .with_antithetic(false);
        config.path_capture = PathCaptureConfig::sample(32, 17);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.02, 0.2).expect("valid GBM parameters"));
        let call = European::new(OptionType::Call, 100.0, 1.0, 4);

        let result = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &call,
                Currency::USD,
                0.95,
            )
            .expect("sampled path capture should succeed");

        assert!(result.estimate.mean.amount().is_finite());
        assert!(result.estimate.mean.amount() > 0.0);
        let captured = result.paths.as_ref().expect("sampled paths should exist");
        assert_eq!(captured.num_paths_total, 256);
        assert_eq!(
            captured.sampling_method,
            PathSamplingMethod::RandomSample {
                count: 32,
                seed: 17,
            }
        );
        assert!(!captured.is_complete());
        assert!(
            (16..=48).contains(&captured.paths.len()),
            "target sample size 32 should yield an approximate sample, got {}",
            captured.paths.len()
        );
        assert!(captured.paths.iter().all(|path| path.points.len() == 5));
    }

    /// LRM Greeks must use the joint path-density score, not the terminal
    /// marginal score (quant . Discriminating case: a
    /// single-fixing "Asian" at step n/2 is exactly a European call on
    /// `S_{T/2}` paid at T, with closed forms
    /// `delta = e^{-rT} e^{(r-q)τ} N(d₁)` and
    /// `vega = e^{-rT} e^{(r-q)τ} S₀ φ(d₁) √τ` (per unit vol), τ = T/2.
    /// The previous terminal-shock implementation converged to ≈ half the
    /// true delta for this payoff.
    #[test]
    fn test_lrm_greeks_unbiased_for_path_dependent_payoff() {
        use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

        let (s0, k, r, q, sigma, t) = (100.0, 100.0, 0.05, 0.01, 0.2, 1.0);
        let num_steps = 10usize;
        let fixing_step = num_steps / 2;
        let tau = t * fixing_step as f64 / num_steps as f64;

        let config = PathDependentPricerConfig::new(100_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(r, q, sigma).unwrap());
        let payoff = Asian::new(
            crate::types::OptionType::Call,
            k,
            1.0,
            AveragingMethod::Arithmetic,
            vec![fixing_step],
        )
        .expect("nonempty fixing schedule");

        let df = (-r * t).exp();
        let greeks = pricer
            .price_with_lrm_greeks_from_payoff(
                &gbm,
                s0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(t, num_steps)
                    .expect("valid time grid"),
                &payoff,
                Currency::USD,
                df,
            )
            .expect("LRM pricing should succeed");
        let (delta, vega) = (greeks.delta.mean, greeks.vega.mean);
        assert_eq!(greeks.delta.num_paths, 100_000);
        assert_eq!(greeks.price.num_paths, 100_000);
        assert!(greeks.delta.stderr > 0.0 && greeks.vega.stderr > 0.0);

        let d1 = ((s0 / k).ln() + (r - q + 0.5 * sigma * sigma) * tau) / (sigma * tau.sqrt());
        let growth = ((r - q) * tau).exp();
        let delta_true = df * growth * norm_cdf(d1).expect("valid Gaussian variate");
        let vega_true =
            df * growth * s0 * norm_pdf(d1).expect("valid Gaussian variate") * tau.sqrt() * 0.01;

        // ≈5σ/8σ statistical tolerances at 100k paths; the old terminal-score
        // estimator was off by ≈50% (≈0.3 absolute in delta), far outside.
        assert!(
            (delta - delta_true).abs() < 0.025,
            "LRM delta {delta} should match closed form {delta_true}"
        );
        assert!(
            (vega - vega_true).abs() < 0.05,
            "LRM vega {vega} should match closed form {vega_true}"
        );
    }

    #[test]
    fn lrm_greeks_reject_degenerate_inputs_and_oversized_capture() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(1_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let price = |spot: f64, vol: f64, expiry: f64, num_steps: usize| {
            pricer.price_with_lrm_greeks(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Call,
                    spot,
                    strike: 100.0,
                    rate: 0.03,
                    dividend_yield: 0.01,
                    volatility: vol,
                    time_to_expiry: expiry,
                },
                Some(num_steps),
                Some(Currency::USD),
            )
        };
        price(100.0, 0.2, 1.0, 12).expect("valid inputs price");
        for (label, result) in [
            ("zero spot", price(0.0, 0.2, 1.0, 12)),
            ("negative spot", price(-100.0, 0.2, 1.0, 12)),
            ("non-finite spot", price(f64::NAN, 0.2, 1.0, 12)),
            ("zero vol", price(100.0, 0.0, 1.0, 12)),
            ("zero expiry", price(100.0, 0.2, 0.0, 12)),
            ("zero steps", price(100.0, 0.2, 1.0, 0)),
        ] {
            assert!(result.is_err(), "{label} must be rejected");
        }

        // 1,000 paths x 4,000 points is exactly the cap; one more step exceeds it.
        let at_cap = super::MAX_LRM_CAPTURED_POINTS / 1_000 - 1;
        let error = price(100.0, 0.2, 1.0, at_cap + 1)
            .expect_err("capture above the cap must be rejected before simulating");
        assert!(
            error.to_string().contains("must not exceed 4000000"),
            "{error}"
        );
        // A step count that overflows `num_steps + 1` is rejected the same way.
        assert!(price(100.0, 0.2, 1.0, usize::MAX).is_err());
    }

    /// Likelihood-ratio Asian Greeks against central common-random-number
    /// bumps of the plain Asian pricer on the same seed. The bump estimates
    /// reuse every shock, so their own sampling error is far below the
    /// likelihood-ratio standard error that sets the tolerance.
    #[test]
    fn lrm_asian_greeks_match_common_random_number_bumps() {
        let (spot, strike, rate, div_yield, vol, expiry) = (100.0, 100.0, 0.04, 0.01, 0.25, 1.0);
        let num_steps = 12;
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(100_000)
                .with_seed(7)
                .with_parallel(false),
        );
        for option_type in [
            crate::types::OptionType::Call,
            crate::types::OptionType::Put,
        ] {
            let plain = |spot: f64, vol: f64| -> f64 {
                pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type,
                            spot,
                            strike,
                            rate,
                            dividend_yield: div_yield,
                            volatility: vol,
                            time_to_expiry: expiry,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("plain Asian price")
                    .mean
                    .amount()
            };
            let greeks = pricer
                .price_with_lrm_greeks(
                    &crate::types::OptionMarketParams {
                        option_type,
                        spot,
                        strike,
                        rate,
                        dividend_yield: div_yield,
                        volatility: vol,
                        time_to_expiry: expiry,
                    },
                    Some(num_steps),
                    Some(Currency::USD),
                )
                .expect("LRM Asian price");

            // Capturing paths must not change the price estimate.
            assert_eq!(
                greeks.price.mean.amount().to_bits(),
                plain(spot, vol).to_bits(),
                "{option_type:?} price"
            );

            let spot_bump = 0.5;
            let bump_delta =
                (plain(spot + spot_bump, vol) - plain(spot - spot_bump, vol)) / (2.0 * spot_bump);
            // Per volatility point: a 0.01 bump each way, divided by two points.
            let bump_vega = (plain(spot, vol + 0.01) - plain(spot, vol - 0.01)) / 2.0;

            assert!(greeks.delta.stderr > 0.0 && greeks.vega.stderr > 0.0);
            assert!(
                (greeks.delta.mean - bump_delta).abs() < 5.0 * greeks.delta.stderr,
                "{option_type:?} delta: LRM {} +/- {}, bump {bump_delta}",
                greeks.delta.mean,
                greeks.delta.stderr
            );
            assert!(
                (greeks.vega.mean - bump_vega).abs() < 5.0 * greeks.vega.stderr,
                "{option_type:?} vega: LRM {} +/- {}, bump {bump_vega}",
                greeks.vega.mean,
                greeks.vega.stderr
            );
        }
    }

    /// For this seed and path budget, compare the observed replicate spread
    /// and price error against the Black-Scholes closed form.
    #[test]
    fn test_sobol_rqmc_stderr_covers_true_error() {
        let (s0, k, r, q, sigma, t) = (100.0, 100.0, 0.05, 0.0, 0.2, 1.0);
        let num_steps = 16usize;
        let config = PathDependentPricerConfig::new(16_384)
            .with_seed(42)
            .with_parallel(false)
            .with_sobol(true);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(r, q, sigma).unwrap());
        let payoff = European::new(OptionType::Call, k, 1.0, num_steps);
        let df = (-r * t).exp();

        let result = pricer
            .price_with_paths_and_grid(
                &gbm,
                s0,
                TimeGrid::uniform(t, num_steps).expect("grid"),
                &payoff,
                Currency::USD,
                df,
            )
            .expect("sobol pricing should succeed");

        let bs = crate::closed_form::bs_price(&crate::types::OptionMarketParams {
            spot: s0,
            strike: k,
            rate: r,
            dividend_yield: q,
            volatility: sigma,
            time_to_expiry: t,
            option_type: crate::types::OptionType::Call,
        })
        .expect("valid Black-Scholes inputs");
        let mean = result.estimate.mean.amount();
        let stderr = result.estimate.stderr;

        assert!(stderr.is_finite() && stderr > 0.0, "stderr = {stderr}");
        assert!(
            (mean - bs).abs() < 6.0 * stderr,
            "RQMC mean {mean} should match BS {bs} within 6×stderr = {}",
            6.0 * stderr
        );
        // QMC should beat the plain-MC error scale at this path count.
        assert!(
            stderr < 0.05,
            "RQMC stderr should be tight at 16k paths, got {stderr}"
        );
    }

    #[test]
    fn test_path_dependent_pricer_lookback() {
        let config = PathDependentPricerConfig::new(10_000)
            .with_seed(42)
            .with_parallel(false);
        let pricer = PathDependentPricer::new(config);

        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.3).unwrap());
        let lookback = Lookback::new(crate::types::OptionType::Call, 100.0, 1.0, 252);

        let result = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(1.0, 252)
                    .expect("valid time grid"),
                &lookback,
                Currency::USD,
                1.0,
            )
            .expect("should succeed");

        assert!(result.mean.amount() > 0.0);
    }

    #[test]
    fn test_sobol_price_custom_grid_multiple_paths() {
        let config = PathDependentPricerConfig::new(8)
            .with_seed(7)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let time_grid = TimeGrid::uniform(1.0, 4).expect("grid should build");
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
            .expect("Sobol pricing should succeed for multiple paths");

        assert_eq!(result.num_paths, 8);
    }

    #[test]
    fn test_sobol_price_with_paths_multiple_paths() {
        fn interpolated_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
            let rank = percentile * (sorted_values.len() - 1) as f64;
            let lower = rank.floor() as usize;
            let upper = rank.ceil() as usize;
            if lower == upper {
                sorted_values[lower]
            } else {
                let weight = rank - lower as f64;
                sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight
            }
        }

        let mut config = PathDependentPricerConfig::new(8)
            .with_seed(11)
            .with_parallel(false)
            .with_sobol(true);
        config.path_capture = PathCaptureConfig::all();
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![1, 2, 3, 4];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let first = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("Sobol path capture should succeed for multiple paths");
        let second = pricer
            .price_with_paths_and_grid(
                &gbm,
                100.0,
                TimeGrid::uniform(1.0, 4).expect("grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect("repeated Sobol path capture should succeed");

        assert_eq!(first.estimate.num_paths, 8);
        assert_eq!(first.paths.as_ref().map(|p| p.paths.len()).unwrap_or(0), 8);

        let captured = first.paths.as_ref().expect("paths should be captured");
        let repeated = second
            .paths
            .as_ref()
            .expect("repeated paths should be captured");
        assert_eq!(captured.paths.len(), repeated.paths.len());
        for (path_a, path_b) in captured.paths.iter().zip(&repeated.paths) {
            assert_eq!(path_a.path_id, path_b.path_id);
            assert_eq!(path_a.final_value.to_bits(), path_b.final_value.to_bits());
            assert_eq!(path_a.points.len(), path_b.points.len());
            for (point_a, point_b) in path_a.points.iter().zip(&path_b.points) {
                assert_eq!(point_a.step, point_b.step);
                assert_eq!(point_a.time.to_bits(), point_b.time.to_bits());
                assert_eq!(point_a.state, point_b.state);
                assert_eq!(point_a.payoff_value, point_b.payoff_value);
                assert_eq!(point_a.cashflows, point_b.cashflows);
            }
        }

        let mut final_values: Vec<f64> =
            captured.paths.iter().map(|path| path.final_value).collect();
        final_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = final_values.len();
        let expected_median = if len.is_multiple_of(2) {
            (final_values[len / 2 - 1] + final_values[len / 2]) / 2.0
        } else {
            final_values[len / 2]
        };
        let expected_p25 = interpolated_percentile(&final_values, 0.25);
        let expected_p75 = interpolated_percentile(&final_values, 0.75);

        assert_eq!(first.estimate.median, Some(expected_median));
        assert_eq!(first.estimate.percentile_25, Some(expected_p25));
        assert_eq!(first.estimate.percentile_75, Some(expected_p75));
        assert_eq!(first.estimate.min, Some(final_values[0]));
        assert_eq!(first.estimate.max, Some(final_values[len - 1]));
    }

    #[test]
    fn test_sobol_brownian_bridge_supports_irregular_grid() {
        let config = PathDependentPricerConfig::new(8)
            .with_seed(13)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(true);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let time_grid = TimeGrid::from_times(vec![0.0, 0.2, 0.55, 1.0]).expect("grid should build");
        let fixing_steps = vec![1, 2, 3];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let result = pricer
            .price(&gbm, 100.0, time_grid, &asian, Currency::USD, 1.0)
            .expect("Irregular-grid Sobol Brownian bridge pricing should succeed");

        assert_eq!(result.num_paths, 8);
    }

    #[test]
    fn test_sobol_rejects_excessive_dimension() {
        let config = PathDependentPricerConfig::new(2)
            .with_seed(17)
            .with_parallel(false)
            .with_sobol(true)
            .with_brownian_bridge(false);
        let pricer = PathDependentPricer::new(config);
        let gbm = GbmProcess::new(GbmParams::new(0.05, 0.0, 0.2).unwrap());
        let fixing_steps = vec![MAX_SOBOL_DIMENSION + 1];
        let asian = Asian::new(
            crate::types::OptionType::Call,
            100.0,
            1.0,
            AveragingMethod::Arithmetic,
            fixing_steps,
        )
        .expect("nonempty fixing schedule");

        let err = pricer
            .price(
                &gbm,
                100.0,
                finstack_quant_core::math::time_grid::TimeGrid::uniform(
                    1.0,
                    MAX_SOBOL_DIMENSION + 1,
                )
                .expect("valid time grid"),
                &asian,
                Currency::USD,
                1.0,
            )
            .expect_err("excessive Sobol dimension should be rejected");

        assert!(err.to_string().contains("Sobol"));
    }

    #[test]
    fn price_gbm_asian_call_atm_is_positive() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(2_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let estimate = pricer
            .price_asian(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Call,
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.05,
                    dividend_yield: 0.0,
                    volatility: 0.2,
                    time_to_expiry: 1.0,
                },
                Some(12),
                Some(Currency::USD),
            )
            .expect("Asian call pricing should succeed");
        assert!(estimate.mean.amount() > 0.0);
    }

    #[test]
    fn price_gbm_asian_put_atm_is_positive() {
        let pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(2_000)
                .with_seed(42)
                .with_parallel(false),
        );
        let estimate = pricer
            .price_asian(
                &crate::types::OptionMarketParams {
                    option_type: crate::types::OptionType::Put,
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.05,
                    dividend_yield: 0.0,
                    volatility: 0.2,
                    time_to_expiry: 1.0,
                },
                Some(12),
                Some(Currency::USD),
            )
            .expect("Asian put pricing should succeed");
        assert!(estimate.mean.amount() > 0.0);
    }

    /// The references use an independent Philox-4x32-10 and reflected Box–Muller
    /// implementation, direct GBM averaging, and two-pass sample standard errors.
    /// They cover call/put, ordinary/antithetic sampling, and both joint-density scores.
    #[test]
    fn seeded_gbm_asian_prices_and_scores_match_independent_reference() {
        const EXPECTED: [f64; 36] = [
            6.917895688676028,
            0.4987205883794049,
            6.351896473050624,
            0.3701560025674384,
            12.442001387555337,
            0.6525223654989073,
            2.730854558423357,
            0.23512676151583928,
            2.425365368573234,
            0.29871951989664663,
            15.77033155046684,
            0.5328260622224886,
            7.695843219801653,
            0.2743338158588275,
            5.756545624462204,
            0.17678004245987322,
            13.926293553001408,
            0.2550450012457795,
            2.3766187264277625,
            0.1423359093765034,
            2.3505780520822483,
            0.17955158417434333,
            14.852697294079867,
            0.09422589104155085,
            7.2652723643646695,
            0.6738587120298595,
            0.5737309142702524,
            0.11179889485760279,
            0.2844740978108626,
            0.13266242988559637,
            5.614876535584638,
            0.5226074174710704,
            -0.47141690055608493,
            0.092812477014451,
            0.501221875748187,
            0.14008795855369172,
        ];
        let mut actual = Vec::new();
        for antithetic in [false, true] {
            let pricer = PathDependentPricer::new(
                PathDependentPricerConfig::new(512)
                    .with_seed(7)
                    .with_parallel(false)
                    .with_antithetic(antithetic),
            );
            for (strike, num_steps) in [(100.0, 12), (90.0, 5), (115.0, 30)] {
                let call = pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type: crate::types::OptionType::Call,
                            spot: 100.0,
                            strike,
                            rate: 0.05,
                            dividend_yield: 0.02,
                            volatility: 0.25,
                            time_to_expiry: 1.0,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("call");
                let put = pricer
                    .price_asian(
                        &crate::types::OptionMarketParams {
                            option_type: crate::types::OptionType::Put,
                            spot: 100.0,
                            strike,
                            rate: 0.05,
                            dividend_yield: 0.02,
                            volatility: 0.25,
                            time_to_expiry: 1.0,
                        },
                        Some(num_steps),
                        Some(Currency::USD),
                    )
                    .expect("put");
                for estimate in [call, put] {
                    actual.push(estimate.mean.amount());
                    actual.push(estimate.stderr);
                }
            }
        }
        let lrm_pricer = PathDependentPricer::new(
            PathDependentPricerConfig::new(256)
                .with_seed(11)
                .with_parallel(false)
                .with_antithetic(false),
        );
        for option_type in [
            crate::types::OptionType::Call,
            crate::types::OptionType::Put,
        ] {
            let greeks = lrm_pricer
                .price_with_lrm_greeks(
                    &crate::types::OptionMarketParams {
                        option_type,
                        spot: 100.0,
                        strike: 100.0,
                        rate: 0.05,
                        dividend_yield: 0.02,
                        volatility: 0.25,
                        time_to_expiry: 1.0,
                    },
                    Some(8),
                    Some(Currency::USD),
                )
                .expect("lrm greeks");
            actual.push(greeks.price.mean.amount());
            actual.push(greeks.price.stderr);
            actual.push(greeks.delta.mean);
            actual.push(greeks.delta.stderr);
            actual.push(greeks.vega.mean);
            actual.push(greeks.vega.stderr);
        }
        assert_eq!(actual.len(), EXPECTED.len());
        for (i, (actual, expected)) in actual.into_iter().zip(EXPECTED).enumerate() {
            // libm evaluation, statistics summation and decimal currency
            // conversion can vary by a few ULP across supported targets.
            assert!(
                (actual - expected).abs() <= 5e-13 * expected.abs().max(1.0),
                "Asian estimator {i}: got {actual}, reference {expected}"
            );
        }
    }
}

mod asian_pair_tests {
    use super::{PathDependentPricer, PathDependentPricerConfig};
    use crate::monte_carlo::control_variate::apply_control_variate;
    use crate::monte_carlo::engine::{MAX_NUM_PATHS, PathCaptureConfig};
    use crate::monte_carlo::payoff::asian::{Asian, AsianFixingHistory, AveragingMethod};
    use crate::monte_carlo::process::gbm::{DriftSchedule, GbmParams, GbmProcess};
    use crate::types::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::math::random::MAX_SOBOL_DIMENSION;
    use finstack_quant_core::math::stats::OnlineCovariance;
    use finstack_quant_core::math::time_grid::TimeGrid;
    use std::sync::Arc;

    const DISCOUNT_FACTOR: f64 = 0.94;

    fn process() -> GbmProcess {
        let drift = DriftSchedule::new(vec![0.0, 0.2, 0.6, 1.0], vec![0.0, 0.006, 0.024, 0.037])
            .expect("nonflat carry schedule");
        GbmProcess::new(GbmParams::new(0.04, 0.01, 0.3).expect("GBM parameters"))
            .with_drift_schedule(Arc::new(drift))
    }

    fn grid() -> TimeGrid {
        TimeGrid::from_times(vec![0.0, 0.07, 0.18, 0.43, 0.69, 1.0])
            .expect("irregular observation grid")
    }

    fn payoff(option_type: OptionType, quantity: f64, averaging: AveragingMethod) -> Asian {
        Asian::with_history(
            option_type,
            100.0,
            quantity,
            averaging,
            vec![3, 0, 1, 3, 5],
            AsianFixingHistory {
                sum: 193.0,
                log_sum: 90.0_f64.ln() + 103.0_f64.ln(),
                count: 2,
            },
        )
        .expect("seasoned Asian contract")
        .with_fixing_multipliers(&[1.0, 0.95, 1.03, 1.08, 0.97])
        .expect("distinct multipliers at repeated fixing steps")
    }

    fn config(num_paths: usize, sobol: bool) -> PathDependentPricerConfig {
        PathDependentPricerConfig {
            chunk_size: 17,
            ..PathDependentPricerConfig::new(num_paths)
                .with_seed(0x1234_5678)
                .with_parallel(false)
                .with_antithetic(false)
                .with_sobol(sobol)
        }
    }

    fn values(moments: &OnlineCovariance) -> [f64; 5] {
        [
            moments
                .get_mean_x()
                .expect("finite mean_x")
                .expect("non-empty mean_x"),
            moments
                .get_mean_y()
                .expect("finite mean_y")
                .expect("non-empty mean_y"),
            moments
                .get_variance_x()
                .expect("finite variance_x")
                .expect("at least 2 samples for variance_x"),
            moments
                .get_variance_y()
                .expect("finite variance_y")
                .expect("at least 2 samples for variance_y"),
            moments
                .get_covariance()
                .expect("finite covariance")
                .expect("at least 2 samples for covariance"),
        ]
    }

    // Retain the old two-pass, global point-order fold as an independent
    // reference. No candidate pair implementation is used here.
    fn captured_reference(
        config: &PathDependentPricerConfig,
        option_type: OptionType,
        quantity: f64,
    ) -> OnlineCovariance {
        let mut capture_config = config.clone();
        capture_config.path_capture = PathCaptureConfig::all();
        let pricer = PathDependentPricer::new(capture_config);
        let gbm = process();
        let capture = |averaging| {
            pricer
                .price_with_paths_and_grid(
                    &gbm,
                    100.0,
                    grid(),
                    &payoff(option_type, quantity, averaging),
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("independent scalar capture")
        };
        let arithmetic = capture(AveragingMethod::Arithmetic);
        let geometric = capture(AveragingMethod::Geometric);
        for result in [&arithmetic, &geometric] {
            assert_eq!(result.estimate.num_paths, config.num_paths);
            assert_eq!(result.estimate.num_simulated_paths, config.num_paths);
            let paths = result.paths.as_ref().expect("complete path capture");
            assert_eq!(paths.num_paths_total, config.num_paths);
            assert_eq!(paths.paths.len(), config.num_paths);
        }
        let mut reference = OnlineCovariance::new();
        for (path_id, (arithmetic, geometric)) in arithmetic
            .paths
            .as_ref()
            .expect("arithmetic capture")
            .paths
            .iter()
            .zip(&geometric.paths.as_ref().expect("geometric capture").paths)
            .enumerate()
        {
            assert_eq!(arithmetic.path_id, path_id);
            assert_eq!(geometric.path_id, path_id);
            assert!(arithmetic.final_value.is_finite() && geometric.final_value.is_finite());
            reference
                .update(arithmetic.final_value, geometric.final_value)
                .expect("finite paired payoffs");
        }
        reference
    }

    fn assert_moments_close(
        actual: &OnlineCovariance,
        reference: &OnlineCovariance,
        quantity: f64,
    ) {
        assert_eq!(actual.get_count(), reference.get_count());
        for (index, (actual, reference)) in values(actual)
            .into_iter()
            .zip(values(reference))
            .enumerate()
        {
            let scale = if index < 2 {
                quantity
            } else {
                quantity * quantity
            };
            assert!(actual.is_finite() && reference.is_finite());
            assert!(
                ((actual - reference) / scale).abs() <= 1e-10,
                "moment {index}: actual={actual:.17e}, reference={reference:.17e}, scale={scale}"
            );
        }
    }

    #[test]
    fn asian_pair_philox_matches_captured_moments_and_parallel_bits() {
        let config = config(65, false);
        let gbm = process();
        for option_type in [OptionType::Call, OptionType::Put] {
            for quantity in [1.0, 250.0] {
                let reference = captured_reference(&config, option_type, quantity);
                let arithmetic = payoff(option_type, quantity, AveragingMethod::Arithmetic);
                let price = |parallel| {
                    PathDependentPricer::new(config.clone().with_parallel(parallel))
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("paired Philox moments")
                };
                let serial = price(false);
                let parallel = price(true);
                assert_eq!(serial.get_count(), config.num_paths);
                assert_moments_close(&serial, &reference, quantity);
                assert_moments_close(&parallel, &reference, quantity);
                assert_eq!(
                    values(&serial).map(f64::to_bits),
                    values(&parallel).map(f64::to_bits)
                );
            }
        }
    }

    #[test]
    fn asian_pair_sobol_preserves_global_point_fold_on_irregular_grid() {
        let gbm = process();
        // 35 points allocate three points to three scrambles and two to the
        // other thirteen. A replicate-level fold or beta would differ.
        for bridge in [false, true] {
            let config = config(35, true).with_brownian_bridge(bridge);
            for option_type in [OptionType::Call, OptionType::Put] {
                for quantity in [1.0, 250.0] {
                    let reference = captured_reference(&config, option_type, quantity);
                    let actual = PathDependentPricer::new(config.clone())
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &payoff(option_type, quantity, AveragingMethod::Arithmetic),
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("paired Sobol moments");
                    assert_eq!(actual.get_count(), 35);
                    assert_eq!(
                        values(&actual).map(f64::to_bits),
                        values(&reference).map(f64::to_bits),
                        "bridge={bridge}, direction={option_type:?}, quantity={quantity}"
                    );
                }
            }
        }
    }

    #[test]
    fn asian_pair_zero_volatility_and_fully_observed_contracts_preserve_constant_payoffs() {
        let zero_volatility =
            GbmProcess::new(GbmParams::new(0.0, 0.0, 0.0).expect("zero-volatility GBM"));
        let future = Asian::new(
            OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            vec![0, 2, 2, 5],
        )
        .expect("future deterministic fixings");
        let observed = Asian::with_history(
            OptionType::Call,
            90.0,
            1.0,
            AveragingMethod::Arithmetic,
            Vec::new(),
            AsianFixingHistory {
                sum: 200.0,
                log_sum: 2.0 * 100.0_f64.ln(),
                count: 2,
            },
        )
        .expect("fully observed contract");
        for (gbm, arithmetic) in [(zero_volatility, future), (process(), observed)] {
            for sobol in [false, true] {
                for chunk_size in [7, 35] {
                    let config = PathDependentPricerConfig {
                        chunk_size,
                        ..config(35, sobol)
                    };
                    let actual = PathDependentPricer::new(config)
                        .price_asian_pair(
                            &gbm,
                            100.0,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .expect("deterministic paired payoffs");
                    assert_eq!(actual.get_count(), 35);
                    let mean_x = actual
                        .get_mean_x()
                        .expect("finite mean_x")
                        .expect("non-empty");
                    let mean_y = actual
                        .get_mean_y()
                        .expect("finite mean_y")
                        .expect("non-empty");
                    let var_x = actual
                        .get_variance_x()
                        .expect("finite var_x")
                        .expect("35 samples");
                    let var_y = actual
                        .get_variance_y()
                        .expect("finite var_y")
                        .expect("35 samples");
                    let cov = actual
                        .get_covariance()
                        .expect("finite cov")
                        .expect("35 samples");
                    assert!((mean_x - 10.0 * DISCOUNT_FACTOR).abs() < 1e-12);
                    assert!((mean_y - 10.0 * DISCOUNT_FACTOR).abs() < 1e-12);
                    // One Philox chunk and the global Sobol fold give exact
                    // zero. Weighted multi-chunk means can round, but the
                    // variance must stay below the existing CV cutoff.
                    if sobol || chunk_size == 35 {
                        assert_eq!(var_x, 0.0);
                        assert_eq!(var_y, 0.0);
                        assert_eq!(cov, 0.0);
                    } else {
                        assert!(var_x.abs() < 1e-10);
                        assert!(var_y.abs() < 1e-10);
                        assert!(cov.abs() < 1e-10);
                    }
                }
            }
        }
    }

    #[test]
    fn asian_pair_preserves_control_adjustment_on_both_sides_of_variance_cutoff() {
        let config = config(65, false);
        let gbm = process();
        let unit_reference = captured_reference(&config, OptionType::Call, 1.0);
        let unit_var_y = unit_reference
            .get_variance_y()
            .expect("finite var_y")
            .expect("65 samples");
        assert!(unit_var_y.is_finite() && unit_var_y > 0.0);
        for target_variance in [0.5e-10, 2.0e-10] {
            let quantity = (target_variance / unit_var_y).sqrt();
            let reference = captured_reference(&config, OptionType::Call, quantity);
            let actual = PathDependentPricer::new(config.clone())
                .price_asian_pair(
                    &gbm,
                    100.0,
                    grid(),
                    &payoff(OptionType::Call, quantity, AveragingMethod::Arithmetic),
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("paired moments near variance cutoff");
            assert_moments_close(&actual, &reference, quantity);
            let applies_control = target_variance > 1e-10;
            let ref_var_y = reference
                .get_variance_y()
                .expect("finite var_y")
                .expect("65 samples");
            let actual_var_y = actual
                .get_variance_y()
                .expect("finite var_y")
                .expect("65 samples");
            assert_eq!(ref_var_y > 1e-10, applies_control);
            assert_eq!(actual_var_y > 1e-10, applies_control);

            // A synthetic control anchor makes the existing coefficient
            // cutoff observable. This tests its propagation, not an Asian
            // analytical pricing formula.
            let ref_mean_y = reference
                .get_mean_y()
                .expect("finite mean_y")
                .expect("65 samples");
            let control_mean = 0.9 * ref_mean_y;
            let adjust = |moments: &OnlineCovariance| {
                let [mean_x, mean_y, variance_x, variance_y, covariance] = values(moments);
                apply_control_variate(
                    mean_x,
                    variance_x,
                    mean_y,
                    variance_y,
                    covariance,
                    control_mean,
                    moments.get_count(),
                )
            };
            let actual_adjusted = adjust(&actual);
            let reference_adjusted = adjust(&reference);
            assert!(((actual_adjusted.mean - reference_adjusted.mean) / quantity).abs() <= 1e-10);
            let actual_variance = actual.get_count() as f64 * actual_adjusted.stderr.powi(2);
            let reference_variance =
                reference.get_count() as f64 * reference_adjusted.stderr.powi(2);
            assert!(((actual_variance - reference_variance) / quantity.powi(2)).abs() <= 1e-10);
            let actual_mean_x = actual
                .get_mean_x()
                .expect("finite mean_x")
                .expect("65 samples");
            if applies_control {
                assert!(actual_adjusted.mean < actual_mean_x);
            } else {
                assert_eq!(actual_adjusted.mean.to_bits(), actual_mean_x.to_bits());
            }
        }
    }

    #[test]
    fn asian_pair_rejects_incompatible_requests_and_invalid_configuration() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        let geometric = payoff(OptionType::Call, 1.0, AveragingMethod::Geometric);
        for sobol in [false, true] {
            let base = config(35, sobol);
            let error = PathDependentPricer::new(base.clone())
                .price_asian_pair(
                    &gbm,
                    100.0,
                    grid(),
                    &geometric,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("geometric target is not an arithmetic/control pair");
            assert!(error.to_string().contains("arithmetic"), "{error}");

            let mut full_capture = base.clone();
            full_capture.path_capture = PathCaptureConfig::all();
            let mut sample_capture = base.clone();
            sample_capture.path_capture = PathCaptureConfig::sample(4, 17);
            let mut zero_chunk = base.clone();
            zero_chunk.chunk_size = 0;
            let mut zero_step_density = base.clone();
            zero_step_density.steps_per_year = 0.0;
            let mut nonfinite_step_density = base.clone();
            nonfinite_step_density.steps_per_year = f64::NAN;
            for (config, expected_message) in [
                (config(0, sobol), "num_paths"),
                (config(1, sobol), "num_paths"),
                (config(MAX_NUM_PATHS + 1, sobol), "maximum"),
                (zero_chunk, "chunk_size"),
                (zero_step_density, "steps_per_year"),
                (nonfinite_step_density, "steps_per_year"),
                (base.with_antithetic(true), "antithetic"),
                (full_capture, "captured paths"),
                (sample_capture, "captured paths"),
            ] {
                let error = PathDependentPricer::new(config)
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &arithmetic,
                        Currency::USD,
                        DISCOUNT_FACTOR,
                    )
                    .expect_err("invalid paired request must fail");
                assert!(error.to_string().contains(expected_message), "{error}");
            }
        }
    }

    #[test]
    fn asian_pair_preserves_runtime_and_payoff_errors() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        for sobol in [false, true] {
            let pricer = PathDependentPricer::new(config(35, sobol));
            let schedule_error = pricer
                .price_asian_pair(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, 4).expect("short grid"),
                    &arithmetic,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("fixing step 5 must not silently disappear");
            assert!(schedule_error.to_string().contains("event at step 5"));
            for discount_factor in [-1.0, f64::NAN, f64::INFINITY] {
                let error = pricer
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &arithmetic,
                        Currency::USD,
                        discount_factor,
                    )
                    .expect_err("invalid discount factor");
                assert!(error.to_string().contains("discount_factor"), "{error}");
            }
            for spot in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(
                    pricer
                        .price_asian_pair(
                            &gbm,
                            spot,
                            grid(),
                            &arithmetic,
                            Currency::USD,
                            DISCOUNT_FACTOR,
                        )
                        .is_err(),
                    "a fixing must reject nonfinite spot"
                );
            }
            let invalid_control = Asian::with_history(
                OptionType::Call,
                100.0,
                1.0,
                AveragingMethod::Arithmetic,
                Vec::new(),
                AsianFixingHistory {
                    sum: 100.0,
                    log_sum: f64::INFINITY,
                    count: 1,
                },
            )
            .expect("history is validated when evaluated");
            assert!(
                pricer
                    .price_asian_pair(
                        &gbm,
                        100.0,
                        grid(),
                        &invalid_control,
                        Currency::USD,
                        DISCOUNT_FACTOR,
                    )
                    .is_err(),
                "a finite arithmetic payoff must not hide an invalid geometric control"
            );
        }
    }

    #[test]
    fn asian_pair_rejects_unsupported_sobol_execution_and_dimensions() {
        let gbm = process();
        let arithmetic = payoff(OptionType::Call, 1.0, AveragingMethod::Arithmetic);
        let parallel_error = PathDependentPricer::new(config(35, true).with_parallel(true))
            .price_asian_pair(
                &gbm,
                100.0,
                grid(),
                &arithmetic,
                Currency::USD,
                DISCOUNT_FACTOR,
            )
            .expect_err("Sobol stream cannot be split across workers");
        assert!(parallel_error.to_string().contains("serial execution"));
        for bridge in [false, true] {
            let error = PathDependentPricer::new(config(35, true).with_brownian_bridge(bridge))
                .price_asian_pair(
                    &gbm,
                    100.0,
                    TimeGrid::uniform(1.0, MAX_SOBOL_DIMENSION + 1)
                        .expect("grid beyond Sobol dimension cap"),
                    &arithmetic,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect_err("unsupported Sobol dimension");
            assert!(error.to_string().contains("Sobol dimension"), "{error}");
        }
    }

    #[test]
    fn geometric_asian_antithetic_pricing_remains_available() {
        let gbm = process();
        for option_type in [OptionType::Call, OptionType::Put] {
            let geometric = payoff(option_type, 1.0, AveragingMethod::Geometric);
            for parallel in [false, true] {
                let estimate = PathDependentPricer::new(
                    config(31, false)
                        .with_parallel(parallel)
                        .with_antithetic(true),
                )
                .price(
                    &gbm,
                    100.0,
                    grid(),
                    &geometric,
                    Currency::USD,
                    DISCOUNT_FACTOR,
                )
                .expect("ordinary geometric antithetic price");
                assert_eq!(estimate.num_paths, 31);
                assert_eq!(estimate.num_simulated_paths, 62);
                assert_eq!(estimate.mean.currency(), Currency::USD);
                assert!(estimate.mean.amount().is_finite() && estimate.mean.amount() > 0.0);
                assert!(estimate.stderr.is_finite());
            }
        }
    }
}
