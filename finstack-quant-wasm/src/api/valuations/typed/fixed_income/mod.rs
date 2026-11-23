//! Typed fixed-income instrument classes (`Bond`, `TermLoan`,
//! `RevolvingCredit`, `AssetBackedFacility`) and their fluent builders.

mod asset_backed;
mod bond;
mod ladders;
mod revolving_credit;
mod term_loan;

pub use asset_backed::*;
pub use bond::*;
pub use ladders::*;
pub use revolving_credit::*;
pub use term_loan::*;
