//! Schema validation tests for the credit factor hierarchy (PR-9).
//!
//! Four named tests:
//!
//! 1. `credit_factor_model_schema_accepts_golden_artifact` — the PR-5b
//!    golden fixture validates against the new schema.
//! 2. `credit_factor_model_schema_rejects_wrong_schema_version` — wrong
//!    `schema_version` value must fail.
//! 3. `old_attribution_result_schema_payload_still_valid` — a pre-feature
//!    attribution result JSON still validates against the extended schema.
//! 4. `new_attribution_result_schema_accepts_credit_factor_detail` — a
//!    result payload with the new `credit_factor_detail` and
//!    `credit_carry_decomposition` fields validates.

use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load and parse the credit_factor_model schema.
fn credit_factor_model_schema() -> Value {
    let content = include_str!("../../../schemas/factor_model/1/credit_factor_model.schema.json");
    serde_json::from_str(content).expect("credit_factor_model schema must be valid JSON")
}

/// Load and parse the attribution result schema.
fn attribution_result_schema() -> Value {
    let content = include_str!(
        "../../../../attribution/schemas/attribution/1/attribution_result.schema.json"
    );
    serde_json::from_str(content).expect("attribution_result schema must be valid JSON")
}

/// Validate `instance` against `schema`, returning `Ok(())` or a descriptive error.
fn validate(instance: &Value, schema: &Value) -> Result<(), String> {
    let validator =
        jsonschema::validator_for(schema).map_err(|e| format!("Failed to compile schema: {e}"))?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| {
            let path = e.instance_path.to_string();
            if path.is_empty() {
                e.to_string()
            } else {
                format!("{path}: {e}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Validation failed with {} error(s):\n  {}",
            errors.len(),
            errors.join("\n  ")
        ))
    }
}

// ---------------------------------------------------------------------------
// Test 1: golden artifact validates
// ---------------------------------------------------------------------------

/// Load the PR-5b golden artifact and validate it against the new
/// `credit_factor_model.schema.json`.
///
/// This test locks in that the live calibration output produced by PR-5b
/// continues to satisfy the schema introduced in PR-9, preventing silent
/// schema drift.
#[test]
fn credit_factor_model_schema_accepts_golden_artifact() {
    let schema = credit_factor_model_schema();

    // The schema fixture is at tests/schema_fixtures/credit_factor_model_v1.json
    let golden_content = include_str!("../../schema_fixtures/credit_factor_model_v1.json");
    let instance: Value =
        serde_json::from_str(golden_content).expect("golden artifact must be valid JSON");

    validate(&instance, &schema).unwrap_or_else(|e| {
        panic!("Golden artifact failed schema validation:\n{e}");
    });
}

// ---------------------------------------------------------------------------
// Test 2: wrong schema_version is rejected
// ---------------------------------------------------------------------------

/// A modified golden artifact with `schema_version: "wrong/1"` must fail
/// validation. The schema uses `"const": "finstack.credit_factor_model/1"`
/// which enforces the exact version string.
#[test]
fn credit_factor_model_schema_rejects_wrong_schema_version() {
    let schema = credit_factor_model_schema();

    let golden_content = include_str!("../../schema_fixtures/credit_factor_model_v1.json");
    let mut instance: Value =
        serde_json::from_str(golden_content).expect("golden artifact must be valid JSON");

    // Overwrite the schema_version with a bogus value
    instance["schema_version"] = serde_json::Value::String("wrong/1".to_string());

    let result = validate(&instance, &schema);
    assert!(
        result.is_err(),
        "Artifact with wrong schema_version must fail validation, but got Ok"
    );
}

// ---------------------------------------------------------------------------
// Test 3: pre-feature attribution result still validates
// ---------------------------------------------------------------------------

/// An attribution result payload that predates the credit-factor hierarchy
/// feature (no `credit_factor_detail`, no `credit_carry_decomposition`) must
/// still validate against the extended attribution result schema.
///
/// This confirms that the schema additions in PR-9 are strictly additive and
/// do not break old consumers.
#[test]
fn old_attribution_result_schema_payload_still_valid() {
    let schema = attribution_result_schema();

    // Minimal pre-feature payload: required fields only, none of the new
    // optional credit-factor fields.
    let instance = serde_json::json!({
        "schema": "finstack.attribution/1",
        "result": {
            "attribution": {
                "total_pnl": { "amount": "1000.00", "currency": "USD" },
                "carry":           { "amount": "50.00", "currency": "USD" },
                "rates_curves_pnl": { "amount": "200.00", "currency": "USD" },
                "credit_curves_pnl": { "amount": "150.00", "currency": "USD" },
                "inflation_curves_pnl": { "amount": "0.00", "currency": "USD" },
                "correlations_pnl": { "amount": "0.00", "currency": "USD" },
                "fx_pnl":          { "amount": "100.00", "currency": "USD" },
                "vol_pnl":         { "amount": "0.00", "currency": "USD" },
                "model_params_pnl": { "amount": "0.00", "currency": "USD" },
                "market_scalars_pnl": { "amount": "0.00", "currency": "USD" },
                "residual":        { "amount": "500.00", "currency": "USD" },
                "meta": {
                    "method": "Parallel",
                    "t0": "2024-01-01",
                    "t1": "2024-01-02",
                    "instrument_id": "BOND-1",
                    "num_repricings": 0,
                    "tolerance_abs": 1.0,
                    "tolerance_pct": 0.01,
                    "residual_pct": 50.0,
                    "rounding": {}
                }
            },
            "results_meta": {}
        }
    });

    validate(&instance, &schema).unwrap_or_else(|e| {
        panic!("Pre-feature attribution payload failed schema validation:\n{e}");
    });
}

// ---------------------------------------------------------------------------
// Test 4: new payload with credit_factor_detail validates
// ---------------------------------------------------------------------------

/// An attribution result payload that includes the new PR-9 fields
/// (`credit_factor_detail` and `credit_carry_decomposition`) must validate
/// against the extended schema.
///
/// This confirms the schema correctly describes the new fields.
#[test]
fn new_attribution_result_schema_accepts_credit_factor_detail() {
    let schema = attribution_result_schema();

    let instance = serde_json::json!({
        "schema": "finstack.attribution/1",
        "result": {
            "attribution": {
                "total_pnl": { "amount": "1000.00", "currency": "USD" },
                "carry":           { "amount": "50.00", "currency": "USD" },
                "rates_curves_pnl": { "amount": "200.00", "currency": "USD" },
                "credit_curves_pnl": { "amount": "150.00", "currency": "USD" },
                "inflation_curves_pnl": { "amount": "0.00", "currency": "USD" },
                "correlations_pnl": { "amount": "0.00", "currency": "USD" },
                "fx_pnl":          { "amount": "100.00", "currency": "USD" },
                "vol_pnl":         { "amount": "0.00", "currency": "USD" },
                "model_params_pnl": { "amount": "0.00", "currency": "USD" },
                "market_scalars_pnl": { "amount": "0.00", "currency": "USD" },
                "residual":        { "amount": "500.00", "currency": "USD" },
                // New PR-9 field: credit factor hierarchy decomposition
                "credit_factor_detail": {
                    "model_id": "2024-03-31/abcdef0123456789",
                    "generic_pnl": { "amount": "80.00", "currency": "USD" },
                    "levels": [
                        {
                            "level_name": "rating",
                            "total": { "amount": "40.00", "currency": "USD" },
                            "by_bucket": {
                                "IG": { "amount": "40.00", "currency": "USD" }
                            }
                        },
                        {
                            "level_name": "region",
                            "total": { "amount": "20.00", "currency": "USD" }
                        }
                    ],
                    "adder_pnl_total": { "amount": "10.00", "currency": "USD" },
                    "adder_pnl_by_issuer": {
                        "ISSUER-A": { "amount": "10.00", "currency": "USD" }
                    }
                },
                // New PR-9 field: credit carry decomposition
                "credit_carry_decomposition": {
                    "model_id": "2024-03-31/abcdef0123456789",
                    "rates_carry_total": { "amount": "30.00", "currency": "USD" },
                    "credit_carry_total": { "amount": "20.00", "currency": "USD" },
                    "credit_by_level": {
                        "generic": { "amount": "10.00", "currency": "USD" },
                        "levels": [
                            {
                                "level_name": "rating",
                                "total": { "amount": "8.00", "currency": "USD" }
                            }
                        ],
                        "adder_total": { "amount": "2.00", "currency": "USD" }
                    }
                },
                "meta": {
                    "method": "MetricsBased",
                    "t0": "2024-01-01",
                    "t1": "2024-01-02",
                    "instrument_id": "BOND-A",
                    "num_repricings": 0,
                    "tolerance_abs": 1.0,
                    "tolerance_pct": 0.01,
                    "residual_pct": 50.0,
                    "rounding": {}
                }
            },
            "results_meta": {}
        }
    });

    validate(&instance, &schema).unwrap_or_else(|e| {
        panic!("New attribution payload with credit_factor_detail failed schema validation:\n{e}");
    });
}

// ---------------------------------------------------------------------------
// Bonus: carry_detail with legacy Money shape still validates
// ---------------------------------------------------------------------------

/// The extended schema allows `coupon_income` and `roll_down` in `carry_detail`
/// to be either a legacy bare `Money` object (`{amount, currency}`) or the new
/// `SourceLine` shape (`{total, ...}`). Verify the legacy shape is still accepted.
#[test]
fn attribution_result_carry_detail_accepts_legacy_money_shape() {
    let schema = attribution_result_schema();

    let instance = serde_json::json!({
        "schema": "finstack.attribution/1",
        "result": {
            "attribution": {
                "total_pnl": { "amount": "1000.00", "currency": "USD" },
                "carry":           { "amount": "50.00", "currency": "USD" },
                "rates_curves_pnl": { "amount": "200.00", "currency": "USD" },
                "credit_curves_pnl": { "amount": "150.00", "currency": "USD" },
                "inflation_curves_pnl": { "amount": "0.00", "currency": "USD" },
                "correlations_pnl": { "amount": "0.00", "currency": "USD" },
                "fx_pnl":          { "amount": "100.00", "currency": "USD" },
                "vol_pnl":         { "amount": "0.00", "currency": "USD" },
                "model_params_pnl": { "amount": "0.00", "currency": "USD" },
                "market_scalars_pnl": { "amount": "0.00", "currency": "USD" },
                "residual":        { "amount": "500.00", "currency": "USD" },
                // carry_detail with legacy bare-Money shape for coupon_income and roll_down
                "carry_detail": {
                    "total": { "amount": "50.00", "currency": "USD" },
                    "coupon_income": { "amount": "30.00", "currency": "USD" },
                    "roll_down": { "amount": "20.00", "currency": "USD" }
                },
                "meta": {
                    "method": "MetricsBased",
                    "t0": "2024-01-01",
                    "t1": "2024-01-02",
                    "instrument_id": "BOND-A",
                    "num_repricings": 0,
                    "tolerance_abs": 1.0,
                    "tolerance_pct": 0.01,
                    "residual_pct": 5.0,
                    "rounding": {}
                }
            },
            "results_meta": {}
        }
    });

    validate(&instance, &schema).unwrap_or_else(|e| {
        panic!("Legacy Money shape in carry_detail failed schema validation:\n{e}");
    });
}

// ---------------------------------------------------------------------------
// Schema file sanity: all 3 new schema files parse correctly
// ---------------------------------------------------------------------------

#[test]
fn all_new_schema_files_parse_as_valid_json() {
    let _ = credit_factor_model_schema(); // panics on malformed JSON
    let _: Value = serde_json::from_str(include_str!(
        "../../../schemas/factor_model/1/credit_calibration_inputs.schema.json"
    ))
    .expect("credit_calibration_inputs schema must be valid JSON");
    let _: Value = serde_json::from_str(include_str!(
        "../../../schemas/factor_model/1/credit_calibration_config.schema.json"
    ))
    .expect("credit_calibration_config schema must be valid JSON");
}
