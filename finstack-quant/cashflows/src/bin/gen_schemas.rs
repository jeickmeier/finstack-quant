//! Generate the JSON Schemas owned by `finstack-quant-cashflows`.

use finstack_quant_cashflows::builder::{
    AmortizationSpec, DefaultModelSpec, FeeSpec, FixedCouponSpec, PrepaymentModelSpec,
    RecoveryModelSpec, ScheduleParams,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const COMMON_BASE: &str = "https://finstack_quant.dev/schemas/common/1/";
const CASHFLOW_BASE: &str = "https://finstack_quant.dev/schemas/cashflow/1/";
const DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$";
const SCHEMARS_DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE]\d+)?$";

fn schema_dir() -> PathBuf {
    Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("schemas/cashflow/1")
}

fn external_ref(name: &str) -> Option<String> {
    let common = match name {
        "Attributes" => "attributes.schema.json",
        "BusinessDayConvention" => "business_day_convention.schema.json",
        "Currency" => "currency.schema.json",
        "DayCount" => "day_count.schema.json",
        "Id" => "id.schema.json",
        "Money" => "money.schema.json",
        "PricingOverrides" => "pricing_overrides.schema.json",
        "Tenor" => "tenor.schema.json",
        _ => "",
    };
    if !common.is_empty() {
        return Some(format!("{COMMON_BASE}{common}"));
    }
    let cashflow = match name {
        "DefaultModelSpec" => "default_model_spec.schema.json",
        "FeeSpec" => "fee_specs.schema.json",
        "FixedCouponSpec" => "coupon_specs.schema.json",
        "PrepaymentModelSpec" => "prepayment_model_spec.schema.json",
        "RecoveryModelSpec" => "recovery_model_spec.schema.json",
        "ScheduleParams" => "schedule_params.schema.json",
        _ => return None,
    };
    Some(format!("{CASHFLOW_BASE}{cashflow}"))
}

fn is_date_like(name: &str) -> bool {
    name == "date"
        || name.ends_with("_date")
        || name == "maturity"
        || name.ends_with("_maturity")
        || name == "expiry"
        || name.ends_with("_expiry")
}

fn walk(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(properties) = map.get_mut("properties").and_then(Value::as_object_mut) {
                for (name, schema) in properties {
                    if is_date_like(name)
                        && schema.get("type") == Some(&Value::String("string".into()))
                    {
                        schema
                            .as_object_mut()
                            .expect("property schema object")
                            .entry("format")
                            .or_insert_with(|| Value::String("date".into()));
                    }
                }
            }
            if map.get("type").is_some_and(|kind| {
                kind == "string"
                    || kind
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "string"))
            }) && map.get("pattern").and_then(Value::as_str) == Some(SCHEMARS_DECIMAL_PATTERN)
            {
                map.insert("pattern".into(), Value::String(DECIMAL_PATTERN.into()));
            }
            let replacement = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
                .and_then(external_ref);
            if let Some(reference) = replacement {
                map.insert("$ref".into(), Value::String(reference));
            }
            for child in map.values_mut() {
                walk(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(walk),
        _ => {}
    }
}

fn collect_refs(value: &Value, refs: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
                .and_then(|rest| rest.split('/').next())
            {
                refs.insert(name.replace("~1", "/").replace("~0", "~"));
            }
            map.values().for_each(|child| collect_refs(child, refs));
        }
        Value::Array(items) => items.iter().for_each(|child| collect_refs(child, refs)),
        _ => {}
    }
}

fn prune_defs(value: &mut Value) {
    let Some(defs) = value.get("$defs").and_then(Value::as_object).cloned() else {
        return;
    };
    let mut root = value.clone();
    root.as_object_mut().expect("schema object").remove("$defs");
    let mut pending = BTreeSet::new();
    collect_refs(&root, &mut pending);
    let mut reachable = BTreeSet::new();
    while let Some(name) = pending.pop_first() {
        if reachable.insert(name.clone()) {
            if let Some(definition) = defs.get(&name) {
                collect_refs(definition, &mut pending);
            }
        }
    }
    if let Some(current) = value.get_mut("$defs").and_then(Value::as_object_mut) {
        current.retain(|name, _| external_ref(name).is_none() && reachable.contains(name));
    }
    if value
        .get("$defs")
        .and_then(Value::as_object)
        .is_some_and(Map::is_empty)
    {
        value
            .as_object_mut()
            .expect("schema object")
            .remove("$defs");
    }
}

fn write_schema<T: schemars::JsonSchema>(name: &str, filename: &str) {
    let path = schema_dir().join(format!("{filename}.schema.json"));
    let existing: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| {
            json!({
                "$id": format!("{CASHFLOW_BASE}{filename}.schema.json"),
                "title": name,
                "description": format!("{name} specification")
            })
        });
    let mut generated = serde_json::to_value(schemars::schema_for!(T)).expect("serialize schema");
    walk(&mut generated);
    prune_defs(&mut generated);
    let mut output = Map::new();
    for key in ["$id", "title", "description", "examples"] {
        if let Some(value) = existing.get(key) {
            output.insert(key.into(), value.clone());
        }
    }
    output.insert("$schema".into(), Value::String(DIALECT.into()));
    for key in [
        "type",
        "properties",
        "required",
        "$defs",
        "additionalProperties",
        "oneOf",
        "anyOf",
    ] {
        if let Some(value) = generated.get(key) {
            output.insert(key.into(), value.clone());
        }
    }
    std::fs::create_dir_all(schema_dir()).expect("create schema directory");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output") + "\n",
    )
    .unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
    println!("updated {}", path.display());
}

fn main() {
    write_schema::<FixedCouponSpec>("coupon_specs", "coupon_specs");
    write_schema::<AmortizationSpec>("amortization_spec", "amortization_spec");
    write_schema::<ScheduleParams>("schedule_params", "schedule_params");
    write_schema::<FeeSpec>("fee_specs", "fee_specs");
    write_schema::<DefaultModelSpec>("default_model_spec", "default_model_spec");
    write_schema::<PrepaymentModelSpec>("prepayment_model_spec", "prepayment_model_spec");
    write_schema::<RecoveryModelSpec>("recovery_model_spec", "recovery_model_spec");
}
