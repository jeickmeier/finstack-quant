//! Contract tests for checked-in attribution JSON schemas.

#![allow(clippy::expect_used)]

use serde_json::Value;
use std::path::{Path, PathBuf};

const JSON_SCHEMA_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
const SCHEMA_ID_HOST: &str = "https://finstack_quant.dev/";

fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("attribution")
        .join("1")
}

fn read_schema(path: &Path) -> Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&content).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn schema_files() -> Vec<PathBuf> {
    let mut files = std::fs::read_dir(schema_root())
        .expect("read attribution schema directory")
        .map(|entry| {
            entry
                .expect("read attribution schema directory entry")
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn attribution_schemas_use_canonical_id_host() {
    for path in schema_files() {
        let schema = read_schema(&path);
        let Some(id) = schema.get("$id").and_then(Value::as_str) else {
            panic!("{} is missing $id", path.display());
        };
        assert!(
            id.starts_with(SCHEMA_ID_HOST),
            "{} has non-canonical $id host: {id}",
            path.display()
        );
    }
}

#[test]
fn attribution_schemas_declare_2020_12_dialect() {
    for path in schema_files() {
        let schema = read_schema(&path);
        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some(JSON_SCHEMA_2020_12),
            "{} declares the wrong JSON Schema dialect",
            path.display()
        );
    }
}
