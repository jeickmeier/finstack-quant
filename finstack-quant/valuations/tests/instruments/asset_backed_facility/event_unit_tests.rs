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

/// `CumulativeLoss { max_cumulative_loss }` is the decimal fraction of the
/// original pool the engine uses; it reaches the engine unchanged (bitwise),
/// so a facility written with the retired percent `max_pct: 0.5` (divided by
/// 100 at the boundary) and one written with `0.005` project identically. An
/// excess-spread-only rule is expressible.
#[test]
fn p4_facility_amortization_event_units() {
    let base = AssetBackedFacility::example().expect("example");
    let mkt = market(base.closing_date);

    let mut loss = base.clone();
    loss.amortization_events = vec![AmortizationEvent::CumulativeLoss {
        max_cumulative_loss: 0.04,
    }];
    loss.project(&mkt, base.closing_date)
        .expect("a 4% cumulative-loss event projects");

    let mut es_only = base.clone();
    es_only.amortization_events = vec![AmortizationEvent::ExcessSpread {
        min_excess_spread_3m: 0.01,
    }];
    es_only
        .project(&mkt, base.closing_date)
        .expect("an excess-spread-only facility projects");

    let mut half_pct = base;
    half_pct.amortization_events = vec![AmortizationEvent::CumulativeLoss {
        max_cumulative_loss: 0.005,
    }];
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
    // Identity with the retired percent input: 0.5 / 100.0 is the correctly
    // rounded 0.005, the literal the engine now receives directly.
    assert_eq!(fraction.to_bits(), (0.5_f64 / 100.0).to_bits());
}

/// A percent-scale threshold is outside `(0, 1]` and is rejected.
#[test]
fn cumulative_loss_threshold_is_a_decimal_fraction() {
    let mut facility = AssetBackedFacility::example().expect("example");
    facility.amortization_events = vec![AmortizationEvent::CumulativeLoss {
        max_cumulative_loss: 4.0,
    }];
    let err = facility.validate().expect_err("4.0 is 400%");
    assert!(err.to_string().contains("max_cumulative_loss"), "{err}");
}

#[test]
// schema-rejection-test
fn retired_max_pct_and_min_3m_keys_are_rejected() {
    for retired in [
        serde_json::json!({"kind": "cumulative_loss", "max_pct": 4.0}),
        serde_json::json!({"kind": "excess_spread", "min_3m": 0.01}),
    ] {
        serde_json::from_value::<AmortizationEvent>(retired.clone())
            .expect_err(&format!("{retired} uses a retired key"));
    }
}
