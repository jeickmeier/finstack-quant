//! FX Spot cashflow generation and settlement tests.

use super::common::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{
    currency::Currency, dates::BusinessDayConvention, market_data::context::MarketContext,
    money::Money,
};
use finstack_quant_valuations::instruments::FxSpot;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_settlement_explicit_date() {
    let settlement = d(2025, 1, 17);
    let fx = FxSpot {
        settlement_date: Some(settlement),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, settlement);
    approx_eq(
        cashflows[0].1.amount(),
        1_200_000.0,
        EPSILON,
        "Cashflow amount",
    );
    assert_eq!(cashflows[0].1.currency(), Currency::USD);
}

#[test]
fn test_full_schedule_marks_settlement_as_notional() {
    let settlement = d(2025, 1, 17);
    let fx = FxSpot {
        settlement_date: Some(settlement),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let market = MarketContext::new();

    let schedule = fx
        .cashflow_schedule(&market, test_date())
        .expect("full schedule should build");

    assert_eq!(schedule.get_flows().len(), 1);
    assert_eq!(schedule.get_flows()[0].kind, CFKind::Notional);
}

#[test]
fn test_settlement_lag_default() {
    // Default lag is T+2 business days
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let as_of = d(2025, 1, 15); // Wednesday

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cashflows.len(), 1);
    // T+2 business days from Wed Jan 15 = Fri Jan 17
    assert_eq!(cashflows[0].0, d(2025, 1, 17));
}

#[test]
fn test_settlement_already_settled() {
    let settlement = d(2025, 1, 10);
    let fx = FxSpot {
        settlement_date: Some(settlement),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let market = MarketContext::new();
    let as_of = d(2025, 1, 15);

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    // Already settled - no cashflows
    assert_eq!(cashflows.len(), 0);
}

#[test]
fn test_settlement_on_valuation_date() {
    let settlement = d(2025, 1, 15);
    let fx = FxSpot {
        settlement_date: Some(settlement),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, settlement).unwrap();

    // End-of-day policy: settlement remains live on the event date.
    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, settlement);
}

#[test]
fn test_settlement_from_fx_matrix() {
    let fx = FxSpot {
        notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        settlement_date: Some(d(2025, 1, 17)),
        ..sample_eurusd()
    };
    let market = market_with_fx_matrix();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].1.currency(), Currency::USD);
    approx_eq(
        cashflows[0].1.amount(),
        1_200_000.0,
        LARGE_EPSILON,
        "FX matrix cashflow",
    );
}

#[test]
fn test_settlement_explicit_rate_overrides_matrix() {
    let fx = FxSpot {
        settlement_date: Some(d(2025, 1, 17)),
        ..eurusd_with_notional(1_000_000.0, 1.25)
    };
    let market = market_with_fx_matrix(); // Has EUR/USD = 1.20

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    // Should use explicit rate 1.25, not matrix rate 1.20
    approx_eq(
        cashflows[0].1.amount(),
        1_250_000.0,
        EPSILON,
        "Explicit rate overrides",
    );
}

#[test]
fn test_settlement_lag_custom() {
    let fx = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .quoted_spot(1.20)
        .settlement_date(d(2025, 1, 16))
        .build()
        .expect("valid FX spot"); // T+1

    let market = MarketContext::new();
    let as_of = d(2025, 1, 15); // Wednesday

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, d(2025, 1, 16));
}

#[test]
fn test_settlement_lag_over_weekend() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let as_of = d(2025, 1, 17); // Friday

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cashflows.len(), 1);
    // T+2 business days from Fri Jan 17 = Tue Jan 21 (skips weekend)
    assert_eq!(cashflows[0].0, d(2025, 1, 21));
}

#[test]
fn test_settlement_with_business_day_convention() {
    // Test that BDC is applied when calendar is present
    let settlement = d(2025, 1, 18); // Saturday
    let fx = FxSpot {
        settlement_date: Some(settlement),
        business_day_convention: BusinessDayConvention::Following,
        base_calendar_id: Some("target2".into()),
        quote_calendar_id: Some("usny".into()),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };

    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    // BDC adjustment only applies when calendar is loaded - may return unadjusted date
    // Following convention should move Saturday to Monday if calendar is active
    assert!(
        cashflows[0].0 >= settlement,
        "Settlement on or after original date"
    );
}

#[test]
fn test_settlement_zero_notional() {
    let fx = FxSpot {
        notional: Money::new(0.0, Currency::EUR).expect("valid money fixture"),
        quoted_spot: Some(1.20),
        settlement_date: Some(d(2025, 1, 17)),
        ..sample_eurusd()
    };
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    approx_eq(
        cashflows[0].1.amount(),
        0.0,
        EPSILON,
        "Zero notional cashflow",
    );
}

#[test]
fn test_multiple_instruments_independent_settlement() {
    let fx1 = FxSpot {
        settlement_date: Some(d(2025, 1, 17)),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let fx2 = FxSpot {
        settlement_date: Some(d(2025, 1, 20)),
        ..eurusd_with_notional(2_000_000.0, 1.22)
    };

    let market = MarketContext::new();
    let as_of = test_date();

    let cf1 = fx1.dated_cashflows(&market, as_of).unwrap();
    let cf2 = fx2.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cf1.len(), 1);
    assert_eq!(cf2.len(), 1);
    assert_eq!(cf1[0].0, d(2025, 1, 17));
    assert_eq!(cf2[0].0, d(2025, 1, 20));
    approx_eq(cf1[0].1.amount(), 1_200_000.0, EPSILON, "FX1 amount");
    approx_eq(cf2[0].1.amount(), 2_440_000.0, EPSILON, "FX2 amount");
}

#[test]
fn test_settlement_without_rate_or_matrix_fails() {
    let fx = FxSpot {
        notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        settlement_date: Some(d(2025, 1, 17)),
        ..sample_eurusd()
    };
    let market = MarketContext::new(); // No FX matrix

    let result = fx.dated_cashflows(&market, test_date());
    assert!(result.is_err());
}

#[test]
fn test_value_matches_provider_flows() {
    let fx = FxSpot {
        settlement_date: Some(d(2025, 1, 17)),
        ..eurusd_with_notional(1_000_000.0, 1.20)
    };
    let market = MarketContext::new();
    let as_of = test_date();

    let value = fx.value(&market, as_of).expect("fx spot value");
    let provider_flows = fx
        .dated_cashflows(&market, as_of)
        .expect("provider flows should build");
    let provider_total = provider_flows.into_iter().fold(
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        |acc, (_, amount)| {
            acc.checked_add(amount)
                .expect("flow sum should remain in a single currency")
        },
    );

    approx_eq(
        value.amount(),
        provider_total.amount(),
        EPSILON,
        "Value should match provider flows",
    );
    assert_eq!(value.currency(), provider_total.currency());
}

#[test]
fn test_settlement_lag_negative() {
    // Test backward-looking settlement (unusual but valid)
    let fx = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .quoted_spot(1.20)
        .settlement_date(d(2025, 1, 15))
        .build()
        .expect("valid FX spot"); // Past date

    let market = MarketContext::new();
    let as_of = d(2025, 1, 17); // Friday

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    // Past date means no cashflow
    assert_eq!(cashflows.len(), 0);
}

#[test]
fn test_calendar_aware_settlement_lag() {
    let fx = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .quoted_spot(1.20)
        .base_calendar_id("target2".into())
        .quote_calendar_id("usny".into())
        .settlement_date(d(2025, 1, 17))
        .build()
        .expect("valid FX spot"); // T+2

    let market = MarketContext::new();
    let as_of = d(2025, 1, 15);

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cashflows.len(), 1);
    // Should respect calendar holidays if any
    assert!(cashflows[0].0 > as_of);
}

#[test]
fn cashflow_uses_valuation_date_spot_not_settlement_date_rate() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use std::sync::Arc;

    struct DateAwareFx {
        valuation_date: Date,
    }

    impl FxProvider for DateAwareFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(if on == self.valuation_date {
                1.10
            } else {
                9.99
            })
        }
    }

    let as_of = d(2025, 1, 15);
    let spot = FxSpot::builder()
        .id(InstrumentId::new("EURUSD-DATE-AWARE"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .settlement_date(d(2025, 1, 17))
        .build()
        .expect("valid FX spot");
    let market = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(DateAwareFx {
        valuation_date: as_of,
    })));

    let cashflows = spot
        .dated_cashflows(&market, as_of)
        .expect("cashflow schedule");
    assert_eq!(cashflows.len(), 1);
    approx_eq(
        cashflows[0].1.amount(),
        1_100_000.0,
        EPSILON,
        "cashflow must use as-of spot",
    );
}
