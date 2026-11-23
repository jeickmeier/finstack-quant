//! Foreign-exchange interfaces and a simplified FX matrix.
//!
//! Design goals:
//! - store raw FX quotes for currency pairs
//! - compute reciprocal and triangulated rates on demand
//! - provide deterministic lookups with bounded LRU caching
//!
//! The public surface remains stable:
//! - `FxProvider` trait for on-demand quotes
//! - `FxMatrix` offering `FxMatrix::rate` for consumers and `MarketContext`
//! - standard provider implementations (e.g. `SimpleFxProvider`)
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider, FxQuery};
//! use finstack_quant_core::money::fx::SimpleFxProvider;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use std::sync::Arc;
//! use time::Month;
//!
//! let provider = Arc::new(SimpleFxProvider::new());
//! provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.1)]);
//!
//! let matrix = FxMatrix::new(provider.clone(), None);
//! let date = Date::from_calendar_date(2024, Month::January, 5).expect("Valid date");
//! let res = matrix.rate(FxQuery::new(Currency::EUR, Currency::USD, date, FxConversionPolicy::CashflowDate)).expect("FX rate lookup should succeed");
//! // Observed quotes use a canonical storage orientation, so compare rates with
//! // a numerical tolerance rather than relying on reciprocal round-trip equality.
//! assert!((res.rate - 1.1).abs() < 1e-12);
//! ```

mod conventions;
mod matrix;
mod provider;
mod providers;
mod types;

pub use conventions::{FxPairConvention, invert_fx_rate};
pub use matrix::FxMatrix;
pub(crate) use provider::validate_fx_rate;
pub use provider::{FxProvider, FxProviderSnapshot};
pub use providers::SimpleFxProvider;
pub use types::{
    CurrencyPair, FxConfig, FxConversionPolicy, FxMatrixState, FxPolicyMeta, FxQuery, FxRateResult,
};

#[cfg(test)]
mod tests;
