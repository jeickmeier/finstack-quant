//! GIRR (General Interest Rate Risk) prescribed parameters.
//!
//! Risk weights and tenor correlations per BCBS d457.

/// GIRR delta risk weights by tenor bucket (MAR21.45).
///
/// Values are expressed in **percent of notional** (e.g. `1.7` means
/// `1.7%` = `170 bp`). They multiply a delta stated as P&L per 1
/// percentage-point yield shift — see
/// [`FrtbSensitivities::girr_delta`](crate::regulatory::frtb::types::FrtbSensitivities::girr_delta)
/// for the input-unit convention.
///
/// Tenor labels: 0.25Y, 0.5Y, 1Y, 2Y, 3Y, 5Y, 10Y, 15Y, 20Y, 30Y.
pub const GIRR_DELTA_RISK_WEIGHTS: &[(&str, f64)] = &[
    ("0.25Y", 1.7),
    ("0.5Y", 1.7),
    ("1Y", 1.6),
    ("2Y", 1.3),
    ("3Y", 1.2),
    ("5Y", 1.1),
    ("10Y", 1.1),
    ("15Y", 1.1),
    ("20Y", 1.1),
    ("30Y", 1.1),
];

/// GIRR inflation delta risk weight (percentage of notional).
pub const GIRR_INFLATION_RISK_WEIGHT: f64 = 1.6;

/// GIRR cross-currency basis risk weight.
pub const GIRR_XCCY_BASIS_RISK_WEIGHT: f64 = 1.6;

/// GIRR vega risk weight after liquidity-horizon scaling.
pub const GIRR_VEGA_RISK_WEIGHT: f64 = 1.00;

/// GIRR curvature risk weight scale factor.
pub const GIRR_CURVATURE_RISK_WEIGHT: f64 = 0.5;

/// Intra-bucket (same currency) tenor correlation.
///
/// `rho(t_k, t_l) = max(e^{-theta * |T_k - T_l| / min(T_k, T_l)}, 0.40)`
/// where theta = 0.03.
///
/// Pre-computed correlations for the standard tenor grid.
/// Rows and columns follow the order in `GIRR_DELTA_RISK_WEIGHTS`.
pub const GIRR_TENOR_CORRELATION_THETA: f64 = 0.03;
/// Minimum allowed correlation between any two GIRR tenors.
pub const GIRR_TENOR_CORRELATION_FLOOR: f64 = 0.40;

/// Inter-bucket (cross-currency) correlation for GIRR.
pub const GIRR_INTER_BUCKET_CORRELATION: f64 = 0.50;

/// Correlation between GIRR delta and inflation within the same currency.
pub const GIRR_INFLATION_CORRELATION: f64 = 0.40;

/// Correlation between GIRR delta and cross-currency basis.
pub const GIRR_XCCY_BASIS_CORRELATION: f64 = 0.0;

/// Compute the GIRR intra-bucket tenor correlation.
///
/// Uses the parametric formula from BCBS d457:
/// `rho = max(exp(-theta * |T_k - T_l| / min(T_k, T_l)), floor)`
///
/// # Arguments
///
/// * `tenor_k_years` - First GIRR risk-factor tenor, expressed as a positive
///   number of years.
/// * `tenor_l_years` - Second GIRR risk-factor tenor, expressed as a positive
///   number of years; equal tenors return correlation 1.0.
#[must_use]
pub fn girr_tenor_correlation(tenor_k_years: f64, tenor_l_years: f64) -> f64 {
    if (tenor_k_years - tenor_l_years).abs() < 1e-12 {
        return 1.0;
    }
    let min_tenor = f64::min(tenor_k_years, tenor_l_years);
    if min_tenor <= 0.0 {
        return GIRR_TENOR_CORRELATION_FLOOR;
    }
    let diff = (tenor_k_years - tenor_l_years).abs();
    let rho = (-GIRR_TENOR_CORRELATION_THETA * diff / min_tenor).exp();
    f64::max(rho, GIRR_TENOR_CORRELATION_FLOOR)
}

/// Convert a standard tenor label to years.
///
/// # Arguments
///
/// * `tenor` - Exact BCBS standard tenor label, such as `"0.25Y"` or `"10Y"`;
///   unsupported labels return `None`.
#[must_use]
pub fn tenor_to_years(tenor: &str) -> Option<f64> {
    match tenor {
        "0.25Y" => Some(0.25),
        "0.5Y" => Some(0.5),
        "1Y" => Some(1.0),
        "2Y" => Some(2.0),
        "3Y" => Some(3.0),
        "5Y" => Some(5.0),
        "10Y" => Some(10.0),
        "15Y" => Some(15.0),
        "20Y" => Some(20.0),
        "30Y" => Some(30.0),
        _ => None,
    }
}
