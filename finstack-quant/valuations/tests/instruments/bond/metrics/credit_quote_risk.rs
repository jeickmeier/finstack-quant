//! Quoted credit-bond risk regression.
//!
//! A bond with a credit (hazard) curve AND a `quoted_clean_price` must still
//! produce non-zero CS01 / bucketed CS01 — the engine calibrates a flat hazard
//! shift that reproduces the quote and bumps that shifted curve, mirroring the
//! same bond priced WITHOUT a quote. Before the fix, `Bond::base_value`
//! short-circuits to the constant quoted price, so the hazard bump reprices the
//! same constant and CS01 collapses to zero.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions, PricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_credit_bond(as_of: time::Date) -> Bond {
    let mut bond = Bond::fixed(
        "CREDIT-Q",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .expect("credit bond should build");
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn build_market(as_of: time::Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (3.0, 0.91),
            (5.0, 0.83),
        ])
        .build()
        .expect("discount curve should build");
    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (5.0, 0.02)])
        .build()
        .expect("hazard curve should build");
    MarketContext::new().insert(disc).insert(hazard)
}

#[test]
fn test_quoted_credit_bond_cs01_nonzero_and_matches_unquoted() {
    let as_of = date!(2025 - 01 - 01);
    let market = build_market(as_of);

    // Unquoted: model clean price + reference CS01.
    let unquoted = build_credit_bond(as_of);
    let base = unquoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::CleanPrice],
            PricingOptions::default(),
        )
        .expect("unquoted credit bond should price");
    let base_cs01 = *base.measures.get("cs01").unwrap();
    let model_clean_pct = *base.measures.get("clean_price").unwrap() / 1_000_000.0 * 100.0;
    assert!(
        base_cs01.abs() > 1e-3,
        "sanity: unquoted credit CS01 should be non-zero, got {base_cs01}"
    );

    // Quoted at the model clean price → calibrated hazard shift ≈ 0 → risk ≈ unquoted.
    let mut quoted = build_credit_bond(as_of);
    quoted.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(model_clean_pct);
    let result = quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            PricingOptions::default(),
        )
        .expect("quoted credit bond should price");

    let cs01 = *result.measures.get("cs01").unwrap();
    assert!(
        cs01.abs() > 1e-3,
        "quoted credit CS01 must be non-zero (was 0 before the fix), got {cs01}"
    );

    let bucketed_nonzero = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with("bucketed_cs01") && v.abs() > 1e-6)
        .count();
    assert!(
        bucketed_nonzero >= 1,
        "quoted credit bucketed CS01 must be populated, got {bucketed_nonzero}"
    );

    assert!(
        (cs01 - base_cs01).abs() < (base_cs01.abs() * 0.05 + 1.0),
        "quoted CS01 ({cs01:.4}) should reconcile with unquoted ({base_cs01:.4})"
    );
}
