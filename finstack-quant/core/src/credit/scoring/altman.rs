//! Altman Z-Score family: original (1968), Z'-Score (private firms),
//! and Z''-Score (non-manufacturing firms; the emerging-market EM-Score
//! variant with the +3.25 constant is not implemented).
//!
//! # References
//!
//! - Altman, E. I. (1968). "Financial Ratios, Discriminant Analysis and the
//!   Prediction of Corporate Bankruptcy." *Journal of Finance*, 23(4), 589-609.
//! - Altman, E. I. (2002). "Revisiting Credit Scoring Models in a Basel 2
//!   Environment." Working paper.
//! - Altman, E. I. (2005). "An Emerging Market Credit Scoring System for
//!   Corporate Bonds." *Emerging Markets Review*, 6(4), 311-323.

use serde::{Deserialize, Serialize};

use super::types::{check_finite, CreditScoringError, ScoringResult, ScoringZone};

/// Explicit, versioned mappings from an Altman score to a PD-like heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum AltmanPdCalibration {
    /// Legacy piecewise mapping retained for compatibility.
    ///
    /// This is an uncalibrated house heuristic, not an empirical Altman
    /// bankruptcy-probability calibration.
    HeuristicV1,
}

// ---------------------------------------------------------------------------
// Input structs
// ---------------------------------------------------------------------------

/// Input ratios for the original Altman Z-Score (1968).
///
/// Designed for publicly traded manufacturing firms. Uses market value
/// of equity in the X4 ratio.
///
/// # References
///
/// Altman, E. I. (1968). "Financial Ratios, Discriminant Analysis and the
/// Prediction of Corporate Bankruptcy." *Journal of Finance*, 23(4), 589-609.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AltmanZScoreInput {
    /// X1: Working Capital / Total Assets.
    pub working_capital_to_total_assets: f64,
    /// X2: Retained Earnings / Total Assets.
    pub retained_earnings_to_total_assets: f64,
    /// X3: EBIT / Total Assets.
    pub ebit_to_total_assets: f64,
    /// X4: Market Value of Equity / Book Value of Total Liabilities.
    pub market_equity_to_total_liabilities: f64,
    /// X5: Sales / Total Assets.
    pub sales_to_total_assets: f64,
}

/// Input ratios for the Altman Z'-Score (private firms).
///
/// Replaces market equity with book equity in X4. Coefficients are
/// re-estimated for the private-firm sample.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AltmanZPrimeInput {
    /// X1: Working Capital / Total Assets.
    pub working_capital_to_total_assets: f64,
    /// X2: Retained Earnings / Total Assets.
    pub retained_earnings_to_total_assets: f64,
    /// X3: EBIT / Total Assets.
    pub ebit_to_total_assets: f64,
    /// X4: Book Value of Equity / Book Value of Total Liabilities.
    pub book_equity_to_total_liabilities: f64,
    /// X5: Sales / Total Assets.
    pub sales_to_total_assets: f64,
}

/// Input ratios for the Altman Z''-Score (non-manufacturing firms).
///
/// Drops the Sales/Total Assets ratio to remove industry bias. The
/// implemented model is the constant-free non-EM Z''; the emerging-market
/// "EM-Score" (+3.25 constant, cutoffs 5.85/4.35) is not implemented.
///
/// # References
///
/// - Altman, E. I. (1993). *Corporate Financial Distress and Bankruptcy*.
///   Wiley. (Four-variable Z'' for non-manufacturers.)
/// - Altman, E. I. (2005). "An Emerging Market Credit Scoring System for
///   Corporate Bonds." *Emerging Markets Review*, 6(4), 311-323. (EM-Score
///   variant, not implemented.)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AltmanZDoublePrimeInput {
    /// X1: Working Capital / Total Assets.
    pub working_capital_to_total_assets: f64,
    /// X2: Retained Earnings / Total Assets.
    pub retained_earnings_to_total_assets: f64,
    /// X3: EBIT / Total Assets.
    pub ebit_to_total_assets: f64,
    /// X4: Book Value of Equity / Book Value of Total Liabilities.
    pub book_equity_to_total_liabilities: f64,
}

// ---------------------------------------------------------------------------
// Scoring functions
// ---------------------------------------------------------------------------

/// Compute the original Altman Z-Score (1968).
///
/// Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5
///
/// Zone cutoffs:
/// - Z > 2.99: Safe
/// - 1.81 <= Z <= 2.99: Grey
/// - Z < 1.81: Distress
///
/// The canonical result does not contain an implied PD. Use
/// [`altman_z_score_with_pd`] and explicitly select a versioned heuristic if
/// a non-empirical score-to-PD mapping is required.
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input ratio is NaN or infinite.
///
/// # Examples
///
/// A healthy public manufacturing firm scores in the Safe zone:
///
/// ```
/// use finstack_quant_core::credit::scoring::{altman_z_score, AltmanZScoreInput, ScoringZone};
///
/// let healthy = AltmanZScoreInput {
///     working_capital_to_total_assets: 0.20,
///     retained_earnings_to_total_assets: 0.30,
///     ebit_to_total_assets: 0.15,
///     market_equity_to_total_liabilities: 1.50,
///     sales_to_total_assets: 1.00,
/// };
/// let result = altman_z_score(&healthy)?;
/// assert!(result.score > 2.99);
/// assert_eq!(result.zone, ScoringZone::Safe);
/// assert_eq!(result.implied_pd, None);
/// # Ok::<_, finstack_quant_core::credit::scoring::CreditScoringError>(())
/// ```
///
/// A distressed firm with weak earnings and leverage scores in Distress:
///
/// ```
/// use finstack_quant_core::credit::scoring::{altman_z_score, AltmanZScoreInput, ScoringZone};
///
/// let distressed = AltmanZScoreInput {
///     working_capital_to_total_assets: -0.10,
///     retained_earnings_to_total_assets: -0.20,
///     ebit_to_total_assets: -0.05,
///     market_equity_to_total_liabilities: 0.20,
///     sales_to_total_assets: 0.50,
/// };
/// let result = altman_z_score(&distressed)?;
/// assert!(result.score < 1.81);
/// assert_eq!(result.zone, ScoringZone::Distress);
/// # Ok::<_, finstack_quant_core::credit::scoring::CreditScoringError>(())
/// ```
///
/// # Arguments
///
/// * `input` - Finite public-company accounting ratios for the original
///   five-factor 1968 Altman Z-Score model.
pub fn altman_z_score(input: &AltmanZScoreInput) -> Result<ScoringResult, CreditScoringError> {
    check_finite(
        "working_capital_to_total_assets",
        input.working_capital_to_total_assets,
    )?;
    check_finite(
        "retained_earnings_to_total_assets",
        input.retained_earnings_to_total_assets,
    )?;
    check_finite("ebit_to_total_assets", input.ebit_to_total_assets)?;
    check_finite(
        "market_equity_to_total_liabilities",
        input.market_equity_to_total_liabilities,
    )?;
    check_finite("sales_to_total_assets", input.sales_to_total_assets)?;

    let z = 1.2 * input.working_capital_to_total_assets
        + 1.4 * input.retained_earnings_to_total_assets
        + 3.3 * input.ebit_to_total_assets
        + 0.6 * input.market_equity_to_total_liabilities
        + 1.0 * input.sales_to_total_assets;

    let zone = z_score_zone(z, 2.99, 1.81);
    Ok(ScoringResult {
        score: z,
        zone,
        implied_pd: None,
        model: "Altman Z-Score (1968)",
    })
}

/// Compute the original Altman Z-Score and apply an explicit PD heuristic.
///
/// # Arguments
///
/// * `input` - Finite public-company accounting ratios for the original
///   five-factor 1968 Altman Z-Score model.
/// * `calibration` - Explicit score-to-probability-of-default mapping applied
///   after the model score and zone are calculated.
pub fn altman_z_score_with_pd(
    input: &AltmanZScoreInput,
    calibration: AltmanPdCalibration,
) -> Result<ScoringResult, CreditScoringError> {
    let mut result = altman_z_score(input)?;
    result.implied_pd = Some(calibration.map(result.score, 2.99, 1.81));
    Ok(result)
}

/// Compute the Altman Z'-Score for private firms.
///
/// Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5
///
/// Zone cutoffs:
/// - Z' > 2.90: Safe
/// - 1.23 <= Z' <= 2.90: Grey
/// - Z' < 1.23: Distress
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input ratio is NaN or infinite.
///
/// # Examples
///
/// A healthy private manufacturing firm. Note that X4 uses *book* equity
/// (rather than market equity) since private firms have no market price:
///
/// ```
/// use finstack_quant_core::credit::scoring::{altman_z_prime, AltmanZPrimeInput, ScoringZone};
///
/// let healthy = AltmanZPrimeInput {
///     working_capital_to_total_assets: 0.30,
///     retained_earnings_to_total_assets: 0.40,
///     ebit_to_total_assets: 0.20,
///     book_equity_to_total_liabilities: 2.00,
///     sales_to_total_assets: 1.20,
/// };
/// let result = altman_z_prime(&healthy)?;
/// assert!(result.score > 2.90);
/// assert_eq!(result.zone, ScoringZone::Safe);
/// # Ok::<_, finstack_quant_core::credit::scoring::CreditScoringError>(())
/// ```
///
/// # Arguments
///
/// * `input` - Finite private-company accounting ratios for the five-factor
///   Z' model, including book rather than market equity.
pub fn altman_z_prime(input: &AltmanZPrimeInput) -> Result<ScoringResult, CreditScoringError> {
    check_finite(
        "working_capital_to_total_assets",
        input.working_capital_to_total_assets,
    )?;
    check_finite(
        "retained_earnings_to_total_assets",
        input.retained_earnings_to_total_assets,
    )?;
    check_finite("ebit_to_total_assets", input.ebit_to_total_assets)?;
    check_finite(
        "book_equity_to_total_liabilities",
        input.book_equity_to_total_liabilities,
    )?;
    check_finite("sales_to_total_assets", input.sales_to_total_assets)?;

    let z = 0.717 * input.working_capital_to_total_assets
        + 0.847 * input.retained_earnings_to_total_assets
        + 3.107 * input.ebit_to_total_assets
        + 0.420 * input.book_equity_to_total_liabilities
        + 0.998 * input.sales_to_total_assets;

    let zone = z_score_zone(z, 2.90, 1.23);
    Ok(ScoringResult {
        score: z,
        zone,
        implied_pd: None,
        model: "Altman Z'-Score (Private)",
    })
}

/// Compute the Altman Z'-Score and apply an explicit PD heuristic.
///
/// # Arguments
///
/// * `input` - Finite private-company accounting ratios for the five-factor
///   Z' model, including book rather than market equity.
/// * `calibration` - Explicit score-to-probability-of-default mapping applied
///   after the model score and zone are calculated.
pub fn altman_z_prime_with_pd(
    input: &AltmanZPrimeInput,
    calibration: AltmanPdCalibration,
) -> Result<ScoringResult, CreditScoringError> {
    let mut result = altman_z_prime(input)?;
    result.implied_pd = Some(calibration.map(result.score, 2.90, 1.23));
    Ok(result)
}

/// Compute the Altman Z''-Score for non-manufacturing firms.
///
/// Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4
///
/// Zone cutoffs:
/// - Z'' > 2.60: Safe
/// - 1.10 <= Z'' <= 2.60: Grey
/// - Z'' < 1.10: Distress
///
/// This is the non-emerging-market four-variable Z'' model (Altman 1993;
/// Altman, Hartzell & Peck 1995), whose 2.60 / 1.10 zone cutoffs are
/// defined on the constant-free scale. The emerging-market "EM-Score"
/// variant, which adds a +3.25 constant and uses cutoffs
/// Safe > 5.85 / Distress < 4.35, is *not* implemented.
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input ratio is NaN or infinite.
///
/// # Examples
///
/// The Z''-Score drops the Sales/Total Assets ratio (X5) to remove industry bias,
/// making it suitable for non-manufacturing firms:
///
/// ```
/// use finstack_quant_core::credit::scoring::{altman_z_double_prime, AltmanZDoublePrimeInput, ScoringZone};
///
/// let healthy = AltmanZDoublePrimeInput {
///     working_capital_to_total_assets: 0.20,
///     retained_earnings_to_total_assets: 0.30,
///     ebit_to_total_assets: 0.15,
///     book_equity_to_total_liabilities: 1.20,
/// };
/// let result = altman_z_double_prime(&healthy)?;
/// assert!(result.score > 2.60);
/// assert_eq!(result.zone, ScoringZone::Safe);
/// # Ok::<_, finstack_quant_core::credit::scoring::CreditScoringError>(())
/// ```
///
/// # Arguments
///
/// * `input` - Finite non-manufacturing-company accounting ratios for the
///   four-factor constant-free Z'' model.
pub fn altman_z_double_prime(
    input: &AltmanZDoublePrimeInput,
) -> Result<ScoringResult, CreditScoringError> {
    check_finite(
        "working_capital_to_total_assets",
        input.working_capital_to_total_assets,
    )?;
    check_finite(
        "retained_earnings_to_total_assets",
        input.retained_earnings_to_total_assets,
    )?;
    check_finite("ebit_to_total_assets", input.ebit_to_total_assets)?;
    check_finite(
        "book_equity_to_total_liabilities",
        input.book_equity_to_total_liabilities,
    )?;

    // Non-EM Z'' has no constant term; the +3.25 constant belongs to the
    // EM-Score variant with cutoffs 5.85/4.35 (see 2026-06-09 quant review).
    let z = 6.56 * input.working_capital_to_total_assets
        + 3.26 * input.retained_earnings_to_total_assets
        + 6.72 * input.ebit_to_total_assets
        + 1.05 * input.book_equity_to_total_liabilities;

    let zone = z_score_zone(z, 2.60, 1.10);
    Ok(ScoringResult {
        score: z,
        zone,
        implied_pd: None,
        model: "Altman Z''-Score (Non-Manufacturer)",
    })
}

/// Compute the Altman Z''-Score and apply an explicit PD heuristic.
///
/// # Arguments
///
/// * `input` - Finite non-manufacturing-company accounting ratios for the
///   four-factor constant-free Z'' model.
/// * `calibration` - Explicit score-to-probability-of-default mapping applied
///   after the model score and zone are calculated.
pub fn altman_z_double_prime_with_pd(
    input: &AltmanZDoublePrimeInput,
    calibration: AltmanPdCalibration,
) -> Result<ScoringResult, CreditScoringError> {
    let mut result = altman_z_double_prime(input)?;
    result.implied_pd = Some(calibration.map(result.score, 2.60, 1.10));
    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Classify a Z-score into Safe/Grey/Distress zones.
fn z_score_zone(z: f64, safe_threshold: f64, distress_threshold: f64) -> ScoringZone {
    if z > safe_threshold {
        ScoringZone::Safe
    } else if z < distress_threshold {
        ScoringZone::Distress
    } else {
        ScoringZone::Grey
    }
}

impl AltmanPdCalibration {
    fn map(self, z: f64, safe_threshold: f64, distress_threshold: f64) -> f64 {
        match self {
            Self::HeuristicV1 => z_score_heuristic_v1(z, safe_threshold, distress_threshold),
        }
    }
}

/// Legacy uncalibrated house heuristic. It is not an empirical Altman mapping.
fn z_score_heuristic_v1(z: f64, safe_threshold: f64, distress_threshold: f64) -> f64 {
    const PD_SAFE: f64 = 0.01;
    const PD_DISTRESS: f64 = 0.50;

    if z > safe_threshold {
        // Deep safe: use exponential decay toward zero
        let excess = z - safe_threshold;
        PD_SAFE * (-0.5 * excess).exp()
    } else if z < distress_threshold {
        // Deep distress: increase toward cap
        let deficit = distress_threshold - z;
        (PD_DISTRESS + (1.0 - PD_DISTRESS) * (1.0 - (-0.5 * deficit).exp())).min(0.99)
    } else {
        // Grey zone: linear interpolation
        let range = safe_threshold - distress_threshold;
        let t = (safe_threshold - z) / range;
        PD_SAFE + t * (PD_DISTRESS - PD_SAFE)
    }
}
