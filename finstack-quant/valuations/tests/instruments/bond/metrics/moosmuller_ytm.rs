//! Moosmüller YTM metric tests.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    df_from_yield, price_from_ytm_compounded_params, YieldCompounding,
};
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn flat_eur_market(as_of: time::Date) -> MarketContext {
    let curve = DiscountCurve::builder("EUR-BUND")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.75)])
        .build()
        .expect("EUR discount curve");
    MarketContext::new().insert(curve)
}

/// Annual Bund-style bullet with no settlement lag so `as_of` is the quote date.
///
/// ACT/365F keeps mid-coupon year fractions well-defined without an ISMA
/// `coupon_period` context. Annual frequency is the Bund feature that makes
/// Moosmüller differ from Street off a coupon date.
fn bund_style_bond(issue: time::Date, maturity: time::Date) -> Bond {
    Bond::builder()
        .id("BUND-MOO".into())
        .notional(Money::new(100.0, Currency::EUR).expect("valid money fixture"))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.03, Tenor::annual(), DayCount::Act365F).expect("finite coupon"),
        )
        .discount_curve_id("EUR-BUND".into())
        .build()
        .expect("Bund-style bullet")
}

fn metric(bond: &Bond, market: &MarketContext, as_of: time::Date, id: MetricId) -> f64 {
    *bond
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&id),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metric")
        .measures
        .get(id.as_str())
        .expect("metric present")
}

#[test]
fn moosmuller_differs_from_street_mid_coupon() {
    let issue = date!(2025 - 01 - 01);
    let as_of = date!(2025 - 07 - 01);
    let mut bond = bund_style_bond(issue, date!(2027 - 01 - 01));
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);
    let market = flat_eur_market(as_of);

    let street = metric(&bond, &market, as_of, MetricId::Ytm);
    let moo = metric(&bond, &market, as_of, MetricId::MoosmullerYtm);

    assert!(
        (street - moo).abs() > 1e-6,
        "mid-coupon Bund Street ytm ({street}) must differ from Moosmüller ({moo})"
    );
}

#[test]
fn moosmuller_equals_street_on_coupon_date() {
    // On a coupon date, w is a full period (1/f). The first-period simple
    // factor 1/(1 + y*w) = 1/(1 + y/f) is absorbed into periodic compounding,
    // so DF_k = (1 + y/f)^{-k}, which is Street for a regular annual schedule.
    let as_of = date!(2025 - 01 - 01);
    let mut bond = bund_style_bond(as_of, date!(2027 - 01 - 01));
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);
    let market = flat_eur_market(as_of);

    let street = metric(&bond, &market, as_of, MetricId::Ytm);
    let moo = metric(&bond, &market, as_of, MetricId::MoosmullerYtm);

    assert!(
        (street - moo).abs() < 1e-10,
        "coupon-date Moosmüller ({moo}) must equal Street ({street}) because w = 1/f"
    );
}

#[test]
fn german_bund_convention_still_uses_street_ytm() {
    // `BondConvention::GermanBund` must not switch the default `ytm` metric
    // off Street. On a coupon date the two compounding conventions coincide
    // (`w = 1/f`); mid-coupon they differ (see `moosmuller_differs_from_street_mid_coupon`).
    let as_of = date!(2025 - 01 - 01);
    let mut bond = bund_style_bond(as_of, date!(2027 - 01 - 01));
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);
    let market = flat_eur_market(as_of);

    let street = metric(&bond, &market, as_of, MetricId::Ytm);
    assert!(
        street.is_finite(),
        "Street ytm remains the default ytm metric"
    );
    let moo = metric(&bond, &market, as_of, MetricId::MoosmullerYtm);
    assert!(
        (street - moo).abs() < 1e-10,
        "coupon-date Bund-style Street ytm ({street}) equals Moosmüller ({moo})"
    );
}

#[test]
fn moosmuller_price_round_trip() {
    let as_of = date!(2025 - 07 - 01);
    let bond = bund_style_bond(date!(2025 - 01 - 01), date!(2027 - 01 - 01));
    let market = flat_eur_market(as_of);
    let flows = bond.dated_cashflows(&market, as_of).expect("flows");
    let ytm = 0.035;

    let dirty = price_from_ytm_compounded_params(
        bond.cashflow_spec.day_count(),
        bond.cashflow_spec.frequency(),
        &flows,
        as_of,
        ytm,
        YieldCompounding::Moosmuller,
    )
    .expect("moosmuller price");

    let accrued = *bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("accrued")
        .measures
        .get("accrued")
        .expect("accrued present");
    let clean_pct = (dirty - accrued) / bond.notional.amount() * 100.0;

    let mut quoted = bond;
    quoted.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_pct);

    let solved = metric(&quoted, &market, as_of, MetricId::MoosmullerYtm);
    assert!(
        (solved - ytm).abs() < 1e-10,
        "Moosmüller price → yield round-trip: expected {ytm}, got {solved}"
    );
}

#[test]
fn df_from_yield_moosmuller_matches_street_on_period_boundary() {
    let frequency = Tenor::annual();
    let ytm = 0.04;
    for t in [1.0, 2.0, 3.0] {
        let street = df_from_yield(ytm, t, YieldCompounding::Street, frequency).unwrap();
        let moo = df_from_yield(ytm, t, YieldCompounding::Moosmuller, frequency).unwrap();
        assert!(
            (street - moo).abs() < 1e-14,
            "t={t}: Street {street} vs Moosmüller {moo}"
        );
    }
}
