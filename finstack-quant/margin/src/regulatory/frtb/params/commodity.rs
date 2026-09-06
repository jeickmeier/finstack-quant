//! Prescribed commodity parameters from Basel MAR21 (January 2023 framework).
//! Source: <https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2024-07-05>

/// Commodity delta risk weights by bucket, in percent (MAR21.82, Table 11).
///
/// Bucket names per MAR21.81:
/// 1: Energy - solid combustibles
/// 2: Energy - liquid combustibles
/// 3: Energy - electricity and carbon trading
/// 4: Freight
/// 5: Metals - non-precious
/// 6: Gaseous combustibles
/// 7: Precious metals (including gold)
/// 8: Grains and oilseed
/// 9: Livestock and dairy
/// 10: Softs and other agriculturals
/// 11: Other commodity
pub const COMMODITY_RISK_WEIGHTS: &[(u8, f64)] = &[
    (1, 30.0),
    (2, 35.0),
    (3, 60.0),
    (4, 80.0),
    (5, 40.0),
    (6, 45.0),
    (7, 20.0),
    (8, 35.0),
    (9, 25.0),
    (10, 35.0),
    (11, 50.0),
];

/// Look up the prescribed delta risk weight, in percentage points.
///
/// # Arguments
///
/// * `bucket` - Validated Basel bucket number for this risk class.
#[must_use]
pub fn commodity_risk_weight(bucket: u8) -> f64 {
    COMMODITY_RISK_WEIGHTS
        .iter()
        .find(|(b, _)| *b == bucket)
        .map_or(f64::NAN, |(_, w)| *w)
}

/// Commodity vega weight at the MAR21.92 liquidity-horizon cap.
pub const COMMODITY_VEGA_RISK_WEIGHT: f64 = 1.0;
