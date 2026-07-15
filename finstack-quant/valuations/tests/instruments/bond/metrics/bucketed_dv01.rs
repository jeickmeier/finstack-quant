//! Bucketed DV01 tests for bond instruments.
//!
//! Covers the fix for plain (non-callable) bonds with a `quoted_clean_price`
//! override returning all-zero bucketed DV01 values. The engine must calibrate
//! a Z-spread from the quoted price and reprice the spread-pinned clone through
//! the curve-bump loop so that key-rate buckets are non-zero and reconcile with
//! the aggregate.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Build a realistic multi-tenor discount curve for bucketed DV01 tests.
fn build_multi_tenor_curve(as_of: time::Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.25, 0.989),
            (0.5, 0.978),
            (1.0, 0.956),
            (2.0, 0.912),
            (3.0, 0.868),
            (5.0, 0.790),
            (7.0, 0.715),
            (10.0, 0.640),
        ])
        .build()
        .expect("multi-tenor discount curve should build")
}

/// A plain fixed-rate 10Y bond (no call/put schedule).
fn build_plain_bond(as_of: time::Date) -> Bond {
    Bond::fixed(
        "BDKR-PLAIN",
        Money::new(10_000_000.0, Currency::USD),
        0.0425,
        as_of,
        date!(2034 - 03 - 15),
        "USD-OIS",
    )
    .expect("plain fixed bond should build")
}

/// Test that a quoted-price plain bond produces non-zero bucketed DV01 values
/// that reconcile with the scalar DV01 and bucketed aggregate.
///
/// This is the regression for the bug: `Bond::base_value` short-circuits to
/// the constant quoted dirty price, so the curve-bump loop reprices the same
/// constant and every key-rate bucket is 0. The fix calibrates a Z-spread from
/// the quote and reprices the spread-pinned clone through the bump loop.
#[test]
fn test_plain_bond_quoted_price_bucketed_dv01_nonzero() {
    let as_of = date!(2024 - 03 - 15);
    let mut bond = build_plain_bond(as_of);
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.5);

    let market = MarketContext::new().insert(build_multi_tenor_curve(as_of));

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::ZSpread, MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("quoted-price bond bucketed DV01 should compute");

    // 1. The composite bucketed keys are populated and NOT all zero.
    let nonzero = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with("bucketed_dv01::") && v.abs() > 1e-6)
        .count();
    assert!(
        nonzero >= 4,
        "expected >=4 populated key-rate buckets, got {nonzero}; measures: {:?}",
        result
            .measures
            .iter()
            .filter(|(k, _)| k.as_str().starts_with("bucketed_dv01::"))
            .collect::<Vec<_>>()
    );

    // 2. The bucketed aggregate is non-zero and same sign as scalar DV01.
    let agg = result.measures.get("bucketed_dv01").copied().unwrap();
    let scalar = result.measures.get("dv01").copied().unwrap();
    assert!(
        agg.abs() > 1e-6,
        "bucketed_dv01 aggregate should be non-zero, got {agg}"
    );
    assert!(
        agg.signum() == scalar.signum(),
        "bucketed aggregate sign ({}) should match scalar dv01 sign ({})",
        agg,
        scalar
    );

    // 3. Reconciliation: summed buckets ≈ bucketed aggregate (same model).
    let sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_dv01::"))
        .map(|(_, v)| *v)
        .sum();
    assert!(
        (sum - agg).abs() < (agg.abs() * 0.02 + 1.0),
        "sum of bucketed key-rate DV01 ({sum:.4}) should reconcile with aggregate ({agg:.4})"
    );
}

/// Regression: a plain bond WITHOUT a quoted price still produces non-zero
/// bucketed DV01 (the no-quote path is unchanged).
#[test]
fn test_plain_bond_no_quote_bucketed_dv01_nonzero() {
    let as_of = date!(2024 - 03 - 15);
    let bond = build_plain_bond(as_of); // no pricing_overrides set

    let market = MarketContext::new().insert(build_multi_tenor_curve(as_of));

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("no-quote bond bucketed DV01 should compute");

    let nonzero = result
        .measures
        .iter()
        .filter(|(k, v)| k.as_str().starts_with("bucketed_dv01::") && v.abs() > 1e-6)
        .count();
    assert!(
        nonzero >= 4,
        "no-quote path: expected >=4 populated key-rate buckets, got {nonzero}"
    );

    let agg = result.measures.get("bucketed_dv01").copied().unwrap();
    assert!(
        agg.abs() > 1e-6,
        "no-quote bucketed_dv01 aggregate should be non-zero, got {agg}"
    );
}
