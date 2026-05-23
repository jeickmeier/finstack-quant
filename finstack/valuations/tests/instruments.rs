//! Instrument integration tests - comprehensive test runner for all instruments
//!
//! This module organizes instrument tests by asset class and product type.

// ============================================================================
// Common Test Infrastructure
// ============================================================================

/// Common test utilities, helpers, models, and shared functionality.
/// Includes parity testing framework for validating against QuantLib, Bloomberg, etc.
///
/// Tests should use `use crate::parity::*;` to import types.
/// Macros (assert_parity!, etc.) are available via `#[macro_use]` below.
#[macro_use]
#[path = "instruments/common/mod.rs"]
mod common;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

// Re-export parity module at crate level for `use crate::parity::*;` compatibility
pub use common::parity;

// ============================================================================
// Fixed Income Instruments
// ============================================================================

/// Bond tests - Fixed coupon bonds, zero-coupon bonds, floating rate notes
#[path = "instruments/bond/mod.rs"]
mod bond;

/// Deposit tests - Money market deposits and term deposits
#[path = "instruments/deposit/mod.rs"]
mod deposit;

/// Inflation-linked bond tests - Index-linked bonds (e.g., TIPS, linkers)
#[path = "instruments/inflation_linked_bond/mod.rs"]
mod inflation_linked_bond;

/// Bond future tests - Treasury and government bond futures
#[path = "instruments/bond_future/mod.rs"]
mod bond_future;

// ============================================================================
// Interest Rate Derivatives
// ============================================================================

/// Basis swap tests - Multi-curve basis swaps (e.g., 3m vs 6m LIBOR)
#[path = "instruments/basis_swap/mod.rs"]
mod basis_swap;

/// Cap/Floor tests - Interest rate caps and floors
#[path = "instruments/cap_floor/mod.rs"]
mod cap_floor;

/// Forward Rate Agreement (FRA) tests - Single period rate agreements
#[path = "instruments/fra/mod.rs"]
mod fra;

/// CMS Option tests - Constant Maturity Swap caps and floors
#[path = "instruments/cms_option/mod.rs"]
mod cms_option;

/// Interest rate future tests - Exchange-traded IR futures
#[path = "instruments/ir_future/mod.rs"]
mod ir_future;

/// Inflation swap tests - Zero-coupon and year-on-year inflation swaps
#[path = "instruments/inflation_swap/mod.rs"]
mod inflation_swap;

/// Inflation cap/floor tests - YoY inflation caps/floors
#[path = "instruments/inflation_cap_floor/mod.rs"]
mod inflation_cap_floor;

/// Interest Rate Swap (IRS) tests - Fixed-for-floating swaps
#[path = "instruments/irs/mod.rs"]
mod irs;

/// Swaption tests - Options on interest rate swaps
#[path = "instruments/swaption/mod.rs"]
mod swaption;

/// Exotic-rate MC/LSMC harness validation tests
/// (see `instruments/exotic_harness/mod.rs`)
#[path = "instruments/exotic_harness/mod.rs"]
mod exotic_harness;

// ============================================================================
// Credit Derivatives
// ============================================================================

/// Credit Default Swap (CDS) tests - Single-name protection
#[path = "instruments/cds/mod.rs"]
mod cds;

/// CDS index tests - Index CDS (e.g., CDX, iTraxx)
#[path = "instruments/cds_index/mod.rs"]
mod cds_index;

/// CDS option tests - Options on credit default swaps
#[path = "instruments/cds_option/mod.rs"]
mod cds_option;

/// CDS tranche tests - Structured credit tranches
#[path = "instruments/cds_tranche/mod.rs"]
mod cds_tranche;

// ============================================================================
// Equity Derivatives
// ============================================================================

/// Equity tests - Equity spot instruments
#[path = "instruments/equity/mod.rs"]
mod equity;

/// Barrier option tests - Knock-in/knock-out options
#[path = "instruments/barrier_option/mod.rs"]
mod barrier_option;

/// Basket option tests - Multi-underlying options
#[path = "instruments/basket/mod.rs"]
mod basket;

/// Equity option tests - Vanilla and exotic equity options
#[path = "instruments/equity_option/mod.rs"]
mod equity_option;

/// Equity index future tests - Index futures (ES, NQ, FESX, etc.)
#[path = "instruments/equity_index_future/mod.rs"]
mod equity_index_future;

/// Lookback option tests
#[path = "instruments/lookback_option/mod.rs"]
mod lookback_option;

/// Total Return Swap (TRS) tests - Equity and asset swaps
#[path = "instruments/trs/mod.rs"]
mod trs;

/// Variance swap tests - Volatility derivatives
#[path = "instruments/variance_swap/mod.rs"]
mod variance_swap;

// ============================================================================
// FX Derivatives
// ============================================================================

/// FX forward tests - Outright FX forward contracts
#[path = "instruments/fx_forward/mod.rs"]
mod fx_forward;

/// FX option tests - Currency options (vanilla and exotic)
#[path = "instruments/fx_option/mod.rs"]
mod fx_option;

/// FX barrier option tests - Barrier options on FX rates
#[path = "instruments/fx_barrier_option/mod.rs"]
mod fx_barrier_option;

/// FX spot tests - Spot foreign exchange transactions
#[path = "instruments/fx_spot/mod.rs"]
mod fx_spot;

/// FX swap tests - FX forward and swap transactions
#[path = "instruments/fx_swap/mod.rs"]
mod fx_swap;

/// FX variance swap tests - Volatility exposure on FX
#[path = "instruments/fx_variance_swap/mod.rs"]
mod fx_variance_swap;

/// Quanto option tests - Cross-currency quanto-adjusted equity options
#[path = "instruments/quanto_option/mod.rs"]
mod quanto_option;

/// NDF tests - Non-deliverable forwards for restricted currencies
#[path = "instruments/ndf/mod.rs"]
mod ndf;

/// XCCY swap tests - Cross-currency swaps (multi-currency floating legs)
#[path = "instruments/xccy_swap/mod.rs"]
mod xccy_swap;

// ============================================================================
// Commodity Derivatives
// ============================================================================

// NOTE: instruments/commodity/ tests are orphaned and need API updates
// They use legacy `finstack_core::market_data::curves` imports
#[path = "instruments/commodity/mod.rs"]
mod commodity;

/// Commodity option tests
#[path = "instruments/commodity_option/mod.rs"]
mod commodity_option;

// ============================================================================
// Real Estate
// ============================================================================

/// Real estate asset tests
#[path = "instruments/equity/real_estate/mod.rs"]
mod real_estate;

// ============================================================================
// Structured Products
// ============================================================================

/// Autocallable tests - Path-dependent structured notes with early redemption
#[path = "instruments/autocallable/mod.rs"]
mod autocallable;

/// Convertible bond tests - Bonds with embedded conversion options
#[path = "instruments/convertible/mod.rs"]
mod convertible;

/// Private market fund tests - Private equity and credit fund structures
#[path = "instruments/private_market_fund/mod.rs"]
mod private_market_fund;

/// Structured credit tests - CLOs, CDOs, and synthetic structures
#[path = "instruments/structured_credit/mod.rs"]
mod structured_credit;

// ============================================================================
// Repo and Financing
// ============================================================================

/// Repo tests - Repurchase agreements
#[path = "instruments/repo/mod.rs"]
mod repo;

/// Revolving credit tests - Revolving credit facilities
#[path = "instruments/revolving_credit/mod.rs"]
mod revolving_credit;

/// Term loan tests - Institutional term loans and DDTL facilities
#[path = "instruments/term_loan/mod.rs"]
mod term_loan;

// ============================================================================
// Golden Test Vectors
// ============================================================================

// ============================================================================
// Market Edge Case Tests
// ============================================================================

/// Market-edge tests for CDS and Bond instruments
/// Tests for upfront conventions, accrual-on-default, ex-coupon, stub periods
#[path = "instruments/market_edge_tests.rs"]
mod market_edge_tests;

// ============================================================================
// Curve Dependency Completeness Tests
// ============================================================================

/// Curve dependency completeness tests
/// Verifies that instruments correctly declare all their curve dependencies
#[path = "instruments/curve_dependency_completeness.rs"]
mod curve_dependency_completeness;

/// Equity dependency completeness tests
#[path = "instruments/equity_dependency_completeness.rs"]
mod equity_dependency_completeness;

/// Forward curve dependency completeness tests
#[path = "instruments/forward_curve_dependency_completeness.rs"]
mod forward_curve_dependency_completeness;

/// Forward dependency completeness tests
#[path = "instruments/forward_dependency_completeness.rs"]
mod forward_dependency_completeness;

// NOTE: fx_dependency_completeness.rs uses private `providers` module
#[path = "instruments/fx_dependency_completeness.rs"]
mod fx_dependency_completeness;

// ============================================================================
// Option Bounds Tests
// ============================================================================

/// Option bounds tests - arbitrage-free bounds for options
#[path = "instruments/test_option_bounds.rs"]
mod test_option_bounds;
