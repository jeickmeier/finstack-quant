//! Loss Given Default modeling primitives.
//!
//! Provides seniority-based recovery distributions, collateral-waterfall
//! workout LGD, downturn LGD adjustments, and EAD computation.
//!
//! # Module Organization
//!
//! - [`seniority`]: Beta-distributed recovery
//!   by debt seniority class.
//! - [`workout`]: Collateral-first recovery
//!   waterfall with costs and time-to-resolution discounting.
//! - [`downturn`]: Frye-Jacobs and
//!   regulatory-floor downturn LGD adjustments.
//! - [`ead`]: Exposure at default with Credit
//!   Conversion Factors.

pub mod downturn;
pub mod ead;
pub mod seniority;
pub mod workout;

pub use downturn::{DownturnLgd, DownturnMethod};
pub use ead::{CreditConversionFactor, EadCalculator};
pub use seniority::{BetaRecovery, SeniorityCalibration, SeniorityClass, SeniorityRecovery};
pub use workout::{
    CollateralPiece, WorkoutCollateralType, WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder,
    WorkoutLgdResult,
};

/// Return historical recovery distribution parameters for a seniority class.
///
/// # Arguments
///
/// * `seniority` - Debt-seniority label accepted by [`SeniorityClass`], such as
///   `senior_secured`.
/// * `rating_agency` - Calibration-provider name accepted by
///   [`SeniorityCalibration::from_agency`].
///
/// # Errors
/// Returns an error if the seniority class or rating agency name is unknown.
pub fn seniority_recovery_stats(
    seniority: &str,
    rating_agency: &str,
) -> finstack_quant_core::Result<BetaRecovery> {
    let class = seniority.parse::<SeniorityClass>()?;
    let calibration = SeniorityCalibration::from_agency(rating_agency)?;
    calibration.get(class).copied().ok_or_else(|| {
        finstack_quant_core::Error::Validation("seniority not in calibration".into())
    })
}

/// Return recovery distribution parameters from the registry default seniority calibration.
///
/// # Arguments
///
/// * `seniority` - Debt-seniority label accepted by [`SeniorityClass`], such as
///   `senior_secured`.
///
/// # Errors
/// Returns an error if the seniority class is unknown or absent from the
/// registry default calibration.
pub fn seniority_recovery_stats_default(
    seniority: &str,
) -> finstack_quant_core::Result<BetaRecovery> {
    let class = seniority.parse::<SeniorityClass>()?;
    let calibration = SeniorityCalibration::moodys_historical()?;
    calibration.get(class).copied().ok_or_else(|| {
        finstack_quant_core::Error::Validation("seniority not in calibration".into())
    })
}

/// Compute workout net recovery, LGD, and recovery rate from collateral specs.
///
/// Each collateral tuple is `(type_name, book_value, haircut)`.
///
/// # Arguments
///
/// * `ead` - Exposure at default in the same monetary units as collateral book
///   values. It must be valid for the selected workout model.
/// * `collateral` - Owned collateral specifications of `(type_name, book_value,
///   haircut)`, where `haircut` is a decimal fraction deducted from value.
/// * `direct_cost_pct` - Direct workout costs as a decimal fraction of EAD.
/// * `indirect_cost_pct` - Indirect workout costs as a decimal fraction of EAD.
/// * `time_to_resolution_years` - Expected workout horizon in years.
/// * `discount_rate` - Annual decimal rate used to discount recoveries over the
///   workout horizon.
///
/// # Errors
/// Returns an error if any collateral type or model input is invalid.
pub fn workout_lgd(
    ead: f64,
    collateral: Vec<(String, f64, f64)>,
    direct_cost_pct: f64,
    indirect_cost_pct: f64,
    time_to_resolution_years: f64,
    discount_rate: f64,
) -> finstack_quant_core::Result<WorkoutLgdResult> {
    let pieces = collateral
        .into_iter()
        .map(|(type_name, value, haircut)| {
            let collateral_type = type_name.parse::<WorkoutCollateralType>()?;
            CollateralPiece::new(collateral_type, value, haircut)
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;

    let costs = WorkoutCosts::new(direct_cost_pct, indirect_cost_pct)?;
    let model = WorkoutLgd::builder()
        .collateral_pieces(pieces)
        .workout_years(time_to_resolution_years)
        .discount_rate(discount_rate)
        .costs(costs)
        .build()?;

    model.evaluate(ead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seniority_recovery_stats_accepts_binding_strings() {
        let stats = seniority_recovery_stats("senior_secured", "s&p").unwrap();
        assert!((stats.mean() - 0.53).abs() < 1e-12);
    }

    #[test]
    fn seniority_recovery_stats_default_uses_registry_default() {
        let stats = seniority_recovery_stats_default("senior_secured").unwrap();
        let explicit = seniority_recovery_stats("senior_secured", "moodys").unwrap();
        assert!((stats.mean() - explicit.mean()).abs() < 1e-12);
        assert!((stats.std_dev() - explicit.std_dev()).abs() < 1e-12);
    }

    #[test]
    fn beta_recovery_sample_is_seeded() {
        let first = BetaRecovery::new(0.4, 0.2)
            .unwrap()
            .sample_seeded(4, 42)
            .unwrap();
        let second = BetaRecovery::new(0.4, 0.2)
            .unwrap()
            .sample_seeded(4, 42)
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn workout_lgd_returns_net_recovery_and_lgd() {
        let WorkoutLgdResult {
            net_recovery,
            lgd,
            recovery_rate,
        } = workout_lgd(
            100.0,
            vec![("real_estate".to_string(), 80.0, 0.30)],
            0.05,
            0.03,
            2.0,
            0.05,
        )
        .unwrap();

        // net = (gross − costs·EAD) · DF = (56 − 8) / 1.05² — workout costs
        // are discounted alongside recoveries per the Basel workout-LGD
        // methodology ().
        let expected_net = 48.0 / (1.05_f64 * 1.05);
        assert!((net_recovery - expected_net).abs() < 1e-12);
        assert!((lgd - (1.0 - expected_net / 100.0)).abs() < 1e-12);
        assert!((recovery_rate - (1.0 - lgd)).abs() < 1e-12);
    }

    #[test]
    fn workout_lgd_rejects_noncanonical_collateral_type() {
        assert!(workout_lgd(
            100.0,
            vec![("real-estate".to_string(), 80.0, 0.30)],
            0.05,
            0.03,
            2.0,
            0.05,
        )
        .is_err());
    }
}
