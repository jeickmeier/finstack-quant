//! Clean and dirty price calculator tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::bond::Bond;
use time::macros::date;

#[test]
fn test_clean_price_from_quoted() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CLEAN1",
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price_pct(98.5);

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
            &[MetricId::CleanPrice],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let clean = *result.measures.get("clean_price").unwrap();
    assert!((clean - 98.5).abs() < 1e-10); // Quoted clean price round-trip must be exact
}

#[test]
fn model_price_metrics_are_settlement_anchored() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "MODEL-CLEAN-DIRTY",
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .unwrap();
    let settlement_df = curve
        .df_between_dates(as_of, date!(2025 - 01 - 02))
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);
    let pv_as_of = bond.value(&market, as_of).unwrap().amount();

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Accrued,
                MetricId::CleanPrice,
                MetricId::DirtyPrice,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let accrued = result.measures["accrued"];
    let clean = result.measures["clean_price"];
    let dirty = result.measures["dirty_price"];

    assert!((dirty - pv_as_of / settlement_df).abs() < 1e-10);
    assert!((dirty - clean - accrued).abs() < 1e-10);
}
