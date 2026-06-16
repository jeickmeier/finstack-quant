//! Concrete extension implementations.
//!
//! - [`crate::extensions::corkscrew`] — roll-forward validation for balance-sheet accounts
//! - [`crate::extensions::scorecards`] — credit scorecard rating assignment

pub mod corkscrew;
pub mod scorecards;

pub use corkscrew::{
    AccountType, CorkscrewAccount, CorkscrewConfig, CorkscrewExtension, CorkscrewReport,
    CorkscrewStatus,
};
pub use scorecards::{
    CreditScorecardExtension, ScorecardConfig, ScorecardMetric, ScorecardReport, ScorecardStatus,
};
