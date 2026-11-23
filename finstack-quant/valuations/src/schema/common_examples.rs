//! Canonical JSON example payloads for shared schema components.

use crate::instruments::json_loader::instrument_registry;
use serde_json::Value;

/// Canonical `Attributes` payloads.
pub(super) fn attributes_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"tags": ["rates", "hedge"], "meta": {"desk": "ny_rates"}}),
        serde_json::json!({}),
    ])
}

/// Canonical `BusinessDayConvention` values.
pub(super) fn business_day_convention_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("modified_following".to_string()),
        Value::String("following".to_string()),
    ])
}

/// Canonical ISO 4217 codes.
pub(super) fn currency_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("USD".to_string()),
        Value::String("EUR".to_string()),
        Value::String("JPY".to_string()),
    ])
}

/// Canonical `Date` payloads.
pub(super) fn date_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("2024-01-15".to_string()),
        Value::String("2034-01-15".to_string()),
    ])
}

/// Canonical day-count labels.
pub(super) fn day_count_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("act_360".to_string()),
        Value::String("30_360".to_string()),
        Value::String("act_act_isma".to_string()),
    ])
}

/// Canonical identifier payloads.
///
/// Ids are opaque strings resolved against whatever market or registry the call
/// supplies; the examples show the spellings used throughout the fixtures.
pub(super) fn id_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("USD-OIS".to_string()),
        Value::String("US912828XG33".to_string()),
    ])
}

/// Canonical `Tenor` payloads.
pub(super) fn tenor_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"count": 3, "unit": "months"}),
        serde_json::json!({"count": 10, "unit": "years"}),
    ])
}

/// Canonical `Diagnostic` payloads.
pub(super) fn diagnostic_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![serde_json::json!({
        "code": "contract/version_mismatch",
        "message": "unsupported schema marker",
        "phase": "version",
        "severity": "error",
        "pointer": "/schema"
    })])
}

/// Canonical `ValidationReport` payloads.
pub(super) fn validation_report_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"diagnostics": [], "truncated": false}),
        serde_json::json!({
            "diagnostics": [{
                "code": "contract/unknown_field",
                "message": "unknown field `maturity_date`",
                "phase": "structure",
                "severity": "error",
                "pointer": "/instrument/spec"
            }],
            "truncated": false
        }),
    ])
}

/// Canonical pricing-override payloads.
///
/// All three override maps default to empty and are omitted when empty, so the
/// first example is the one almost every payload uses.
pub(super) fn empty_override_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![serde_json::json!({})])
}

/// Canonical `ScenarioPricingOverrides` payloads.
pub(super) fn scenario_override_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({}),
        serde_json::json!({"scenario_spread_shock_bp": 150.0}),
    ])
}

/// Canonical `ValuationResult`: a priced bond with its policy stamps.
pub(super) fn valuation_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let as_of =
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 15)
            .map_err(|error| {
                finstack_quant_core::Error::Internal(format!("build example as-of date: {error}"))
            })?;
    let value = finstack_quant_core::money::Money::new(
        1_012_345.67,
        finstack_quant_core::currency::Currency::USD,
    )?;
    let meta = finstack_quant_core::config::results_meta(
        &finstack_quant_core::config::FinstackConfig::default(),
    );
    let result =
        crate::results::ValuationResult::stamped_with_meta("US912828XG33", as_of, value, meta);
    let value = serde_json::to_value(&result).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize valuation result example: {error}"))
    })?;
    Ok(vec![value])
}

/// Canonical examples for the instrument umbrella union.
pub(super) fn instrument_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let mut examples = Vec::new();
    for entry in instrument_registry() {
        let entry_examples = entry.examples().map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "build canonical {} instrument example: {error}",
                entry.tag
            ))
        })?;
        examples.extend(entry_examples);
    }
    Ok(examples)
}

/// Canonical `Decimal` payloads.
///
/// Exact decimals are JSON *strings*; emitting a JSON number here is the most
/// common way to fail an otherwise well-formed payload, so the contract is shown
/// rather than only described.
pub(super) fn decimal_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        Value::String("1000000".to_string()),
        Value::String("0.0425".to_string()),
        Value::String("-1234.56789".to_string()),
    ])
}

/// Canonical `Money` payloads, pairing a decimal string with an ISO 4217 code.
pub(super) fn money_examples() -> finstack_quant_core::Result<Vec<Value>> {
    Ok(vec![
        serde_json::json!({"amount": "1000000", "currency": "USD"}),
        serde_json::json!({"amount": "-2500.75", "currency": "EUR"}),
    ])
}
