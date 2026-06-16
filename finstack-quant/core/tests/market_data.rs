//! Market data module integration tests.
//!
//! This test suite verifies market data types and functionality:
//! - Term structure curves (discount, forward, hazard, inflation, base correlation)
//! - MarketContext container
//! - Bump infrastructure
//! - FX providers
//! - Scalar time series and dividends
//! - Credit index data
//! - Volatility surfaces
//!
//! # Test Organization
//!
//! - `test_helpers`: Shared test utilities
//! - `curves/`: Term structure curve tests
//! - `surfaces/`: Volatility surface tests
//! - `context`: MarketContext container tests
//! - `bumps`: Bump infrastructure tests
//! - `fx`: FX provider tests
//! - `scalars`: Scalar time series and dividend tests
//! - `credit_index`: Credit index data tests
//! - `serde`: Cross-cutting serialization tests

// Test helpers shared across modules
#[path = "market_data/test_helpers.rs"]
mod test_helpers;

// Term structure curves
#[path = "market_data/curves/mod.rs"]
mod curves;

// Volatility surfaces
#[path = "market_data/surfaces/mod.rs"]
mod surfaces;

// Market context
#[path = "market_data/context.rs"]
mod context;

// Bump infrastructure
#[path = "market_data/bumps.rs"]
mod bumps;

// Diff measurements
#[path = "market_data/diff_tests.rs"]
mod diff_tests;

// FX providers
#[path = "market_data/fx.rs"]
mod fx;

// Scalar types (time series, dividends)
#[path = "market_data/scalars.rs"]
mod scalars;

// Credit index data
#[path = "market_data/credit_index.rs"]
mod credit_index;

// Serialization tests
#[path = "market_data/serde.rs"]
mod market_data_serde;

// Hierarchy tests
#[path = "market_data/hierarchy.rs"]
mod hierarchy;
