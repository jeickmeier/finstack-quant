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
mod master_scale_tests {
    use crate::credit::pd::{MasterScale, MasterScaleGrade, PdCalibrationError};

    #[test]
    fn sp_assumptions_mapping() {
        let scale = MasterScale::default_calibration().expect("registry scale");
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
    fn moodys_assumptions_mapping() {
        let scale = MasterScale::from_registry_id("moodys_assumptions").expect("registry scale");
        assert_eq!(scale.n_grades(), 8);

        let baa = scale.map_pd(0.003).unwrap();
        assert_eq!(baa.grade, "Baa");
    }

    #[test]
    fn pd_at_unit_maps_to_worst_grade() {
        let scale = MasterScale::default_calibration().expect("registry scale");
        let result = scale.map_pd(1.0).unwrap();
        assert_eq!(result.grade, "CC/C");
        assert_eq!(result.grade_index, 7);
    }

    #[test]
    fn pd_outside_unit_interval_is_rejected() {
        let scale = MasterScale::default_calibration().expect("registry scale");
        for bad in [-0.1, 1.5, 5.0] {
            assert!(matches!(
                scale.map_pd(bad),
                Err(PdCalibrationError::ValueOutOfRange { value, min, max })
                    if value == bad && min == 0.0 && max == 1.0
            ));
        }
    }

    #[test]
    fn pd_at_boundary() {
        let scale = MasterScale::default_calibration().expect("registry scale");
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
    fn map_score_rejects_uncalibrated_score() {
        use crate::credit::scoring::{altman_z_score, AltmanZScoreInput};

        let input = AltmanZScoreInput {
            working_capital_to_total_assets: 0.10,
            retained_earnings_to_total_assets: 0.20,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.50,
            sales_to_total_assets: 1.80,
        };
        let scale = MasterScale::default_calibration().expect("registry scale");
        let uncalibrated = altman_z_score(&input).unwrap();
        assert!(matches!(
            scale.map_score(&uncalibrated),
            Err(PdCalibrationError::MissingImpliedPd)
        ));
    }

    #[test]
    fn grades_accessor() {
        let scale = MasterScale::default_calibration().expect("registry scale");
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
        let scale = MasterScale::default_calibration().expect("registry scale");
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
    use crate::credit::pd::{MasterScale, MasterScaleGrade};

    #[test]
    fn malformed_pd_types_fail_deserialization() {
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
