//! Concrete extension implementations.
//!
//! - [`CorkscrewExtension`] — roll-forward validation for balance-sheet accounts
//! - [`CreditScorecardExtension`] — credit scorecard rating assignment

mod corkscrew;
mod scorecards;

pub use corkscrew::{
    AccountType, CorkscrewAccount, CorkscrewConfig, CorkscrewExtension, CorkscrewReport,
    CorkscrewStatus,
};
pub use scorecards::{
    CreditScorecardExtension, ScorecardConfig, ScorecardMetric, ScorecardReport, ScorecardStatus,
};
