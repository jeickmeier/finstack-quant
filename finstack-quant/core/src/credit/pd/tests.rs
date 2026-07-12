//! Tests for PD calibration, term structure, and master scale.

#[cfg(test)]
mod calibration_tests {
    use crate::credit::pd::{
        apply_basel_irb_pd_floor, pit_to_ttc, ttc_to_pit, PdCalibrationError, PdCycleParams,
        BASEL_IRB_PD_FLOOR,
    };

    /// PiT/TtC round-trip: converting TtC -> PiT -> TtC should recover the original.
    #[test]
    fn round_trip_consistency() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: -1.5,
        };
        let pd_ttc = 0.02;
        let pd_pit = ttc_to_pit(pd_ttc, &params).unwrap();
        let recovered = pit_to_ttc(pd_pit, &params).unwrap();
        assert!(
            (recovered - pd_ttc).abs() < 1e-10,
            "Round-trip failed: original={}, recovered={}",
            pd_ttc,
            recovered
        );
    }

    /// z = 0 with round-trip: ttc -> pit -> ttc recovers original at z=0.
    ///
    /// Note: z=0 does NOT imply PiT == TtC in the single-factor model
    /// (that only holds when rho=0). But the round-trip property holds
    /// for any z value.
    #[test]
    fn neutral_cycle_round_trip() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: 0.0,
        };
        let pd_ttc = 0.03;
        let pd_pit = ttc_to_pit(pd_ttc, &params).unwrap();
        let recovered = pit_to_ttc(pd_pit, &params).unwrap();
        assert!(
            (recovered - pd_ttc).abs() < 1e-10,
            "z=0 round-trip failed: original={}, recovered={}",
            pd_ttc,
            recovered
        );
    }

    /// z < 0 (downturn) => PiT > TtC.
    #[test]
    fn downturn_increases_pd() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: -2.0,
        };
        let pd_ttc = 0.02;
        let pd_pit = ttc_to_pit(pd_ttc, &params).unwrap();
        assert!(
            pd_pit > pd_ttc,
            "Downturn should increase PD: pit={}, ttc={}",
            pd_pit,
            pd_ttc
        );
    }

    /// z > 0 (benign) => PiT < TtC.
    #[test]
    fn benign_decreases_pd() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: 1.5,
        };
        let pd_ttc = 0.05;
        let pd_pit = ttc_to_pit(pd_ttc, &params).unwrap();
        assert!(
            pd_pit < pd_ttc,
            "Benign conditions should decrease PD: pit={}, ttc={}",
            pd_pit,
            pd_ttc
        );
    }

    #[test]
    fn basel_irb_pd_floor_is_explicit_opt_in() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: 1.5,
        };
        let raw = ttc_to_pit(0.0001, &params).unwrap();

        assert!(raw < BASEL_IRB_PD_FLOOR);
        assert_eq!(apply_basel_irb_pd_floor(raw), BASEL_IRB_PD_FLOOR);
        assert_eq!(apply_basel_irb_pd_floor(0.01), 0.01);
    }

    /// PD output is always in (0, 1).
    #[test]
    fn output_in_valid_range() {
        let params = PdCycleParams {
            asset_correlation: 0.15,
            cycle_index: -3.0,
        };
        let pd_pit = ttc_to_pit(0.01, &params).unwrap();
        assert!(pd_pit > 0.0 && pd_pit < 1.0, "pd_pit={}", pd_pit);

        let pd_ttc = pit_to_ttc(0.99, &params).unwrap();
        assert!(pd_ttc > 0.0 && pd_ttc < 1.0, "pd_ttc={}", pd_ttc);
    }

    /// Multiple correlation values and round-trips.
    #[test]
    fn various_correlations() {
        for &rho in &[0.05, 0.12, 0.20, 0.24, 0.50, 0.90] {
            let params = PdCycleParams {
                asset_correlation: rho,
                cycle_index: -1.0,
            };
            let pd = 0.05;
            let pit = ttc_to_pit(pd, &params).unwrap();
            let recovered = pit_to_ttc(pit, &params).unwrap();
            assert!(
                (recovered - pd).abs() < 1e-8,
                "rho={}: original={}, recovered={}",
                rho,
                pd,
                recovered
            );
        }
    }

    /// Reject PD outside (0, 1).
    #[test]
    fn reject_invalid_pd() {
        let params = PdCycleParams {
            asset_correlation: 0.20,
            cycle_index: 0.0,
        };
        assert!(matches!(
            ttc_to_pit(0.0, &params),
            Err(PdCalibrationError::PdOutOfRange { .. })
        ));
        assert!(matches!(
            ttc_to_pit(1.0, &params),
            Err(PdCalibrationError::PdOutOfRange { .. })
        ));
        assert!(matches!(
            ttc_to_pit(-0.5, &params),
            Err(PdCalibrationError::PdOutOfRange { .. })
        ));
        assert!(matches!(
            pit_to_ttc(1.5, &params),
            Err(PdCalibrationError::PdOutOfRange { .. })
        ));
    }

    /// Reject correlation outside (0, 1).
    #[test]
    fn reject_invalid_correlation() {
        let bad_params = PdCycleParams {
            asset_correlation: 0.0,
            cycle_index: 0.0,
        };
        assert!(matches!(
            ttc_to_pit(0.05, &bad_params),
            Err(PdCalibrationError::InvalidCorrelation { .. })
        ));

        let bad_params2 = PdCycleParams {
            asset_correlation: 1.0,
            cycle_index: 0.0,
        };
        assert!(matches!(
            ttc_to_pit(0.05, &bad_params2),
            Err(PdCalibrationError::InvalidCorrelation { .. })
        ));
    }

    #[test]
    fn reject_non_finite_cycle_index() {
        for cycle_index in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let params = PdCycleParams {
                asset_correlation: 0.20,
                cycle_index,
            };
            assert!(matches!(
                ttc_to_pit(0.05, &params),
                Err(PdCalibrationError::NonFiniteValue { .. })
            ));
            assert!(matches!(
                pit_to_ttc(0.05, &params),
                Err(PdCalibrationError::NonFiniteValue { .. })
            ));
        }
    }
}

#[cfg(test)]
mod central_tendency_tests {
    use crate::credit::pd::{central_tendency, PdCalibrationError};

    #[test]
    fn single_year() {
        let result = central_tendency(&[0.03]).unwrap();
        assert!((result - 0.03).abs() < 1e-12);
    }

    #[test]
    fn arithmetic_mean() {
        // Arithmetic mean per Basel IRB / EBA GL/2017/16 (long-run average
        // default rate); previously pinned the geometric mean 0.02
        // (see  Major — credit).
        let result = central_tendency(&[0.01, 0.04]).unwrap();
        assert!(
            (result - 0.025).abs() < 1e-12,
            "expected=0.025, got={}",
            result
        );
    }

    #[test]
    fn zero_rate_years_are_included() {
        // Zero-default years are valid observations in the arithmetic
        // long-run average (previously rejected under the geometric mean;
        // see ).
        let result = central_tendency(&[0.0, 0.0, 0.0]).unwrap();
        assert!(result.abs() < 1e-15);

        let result = central_tendency(&[0.0, 0.02, 0.04]).unwrap();
        assert!((result - 0.02).abs() < 1e-12, "got={}", result);
    }

    #[test]
    fn empty_input() {
        assert!(matches!(
            central_tendency(&[]),
            Err(PdCalibrationError::EmptyInput)
        ));
    }

    #[test]
    fn out_of_range() {
        assert!(matches!(
            central_tendency(&[0.5, 1.5]),
            Err(PdCalibrationError::ValueOutOfRange { .. })
        ));
        assert!(matches!(
            central_tendency(&[-0.1, 0.5]),
            Err(PdCalibrationError::ValueOutOfRange { .. })
        ));
    }
}

#[cfg(test)]
mod term_structure_tests {
    use crate::credit::pd::{PdCalibrationError, PdTermStructureBuilder};

    #[test]
    fn basic_construction_and_interpolation() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.002), (3.0, 0.008), (5.0, 0.018)])
            .build()
            .unwrap();

        // At grid points
        assert!((ts.cumulative_pd(1.0) - 0.002).abs() < 1e-10);
        assert!((ts.cumulative_pd(3.0) - 0.008).abs() < 1e-10);
        assert!((ts.cumulative_pd(5.0) - 0.018).abs() < 1e-10);

        // Interpolated: should be between neighbors
        let pd_2y = ts.cumulative_pd(2.0);
        assert!(pd_2y > 0.002 && pd_2y < 0.008, "pd_2y={}", pd_2y);

        let pd_4y = ts.cumulative_pd(4.0);
        assert!(pd_4y > 0.008 && pd_4y < 0.018, "pd_4y={}", pd_4y);
    }

    #[test]
    fn monotonicity_at_t_zero() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01)])
            .build()
            .unwrap();

        assert_eq!(ts.cumulative_pd(0.0), 0.0);
        assert!(ts.cumulative_pd(0.5) > 0.0);
        assert!(ts.cumulative_pd(0.5) < 0.01);
    }

    #[test]
    fn extrapolation_beyond_last_tenor() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (5.0, 0.05)])
            .build()
            .unwrap();

        let pd_10 = ts.cumulative_pd(10.0);
        assert!(pd_10 > 0.05, "pd_10={}", pd_10);
        assert!(pd_10 < 1.0, "pd_10={}", pd_10);
    }

    #[test]
    fn marginal_pd() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (2.0, 0.025), (5.0, 0.06)])
            .build()
            .unwrap();

        let marginal = ts.marginal_pd(1.0, 2.0);
        // S(1) = 0.99, S(2) = 0.975 => marginal = (0.99-0.975)/0.99 ~ 0.01515
        assert!(marginal > 0.0, "marginal={}", marginal);
        assert!(marginal < 1.0, "marginal={}", marginal);
    }

    #[test]
    fn hazard_rate_positive() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (5.0, 0.05)])
            .build()
            .unwrap();

        assert!(ts.hazard_rate(0.5) > 0.0);
        assert!(ts.hazard_rate(3.0) > 0.0);
        assert!(ts.hazard_rate(7.0) > 0.0);
    }

    #[test]
    fn monotonicity_enforcement() {
        // Provide non-monotonic data; builder should fix it
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.05), (2.0, 0.03), (3.0, 0.08)])
            .build()
            .unwrap();

        let pds = ts.cumulative_pds();
        for i in 1..pds.len() {
            assert!(
                pds[i] >= pds[i - 1],
                "Non-monotonic: pds[{}]={} < pds[{}]={}",
                i,
                pds[i],
                i - 1,
                pds[i - 1]
            );
        }
    }

    /// Regression test for the broken pairwise-averaging "PAV"
    /// (): input
    /// [0.05, 0.03, 0.03] used to produce the *decreasing* output
    /// [0.0375, 0.0375, 0.035]. True weighted PAV pools all three points
    /// into one block with mean 0.11/3.
    #[test]
    fn pav_pools_decreasing_run_into_single_block() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.05), (2.0, 0.03), (3.0, 0.03)])
            .build()
            .unwrap();

        let expected = 0.11 / 3.0;
        for &pd in ts.cumulative_pds() {
            assert!(
                (pd - expected).abs() < 1e-12,
                "pd={}, expected={}",
                pd,
                expected
            );
        }
    }

    /// Second verified failing input from the review:
    /// [0.06, 0.05, 0.01, 0.02] used to yield [0.0394, 0.0394, 0.035, 0.0263]
    /// (decreasing). True PAV pools everything to mean 0.14/4 = 0.035.
    #[test]
    fn pav_merges_backward_across_blocks() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.06), (2.0, 0.05), (3.0, 0.01), (4.0, 0.02)])
            .build()
            .unwrap();

        for &pd in ts.cumulative_pds() {
            assert!((pd - 0.035).abs() < 1e-12, "pd={}", pd);
        }
    }

    /// PAV pooled-block values: scaled version of the canonical [3, 1, 1]
    /// example whose correct output is [5/3, 5/3, 5/3] (here /10 to stay in
    /// PD space): [0.3, 0.1, 0.1] -> [0.5/3, 0.5/3, 0.5/3].
    #[test]
    fn pav_canonical_pooled_average() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.3), (2.0, 0.1), (3.0, 0.1)])
            .build()
            .unwrap();

        let expected = 0.5 / 3.0;
        for &pd in ts.cumulative_pds() {
            assert!((pd - expected).abs() < 1e-12, "pd={}", pd);
        }
    }

    /// Property-style check over fixed non-monotone inputs: output must be
    /// monotone non-decreasing, and marginal PDs / hazard rates non-negative.
    #[test]
    fn pav_monotone_and_nonnegative_hazards_on_fixed_vectors() {
        let cases: [&[f64]; 5] = [
            &[0.05, 0.03, 0.03],
            &[0.06, 0.05, 0.01, 0.02],
            &[0.10, 0.02, 0.08, 0.04, 0.12],
            &[0.01, 0.01, 0.005, 0.02, 0.015, 0.03],
            &[0.20, 0.10, 0.30, 0.05, 0.25, 0.15, 0.35],
        ];

        for case in cases {
            let pairs: Vec<(f64, f64)> = case
                .iter()
                .enumerate()
                .map(|(i, &pd)| ((i + 1) as f64, pd))
                .collect();
            let ts = PdTermStructureBuilder::new()
                .with_cumulative_pds(&pairs)
                .build()
                .unwrap();

            let pds = ts.cumulative_pds();
            for i in 1..pds.len() {
                assert!(
                    pds[i] >= pds[i - 1],
                    "case {:?}: pds[{}]={} < pds[{}]={}",
                    case,
                    i,
                    pds[i],
                    i - 1,
                    pds[i - 1]
                );
            }

            for i in 1..pds.len() {
                let t1 = i as f64;
                let t2 = (i + 1) as f64;
                assert!(
                    ts.marginal_pd(t1, t2) >= 0.0,
                    "case {:?}: negative marginal PD on [{}, {}]",
                    case,
                    t1,
                    t2
                );
                assert!(
                    ts.hazard_rate(t1 + 0.5) >= 0.0,
                    "case {:?}: negative hazard at {}",
                    case,
                    t1 + 0.5
                );
            }
        }
    }

    #[test]
    fn empty_builder_fails() {
        assert!(matches!(
            PdTermStructureBuilder::new().build(),
            Err(PdCalibrationError::EmptyTermStructure)
        ));
    }

    #[test]
    fn invalid_tenor_fails() {
        assert!(matches!(
            PdTermStructureBuilder::new()
                .with_cumulative_pds(&[(0.0, 0.01)])
                .build(),
            Err(PdCalibrationError::InvalidTenor { .. })
        ));
        assert!(matches!(
            PdTermStructureBuilder::new()
                .with_cumulative_pds(&[(-1.0, 0.01)])
                .build(),
            Err(PdCalibrationError::InvalidTenor { .. })
        ));
    }

    #[test]
    fn non_finite_and_out_of_range_pds_fail_before_interpolation() {
        for pd in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                PdTermStructureBuilder::new()
                    .with_cumulative_pds(&[(1.0, pd)])
                    .build(),
                Err(PdCalibrationError::NonFiniteValue { .. })
            ));
        }
        for pd in [-0.01, 1.01] {
            assert!(matches!(
                PdTermStructureBuilder::new()
                    .with_cumulative_pds(&[(1.0, pd)])
                    .build(),
                Err(PdCalibrationError::ValueOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn non_finite_query_times_propagate_nan_without_panicking() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (3.0, 0.03)])
            .build()
            .unwrap();
        assert!(ts.cumulative_pd(f64::NAN).is_nan());
        assert!(ts.hazard_rate(f64::NAN).is_nan());
        assert!(ts.marginal_pd(1.0, f64::NAN).is_nan());
    }

    #[test]
    fn accessors() {
        let ts = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (3.0, 0.03)])
            .build()
            .unwrap();

        assert_eq!(ts.tenors(), &[1.0, 3.0]);
        assert_eq!(ts.cumulative_pds(), &[0.01, 0.03]);
    }
}

#[cfg(test)]
mod term_structure_from_matrix_tests {
    use crate::credit::migration::{RatingScale, TransitionMatrix};
    use crate::credit::pd::PdTermStructureBuilder;

    /// Simple 3-state matrix: AAA can default, BBB can default, D absorbing.
    #[test]
    fn from_transition_matrix_basic() {
        let scale =
            RatingScale::custom(vec!["AAA".to_string(), "BBB".to_string(), "D".to_string()])
                .unwrap();
        // AAA: 95% stay, 4% -> BBB, 1% -> D
        // BBB: 5% -> AAA, 90% stay, 5% -> D
        // D:   absorbing
        #[rustfmt::skip]
        let data = &[
            0.95, 0.04, 0.01,
            0.05, 0.90, 0.05,
            0.00, 0.00, 1.00,
        ];
        let tm = TransitionMatrix::new(scale, data, 1.0).unwrap();

        let ts = PdTermStructureBuilder::new()
            .from_transition_matrix(&tm, "AAA", &[1.0, 2.0, 5.0])
            .unwrap()
            .build()
            .unwrap();

        // 1-year PD for AAA should be 0.01
        assert!(
            (ts.cumulative_pd(1.0) - 0.01).abs() < 1e-10,
            "1y pd={}",
            ts.cumulative_pd(1.0)
        );
        // Multi-year PD should increase
        assert!(ts.cumulative_pd(2.0) > ts.cumulative_pd(1.0));
        assert!(ts.cumulative_pd(5.0) > ts.cumulative_pd(2.0));
    }

    #[rustfmt::skip]
    const SIMPLE_3STATE: &[f64] = &[
        0.95, 0.04, 0.01,
        0.05, 0.90, 0.05,
        0.00, 0.00, 1.00,
    ];

    fn simple_scale() -> RatingScale {
        RatingScale::custom(vec!["AAA".to_string(), "BBB".to_string(), "D".to_string()]).unwrap()
    }

    /// `from_transition_matrix` must scale
    /// tenors by `tm.horizon()`. A 6-month matrix reaches the 1y tenor via
    /// P^2 and the 0.5y tenor via P^1.
    #[test]
    fn from_transition_matrix_respects_horizon() {
        let tm_6m = TransitionMatrix::new(simple_scale(), SIMPLE_3STATE, 0.5).unwrap();

        let ts = PdTermStructureBuilder::new()
            .from_transition_matrix(&tm_6m, "AAA", &[0.5, 1.0, 2.0])
            .unwrap()
            .build()
            .unwrap();

        // 0.5y = one step: PD = P[AAA, D] = 0.01.
        assert!(
            (ts.cumulative_pd(0.5) - 0.01).abs() < 1e-12,
            "0.5y pd={}",
            ts.cumulative_pd(0.5)
        );
        // 1y = two steps: PD = (P^2)[AAA, D]
        //   = 0.95*0.01 + 0.04*0.05 + 0.01*1.0 = 0.0215.
        assert!(
            (ts.cumulative_pd(1.0) - 0.0215).abs() < 1e-12,
            "1y pd={}",
            ts.cumulative_pd(1.0)
        );
        assert!(ts.cumulative_pd(2.0) > ts.cumulative_pd(1.0));
    }

    /// tenors that are not integer multiples
    /// of the horizon must error (no silent rounding); the error suggests
    /// the generator-based `project` instead.
    #[test]
    fn from_transition_matrix_rejects_non_integer_multiple_tenor() {
        let tm_annual = TransitionMatrix::new(simple_scale(), SIMPLE_3STATE, 1.0).unwrap();

        let err = PdTermStructureBuilder::new()
            .from_transition_matrix(&tm_annual, "AAA", &[2.5])
            .expect_err("tenor 2.5 with an annual matrix must error");

        let message = err.to_string();
        match err {
            crate::credit::pd::PdCalibrationError::TenorNotMultipleOfHorizon { tenor, horizon } => {
                assert!((tenor - 2.5).abs() < 1e-12);
                assert!((horizon - 1.0).abs() < 1e-12);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            message.contains("project"),
            "error should suggest the generator-based projection: {message}"
        );
    }
}

#[cfg(test)]
mod master_scale_tests {
    use crate::credit::pd::{MasterScale, MasterScaleGrade, PdCalibrationError};

    #[test]
    fn sp_assumptions_v1_mapping() {
        let scale = MasterScale::sp_assumptions_v1().expect("registry scale");
        assert_eq!(scale.n_grades(), 8);

        // AAA: PD <= 0.0001
        let aaa = scale.map_pd(0.00005).unwrap();
        assert_eq!(aaa.grade, "AAA");
        assert_eq!(aaa.grade_index, 0);

        // BBB: PD <= 0.005
        let bbb = scale.map_pd(0.0015).unwrap();
        assert_eq!(bbb.grade, "BBB");
        assert_eq!(bbb.grade_index, 3);

        // B: PD <= 0.07
        let b = scale.map_pd(0.05).unwrap();
        assert_eq!(b.grade, "B");
        assert_eq!(b.grade_index, 5);

        // CC/C: PD > 0.25
        let ccc_plus = scale.map_pd(0.30).unwrap();
        assert_eq!(ccc_plus.grade, "CC/C");
        assert_eq!(ccc_plus.grade_index, 7);
    }

    #[test]
    fn moodys_assumptions_v1_mapping() {
        let scale = MasterScale::moodys_assumptions_v1().expect("registry scale");
        assert_eq!(scale.n_grades(), 8);

        let baa = scale.map_pd(0.003).unwrap();
        assert_eq!(baa.grade, "Baa");
    }

    #[test]
    fn deprecated_registry_ids_resolve_to_canonical_assumptions() {
        for (canonical, aliases) in [
            (
                "sp_assumptions_v1",
                ["sp_empirical", "sp_corporate_default_1981_2023"],
            ),
            (
                "moodys_assumptions_v1",
                ["moodys_empirical", "moodys_default_1983_2023"],
            ),
        ] {
            let canonical_scale =
                MasterScale::from_registry_id(canonical).expect("canonical scale");
            for alias in aliases {
                let alias_scale =
                    MasterScale::from_registry_id(alias).expect("deprecated alias must resolve");
                assert_eq!(alias_scale.n_grades(), canonical_scale.n_grades());
                assert_eq!(
                    alias_scale.grades()[0].label,
                    canonical_scale.grades()[0].label
                );
            }
        }
    }

    #[test]
    fn pd_exceeds_all_grades() {
        let scale = MasterScale::sp_empirical().expect("registry scale");
        let result = scale.map_pd(1.5).unwrap();
        assert_eq!(result.grade, "CC/C");
        assert_eq!(result.grade_index, 7);
    }

    #[test]
    fn pd_at_boundary() {
        let scale = MasterScale::sp_empirical().expect("registry scale");
        // Exactly at AAA upper boundary (0.0001)
        let result = scale.map_pd(0.0001).unwrap();
        assert_eq!(result.grade, "AAA");

        // Just above AAA boundary
        let result = scale.map_pd(0.00011).unwrap();
        assert_eq!(result.grade, "AA");
    }

    #[test]
    fn custom_scale() {
        let grades = vec![
            MasterScaleGrade {
                label: "Good".to_owned(),
                upper_pd: 0.01,
                central_pd: 0.005,
            },
            MasterScaleGrade {
                label: "Medium".to_owned(),
                upper_pd: 0.10,
                central_pd: 0.05,
            },
            MasterScaleGrade {
                label: "Bad".to_owned(),
                upper_pd: 1.0,
                central_pd: 0.50,
            },
        ];
        let scale = MasterScale::new(grades).unwrap();
        assert_eq!(scale.n_grades(), 3);
        assert_eq!(scale.map_pd(0.005).unwrap().grade, "Good");
        assert_eq!(scale.map_pd(0.05).unwrap().grade, "Medium");
        assert_eq!(scale.map_pd(0.80).unwrap().grade, "Bad");
    }

    #[test]
    fn empty_grades_fails() {
        assert!(matches!(
            MasterScale::new(vec![]),
            Err(PdCalibrationError::EmptyInput)
        ));
    }

    #[test]
    fn unsorted_grades_fails() {
        let grades = vec![
            MasterScaleGrade {
                label: "B".to_owned(),
                upper_pd: 0.10,
                central_pd: 0.05,
            },
            MasterScaleGrade {
                label: "A".to_owned(),
                upper_pd: 0.01,
                central_pd: 0.005,
            },
        ];
        assert!(matches!(
            MasterScale::new(grades),
            Err(PdCalibrationError::GradesNotSorted)
        ));
    }

    #[test]
    fn map_score_uses_implied_pd() {
        use crate::credit::scoring::{
            altman_z_score, altman_z_score_with_pd, AltmanPdCalibration, AltmanZScoreInput,
        };

        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.80,
        };
        let scale = MasterScale::sp_empirical().expect("registry scale");
        let uncalibrated = altman_z_score(&input).unwrap();
        assert!(matches!(
            scale.map_score(&uncalibrated),
            Err(PdCalibrationError::MissingImpliedPd)
        ));

        let scoring_result =
            altman_z_score_with_pd(&input, AltmanPdCalibration::HeuristicV1).unwrap();
        let mapped = scale.map_score(&scoring_result).unwrap();
        assert_eq!(Some(mapped.input_pd), scoring_result.implied_pd);
        // Safe zone has low PD, should not be in the worst grades
        assert!(
            mapped.grade_index < scale.n_grades() - 1,
            "grade={}",
            mapped.grade
        );
    }

    #[test]
    fn grades_accessor() {
        let scale = MasterScale::sp_empirical().expect("registry scale");
        let grades = scale.grades();
        assert_eq!(grades.len(), 8);
        assert_eq!(grades[0].label, "AAA");
        assert_eq!(grades[7].label, "CC/C");
    }

    /// A NaN PD previously fell through
    /// every comparison and silently mapped to the worst grade; it must now
    /// be a validation error.
    #[test]
    fn map_pd_rejects_non_finite() {
        let scale = MasterScale::sp_empirical().expect("registry scale");
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                scale.map_pd(bad),
                Err(PdCalibrationError::NonFiniteValue { .. })
            ));
        }
    }
}

#[cfg(test)]
mod serde_invariant_tests {
    use crate::credit::pd::{
        MasterScale, MasterScaleGrade, PdTermStructure, PdTermStructureBuilder,
    };

    #[test]
    fn malformed_pd_types_fail_deserialization() {
        let term = PdTermStructureBuilder::new()
            .with_cumulative_pds(&[(1.0, 0.01), (2.0, 0.02)])
            .build()
            .expect("term structure");
        let mut term_json = serde_json::to_value(&term).expect("serialize");
        term_json["tenors"] = serde_json::json!([2.0, 1.0]);
        assert!(serde_json::from_value::<PdTermStructure>(term_json).is_err());
        let mut mismatched_json = serde_json::to_value(&term).expect("serialize");
        mismatched_json["cumulative_pds"] = serde_json::json!([0.01]);
        assert!(serde_json::from_value::<PdTermStructure>(mismatched_json).is_err());

        let scale = MasterScale::new(vec![
            MasterScaleGrade {
                label: "A".to_string(),
                upper_pd: 0.01,
                central_pd: 0.005,
            },
            MasterScaleGrade {
                label: "B".to_string(),
                upper_pd: 0.10,
                central_pd: 0.05,
            },
        ])
        .expect("master scale");
        let mut scale_json = serde_json::to_value(&scale).expect("serialize");
        scale_json["grades"][1]["upper_pd"] = serde_json::json!(0.001);
        assert!(serde_json::from_value::<MasterScale>(scale_json).is_err());
    }
}
