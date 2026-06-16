//! Convexity calculator tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Convexity validation for vanilla bonds
///
/// All vanilla (non-callable) bonds have positive convexity.
///
/// For a 5-year par bond at ~5% YTM, raw convexity is typically in the range
/// 15-40, and the public metric reports Bloomberg-style display units (`raw / 100`).
/// The analytical formula: C = Σ[t(t+1) × CF_t × DF_t] / (P × (1+y/m)²)
#[test]
fn test_convexity_positive() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CVX1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.pricing_overrides = finstack_quant_valuations::instruments::PricingOverrides::default()
        .with_quoted_clean_price(100.0);

    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Convexity],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cvx = *result.measures.get("convexity").unwrap();

    // All vanilla bonds have positive convexity
    assert!(cvx > 0.0, "Vanilla bonds should have positive convexity");

    // Bloomberg YAS display units scale raw mathematical convexity by 1/100.
    assert!(
        cvx > 0.15 && cvx < 0.40,
        "5Y par bond convexity {:.2} outside expected range [0.15, 0.40]",
        cvx
    );
}
