//! Deal-type specific metrics for structured credit.

pub(crate) mod abs;
pub(crate) mod cmbs;

pub(crate) use abs::AbsDelinquencyCalculator;
pub(crate) use abs::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator,
};

pub(crate) use cmbs::CmbsDscrCalculator;
