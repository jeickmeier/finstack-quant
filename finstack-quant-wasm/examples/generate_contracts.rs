//! Publish validation metadata for the facade's bigint-enabled result serializer.
use finstack_quant_core::schema::{run_schema_generator, SchemaArtifact, SchemaGenerationCommand};
use finstack_quant_core::Result;
use serde_json::Value;
use std::path::Path;

// serde serializes usize/isize through u64/i64. The facade's bigint serializer
// preserves those as bigint; fields with custom serializers describe their actual
// serialized widths in Rust's JsonSchema annotations (not a UI exception list).
fn host_numbers(schema: &mut Value) -> Result<()> {
    if let Some(object) = schema.as_object_mut() {
        if object.get("type").and_then(Value::as_str) == Some("integer") {
            match object.get("format").and_then(Value::as_str) {
                Some("uint") => {
                    object.insert("format".into(), "uint64".into());
                }
                Some("int") => {
                    object.insert("format".into(), "int64".into());
                }
                _ => {}
            }
        }
        for key in ["$defs", "properties", "patternProperties"] {
            if let Some(children) = object.get_mut(key).and_then(Value::as_object_mut) {
                for child in children.values_mut() {
                    host_numbers(child)?;
                }
            }
        }
        for key in ["oneOf", "anyOf", "allOf", "prefixItems"] {
            if let Some(children) = object.get_mut(key).and_then(Value::as_array_mut) {
                for child in children {
                    host_numbers(child)?;
                }
            }
        }
        for key in [
            "items",
            "additionalProperties",
            "not",
            "contains",
            "propertyNames",
        ] {
            if let Some(child) = object.get_mut(key) {
                host_numbers(child)?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let artifact = SchemaArtifact::new::<finstack_quant_valuations::results::ValuationResult>(
        "schemas/host/1/valuation_result.schema.json",
        "https://finstack_quant.dev/schemas/wasm/1/valuation_result.schema.json",
        "ValuationResult",
        "Facade valuation result. int64/uint64 fields are host bigint; JSON export uses integer tokens.",
    ).with_packager(host_numbers);
    run_schema_generator(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new("schemas/host/1"),
        &[artifact],
        &SchemaGenerationCommand::from_env()?,
    )
}
