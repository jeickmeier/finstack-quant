//! Pins the standard pricer and metric registries.
//!
//! The snapshot lists, per instrument type, the registered model keys and the
//! applicable metric identifiers. It guards the registry refactors of the
//! valuations crate split: assembling the registries from per-family tables
//! must leave both maps byte-identical. It is the one pinned list of models
//! and metrics per instrument type: regenerate it with `mise run gen-write`
//! after an intentional registration change and review the diff.

use finstack_quant_valuations::pricer::STANDARD;
use std::collections::BTreeMap;

const SNAPSHOT: &str = include_str!("data/registry_snapshot.json");

fn current_snapshot() -> String {
    let pricers = STANDARD.pricers();
    let metrics = STANDARD.metrics();
    let mut by_type: BTreeMap<String, serde_json::Value> = BTreeMap::new();
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
    let mut text = serde_json::to_string_pretty(&document).expect("serialize snapshot");
    text.push('\n');
    text
}

#[test]
fn standard_registries_match_the_pinned_snapshot() {
    let current = current_snapshot();
    if std::env::var_os("UPDATE_REGISTRY_SNAPSHOT").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/registry_snapshot.json"
        );
        std::fs::write(path, &current).expect("write registry snapshot");
        return;
    }
    // Compare per key first, so a mismatch names the instrument type (or the
    // universal metric list) that moved; then the bytes, which pin formatting.
    let parse = |text: &str| -> BTreeMap<String, serde_json::Value> {
        serde_json::from_str(text).expect("registry snapshot is a JSON object")
    };
    let (current_doc, pinned_doc) = (parse(&current), parse(SNAPSHOT));
    let entries =
        |doc: &BTreeMap<String, serde_json::Value>| -> BTreeMap<String, serde_json::Value> {
            let mut out = BTreeMap::new();
            if let Some(serde_json::Value::Object(types)) = doc.get("instrument_types") {
                for (key, value) in types {
                    out.insert(key.clone(), value.clone());
                }
            }
            out.insert(
                "available_metrics".to_string(),
                doc.get("available_metrics").cloned().unwrap_or_default(),
            );
            out
        };
    let (current_entries, pinned_entries) = (entries(&current_doc), entries(&pinned_doc));
    for key in current_entries.keys().chain(pinned_entries.keys()) {
        assert_eq!(
            current_entries.get(key),
            pinned_entries.get(key),
            "registry entry `{key}` differs from tests/data/registry_snapshot.json; if the \
             change is intentional, run `mise run gen-write` and review the diff"
        );
    }
    assert!(
        current == SNAPSHOT,
        "registry snapshot formatting differs from tests/data/registry_snapshot.json; \
         run `mise run gen-write`"
    );
}
