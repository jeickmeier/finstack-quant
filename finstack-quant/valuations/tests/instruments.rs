//! Instrument integration tests - comprehensive test runner for all instruments
//!
//! This module organizes instrument tests by asset class and product type.

// Common Test Infrastructure

/// Shared parameter and registry contract tests.
///
/// Curve builders, tolerances and the parity helpers come from
/// `finstack_quant_valuations_test_support`.
#[path = "instruments/common/mod.rs"]
mod common;
#[path = "instruments/compounding_leg_wire_keys.rs"]
mod compounding_leg_wire_keys;
#[path = "instruments/coupon_strike_inflation_wire_keys.rs"]
mod coupon_strike_inflation_wire_keys;
#[path = "instruments/credit_wire_keys.rs"]
mod credit_wire_keys;
#[path = "instruments/dates_calendars_wire_keys.rs"]
mod dates_calendars_wire_keys;
#[path = "instruments/exotic_payoff_wire_keys.rs"]
mod exotic_payoff_wire_keys;
#[path = "instruments/fx_money_market_wire_keys.rs"]
mod fx_money_market_wire_keys;
#[path = "instruments/listed_future_wire_keys.rs"]
mod listed_future_wire_keys;
#[path = "instruments/loan_facility_wire_keys.rs"]
pub(crate) mod loan_facility_wire_keys;
#[path = "instruments/private_markets_wire_keys.rs"]
mod private_markets_wire_keys;
#[path = "instruments/rates_projection_wire_keys.rs"]
mod rates_projection_wire_keys;
#[path = "instruments/registry_coverage.rs"]
mod registry_coverage;
#[path = "instruments/serde_skip_guard.rs"]
mod serde_skip_guard;
#[path = "instruments/settlement_timing_wire_keys.rs"]
mod settlement_timing_wire_keys;
#[path = "instruments/underlying_identity_wire_keys.rs"]
mod underlying_identity_wire_keys;
#[path = "instruments/unit_suffix_wire_keys.rs"]
mod unit_suffix_wire_keys;

#[path = "support/mod.rs"]
#[allow(dead_code, unused_imports)]
pub(crate) mod test_support;

// Fixed Income Instruments

/// Bond tests - Fixed coupon bonds, zero-coupon bonds, floating rate notes
#[path = "instruments/bond/mod.rs"]
mod bond;

/// Bond future tests - Treasury and government bond futures
#[path = "instruments/bond_future/mod.rs"]
mod bond_future;

// Interest Rate Derivatives

/// Futures-option tests - Shared option-on-future wrappers
#[path = "instruments/future_option/mod.rs"]
mod future_option;

// Credit Derivatives

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

// Equity Derivatives

/// Basket option tests - Multi-underlying options
#[path = "instruments/basket/mod.rs"]
mod basket;

/// Total Return Swap (TRS) tests - Cross-family equity and FI index TRS suites
#[path = "instruments/trs/mod.rs"]
mod trs;

// FX Derivatives

/// FX barrier option tests - JSON/observed-state regression suite (single-family FX tests live in `fx/tests/`)
#[path = "instruments/fx_barrier_option/mod.rs"]
mod fx_barrier_option;

// Commodity Derivatives

/// Commodity option tests (JSON-pinned metrics regression; single-family commodity tests live in `exotics/tests/`)
#[path = "instruments/commodity_spot_option/mod.rs"]
mod commodity_spot_option;

// Real Estate

/// Real estate asset tests
#[path = "instruments/equity/real_estate/mod.rs"]
mod real_estate;

// Structured Products

/// Convertible bond tests - Bonds with embedded conversion options
#[path = "instruments/convertible/mod.rs"]
mod convertible;

/// Structured credit tests - CLOs, CDOs, and synthetic structures
#[path = "instruments/structured_credit/mod.rs"]
mod structured_credit;

// Financing

/// Revolving credit tests - Revolving credit facilities
#[path = "instruments/revolving_credit/mod.rs"]
mod revolving_credit;

/// Asset-backed facility tests - warehouse lines against collateral pools
#[path = "instruments/asset_backed_facility/mod.rs"]
mod asset_backed_facility;

/// Term loan tests - Institutional term loans and DDTL facilities
#[path = "instruments/term_loan/mod.rs"]
mod term_loan;

// Golden Test Vectors

// Market Edge Case Tests

/// Market-edge tests for CDS and Bond instruments
/// Tests for upfront conventions, accrual-on-default, ex-coupon, stub periods
#[path = "instruments/market_edge_tests.rs"]
mod market_edge_tests;

// Curve Dependency Completeness Tests

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

#[path = "instruments/fx_dependency_completeness.rs"]
mod fx_dependency_completeness;

// Option Bounds Tests

/// Option bounds tests - arbitrage-free bounds for options
#[path = "instruments/test_option_bounds.rs"]
mod test_option_bounds;

/// Focused pricing-override bags serialize under the three canonical keys.
#[path = "instruments/override_wire_shape.rs"]
mod override_wire_shape;

/// Dividend-yield identifiers are market scalars, not series.
#[path = "instruments/dividend_yield_dependency.rs"]
mod dividend_yield_dependency;
