//! Market data model tests.
//!
//! Tests for market quote types, bumping, schema validation, and instrument building.
//!
//! ## Test Organization
//!
//! - `build/` - Instrument building from market quotes
//!   - `credit` - Credit instrument building from CDS quotes
//!   - `rates` - Rate instrument building from deposit/swap/FRA quotes
//! - `quote_schemas` - Schema validation, serialization, and concrete quote bump tests

pub mod build;
mod market_quote;
mod quote_schemas;
