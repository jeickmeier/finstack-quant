//! Member parity for the typed instrument classes and their builders.
//!
//! Every typed instrument class exposes one getter per public Rust field, the
//! `Instrument` trait surface, `toDict`, and a `builder()` whose setters map
//! one for one onto the Rust `FinancialBuilder` setters. The members are
//! emitted by the macros below so each one is a single conversion plus one
//! Rust call:
//!
//! - `get` converts a Rust field to its JavaScript value: `Money`,
//!   `Tenor`, `DayCount` and `Currency` become their WASM handles, dates
//!   become ISO-8601 strings, identifiers and enums become their serde
//!   strings, decimals become numbers, and nested specs become plain objects
//!   (the schema-generated TypeScript types).
//! - `arg` converts a JavaScript setter argument to the Rust setter type
//!   with the strict `utils::input` checks.

#[macro_use]
pub(crate) mod macros;

pub mod bond_future;
pub mod equity_future;
pub mod equity_future_option;
pub mod fx_future;
pub mod fx_future_option;
pub mod interest_rate_future;
pub mod interest_rate_future_option;
pub mod volatility_index_future;
pub mod volatility_index_future_option;

pub mod agency_cmo;
pub mod agency_mbs_passthrough;
pub mod agency_tba;
pub mod asian_option;
pub mod autocallable;
pub mod barrier_option;
pub mod basis_swap;
pub mod basket;
pub mod bermudan_swaption;
pub mod callable_range_accrual;
pub mod cliquet_option;
pub mod cms_option;
pub mod cms_spread_option;
pub mod cms_swap;
pub mod commodity_asian_option;
pub mod commodity_forward;
pub mod commodity_future;
pub mod commodity_future_option;
pub mod commodity_spot_option;
pub mod commodity_spread_option;
pub mod commodity_swap;
pub mod commodity_swaption;
pub mod credit;
pub mod data;
pub mod dollar_roll;
pub mod equity;
pub mod equity_spot;
pub mod equity_total_return_future;
pub mod equity_total_return_swap;
pub mod fi_index_total_return_swap;
pub mod fixed_income;
pub mod fx;
pub mod fx_barrier_option;
pub mod fx_digital_option;
pub mod fx_spot;
pub mod fx_swap;
pub mod fx_touch_option;
pub mod fx_variance_swap;
pub mod inflation_bond;
pub mod inflation_cap_floor;
pub mod inflation_swap;
pub mod levered_real_estate_equity;
pub mod listed;
pub mod lookback_option;
pub mod ndf;
pub mod private_markets_fund;
pub mod quanto_option;
pub mod range_accrual;
pub mod rates;
pub mod real_estate_asset;
pub mod repo;
pub mod snowball;
pub mod structured;
pub(crate) mod support;
pub mod tarn;
pub mod variance_swap;
pub mod xccy_swap;
pub mod yoy_inflation_swap;

pub(crate) use support::{
    Staged, arg, envelope_json, get, market_dependencies, metric_value, option_greeks,
};
