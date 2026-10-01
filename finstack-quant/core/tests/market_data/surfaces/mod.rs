//! Market data surface tests.

#[test]
fn sabr_zero_vol_of_vol_survives_cube_construction_and_serde() {
    use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};

    let node = SabrParameterData::new(0.2, 1.0, -0.5, 0.0).unwrap();
    let cube = VolCube::from_grid("BLACK", &[1.0, 2.0], &[5.0], &[node; 2], &[0.03; 2]).unwrap();
    let restored: VolCube = serde_json::from_str(&serde_json::to_string(&cube).unwrap()).unwrap();
    assert_eq!(restored.params_at(0, 0).nu, 0.0);
    assert_eq!(restored.params_at(1, 0), &node);
    for nu in [-0.01, f64::NAN, f64::INFINITY] {
        assert!(SabrParameterData::new(0.2, 1.0, -0.5, nu).is_err());
    }
}
