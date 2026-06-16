//! Deal-type specific metrics for structured credit.

pub(crate) mod abs;
pub(crate) mod clo_wal;
pub(crate) mod cmbs;
pub(crate) mod rmbs;

// Re-export ABS metrics
pub use abs::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator, AbsSpeedCalculator,
};

// Re-export CLO metrics
pub use clo_wal::CloWalCalculator;

// Re-export CMBS metrics
pub use cmbs::{CmbsDscrCalculator, CmbsLtvCalculator};

// Re-export RMBS metrics
pub use rmbs::{RmbsFicoCalculator, RmbsLtvCalculator, RmbsWalCalculator};
