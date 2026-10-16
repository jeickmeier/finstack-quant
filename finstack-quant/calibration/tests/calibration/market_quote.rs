//! Tests for `market::quotes::market_quote` (`MarketQuote` serde).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::quotes::MarketQuote;
use finstack_quant_core::types::IndexId;

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
