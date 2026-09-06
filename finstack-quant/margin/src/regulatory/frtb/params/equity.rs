//! Prescribed equity parameters from Basel MAR21 (January 2023 framework).
//! Source: <https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2024-07-05>

/// Equity **spot** delta risk weights by bucket, in percent
/// (MAR21.77, Table 10, spot column).
///
/// Bucket names per MAR21.72 Table 9:
/// 1-4: Large cap, emerging market economy
/// 5-8: Large cap, advanced economy
/// 9: Small cap, emerging market economy
/// 10: Small cap, advanced economy
/// 11: Other sector
/// 12: Large-cap advanced-economy equity indices
/// 13: Other equity indices
pub const EQUITY_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 55.0),
    (2, 60.0),
    (3, 45.0),
    (4, 55.0),
    (5, 30.0),
    (6, 35.0),
    (7, 40.0),
    (8, 50.0),
    (9, 70.0),
    (10, 50.0),
    (11, 70.0),
    (12, 15.0),
    (13, 25.0),
];

/// Look up the prescribed delta risk weight, in percentage points.
///
/// # Arguments
///
/// * `bucket` - Validated Basel bucket number for this risk class.
#[must_use]
pub fn equity_risk_weight(bucket: u8) -> f64 {
    EQUITY_RISK_WEIGHTS
        .iter()
        .find(|(b, _)| *b == bucket)
        .map_or(f64::NAN, |(_, w)| *w)
}

/// Equity vega weight by liquidity horizon, MAR21.92.
///
/// # Arguments
///
/// * `bucket` - Equity bucket 1 through 13; small-cap/other buckets 9-11 use the 100% cap.
#[must_use]
pub fn equity_vega_risk_weight(bucket: u8) -> f64 {
    if (9..=11).contains(&bucket) {
        1.0
    } else {
        0.55 * 2.0_f64.sqrt()
    }
}
