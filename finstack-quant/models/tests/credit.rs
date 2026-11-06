//! Cross-module credit workflow integration tests.

use finstack_quant_models::credit::lgd::{
    CollateralPiece, DownturnLgd, EadCalculator, WorkoutCollateralType, WorkoutCosts, WorkoutLgd,
};
use finstack_quant_models::credit::migration::{
    GeneratorMatrix, MigrationSimulator, RatingScale, TransitionMatrix, project,
};
use finstack_quant_models::credit::pd::MasterScale;

fn reference_transition_matrix() -> TransitionMatrix {
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

    TransitionMatrix::new(scale, &data, 1.0).unwrap()
}

#[test]
fn credit_workflow_maps_migration_and_loss_modules_together() {
    let annual = reference_transition_matrix();
    let generator = GeneratorMatrix::from_transition_matrix(&annual, None).unwrap();
    let half_year = project(&generator, 0.5).unwrap();

    // Propagate all surviving rating states across both half-year periods.
    let pd_half = half_year.probability("BBB", "D").unwrap();
    let pd_1y = half_year
        .compose(&half_year)
        .unwrap()
        .probability("BBB", "D")
        .unwrap();
    let direct_pd = project(&generator, 1.0)
        .unwrap()
        .probability("BBB", "D")
        .unwrap();
    assert!((pd_1y - direct_pd).abs() < 1e-12);
    assert!(pd_1y > 0.0);
    assert!(pd_1y > pd_half, "one-year PD must exceed the half-year PD");

    let mapped = MasterScale::default_calibration()
        .unwrap()
        .map_pd(pd_1y)
        .unwrap();
    assert!(mapped.central_pd > 0.0);

    let ead = EadCalculator::revolver(60.0, 40.0).unwrap().ead();
    let workout = WorkoutLgd::builder()
        .collateral(CollateralPiece::new(WorkoutCollateralType::RealEstate, 75.0, 0.30).unwrap())
        .workout_years(2.0)
        .discount_rate(0.06)
        .costs(WorkoutCosts::new(0.04, 0.03).unwrap())
        .build()
        .unwrap();
    let base_lgd = workout.evaluate(ead).unwrap().lgd;
    let downturn_lgd = DownturnLgd::regulatory_floor(0.08, 0.25)
        .unwrap()
        .adjust(base_lgd)
        .unwrap();
    let expected_loss = ead * pd_1y * downturn_lgd;

    assert!(base_lgd > 0.0 && base_lgd < 1.0);
    assert!(downturn_lgd >= base_lgd);
    assert!(expected_loss > 0.0);
}

#[test]
fn migration_empirical_matrix_is_reproducible_for_same_seed() {
    let scale = RatingScale::custom(vec!["IG".to_string(), "D".to_string()]).unwrap();
    let generator = GeneratorMatrix::new(scale, &[-0.1, 0.1, 0.0, 0.0]).unwrap();
    let simulator = MigrationSimulator::new(generator, 1.0).unwrap();
    let first = simulator.empirical_matrix(1_000, 12345).unwrap();
    let second = simulator.empirical_matrix(1_000, 12345).unwrap();

    for i in 0..first.n_states() {
        for j in 0..first.n_states() {
            assert_eq!(
                first.probability_by_index(i, j).unwrap(),
                second.probability_by_index(i, j).unwrap()
            );
        }
    }
}

#[test]
fn merton_model_json_rejects_unknown_fields() {
    use finstack_quant_models::credit::MertonModel;

    let valid = serde_json::json!({
        "asset_value": 100.0,
        "asset_vol": 0.25,
        "debt_barrier": 80.0,
        "risk_free_rate": 0.03,
        "payout_rate": 0.0,
        "barrier_type": "terminal",
        "dynamics": "geometric_brownian"
    });
    serde_json::from_value::<MertonModel>(valid.clone()).expect("valid Merton model JSON");

    let mut nested = valid.clone();
    nested["dynamics"] = serde_json::json!({
        "jump_diffusion": {"jump_intensity": 1.0, "jump_mean": 0.0, "jump_vol": 0.1, "typo": 5.0}
    });
    let error = serde_json::from_value::<MertonModel>(nested)
        .expect_err("an unknown dynamics field must not be silently ignored");
    assert!(
        error.to_string().contains("unknown field `typo`"),
        "{error}"
    );
    let mut barrier = valid.clone();
    barrier["barrier_type"] = serde_json::json!({
        "first_passage": {"barrier_growth_rate": 0.0, "typo": 1.0}
    });
    let error = serde_json::from_value::<MertonModel>(barrier)
        .expect_err("an unknown barrier field must not be silently ignored");
    assert!(
        error.to_string().contains("unknown field `typo`"),
        "{error}"
    );

    let mut misspelled = valid;
    misspelled["asset_volatility"] = serde_json::json!(0.4);
    let error = serde_json::from_value::<MertonModel>(misspelled)
        .expect_err("an unknown field must not be silently ignored");
    assert!(
        error
            .to_string()
            .contains("unknown field `asset_volatility`"),
        "{error}"
    );
}
