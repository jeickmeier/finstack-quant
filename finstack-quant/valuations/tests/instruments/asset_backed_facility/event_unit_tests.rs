//! Regression baseline for the 2026-09-17 analyst audit (P4): facility
//! amortization events must project with the documented units.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, AssetBackedFacility,
};

fn market(base: finstack_quant_core::dates::Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=15)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// `CumulativeLoss { max_pct }` is a percent at the facility boundary and a
/// fraction in the engine; an excess-spread-only rule is expressible.
#[test]
fn p4_facility_amortization_event_units() {
    let base = AssetBackedFacility::example().expect("example");
    let mkt = market(base.closing_date);

    let mut pct = base.clone();
    pct.amortization_events = vec![AmortizationEvent::CumulativeLoss { max_pct: 4.0 }];
    pct.project(&mkt, base.closing_date)
        .expect("a 4% cumulative-loss event projects");

    let mut es_only = base.clone();
    es_only.amortization_events = vec![AmortizationEvent::ExcessSpread { min_3m: 0.01 }];
    es_only
        .project(&mkt, base.closing_date)
        .expect("an excess-spread-only facility projects");

    let mut half_pct = base;
    half_pct.amortization_events = vec![AmortizationEvent::CumulativeLoss { max_pct: 0.5 }];
    let deal = half_pct.synthesized_deal().expect("deal");
    let early_am = deal
        .waterfall_rules
        .as_ref()
        .and_then(|rules| rules.early_amortization.as_ref())
        .expect("early amortization rule");
    let fraction = serde_json::to_value(early_am)
        .expect("json")
        .get("max_cumulative_loss")
        .and_then(serde_json::Value::as_f64)
        .expect("max_cumulative_loss fraction");
    assert!(
        (fraction - 0.005).abs() < 1e-12,
        "0.5% must reach the engine as the fraction 0.005, got {fraction}"
    );
}
