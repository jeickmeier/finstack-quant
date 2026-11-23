//! Product-independent stochastic models for structured-credit collateral pools.
//!
//! This module owns default, prepayment, and correlation engines and their
//! serializable specifications. Deal construction, calibration presets,
//! waterfalls, and tranche pricing remain in `finstack-quant-valuations`.

mod correlation;
mod default;
mod prepayment;
mod smm;

pub use correlation::CorrelationStructure;
pub use default::per_name::{PerNameCopulaDefault, PoolGranularity};
pub use default::spec::StochasticDefaultSpec;
pub use default::traits::StochasticDefault;
pub use prepayment::spec::StochasticPrepaySpec;
pub use prepayment::traits::{PrepaymentState, StochasticPrepayment};

pub(super) use smm::capped_annual_to_monthly;
use smm::expected_shocked_smm;
