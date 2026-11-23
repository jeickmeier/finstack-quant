use super::loader::*;
use super::simm_validation::*;
use super::*;
use crate::types::{CollateralAssetClass, SimmCreditSector, SimmRiskClass, ordered_pair};

#[test]
fn embedded_registry_loads_successfully() {
    let registry = embedded_registry().expect("embedded registry should load");

    assert!(
        !registry.schedule_im.is_empty(),
        "schedule_im should have entries"
    );
    assert!(
        registry.schedule_im.contains_key("bcbs_iosco"),
        "bcbs_iosco schedule should be present"
    );

    assert!(
        !registry.collateral_schedules.is_empty(),
        "collateral_schedules should have entries"
    );
    assert!(
        registry.collateral_schedules.contains_key("bcbs_standard"),
        "bcbs_standard collateral schedule should be present"
    );
    assert!(
        registry.collateral_schedules.contains_key("cash_only"),
        "cash_only collateral schedule should be present"
    );

    assert!(
        !registry.collateral_asset_class_defaults.is_empty(),
        "collateral_asset_class_defaults should have entries"
    );
    assert!(
        registry
            .collateral_asset_class_defaults
            .contains_key(&CollateralAssetClass::Cash),
        "Cash default should be present"
    );

    assert!(
        registry.simm.contains_key("v2_6"),
        "v2_6 SIMM params should be present"
    );

    assert!(!registry.ccp.is_empty(), "ccp should have entries");
    assert!(
        registry.ccp_default_params.mpor_days > 0,
        "ccp default params should be resolved"
    );
    assert!(
        registry.defaults.im.simm.mpor_days > 0,
        "simm mpor_days should be positive"
    );
}

#[test]
fn simm_params_have_required_weights() {
    let registry = embedded_registry().expect("embedded registry should load");
    let params = registry.simm.get("v2_6").expect("v2_6 simm params");

    assert!(
        params.ir_delta_weights.contains_key("5Y"),
        "5y IR weight should be present"
    );
    assert!(
        params
            .cq_bucket_weights
            .contains_key(&SimmCreditSector::Financial),
        "financial CQ bucket weight should be present"
    );
    assert!(params.cnq_delta_weight > 0.0, "CNQ weight should be > 0");
    assert!(
        params.equity_delta_weight > 0.0,
        "equity weight should be > 0"
    );
    assert!(params.fx_delta_weight > 0.0, "FX weight should be > 0");
    assert!(
        !params.risk_class_correlations.is_empty(),
        "risk class correlations should be populated"
    );
}

// ---- PSD validation at registry load --------------------------------------

/// Clone the embedded v2_6 SIMM params for mutation in PSD tests.
fn base_simm_params() -> SimmParams {
    let registry = embedded_registry().expect("embedded registry should load");
    registry.simm.get("v2_6").expect("v2_6 simm params").clone()
}

#[test]
fn psd_check_accepts_embedded_registry() {
    // Regression guard: shipped ISDA correlation matrices must continue to
    // satisfy PSD after the new check is wired in.
    let params = base_simm_params();
    validate_simm_correlations_psd(&params).expect("embedded SIMM correlations must be PSD");
}

#[test]
fn simm_v26_credit_qualifying_tables_are_registry_backed() {
    let registry = embedded_registry().expect("embedded registry should load");
    let params = registry.simm.get("v2_6").expect("v2_6 params");

    assert_eq!(
        params
            .cq_bucket_weights
            .get(&SimmCreditSector::Sovereign)
            .copied(),
        Some(75.0)
    );
    assert_eq!(
        params
            .cq_inter_bucket_correlations
            .get(&ordered_pair(
                SimmCreditSector::Sovereign,
                SimmCreditSector::Financial
            ))
            .copied(),
        Some(0.38)
    );
    assert_eq!(
        params
            .cq_concentration_thresholds
            .get(&SimmCreditSector::Sovereign)
            .copied(),
        Some(1_000_000.0)
    );
}

#[test]
fn psd_check_rejects_non_psd_risk_class_matrix() {
    // Non-PSD 3-way pattern: ρ(IR,CQ)=0.9, ρ(IR,EQ)=0.9, ρ(CQ,EQ)=-0.9.
    // Determinant of the embedded 3x3 submatrix is -2.888 < 0 → one negative
    // eigenvalue → NOT positive semi-definite, even though every entry is in
    // [-1, 1]. Exactly the silent-propagation failure mode the PSD
    // check targets.
    let mut params = base_simm_params();
    params.risk_class_correlations.insert(
        ordered_pair(SimmRiskClass::InterestRate, SimmRiskClass::CreditQualifying),
        0.9,
    );
    params.risk_class_correlations.insert(
        ordered_pair(SimmRiskClass::InterestRate, SimmRiskClass::Equity),
        0.9,
    );
    params.risk_class_correlations.insert(
        ordered_pair(SimmRiskClass::CreditQualifying, SimmRiskClass::Equity),
        -0.9,
    );

    let err = validate_simm_correlations_psd(&params)
        .expect_err("non-PSD risk-class matrix must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("risk_class_correlations"),
        "error should name the offending matrix: {msg}"
    );
}

#[test]
fn psd_check_rejects_non_psd_cq_inter_bucket_matrix() {
    // Same 3-way non-PSD pattern applied to the CQ inter-bucket matrix.
    let mut params = base_simm_params();
    params.cq_inter_bucket_correlations.insert(
        ordered_pair(SimmCreditSector::Sovereign, SimmCreditSector::Financial),
        0.9,
    );
    params.cq_inter_bucket_correlations.insert(
        ordered_pair(
            SimmCreditSector::Sovereign,
            SimmCreditSector::BasicMaterials,
        ),
        0.9,
    );
    params.cq_inter_bucket_correlations.insert(
        ordered_pair(
            SimmCreditSector::Financial,
            SimmCreditSector::BasicMaterials,
        ),
        -0.9,
    );

    let err = validate_simm_correlations_psd(&params)
        .expect_err("non-PSD CQ inter-bucket matrix must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("cq_inter_bucket_correlations"),
        "error should name the offending matrix: {msg}"
    );
}

#[test]
fn psd_check_accepts_registry_commodity_matrix() {
    // Regression guard: the ISDA SIMM commodity matrix loaded from the
    // registry must continue to pass PSD validation after future edits.
    let params = base_simm_params();
    validate_simm_correlations_psd(&params).expect("embedded commodity 17x17 matrix must be PSD");
}

#[test]
fn psd_check_rejects_non_psd_commodity_matrix() {
    // Same 3-way non-PSD pattern applied to the commodity matrix.
    // Write (i,j) and (j,i) symmetrically so only the PSD branch fires.
    let mut params = base_simm_params();
    let n = usize::from(crate::types::SIMM_COMMODITY_BUCKET_COUNT);
    let set = |m: &mut Vec<f64>, i: usize, j: usize, v: f64| {
        m[i * n + j] = v;
        m[j * n + i] = v;
    };
    set(&mut params.commodity_inter_bucket_correlations, 0, 1, 0.9);
    set(&mut params.commodity_inter_bucket_correlations, 0, 2, 0.9);
    set(&mut params.commodity_inter_bucket_correlations, 1, 2, -0.9);

    let err = validate_simm_correlations_psd(&params)
        .expect_err("non-PSD commodity matrix must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("commodity_inter_bucket_correlations"),
        "error should name the offending matrix: {msg}"
    );
}

#[test]
fn psd_check_rejects_wrong_shape_commodity_matrix() {
    let mut params = base_simm_params();
    params.commodity_inter_bucket_correlations.truncate(16 * 16);
    let err = validate_simm_correlations_psd(&params)
        .expect_err("wrong-shape commodity matrix must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("commodity_inter_bucket_correlations") && msg.contains("expected"),
        "error should name the matrix and expected shape: {msg}"
    );
}

#[test]
fn psd_check_rejects_out_of_range_correlation() {
    // |ρ| > 1 is a range-check failure (validate_correlation_matrix catches
    // it before the Cholesky stage). Still routed through the PSD validator
    // so the registry rejects at load time rather than producing complex
    // sqrt downstream.
    let mut params = base_simm_params();
    params.risk_class_correlations.insert(
        ordered_pair(SimmRiskClass::InterestRate, SimmRiskClass::Equity),
        1.5,
    );
    let err = validate_simm_correlations_psd(&params).expect_err("|ρ| > 1 must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("risk_class_correlations"),
        "error should name the offending matrix: {msg}"
    );
}

#[test]
fn parse_credit_sector_number_map_rejects_negative_values() {
    // Use serde_json::json! which permits regular floats; -5.0
    // exercises the validate_non_negative call, not the finiteness
    // check.
    let val = serde_json::json!({
        "sovereign": -5.0,
    });
    let err = parse_credit_sector_number_map(&val, "simm.cq_bucket_weights")
        .expect_err("negative bucket weight must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("simm.cq_bucket_weights"),
        "error should name the offending field: {msg}"
    );
}

#[test]
fn parse_concentration_thresholds_rejects_negative() {
    let val = serde_json::json!({
        "interest_rate": -10.0,
    });
    let err = parse_concentration_thresholds(&val)
        .expect_err("negative concentration threshold must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("concentration_thresholds"),
        "error should name the offending field: {msg}"
    );
}

#[test]
fn parse_ir_tenor_correlations_rejects_out_of_range() {
    let val = serde_json::json!({
        "1y_5y": 1.5,
    });
    let err = parse_ir_tenor_correlations(&val).expect_err("|ρ| > 1 must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("ir_tenor_correlations") && msg.contains("[-1,1]"),
        "error should name the offending field and range: {msg}"
    );
}

#[test]
fn parse_ir_tenor_correlations_rejects_malformed_key() {
    // No underscore separator → parse error mentioning the
    // tenor-label-no-underscore convention.
    let val = serde_json::json!({
        "5Y": 0.5,
    });
    let err = parse_ir_tenor_correlations(&val).expect_err("malformed key must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("expected format 'tenor1_tenor2'"),
        "error should describe the expected format: {msg}"
    );
}

#[test]
fn registry_files_reject_a_foreign_schema_or_version() {
    assert!(check_envelope(None, None, "finstack_quant.margin.ccp.v1").is_ok());
    assert!(
        check_envelope(
            Some("finstack_quant.margin.ccp.v1"),
            Some(1),
            "finstack_quant.margin.ccp.v1"
        )
        .is_ok()
    );
    let schema = check_envelope(
        Some("finstack_quant.margin.simm.v1"),
        None,
        "finstack_quant.margin.ccp.v1",
    )
    .expect_err("a SIMM file must not parse as a CCP file");
    assert!(
        schema
            .to_string()
            .contains("expected \"finstack_quant.margin.ccp.v1\"")
    );
    let version = check_envelope(None, Some(2), "finstack_quant.margin.ccp.v1")
        .expect_err("a version-2 file must be rejected");
    assert!(version.to_string().contains("version 2"));
}

#[test]
fn schedule_im_rejects_implausible_default_maturity() {
    // Synthesise a minimal ScheduleImFile with a bogus
    // default_maturity_years = 1000.0 and verify the parser bails.
    let val = serde_json::json!({
        "schema": "finstack_quant.margin.schedule_im.v1",
        "version": 1,
        "entries": [{
            "ids": ["test"],
            "record": {
                "default_rate": 0.04,
                "default_asset_class": "interest_rate",
                "default_maturity_years": 1000.0,
                "mpor_days": 10,
                "bucket_boundaries_years": {
                    "short_to_medium": 2.0,
                    "medium_to_long": 5.0,
                },
                "rates": [],
            }
        }]
    });
    let err = parse_schedule_im(Some(&val))
        .expect_err("implausible default_maturity_years must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("default_maturity_years"),
        "error should name the offending field: {msg}"
    );
}

#[test]
fn v26_overlay_missing_cq_bucket_weights_is_rejected() {
    // Build a minimal v2.6-tagged record that omits cq_bucket_weights
    // entirely; the parser must reject because v2.6 requires the
    // explicit table and no fabricated fallback is permitted.
    let val = serde_json::json!({
        "schema": "finstack_quant.margin.simm.v1",
        "version": 1,
        "entries": [{
            "ids": ["v2_6"],
            "record": {
                "ir_delta_weights": { "5Y": 50.0 },
                "cnq_delta_weight": 169.0,
                "equity_delta_weight": 25.0,
                "fx_delta_weight": 8.1,
                "fx_intra_bucket_correlation": 0.5,
                "commodity_bucket_weights": {},
                "risk_class_correlations": [],
                "ir_tenor_correlations": {},
                "concentration_thresholds": {},
                "mpor_days": 10,
                "cq_bucket_weights": null
            }
        }]
    });
    let result = parse_simm(Some(&val));
    assert!(
        result.is_err(),
        "v2.6 without cq_bucket_weights must be rejected, got {result:?}"
    );
}
