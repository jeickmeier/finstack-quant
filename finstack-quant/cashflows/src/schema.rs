//! Embedded JSON Schema resources owned by the cashflows crate.

use finstack_quant_core::{Error, Result};
use serde_json::Value;

/// Stable base URI for cashflow component schemas.
pub const CASHFLOW_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/cashflow/1/";

/// Return the canonical schema URI for a cashflow-owned schemars definition.
///
/// # Arguments
///
/// * `name` - Exact generated schemars definition name, such as
///   `"ScheduleParams"`; unsupported names return `None`.
#[must_use]
pub fn definition_uri(name: &str) -> Option<String> {
    let filename = match name {
        "DefaultModelSpec" => "default_model_spec.schema.json",
        "FeeSpec" => "fee_specs.schema.json",
        "FixedCouponSpec" => "coupon_specs.schema.json",
        "PrepaymentModelSpec" => "prepayment_model_spec.schema.json",
        "RecoveryModelSpec" => "recovery_model_spec.schema.json",
        "ScheduleParams" => "schedule_params.schema.json",
        _ => return None,
    };
    Some(format!("{CASHFLOW_SCHEMA_BASE}{filename}"))
}

const SCHEMAS: [(&str, &str); 7] = [
    (
        "amortization_spec.schema.json",
        include_str!("../schemas/cashflow/1/amortization_spec.schema.json"),
    ),
    (
        "coupon_specs.schema.json",
        include_str!("../schemas/cashflow/1/coupon_specs.schema.json"),
    ),
    (
        "default_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/default_model_spec.schema.json"),
    ),
    (
        "fee_specs.schema.json",
        include_str!("../schemas/cashflow/1/fee_specs.schema.json"),
    ),
    (
        "prepayment_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/prepayment_model_spec.schema.json"),
    ),
    (
        "recovery_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/recovery_model_spec.schema.json"),
    ),
    (
        "schedule_params.schema.json",
        include_str!("../schemas/cashflow/1/schedule_params.schema.json"),
    ),
];

/// Parse the embedded schemas once per process.
///
/// Errors are cached as `String` because `finstack_quant_core::Error` is not
/// `Clone`.
fn parsed_schemas() -> &'static std::result::Result<Vec<(String, jsonschema::Resource)>, String> {
    static CACHE: std::sync::OnceLock<
        std::result::Result<Vec<(String, jsonschema::Resource)>, String>,
    > = std::sync::OnceLock::new();

    CACHE.get_or_init(|| {
        SCHEMAS
            .into_iter()
            .map(|(filename, raw)| {
                let schema = serde_json::from_str::<Value>(raw)
                    .map_err(|err| format!("invalid cashflow schema JSON at {filename}: {err}"))?;
                let resource = jsonschema::Resource::from_contents(schema).map_err(|err| {
                    format!("invalid cashflow schema resource at {filename}: {err}")
                })?;
                Ok((format!("{CASHFLOW_SCHEMA_BASE}{filename}"), resource))
            })
            .collect()
    })
}

/// Return the embedded cashflow schemas as JSON-Schema resolver resources.
///
/// # Errors
///
/// Returns a validation error if a checked-in schema is malformed.
pub fn resources() -> Result<Vec<(String, jsonschema::Resource)>> {
    match parsed_schemas() {
        Ok(entries) => Ok(entries.clone()),
        Err(err) => Err(Error::Validation(err.clone())),
    }
}
