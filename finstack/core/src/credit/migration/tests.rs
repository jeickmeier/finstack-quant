//! Tests for the credit migration module.
//!
//! Covers: RatingScale, TransitionMatrix, GeneratorMatrix extraction,
//! matrix exponentiation, Gillespie simulation, and the 7×7 reference matrix.

#[cfg(test)]
mod scale_tests {
    use crate::credit::migration::{MigrationError, RatingScale};

    #[test]
    fn standard_scale_dimensions() {
        let scale = RatingScale::standard();
        assert_eq!(scale.n_states(), 10);
        assert_eq!(scale.default_state(), Some(9));
        assert_eq!(scale.index_of("AAA"), Some(0));
        assert_eq!(scale.index_of("BBB"), Some(3));
        assert_eq!(scale.index_of("D"), Some(9));
        assert_eq!(scale.index_of("XYZ"), None);
        assert_eq!(scale.label_of(0), Some("AAA"));
        assert_eq!(scale.label_of(9), Some("D"));
        assert_eq!(scale.label_of(10), None);
    }

    #[test]
    fn standard_with_nr_scale() {
        let scale = RatingScale::standard_with_nr();
        assert_eq!(scale.n_states(), 11);
        assert_eq!(scale.default_state(), Some(10));
        assert_eq!(scale.index_of("NR"), Some(9));
    }

    #[test]
    fn notched_scale() {
        let scale = RatingScale::notched();
        assert_eq!(scale.n_states(), 22);
        assert_eq!(scale.default_state(), Some(21));
        assert_eq!(scale.index_of("AA+"), Some(1));
    }

    #[test]
    fn custom_scale_default_is_last() {
        let scale =
            RatingScale::custom(vec!["A".to_string(), "B".to_string(), "D".to_string()]).unwrap();
        assert_eq!(scale.default_state(), Some(2));
        assert_eq!(scale.index_of("D"), Some(2));
    }

    #[test]
    fn custom_with_explicit_default() {
        let scale = RatingScale::custom_with_default(
            vec!["IG".to_string(), "HY".to_string(), "D".to_string()],
            "D",
        )
        .unwrap();
        assert_eq!(scale.default_state(), Some(2));
    }

    #[test]
    fn insufficient_states_error() {
        let err = RatingScale::custom(vec!["A".to_string()]);
        assert!(matches!(err, Err(MigrationError::InsufficientStates)));
    }

    #[test]
    fn duplicate_label_error() {
        let err = RatingScale::custom(vec!["A".to_string(), "A".to_string()]);
        assert!(matches!(err, Err(MigrationError::DuplicateLabel { .. })));
    }

    #[test]
    fn unknown_default_error() {
        let err =
            RatingScale::custom_with_default(vec!["A".to_string(), "B".to_string()], "DEFAULT");
        assert!(matches!(err, Err(MigrationError::UnknownState { .. })));
    }

    #[test]
    fn warf_standard_scale() {
        let scale = RatingScale::standard();
        assert_eq!(scale.warf("AAA").unwrap(), 1.0);
        assert_eq!(scale.warf("BBB").unwrap(), 360.0);
        assert_eq!(scale.warf("B").unwrap(), 2720.0);
        assert_eq!(scale.warf("D").unwrap(), 10000.0);
    }

    #[test]
    fn warf_notched_scale() {
        let scale = RatingScale::notched();
        assert_eq!(scale.warf("AA+").unwrap(), 10.0);
        assert_eq!(scale.warf("BBB-").unwrap(), 610.0);
        assert_eq!(scale.warf("B+").unwrap(), 2220.0);
        assert_eq!(scale.warf("CCC-").unwrap(), 8070.0);
    }

    #[test]
    fn warf_unknown_label() {
        let scale = RatingScale::standard();
        assert!(matches!(
            scale.warf("XYZ"),
            Err(MigrationError::UnknownState { .. })
        ));
    }

    #[test]
    fn warf_unparseable_label() {
        let scale =
            RatingScale::custom(vec!["IG".to_string(), "HY".to_string(), "D".to_string()]).unwrap();
        assert!(matches!(
            scale.warf("IG"),
            Err(MigrationError::NoWarfFactor { .. })
        ));
        assert_eq!(scale.warf("D").unwrap(), 10000.0);
    }

    #[test]
    fn rating_from_warf_exact() {
        let scale = RatingScale::standard();
        assert_eq!(scale.rating_from_warf(1.0).unwrap(), "AAA");
        assert_eq!(scale.rating_from_warf(360.0).unwrap(), "BBB");
        assert_eq!(scale.rating_from_warf(2720.0).unwrap(), "B");
    }

    #[test]
    fn rating_from_warf_nearest() {
        let scale = RatingScale::standard();
        // 400 is between BBB (360) and BB (1350) — closer to BBB
        assert_eq!(scale.rating_from_warf(400.0).unwrap(), "BBB");
        // 1000 is between BBB (360) and BB (1350) — closer to BB
        assert_eq!(scale.rating_from_warf(1000.0).unwrap(), "BB");
    }

    #[test]
    fn rating_from_warf_notched() {
        let scale = RatingScale::notched();
        assert_eq!(scale.rating_from_warf(260.0).unwrap(), "BBB+");
        assert_eq!(scale.rating_from_warf(500.0).unwrap(), "BBB-");
    }

    #[test]
    fn rating_from_warf_no_valid_labels() {
        let scale = RatingScale::custom(vec!["IG".to_string(), "HY".to_string()]).unwrap();
        assert!(matches!(
            scale.rating_from_warf(100.0),
            Err(MigrationError::NoWarfMapping)
        ));
    }
}

#[cfg(test)]
mod matrix_tests {
    use crate::credit::migration::{MigrationError, RatingScale, TransitionMatrix};

    fn two_state_scale() -> RatingScale {
        RatingScale::custom(vec!["IG".to_string(), "D".to_string()]).unwrap()
    }

    #[test]
    fn identity_matrix() {
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale, &[1.0, 0.0, 0.0, 1.0], 1.0).unwrap();
        assert_eq!(p.probability("IG", "D").unwrap(), 0.0);
        assert_eq!(p.probability("IG", "IG").unwrap(), 1.0);
    }

    #[test]
    fn rejects_negative_entry() {
        let scale = two_state_scale();
        let err = TransitionMatrix::new(scale, &[1.1, -0.1, 0.0, 1.0], 1.0);
        assert!(matches!(err, Err(MigrationError::EntryOutOfRange { .. })));
    }

    #[test]
    fn rejects_bad_row_sum() {
        let scale = two_state_scale();
        let err = TransitionMatrix::new(scale, &[0.9, 0.0, 0.0, 1.0], 1.0);
        assert!(matches!(err, Err(MigrationError::RowSumViolation { .. })));
    }

    #[test]
    fn rejects_non_absorbing_default() {
        let scale = two_state_scale();
        // Default state (D, index 1) row is [0.1, 0.9] — not absorbing.
        let err = TransitionMatrix::new(scale, &[0.9, 0.1, 0.1, 0.9], 1.0);
        assert!(matches!(
            err,
            Err(MigrationError::NonAbsorbingDefault { .. })
        ));
    }

    #[test]
    fn rejects_invalid_horizon() {
        let scale = two_state_scale();
        let err = TransitionMatrix::new(scale, &[1.0, 0.0, 0.0, 1.0], 0.0);
        assert!(matches!(err, Err(MigrationError::InvalidHorizon(_))));
    }

    #[test]
    fn compose_identity() {
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale.clone(), &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        let _p = TransitionMatrix::new(scale, &[1.0, 0.0, 0.0, 1.0], 0.0);
        // identity has invalid horizon 0 so build it manually
        let id = TransitionMatrix {
            data: nalgebra::DMatrix::identity(2, 2),
            horizon: 0.0,
            scale: p.scale().clone(),
        };
        let composed = p.compose(&id).unwrap();
        assert!((composed.probability("IG", "IG").unwrap() - 0.9).abs() < 1e-12);
    }

    #[test]
    fn compose_scale_mismatch() {
        let scale_a = RatingScale::custom(vec!["A".to_string(), "D".to_string()]).unwrap();
        let scale_b = RatingScale::custom(vec!["B".to_string(), "D".to_string()]).unwrap();
        let p1 = TransitionMatrix::new(scale_a, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        let p2 = TransitionMatrix::new(scale_b, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        assert!(matches!(
            p1.compose(&p2),
            Err(MigrationError::ScaleMismatch)
        ));
    }

    #[test]
    fn default_probabilities() {
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        let dps = p.default_probabilities().unwrap();
        assert!((dps[0] - 0.1).abs() < 1e-12);
        assert!((dps[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn row_accessor() {
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        let row = p.row("IG").unwrap();
        assert_eq!(row, vec![0.9, 0.1]);
    }
}

#[cfg(test)]
mod generator_tests {
    use crate::credit::migration::{
        GeneratorMatrix, MigrationError, RatingScale, TransitionMatrix,
    };

    fn two_state_scale() -> RatingScale {
        RatingScale::custom(vec!["IG".to_string(), "D".to_string()]).unwrap()
    }

    #[test]
    fn known_2x2_round_trip() {
        // P = [[0.9, 0.1], [0.0, 1.0]]
        // Q = [[ln(0.9), -ln(0.9)], [0, 0]]
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        let gen = GeneratorMatrix::from_transition_matrix(&p)
            .expect("generator extraction should succeed");
        let q_00 = gen.intensity("IG", "IG").unwrap();
        let q_01 = gen.intensity("IG", "D").unwrap();
        let expected = 0.9_f64.ln();
        assert!(
            (q_00 - expected).abs() < 1e-8,
            "q_00 = {q_00}, expected {expected}"
        );
        assert!(
            (q_01 + expected).abs() < 1e-8,
            "q_01 = {q_01}, expected {}",
            -expected
        );
        assert!(gen.intensity("D", "IG").unwrap().abs() < 1e-10);
    }

    #[test]
    fn direct_construction_and_accessors() {
        let scale = two_state_scale();
        let lambda = 0.05;
        let gen = GeneratorMatrix::new(scale, &[-lambda, lambda, 0.0, 0.0]).unwrap();
        assert!((gen.exit_rate("IG").unwrap() - lambda).abs() < 1e-12);
        assert!(gen.exit_rate("D").unwrap().abs() < 1e-12);
    }

    #[test]
    fn rejects_positive_off_diagonal() {
        // Diagonal must be ≤ 0
        let scale = two_state_scale();
        let err = GeneratorMatrix::new(scale, &[0.05, 0.0, 0.0, -0.05]);
        assert!(matches!(err, Err(MigrationError::EntryOutOfRange { .. })));
    }

    #[test]
    fn rejects_bad_row_sum() {
        let scale = two_state_scale();
        let err = GeneratorMatrix::new(scale, &[-0.1, 0.05, 0.0, 0.0]);
        assert!(matches!(err, Err(MigrationError::RowSumViolation { .. })));
    }

    #[test]
    fn kreinin_sidenius_correction() {
        // The correction should clamp small negative off-diagonals and produce
        // a valid generator.
        let scale = two_state_scale();
        let p = TransitionMatrix::new(scale, &[0.9, 0.1, 0.0, 1.0], 1.0).unwrap();
        // Should succeed even if raw log has a tiny negative off-diagonal.
        GeneratorMatrix::from_transition_matrix(&p)
            .expect("Kreinin-Sidenius should produce valid generator");
    }

    /// Policy-visibility stamping : a matrix
    /// whose log requires Kreinin-Sidenius clamping must report non-zero
    /// `regularization_l1`, and the round-trip error must be stamped.
    #[test]
    fn regularization_and_round_trip_error_are_stamped() {
        // 3-state downgrade chain with distinct eigenvalues (0.90, 0.85, 1):
        // A can only reach D through B over one period, so log(P) has a
        // negative (A, D) entry (≈ −0.009), forcing K-S clamping.
        let scale =
            RatingScale::custom(vec!["A".to_string(), "B".to_string(), "D".to_string()]).unwrap();
        #[rustfmt::skip]
        let data = &[
            0.90, 0.10, 0.00,
            0.00, 0.85, 0.15,
            0.00, 0.00, 1.00,
        ];
        let p = TransitionMatrix::new(scale, data, 1.0).unwrap();
        // K-S clamping perturbs the generator enough that the round-trip
        // error (~0.022) exceeds the default 1e-2 tolerance — exactly the
        // distortion the stamps make visible. Use a loosened tolerance to
        // obtain the regularized generator and inspect the stamps.
        let gen = GeneratorMatrix::from_transition_matrix_with_tol(&p, 5e-2)
            .expect("generator extraction should succeed");

        assert!(
            gen.regularization_l1() > 0.0,
            "K-S clamping occurred but regularization_l1 = {}",
            gen.regularization_l1()
        );
        assert!(
            gen.round_trip_error() > 0.0 && gen.round_trip_error() <= 5e-2,
            "round-trip error must be stamped and within the requested \
             tolerance: {}",
            gen.round_trip_error()
        );
    }

    /// Directly constructed generators are unregularized: both stamps are zero.
    #[test]
    fn direct_construction_stamps_zero_regularization() {
        let scale = two_state_scale();
        let gen = GeneratorMatrix::new(scale, &[-0.05, 0.05, 0.0, 0.0]).unwrap();
        assert_eq!(gen.regularization_l1(), 0.0);
        assert_eq!(gen.round_trip_error(), 0.0);
    }
}

#[cfg(test)]
mod projection_tests {
    use crate::credit::migration::{projection, GeneratorMatrix, RatingScale};

    fn two_state_gen() -> (RatingScale, GeneratorMatrix) {
        let scale = RatingScale::custom(vec!["IG".to_string(), "D".to_string()]).unwrap();
        let gen = GeneratorMatrix::new(scale.clone(), &[-0.05, 0.05, 0.0, 0.0]).unwrap();
        (scale, gen)
    }

    #[test]
    fn exp_at_zero_approaches_identity() {
        // Use a very small t instead of 0 (which is invalid).
        let (_, gen) = two_state_gen();
        let p = projection::project(&gen, 1e-10).unwrap();
        assert!((p.probability_by_index(0, 0) - 1.0).abs() < 1e-6);
        assert!(p.probability_by_index(0, 1) < 1e-6);
    }

    #[test]
    fn semi_group_property() {
        // P(s+t) ≈ P(s) · P(t)
        let (_, gen) = two_state_gen();
        let ps = projection::project(&gen, 1.0).unwrap();
        let pt = projection::project(&gen, 2.0).unwrap();
        let p3 = projection::project(&gen, 3.0).unwrap();
        let composed = ps.compose(&pt).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let diff =
                    (composed.probability_by_index(i, j) - p3.probability_by_index(i, j)).abs();
                assert!(
                    diff < 1e-8,
                    "semi-group property failed at ({i},{j}): diff={diff}"
                );
            }
        }
    }

    #[test]
    fn row_stochastic_output() {
        let (_, gen) = two_state_gen();
        let p = projection::project(&gen, 5.0).unwrap();
        for i in 0..2 {
            let sum: f64 = (0..2).map(|j| p.probability_by_index(i, j)).sum();
            assert!((sum - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn all_entries_non_negative() {
        let (_, gen) = two_state_gen();
        let p = projection::project(&gen, 10.0).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                assert!(p.probability_by_index(i, j) >= 0.0);
            }
        }
    }

    #[test]
    fn pade_equals_standard() {
        let (_, gen) = two_state_gen();
        let p1 = projection::project(&gen, 3.0).unwrap();
        let p2 = projection::project_pade(&gen, 3.0).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                assert!(
                    (p1.probability_by_index(i, j) - p2.probability_by_index(i, j)).abs() < 1e-12
                );
            }
        }
    }
}

#[cfg(test)]
mod simulation_tests {
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    use crate::credit::migration::{simulation::MigrationSimulator, GeneratorMatrix, RatingScale};

    fn two_state_gen() -> GeneratorMatrix {
        let scale = RatingScale::custom(vec!["IG".to_string(), "D".to_string()]).unwrap();
        GeneratorMatrix::new(scale, &[-0.1, 0.1, 0.0, 0.0]).unwrap()
    }

    #[test]
    fn absorbing_state_stays_put() {
        let gen = two_state_gen();
        let sim = MigrationSimulator::new(gen, 10.0).unwrap();
        let mut rng = Pcg64::seed_from_u64(42);
        // Start from D (index 1), which is absorbing.
        let paths = sim.simulate(1, 100, &mut rng);
        for path in &paths {
            assert_eq!(path.n_transitions(), 0);
            assert_eq!(path.state_at(10.0), 1);
        }
    }

    #[test]
    fn deterministic_seed_reproducible() {
        let gen = two_state_gen();
        let sim = MigrationSimulator::new(gen, 5.0).unwrap();
        let mut rng1 = Pcg64::seed_from_u64(99);
        let mut rng2 = Pcg64::seed_from_u64(99);
        let paths1 = sim.simulate(0, 10, &mut rng1);
        let paths2 = sim.simulate(0, 10, &mut rng2);
        for (p1, p2) in paths1.iter().zip(paths2.iter()) {
            assert_eq!(p1.transitions(), p2.transitions());
        }
    }

    #[test]
    fn convergence_to_analytical() {
        // P(1) for lambda=0.1: P(IG→D) = 1 - exp(-0.1) ≈ 0.09516
        let gen = two_state_gen();
        let sim = MigrationSimulator::new(gen, 1.0).unwrap();
        let mut rng = Pcg64::seed_from_u64(12345);
        let emp = sim.empirical_matrix(100_000, &mut rng);
        let empirical_pd = emp.probability_by_index(0, 1);
        let analytical_pd = 1.0 - (-0.1_f64).exp();
        assert!(
            (empirical_pd - analytical_pd).abs() < 0.005,
            "empirical PD {empirical_pd:.4} too far from analytical {analytical_pd:.4}"
        );
    }

    #[test]
    fn default_time_recorded() {
        let gen = two_state_gen();
        let sim = MigrationSimulator::new(gen, 20.0).unwrap();
        let mut rng = Pcg64::seed_from_u64(7);
        let paths = sim.simulate(0, 1000, &mut rng);
        let defaults: usize = paths.iter().filter(|p| p.defaulted()).count();
        // With lambda=0.1, P(default within 20y) = 1 - exp(-2) ≈ 0.865
        assert!(
            defaults > 700,
            "expected >700/1000 defaults, got {defaults}"
        );
    }
}

#[cfg(test)]
mod reference_matrix_tests {
    use crate::credit::migration::{projection, GeneratorMatrix, RatingScale, TransitionMatrix};

    /// 7×7 annual transition matrix from the design spec.
    fn reference_matrix() -> (RatingScale, TransitionMatrix) {
        let labels = vec!["AAA", "AA", "A", "BBB", "BB", "B", "D"]
            .into_iter()
            .map(String::from)
            .collect();
        let scale = RatingScale::custom(labels).unwrap();

        #[rustfmt::skip]
        let data: Vec<f64> = vec![
            0.9081, 0.0833, 0.0068, 0.0006, 0.0012, 0.0000, 0.0000,
            0.0070, 0.9065, 0.0779, 0.0064, 0.0006, 0.0014, 0.0002,
            0.0009, 0.0227, 0.9105, 0.0552, 0.0074, 0.0026, 0.0007,
            0.0002, 0.0033, 0.0595, 0.8693, 0.0530, 0.0117, 0.0030,
            0.0003, 0.0014, 0.0067, 0.0773, 0.8053, 0.0884, 0.0206,
            0.0000, 0.0011, 0.0024, 0.0043, 0.0648, 0.8346, 0.0928,
            0.0000, 0.0000, 0.0000, 0.0000, 0.0000, 0.0000, 1.0000,
        ];
        let p = TransitionMatrix::new(scale.clone(), &data, 1.0).unwrap();
        (scale, p)
    }

    #[test]
    fn reference_matrix_constructs() {
        let (_scale, p) = reference_matrix();
        assert_eq!(p.n_states(), 7);
    }

    #[test]
    fn generator_extraction_succeeds() {
        let (_scale, p) = reference_matrix();
        let gen = GeneratorMatrix::from_transition_matrix(&p)
            .expect("generator extraction of reference matrix should succeed");
        // All diagonal entries should be ≤ 0 and off-diagonal ≥ 0.
        for i in 0..7 {
            assert!(
                gen.as_matrix()[(i, i)] <= 0.0,
                "diagonal ({i},{i}) must be ≤ 0"
            );
            for j in 0..7 {
                if j != i {
                    assert!(
                        gen.as_matrix()[(i, j)] >= -1e-10,
                        "off-diagonal ({i},{j}) = {} must be ≥ 0",
                        gen.as_matrix()[(i, j)]
                    );
                }
            }
        }
    }

    #[test]
    fn six_month_projection() {
        let (_scale, p) = reference_matrix();
        let gen = GeneratorMatrix::from_transition_matrix(&p).unwrap();
        let p_half = projection::project(&gen, 0.5).unwrap();
        // Row sums = 1, all entries ≥ 0.
        for i in 0..7 {
            let sum: f64 = (0..7).map(|j| p_half.probability_by_index(i, j)).sum();
            assert!((sum - 1.0).abs() < 1e-8, "row {i} sum = {sum}");
            for j in 0..7 {
                assert!(p_half.probability_by_index(i, j) >= 0.0);
            }
        }
        // Default state is still absorbing.
        assert!((p_half.probability_by_index(6, 6) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn five_year_projection() {
        let (_scale, p) = reference_matrix();
        let gen = GeneratorMatrix::from_transition_matrix(&p).unwrap();
        let p5 = projection::project(&gen, 5.0).unwrap();
        // Cumulative default PD for BBB (index 3) over 5y should be material.
        let pd_bbb = p5.probability_by_index(3, 6);
        assert!(pd_bbb > 0.01, "5y BBB PD = {pd_bbb:.4} is too low");
        assert!(
            pd_bbb < 0.20,
            "5y BBB PD = {pd_bbb:.4} is surprisingly high"
        );
    }

    #[test]
    fn semi_group_7x7() {
        let (_scale, p) = reference_matrix();
        let gen = GeneratorMatrix::from_transition_matrix(&p).unwrap();
        let p1 = projection::project(&gen, 1.0).unwrap();
        let p2 = projection::project(&gen, 2.0).unwrap();
        let p3 = projection::project(&gen, 3.0).unwrap();
        let composed = p1.compose(&p2).unwrap();
        for i in 0..7 {
            for j in 0..7 {
                let diff =
                    (composed.probability_by_index(i, j) - p3.probability_by_index(i, j)).abs();
                assert!(diff < 1e-6, "semi-group ({i},{j}): diff={diff}");
            }
        }
    }
}
