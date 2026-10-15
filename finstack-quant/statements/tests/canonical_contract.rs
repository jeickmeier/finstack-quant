//! Canonical bytes, hash, and insertion-order invariance for financial models.

use finstack_quant_core::canonical::to_canonical_bytes;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_statements::types::FinancialModelSpec;

fn model_bytes(meta: &str) -> Vec<u8> {
    let mut base: serde_json::Value =
        serde_json::from_str(include_str!("data/canonical/financial_model.json"))
            .expect("canonical model parses");
    base["meta"] = serde_json::json!({});
    serde_json::to_string(&base)
        .expect("financial-model fixture serializes")
        .replace(r#""meta":{}"#, &format!(r#""meta":{meta}"#))
        .into_bytes()
}

#[test]
fn financial_model_has_golden_canonical_bytes_hash_and_order_invariance() {
    let first_bytes = model_bytes(r#"{"desk":"corp","source":"golden"}"#);
    let second_bytes = model_bytes(r#"{"source":"golden","desk":"corp"}"#);
    let (first, _) = FinancialModelSpec::from_slice_strict(&first_bytes, &LoadLimits::default())
        .expect("first financial model is strictly valid");
    let (second, _) = FinancialModelSpec::from_slice_strict(&second_bytes, &LoadLimits::default())
        .expect("second financial model is strictly valid");

    let canonical = to_canonical_bytes(&first).expect("financial model canonicalizes");
    let fixture = include_bytes!("data/canonical/financial_model.json");
    let fixture = fixture.strip_suffix(b"\n").unwrap_or(fixture);
    assert_eq!(canonical, fixture);
    assert_eq!(
        first.content_hash().expect("financial model hashes"),
        include_str!("data/canonical/financial_model.sha256").trim()
    );
    assert_eq!(
        canonical,
        to_canonical_bytes(&second).expect("reverse-order financial model canonicalizes")
    );
}
