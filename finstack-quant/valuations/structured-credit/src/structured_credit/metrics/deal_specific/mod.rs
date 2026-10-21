//! Deal-type specific metrics for structured credit.

pub(crate) mod abs;
pub(crate) mod cmbs;

pub(crate) use abs::AbsDelinquencyCalculator;
pub use abs::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator,
};

pub use cmbs::CmbsDscrCalculator;
