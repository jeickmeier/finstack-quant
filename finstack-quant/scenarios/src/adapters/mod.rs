//! Adapter modules that implement the mechanics of each `OperationSpec`.
//!
//! The engine dispatches each [`OperationSpec`](crate::spec::OperationSpec)
//! variant via a centralized `match` to the appropriate free function in the
//! submodules below. There is no polymorphic adapter trait — the enum is
//! closed and the dispatch is exhaustive at compile time.

pub mod asset_corr;
pub mod basecorr;
pub mod curves;
pub mod equity;
pub mod fx;
pub mod instruments;
pub mod statements;
pub mod time_roll;
pub(crate) mod traits;
pub mod vol;

pub use time_roll::RollForwardReport;
pub use vol::ArbitrageViolation;
