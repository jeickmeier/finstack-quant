use super::*;

#[test]
fn seniority_recovery_stats_accepts_binding_strings() {
    let stats = seniority_recovery_stats("senior_secured", "sp_recovery_historical").unwrap();
    assert!((stats.mean() - 0.53).abs() < 1e-12);
}

#[test]
fn seniority_calibration_ids_lists_the_default_first_and_each_resolves() {
    let ids = seniority_calibration_ids().unwrap();
    assert_eq!(ids, ["moodys_recovery_1982_2023", "sp_recovery_historical"]);
    for id in &ids {
        assert!(seniority_recovery_stats("senior_secured", id).is_ok());
    }
}

#[test]
fn beta_recovery_sample_is_seeded() {
    let first = BetaRecovery::new(0.4, 0.2)
        .unwrap()
        .sample_seeded(4, 42)
        .unwrap();
    let second = BetaRecovery::new(0.4, 0.2)
        .unwrap()
        .sample_seeded(4, 42)
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn workout_lgd_returns_net_recovery_and_lgd() {
    let WorkoutLgdResult {
        net_recovery,
        lgd,
        recovery_rate,
    } = WorkoutLgd::builder()
        .collateral(CollateralPiece::new(WorkoutCollateralType::RealEstate, 80.0, 0.30).unwrap())
        .workout_years(2.0)
        .discount_rate(0.05)
        .costs(WorkoutCosts::new(0.05, 0.03).unwrap())
        .build()
        .unwrap()
        .evaluate(100.0)
        .unwrap();

    // net = (gross − costs·EAD) · DF = (56 − 8) / 1.05² — workout costs
    // are discounted alongside recoveries per the Basel workout-LGD
    // methodology ().
    let expected_net = 48.0 / (1.05_f64 * 1.05);
    assert!((net_recovery - expected_net).abs() < 1e-12);
    assert!((lgd - (1.0 - expected_net / 100.0)).abs() < 1e-12);
    assert!((recovery_rate - (1.0 - lgd)).abs() < 1e-12);
}

#[test]
fn workout_lgd_rejects_noncanonical_collateral_type() {
    let json = serde_json::json!({
        "collateral": [{"collateral_type": "real-estate", "book_value": 80.0, "haircut": 0.30}],
        "workout_years": 2.0,
        "discount_rate": 0.05,
        "costs": {"direct_cost_rate": 0.05, "indirect_cost_rate": 0.03},
    });
    assert!(serde_json::from_value::<WorkoutLgd>(json).is_err());
}
