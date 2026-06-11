//! Loss Given Default modeling primitives.
//!
//! Provides seniority-based recovery distributions, collateral-waterfall
//! workout LGD, downturn LGD adjustments, and EAD computation.
//!
//! # Module Organization
//!
//! - [`seniority`][crate::credit::lgd::seniority]: Beta-distributed recovery
//!   by debt seniority class.
//! - [`workout`][crate::credit::lgd::workout]: Collateral-first recovery
//!   waterfall with costs and time-to-resolution discounting.
//! - [`downturn`][crate::credit::lgd::downturn]: Frye-Jacobs and
//!   regulatory-floor downturn LGD adjustments.
//! - [`ead`][crate::credit::lgd::ead]: Exposure at default with Credit
//!   Conversion Factors.

pub mod downturn;
pub mod ead;
pub mod seniority;
pub mod workout;

pub use downturn::{DownturnLgd, DownturnMethod};
pub use ead::{CreditConversionFactor, EadCalculator};
pub use seniority::{BetaRecovery, SeniorityCalibration, SeniorityClass, SeniorityRecovery};
pub use workout::{CollateralPiece, CollateralType, WorkoutCosts, WorkoutLgd, WorkoutLgdBuilder};

/// Return historical recovery distribution parameters for a seniority class.
///
/// # Errors
/// Returns an error if the seniority class or rating agency name is unknown.
pub fn seniority_recovery_stats(
    seniority: &str,
    rating_agency: &str,
) -> crate::Result<BetaRecovery> {
    let class = seniority.parse::<SeniorityClass>()?;
    let calibration = SeniorityCalibration::from_agency(rating_agency)?;
    calibration
        .get(class)
        .copied()
        .ok_or_else(|| crate::Error::Validation("seniority not in calibration".into()))
}

/// Return recovery distribution parameters from the registry default seniority calibration.
///
/// # Errors
/// Returns an error if the seniority class is unknown or absent from the
/// registry default calibration.
pub fn seniority_recovery_stats_default(seniority: &str) -> crate::Result<BetaRecovery> {
    let class = seniority.parse::<SeniorityClass>()?;
    let calibration = SeniorityCalibration::moodys_historical()?;
    calibration
        .get(class)
        .copied()
        .ok_or_else(|| crate::Error::Validation("seniority not in calibration".into()))
}

/// Draw recovery rates from a Beta distribution with a deterministic seed.
///
/// # Errors
/// Returns an error if the mean or standard deviation cannot parameterize a
/// valid Beta recovery distribution, or if sampling fails.
pub fn beta_recovery_sample(
    mean: f64,
    std: f64,
    n_samples: usize,
    seed: u64,
) -> crate::Result<Vec<f64>> {
    BetaRecovery::new(mean, std)?.sample_seeded(n_samples, seed)
}

/// Return the value at quantile `q` for a Beta recovery distribution.
///
/// # Errors
/// Returns an error if the mean or standard deviation cannot parameterize a
/// valid Beta recovery distribution, or if the quantile evaluation fails.
pub fn beta_recovery_quantile(mean: f64, std: f64, q: f64) -> crate::Result<f64> {
    BetaRecovery::new(mean, std)?.quantile(q)
}

/// Compute workout net recovery and LGD from collateral specs.
///
/// Each collateral tuple is `(type_name, book_value, haircut)`.
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
) -> crate::Result<(f64, f64)> {
    let pieces = collateral
        .into_iter()
        .map(|(type_name, value, haircut)| {
            let collateral_type = type_name.parse::<CollateralType>()?;
            CollateralPiece::new(collateral_type, value, haircut)
        })
        .collect::<crate::Result<Vec<_>>>()?;

    let costs = WorkoutCosts::new(direct_cost_pct, indirect_cost_pct)?;
    let model = WorkoutLgd::builder()
        .collateral_pieces(pieces)
        .workout_years(time_to_resolution_years)
        .discount_rate(discount_rate)
        .costs(costs)
        .build()?;

    Ok((model.net_recovery(ead)?, model.lgd(ead)?))
}

/// Apply a stressed downturn adjustment to base LGD.
///
/// Uses the proprietary mean-plus-multiple-of-Bernoulli-stdev approximation
/// (see [`DownturnMethod::FryeJacobs`](downturn::DownturnMethod::FryeJacobs)
/// for the formula and naming note — this is *not* the Frye-Jacobs (2012)
/// model):
///
/// ```text
/// LGD_downturn = LGD_base + lgd_sensitivity * sqrt(rho)
///              * Phi_inv(stress_quantile) * sqrt(LGD_base * (1 - LGD_base))
/// ```
///
/// # Arguments
///
/// * `base_lgd` - Through-the-cycle LGD in \[0, 1\].
/// * `asset_correlation` - Asset correlation rho in (0, 1). Basel: 0.12-0.24.
/// * `lgd_sensitivity` - LGD sensitivity to the systematic factor (>= 0).
///   Typical: 0.3-0.5.
/// * `stress_quantile` - Downturn quantile in (0, 1), e.g. 0.999.
///
/// # Errors
/// Returns an error if the downturn model parameters or base LGD are invalid.
pub fn downturn_lgd_stressed(
    base_lgd: f64,
    asset_correlation: f64,
    lgd_sensitivity: f64,
    stress_quantile: f64,
) -> crate::Result<f64> {
    DownturnLgd::frye_jacobs(asset_correlation, lgd_sensitivity, stress_quantile)?.adjust(base_lgd)
}

/// Apply a regulatory-floor downturn adjustment to base LGD.
///
/// # Errors
/// Returns an error if the downturn model parameters or base LGD are invalid.
pub fn downturn_lgd_regulatory_floor(base_lgd: f64, add_on: f64, floor: f64) -> crate::Result<f64> {
    DownturnLgd::regulatory_floor(add_on, floor)?.adjust(base_lgd)
}

/// Exposure at default for a fully drawn term loan.
///
/// # Errors
/// Returns an error if the principal is invalid.
pub fn ead_term_loan(principal: f64) -> crate::Result<f64> {
    Ok(EadCalculator::term_loan(principal)?.ead())
}

/// Exposure at default for a revolving facility.
///
/// # Errors
/// Returns an error if drawn, undrawn, or CCF inputs are invalid.
pub fn ead_revolver(drawn: f64, undrawn: f64, ccf: f64) -> crate::Result<f64> {
    let ccf_obj = CreditConversionFactor::new(ccf)?;
    Ok(EadCalculator::new(drawn, undrawn, ccf_obj)?.ead())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seniority_recovery_stats_accepts_binding_strings() {
        let stats = seniority_recovery_stats("senior-secured", "s&p").unwrap();
        assert!((stats.mean() - 0.53).abs() < 1e-12);
    }

    #[test]
    fn seniority_recovery_stats_default_uses_registry_default() {
        let stats = seniority_recovery_stats_default("senior-secured").unwrap();
        let explicit = seniority_recovery_stats("senior-secured", "moodys").unwrap();
        assert!((stats.mean() - explicit.mean()).abs() < 1e-12);
        assert!((stats.std_dev() - explicit.std_dev()).abs() < 1e-12);
    }

    #[test]
    fn beta_recovery_sample_is_seeded() {
        let first = beta_recovery_sample(0.4, 0.2, 4, 42).unwrap();
        let second = beta_recovery_sample(0.4, 0.2, 4, 42).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn workout_lgd_returns_net_recovery_and_lgd() {
        let (net_recovery, lgd) = workout_lgd(
            100.0,
            vec![("real-estate".to_string(), 80.0, 0.30)],
            0.05,
            0.03,
            2.0,
            0.05,
        )
        .unwrap();

        // net = (gross − costs·EAD) · DF = (56 − 8) / 1.05² — workout costs
        // are discounted alongside recoveries per the Basel workout-LGD
        // methodology (2026-06-09 core quant review; previously 56·DF − 8).
        let expected_net = 48.0 / (1.05_f64 * 1.05);
        assert!((net_recovery - expected_net).abs() < 1e-12);
        assert!((lgd - (1.0 - expected_net / 100.0)).abs() < 1e-12);
    }
}
