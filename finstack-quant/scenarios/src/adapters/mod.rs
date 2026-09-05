//! Adapter modules that implement the mechanics of each `OperationSpec`.
//!
//! The engine dispatches each [`OperationSpec`](crate::spec::OperationSpec)
//! variant to a free function in the submodules below.

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
