//! Generates typed JSON Schema property definitions for all instrument types.
//!
//! For each instrument, this binary:
//! 1. Generates its JSON Schema using `schemars::schema_for!()`
//! 2. Reads the corresponding existing schema file
//! 3. Replaces `properties.instrument` with a fully typed version
//!    (discriminator `type` const + generated `spec` schema)
//! 4. Writes back the updated schema file, preserving all other fields

use finstack_valuations::instruments::*;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Locate the schemas directory relative to the crate root.
fn schemas_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    Path::new(&manifest_dir)
        .join("schemas")
        .join("instruments")
        .join("1")
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

/// Read an existing schema file, merge the generated instrument schema, and write back.
fn update_schema_file(name: &str, category: &str, generated_schema: Value) {
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

    // Extract the generated schema's properties and required fields for embedding
    // into the spec sub-schema. Also collect any $defs for inlining.
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
    // Carry forward $defs if present
    if let Some(defs) = generated_schema.get("$defs") {
        spec_schema.insert("$defs".to_string(), defs.clone());
    }

    let title = to_title(name);

    // Build the new properties.instrument value
    let instrument_value = json!({
        "description": format!("The {title} instrument definition"),
        "type": "object",
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
        "$schema",
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

    // Build properties: keep existing non-instrument properties, replace instrument
    let mut properties = Map::new();
    if let Some(existing_props) = existing_obj.get("properties").and_then(|v| v.as_object()) {
        for (k, v) in existing_props {
            if k != "instrument" {
                properties.insert(k.clone(), v.clone());
            }
        }
    }
    properties.insert("instrument".to_string(), instrument_value);
    output.insert("properties".to_string(), Value::Object(properties));

    // Preserve required
    if let Some(req) = existing_obj.get("required") {
        output.insert("required".to_string(), req.clone());
    }

    let json_str = serde_json::to_string_pretty(&Value::Object(output)).expect("serialize output");

    // serde_json default pretty-print uses 2-space indent, which is what we want
    std::fs::write(&path, json_str + "\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));

    println!("  updated {}", path.display());
}

/// Locate the top-level schemas directory (parent of instruments/).
fn all_schemas_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    Path::new(&manifest_dir).join("schemas")
}

/// Update a standalone (non-instrument) schema file, replacing the top-level
/// typed properties with the schemars-generated schema.
fn update_standalone_schema_file(name: &str, subdir: &str, filename: &str, generated: Value) {
    let base = all_schemas_dir();
    let path = base.join(subdir).join(format!("{filename}.schema.json"));

    let existing: Value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    } else {
        // Create minimal placeholder if file doesn't exist
        json!({
            "$id": format!("https://finstack.dev/schemas/{subdir}/{filename}.schema.json"),
            "$schema": "http://json-schema.org/draft-07/schema#",
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
    for key in ["$id", "$schema", "title", "description"] {
        if let Some(val) = existing_obj.get(key) {
            output.insert(key.to_string(), val.clone());
        }
    }

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

/// Generate a standalone schema for a type and update the corresponding file.
macro_rules! gen_standalone_schema {
    ($name:literal, $ty:ty, $subdir:literal, $filename:literal) => {{
        let schema = schemars::schema_for!($ty);
        let schema_value =
            serde_json::to_value(&schema).expect(concat!("serialize schema for ", $name));
        update_standalone_schema_file($name, $subdir, $filename, schema_value);
    }};
}

/// Generate schema for a type and update the corresponding schema file.
macro_rules! gen_schema {
    ($name:literal, $ty:ty, $category:literal) => {{
        let schema = schemars::schema_for!($ty);
        let schema_value =
            serde_json::to_value(&schema).expect(concat!("serialize schema for ", $name));
        update_schema_file($name, $category, schema_value);
    }};
}

fn main() {
    println!("Generating instrument schemas...\n");

    // --- Fixed Income ---
    gen_schema!("bond", Bond, "fixed_income");
    gen_schema!("convertible_bond", ConvertibleBond, "fixed_income");
    gen_schema!("inflation_linked_bond", InflationLinkedBond, "fixed_income");
    gen_schema!("term_loan", TermLoan, "fixed_income");
    gen_schema!("revolving_credit", RevolvingCredit, "fixed_income");
    gen_schema!("bond_future", BondFuture, "fixed_income");
    gen_schema!(
        "agency_mbs_passthrough",
        AgencyMbsPassthrough,
        "fixed_income"
    );
    gen_schema!("agency_tba", AgencyTba, "fixed_income");
    gen_schema!("agency_cmo", AgencyCmo, "fixed_income");
    gen_schema!("dollar_roll", DollarRoll, "fixed_income");
    gen_schema!(
        "trs_fixed_income_index",
        FIIndexTotalReturnSwap,
        "fixed_income"
    );
    gen_schema!("structured_credit", StructuredCredit, "fixed_income");

    // --- Rates ---
    gen_schema!("interest_rate_swap", InterestRateSwap, "rates");
    gen_schema!("basis_swap", BasisSwap, "rates");
    gen_schema!("xccy_swap", XccySwap, "rates");
    gen_schema!("inflation_swap", InflationSwap, "rates");
    gen_schema!("yoy_inflation_swap", YoYInflationSwap, "rates");
    gen_schema!("inflation_cap_floor", InflationCapFloor, "rates");
    gen_schema!("forward_rate_agreement", ForwardRateAgreement, "rates");
    gen_schema!("swaption", Swaption, "rates");
    gen_schema!("bermudan_swaption", BermudanSwaption, "rates");
    gen_schema!("interest_rate_future", InterestRateFuture, "rates");
    gen_schema!("cap_floor", CapFloor, "rates");
    gen_schema!("cms_option", CmsOption, "rates");
    gen_schema!("cms_swap", CmsSwap, "rates");
    gen_schema!("ir_future_option", IrFutureOption, "rates");
    gen_schema!("deposit", Deposit, "rates");
    gen_schema!("repo", Repo, "rates");
    gen_schema!("range_accrual", RangeAccrual, "rates");
    gen_schema!("callable_range_accrual", CallableRangeAccrual, "rates");

    // --- Credit Derivatives ---
    gen_schema!(
        "credit_default_swap",
        CreditDefaultSwap,
        "credit_derivatives"
    );
    gen_schema!("cds_index", CDSIndex, "credit_derivatives");
    gen_schema!("cds_tranche", CDSTranche, "credit_derivatives");
    gen_schema!("cds_option", CDSOption, "credit_derivatives");

    // --- Equity ---
    gen_schema!("equity", Equity, "equity");
    gen_schema!("equity_option", EquityOption, "equity");
    gen_schema!("autocallable", Autocallable, "equity");
    gen_schema!("cliquet_option", CliquetOption, "equity");
    gen_schema!("variance_swap", VarianceSwap, "equity");
    gen_schema!("equity_index_future", EquityIndexFuture, "equity");
    gen_schema!("volatility_index_future", VolatilityIndexFuture, "equity");
    gen_schema!("volatility_index_option", VolatilityIndexOption, "equity");
    gen_schema!("trs_equity", EquityTotalReturnSwap, "equity");
    gen_schema!("private_markets_fund", PrivateMarketsFund, "equity");
    gen_schema!("real_estate_asset", RealEstateAsset, "equity");
    gen_schema!("discounted_cash_flow", DiscountedCashFlow, "equity");
    gen_schema!(
        "levered_real_estate_equity",
        LeveredRealEstateEquity,
        "equity"
    );

    // --- FX ---
    gen_schema!("fx_spot", FxSpot, "fx");
    gen_schema!("fx_swap", FxSwap, "fx");
    gen_schema!("fx_forward", FxForward, "fx");
    gen_schema!("ndf", Ndf, "fx");
    gen_schema!("fx_option", FxOption, "fx");
    gen_schema!("fx_digital_option", FxDigitalOption, "fx");
    gen_schema!("fx_touch_option", FxTouchOption, "fx");
    gen_schema!("fx_barrier_option", FxBarrierOption, "fx");
    gen_schema!("fx_variance_swap", FxVarianceSwap, "fx");
    gen_schema!("quanto_option", QuantoOption, "fx");

    // --- Commodity ---
    gen_schema!("commodity_option", CommodityOption, "commodity");
    gen_schema!("commodity_asian_option", CommodityAsianOption, "commodity");
    gen_schema!("commodity_forward", CommodityForward, "commodity");
    gen_schema!("commodity_swap", CommoditySwap, "commodity");
    gen_schema!("commodity_swaption", CommoditySwaption, "commodity");
    gen_schema!(
        "commodity_spread_option",
        CommoditySpreadOption,
        "commodity"
    );

    // --- Exotics ---
    gen_schema!("asian_option", AsianOption, "exotics");
    gen_schema!("barrier_option", BarrierOption, "exotics");
    gen_schema!("lookback_option", LookbackOption, "exotics");
    gen_schema!("basket", Basket, "exotics");

    println!("\nDone! Updated 67 instrument schema files.");

    // =========================================================================
    // Non-instrument schemas (calibration, attribution, cashflow, margin, results)
    // =========================================================================
    println!("\nGenerating non-instrument schemas...\n");

    // The on-disk v2 schema is frozen for historical/parity tests; the current
    // Rust `CalibrationEnvelope` reflects the v3 shape (flat market_data /
    // prior_market lists, no initial_market), so the generator only targets v3.
    gen_standalone_schema!(
        "calibration",
        finstack_valuations::calibration::api::schema::CalibrationEnvelope,
        "calibration/3",
        "calibration"
    );
    gen_standalone_schema!(
        "valuation_result",
        finstack_valuations::results::ValuationResult,
        "results/1",
        "valuation_result"
    );

    // Cashflow specs — use public re-exports from finstack_cashflows::builder
    gen_standalone_schema!(
        "coupon_specs",
        finstack_cashflows::builder::FixedCouponSpec,
        "cashflow/1",
        "coupon_specs"
    );
    gen_standalone_schema!(
        "amortization_spec",
        finstack_cashflows::builder::AmortizationSpec,
        "cashflow/1",
        "amortization_spec"
    );
    gen_standalone_schema!(
        "schedule_params",
        finstack_cashflows::builder::ScheduleParams,
        "cashflow/1",
        "schedule_params"
    );
    gen_standalone_schema!(
        "fee_specs",
        finstack_cashflows::builder::FeeSpec,
        "cashflow/1",
        "fee_specs"
    );
    gen_standalone_schema!(
        "default_model_spec",
        finstack_cashflows::builder::DefaultModelSpec,
        "cashflow/1",
        "default_model_spec"
    );
    gen_standalone_schema!(
        "prepayment_model_spec",
        finstack_cashflows::builder::PrepaymentModelSpec,
        "cashflow/1",
        "prepayment_model_spec"
    );
    gen_standalone_schema!(
        "recovery_model_spec",
        finstack_cashflows::builder::RecoveryModelSpec,
        "cashflow/1",
        "recovery_model_spec"
    );

    // Market quotes
    gen_standalone_schema!(
        "market_quote",
        finstack_valuations::market::quotes::market_quote::MarketQuote,
        "market/1",
        "market_quote"
    );

    println!("\nDone! Updated all schemas.");
}
