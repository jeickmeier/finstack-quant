//! Bond quote engine round-trip tests for price / yield / spread metrics.
//!
//! These tests verify that using a metric as the *input* to the quote
//! engine (YTM, Z-spread, DM, OAS, ASW Market, I-Spread) produces a
//! price that, when fed back through the standard metrics pipeline,
//! recovers the original metric within tight tolerances.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    compute_quotes, BondQuoteInput,
};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_simple_discount_curve(as_of: time::Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve builder should succeed in test")
}

#[test]
fn test_quote_engine_roundtrip_ytm_and_zspread_fixed_bond() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "QE-FIXED",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let disc = build_simple_discount_curve(as_of);
    let market = MarketContext::new().insert(disc);

    // YTM → price → YTM
    let target_ytm = 0.045;
    let quotes_from_ytm =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::Ytm(target_ytm)).unwrap();
    let clean_pct = quotes_from_ytm.clean_price_pct;

    // Feed the resulting clean price back into the standard metrics pipeline.
    let mut bond_with_price = bond.clone();
    bond_with_price
        .pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(clean_pct);
    let res = bond_with_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ytm_metric = *res.measures.get("ytm").unwrap();

    // YTM round-trip tolerance: 1 bp = 1e-4 is reasonable for iterative solvers
    assert!(
        (ytm_metric - target_ytm).abs() < 1e-4,
        "YTM round-trip mismatch: target={}, metric={}",
        target_ytm,
        ytm_metric,
    );

    // Z-spread → price → Z-spread
    let target_z = 0.0123;
    let quotes_from_z =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::ZSpread(target_z)).unwrap();
    let clean_pct_z = quotes_from_z.clean_price_pct;

    let mut bond_with_price_z = bond;
    bond_with_price_z
        .pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(clean_pct_z);
    let res_z = bond_with_price_z
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let z_metric = *res_z.measures.get("z_spread").unwrap();

    // Z-spread round-trip tolerance: 1 bp = 1e-4 is reasonable for iterative solvers
    assert!(
        (z_metric - target_z).abs() < 1e-4,
        "Z-spread round-trip mismatch: target={}, metric={}",
        target_z,
        z_metric,
    );
}

#[test]
fn test_quote_engine_roundtrip_dm_for_frn() {
    use finstack_quant_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Simple FRN: 3M SOFR + 150bp
    let frn = Bond::floating(
        "QE-FRN",
        notional,
        "USD-SOFR-3M",
        150,
        as_of,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Flat discount and forward curves.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (10.0, 0.03)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc).insert(fwd);

    let target_dm = 0.01; // 100bp
    let quotes = compute_quotes(
        &frn,
        &market,
        as_of,
        BondQuoteInput::DiscountMargin(target_dm),
    )
    .unwrap();
    let clean_pct = quotes.clean_price_pct;

    let mut frn_with_price = frn;
    frn_with_price.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(clean_pct);

    let res = frn_with_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dm_metric = *res.measures.get("discount_margin").unwrap();

    assert!(
        (dm_metric - target_dm).abs() < 5e-8,
        "DM round-trip mismatch: target={}, metric={}",
        target_dm,
        dm_metric,
    );
}

#[test]
fn test_quote_engine_roundtrip_oas_and_asw_market_fixed_bond() {
    use finstack_quant_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "QE-OAS-ASW",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    // OAS calculations use short-rate tree which needs a curve with more knots
    // for stable calibration
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97),
            (2.0, 0.94),
            (3.0, 0.91),
            (5.0, 0.85),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve builder should succeed");
    let market = MarketContext::new().insert(disc);

    // OAS → price → OAS
    let target_oas = 0.01; // 100bp in decimal
    let quotes_oas =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::Oas(target_oas)).unwrap();
    let clean_pct_oas = quotes_oas.clean_price_pct;

    let mut bond_with_oas_price = bond.clone();
    bond_with_oas_price.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(clean_pct_oas);
    let res_oas = bond_with_oas_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let oas_metric = *res_oas.measures.get("oas").unwrap();

    // OAS round-trip tolerance: 10 bp = 1e-3 for tree-based pricing
    assert!(
        (oas_metric - target_oas).abs() < 1e-3,
        "OAS round-trip mismatch: target={}, metric={}",
        target_oas,
        oas_metric,
    );

    // ASW Market → price → ASW Market
    let target_asw_mkt = 0.005; // 50bp
    let quotes_asw = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::AswMarket(target_asw_mkt),
    )
    .unwrap();
    let clean_pct_asw = quotes_asw.clean_price_pct;

    let mut bond_with_asw_price = bond;
    bond_with_asw_price.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(clean_pct_asw);
    let res_asw = bond_with_asw_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let asw_metric = *res_asw.measures.get("asw_market").unwrap();

    // ASW round-trip tolerance: 1 bp = 1e-4 for iterative solvers
    assert!(
        (asw_metric - target_asw_mkt).abs() < 1e-4,
        "ASW Market round-trip mismatch: target={}, metric={}",
        target_asw_mkt,
        asw_metric,
    );
}

#[test]
fn test_quote_engine_roundtrip_i_spread_fixed_bond() {
    use finstack_quant_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "QE-ISPR",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let disc = build_simple_discount_curve(as_of);
    let market = MarketContext::new().insert(disc);

    let target_ispr = 0.0075; // 75bp
    let quotes =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::ISpread(target_ispr)).unwrap();
    let clean_pct = quotes.clean_price_pct;

    let mut bond_with_price = bond;
    bond_with_price.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(clean_pct);
    let res = bond_with_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ISpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let ispr_metric = *res.measures.get("i_spread").unwrap();

    // I-spread round-trip tolerance: 1 bp = 1e-4 is reasonable for iterative solvers
    assert!(
        (ispr_metric - target_ispr).abs() < 1e-4,
        "I-spread round-trip mismatch: target={}, metric={}",
        target_ispr,
        ispr_metric,
    );
}

#[test]
fn test_quote_engine_asw_market_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;
    let bond = Bond::fixed(
        "QE-ASW-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc = build_simple_discount_curve(issue);
    let market = MarketContext::new().insert(disc);

    let err = compute_quotes(&bond, &market, as_of, BondQuoteInput::AswMarket(0.005))
        .expect_err("ASW market inversion should fail for matured schedule");
    let msg = format!("{err}");
    assert!(
        msg.contains("ASW market price inversion requires at least two fixed-leg schedule dates"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_quote_engine_spread_and_yield_paths_reprice_to_same_clean_price() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "QE-SPREAD-INVARIANTS",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let disc = build_simple_discount_curve(as_of);
    let market = MarketContext::new().insert(disc);

    // Start from a market clean-price quote and infer all spread/yield views.
    let base_clean_pct = 99.25;
    let base = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::CleanPricePct(base_clean_pct),
    )
    .expect("base quote set");

    let ytm = base.ytm.expect("ytm should be available");
    let z = base.z_spread.expect("z-spread should be available");
    let i = base.i_spread.expect("i-spread should be available");

    let from_ytm =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::Ytm(ytm)).expect("ytm repricing");
    let from_z =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::ZSpread(z)).expect("z repricing");
    let from_i =
        compute_quotes(&bond, &market, as_of, BondQuoteInput::ISpread(i)).expect("i repricing");

    // Tight for YTM and Z-spread, looser for I-spread due to proxy par-swap approximation.
    assert!(
        (from_ytm.clean_price_pct - base_clean_pct).abs() < 5e-3,
        "YTM repricing mismatch: base={}, from_ytm={}",
        base_clean_pct,
        from_ytm.clean_price_pct
    );
    assert!(
        (from_z.clean_price_pct - base_clean_pct).abs() < 1e-4,
        "Z-spread repricing mismatch: base={}, from_z={}",
        base_clean_pct,
        from_z.clean_price_pct
    );
    assert!(
        (from_i.clean_price_pct - base_clean_pct).abs() < 2e-2,
        "I-spread repricing mismatch: base={}, from_i={}",
        base_clean_pct,
        from_i.clean_price_pct
    );
}

#[test]
fn test_quote_engine_i_spread_rejects_matured_schedule() {
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let as_of = maturity;
    let bond = Bond::fixed(
        "QE-ISPR-MATURED",
        Money::new(100.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let disc = build_simple_discount_curve(issue);
    let market = MarketContext::new().insert(disc);

    let err = compute_quotes(&bond, &market, as_of, BondQuoteInput::ISpread(0.005))
        .expect_err("I-spread quote inversion should fail for matured schedule");
    let msg = format!("{err}");
    assert!(
        msg.contains("Invalid date range"),
        "unexpected error: {msg}"
    );
}
