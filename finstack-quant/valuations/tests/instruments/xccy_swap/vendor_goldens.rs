//! Vendor-benchmark golden fixtures for MtM-resetting cross-currency swap pricing.
//!
//! These tests pin the codebase against a known vendor pricer (Bloomberg SWPM, Numerix,
//! ICE, etc.) once reference data is available. Until vendor numbers are dropped in,
//! every test in this module is `#[ignore]`d so it does NOT run as part of the default
//! `cargo test` suite — they fail loudly only when a maintainer explicitly opts in via
//! `cargo test -- --ignored vendor_golden`.
//!
//! ## How to populate a vendor golden
//!
//! 1. Build the same swap definition in the target vendor's UI (e.g., Bloomberg SWPM).
//!    Use the convention that maps 1:1 to `EUR/USD-XCCY` in `xccy_conventions.json`.
//! 2. Read off:
//!    - The vendor's reported MV (mark-to-market value, USD).
//!    - The par basis spread (bp).
//!    - Any intermediate values: leg PVs, FX-adjusted leg notionals at each reset.
//! 3. Drop the values into the constants below.
//! 4. Remove the `#[ignore]` attribute on the corresponding test.
//! 5. Confirm `cargo test --test instruments vendor_golden` passes.
//!
//! ## Why these are gated as ignored
//!
//! The pricing model is the CIP no-FX-vol approximation per the spec. Vendor pricers
//! may use:
//!   - A slightly different multi-curve convention (e.g., dual-curve OIS discounting
//!     with cross-currency spreads applied differently).
//!   - FX-rate correlation convexity (which our spec lists as a Non-Goal).
//!   - Different day-count or business-day conventions per pair.
//!
//! Expected agreement: ~5-10 bp on par spread, ~1e-4 relative on MV for vanilla G10
//! short-dated swaps. Larger deviations (>50bp) typically indicate a convention
//! mismatch worth investigating before adding `#[ignore]` back in.
//!
//! ## Status
//!
//! No vendor data available as of the initial Phase 6 implementation. Tests are
//! scaffold-only; populate them when a vendor quote sheet becomes available.

#![allow(dead_code)] // Constants and helpers are populated by vendor-data drops; until
                     // then, the unused-code lint would otherwise fire.

use finstack_quant_core::dates::Date;
use rust_decimal::Decimal;
use time::Month;

// ============================================================================
// Vendor reference: 5Y EUR/USD MtM-resetting basis swap
// ============================================================================
//
// Vendor: <FILL IN: bloomberg | numerix | ice | other>
// Trade date / value date: <FILL IN: e.g. 2025-01-02>
// Convention: EUR/USD-XCCY (MtM-resetting on EUR leg, USD as constant)
// Notional: $10,000,000 USD; €9,090,909.09 EUR initial (at spot 1.10)
// Pay/Receive: <FILL IN>
// Spread: <FILL IN: bp on the EUR (non-USD) leg>
// FX spot: 1.10 USD/EUR
// USD discount: <FILL IN: market curve snapshot date>
// EUR discount: <FILL IN: market curve snapshot date>

/// Expected MV (USD) from the vendor pricer for the 5Y EUR/USD MtM-reset basis swap.
/// Populate when vendor data is available.
const VENDOR_GOLDEN_5Y_MV_USD: Option<f64> = None;

/// Expected par basis spread (bp) from the vendor pricer.
/// Populate when vendor data is available.
const VENDOR_GOLDEN_5Y_PAR_SPREAD_BP: Option<f64> = None;

#[test]
#[ignore = "Requires vendor reference data; populate VENDOR_GOLDEN_5Y_MV_USD then remove the ignore."]
fn vendor_golden_5y_eur_usd_mtm_reset_mv_matches() {
    let Some(_expected_mv) = VENDOR_GOLDEN_5Y_MV_USD else {
        panic!(
            "VENDOR_GOLDEN_5Y_MV_USD is None. Populate the constant with the vendor's reported \
             MV (in USD) for the 5Y EUR/USD MtM-resetting basis swap, then remove the `#[ignore]` \
             attribute on this test. See module docstring for the build-instructions checklist."
        );
    };

    // Once vendor data is available, the test body should:
    //   1. Build the same swap definition (XccySwap with MtmResetting, EUR/USD-XCCY
    //      convention, specified spread/notional/dates).
    //   2. Build a MarketContext that replicates the vendor's curve snapshot
    //      (USD/EUR discount curves + forward curves + FX spot).
    //   3. Call swap.base_value(&ctx, as_of) and assert |pv - expected_mv| < tol.
    //
    // Tolerance: 1e-4 * notional is a reasonable starting point for vanilla G10
    // short-dated swaps. Tighten when comparing against a vendor that uses the same
    // discounting convention.
    panic!("Test body intentionally left unimplemented until vendor data is dropped in.");
}

#[test]
#[ignore = "Requires vendor reference data; populate VENDOR_GOLDEN_5Y_PAR_SPREAD_BP then remove the ignore."]
fn vendor_golden_5y_eur_usd_mtm_reset_par_spread_matches() {
    let Some(_expected_spread_bp) = VENDOR_GOLDEN_5Y_PAR_SPREAD_BP else {
        panic!(
            "VENDOR_GOLDEN_5Y_PAR_SPREAD_BP is None. Populate the constant with the vendor's \
             par basis spread (bp) for the 5Y EUR/USD MtM-resetting basis swap."
        );
    };

    // Once vendor data is available, the test body should:
    //   1. Build the same swap definition (same convention + dates as above).
    //   2. Build the same MarketContext.
    //   3. Bisect on the EUR-leg spread until base_value() = 0.
    //   4. Assert |s_par - expected_spread_bp| < tol_bp (e.g. 0.5 bp for G10 short-dated;
    //      relax to 1-2 bp for long-dated or EM crosses).
    panic!("Test body intentionally left unimplemented until vendor data is dropped in.");
}

#[allow(dead_code)] // helper kept ready for the test bodies above
fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).expect("valid base date")
}

#[allow(dead_code)]
fn vendor_spread_decimal(bp: f64) -> Decimal {
    Decimal::try_from(bp).expect("convert vendor spread (bp) to Decimal")
}
