//! Tests for the Rust functions that replaced logic the host bindings used to
//! carry themselves (seeded simulation, batch lookups, labels and checked
//! accessors). Python and WebAssembly both call these, so each case pins the
//! one behaviour the two hosts share.

use finstack_quant_core::error::ErrorKind;
use finstack_quant_models::correlation::LatentMultiFactor;
use finstack_quant_models::credit::migration::{
    default_rate, GeneratorMatrix, MigrationSimulator, RatingScale, TransitionMatrix,
};
use finstack_quant_models::credit::pd::{MasterScale, MasterScaleGrade, PdCalibrationError};
use finstack_quant_models::credit::scoring::CreditScoringError;

fn three_state_scale() -> RatingScale {
    RatingScale::custom(vec!["A".into(), "B".into(), "D".into()]).expect("valid scale")
}

fn three_state_generator() -> GeneratorMatrix {
    GeneratorMatrix::new(
        three_state_scale(),
        &[-0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0.0, 0.0, 0.0],
    )
    .expect("valid generator")
}

#[test]
fn transition_matrix_checked_index_lookup_and_rows() {
    let data = [0.9, 0.08, 0.02, 0.1, 0.8, 0.1, 0.0, 0.0, 1.0];
    let matrix = TransitionMatrix::new(three_state_scale(), &data, 1.0).expect("valid matrix");

    assert_eq!(matrix.probability_by_index(0, 2).expect("in range"), 0.02);
    let err = matrix.probability_by_index(0, 3).expect_err("out of range");
    assert_eq!(err.kind(), ErrorKind::Validation);

    let rows = matrix.to_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1], vec![0.1, 0.8, 0.1]);
    assert_eq!(three_state_generator().to_rows()[0], vec![-0.1, 0.08, 0.02]);
}

#[test]
fn seeded_migration_simulation_is_reproducible() {
    let simulator = MigrationSimulator::new(three_state_generator(), 5.0).expect("valid horizon");
    let first = simulator.simulate(0, 200, 7).expect("valid state");
    let second = simulator.simulate(0, 200, 7).expect("valid state");
    assert_eq!(
        serde_json::to_string(&first).expect("serialize"),
        serde_json::to_string(&second).expect("serialize")
    );

    let rate = default_rate(&first);
    assert!(rate > 0.0 && rate < 1.0, "default rate {rate}");
    assert_eq!(default_rate(&[]), 0.0);

    let empirical = simulator
        .empirical_matrix(500, 7)
        .expect("positive path count");
    let again = simulator
        .empirical_matrix(500, 7)
        .expect("positive path count");
    assert_eq!(empirical.to_rows(), again.to_rows());
    assert!(simulator.empirical_matrix(0, 7).is_err());
    assert!(simulator.simulate(9, 1, 7).is_err());
}

#[test]
fn master_scale_maps_batches_in_order_and_rejects_bad_pds() {
    let scale = MasterScale::new(vec![
        MasterScaleGrade {
            label: "A".into(),
            upper_pd: 0.01,
            central_pd: 0.005,
        },
        MasterScaleGrade {
            label: "B".into(),
            upper_pd: 0.10,
            central_pd: 0.04,
        },
        MasterScaleGrade {
            label: "C".into(),
            upper_pd: 1.0,
            central_pd: 0.30,
        },
    ])
    .expect("valid scale");

    let grades: Vec<String> = scale
        .map_pds(&[0.5, 0.001, 0.05])
        .expect("valid pds")
        .into_iter()
        .map(|result| result.grade)
        .collect();
    assert_eq!(grades, ["C", "A", "B"]);
    assert!(scale.map_pds(&[]).expect("empty batch").is_empty());

    let err = scale.map_pds(&[0.01, 5.0]).expect_err("5.0 is not a PD");
    assert!(matches!(err, PdCalibrationError::ValueOutOfRange { .. }));
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert_eq!(
        CreditScoringError::NonFiniteInput {
            field: "x",
            value: f64::NAN
        }
        .kind(),
        ErrorKind::Validation
    );
}

#[test]
fn latent_multi_factor_rejects_a_draw_vector_of_the_wrong_length() {
    let model = LatentMultiFactor::uncorrelated(2, vec![0.2, 0.3]).expect("valid factors");
    assert_eq!(
        model
            .generate_correlated_factors(&[1.0, -1.0])
            .expect("two draws"),
        vec![0.2, -0.3]
    );
    let err = model
        .generate_correlated_factors(&[1.0])
        .expect_err("one draw for two factors");
    let core: finstack_quant_core::Error = err.into();
    assert_eq!(core.kind(), ErrorKind::Validation);
    assert!(core.to_string().contains("expected 2"), "{core}");
}
