use super::*;
use crate::correlation::error::Error;

#[test]
fn mixing_draws_reject_invalid_uniforms_for_every_sampling_family() {
    let copulas: [Box<dyn Copula>; 3] = [
        Box::new(GaussianCopula::new()),
        Box::new(StudentTCopula::new(5.0).expect("valid df")),
        Box::new(RandomFactorLoadingCopula::new(0.1).expect("valid loading vol")),
    ];
    for copula in copulas {
        for u in [0.0, 1.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(
                copula.sample_mixing(u).is_err(),
                "{}: u={u}",
                copula.model_name()
            );
        }
        assert!(copula.sample_mixing(0.5).expect("valid draw").is_finite());
    }
}

#[test]
fn test_copula_spec_default() {
    let spec = CopulaSpec::default();
    assert_eq!(spec, CopulaSpec::Gaussian);
}

#[test]
fn test_copula_spec_builders() {
    let gaussian = CopulaSpec::gaussian();
    assert!(matches!(gaussian, CopulaSpec::Gaussian));

    let student_t = CopulaSpec::student_t(5.0).expect("valid Student-t df");
    assert!(matches!(
        student_t,
        CopulaSpec::StudentT {
            degrees_of_freedom: df
        } if (df - 5.0).abs() < 1e-10
    ));

    let rfl = CopulaSpec::random_factor_loading(0.15);
    assert!(matches!(
        rfl,
        CopulaSpec::RandomFactorLoading {
            loading_volatility: v
        } if (v - 0.15).abs() < 1e-10
    ));
}

#[test]
fn test_student_t_invalid_df_is_rejected() {
    assert!(matches!(
        CopulaSpec::student_t(2.0),
        Err(Error::InvalidStudentTDegreesOfFreedom { .. })
    ));
}

#[test]
fn test_copula_build() {
    let gaussian = CopulaSpec::gaussian();
    assert!(gaussian.is_gaussian());
    let g_copula = gaussian.build().expect("Gaussian copula should build");
    assert_eq!(g_copula.num_factors(), 1);

    let student_t = CopulaSpec::student_t(5.0).expect("valid Student-t df");
    assert!(student_t.is_student_t());
    let t_copula = student_t.build().expect("Student-t copula should build");
    // [Z, W]: Gaussian systematic factor plus the shared mixing variable.
    assert_eq!(t_copula.num_factors(), 2);

    let rfl = CopulaSpec::random_factor_loading(0.1);
    assert!(rfl.is_rfl());
    let rfl_copula = rfl.build().expect("RFL copula should build");
    assert_eq!(rfl_copula.num_factors(), 2);

    let mf = CopulaSpec::multi_factor();
    assert!(mf.is_multi_factor());
    let mf_copula = mf.build().expect("multi-factor copula should build");
    assert_eq!(mf_copula.num_factors(), 2);
}

#[test]
fn test_deserialized_invalid_student_t_df_is_rejected_on_build() {
    // Simulate config file with invalid df <= 2
    let spec: CopulaSpec = serde_json::from_str(r#"{"type":"student_t","degrees_of_freedom":1.5}"#)
        .expect("should deserialize");
    assert!(matches!(
        spec.build(),
        Err(Error::InvalidStudentTDegreesOfFreedom { .. })
    ));
}

#[test]
fn invalid_rfl_loading_volatility_is_rejected_before_construction() {
    for value in [f64::NAN, f64::INFINITY, -0.1, 0.500_001] {
        let spec = CopulaSpec::RandomFactorLoading {
            loading_volatility: value,
        };
        assert!(matches!(
            spec.build(),
            Err(Error::InvalidLoadingVolatility { .. })
        ));
    }
    let spec: CopulaSpec =
        serde_json::from_str(r#"{"type":"random_factor_loading","loading_volatility":0.7}"#)
            .expect("syntactically valid persisted spec");
    assert!(spec.build().is_err());
}
