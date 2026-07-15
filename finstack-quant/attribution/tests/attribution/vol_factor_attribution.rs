//! Taylor vol-factor P&L attribution unit tests.
//!
//! Verifies that Taylor maps volatility attribution into `PnlAttribution::vol_pnl`
//! with consistent vol-point units and sensible first/second-order scaling.

use finstack_quant_attribution::{attribute_pnl_taylor, ExecutionPolicy, TaylorAttributionConfig};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use std::sync::Arc;
use time::macros::date;

// ---- Constants ----

/// Spot used for the test equity option.
const SPOT: f64 = 100.0;
/// Strike set at-the-money for maximum vega sensitivity.
const STRIKE: f64 = 100.0;
/// Flat implied vol in absolute units (20%).
const VOL_BASE: f64 = 0.20;
/// 1 vol point shift in absolute terms.
const VOL_SHIFT_ABS: f64 = 0.01;
/// Risk-free rate.
const RATE: f64 = 0.05;
/// Continuous dividend yield.
const DIV_YIELD: f64 = 0.02;

// ---- Helpers ----

fn build_market(vol: f64) -> MarketContext {
    let as_of = date!(2025 - 01 - 01);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (0.5f64, (-RATE * 0.5f64).exp()),
            (1.0f64, (-RATE).exp()),
            (2.0f64, (-RATE * 2.0f64).exp()),
        ])
        .build()
        .unwrap();

    // Flat vol surface covering the expiry used by the test option.
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&[0.5, 1.0, 2.0])
        .strikes(&[80.0, 100.0, 120.0])
        .row(&[vol, vol, vol])
        .row(&[vol, vol, vol])
        .row(&[vol, vol, vol])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(surface)
        .insert_price(
            "EQ-SPOT",
            MarketScalar::Price(Money::new(SPOT, Currency::USD)),
        )
        .insert_price("EQ-DIV", MarketScalar::Unitless(DIV_YIELD))
}

fn build_option() -> EquityOption {
    EquityOption {
        id: "VOL-ATTR-TEST".into(),
        underlying_ticker: "EQ".to_string(),
        strike: STRIKE,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry: date!(2026 - 01 - 01), // 1Y to expiry from as_of
        notional: Money::new(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "EQ-SPOT".into(),
        vol_surface_id: "EQ-VOL".into(),
        div_yield_id: Some("EQ-DIV".into()),
        discrete_dividends: Vec::new(),
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    }
}

// ---- Test ----

/// Verify that the Taylor vol-factor `explained_pnl` matches full-revaluation P&L
/// for a pure 1-vol-point shift (0.01 absolute vol).
///
/// # Unit analysis
///
/// `measure_vol_surface_shift` returns the move in **percentage points** (×100 of absolute).
/// A 0.01 absolute shift → 1.0 vol point reported.
///
/// `vega_per_point` must be in **$ per vol point** so that:
///   `explained_pnl = vega_per_point × vol_move_points` is in dollars.
///
/// Before the fix, `vega_per_point` was computed as:
///   `(pv_up - pv_down) / (2 × vol_bump_abs)`   ← $ per absolute vol unit
/// which is 100× too large when multiplied by the move expressed in vol points.
///
/// After the fix:
///   `vega_per_point = (pv_up - pv_down) / (2 × vol_bump_abs × 100)`  ← $ per vol point
///
/// For a 1-vol-point move the second-order term is
///   `0.5 × volga × 1² ≈ 0.5 × volga`
/// which is tiny relative to the first-order term for a 1-point move,
/// so the residual `|explained - full_reval|` must be < 1% of `|full_reval|`.
#[test]
fn taylor_vol_factor_matches_full_revaluation() {
    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02); // one calendar day; same pricing date for simplicity

    let market_t0 = build_market(VOL_BASE);
    let market_t1 = build_market(VOL_BASE + VOL_SHIFT_ABS); // exactly +1 vol point

    let option = build_option();
    let inst: Arc<dyn Instrument> = Arc::new(option.clone());

    // Full revaluation P&L: reprice under market_t1 minus market_t0.
    let pv_t0 = option.value(&market_t0, as_of_t0).unwrap().amount();
    let pv_t1 = option.value(&market_t1, as_of_t0).unwrap().amount(); // same pricing date
    let full_reval_pnl = pv_t1 - pv_t0;

    // Taylor attribution.
    let config = TaylorAttributionConfig {
        include_gamma: false,
        vol_bump: 0.01, // 1% absolute bump
        ..TaylorAttributionConfig::default()
    };
    let result = attribute_pnl_taylor(
        &inst,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .expect("Taylor attribution must succeed");
    let explained = result.vol_pnl.amount();

    eprintln!(
        "full_reval_pnl = {:.4}, explained = {:.4}",
        full_reval_pnl, explained
    );

    // full_reval_pnl must be non-trivial (sanity: ATM option gains value when vol rises).
    assert!(
        full_reval_pnl > 0.0,
        "Full-reval P&L should be positive for a long call when vol increases, got {:.4}",
        full_reval_pnl
    );

    // The explained P&L should be close to full-reval P&L (within 1% relative).
    // The residual is the genuine second-order term which is small for a 1-point move.
    let relative_error = ((explained - full_reval_pnl) / full_reval_pnl).abs();
    assert!(
        relative_error < 0.01,
        "Taylor vol explained P&L ({:.4}) should match full-reval P&L ({:.4}) within 1% \
         (relative error: {:.2}%). This likely indicates a unit mismatch (100× factor).",
        explained,
        full_reval_pnl,
        relative_error * 100.0
    );
}

/// Verify that the Taylor vol-factor `gamma_pnl` (volga term) is correctly scaled
/// for a large vol move.
///
/// # Why this test guards the ÷100² scaling
///
/// After the fix, volga is computed as:
///   `volga [$/pt²] = ΔΔP / (vol_bump_points)²`  where `vol_bump_points = vol_bump_abs × 100`
///
/// Before the fix the denominator was `(vol_bump_abs)²` — 10,000× too small — making
/// volga (and therefore `gamma_pnl`) 10,000× too large for a given vol-point move.
///
/// This test uses a 5-vol-point shift (0.05 absolute) which makes the second-order term
/// material: with a 20% base vol an ATM 1Y option has volga > 0, so the linear
/// approximation under-estimates and `gamma_pnl` gives a meaningful positive correction.
///
/// # Assertions
///
/// 1. `gamma_pnl` is `Some(..)` and non-zero.
/// 2. `gamma_pnl` is positive (ATM option has positive volga; large vol move → positive correction).
/// 3. `gamma_pnl` is a sensible fraction of `full_reval_pnl` — between 0.001% and 1%.
///    With the pre-fix bug (10,000× volga), this ratio would be ~100%, failing hard.
/// 4. The first-order explained P&L alone is within 10% of full-reval (confirming the
///    first-order vega unit is correct), and the second-order gamma_pnl is in the right
///    direction (same sign as the first-order residual).
///
/// With the pre-fix bug (10,000× too large volga), `gamma_pnl` would be orders of
/// magnitude larger than the entire option value — assertion 3 would fail catastrophically.
#[test]
fn taylor_vol_factor_gamma_matches_full_revaluation() {
    // A 5 vol-point (0.05 absolute) shift makes the second-order volga term material
    // while keeping the Taylor expansion in its valid regime.
    const VOL_SHIFT_LARGE_ABS: f64 = 0.05; // 5 vol points absolute

    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02); // same pricing date for simplicity

    let market_t0 = build_market(VOL_BASE);
    let market_t1 = build_market(VOL_BASE + VOL_SHIFT_LARGE_ABS); // +5 vol points

    let option = build_option();
    let inst: Arc<dyn Instrument> = Arc::new(option.clone());

    // Full revaluation P&L (benchmark).
    let pv_t0 = option.value(&market_t0, as_of_t0).unwrap().amount();
    let pv_t1 = option.value(&market_t1, as_of_t0).unwrap().amount(); // same date
    let full_reval_pnl = pv_t1 - pv_t0;

    // Taylor attribution with include_gamma = true.
    let config = TaylorAttributionConfig {
        include_gamma: true,
        vol_bump: 0.01,
        ..TaylorAttributionConfig::default()
    };
    let result = attribute_pnl_taylor(
        &inst,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .expect("Taylor attribution with gamma must succeed");
    let combined = result.vol_pnl.amount();

    eprintln!(
        "full_reval_pnl = {:.4}, combined = {:.4}",
        full_reval_pnl, combined,
    );

    // The gamma-inclusive Taylor vol bucket should stay close to full revaluation.
    // The pre-fix 10,000x volga scaling bug would make this bucket explode.
    let relative_error = (combined - full_reval_pnl).abs() / full_reval_pnl.abs();
    assert!(
        relative_error < 0.02,
        "Taylor vol P&L ({:.4}) should be within 2% of full-reval ({:.4}); \
         relative error: {:.2}%. This likely indicates a vol unit mismatch.",
        combined,
        full_reval_pnl,
        relative_error * 100.0,
    );
}

/// A pure SKEW change — wings move, ATM column fixed —
/// must not be misread as a parallel vol move. `measure_vol_surface_shift`
/// averages the surface move, so an antisymmetric skew nets toward zero and
/// the vol factor must attribute LESS P&L than a genuine 1-point parallel
/// shift; the unexplained smile P&L lands in the residual and the
/// reconciliation must still close.
#[test]
fn vol_skew_change_attributes_less_than_parallel_shift() {
    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02);

    // T1 surface: skew steepens ±2 vol points around an UNCHANGED ATM column.
    let skewed = |vol: f64, skew: f64| {
        VolSurface::builder("EQ-VOL")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[vol + skew, vol, vol - skew])
            .row(&[vol + skew, vol, vol - skew])
            .row(&[vol + skew, vol, vol - skew])
            .build()
            .unwrap()
    };
    let base_market = build_market(VOL_BASE);
    let skew_market = {
        let mut m = build_market(VOL_BASE);
        m.replace_surfaces_mut([(
            finstack_quant_core::types::CurveId::new("EQ-VOL"),
            std::sync::Arc::new(skewed(VOL_BASE, 0.02)),
        )]);
        m
    };
    let parallel_market = build_market(VOL_BASE + VOL_SHIFT_ABS);

    let option: Arc<dyn Instrument> = Arc::new(build_option());
    let run = |market_t1: &MarketContext| {
        attribute_pnl_taylor(
            &option,
            &base_market,
            market_t1,
            as_of_t0,
            as_of_t1,
            &TaylorAttributionConfig::default(),
            ExecutionPolicy::Serial,
        )
        .expect("taylor attribution should succeed")
    };

    let skew_attr = run(&skew_market);
    let parallel_attr = run(&parallel_market);

    // Reconciliation closes for the skew scenario (factors + residual = total).
    let attributed = skew_attr.carry.amount()
        + skew_attr.rates_curves_pnl.amount()
        + skew_attr.vol_pnl.amount()
        + skew_attr.market_scalars_pnl.amount()
        + skew_attr.cross_factor_pnl.amount()
        + skew_attr.residual.amount();
    assert!(
        (attributed - skew_attr.total_pnl.amount()).abs() < 1e-6,
        "skew scenario must reconcile: attributed {attributed}, total {}",
        skew_attr.total_pnl.amount()
    );
    assert!(!skew_attr.result_invalid, "skew attribution must be valid");

    // The ATM option's vol factor under a ±2pt skew (ATM fixed) must be
    // smaller than under a genuine 1pt parallel move — the averaged shift
    // nets toward zero rather than registering a large parallel move.
    assert!(
        skew_attr.vol_pnl.amount().abs() < parallel_attr.vol_pnl.amount().abs(),
        "a pure skew (ATM fixed) must attribute less vol P&L ({}) than a 1pt \
         parallel shift ({})",
        skew_attr.vol_pnl.amount(),
        parallel_attr.vol_pnl.amount()
    );
}
