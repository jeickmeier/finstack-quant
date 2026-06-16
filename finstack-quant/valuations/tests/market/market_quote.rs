//! Tests for `market::quotes::market_quote` (`MarketQuote` serde).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations::market::conventions::ids::BondConventionId;
use finstack_quant_valuations::market::quotes::bond::BondQuote;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;

fn sample_bond_quote() -> BondQuote {
    let issue = Date::from_calendar_date(2025, time::Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, time::Month::January, 15).unwrap();
    BondQuote::FixedRateBulletCleanPrice {
        id: QuoteId::new("BOND-MQ"),
        currency: Currency::USD,
        issue_date: issue,
        maturity,
        coupon_rate: 0.04,
        convention: BondConventionId::new("USD-UST"),
        clean_price_pct: 99.0,
    }
}

#[test]
fn market_quote_rates_round_trips_serde() {
    let original = MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new("USD-DEP-1M"),
        index: IndexId::new("USD-SOFR-1M"),
        pillar: Pillar::Tenor("1M".parse().unwrap()),
        rate: 0.0525,
    });
    let json = serde_json::to_string(&original).expect("serialize");
    let back: MarketQuote = serde_json::from_str(&json).expect("deserialize");
    match back {
        MarketQuote::Rates(RateQuote::Deposit { rate, .. }) => {
            assert!((rate - 0.0525).abs() < 1e-12);
        }
        other => panic!("expected deposit quote, got {other:?}"),
    }
}

#[test]
fn market_quote_bond_round_trips_serde() {
    let bond = MarketQuote::Bond(sample_bond_quote());
    let json = serde_json::to_string(&bond).expect("serialize");
    let back: MarketQuote = serde_json::from_str(&json).expect("deserialize");
    match back {
        MarketQuote::Bond(BondQuote::FixedRateBulletCleanPrice {
            clean_price_pct, ..
        }) => assert!((clean_price_pct - 99.0).abs() < 1e-9),
        other => panic!("expected bond quote, got {other:?}"),
    }
}
