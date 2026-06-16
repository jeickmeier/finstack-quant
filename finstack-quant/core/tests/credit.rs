//! Cross-module credit workflow integration tests.

use finstack_quant_core::credit::lgd::{
    CollateralPiece, CollateralType, DownturnLgd, EadCalculator, WorkoutCosts, WorkoutLgd,
};
use finstack_quant_core::credit::migration::{
    projection, GeneratorMatrix, MigrationSimulator, RatingScale, TransitionMatrix,
};
use finstack_quant_core::credit::pd::{MasterScale, PdTermStructureBuilder};
use finstack_quant_core::credit::scoring::{altman_z_score, AltmanZScoreInput};
use rand::SeedableRng;
use rand_pcg::Pcg64;

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
fn credit_workflow_maps_scoring_migration_and_loss_modules_together() {
    let annual = reference_transition_matrix();
    let generator = GeneratorMatrix::from_transition_matrix(&annual).unwrap();
    let half_year = projection::project(&generator, 0.5).unwrap();
    let term_structure = PdTermStructureBuilder::new()
        .from_transition_matrix(&half_year, "BBB", &[0.5, 1.0, 2.0])
        .unwrap()
        .build()
        .unwrap();

    let pd_1y = term_structure.cumulative_pd(1.0);
    assert!(pd_1y > 0.0);
    assert!(pd_1y < term_structure.cumulative_pd(2.0));

    let score = altman_z_score(&AltmanZScoreInput {
        working_capital_to_total_assets: 0.10,
        retained_earnings_to_total_assets: 0.20,
        ebit_to_total_assets: 0.15,
        market_equity_to_total_liabilities: 1.50,
        sales_to_total_assets: 1.80,
    })
    .unwrap();
    let mapped = MasterScale::sp_empirical()
        .unwrap()
        .map_score(&score)
        .unwrap();
    assert_eq!(mapped.input_pd, score.implied_pd);
    assert!(mapped.central_pd > 0.0);

    let ead = EadCalculator::revolver(60.0, 40.0).unwrap().ead();
    let workout = WorkoutLgd::builder()
        .collateral(CollateralPiece::new(CollateralType::RealEstate, 75.0, 0.30).unwrap())
        .workout_years(2.0)
        .discount_rate(0.06)
        .costs(WorkoutCosts::new(0.04, 0.03).unwrap())
        .build()
        .unwrap();
    let base_lgd = workout.lgd(ead).unwrap();
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
    let mut rng1 = Pcg64::seed_from_u64(12345);
    let mut rng2 = Pcg64::seed_from_u64(12345);

    let first = simulator.empirical_matrix(1_000, &mut rng1);
    let second = simulator.empirical_matrix(1_000, &mut rng2);

    for i in 0..first.n_states() {
        for j in 0..first.n_states() {
            assert_eq!(
                first.probability_by_index(i, j),
                second.probability_by_index(i, j)
            );
        }
    }
}
