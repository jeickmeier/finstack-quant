//! Typed `CreditDefaultSwap`, `CdsIndex`, `CdsTranche` and `ConvertibleBond`
//! classes and their fluent builders.

#[macro_use]
mod macros;

mod cds;
mod cds_index;
mod cds_tranche;
mod convertible;

pub use cds::*;
pub use cds_index::*;
pub use cds_tranche::*;
pub use convertible::*;
