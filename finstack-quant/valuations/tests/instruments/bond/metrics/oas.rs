//! Option-adjusted spread calculator tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_oas_with_quoted_price() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "OAS2",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(98.0);

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
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let oas = *result.measures.get("oas").unwrap();
    assert!(oas.is_finite());
}
