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
pub(super) fn effective_recovery(
    spec: &finstack_quant_cashflows::builder::RecoveryModelSpec,
) -> f64 {
    match &spec.severity_vector {
        Some(severities) if !severities.is_empty() => {
            1.0 - severities.iter().sum::<f64>() / severities.len() as f64
        }
        _ => spec.rate,
    }
}

pub use breakeven_cdr::tranche_breakeven_cdr;
pub(crate) use convexity::ConvexityCalculator;
pub use convexity::calculate_tranche_convexity;
pub use duration::calculate_tranche_duration;
pub(crate) use duration::{MacaulayDurationCalculator, ModifiedDurationCalculator};
pub use oas::{OasConfig, OasResult, tranche_oas};
pub(crate) use spreads::{
    BucketedCs01Calculator, Cs01Calculator, SpreadDurationCalculator, ZSpreadCalculator,
    calculate_tranche_spread_convexity,
};
pub use spreads::{calculate_tranche_cs01, calculate_tranche_z_spread, tranche_discount_margin};
pub(crate) use ytm::YtmCalculator;
