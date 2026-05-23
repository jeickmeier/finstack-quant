//! Integration tests for market scalars attribution.
//!
//! Tests attribution of P&L from changes in dividends, equity prices,
//! inflation indices, and other market scalars.

use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::equity::spot::Equity;
use finstack_valuations::instruments::Instrument;

#[test]
fn test_scalars_snapshot_extraction() {
    use finstack_attribution::{CurveRestoreFlags, MarketSnapshot};

    // Create market with various scalars
    let market = MarketContext::new()
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(180.0, Currency::USD)),
        )
        .insert_price(
            "MSFT",
            MarketScalar::Price(Money::new(400.0, Currency::USD)),
        );

    // Extract scalars through the unified market snapshot path.
    let snapshot = MarketSnapshot::extract(&market, CurveRestoreFlags::SCALARS);

    // Verify extraction
    assert_eq!(snapshot.prices.len(), 2);
    assert!(snapshot.prices.contains_key(&CurveId::from("AAPL")));
    assert!(snapshot.prices.contains_key(&CurveId::from("MSFT")));

    // Restore to new market via the unified MarketSnapshot::restore_market path.
    let empty_market = MarketContext::new();
    let restored =
        MarketSnapshot::restore_market(&empty_market, &snapshot, CurveRestoreFlags::SCALARS);

    // Verify restoration
    let aapl_price = restored.get_price("AAPL").unwrap();
    if let MarketScalar::Price(money) = aapl_price {
        assert_eq!(money.amount(), 180.0);
    } else {
        panic!("Expected Price scalar");
    }
}

#[test]
fn test_market_scalar_freeze_restore() {
    use finstack_attribution::{CurveRestoreFlags, MarketSnapshot};

    // Market at T₀ with lower prices
    let market_t0 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(180.0, Currency::USD)),
    );

    // Market at T₁ with higher prices
    let market_t1 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(185.0, Currency::USD)),
    );

    // Extract T₀ scalars and splice them into the T₁ market.
    let scalars_t0 = MarketSnapshot::extract(&market_t0, CurveRestoreFlags::SCALARS);
    let hybrid_market =
        MarketSnapshot::restore_market(&market_t1, &scalars_t0, CurveRestoreFlags::SCALARS);

    // Verify T₀ price was restored
    let price = hybrid_market.get_price("AAPL").unwrap();
    if let MarketScalar::Price(money) = price {
        assert_eq!(money.amount(), 180.0); // Should be T₀ value
    }
}

#[test]
fn test_equity_price_id_uses_restored_scalar_price() {
    use finstack_attribution::{CurveRestoreFlags, MarketSnapshot};

    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_price_id("AAPL-SPOT")
        .with_shares(1.0);

    let market_t0 = MarketContext::new()
        .insert(
            finstack_core::market_data::term_structures::DiscountCurve::builder("USD")
                .base_date(
                    finstack_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                        .unwrap(),
                )
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .unwrap(),
        )
        .insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(180.0, Currency::USD)),
        );
    let market_t1 = MarketContext::new()
        .insert(
            finstack_core::market_data::term_structures::DiscountCurve::builder("USD")
                .base_date(
                    finstack_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                        .unwrap(),
                )
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .unwrap(),
        )
        .insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(185.0, Currency::USD)),
        );

    let snapshot = MarketSnapshot::extract(&market_t0, CurveRestoreFlags::SCALARS);
    let restored_market =
        MarketSnapshot::restore_market(&market_t1, &snapshot, CurveRestoreFlags::SCALARS);
    let as_of =
        finstack_core::dates::Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let restored_value = equity.value(&restored_market, as_of).unwrap();
    assert_eq!(restored_value.amount(), 180.0);
}
