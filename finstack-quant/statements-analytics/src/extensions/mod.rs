//! Concrete extension implementations.
//!
//! - [`CorkscrewExtension`] — roll-forward validation for balance-sheet accounts
//! - [`CreditScorecardExtension`] — credit scorecard rating assignment

mod corkscrew;
mod scorecards;

pub use corkscrew::extension::{
    AccountType, CorkscrewAccount, CorkscrewConfig, CorkscrewExtension, CorkscrewReport,
    CorkscrewStatus,
};
pub use scorecards::extension::{
    CreditScorecardExtension, ScorecardConfig, ScorecardMetric, ScorecardReport, ScorecardStatus,
};
