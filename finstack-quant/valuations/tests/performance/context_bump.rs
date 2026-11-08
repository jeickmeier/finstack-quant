//! Exact-output capture and direct-stencil contract for the I1 Vanna consumer.

#[path = "context_bump_fixture.rs"]
mod fixture;

use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use serde_json::{Value, json};

fn bits(value: f64) -> String {
    assert!(value.is_finite());
    format!("{:016x}", value.to_bits())
}

fn record(unrelated: usize, quote_override: bool) -> Value {
    let market = fixture::market(unrelated);
    let mut instrument = fixture::option();
    if quote_override {
        instrument
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
    }
    let result = instrument
        .price_with_metrics(
            &market,
            fixture::as_of(),
            &[MetricId::Vanna],
            PricingOptions::default(),
        )
        .expect("native Vanna consumer");
    let expected = ((fixture::reference_price(101.0, 0.21) - fixture::reference_price(99.0, 0.21))
        / 2.0
        - (fixture::reference_price(101.0, 0.19) - fixture::reference_price(99.0, 0.19)) / 2.0)
        / 2.0;
    let vanna = *result.measures.get(&MetricId::Vanna).expect("Vanna output");
    assert!(vanna.is_finite());
    assert!((vanna - expected).abs() <= 1e-9 * expected.abs().max(1.0));
    assert_eq!(result.value.currency(), Currency::USD);
    assert_eq!(market.curve_ids().len(), unrelated + 1);
    assert!(
        market
            .get_surface("CCM-EQ-VOL")
            .expect("base surface")
            .vols()
            .iter()
            .all(|&vol| vol == 0.20)
    );
    let measures: std::collections::BTreeMap<_, _> = result
        .measures
        .iter()
        .map(|(id, value)| (id.to_string(), bits(*value)))
        .collect();
    let decimal = result.value.amount_decimal();
    json!({
        "case_id": if quote_override { format!("context_bump_consumer/equity_vanna_quote_control/curves_{unrelated}") }
            else { format!("context_bump_consumer/equity_vanna/curves_{unrelated}") },
        "role": if quote_override { "unchanged_quote_override_control" } else { "generic_surface_consumer" },
        "unrelated_curve_count": unrelated, "total_curve_count": unrelated + 1,
        "knots_per_curve": 40, "surface_shape": [3,3], "surface_backed": !quote_override,
        "spot": bits(100.0), "strike": bits(100.0), "rate": bits(0.05),
        "dividend_yield": bits(0.02), "volatility": bits(0.20),
        "maturity_days": 365, "day_count": "act_365f", "quantity": bits(1.0),
        "spot_bump_decimal": bits(0.01), "vol_bump_decimal": bits(0.01),
        "vanna_units": "delta per absolute volatility percentage point",
        "model": format!("{:?}", instrument.default_model()),
        "price_bits": bits(result.value.amount()), "currency": "USD",
        "price_decimal_mantissa": decimal.mantissa().to_string(), "price_decimal_scale": decimal.scale(),
        "measures_bits": measures, "reference_vanna_bits": bits(expected),
        "independent_reference_bound": "1e-9*max(1,abs(reference))",
        "candidate_comparison_bound": "all numeric/decimal/config records exact",
        "base_surface_unchanged": true,
    })
}

#[test]
fn context_bump_vanna_consumer_matches_direct_stencil() {
    let _ = record(8, false);
    let _ = record(8, true);
}

#[test]
#[ignore = "exports the frozen CCM I1 consumer panel on explicit request"]
fn export_context_bump_consumer_reference() {
    let directory =
        std::env::var_os("CCM_CONSUMER_EXPORT_DIR").expect("CCM_CONSUMER_EXPORT_DIR is required");
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create export directory");
    let report = json!({
        "schema_version": 1, "panel": "ccm_i1_equity_vanna", "records": [record(64, false), record(1024, false), record(64, true)],
        "comparison": "C0/candidate exact float-bit, Decimal and structural records; independently checked scalar stencil",
    });
    if let Some(reference) = std::env::var_os("CCM_VANNA_REFERENCE_FILE") {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(reference).expect("read reference"))
                .expect("parse reference");
        assert_eq!(
            report, expected,
            "I1 Vanna outputs/configuration must match C0 exactly"
        );
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("equity-vanna-reference.json"))
        .expect("create new panel without overwriting evidence");
    std::io::Write::write_all(
        &mut output,
        &serde_json::to_vec_pretty(&report).expect("serialize panel"),
    )
    .expect("write panel");
}
