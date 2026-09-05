//! Integration tests for market scalars attribution.
//!
//! Tests attribution of P&L from changes in dividends, equity prices,
//! inflation indices, and other market scalars.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::equity::spot::Equity;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_scalars_snapshot_extraction() {
    use finstack_quant_attribution::{MarketRestoreFlags, MarketSnapshot};

    // Create market with various scalars
    let market = MarketContext::new()
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "MSFT",
            MarketScalar::Price(Money::new(400.0, Currency::USD).expect("valid money fixture")),
        );

    // Extract scalars through the unified market snapshot path.
    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::SCALARS);

    // Verify extraction
    assert_eq!(snapshot.prices.len(), 2);
    assert!(snapshot.prices.contains_key(&CurveId::from("AAPL")));
    assert!(snapshot.prices.contains_key(&CurveId::from("MSFT")));

    // Restore to new market via the unified MarketSnapshot::restore_market path.
    let empty_market = MarketContext::new();
    let restored =
        MarketSnapshot::restore_market(&empty_market, &snapshot, MarketRestoreFlags::SCALARS);

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
    use finstack_quant_attribution::{MarketRestoreFlags, MarketSnapshot};

    // Market at T₀ with lower prices
    let market_t0 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
    );

    // Market at T₁ with higher prices
    let market_t1 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(185.0, Currency::USD).expect("valid money fixture")),
    );

    // Extract T₀ scalars and splice them into the T₁ market.
    let scalars_t0 = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::SCALARS);
    let hybrid_market =
        MarketSnapshot::restore_market(&market_t1, &scalars_t0, MarketRestoreFlags::SCALARS);

    // Verify T₀ price was restored
    let price = hybrid_market.get_price("AAPL").unwrap();
    if let MarketScalar::Price(money) = price {
        assert_eq!(money.amount(), 180.0); // Should be T₀ value
    }
}

#[test]
fn test_equity_price_id_uses_restored_scalar_price() {
    use finstack_quant_attribution::{MarketRestoreFlags, MarketSnapshot};

    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_price_id("AAPL-SPOT")
        .with_shares(1.0);

    let market_t0 = MarketContext::new()
        .insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD")
                .base_date(
                    finstack_quant_core::dates::Date::from_calendar_date(
                        2024,
                        time::Month::January,
                        1,
                    )
                    .unwrap(),
                )
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .unwrap(),
        )
        .insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
        );
    let market_t1 = MarketContext::new()
        .insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD")
                .base_date(
                    finstack_quant_core::dates::Date::from_calendar_date(
                        2024,
                        time::Month::January,
                        1,
                    )
                    .unwrap(),
                )
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .unwrap(),
        )
        .insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(185.0, Currency::USD).expect("valid money fixture")),
        );

    let snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::SCALARS);
    let restored_market =
        MarketSnapshot::restore_market(&market_t1, &snapshot, MarketRestoreFlags::SCALARS);
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let restored_value = equity.value(&restored_market, as_of).unwrap();
    assert_eq!(restored_value.amount(), 180.0);
}

#[test]
fn test_taylor_equity_spot_move_lands_in_market_scalars_pnl() {
    use finstack_quant_attribution::{
        attribute_pnl, AttributionMethod, AttributionRequest, ExecutionPolicy,
        TaylorAttributionConfig,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use std::sync::Arc;
    use time::macros::date;

    let as_of_t0 = date!(2024 - 01 - 01);
    let as_of_t1 = date!(2024 - 01 - 02);
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_price_id("AAPL-SPOT")
        .with_shares(1.0);
    let instrument: Arc<dyn Instrument> = Arc::new(equity);

    let discount = || {
        DiscountCurve::builder("USD")
            .base_date(as_of_t0)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .unwrap()
    };
    let market_t0 = MarketContext::new().insert(discount()).insert_price(
        "AAPL-SPOT",
        MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
    );
    let market_t1 = MarketContext::new().insert(discount()).insert_price(
        "AAPL-SPOT",
        MarketScalar::Price(Money::new(185.0, Currency::USD).expect("valid money fixture")),
    );

    let attribution = attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor equity attribution should succeed");

    assert!(
        (attribution.market_scalars_pnl.amount() - 5.0).abs() < 1e-6,
        "spot move must land in market_scalars_pnl, got {}",
        attribution.market_scalars_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-4,
        "residual must shrink once the spot move is attributed, got {}",
        attribution.residual
    );
}
