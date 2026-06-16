use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;

#[test]
fn valuation_result_roundtrip_serde() {
    // Build a simple result
    let as_of = time::macros::date!(2024 - 01 - 15);
    let mut vr = ValuationResult::stamped("BOND-1", as_of, Money::new(100.0, Currency::USD));
    vr.measures.insert(MetricId::custom("pv"), 100.0);
    vr.measures.insert(MetricId::Dv01, 0.0123);

    // Serialize to JSON
    let json = serde_json::to_string(&vr).expect("serialize");

    // Deserialize back
    let de: ValuationResult = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(de.instrument_id, "BOND-1");
    assert_eq!(de.as_of, as_of);
    assert_eq!(de.value.currency(), Currency::USD);
    assert!((de.value.amount() - 100.0).abs() < 1e-9);
    assert!(de.measures.get("pv").is_some());
    assert!(de.measures.get("dv01").is_some());
    // Ensure meta carried through
    assert_eq!(
        de.meta.numeric_mode as u8,
        finstack_quant_core::config::NUMERIC_MODE as u8
    );
}
