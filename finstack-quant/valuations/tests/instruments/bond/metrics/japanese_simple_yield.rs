//! Japanese simple yield (単利) metric and quote-engine tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    compute_quotes, BondQuoteInput,
};
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::{
    BondConvention, Instrument, InstrumentPricingOverrides,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use time::macros::date;

fn flat_jpy_market(as_of: time::Date) -> MarketContext {
    let curve = DiscountCurve::builder("JPY-JGB")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.90)])
        .build()
        .expect("JPY discount curve");
    MarketContext::new().insert(curve)
}

/// ACT/365F remaining life is exactly two years on this date pair (non-leap).
fn two_year_jgb_style_bond() -> Bond {
    Bond::builder()
        .id("JGB-SIMPLE".into())
        .notional(Money::new(100.0, Currency::JPY).expect("valid money fixture"))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2027 - 01 - 01))
        .cashflow_spec(
            CashflowSpec::fixed(0.02, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite coupon"),
        )
        .discount_curve_id("JPY-JGB".into())
        .build()
        .expect("JGB-style bullet")
}

fn japanese_simple_yield(bond: &Bond, market: &MarketContext, as_of: time::Date) -> f64 {
    *bond
        .price_with_metrics(
            market,
            as_of,
            &[MetricId::JapaneseSimpleYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("japanese_simple_yield")
        .measures
        .get("japanese_simple_yield")
        .expect("metric present")
}

#[test]
fn japanese_simple_yield_at_par_equals_coupon() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = two_year_jgb_style_bond();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
    let market = flat_jpy_market(as_of);

    let y = japanese_simple_yield(&bond, &market, as_of);
    assert!(
        (y - 0.02).abs() < 1e-12,
        "par 2y 2% simple yield should be 2%, got {y}"
    );
}

#[test]
fn japanese_simple_yield_matches_closed_form_discount() {
    let as_of = date!(2025 - 01 - 01);
    let coupon = 0.02;
    let n = 2.0;
    let y = 0.03;
    let dirty_pct = 100.0 * (1.0 + coupon * n) / (1.0 + y * n);

    let mut bond = two_year_jgb_style_bond();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(dirty_pct);
    let market = flat_jpy_market(as_of);

    let solved = japanese_simple_yield(&bond, &market, as_of);
    assert!(
        (solved - y).abs() < 1e-12,
        "closed-form discount invert: expected {y}, got {solved}"
    );
}

#[test]
fn japanese_simple_yield_matches_closed_form_premium() {
    let as_of = date!(2025 - 01 - 01);
    let coupon = 0.02;
    let n = 2.0;
    let y = 0.01;
    let dirty_pct = 100.0 * (1.0 + coupon * n) / (1.0 + y * n);

    let mut bond = two_year_jgb_style_bond();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(dirty_pct);
    let market = flat_jpy_market(as_of);

    let solved = japanese_simple_yield(&bond, &market, as_of);
    assert!(
        (solved - y).abs() < 1e-12,
        "closed-form premium invert: expected {y}, got {solved}"
    );
}

#[test]
fn quote_engine_seeds_from_japanese_simple_yield_without_street_ytm() {
    let as_of = date!(2025 - 01 - 01);
    let bond = two_year_jgb_style_bond();
    let market = flat_jpy_market(as_of);
    let target = 0.03;

    let quotes = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::JapaneseSimpleYield(target),
        finstack_quant_valuations::instruments::PricingOptions::default()
            .with_model(ModelKey::Discounting),
    )
    .expect("quote engine");

    let expected_dirty_pct = 100.0 * (1.0 + 0.02 * 2.0) / (1.0 + target * 2.0);
    assert!(
        (quotes.dirty_price_currency - expected_dirty_pct).abs() < 1e-10,
        "dirty {} should match closed form {expected_dirty_pct}",
        quotes.dirty_price_currency
    );
    assert!(
        (quotes.japanese_simple_yield.expect("simple yield") - target).abs() < 1e-12,
        "quote set should echo the Tokyo simple yield"
    );

    let mut priced = bond;
    priced.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(quotes.clean_price_pct);
    let recovered = japanese_simple_yield(&priced, &market, as_of);
    assert!(
        (recovered - target).abs() < 1e-12,
        "price → simple-yield round-trip: expected {target}, got {recovered}"
    );
}

#[test]
fn jgb_street_ytm_is_unchanged_and_distinct_from_simple_yield() {
    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::with_convention(
        "JGB-STREET",
        Money::new(100_000_000.0, Currency::JPY).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.02).expect("valid rate fixture"),
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        BondConvention::Jgb,
        "JPY-JGB",
    )
    .expect("JGB");
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(98.5);
    let market = flat_jpy_market(as_of);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::JapaneseSimpleYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metrics");
    let street = *result.measures.get("ytm").expect("street ytm");
    let simple = *result
        .measures
        .get("japanese_simple_yield")
        .expect("simple yield");

    assert!(street.is_finite(), "Street JGB ytm must remain solvable");
    assert!(simple.is_finite());
    assert!(
        (street - simple).abs() > 1e-6,
        "mid-life JGB Street ytm ({street}) must stay distinct from 単利 ({simple})"
    );
}

#[test]
fn japanese_simple_yield_rejects_non_positive_remaining_life() {
    let as_of = date!(2027 - 01 - 01);
    let mut bond = two_year_jgb_style_bond();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
    let market = flat_jpy_market(date!(2025 - 01 - 01));

    let err = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JapaneseSimpleYield],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect_err("matured bond must reject n <= 0");
    assert!(
        err.to_string().to_lowercase().contains("invalid")
            || err.to_string().to_lowercase().contains("life")
            || err.to_string().to_lowercase().contains("maturity"),
        "unexpected error: {err}"
    );
}

#[test]
fn quote_engine_japanese_simple_yield_rejects_frn() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    let as_of = date!(2025 - 01 - 01);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(date!(2024 - 12 - 30))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (2.0, 0.035)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc).insert(fwd);
    let bond = Bond::floating(
        "FRN-SIMPLE",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        "USD-SOFR-3M",
        150,
        as_of,
        date!(2027 - 01 - 01),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    let err = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::JapaneseSimpleYield(0.02),
        finstack_quant_valuations::instruments::PricingOptions::default()
            .with_model(ModelKey::Discounting),
    )
    .expect_err("FRN is not a bullet fixed-rate quote");
    assert!(
        err.to_string().to_lowercase().contains("invalid")
            || err.to_string().to_lowercase().contains("fixed"),
        "unexpected error: {err}"
    );
}
