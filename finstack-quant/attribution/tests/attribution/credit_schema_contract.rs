//! Schema contract tests for credit attribution payloads.

use finstack_quant_attribution::{
    AttributionMethod, AttributionResult, AttributionResultEnvelope, CreditCarryByLevel,
    CreditCarryDecomposition, CreditFactorAttribution, LevelCarry, LevelPnl, PnlAttribution,
};
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use serde_json::Value;
use std::collections::BTreeMap;
use time::macros::date;

fn attribution_result_schema() -> Value {
    let content = include_str!("../../schemas/attribution/1/attribution_result.schema.json");
    serde_json::from_str(content).expect("attribution_result schema must be valid JSON")
}

fn validate(instance: &Value, schema: &Value) -> Result<(), String> {
    let validator =
        jsonschema::validator_for(schema).map_err(|error| format!("invalid schema: {error}"))?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| {
            let path = error.instance_path.to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("{path}: {error}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn canonical_envelope() -> AttributionResultEnvelope {
    let attribution = PnlAttribution::new(
        Money::new(1_000.0, Currency::USD).expect("valid money fixture"),
        "BOND-A",
        date!(2024 - 01 - 01),
        date!(2024 - 01 - 02),
        AttributionMethod::Parallel,
    );
    AttributionResultEnvelope::new(AttributionResult {
        attribution,
        results_meta: ResultsMeta::default(),
    })
}

#[test]
fn attribution_result_with_credit_detail_validates() {
    let mut envelope = canonical_envelope();
    envelope.result.attribution.credit_factor_detail = Some(CreditFactorAttribution {
        model_id: "2024-03-31/abcdef0123456789".to_string(),
        generic_pnl: Money::new(80.0, Currency::USD).expect("valid money fixture"),
        levels: vec![LevelPnl {
            level_name: "rating".to_string(),
            total: Money::new(40.0, Currency::USD).expect("valid money fixture"),
            by_bucket: BTreeMap::from([(
                "IG".to_string(),
                Money::new(40.0, Currency::USD).expect("valid money fixture"),
            )]),
        }],
        adder_pnl_total: Money::new(10.0, Currency::USD).expect("valid money fixture"),
        curve_shape_pnl: Money::new(20.0, Currency::USD).expect("valid money fixture"),
        adder_pnl_by_issuer: None,
        adder_magnitude: Some(Money::new(10.0, Currency::USD).expect("valid money fixture")),
    });
    envelope.result.attribution.credit_carry_decomposition = Some(CreditCarryDecomposition {
        model_id: "2024-03-31/abcdef0123456789".to_string(),
        rates_carry_total: Money::new(30.0, Currency::USD).expect("valid money fixture"),
        credit_carry_total: Money::new(20.0, Currency::USD).expect("valid money fixture"),
        credit_by_level: CreditCarryByLevel {
            generic: Money::new(10.0, Currency::USD).expect("valid money fixture"),
            levels: vec![LevelCarry {
                level_name: "rating".to_string(),
                total: Money::new(8.0, Currency::USD).expect("valid money fixture"),
                by_bucket: BTreeMap::new(),
            }],
            adder_total: Money::new(2.0, Currency::USD).expect("valid money fixture"),
            adder_by_issuer: None,
        },
    });

    let instance = serde_json::to_value(envelope).expect("serialize canonical result");
    validate(&instance, &attribution_result_schema()).unwrap_or_else(|error| {
        panic!("canonical attribution result with credit detail failed validation:\n{error}")
    });
}

#[test]
fn attribution_result_carry_detail_rejects_bare_money_shape() {
    let mut instance =
        serde_json::to_value(canonical_envelope()).expect("serialize canonical result");
    instance["result"]["attribution"]["carry_detail"] = serde_json::json!({
        "total": { "amount": "50.00", "currency": "USD" },
        "coupon_income": { "amount": "30.00", "currency": "USD" },
        "roll_down": { "amount": "20.00", "currency": "USD" }
    });

    validate(&instance, &attribution_result_schema())
        .expect_err("bare Money source lines must fail schema validation");
}
