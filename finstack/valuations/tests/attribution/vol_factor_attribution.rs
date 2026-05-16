//! Taylor vol-factor P&L attribution unit tests.
//!
//! Verifies that the Taylor vol attribution uses consistent units throughout:
//! - `vega_per_point`: $ per vol point (1 vol point = 1 percentage point of vol = 0.01 absolute)
//! - `vol_move`: vol points (as returned by `measure_vol_surface_shift`, which multiplies
//!   the absolute move by 100)
//! - `explained_pnl = vega_per_point × vol_move` must match full-revaluation P&L for a
//!   1-vol-point move within the second-order residual.

use finstack_core::currency::Currency;
use finstack_core::dates::DayCount;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_valuations::attribution::{attribute_pnl_taylor, TaylorAttributionConfig};
use finstack_valuations::instruments::equity::equity_option::EquityOption;
use finstack_valuations::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_valuations::instruments::{Instrument, PricingOverrides};
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
        .insert_price("EQ-SPOT", MarketScalar::Price(Money::new(SPOT, Currency::USD)))
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
        pricing_overrides: PricingOverrides::default(),
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
    let result = attribute_pnl_taylor(&inst, &market_t0, &market_t1, as_of_t0, as_of_t1, &config)
        .expect("Taylor attribution must succeed");

    // Find the vol factor result.
    let vol_factor = result
        .factors
        .iter()
        .find(|f| f.factor_name.starts_with("Vol:"))
        .expect("Vol factor must be present in Taylor attribution result");

    let explained = vol_factor.explained_pnl;

    eprintln!(
        "full_reval_pnl = {:.4}, explained = {:.4}, vol_move_points = {:.4}, vega_per_point = {:.4}",
        full_reval_pnl, explained, vol_factor.market_move, vol_factor.sensitivity
    );

    // vol_move must be 1.0 vol point (diff.rs multiplies absolute shift by 100).
    let expected_vol_move = VOL_SHIFT_ABS * 100.0; // 1.0
    assert!(
        (vol_factor.market_move - expected_vol_move).abs() < 1e-6,
        "market_move should be {:.4} vol points, got {:.4}",
        expected_vol_move,
        vol_factor.market_move
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
