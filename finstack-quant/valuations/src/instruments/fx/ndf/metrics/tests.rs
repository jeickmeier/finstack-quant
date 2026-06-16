//! Quote-convention parity tests for NDF Fx01 via the shared
//! `GenericFx01Calculator`.
//!
//! Replaces the unit tests that lived in the deleted per-NDF
//! `fx01::Fx01Calculator` file. The contract is the same: an NDF's Fx01 must
//! match a finite-difference re-pricing under both quote conventions
//! (`BasePerSettlement` and `SettlementPerBase`). Because the generic
//! calculator routes through `MarketBump::FxPct` + the canonical
//! `Ndf::value` pricer, quote-convention awareness comes for free — the
//! pricer already reads spot in its own convention.

use crate::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
use crate::instruments::Attributes;
use crate::instruments::Instrument;
use crate::metrics::sensitivities::fx01::GenericFx01Calculator;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use std::sync::Arc;
use time::Month;

fn curves(as_of: Date) -> MarketContext {
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
        .build()
        .expect("usd curve");
    let cny_curve = DiscountCurve::builder("CNY-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9876), (1.0, 0.9753)])
        .build()
        .expect("cny curve");
    MarketContext::new().insert(usd_curve).insert(cny_curve)
}

fn market_with_quote(as_of: Date, from: Currency, to: Currency, rate: f64) -> MarketContext {
    let provider = Arc::new(SimpleFxProvider::new());
    provider.set_quote(from, to, rate).expect("valid rate");
    curves(as_of).insert_fx(FxMatrix::new(provider))
}

fn fx01_of(ndf: &Ndf, market: &MarketContext, as_of: Date) -> f64 {
    let base_value = ndf.value(market, as_of).expect("base value");
    let instrument: Arc<dyn Instrument> = Arc::new(ndf.clone());
    let mut context = MetricContext::new(
        instrument,
        Arc::new(market.clone()),
        as_of,
        base_value,
        MetricContext::default_config(),
    );
    GenericFx01Calculator
        .calculate(&mut context)
        .expect("fx01 calculation")
}

/// Build two market snapshots that differ only by a +1% spot bump (the
/// generic Fx01's bump direction). The finite-difference reprice through
/// these markets is what Fx01 must agree with (up to the central-difference
/// vs forward-difference convention factor).
fn finite_diff_one_pct(
    ndf: &Ndf,
    base: &MarketContext,
    bumped: &MarketContext,
    as_of: Date,
) -> f64 {
    let pv_base = ndf.value(base, as_of).expect("pv base").amount();
    let pv_bumped = ndf.value(bumped, as_of).expect("pv bumped").amount();
    pv_bumped - pv_base
}

/// Regression for the deleted per-NDF Fx01 test: a `SettlementPerBase` NDF
/// must produce a positive Fx01 (long base) and match a finite-difference
/// re-pricing through the same market path, now expressed in "$ per 1%
/// relative spot move".
#[test]
fn ndf_fx01_settlement_per_base_matches_finite_difference_one_pct() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("date");
    let fixing = Date::from_calendar_date(2024, Month::April, 13).expect("date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("date");

    // SettlementPerBase: contract_rate and spot quoted as USD per CNY.
    let spb_spot = 1.0 / 7.25;
    let base_market = market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot);
    // Forward-difference +1% bump on spot; the generic Fx01 central-differences
    // ±1% so the FD comparison is symmetric within first-order accuracy.
    let bump_up = market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot * 1.01);
    let bump_dn = market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot * 0.99);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-NDF-SPB"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(1.0 / 7.30)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .quote_convention(NdfQuoteConvention::SettlementPerBase)
        .attributes(Attributes::new())
        .build()
        .expect("ndf");

    let fx01 = fx01_of(&ndf, &base_market, as_of);
    let fd_up = finite_diff_one_pct(&ndf, &base_market, &bump_up, as_of);
    let fd_dn = finite_diff_one_pct(&ndf, &base_market, &bump_dn, as_of);
    let fd_central = (fd_up - fd_dn) / 2.0;

    assert!(
        fx01 > 0.0,
        "SettlementPerBase NDF is long base: a +bump to USD-per-CNY spot \
         must raise PV, got fx01={fx01}"
    );
    // Central-difference Fx01 must match the central FD to high precision.
    let scale = fd_central.abs().max(1.0);
    assert!(
        (fx01 - fd_central).abs() < 1e-6 * scale,
        "Fx01 must match central finite-difference: fx01={fx01}, fd_central={fd_central}"
    );
}

/// `BasePerSettlement` companion to the above. Same contract.
///
/// Sign convention: the generic `Fx01Calculator` bumps `rate(base, quote)`
/// directly via `MarketBump::FxPct`. For this NDF that's `rate(CNY, USD)`
/// (= `1 / bps_spot`). To produce a finite-difference comparison in the
/// **same direction** we set up the bumped markets quoted as `rate(CNY,
/// USD)`, not `rate(USD, CNY)` — bumping the latter by +1% goes the
/// opposite way and would flip the sign comparison.
#[test]
fn ndf_fx01_base_per_settlement_matches_finite_difference_one_pct() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("date");
    let fixing = Date::from_calendar_date(2024, Month::April, 13).expect("date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("date");

    let bps_spot = 7.25_f64;
    let base_market = market_with_quote(as_of, Currency::USD, Currency::CNY, bps_spot);
    // Bump in the same orientation the generic Fx01 uses: rate(CNY, USD).
    let inv = 1.0 / bps_spot;
    let bump_up = market_with_quote(as_of, Currency::CNY, Currency::USD, inv * 1.01);
    let bump_dn = market_with_quote(as_of, Currency::CNY, Currency::USD, inv * 0.99);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-NDF-BPS"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.30)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("ndf");

    let fx01 = fx01_of(&ndf, &base_market, as_of);
    let fd_up = finite_diff_one_pct(&ndf, &base_market, &bump_up, as_of);
    let fd_dn = finite_diff_one_pct(&ndf, &base_market, &bump_dn, as_of);
    let fd_central = (fd_up - fd_dn) / 2.0;

    let scale = fd_central.abs().max(1.0);
    assert!(
        (fx01 - fd_central).abs() < 1e-6 * scale,
        "BasePerSettlement Fx01 must match central FD: fx01={fx01}, fd_central={fd_central}"
    );
}

/// Post-fixing NDFs are not sensitive to spot — Fx01 must be exactly 0.
#[test]
fn ndf_fx01_post_fixing_is_zero() {
    let as_of = Date::from_calendar_date(2024, Month::May, 1).expect("date");
    let fixing = Date::from_calendar_date(2024, Month::April, 13).expect("date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("date");

    let spb_spot = 1.0 / 7.25;
    let market = market_with_quote(as_of, Currency::CNY, Currency::USD, spb_spot);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-NDF-POST"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(1.0 / 7.30)
        .fixing_rate_opt(Some(1.0 / 7.28))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .quote_convention(NdfQuoteConvention::SettlementPerBase)
        .attributes(Attributes::new())
        .build()
        .expect("ndf");

    let fx01 = fx01_of(&ndf, &market, as_of);
    assert_eq!(fx01, 0.0, "post-fixing NDF must have zero Fx01, got {fx01}");
}
