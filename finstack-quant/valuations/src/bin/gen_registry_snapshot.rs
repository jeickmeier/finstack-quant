//! Regenerate or verify the pinned standard pricer and metric registry snapshot.
//!
//! `tests/data/registry_snapshot.json` lists, per instrument type, the
//! registered model keys and the applicable metric identifiers, plus the
//! universal metric list. It guards the registry refactors of the valuations
//! crate split: assembling the registries from per-family tables must leave
//! both maps byte-identical.
//!
//! `--write` regenerates the file (`mise run gen-write`) after an intentional
//! registration change; `--check` fails, naming the instrument types that
//! moved, when the file is stale (the `registry_snapshot` integration test).

use finstack_quant_valuations::pricer::STANDARD;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

const SNAPSHOT_PATH: &str = "tests/data/registry_snapshot.json";

fn current_snapshot() -> Result<String, String> {
    let pricers = STANDARD.pricers();
    let metrics = STANDARD.metrics();
    let mut by_type: BTreeMap<String, Value> = BTreeMap::new();
    for (instrument_type, models) in pricers.all_models_grouped() {
        let models: Vec<String> = models.iter().map(ToString::to_string).collect();
        let applicable: Vec<String> = metrics
            .metrics_for_instrument(instrument_type)
            .iter()
            .map(ToString::to_string)
            .collect();
        by_type.insert(
            instrument_type.to_string(),
            serde_json::json!({ "models": models, "metrics": applicable }),
        );
    }
    let universal: Vec<String> = metrics
        .available_metrics()
        .iter()
        .map(ToString::to_string)
        .collect();
    let document = serde_json::json!({
        "instrument_types": by_type,
        "available_metrics": universal,
    });
    let mut text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("serialize registry snapshot: {error}"))?;
    text.push('\n');
    Ok(text)
}

/// Every instrument type entry plus the universal metric list, by key.
fn entries(text: &str) -> Result<BTreeMap<String, Value>, String> {
    let document: BTreeMap<String, Value> = serde_json::from_str(text)
        .map_err(|error| format!("{SNAPSHOT_PATH} is not a JSON object: {error}"))?;
    let mut out = BTreeMap::new();
    if let Some(Value::Object(types)) = document.get("instrument_types") {
        out.extend(
            types
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
    out.insert(
        "available_metrics".to_string(),
        document
            .get("available_metrics")
            .cloned()
            .unwrap_or_default(),
    );
    Ok(out)
}

/// Compare per key first, so a mismatch names the instrument types (or the
/// universal metric list) that moved; then the bytes, which pin formatting.
fn check(current: &str, pinned: &str) -> Result<(), String> {
    let (current_entries, pinned_entries) = (entries(current)?, entries(pinned)?);
    let moved: Vec<&String> = current_entries
        .keys()
        .chain(
            pinned_entries
                .keys()
                .filter(|key| !current_entries.contains_key(*key)),
        )
        .filter(|key| current_entries.get(*key) != pinned_entries.get(*key))
        .collect();
    if !moved.is_empty() {
        let moved: Vec<&str> = moved.iter().map(|key| key.as_str()).collect();
        return Err(format!(
            "registry entries differ from {SNAPSHOT_PATH}: {}; if the change is intentional, \
             run `mise run gen-write` and review the diff",
            moved.join(", ")
        ));
    }
    if current != pinned {
        return Err(format!(
            "registry snapshot formatting differs from {SNAPSHOT_PATH}; run `mise run gen-write`"
        ));
    }
    Ok(())
}

fn run(mode: &str) -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_PATH);
    let current = current_snapshot()?;
    let pinned = std::fs::read_to_string(&path);
    match mode {
        "--check" => check(
            &current,
            &pinned.map_err(|error| format!("read {}: {error}", path.display()))?,
        ),
        "--write" => {
            if pinned.ok().as_deref() != Some(current.as_str()) {
                std::fs::write(&path, &current)
                    .map_err(|error| format!("write {}: {error}", path.display()))?;
                println!("updated {}", path.display());
            }
            Ok(())
        }
        _ => Err("usage: gen_registry_snapshot --write | --check".to_string()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [mode] => run(mode),
        _ => Err("usage: gen_registry_snapshot --write | --check".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
