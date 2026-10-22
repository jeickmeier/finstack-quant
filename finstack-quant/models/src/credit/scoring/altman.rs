//! Altman Z-Score family: original (1968), Z'-Score (private firms),
//! Z''-Score (non-manufacturing firms), and EM-Score (emerging markets variant
//! with +3.25 constant; see [`altman_em_score`]).
//!
//! # References
//!
//! - Altman, E. I. (1968). "Financial Ratios, Discriminant Analysis and the
//!   Prediction of Corporate Bankruptcy." *Journal of Finance*, 23(4), 589-609. `docs/REFERENCES.md#altman-1968`
//! - Altman, E. I. (2002). "Revisiting Credit Scoring Models in a Basel 2
//!   Environment." Working paper.
//! - Altman, E. I. (2005). "An Emerging Market Credit Scoring System for
//!   Corporate Bonds." *Emerging Markets Review*, 6(4), 311-323.

use serde::{Deserialize, Serialize};

use super::types::{check_finite, CreditScoringError, ScoringResult, ScoringZone};

/// Input ratios for the original Altman Z-Score (1968).
///
/// Designed for publicly traded manufacturing firms. Uses market value
/// of equity in the X4 ratio.
///
/// # References
///
/// Altman, E. I. (1968). "Financial Ratios, Discriminant Analysis and the
/// Prediction of Corporate Bankruptcy." *Journal of Finance*, 23(4), 589-609. `docs/REFERENCES.md#altman-1968`
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// Drops the Sales/Total Assets ratio to remove industry bias. Used for both
/// the constant-free Z'' model and the emerging-market EM-Score variant
/// (see [`altman_em_score`]).
///
/// # References
///
/// - Altman, E. I. (1993). *Corporate Financial Distress and Bankruptcy*.
///   Wiley. (Four-variable Z'' for non-manufacturers.)
/// - Altman, E. I. (2005). "An Emerging Market Credit Scoring System for
///   Corporate Bonds." *Emerging Markets Review*, 6(4), 311-323. (EM-Score
///   variant with +3.25 constant.)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// Compute the original Altman Z-Score (1968).
///
/// Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5
///
/// Zone cutoffs:
/// - Z > 2.99: Safe
/// - 1.81 <= Z <= 2.99: Grey
/// - Z < 1.81: Distress
///
/// The canonical result does not contain an implied PD. Calibrate any
/// score-to-PD mapping separately against a documented population and vintage.
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
/// use finstack_quant_models::credit::scoring::{altman_z_score, AltmanZScoreInput, ScoringZone};
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
/// # Ok::<_, finstack_quant_models::credit::scoring::CreditScoringError>(())
/// ```
///
/// A distressed firm with weak earnings and leverage scores in Distress:
///
/// ```
/// use finstack_quant_models::credit::scoring::{altman_z_score, AltmanZScoreInput, ScoringZone};
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
/// # Ok::<_, finstack_quant_models::credit::scoring::CreditScoringError>(())
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
        model: "Altman Z-Score (1968)".to_string(),
    })
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
/// use finstack_quant_models::credit::scoring::{altman_z_prime, AltmanZPrimeInput, ScoringZone};
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
/// # Ok::<_, finstack_quant_models::credit::scoring::CreditScoringError>(())
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
        model: "Altman Z'-Score (Private)".to_string(),
    })
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
/// Safe > 5.85 / Distress < 4.35, is provided by [`altman_em_score`].
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
/// use finstack_quant_models::credit::scoring::{altman_z_double_prime, AltmanZDoublePrimeInput, ScoringZone};
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
/// # Ok::<_, finstack_quant_models::credit::scoring::CreditScoringError>(())
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
    // EM-Score variant with cutoffs 5.85/4.35 (implemented in altman_em_score).
    let z = 6.56 * input.working_capital_to_total_assets
        + 3.26 * input.retained_earnings_to_total_assets
        + 6.72 * input.ebit_to_total_assets
        + 1.05 * input.book_equity_to_total_liabilities;

    let zone = z_score_zone(z, 2.60, 1.10);
    Ok(ScoringResult {
        score: z,
        zone,
        implied_pd: None,
        model: "Altman Z''-Score (Non-Manufacturer)".to_string(),
    })
}

// EM-Score (emerging markets)

/// Constant added to the Z''-Score to obtain the EM-Score.
///
/// Altman, Hartzell & Peck (1995) add +3.25 so that a defaulted (D-rated)
/// issuer maps to an EM-Score near zero.
const EM_SCORE_CONSTANT: f64 = 3.25;
/// EM-Score above this value is Safe (US bond-rating equivalent ~BBB and above).
const EM_SCORE_SAFE_THRESHOLD: f64 = 5.85;
/// EM-Score below this value is Distress (US bond-rating equivalent ~BB- and below).
const EM_SCORE_DISTRESS_THRESHOLD: f64 = 4.35;

/// Compute the Altman EM-Score for emerging-market corporates.
///
/// ```text
/// EM = 3.25 + Z'' = 3.25 + 6.56*X1 + 3.26*X2 + 6.72*X3 + 1.05*X4
/// ```
///
/// Zone cutoffs (on the EM scale, i.e. Z'' cutoffs shifted by +3.25):
/// - EM > 5.85: Safe
/// - 4.35 <= EM <= 5.85: Grey
/// - EM < 4.35: Distress
///
/// # US bond rating equivalents (Altman, Hartzell & Peck 1995)
///
/// | EM-Score >= | Rating equivalent | EM-Score >= | Rating equivalent |
/// |-------------|-------------------|-------------|-------------------|
/// | 8.15        | AAA               | 5.25        | BB+               |
/// | 7.60        | AA+               | 4.95        | BB                |
/// | 7.30        | AA                | 4.75        | BB-               |
/// | 7.00        | AA-               | 4.50        | B+                |
/// | 6.85        | A+                | 4.15        | B                 |
/// | 6.65        | A                 | 3.75        | B-                |
/// | 6.40        | A-                | 3.20        | CCC+              |
/// | 6.25        | BBB+              | 2.50        | CCC               |
/// | 5.85        | BBB               | 1.75        | CCC-              |
/// | 5.65        | BBB-              | < 1.75      | D                 |
///
/// `implied_pd` is always `None`: the score-to-PD heuristic is defined on the
/// Z-score cutoffs and is not extended here.
///
/// # Errors
///
/// Returns [`CreditScoringError::NonFiniteInput`] if any input ratio is NaN or
/// infinite (delegated to [`altman_z_double_prime`]).
///
/// # Examples
///
/// ```
/// use finstack_quant_models::credit::scoring::{altman_em_score, AltmanZDoublePrimeInput, ScoringZone};
///
/// let healthy = AltmanZDoublePrimeInput {
///     working_capital_to_total_assets: 0.20,
///     retained_earnings_to_total_assets: 0.30,
///     ebit_to_total_assets: 0.15,
///     book_equity_to_total_liabilities: 1.20,
/// };
/// let result = altman_em_score(&healthy)?;
/// assert!((result.score - 7.808).abs() < 1e-12);
/// assert_eq!(result.zone, ScoringZone::Safe);
/// # Ok::<_, finstack_quant_models::credit::scoring::CreditScoringError>(())
/// ```
///
/// # References
///
/// - Altman, E. I., Hartzell, J., & Peck, M. (1995). "Emerging Markets
///   Corporate Bonds: A Scoring System." Salomon Brothers.
/// - Altman, E. I. (2005). "An Emerging Market Credit Scoring System for
///   Corporate Bonds." *Emerging Markets Review*, 6(4), 311-323.
///
/// # Arguments
///
/// * `input` - Finite accounting ratios for the four-factor Z'' model; the
///   EM-Score uses book equity in X4.
pub fn altman_em_score(
    input: &AltmanZDoublePrimeInput,
) -> Result<ScoringResult, CreditScoringError> {
    let base = altman_z_double_prime(input)?;
    let em = EM_SCORE_CONSTANT + base.score;
    let zone = z_score_zone(em, EM_SCORE_SAFE_THRESHOLD, EM_SCORE_DISTRESS_THRESHOLD);
    Ok(ScoringResult {
        score: em,
        zone,
        implied_pd: None,
        model: "Altman EM-Score (1995)".to_string(),
    })
}

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
