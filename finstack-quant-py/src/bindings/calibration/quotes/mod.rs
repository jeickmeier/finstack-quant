//! Typed calibration market data: rate, CDS, volatility, inflation,
//! cross-currency and CDS-tranche quotes plus the FX-spot, price,
//! and collateral datums carried by the calibration envelope.
//!
//! Quote constructors convert host arguments and delegate to the typed Rust
//! quote constructors, which own defaults and quote validation. The datum
//! constructors convert host values into the Rust payload structs.

mod market;
mod rates_vol;

pub(crate) use market::extract_market_datum;
pub use market::{PyCdsTrancheQuote, PyFxSpotDatum, PyInflationQuote, PyPriceDatum, PyXccyQuote};
pub use rates_vol::{PyCdsQuote, PyRateQuote, PyVolQuote};
