//! Generates typed JSON Schema property definitions for all instrument types.
//!
//! For each instrument, this binary:
//! 1. Generates its JSON Schema using `schemars::schema_for!()`
//! 2. Reads the corresponding existing schema file
//! 3. Replaces `properties.instrument` with a fully typed version
//!    (discriminator `type` const + generated `spec` schema)
//! 4. Writes back the updated schema file, preserving all other fields

use finstack_quant_valuations::instruments::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const JSON_SCHEMA_DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$";
const SCHEMARS_DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE]\d+)?$";
const COMMON_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/common/1/";

#[derive(Clone, Copy)]
struct InstrumentSchemaEntry {
    name: &'static str,
    category: &'static str,
}

/// Locate the schemas directory relative to the crate root.
fn schemas_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    Path::new(&manifest_dir)
        .join("schemas")
        .join("instruments")
        .join("1")
}

/// Locate the top-level schemas directory.
fn all_schemas_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    Path::new(&manifest_dir).join("schemas")
}

/// Locate the shared common-schema directory.
fn common_schemas_dir() -> PathBuf {
    all_schemas_dir().join("common").join("1")
}

/// Convert a snake_case name to a Title Case display name.
fn to_title(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper = first.to_uppercase().to_string();
                    upper + &chars.collect::<String>()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return true for field names that conventionally carry ISO calendar dates.
fn is_date_like_property(name: &str) -> bool {
    name == "date"
        || name.ends_with("_date")
        || name == "maturity"
        || name.ends_with("_maturity")
        || name == "expiry"
        || name.ends_with("_expiry")
}

fn schema_accepts_string(value: &Value) -> bool {
    match value.get("type") {
        Some(Value::String(schema_type)) => schema_type == "string",
        Some(Value::Array(schema_types)) => schema_types.iter().any(|schema_type| {
            schema_type
                .as_str()
                .is_some_and(|schema_type| schema_type == "string")
        }),
        _ => false,
    }
}

fn common_schema_filename(def_name: &str) -> Option<&'static str> {
    match def_name {
        "Attributes" => Some("attributes.schema.json"),
        "BusinessDayConvention" => Some("business_day_convention.schema.json"),
        "Currency" => Some("currency.schema.json"),
        "DayCount" => Some("day_count.schema.json"),
        "Id" => Some("id.schema.json"),
        "Money" => Some("money.schema.json"),
        "PricingOverrides" => Some("pricing_overrides.schema.json"),
        "Tenor" => Some("tenor.schema.json"),
        _ => None,
    }
}

fn common_schema_ref(def_name: &str) -> Option<String> {
    common_schema_filename(def_name).map(|filename| format!("{COMMON_SCHEMA_BASE}{filename}"))
}

fn cashflow_schema_filename(def_name: &str) -> Option<&'static str> {
    match def_name {
        "DefaultModelSpec" => Some("default_model_spec.schema.json"),
        "FeeSpec" => Some("fee_specs.schema.json"),
        "FixedCouponSpec" => Some("coupon_specs.schema.json"),
        "PrepaymentModelSpec" => Some("prepayment_model_spec.schema.json"),
        "RecoveryModelSpec" => Some("recovery_model_spec.schema.json"),
        "ScheduleParams" => Some("schedule_params.schema.json"),
        _ => None,
    }
}

fn cashflow_schema_ref(def_name: &str) -> Option<String> {
    cashflow_schema_filename(def_name)
        .map(|filename| format!("https://finstack_quant.dev/schemas/cashflow/1/{filename}"))
}

fn external_schema_ref(def_name: &str) -> Option<String> {
    common_schema_ref(def_name).or_else(|| cashflow_schema_ref(def_name))
}

fn is_externalized_def(def_name: &str) -> bool {
    common_schema_filename(def_name).is_some() || cashflow_schema_filename(def_name).is_some()
}

fn rewrite_common_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let replacement = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
                .and_then(external_schema_ref);
            if let Some(external_ref) = replacement {
                if let Some(reference) = map.get_mut("$ref") {
                    *reference = Value::String(external_ref);
                }
            }

            for child in map.values_mut() {
                rewrite_common_refs(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_common_refs(child);
            }
        }
        _ => {}
    }
}

fn json_pointer_unescape(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

fn collect_local_def_refs(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(rest) = reference.strip_prefix("#/$defs/") {
                    if let Some(segment) = rest.split('/').next() {
                        out.insert(json_pointer_unescape(segment));
                    }
                }
            }
            for child in map.values() {
                collect_local_def_refs(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_local_def_refs(child, out);
            }
        }
        _ => {}
    }
}

fn prune_unreachable_defs(value: &mut Value) {
    let Some(defs) = value.get("$defs").and_then(Value::as_object) else {
        return;
    };

    let mut root = value.clone();
    if let Some(root_obj) = root.as_object_mut() {
        root_obj.remove("$defs");
    }

    let mut discovered = BTreeSet::new();
    collect_local_def_refs(&root, &mut discovered);

    let mut reachable = BTreeSet::new();
    while let Some(next) = discovered.iter().next().cloned() {
        discovered.remove(&next);
        if !reachable.insert(next.clone()) {
            continue;
        }
        if let Some(definition) = defs.get(&next) {
            collect_local_def_refs(definition, &mut discovered);
        }
    }

    if let Some(defs) = value.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.retain(|def_name, _| reachable.contains(def_name));
        if defs.is_empty() {
            if let Some(obj) = value.as_object_mut() {
                obj.remove("$defs");
            }
        }
    }
}

fn prune_common_defs(value: &mut Value) {
    if let Some(defs) = value.get_mut("$defs").and_then(Value::as_object_mut) {
        defs.retain(|def_name, _| !is_externalized_def(def_name));
        if defs.is_empty() {
            if let Some(obj) = value.as_object_mut() {
                obj.remove("$defs");
            }
        }
    }
}

fn postprocess_schema(value: &mut Value) {
    normalize_decimal_patterns(value);
    annotate_date_formats(value);
    rewrite_common_refs(value);
    prune_common_defs(value);
    prune_unreachable_defs(value);
}

/// Normalize `rust_decimal::Decimal` schemas emitted by schemars.
fn normalize_decimal_patterns(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let accepts_string = match map.get("type") {
                Some(Value::String(schema_type)) => schema_type == "string",
                Some(Value::Array(schema_types)) => schema_types.iter().any(|schema_type| {
                    schema_type
                        .as_str()
                        .is_some_and(|schema_type| schema_type == "string")
                }),
                _ => false,
            };
            if accepts_string
                && map
                    .get("pattern")
                    .and_then(Value::as_str)
                    .is_some_and(|pattern| pattern == SCHEMARS_DECIMAL_PATTERN)
            {
                map.insert(
                    "pattern".to_string(),
                    Value::String(DECIMAL_PATTERN.to_string()),
                );
            }

            for child in map.values_mut() {
                normalize_decimal_patterns(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                normalize_decimal_patterns(child);
            }
        }
        _ => {}
    }
}

/// Add `format: "date"` to generated date-like string properties.
fn annotate_date_formats(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(properties) = map.get_mut("properties").and_then(Value::as_object_mut) {
                for (property, schema) in properties {
                    if is_date_like_property(property) && schema_accepts_string(schema) {
                        if let Some(schema_obj) = schema.as_object_mut() {
                            schema_obj
                                .entry("format".to_string())
                                .or_insert_with(|| Value::String("date".to_string()));
                        }
                    }
                    annotate_date_formats(schema);
                }
            }

            for (key, child) in map {
                if key != "properties" {
                    annotate_date_formats(child);
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                annotate_date_formats(child);
            }
        }
        _ => {}
    }
}

/// Read an existing schema file, merge the generated instrument schema, and write back.
fn update_schema_file(name: &str, category: &str, mut generated_schema: Value) {
    let base = schemas_dir();
    let path = base.join(category).join(format!("{name}.schema.json"));

    // Read existing file
    let existing: Value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    } else {
        panic!(
            "Schema file does not exist: {}. All schema files should already exist.",
            path.display()
        );
    };

    let existing_obj = existing
        .as_object()
        .expect("existing schema must be an object");
    postprocess_schema(&mut generated_schema);

    // Extract the generated schema's properties and required fields for embedding
    // into the spec sub-schema. Generated refs are document-root pointers
    // (`#/$defs/...`), so `$defs` must stay at the top level of the schema.
    let mut spec_schema = Map::new();

    if let Some(props) = generated_schema.get("properties") {
        spec_schema.insert("properties".to_string(), props.clone());
    }
    if let Some(req) = generated_schema.get("required") {
        spec_schema.insert("required".to_string(), req.clone());
    }
    if let Some(t) = generated_schema.get("type") {
        spec_schema.insert("type".to_string(), t.clone());
    }
    if let Some(additional) = generated_schema.get("additionalProperties") {
        spec_schema.insert("additionalProperties".to_string(), additional.clone());
    }
    let title = to_title(name);

    // Build the new properties.instrument value
    let instrument_value = json!({
        "description": format!("The {title} instrument definition"),
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "type": {
                "const": name,
                "type": "string"
            },
            "spec": Value::Object(spec_schema)
        },
        "required": ["type", "spec"]
    });

    // Build the output, preserving order of existing keys
    let mut output = Map::new();

    // Preserve known top-level keys from the existing file
    let preserve_keys = [
        "$id",
        "additionalProperties",
        "description",
        "examples",
        "title",
        "type",
    ];

    for key in &preserve_keys {
        if let Some(val) = existing_obj.get(*key) {
            output.insert((*key).to_string(), val.clone());
        }
    }
    output.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DIALECT.to_string()),
    );
    output
        .entry("additionalProperties".to_string())
        .or_insert(Value::Bool(false));

    // Carry forward generated `$defs` at document root to match schemars refs.
    if let Some(defs) = generated_schema.get("$defs") {
        output.insert("$defs".to_string(), defs.clone());
    }

    // Build properties: keep existing non-instrument properties, replace instrument
    let mut properties = Map::new();
    if let Some(existing_props) = existing_obj.get("properties").and_then(|v| v.as_object()) {
        for (k, v) in existing_props {
            if k != "instrument" {
                properties.insert(k.clone(), v.clone());
            }
        }
    }
    properties.insert(
        "schema".to_string(),
        json!({
            "const": "finstack_quant.instrument/1",
            "description": "Schema version identifier",
            "type": "string"
        }),
    );
    properties.insert("instrument".to_string(), instrument_value);
    output.insert("properties".to_string(), Value::Object(properties));

    output.insert("required".to_string(), json!(["schema", "instrument"]));

    let json_str = serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output");

    // serde_json default pretty-print uses 2-space indent, which is what we want
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));

    println!("  updated {}", path.display());
}

fn update_instrument_union_schema_file(entries: &[InstrumentSchemaEntry]) {
    let path = schemas_dir().join("instrument.schema.json");
    let existing: Value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    } else {
        json!({
            "$id": "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json",
            "title": "Finstack Quant Instrument",
            "description": "Tagged union of all supported financial instruments"
        })
    };
    let existing_obj = existing
        .as_object()
        .expect("instrument union schema must be an object");

    let mut output = Map::new();
    for key in ["$id", "description", "title"] {
        if let Some(value) = existing_obj.get(key) {
            output.insert(key.to_string(), value.clone());
        }
    }
    output.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DIALECT.to_string()),
    );
    let mut entries = entries.to_vec();
    entries.sort_by_key(|entry| entry.name);
    output.insert(
        "oneOf".to_string(),
        Value::Array(
            entries
                .iter()
                .map(|entry| {
                    json!({
                        "$ref": format!(
                            "https://finstack_quant.dev/schemas/instrument/1/{}/{}.schema.json",
                            entry.category, entry.name
                        )
                    })
                })
                .collect(),
        ),
    );

    let json_str = serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output");
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("  updated {}", path.display());
}

/// Update a standalone (non-instrument) schema file, replacing the top-level
/// typed properties with the schemars-generated schema.
fn update_standalone_schema_file(name: &str, subdir: &str, filename: &str, generated: Value) {
    let base = all_schemas_dir();
    let path = base.join(subdir).join(format!("{filename}.schema.json"));
    let mut generated = generated;
    postprocess_schema(&mut generated);

    let existing: Value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    } else {
        // Create minimal placeholder if file doesn't exist
        json!({
            "$id": format!("https://finstack_quant.dev/schemas/{subdir}/{filename}.schema.json"),
            "$schema": JSON_SCHEMA_DIALECT,
            "title": to_title(name),
            "description": format!("{} specification", to_title(name)),
            "type": "object"
        })
    };

    let existing_obj = existing
        .as_object()
        .expect("existing schema must be an object");

    let mut output = Map::new();

    // Preserve metadata from existing file
    for key in ["$id", "title", "description"] {
        if let Some(val) = existing_obj.get(key) {
            output.insert(key.to_string(), val.clone());
        }
    }
    output.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DIALECT.to_string()),
    );

    // Preserve examples if present
    if let Some(examples) = existing_obj.get("examples") {
        output.insert("examples".to_string(), examples.clone());
    }

    // Insert generated schema properties
    if let Some(t) = generated.get("type") {
        output.insert("type".to_string(), t.clone());
    }
    if let Some(props) = generated.get("properties") {
        output.insert("properties".to_string(), props.clone());
    }
    if let Some(req) = generated.get("required") {
        output.insert("required".to_string(), req.clone());
    }
    if let Some(defs) = generated.get("$defs") {
        output.insert("$defs".to_string(), defs.clone());
    }
    if let Some(additional) = generated.get("additionalProperties") {
        output.insert("additionalProperties".to_string(), additional.clone());
    }
    // For enums (oneOf, anyOf)
    if let Some(one_of) = generated.get("oneOf") {
        output.insert("oneOf".to_string(), one_of.clone());
    }
    if let Some(any_of) = generated.get("anyOf") {
        output.insert("anyOf".to_string(), any_of.clone());
    }

    let json_str = serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output");
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));

    println!("  updated {}", path.display());
}

/// Update a shared common schema file from a schemars-generated type schema.
fn update_common_schema_file(title: &str, description: &str, filename: &str, generated: Value) {
    let dir = common_schemas_dir();
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let path = dir.join(filename);
    let mut schema = generated;
    normalize_decimal_patterns(&mut schema);
    annotate_date_formats(&mut schema);
    rewrite_common_refs(&mut schema);
    prune_common_defs(&mut schema);

    let mut output = Map::new();
    output.insert(
        "$id".to_string(),
        Value::String(format!("{COMMON_SCHEMA_BASE}{filename}")),
    );
    output.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DIALECT.to_string()),
    );
    output.insert("title".to_string(), Value::String(title.to_string()));
    output.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );

    if let Some(obj) = schema.as_object() {
        for key in [
            "type",
            "format",
            "pattern",
            "additionalProperties",
            "properties",
            "required",
            "oneOf",
            "anyOf",
            "enum",
            "$defs",
        ] {
            if let Some(value) = obj.get(key) {
                output.insert(key.to_string(), value.clone());
            }
        }
    }

    let json_str = serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output");
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("  updated {}", path.display());
}

fn update_manual_common_schema_file(filename: &str, schema: Value) {
    let dir = common_schemas_dir();
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let path = dir.join(filename);
    let json_str = serde_json::to_string_pretty(&schema).expect("serialize output");
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("  updated {}", path.display());
}

/// Generate a standalone schema for a type and update the corresponding file.
macro_rules! gen_standalone_schema {
    ($name:literal, $ty:ty, $subdir:literal, $filename:literal) => {{
        let schema = schemars::schema_for!($ty);
        let schema_value =
            serde_json::to_value(&schema).expect(concat!("serialize schema for ", $name));
        update_standalone_schema_file($name, $subdir, $filename, schema_value);
    }};
}

/// Generate a common schema for a canonical shared type.
macro_rules! gen_common_schema {
    ($title:literal, $description:literal, $ty:ty, $filename:literal) => {{
        let schema = schemars::schema_for!($ty);
        let schema_value =
            serde_json::to_value(&schema).expect(concat!("serialize schema for ", $title));
        update_common_schema_file($title, $description, $filename, schema_value);
    }};
}

/// Generate schema for a type and update the corresponding schema file.
macro_rules! gen_schema {
    ($entries:ident, $name:literal, $ty:ty, $category:literal) => {{
        let schema = schemars::schema_for!($ty);
        let schema_value =
            serde_json::to_value(&schema).expect(concat!("serialize schema for ", $name));
        update_schema_file($name, $category, schema_value);
        $entries.push(InstrumentSchemaEntry {
            name: $name,
            category: $category,
        });
    }};
}

fn main() {
    println!("Generating common schemas...\n");
    gen_common_schema!(
        "Attributes",
        "User-defined tags and key-value metadata for classification.",
        finstack_quant_core::types::Attributes,
        "attributes.schema.json"
    );
    gen_common_schema!(
        "Business Day Convention",
        "Business day adjustment convention.",
        finstack_quant_core::dates::BusinessDayConvention,
        "business_day_convention.schema.json"
    );
    gen_common_schema!(
        "Currency",
        "ISO 4217 currency code.",
        finstack_quant_core::currency::Currency,
        "currency.schema.json"
    );
    gen_common_schema!(
        "Day Count",
        "Day-count convention.",
        finstack_quant_core::dates::DayCount,
        "day_count.schema.json"
    );
    gen_common_schema!(
        "Money",
        "Currency-tagged monetary amount.",
        finstack_quant_core::money::Money,
        "money.schema.json"
    );
    gen_common_schema!(
        "Pricing Overrides",
        "Per-instrument pricing and sensitivity override knobs.",
        finstack_quant_valuations::instruments::PricingOverrides,
        "pricing_overrides.schema.json"
    );
    gen_common_schema!(
        "Tenor",
        "A parsed financial tenor.",
        finstack_quant_core::dates::Tenor,
        "tenor.schema.json"
    );
    update_common_schema_file(
        "Id",
        "Opaque string identifier.",
        "id.schema.json",
        json!({ "type": "string" }),
    );
    update_manual_common_schema_file(
        "date.schema.json",
        json!({
            "$id": format!("{COMMON_SCHEMA_BASE}date.schema.json"),
            "$schema": JSON_SCHEMA_DIALECT,
            "title": "Date",
            "description": "ISO 8601 calendar date string.",
            "type": "string",
            "format": "date"
        }),
    );
    update_manual_common_schema_file(
        "decimal.schema.json",
        json!({
            "$id": format!("{COMMON_SCHEMA_BASE}decimal.schema.json"),
            "$schema": JSON_SCHEMA_DIALECT,
            "title": "Decimal",
            "description": "Decimal number encoded as a JSON string.",
            "type": "string",
            "pattern": DECIMAL_PATTERN
        }),
    );
    println!("\nDone! Updated common schema files.");

    println!("Generating instrument schemas...\n");
    let mut instrument_entries = Vec::new();

    // --- Fixed Income ---
    gen_schema!(instrument_entries, "bond", Bond, "fixed_income");
    gen_schema!(
        instrument_entries,
        "convertible_bond",
        ConvertibleBond,
        "fixed_income"
    );
    gen_schema!(
        instrument_entries,
        "inflation_linked_bond",
        InflationLinkedBond,
        "fixed_income"
    );
    gen_schema!(instrument_entries, "term_loan", TermLoan, "fixed_income");
    gen_schema!(
        instrument_entries,
        "revolving_credit",
        RevolvingCredit,
        "fixed_income"
    );
    gen_schema!(
        instrument_entries,
        "bond_future",
        BondFuture,
        "fixed_income"
    );
    gen_schema!(
        instrument_entries,
        "agency_mbs_passthrough",
        AgencyMbsPassthrough,
        "fixed_income"
    );
    gen_schema!(instrument_entries, "agency_tba", AgencyTba, "fixed_income");
    gen_schema!(instrument_entries, "agency_cmo", AgencyCmo, "fixed_income");
    gen_schema!(
        instrument_entries,
        "dollar_roll",
        DollarRoll,
        "fixed_income"
    );
    gen_schema!(
        instrument_entries,
        "trs_fixed_income_index",
        FIIndexTotalReturnSwap,
        "fixed_income"
    );
    gen_schema!(
        instrument_entries,
        "structured_credit",
        StructuredCredit,
        "fixed_income"
    );

    // --- Rates ---
    gen_schema!(
        instrument_entries,
        "interest_rate_swap",
        InterestRateSwap,
        "rates"
    );
    gen_schema!(instrument_entries, "basis_swap", BasisSwap, "rates");
    gen_schema!(instrument_entries, "xccy_swap", XccySwap, "rates");
    gen_schema!(instrument_entries, "inflation_swap", InflationSwap, "rates");
    gen_schema!(
        instrument_entries,
        "yoy_inflation_swap",
        YoYInflationSwap,
        "rates"
    );
    gen_schema!(
        instrument_entries,
        "inflation_cap_floor",
        InflationCapFloor,
        "rates"
    );
    gen_schema!(
        instrument_entries,
        "forward_rate_agreement",
        ForwardRateAgreement,
        "rates"
    );
    gen_schema!(instrument_entries, "swaption", Swaption, "rates");
    gen_schema!(
        instrument_entries,
        "bermudan_swaption",
        BermudanSwaption,
        "rates"
    );
    gen_schema!(
        instrument_entries,
        "interest_rate_future",
        InterestRateFuture,
        "rates"
    );
    gen_schema!(instrument_entries, "cap_floor", CapFloor, "rates");
    gen_schema!(instrument_entries, "cms_option", CmsOption, "rates");
    gen_schema!(
        instrument_entries,
        "cms_spread_option",
        CmsSpreadOption,
        "rates"
    );
    gen_schema!(instrument_entries, "cms_swap", CmsSwap, "rates");
    gen_schema!(
        instrument_entries,
        "ir_future_option",
        IrFutureOption,
        "rates"
    );
    gen_schema!(instrument_entries, "deposit", Deposit, "rates");
    gen_schema!(instrument_entries, "repo", Repo, "rates");
    gen_schema!(instrument_entries, "range_accrual", RangeAccrual, "rates");
    gen_schema!(
        instrument_entries,
        "callable_range_accrual",
        CallableRangeAccrual,
        "rates"
    );
    gen_schema!(instrument_entries, "snowball", Snowball, "rates");
    gen_schema!(instrument_entries, "tarn", Tarn, "rates");

    // --- Credit Derivatives ---
    gen_schema!(
        instrument_entries,
        "credit_default_swap",
        CreditDefaultSwap,
        "credit_derivatives"
    );
    gen_schema!(
        instrument_entries,
        "cds_index",
        CDSIndex,
        "credit_derivatives"
    );
    gen_schema!(
        instrument_entries,
        "cds_tranche",
        CDSTranche,
        "credit_derivatives"
    );
    gen_schema!(
        instrument_entries,
        "cds_option",
        CDSOption,
        "credit_derivatives"
    );

    // --- Equity ---
    gen_schema!(instrument_entries, "equity", Equity, "equity");
    gen_schema!(instrument_entries, "equity_option", EquityOption, "equity");
    gen_schema!(instrument_entries, "autocallable", Autocallable, "equity");
    gen_schema!(
        instrument_entries,
        "cliquet_option",
        CliquetOption,
        "equity"
    );
    gen_schema!(instrument_entries, "variance_swap", VarianceSwap, "equity");
    gen_schema!(
        instrument_entries,
        "equity_index_future",
        EquityIndexFuture,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "volatility_index_future",
        VolatilityIndexFuture,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "volatility_index_option",
        VolatilityIndexOption,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "trs_equity",
        EquityTotalReturnSwap,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "private_markets_fund",
        PrivateMarketsFund,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "real_estate_asset",
        RealEstateAsset,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "discounted_cash_flow",
        DiscountedCashFlow,
        "equity"
    );
    gen_schema!(
        instrument_entries,
        "levered_real_estate_equity",
        LeveredRealEstateEquity,
        "equity"
    );

    // --- FX ---
    gen_schema!(instrument_entries, "fx_spot", FxSpot, "fx");
    gen_schema!(instrument_entries, "fx_swap", FxSwap, "fx");
    gen_schema!(instrument_entries, "fx_forward", FxForward, "fx");
    gen_schema!(instrument_entries, "ndf", Ndf, "fx");
    gen_schema!(instrument_entries, "fx_option", FxOption, "fx");
    gen_schema!(
        instrument_entries,
        "fx_digital_option",
        FxDigitalOption,
        "fx"
    );
    gen_schema!(instrument_entries, "fx_touch_option", FxTouchOption, "fx");
    gen_schema!(
        instrument_entries,
        "fx_barrier_option",
        FxBarrierOption,
        "fx"
    );
    gen_schema!(instrument_entries, "fx_variance_swap", FxVarianceSwap, "fx");
    gen_schema!(instrument_entries, "quanto_option", QuantoOption, "fx");

    // --- Commodity ---
    gen_schema!(
        instrument_entries,
        "commodity_option",
        CommodityOption,
        "commodity"
    );
    gen_schema!(
        instrument_entries,
        "commodity_asian_option",
        CommodityAsianOption,
        "commodity"
    );
    gen_schema!(
        instrument_entries,
        "commodity_forward",
        CommodityForward,
        "commodity"
    );
    gen_schema!(
        instrument_entries,
        "commodity_swap",
        CommoditySwap,
        "commodity"
    );
    gen_schema!(
        instrument_entries,
        "commodity_swaption",
        CommoditySwaption,
        "commodity"
    );
    gen_schema!(
        instrument_entries,
        "commodity_spread_option",
        CommoditySpreadOption,
        "commodity"
    );

    // --- Exotics ---
    gen_schema!(instrument_entries, "asian_option", AsianOption, "exotics");
    gen_schema!(
        instrument_entries,
        "barrier_option",
        BarrierOption,
        "exotics"
    );
    gen_schema!(
        instrument_entries,
        "lookback_option",
        LookbackOption,
        "exotics"
    );
    gen_schema!(instrument_entries, "basket", Basket, "exotics");

    update_instrument_union_schema_file(&instrument_entries);

    println!("\nDone! Updated 70 instrument schema files.");

    // =========================================================================
    // Non-instrument schemas (calibration, attribution, cashflow, margin, results)
    // =========================================================================
    println!("\nGenerating non-instrument schemas...\n");

    // The on-disk v2 schema is frozen for historical/parity tests; the current
    // Rust `CalibrationEnvelope` reflects the v3 shape (flat market_data /
    // prior_market lists, no initial_market), so the generator only targets v3.
    gen_standalone_schema!(
        "calibration",
        finstack_quant_valuations::calibration::api::schema::CalibrationEnvelope,
        "calibration/3",
        "calibration"
    );
    gen_standalone_schema!(
        "valuation_result",
        finstack_quant_valuations::results::ValuationResult,
        "results/1",
        "valuation_result"
    );

    // Cashflow specs — use public re-exports from finstack_quant_cashflows::builder
    gen_standalone_schema!(
        "coupon_specs",
        finstack_quant_cashflows::builder::FixedCouponSpec,
        "cashflow/1",
        "coupon_specs"
    );
    gen_standalone_schema!(
        "amortization_spec",
        finstack_quant_cashflows::builder::AmortizationSpec,
        "cashflow/1",
        "amortization_spec"
    );
    gen_standalone_schema!(
        "schedule_params",
        finstack_quant_cashflows::builder::ScheduleParams,
        "cashflow/1",
        "schedule_params"
    );
    gen_standalone_schema!(
        "fee_specs",
        finstack_quant_cashflows::builder::FeeSpec,
        "cashflow/1",
        "fee_specs"
    );
    gen_standalone_schema!(
        "default_model_spec",
        finstack_quant_cashflows::builder::DefaultModelSpec,
        "cashflow/1",
        "default_model_spec"
    );
    gen_standalone_schema!(
        "prepayment_model_spec",
        finstack_quant_cashflows::builder::PrepaymentModelSpec,
        "cashflow/1",
        "prepayment_model_spec"
    );
    gen_standalone_schema!(
        "recovery_model_spec",
        finstack_quant_cashflows::builder::RecoveryModelSpec,
        "cashflow/1",
        "recovery_model_spec"
    );

    // Market quotes
    gen_standalone_schema!(
        "market_quote",
        finstack_quant_valuations::market::quotes::market_quote::MarketQuote,
        "market/1",
        "market_quote"
    );

    println!("\nDone! Updated all schemas.");
}
