//! Equity types and construction tests

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::Equity;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::Instrument;

fn build_flat_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (0.5, (-rate * 0.5).exp()), (1.0, (-rate).exp())]);

    // For zero or negative rates, the curve may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().expect("Failed to build discount curve")
}

#[test]
fn test_equity_new_defaults() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);

    assert_eq!(equity.id.as_str(), "AAPL");
    assert_eq!(equity.ticker, "AAPL");
    assert_eq!(equity.currency, Currency::USD);
    assert_eq!(equity.quantity, None);
    assert_eq!(equity.quoted_spot, None);
    assert_eq!(equity.spot_id, None);
    assert_eq!(equity.div_yield_id, None);
    assert_eq!(equity.discount_curve_id.as_str(), "USD");
}

#[test]
fn test_equity_new_with_different_currencies() {
    let equity_usd = Equity::new("AAPL", "AAPL", Currency::USD);
    assert_eq!(equity_usd.discount_curve_id.as_str(), "USD");

    let equity_eur = Equity::new("SAP", "SAP", Currency::EUR);
    assert_eq!(equity_eur.discount_curve_id.as_str(), "EUR");

    let equity_gbp = Equity::new("BP", "BP", Currency::GBP);
    assert_eq!(equity_gbp.discount_curve_id.as_str(), "GBP");

    // Other currencies preserve their own currency code
    let equity_jpy = Equity::new("SONY", "SONY", Currency::JPY);
    assert_eq!(equity_jpy.discount_curve_id.as_str(), "JPY");
}

#[test]
fn test_equity_with_shares() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quantity(100.0);

    assert_eq!(equity.quantity, Some(100.0));
}

#[test]
fn test_equity_with_price() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quoted_spot(150.0);

    assert_eq!(equity.quoted_spot, Some(150.0));
}

#[test]
fn test_equity_with_price_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_spot_id("CUSTOM_PRICE");

    assert_eq!(
        equity.spot_id,
        Some(finstack_quant_core::types::PriceId::new("CUSTOM_PRICE"))
    );
}

#[test]
fn test_equity_with_dividend_yield_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_dividend_yield_id("CUSTOM_DIV");

    assert_eq!(
        equity.div_yield_id,
        Some(finstack_quant_core::types::PriceId::new("CUSTOM_DIV"))
    );
}

#[test]
fn test_equity_builder_chaining() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_quantity(100.0)
        .with_quoted_spot(150.0)
        .with_spot_id("CUSTOM_PRICE")
        .with_dividend_yield_id("CUSTOM_DIV");

    assert_eq!(equity.quantity, Some(100.0));
    assert_eq!(equity.quoted_spot, Some(150.0));
    assert_eq!(
        equity.spot_id,
        Some(finstack_quant_core::types::PriceId::new("CUSTOM_PRICE"))
    );
    assert_eq!(
        equity.div_yield_id,
        Some(finstack_quant_core::types::PriceId::new("CUSTOM_DIV"))
    );
}

#[test]
fn test_equity_effective_shares_none() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    assert_eq!(equity.effective_quantity(), 1.0);
}

#[test]
fn test_equity_effective_shares_set() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quantity(50.0);
    assert_eq!(equity.effective_quantity(), 50.0);
}

#[test]
fn test_equity_price_per_share_with_quote() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quoted_spot(175.0);

    let market = MarketContext::new();
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let price = equity.price_per_share(&market, as_of).unwrap();
    assert_eq!(price.amount(), 175.0);
    assert_eq!(price.currency(), Currency::USD);
}

#[test]
fn test_equity_price_per_share_from_spot_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_spot_id("AAPL");

    let market = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(200.0, Currency::USD).expect("valid money fixture")),
    );

    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let price = equity.price_per_share(&market, as_of).unwrap();
    assert_eq!(price.amount(), 200.0);
}

#[test]
fn test_equity_price_per_share_unitless() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_spot_id("AAPL");

    // Unitless scalar is treated as amount in equity currency
    let market = MarketContext::new().insert_price("AAPL", MarketScalar::Unitless(210.0));

    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let price = equity.price_per_share(&market, as_of).unwrap();
    assert_eq!(price.amount(), 210.0);
    assert_eq!(price.currency(), Currency::USD);
}

#[test]
fn test_equity_dividend_yield_default() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    let market = MarketContext::new();

    let div_yield = equity.dividend_yield(&market).unwrap();
    assert_eq!(div_yield, 0.0);
}

#[test]
fn test_equity_dividend_yield_ignores_ticker_suffix() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);

    // No `div_yield_id` means a zero yield; `{ticker}-DIVYIELD` is not read.
    let market = MarketContext::new().insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    let div_yield = equity.dividend_yield(&market).unwrap();
    assert_eq!(div_yield, 0.0);
}

#[test]
fn test_equity_dividend_yield_from_custom_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_dividend_yield_id("MY_DIV_YIELD");

    let market = MarketContext::new().insert_price("MY_DIV_YIELD", MarketScalar::Unitless(0.035));

    let div_yield = equity.dividend_yield(&market).unwrap();
    assert_eq!(div_yield, 0.035);
}

#[test]
fn test_equity_instrument_trait_key() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    assert_eq!(
        equity.key(),
        finstack_quant_valuations::pricer::InstrumentType::Equity
    );
}

#[test]
fn test_equity_instrument_trait_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    assert_eq!(equity.id.as_str(), "AAPL");
}

#[test]
fn test_equity_discount_curve_id() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    let disc_id = equity
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves
        .first()
        .cloned()
        .expect("Equity should declare a discount curve");
    assert_eq!(disc_id.as_str(), "USD");
}

#[test]
fn test_equity_clone() {
    let equity1 = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_quantity(100.0)
        .with_quoted_spot(150.0);

    let equity2 = equity1.clone();

    assert_eq!(equity1.id, equity2.id);
    assert_eq!(equity1.ticker, equity2.ticker);
    assert_eq!(equity1.quantity, equity2.quantity);
    assert_eq!(equity1.quoted_spot, equity2.quoted_spot);
}

#[test]
fn test_equity_with_attributes() {
    let attrs = Attributes::new()
        .with_meta("sector", "Technology")
        .with_meta("exchange", "NASDAQ");

    let equity = Equity::builder()
        .id("AAPL".into())
        .ticker("AAPL".into())
        .currency(Currency::USD)
        .discount_curve_id("USD".into())
        .attributes(attrs)
        .build()
        .unwrap();

    assert_eq!(equity.attributes.get_meta("sector"), Some("Technology"));
    assert_eq!(equity.attributes.get_meta("exchange"), Some("NASDAQ"));
}

#[test]
fn test_equity_price_resolution_priority() {
    // Price quote should override market data
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quoted_spot(100.0);

    let market = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(200.0, Currency::USD).expect("valid money fixture")),
    );

    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let price = equity.price_per_share(&market, as_of).unwrap();

    // Should use the quote, not market data
    assert_eq!(price.amount(), 100.0);
}

#[test]
fn test_equity_custom_price_id_priority() {
    // Custom price_id should override ticker-based lookup
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_spot_id("CUSTOM_PRICE");

    let market = MarketContext::new()
        .insert_price(
            "CUSTOM_PRICE",
            MarketScalar::Price(Money::new(150.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(200.0, Currency::USD).expect("valid money fixture")),
        );

    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let price = equity.price_per_share(&market, as_of).unwrap();

    // Should use custom price_id
    assert_eq!(price.amount(), 150.0);
}

#[test]
fn test_equity_fx_conversion() {
    // EUR equity priced in EUR
    let equity = Equity::new("SAP", "SAP", Currency::EUR).with_spot_id("SAP");

    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let eur_curve = build_flat_curve(0.03, base_date, "EUR-OIS");

    let market = MarketContext::new().insert(eur_curve).insert_price(
        "SAP",
        MarketScalar::Price(Money::new(100.0, Currency::EUR).expect("valid money fixture")),
    );

    let price = equity.price_per_share(&market, base_date).unwrap();
    assert_eq!(price.amount(), 100.0);
    assert_eq!(price.currency(), Currency::EUR);
}

#[test]
fn test_equity_price_missing() {
    let equity = Equity::new("UNKNOWN", "UNKNOWN", Currency::USD);
    let market = MarketContext::new();
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    // Should fail when price not found
    let result = equity.price_per_share(&market, as_of);
    assert!(result.is_err());
}

#[test]
fn test_equity_zero_shares() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quantity(0.0);
    assert_eq!(equity.effective_quantity(), 0.0);
}

#[test]
fn test_equity_negative_shares() {
    // Short position
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quantity(-50.0);
    assert_eq!(equity.effective_quantity(), -50.0);
}

#[test]
fn test_equity_fractional_shares() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_quantity(12.5);
    assert_eq!(equity.effective_quantity(), 12.5);
}

#[test]
fn test_equity_serde_roundtrip() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_quantity(100.0)
        .with_quoted_spot(150.0);

    let json = serde_json::to_string(&equity).unwrap();
    let deserialized: Equity = serde_json::from_str(&json).unwrap();

    assert_eq!(equity.id, deserialized.id);
    assert_eq!(equity.ticker, deserialized.ticker);
    assert_eq!(equity.quantity, deserialized.quantity);
    assert_eq!(equity.quoted_spot, deserialized.quoted_spot);
}
