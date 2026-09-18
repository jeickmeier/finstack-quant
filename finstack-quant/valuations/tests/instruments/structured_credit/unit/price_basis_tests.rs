//! Price basis: every structured-credit price, quote and price-denominated
//! metric is per CURRENT face (factor-adjusted), the secondary-market
//! convention. The 2026-09-15 audit found them per ORIGINAL face, so a
//! tranche paid down to factor 0.5 printed a price near 50 and a desk quote
//! of 99.0 solved a z-spread of −1271 bp.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_metrics, scenario_table, AssetPool, DealType, PoolAsset, ScenarioGrid,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Two-class CLO closed 2022-01-01, valued 2024-01-01. `seasoned` halves the
/// Class A balance and the pool (A has paid down to factor 0.5).
fn fixture(seasoned: bool) -> (StructuredCredit, Date) {
    let close = d(2022, 1, 1);
    let as_of = d(2024, 1, 1);
    let maturity = d(2030, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "L1",
        usd(if seasoned {
            60_000_000.0
        } else {
            100_000_000.0
        }),
        0.05,
        maturity,
        DayCount::Act360,
    ));
    let mut a = Tranche::new(
        "A",
        0.0,
        80.0,
        TrancheSeniority::Senior,
        usd(80_000_000.0),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity,
    )
    .expect("tranche");
    if seasoned {
        a.current_balance = usd(40_000_000.0);
    }
    let eq = Tranche::new(
        "EQ",
        80.0,
        100.0,
        TrancheSeniority::Equity,
        usd(20_000_000.0),
        TrancheCoupon::Fixed { rate: 0.0 },
        maturity,
    )
    .expect("tranche");
    let tranches = TrancheStructure::new(vec![a, eq]).expect("structure");
    let mut deal = StructuredCredit::new_clo(
        "CLO-PRICE-BASIS",
        pool,
        tranches,
        close,
        maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 12);
    (deal, as_of)
}

#[test]
fn seasoned_tranche_price_is_per_current_face() {
    let (deal, as_of) = fixture(true);
    let market = market(as_of);
    let metrics = calculate_tranche_metrics(&deal, "A", &market, as_of, None).expect("metrics");
    let new_issue =
        calculate_tranche_metrics(&fixture(false).0, "A", &market, as_of, None).expect("metrics");

    // Same coupon, curve and schedule: the note at factor 0.5 has half the PV
    // of the unamortized note and the SAME clean price per current face.
    assert!(
        (metrics.pv / new_issue.pv - 0.5).abs() < 1e-9,
        "PV scales with the balance: seasoned {} vs new issue {}",
        metrics.pv,
        new_issue.pv
    );
    assert!(
        (metrics.price_pct - new_issue.price_pct).abs() < 1e-6,
        "price_pct must be per current face: seasoned {} vs new issue {}",
        metrics.price_pct,
        new_issue.price_pct
    );
    assert!(
        metrics.price_pct > 95.0 && metrics.price_pct < 110.0,
        "a par-coupon senior note at factor 0.5 prices near par, got {}",
        metrics.price_pct
    );
    assert!(
        (metrics.factor - 0.5).abs() < 1e-12,
        "factor = current / original, got {}",
        metrics.factor
    );
    assert!(
        (new_issue.factor - 1.0).abs() < 1e-12,
        "an unamortized note has factor 1"
    );
    assert!(
        (metrics.target_price_pct - metrics.price_pct).abs() < 1e-9,
        "with no quote the target is the model price on the same basis"
    );
}

#[test]
fn desk_quote_per_current_face_solves_a_sane_spread() {
    let (deal, as_of) = fixture(true);
    let market = market(as_of);
    let quoted =
        calculate_tranche_metrics(&deal, "A", &market, as_of, Some(99.0)).expect("metrics");
    assert!(
        (quoted.target_price_pct - 99.0).abs() < 1e-9,
        "the quote is echoed as the target on the current-face basis"
    );
    assert!(
        quoted.z_spread_bp.abs() < 150.0,
        "a 99.0 quote on a ~101 model price must solve to a modest spread, got {} bp",
        quoted.z_spread_bp
    );
    assert!(
        quoted.z_spread_bp > 0.0,
        "a quote below the model price means a positive spread, got {} bp",
        quoted.z_spread_bp
    );
}

#[test]
fn scenario_table_price_is_per_current_face() {
    let (deal, as_of) = fixture(true);
    let market = market(as_of);
    let grid = ScenarioGrid {
        cprs: vec![0.0],
        cdrs: vec![0.0],
        severities: vec![0.6],
        recovery_lag: None,
    };
    let table = scenario_table(&deal, "A", &market, as_of, &grid).expect("table");
    let model = calculate_tranche_metrics(&deal, "A", &market, as_of, None).expect("metrics");
    let cell = &table.cells[0];
    // The grid cell reprices at the deal's own assumptions here, so the cell
    // quotes the model's clean settlement price on current face.
    assert!(
        (cell.price - model.price_pct).abs() < 1e-6,
        "scenario price {} must be the clean settlement price {}",
        cell.price,
        model.price_pct
    );
    assert!(model.price_pct > 0.0);
}

/// A note that is fully written down in the projection has no settlement
/// value: it prices at zero and carries no yield instead of failing the
/// yield solve.
#[test]
fn fully_written_down_class_prices_at_zero_without_a_yield() {
    let (mut deal, as_of) = fixture(false);
    // Everything defaults at once with a 12-month recovery lag: the equity
    // sees no cash at all.
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(1.0);
    let market = market(as_of);
    let equity = deal
        .value_tranche_with_metrics("EQ", &market, as_of, &[])
        .expect("an impaired note still values");
    assert_eq!(equity.pv.amount(), 0.0);
    assert_eq!(equity.clean_price, 0.0);
    assert_eq!(equity.dirty_price, 0.0);
    assert!(equity.ytm.is_none(), "no yield on a worthless note");
    assert_eq!(equity.z_spread_bp, 0.0);
    assert_eq!(equity.cs01, 0.0);
    let senior = deal
        .value_tranche_with_metrics("A", &market, as_of, &[])
        .expect("senior");
    assert!(senior.ytm.is_some());
}

/// A 5% fixed note paying quarterly on a 30/360 basis quoted at par yields
/// exactly 5.00%: the yield compounds at the note's own coupon frequency and
/// measures time in the note's day count, so a par note yields its coupon.
#[test]
fn par_quarterly_note_yields_its_coupon() {
    let (mut deal, as_of) = fixture(false);
    deal.tranches.tranches[0].day_count = DayCount::Thirty360;
    // Unadjusted payment dates keep every 30/360 coupon period at exactly a
    // quarter; business-day rolls would move the yield by a few tenths of a
    // basis point.
    deal.payment_business_day_convention =
        Some(finstack_quant_core::dates::BusinessDayConvention::Unadjusted);
    deal.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(100.0);
    let market = market(as_of);
    let senior = deal
        .value_tranche_with_metrics("A", &market, as_of, &[])
        .expect("senior");
    let ytm = senior.ytm.expect("yield");
    assert!(
        (ytm - 0.05).abs() < 1e-6,
        "a par 5% quarterly note yields 5.00%, got {ytm}"
    );
    // `clean_price` stays the MODEL price; the quote only sets the yield's target.
    assert!(senior.clean_price > 0.0);
}

#[test]
fn deal_level_price_uses_current_balances() {
    let (deal, as_of) = fixture(true);
    let market = market(as_of);
    let result = deal
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DirtyPrice],
            PricingOptions::default(),
        )
        .expect("deal pricing");
    let dirty = result.measures["dirty_price"];
    // Deal NPV over the sum of CURRENT tranche balances (40M + 20M).
    let expected = result.value.amount() / 60_000_000.0 * 100.0;
    assert!(
        (dirty - expected).abs() < 1e-6,
        "deal-level dirty price {dirty} must be per current balances (expected {expected})"
    );
}
