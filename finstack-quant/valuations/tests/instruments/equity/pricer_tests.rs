//! Equity pricer tests

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::spot::EquityPricer;
use finstack_quant_valuations::instruments::equity::Equity;
use finstack_quant_valuations::instruments::pricing::GenericInstrumentPricer;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::{InstrumentType, ModelKey, Pricer};

fn build_flat_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    let mut builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.25, (-rate * 0.25).exp()),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
        ]);

    // For zero or negative rates, the curve may be flat or increasing
    // which requires allow_non_monotonic()
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
fn test_equity_pricer_with_price_quote() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_shares(100.0)
        .with_price(150.0);

    let market = MarketContext::new();
    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    // Price per share should return the quote
    let price_per_share = pricer.price_per_share(&equity, &market, as_of).unwrap();
    assert_eq!(price_per_share.amount(), 150.0);
    assert_eq!(price_per_share.currency(), Currency::USD);

    // PV should be shares * price
    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), 15_000.0);
    assert_eq!(pv.currency(), Currency::USD);
}

#[test]
fn test_equity_pricer_from_market_data() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_shares(50.0);

    let market = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::Price(Money::new(200.0, Currency::USD)),
    );

    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    // Should resolve from market data
    let price_per_share = pricer.price_per_share(&equity, &market, as_of).unwrap();
    assert_eq!(price_per_share.amount(), 200.0);

    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), 10_000.0);
}

#[test]
fn test_equity_pricer_with_custom_price_id() {
    let equity = Equity::new("EQUITY1", "AAPL", Currency::USD)
        .with_shares(25.0)
        .with_price_id("CUSTOM_PRICE_ID");

    let market = MarketContext::new().insert_price(
        "CUSTOM_PRICE_ID",
        MarketScalar::Price(Money::new(180.0, Currency::USD)),
    );

    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let price_per_share = pricer.price_per_share(&equity, &market, as_of).unwrap();
    assert_eq!(price_per_share.amount(), 180.0);

    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), 4_500.0);
}

#[test]
fn test_equity_dividend_yield_default() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    let market = MarketContext::new();
    let pricer = EquityPricer;

    // Should default to 0.0 when not present
    let div_yield = pricer.dividend_yield(&equity, &market).unwrap();
    assert_eq!(div_yield, 0.0);
}

#[test]
fn test_equity_dividend_yield_from_market() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    let market = MarketContext::new().insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.025)); // 2.5% dividend yield

    let pricer = EquityPricer;
    let div_yield = pricer.dividend_yield(&equity, &market).unwrap();
    assert_eq!(div_yield, 0.025);
}

#[test]
fn test_equity_dividend_yield_with_custom_id() {
    let equity =
        Equity::new("EQUITY1", "AAPL", Currency::USD).with_dividend_yield_id("CUSTOM_DIV_YIELD");

    let market =
        MarketContext::new().insert_price("CUSTOM_DIV_YIELD", MarketScalar::Unitless(0.03)); // 3% dividend yield

    let pricer = EquityPricer;
    let div_yield = pricer.dividend_yield(&equity, &market).unwrap();
    assert_eq!(div_yield, 0.03);
}

#[test]
fn test_equity_forward_price() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_price(100.0);

    // Add discount curve (5% interest rate)
    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = build_flat_curve(0.05, base_date, "USD");

    let market = MarketContext::new()
        .insert(curve)
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02));

    let pricer = EquityPricer;

    // Forward price at t=1 year: S0 * exp((r - q) * t) = 100 * exp((0.05 - 0.02) * 1)
    let forward_price = pricer
        .forward_price_per_share(&equity, &market, base_date, 1.0)
        .unwrap();
    let expected = 100.0 * (0.03_f64).exp(); // ~103.05
    assert!((forward_price.amount() - expected).abs() < 0.01);
}

#[test]
fn test_equity_forward_value() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_price(100.0)
        .with_shares(10.0);

    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = build_flat_curve(0.04, base_date, "USD");
    let market = MarketContext::new().insert(curve);

    let pricer = EquityPricer;

    // Forward value should be forward_price_per_share * shares
    let forward_value = pricer
        .forward_value(&equity, &market, base_date, 1.0)
        .unwrap();
    let expected = 100.0 * (0.04_f64).exp() * 10.0; // ~104.08 * 10 = 1040.8
    assert!((forward_value.amount() - expected).abs() < 0.1);
}

#[test]
fn test_simple_equity_pricer_key() {
    let pricer = GenericInstrumentPricer::<Equity>::discounting(InstrumentType::Equity);
    let key = pricer.key();

    assert_eq!(key.instrument, InstrumentType::Equity);
    assert_eq!(key.model, ModelKey::Discounting);
}

#[test]
fn test_simple_equity_pricer_price_dyn() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_shares(100.0)
        .with_price(150.0);

    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = build_flat_curve(0.05, base_date, "USD");
    let market = MarketContext::new().insert(curve);

    let pricer = GenericInstrumentPricer::<Equity>::discounting(InstrumentType::Equity);
    let instrument: &dyn Instrument = &equity;
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    let result = pricer.price_dyn(instrument, &market, as_of).unwrap();

    // Should return PV
    assert_eq!(result.value.amount(), 15_000.0);
    assert_eq!(result.value.currency(), Currency::USD);
    assert_eq!(result.instrument_id.as_str(), "AAPL");
}

#[test]
fn test_simple_equity_pricer_type_mismatch() {
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

    // Create a bond (wrong type)
    let issue = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2029, time::Month::January, 1).unwrap();
    let bond = Bond::fixed(
        "BOND",
        Money::new(1000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-TREASURY",
    )
    .unwrap();

    let market = MarketContext::new();
    let pricer = GenericInstrumentPricer::<Equity>::discounting(InstrumentType::Equity);
    let instrument: &dyn Instrument = &bond;
    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .unwrap();

    // Should fail with type mismatch
    let result = pricer.price_dyn(instrument, &market, as_of);
    assert!(result.is_err());
}

#[test]
fn test_equity_effective_shares_default() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD);
    assert_eq!(equity.effective_shares(), 1.0);
}

#[test]
fn test_equity_effective_shares_explicit() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_shares(50.0);
    assert_eq!(equity.effective_shares(), 50.0);
}

#[test]
fn test_equity_pricer_zero_shares() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_shares(0.0)
        .with_price(150.0);

    let market = MarketContext::new();
    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_equity_pricer_negative_shares() {
    // Short position
    let equity = Equity::new("AAPL", "AAPL", Currency::USD)
        .with_shares(-10.0)
        .with_price(150.0);

    let market = MarketContext::new();
    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), -1_500.0);
}

#[test]
fn test_equity_pricer_different_currencies() {
    // EUR equity
    let equity = Equity::new("SAP", "SAP", Currency::EUR)
        .with_shares(20.0)
        .with_price(120.0);

    let market = MarketContext::new();
    let pricer = EquityPricer;
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();

    let pv = pricer.pv(&equity, &market, as_of).unwrap();
    assert_eq!(pv.amount(), 2_400.0);
    assert_eq!(pv.currency(), Currency::EUR);
}

#[test]
fn test_equity_forward_price_zero_rates() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_price(100.0);

    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = build_flat_curve(0.0, base_date, "USD");
    let market = MarketContext::new().insert(curve);

    let pricer = EquityPricer;

    // With zero rates and no dividend, forward = spot
    let forward_price = pricer
        .forward_price_per_share(&equity, &market, base_date, 1.0)
        .unwrap();
    assert!((forward_price.amount() - 100.0).abs() < 0.01);
}

#[test]
fn test_equity_forward_price_high_dividend() {
    let equity = Equity::new("AAPL", "AAPL", Currency::USD).with_price(100.0);

    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let curve = build_flat_curve(0.05, base_date, "USD");

    // High dividend yield (10%) exceeds risk-free rate
    let market = MarketContext::new()
        .insert(curve)
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.10));

    let pricer = EquityPricer;

    // Forward should be lower than spot when div yield > r
    let forward_price = pricer
        .forward_price_per_share(&equity, &market, base_date, 1.0)
        .unwrap();
    assert!(forward_price.amount() < 100.0);

    let expected = 100.0 * (-0.05_f64).exp(); // ~95.12
    assert!((forward_price.amount() - expected).abs() < 0.01);
}

#[test]
fn test_equity_forward_price_with_discrete_dividend() {
    let base_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let mut equity = Equity::new("AAPL", "AAPL", Currency::USD).with_price(100.0);
    equity.discrete_dividends = vec![(
        Date::from_calendar_date(2024, time::Month::April, 1).unwrap(),
        2.50,
    )];

    let curve = build_flat_curve(0.05, base_date, "USD");
    let market = MarketContext::new().insert(curve);
    let pricer = EquityPricer;

    let forward_price = pricer
        .forward_price_per_share(&equity, &market, base_date, 1.0)
        .unwrap();

    let expected = (100.0 - 2.50 * (-0.05_f64 * 0.25).exp()) * (0.05_f64).exp();
    assert!(
        (forward_price.amount() - expected).abs() < 0.02,
        "forward={} expected={}",
        forward_price.amount(),
        expected
    );
}
