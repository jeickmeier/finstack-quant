//! Risk and sensitivity metrics for structured credit.

pub(crate) mod breakeven_cdr;
pub(crate) mod convexity;
pub(crate) mod default01;
pub(crate) mod duration;
pub(crate) mod oas;
pub(crate) mod prepayment01;
pub(crate) mod recovery01;
pub(crate) mod severity01;
pub(crate) mod spreads;
pub(crate) mod ytm;

pub(crate) use convexity::ConvexityCalculator;
pub(crate) use duration::{MacaulayDurationCalculator, ModifiedDurationCalculator};
pub(crate) use oas::OasResult;
pub(crate) use spreads::{
    BucketedCs01Calculator, Cs01Calculator, SpreadDurationCalculator, ZSpreadCalculator,
    calculate_tranche_cs01, calculate_tranche_spread_convexity, calculate_tranche_z_spread,
};
pub(crate) use ytm::YtmCalculator;
