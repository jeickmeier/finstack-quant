//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::market::build_rate_instrument;
use finstack_quant_valuations::market::BuildCtx;
use rust_decimal::Decimal;

use finstack_quant_valuations::market::quotes::ids::Pillar;
use finstack_quant_valuations::market::quotes::rates::RateQuote; // Used in code if needed, or string into

fn usd_build_ctx(as_of: Date) -> BuildCtx {
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("forward".to_string(), "USD-SOFR".to_string());
    BuildCtx::new(as_of, 1_000_000.0, curve_ids)
}

fn eur_build_ctx(as_of: Date) -> BuildCtx {
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("discount".to_string(), "EUR-OIS".to_string());
    curve_ids.insert("forward".to_string(), "EUR-EURIBOR-3M".to_string());
    BuildCtx::new(as_of, 1_000_000.0, curve_ids)
}

#[test]
fn test_build_deposit() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = usd_build_ctx(as_of);

    let quote = RateQuote::Deposit {
        id: "USD-DEP-3M".into(),
        index: "USD-LIBOR-3M".into(),
        pillar: Pillar::Tenor(Tenor::parse("3M").unwrap()),
        rate: 0.035,
    };

    let instrument = build_rate_instrument(&quote, &ctx).expect("build deposit");
    assert_eq!(instrument.id(), "USD-DEP-3M");

    if let Some(dep) = instrument.as_any().downcast_ref::<Deposit>() {
        assert_eq!(dep.notional.currency(), Currency::USD);
        assert_eq!(
            dep.quote_rate,
            Some(Decimal::try_from(0.035).expect("valid decimal"))
        );
    } else {
        panic!("Expected Deposit");
    }
}

#[test]
fn test_build_fra() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = usd_build_ctx(as_of);

    let quote = RateQuote::Fra {
        id: "USD-FRA-3x6".into(),
        index: "USD-LIBOR-3M".into(), // Use 3M conventions for FRA
        start: Pillar::Tenor(Tenor::parse("3M").unwrap()),
        end: Pillar::Tenor(Tenor::parse("6M").unwrap()),
        rate: 0.032,
    };

    let instrument = build_rate_instrument(&quote, &ctx).expect("build fra");
    assert_eq!(instrument.id(), "USD-FRA-3x6");

    if let Some(fra) = instrument.as_any().downcast_ref::<ForwardRateAgreement>() {
        assert_eq!(fra.notional.currency(), Currency::USD);
        assert_eq!(
            fra.fixed_rate,
            Decimal::try_from(0.032).expect("valid decimal")
        );
    } else {
        panic!("Expected ForwardRateAgreement");
    }
}

#[test]
fn test_build_swap() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = usd_build_ctx(as_of);

    // Use USD-SOFR-3M (Term SOFR style or OIS compounded, RateIndexKind=Term in registry will determine builder path)
    // If it's OIS (OvernightRfr), builder uses compounded-in-arrears conventions.
    // If Term, builder uses simple compounding conventions.
    // Let's assume correct behavior.
    let quote = RateQuote::Swap {
        id: "USD-SWAP-5Y".into(),
        index: "USD-SOFR-3M".into(),
        pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
        rate: 0.030,
        spread_decimal: None,
    };

    let instrument = build_rate_instrument(&quote, &ctx).expect("build swap");
    assert_eq!(instrument.id(), "USD-SWAP-5Y");

    if let Some(swap) = instrument.as_any().downcast_ref::<InterestRateSwap>() {
        assert_eq!(swap.notional.currency(), Currency::USD);
        assert_eq!(swap.fixed.rate, Decimal::try_from(0.030).expect("valid"));
    } else {
        panic!("Expected InterestRateSwap");
    }
}

#[test]
fn test_build_futures() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = usd_build_ctx(as_of);

    let quote = RateQuote::Futures {
        id: "USD-FUT-SEP25".into(),
        contract: "SR3".into(),
        expiry: Date::from_calendar_date(2025, time::Month::September, 15).unwrap(),
        price: 96.50,
        convexity_adjustment: None,
        vol_surface_id: None,
    };

    let instrument = build_rate_instrument(&quote, &ctx).expect("build futures");
    assert_eq!(instrument.id(), "USD-FUT-SEP25");

    if instrument
        .as_any()
        .downcast_ref::<finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture>()
        .is_none()
    {
        panic!("Expected InterestRateFuture");
    }
}

#[test]
fn test_build_euribor_futures() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = eur_build_ctx(as_of);

    let quote = RateQuote::Futures {
        id: "EUR-FUT-SEP25".into(),
        contract: "ICE:ER".into(),
        expiry: Date::from_calendar_date(2025, time::Month::September, 15).unwrap(),
        price: 97.15,
        convexity_adjustment: None,
        vol_surface_id: None,
    };

    let instrument = build_rate_instrument(&quote, &ctx).expect("build euribor futures");
    assert_eq!(instrument.id(), "EUR-FUT-SEP25");

    let future = instrument
        .as_any()
        .downcast_ref::<finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture>()
        .expect("Expected InterestRateFuture");
    assert_eq!(future.notional.currency(), Currency::EUR);
    assert_eq!(
        future.day_count,
        finstack_quant_core::dates::DayCount::Act360
    );
}
