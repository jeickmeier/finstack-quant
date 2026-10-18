//! Deal-type specific metrics for structured credit.

pub(crate) mod abs;
pub(crate) mod cmbs;

pub use abs::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator, AbsPaymentRateCalculator,
};

pub use cmbs::CmbsDscrCalculator;
