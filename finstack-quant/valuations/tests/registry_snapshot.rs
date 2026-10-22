//! Pins the standard pricer and metric registries.
//!
//! The snapshot lists, per instrument type, the registered model keys and the
//! applicable metric identifiers. It guards the registry refactors of the
//! valuations crate split: assembling the registries from per-family tables
//! must leave both maps byte-identical. Regenerate deliberately with
//! `UPDATE_REGISTRY_SNAPSHOT=1` after an intentional registration change and
//! review the diff.

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
    assert!(
        current == SNAPSHOT,
        "standard registries differ from tests/data/registry_snapshot.json; \
         if the change is intentional, rerun with UPDATE_REGISTRY_SNAPSHOT=1 and review the diff"
    );
}
