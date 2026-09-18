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

/// Recovery rate a bumped spec actually applies: the flat `rate`, or one
/// minus the average of the severity vector when it is present. Used by the
/// recovery/severity sensitivities to measure the achieved bump width.
pub(super) fn effective_recovery(spec: &crate::cashflow::builder::RecoveryModelSpec) -> f64 {
    match &spec.severity_vector {
        Some(severities) if !severities.is_empty() => {
            1.0 - severities.iter().sum::<f64>() / severities.len() as f64
        }
        _ => spec.rate,
    }
}

pub use breakeven_cdr::calculate_tranche_breakeven_cdr;
pub use convexity::{calculate_tranche_convexity, ConvexityCalculator};
pub use duration::{
    calculate_tranche_duration, MacaulayDurationCalculator, ModifiedDurationCalculator,
};
pub use oas::{calculate_tranche_oas, OasConfig, OasResult};
pub use spreads::{
    calculate_tranche_cs01, calculate_tranche_discount_margin, calculate_tranche_spread_convexity,
    calculate_tranche_z_spread, BucketedCs01Calculator, Cs01Calculator, SpreadDurationCalculator,
    ZSpreadCalculator,
};
pub use ytm::YtmCalculator;
