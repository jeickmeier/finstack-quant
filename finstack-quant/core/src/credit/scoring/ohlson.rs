//! Ohlson O-Score logistic bankruptcy prediction model (1980).
//!
//! The O-Score is a nine-predictor logistic regression model that estimates
//! the probability of bankruptcy within two years. Unlike the Altman Z-Score
//! (which uses discriminant analysis), the O-Score directly produces a
//! probability via the logistic transform.
//!
//! # References
//!
//! Ohlson, J. A. (1980). "Financial Ratios and the Probabilistic Prediction
//! of Bankruptcy." *Journal of Accounting Research*, 18(1), 109-131.

use serde::{Deserialize, Serialize};

use super::types::{check_finite, CreditScoringError, ScoringResult, ScoringZone};

/// Ohlson's published optimal probability cutoff P* = 0.038 (O ≈ −3.23),
/// the Distress zone boundary. Source: Ohlson (1980), Table 6 — the cutoff
/// minimizing total misclassification on the estimation sample.
pub const OHLSON_OPTIMAL_CUTOFF_PD: f64 = 0.038;

/// Safe/Grey boundary at P*/2 (house convention caution band below
/// Ohlson's optimal cutoff).
pub const OHLSON_GREY_PD: f64 = OHLSON_OPTIMAL_CUTOFF_PD / 2.0;

/// Input for the Ohlson O-Score logistic model (1980).
///
/// Nine predictors capturing size, leverage, performance, and liquidity.
///
/// # References
///
/// Ohlson, J. A. (1980). "Financial Ratios and the Probabilistic Prediction
/// of Bankruptcy." *Journal of Accounting Research*, 18(1), 109-131.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OhlsonOScoreInput {
    /// Ohlson's SIZE variable: `ln(Total Assets / GNP price-level index)`.
    ///
    /// The published coefficient `-0.407` is calibrated to this specific
    /// scale using the US GNP deflator with Ohlson's 1970s base period
    /// (Ohlson 1980, Table 4). The deflator normalises for inflation so
    /// firms are compared on a constant-dollar basis.
    ///
    /// # Unit sensitivity (IMPORTANT)
    ///
    /// Because the coefficient is calibrated to Ohlson's specific scale,
    /// substituting a different unit shifts the score by a constant and
    /// therefore shifts every implied PD and every zone boundary. For
    /// example, `ln(total_assets_millions)` differs from Ohlson's SIZE
    /// by `+ ln(10^6 / GNP_deflator_1970s)`, which multiplied by
    /// `-0.407` shifts the O-score by roughly `-2.8` - i.e. every firm
    /// looks ~60%+ "safer" than it should.
    ///
    /// Recommended inputs, in order of fidelity:
    /// 1. `ln(total_assets_nominal_USD / GNP_deflator)` using a deflator
    ///    with an explicit base period close to 1968-70. This reproduces
    ///    Ohlson's original scale.
    /// 2. `ln(total_assets_deflated_to_Ohlson_base_USD)` if you have
    ///    pre-deflated real dollars.
    /// 3. Any other rescaling, together with a re-estimated intercept and
    ///    re-calibrated zone thresholds on your own sample.
    ///
    /// Do **not** feed raw `ln(total_assets_millions)` or
    /// `ln(total_assets_billions)` unless you have re-estimated the model
    /// - the out-of-the-box thresholds will mis-rank.
    pub log_total_assets_adjusted: f64,
    /// Total Liabilities / Total Assets.
    pub total_liabilities_to_total_assets: f64,
    /// Working Capital / Total Assets.
    pub working_capital_to_total_assets: f64,
    /// Current Liabilities / Current Assets.
    pub current_liabilities_to_current_assets: f64,
    /// 1 if Total Liabilities > Total Assets, else 0.
    pub liabilities_exceed_assets: f64,
    /// Net Income / Total Assets (ROA).
    pub net_income_to_total_assets: f64,
    /// Funds from Operations / Total Liabilities.
    pub funds_from_operations_to_total_liabilities: f64,
    /// 1 if net income was negative for the last two years, else 0.
    pub negative_net_income_two_years: f64,
    /// (NI_t - NI_{t-1}) / (|NI_t| + |NI_{t-1}|) -- change in net income.
    pub net_income_change: f64,
}

/// Compute the Ohlson O-Score.
///
/// O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4
///     - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9
///
/// PD = 1 / (1 + exp(-O))    (logistic transform)
///
/// Zone classification on implied PD (mirrors the PD-based zoning used by
/// [`zmijewski_score`](super::zmijewski_score)):
/// - PD < 0.019: Safe
/// - 0.019 <= PD <= 0.038: Grey
/// - PD > 0.038: Distress
///
/// The Distress boundary is Ohlson's published optimal cutoff P* = 0.038
/// (O ≈ −3.23), which minimized total misclassification in Ohlson (1980,
/// Table 6). The Safe/Grey boundary at P*/2 = 0.019 is a house convention
/// creating a caution band below the cutoff. (Re-based per the 2026-06-09
/// the previous raw-O cutoffs {0.38, 0.50} labeled
/// PD ≈ 59% firms "Safe" and were unrelated to Ohlson's published cutoff.)
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input is NaN or infinite.
///
/// # Examples
///
/// A healthy firm: low leverage, positive earnings, no recent losses.
///
/// **Note on `log_total_assets_adjusted`:** Ohlson's published `-0.407`
/// coefficient is calibrated to a specific scale (1970s GNP-deflated USD,
/// log-transformed). See [`OhlsonOScoreInput::log_total_assets_adjusted`]
/// for the recommended input shape.
///
/// ```
/// use finstack_quant_core::credit::scoring::{ohlson_o_score, OhlsonOScoreInput, ScoringZone};
///
/// let healthy = OhlsonOScoreInput {
///     log_total_assets_adjusted: 12.0,
///     total_liabilities_to_total_assets: 0.40,
///     working_capital_to_total_assets: 0.20,
///     current_liabilities_to_current_assets: 0.50,
///     liabilities_exceed_assets: 0.0,
///     net_income_to_total_assets: 0.08,
///     funds_from_operations_to_total_liabilities: 0.30,
///     negative_net_income_two_years: 0.0,
///     net_income_change: 0.10,
/// };
/// let result = ohlson_o_score(&healthy)?;
/// // Well below Ohlson's optimal cutoff O ≈ −3.23 (P* = 0.038)
/// assert!(result.score < -3.23);
/// assert_eq!(result.zone, ScoringZone::Safe);
/// // Implied PD via logistic transform: 1 / (1 + exp(-O))
/// assert!(result.implied_pd < 0.019);
/// # Ok::<_, finstack_quant_core::credit::scoring::CreditScoringError>(())
/// ```
pub fn ohlson_o_score(input: &OhlsonOScoreInput) -> Result<ScoringResult, CreditScoringError> {
    check_finite("log_total_assets_adjusted", input.log_total_assets_adjusted)?;
    check_finite(
        "total_liabilities_to_total_assets",
        input.total_liabilities_to_total_assets,
    )?;
    check_finite(
        "working_capital_to_total_assets",
        input.working_capital_to_total_assets,
    )?;
    check_finite(
        "current_liabilities_to_current_assets",
        input.current_liabilities_to_current_assets,
    )?;
    check_finite("liabilities_exceed_assets", input.liabilities_exceed_assets)?;
    check_finite(
        "net_income_to_total_assets",
        input.net_income_to_total_assets,
    )?;
    check_finite(
        "funds_from_operations_to_total_liabilities",
        input.funds_from_operations_to_total_liabilities,
    )?;
    check_finite(
        "negative_net_income_two_years",
        input.negative_net_income_two_years,
    )?;
    check_finite("net_income_change", input.net_income_change)?;

    let o = -1.32 - 0.407 * input.log_total_assets_adjusted
        + 6.03 * input.total_liabilities_to_total_assets
        - 1.43 * input.working_capital_to_total_assets
        + 0.0757 * input.current_liabilities_to_current_assets
        - 1.72 * input.liabilities_exceed_assets
        - 2.37 * input.net_income_to_total_assets
        - 1.83 * input.funds_from_operations_to_total_liabilities
        + 0.285 * input.negative_net_income_two_years
        - 0.521 * input.net_income_change;

    // Logistic transform: PD = 1 / (1 + exp(-O))
    let implied_pd = logistic(o);

    // Zone classification based on implied PD (consistent with zmijewski.rs).
    // Distress boundary: Ohlson's published optimal cutoff P* = 0.038
    // (O ≈ −3.23). Safe/Grey boundary: P*/2 (house convention caution band).
    let zone = if implied_pd < OHLSON_GREY_PD {
        ScoringZone::Safe
    } else if implied_pd > OHLSON_OPTIMAL_CUTOFF_PD {
        ScoringZone::Distress
    } else {
        ScoringZone::Grey
    };

    Ok(ScoringResult {
        score: o,
        zone,
        implied_pd,
        model: "Ohlson O-Score (1980)",
    })
}

/// Standard logistic function: 1 / (1 + exp(-x)).
///
/// Numerically stable for large |x|.
fn logistic(x: f64) -> f64 {
    if x >= 0.0 {
        let e = (-x).exp();
        1.0 / (1.0 + e)
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
