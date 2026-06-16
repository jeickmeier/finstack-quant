//! Zmijewski (1984) probit bankruptcy prediction model.
//!
//! A three-predictor probit model capturing profitability (ROA),
//! leverage (debt ratio), and liquidity (current ratio). The probit
//! link function maps the linear predictor to a probability via the
//! standard normal CDF.
//!
//! # References
//!
//! Zmijewski, M. E. (1984). "Methodological Issues Related to the Estimation
//! of Financial Distress Prediction Models." *Journal of Accounting Research*,
//! 22, 59-82.

use serde::{Deserialize, Serialize};

use crate::math::norm_cdf;

use super::types::{check_finite, CreditScoringError, ScoringResult, ScoringZone};

/// Input for the Zmijewski (1984) probit bankruptcy prediction model.
///
/// # References
///
/// Zmijewski, M. E. (1984). "Methodological Issues Related to the Estimation
/// of Financial Distress Prediction Models." *Journal of Accounting Research*,
/// 22, 59-82.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ZmijewskiInput {
    /// Net Income / Total Assets (ROA).
    pub net_income_to_total_assets: f64,
    /// Total Liabilities / Total Assets (leverage / financial leverage ratio).
    pub total_liabilities_to_total_assets: f64,
    /// Current Assets / Current Liabilities (current ratio / liquidity).
    pub current_assets_to_current_liabilities: f64,
}

/// Compute the Zmijewski probit score.
///
/// ```text
/// Y = -4.336 - 4.513 * X1 + 5.679 * X2 + 0.004 * X3
/// PD = Phi(Y)
/// ```
///
/// Zone classification on implied PD:
/// - PD < 0.10: Safe
/// - 0.10 <= PD <= 0.50: Grey
/// - PD > 0.50: Distress
///
/// # Coefficient convention
///
/// This uses Zmijewski's originally published 1984 coefficients, including
/// the counterintuitively positive coefficient (+0.004) on the current
/// ratio (CACL). Zmijewski noted this sign in the original paper; it is
/// economically weak (higher liquidity should reduce distress probability)
/// but statistically insignificant in the original sample. Many subsequent
/// replications (e.g. Grice & Dugan 2003) refit the model and report a
/// small negative coefficient. If your calibration target is a refit
/// version rather than the original paper, negate the CACL term.
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input is NaN or infinite.
pub fn zmijewski_score(input: &ZmijewskiInput) -> Result<ScoringResult, CreditScoringError> {
    check_finite(
        "net_income_to_total_assets",
        input.net_income_to_total_assets,
    )?;
    check_finite(
        "total_liabilities_to_total_assets",
        input.total_liabilities_to_total_assets,
    )?;
    check_finite(
        "current_assets_to_current_liabilities",
        input.current_assets_to_current_liabilities,
    )?;

    let y = -4.336 - 4.513 * input.net_income_to_total_assets
        + 5.679 * input.total_liabilities_to_total_assets
        + 0.004 * input.current_assets_to_current_liabilities;

    // Probit transform: PD = Phi(Y)
    let implied_pd = norm_cdf(y);

    // Zone classification based on PD
    let zone = if implied_pd < 0.10 {
        ScoringZone::Safe
    } else if implied_pd > 0.50 {
        ScoringZone::Distress
    } else {
        ScoringZone::Grey
    };

    Ok(ScoringResult {
        score: y,
        zone,
        implied_pd,
        model: "Zmijewski Probit (1984)",
    })
}
