//! Canonical bytes, hash, and insertion-order invariance for scenario envelopes.

use finstack_quant_core::canonical::to_canonical_bytes;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_scenarios::ScenarioEnvelope;

fn scenario_bytes(attrs: &str) -> Vec<u8> {
    include_str!("data/canonical/scenario.json")
        .replace(
            r#""attrs":{"desk":"rates","region":"us"}"#,
            &format!(r#""attrs":{attrs}"#),
        )
        .into_bytes()
}

#[test]
fn scenario_envelope_has_golden_canonical_bytes_hash_and_order_invariance() {
    let first_bytes = scenario_bytes(r#"{"desk":"rates","region":"us"}"#);
    let second_bytes = scenario_bytes(r#"{"region":"us","desk":"rates"}"#);
    ScenarioEnvelope::from_slice_strict(&first_bytes, &LoadLimits::default())
        .expect("first scenario is strictly valid");
    ScenarioEnvelope::from_slice_strict(&second_bytes, &LoadLimits::default())
        .expect("second scenario is strictly valid");
    let first: ScenarioEnvelope =
        serde_json::from_slice(&first_bytes).expect("first envelope parses");
    let second: ScenarioEnvelope =
        serde_json::from_slice(&second_bytes).expect("second envelope parses");

    let canonical = to_canonical_bytes(&first).expect("scenario canonicalizes");
    assert_eq!(canonical, include_bytes!("data/canonical/scenario.json"));
    assert_eq!(
        first.content_hash().expect("scenario hashes"),
        include_str!("data/canonical/scenario.sha256").trim()
    );
    assert_eq!(
        canonical,
        to_canonical_bytes(&second).expect("reverse-order scenario canonicalizes")
    );
}
