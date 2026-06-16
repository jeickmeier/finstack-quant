//! Risk and sensitivity metrics for structured credit.

pub(crate) mod default01;
pub(crate) mod duration;
pub(crate) mod prepayment01;
pub(crate) mod recovery01;
pub(crate) mod severity01;
pub(crate) mod spreads;
pub(crate) mod ytm;

pub use duration::{
    calculate_tranche_duration, MacaulayDurationCalculator, ModifiedDurationCalculator,
};
pub use spreads::{
    calculate_tranche_cs01, calculate_tranche_z_spread, BucketedCs01Calculator, Cs01Calculator,
    SpreadDurationCalculator, ZSpreadCalculator,
};
pub use ytm::YtmCalculator;
