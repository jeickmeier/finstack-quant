//! Adapter modules that implement the mechanics of each `OperationSpec`.
//!
//! The engine dispatches each [`OperationSpec`](crate::spec::OperationSpec)
//! variant via a centralized `match` to the appropriate free function in the
//! submodules below. There is no polymorphic adapter trait — the enum is
//! closed and the dispatch is exhaustive at compile time.

pub(crate) mod asset_corr;
pub(crate) mod basecorr;
pub(crate) mod curves;
pub(crate) mod equity;
pub(crate) mod fx;
pub(crate) mod instruments;
pub(crate) mod statements;
pub(crate) mod time_roll;
pub(crate) mod traits;
pub(crate) mod vol;

pub(crate) use time_roll::RollForwardReport;
