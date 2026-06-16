// Property-based tests for Monte Carlo numerical invariants.
//
// These tests use proptest to verify mathematical properties hold across
// a wide range of inputs. Property tests complement unit tests by:
// - Testing thousands of random but valid input combinations
// - Catching edge cases that manual tests might miss
// - Verifying mathematical relationships hold universally
//
// Coverage areas:
// - Process drift/diffusion properties
// - Discretization scheme convergence
// - RNG distribution properties

mod tests {
    use finstack_quant_monte_carlo::discretization::euler::EulerMaruyama;
    use finstack_quant_monte_carlo::discretization::exact::ExactGbm;
    use finstack_quant_monte_carlo::process::gbm::GbmProcess;
    use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
    use finstack_quant_monte_carlo::{Discretization, RandomStream, StochasticProcess};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: GBM drift should equal (r - q) * S for any spot price.
        #[test]
        fn prop_gbm_drift_property(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
            t in 0.0f64..10.0,
            spot in 1.0f64..1000.0,
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let mut drift = vec![0.0; gbm.dim()];
            let state = vec![spot];
            gbm.drift(t, &state, &mut drift);

            // GBM drift should be (r - q) * S (time-independent)
            let expected_drift = (r - q) * spot;
            prop_assert!(
                (drift[0] - expected_drift).abs() < 1e-10,
                "GBM drift mismatch: expected {}, got {}",
                expected_drift, drift[0]
            );
        }

        /// Property: GBM diffusion should equal σ * S for any spot price.
        #[test]
        fn prop_gbm_diffusion_property(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
            t in 0.0f64..10.0,
            spot in 1.0f64..1000.0,
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let mut diffusion = vec![0.0; gbm.dim()];
            let state = vec![spot];
            gbm.diffusion(t, &state, &mut diffusion);

            // GBM diffusion should be σ * S (time-independent)
            let expected_diffusion = sigma * spot;
            prop_assert!(
                (diffusion[0] - expected_diffusion).abs() < 1e-10,
                "GBM diffusion mismatch: expected {}, got {}",
                expected_diffusion, diffusion[0]
            );
        }

        /// Property: GBM process has dimension 1 and 1 factor.
        #[test]
        fn prop_gbm_dimensions(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();

            prop_assert_eq!(gbm.dim(), 1, "GBM should have dimension 1");
            prop_assert_eq!(gbm.num_factors(), 1, "GBM should have 1 factor");
            prop_assert!(gbm.is_diagonal(), "GBM should have diagonal diffusion");
        }

        /// Property: Euler-Maruyama converges to exact GBM as dt -> 0.
        #[test]
        fn prop_euler_converges_to_exact_gbm(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..0.5,
            spot in 50.0f64..200.0,
            dt_power in 1u32..8,  // dt = 2^(-dt_power), so dt ranges from 0.5 to ~0.004
        ) {
            let dt = 1.0 / (1u64 << dt_power) as f64;
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let exact_disc = ExactGbm::new();
            let euler_disc = EulerMaruyama::new();

            // Initial state
            let mut state_exact = vec![spot];
            let mut state_euler = vec![spot];

            // Standard normal shock (use same shock for fair comparison)
            let z = [0.5]; // Fixed shock for reproducibility

            // Workspace buffers
            let mut work_euler = vec![0.0; euler_disc.work_size(&gbm)];
            let mut work_exact = vec![0.0; exact_disc.work_size(&gbm)];

            // Take one step with each scheme
            exact_disc.step(&gbm, 0.0, dt, &mut state_exact, &z, &mut work_exact);
            euler_disc.step(&gbm, 0.0, dt, &mut state_euler, &z, &mut work_euler);

            // As dt -> 0, Euler should converge to exact
            // For small dt, the difference should be O(dt²) or better
            let error = (state_euler[0] - state_exact[0]).abs();
            let relative_error = error / state_exact[0].abs();

            // Allow larger relative error for larger dt, but should be reasonable
            let tolerance = dt * dt * 100.0; // O(dt²) tolerance scaled

            prop_assert!(
                relative_error < tolerance || error < 1e-6,
                "Euler does not converge to exact GBM: dt={}, exact={}, euler={}, error={}",
                dt, state_exact[0], state_euler[0], error
            );
        }

        /// Property: Exact GBM maintains positivity for any shock.
        #[test]
        fn prop_exact_gbm_positivity(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
            spot in 1.0f64..1000.0,
            dt in 0.001f64..1.0,
            z in -5.0f64..5.0,  // Standard normal range (±5σ covers most cases)
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let disc = ExactGbm::new();

            let mut state = vec![spot];
            let z_array = [z];
            let mut work = vec![0.0; disc.work_size(&gbm)];

            disc.step(&gbm, 0.0, dt, &mut state, &z_array, &mut work);

            // Exact GBM should always produce positive spot prices
            prop_assert!(
                state[0] > 0.0,
                "Exact GBM produced non-positive spot: {}",
                state[0]
            );
        }

        /// Property: Philox RNG generates values in [0, 1) range.
        #[test]
        fn prop_philox_uniform_range(
            seed in 0u64..1000000u64,
            num_samples in 10usize..100usize,
        ) {
            let mut rng = PhiloxRng::new(seed);
            let mut samples = vec![0.0; num_samples];

            rng.fill_u01(&mut samples);

            // All samples should be in [0, 1)
            for (i, &sample) in samples.iter().enumerate() {
                prop_assert!(
                    (0.0..1.0).contains(&sample),
                    "Sample {} out of range [0, 1): {}",
                    i, sample
                );
            }
        }

        /// Property: Philox RNG generates different streams for different stream IDs.
        #[test]
        fn prop_philox_stream_independence(
            seed in 0u64..1000000u64,
            stream1 in 0u64..1000u64,
            stream2 in 1001u64..2000u64,
        ) {
            let mut rng1 = PhiloxRng::with_stream(seed, stream1);
            let mut rng2 = PhiloxRng::with_stream(seed, stream2);

            let mut samples1 = vec![0.0; 10];
            let mut samples2 = vec![0.0; 10];

            rng1.fill_u01(&mut samples1);
            rng2.fill_u01(&mut samples2);

            // Different streams should produce different values (very high probability)
            // Check that at least some samples differ
            let all_same = samples1.iter().zip(samples2.iter())
                .all(|(a, b)| (a - b).abs() < 1e-10);

            prop_assert!(
                !all_same,
                "Streams {} and {} produced identical samples (unlikely but possible)",
                stream1, stream2
            );
        }

        /// Property: Philox RNG is deterministic (same seed + stream = same sequence).
        #[test]
        fn prop_philox_determinism(
            seed in 0u64..1000000u64,
            stream_id in 0u64..1000u64,
        ) {
            let mut rng1 = PhiloxRng::with_stream(seed, stream_id);
            let mut rng2 = PhiloxRng::with_stream(seed, stream_id);

            let mut samples1 = vec![0.0; 20];
            let mut samples2 = vec![0.0; 20];

            rng1.fill_u01(&mut samples1);
            rng2.fill_u01(&mut samples2);

            // Same seed + stream should produce identical sequences
            for (i, (a, b)) in samples1.iter().zip(samples2.iter()).enumerate() {
                prop_assert!(
                    (a - b).abs() < 1e-15,
                    "Determinism violated at sample {}: {} vs {}",
                    i, a, b
                );
            }
        }

        /// Property: Standard normal samples from Philox have reasonable distribution.
        #[test]
        fn prop_philox_std_normal_distribution(
            seed in 0u64..1000000u64,
        ) {
            let mut rng = PhiloxRng::new(seed);
            let num_samples = 1000;
            let mut samples = vec![0.0; num_samples];

            rng.fill_std_normals(&mut samples);

            // Check that samples are roughly centered around 0
            let mean: f64 = samples.iter().sum::<f64>() / num_samples as f64;
            let variance: f64 = samples.iter()
                .map(|x| (x - mean) * (x - mean))
                .sum::<f64>() / (num_samples - 1) as f64;
            let std_dev = variance.sqrt();

            // For 1000 samples, mean should be close to 0 and std_dev close to 1.0. Allow wider
            // tolerance to avoid flaky failures from rare tail draws of the chi-square distribution.
            const MEAN_TOL: f64 = 0.2;
            const STD_TOL: f64 = 0.15;
            prop_assert!(
                mean.abs() < MEAN_TOL,
                "Sample mean {} too far from 0",
                mean
            );
            prop_assert!(
                (std_dev - 1.0).abs() < STD_TOL,
                "Sample std_dev {} too far from 1",
                std_dev
            );
        }

        /// Property: GBM diffusion coefficient is always positive for positive spot.
        #[test]
        fn prop_gbm_diffusion_positive(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
            spot in 1.0f64..1000.0,
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let mut diffusion = vec![0.0; gbm.dim()];
            let state = vec![spot];

            gbm.diffusion(0.0, &state, &mut diffusion);

            // Diffusion coefficient should be positive for positive spot
            prop_assert!(
                diffusion[0] > 0.0,
                "GBM diffusion should be positive for positive spot, got {}",
                diffusion[0]
            );
        }

        /// Property: Euler-Maruyama maintains dimension consistency.
        #[test]
        fn prop_euler_dimension_consistency(
            r in 0.0f64..0.2,
            q in 0.0f64..0.1,
            sigma in 0.01f64..1.0,
            spot in 1.0f64..1000.0,
        ) {
            let gbm = GbmProcess::with_params(r, q, sigma).unwrap();
            let disc = EulerMaruyama::new();

            let work_size = disc.work_size(&gbm);

            // Work size should be 2 * dim (drift + diffusion vectors)
            prop_assert_eq!(
                work_size,
                2 * gbm.dim(),
                "Euler work_size should be 2*dim, got {}",
                work_size
            );

            // Test that workspace is sufficient
            let mut state = vec![spot];
            let z = vec![0.0; gbm.num_factors()];
            let mut work = vec![0.0; work_size];

            // Should not panic when using correct workspace size
            disc.step(&gbm, 0.0, 0.01, &mut state, &z, &mut work);

            prop_assert!(state[0].is_finite(), "State should remain finite");
        }
    }
}
