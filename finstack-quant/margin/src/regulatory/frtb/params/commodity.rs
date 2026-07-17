//! Commodity risk prescribed parameters per BCBS d457.

/// Commodity delta risk weights by bucket (percentage).
///
/// Buckets 1-11 per FRTB specification:
/// 1: Energy - Crude oil
/// 2: Energy - Natural gas
/// 3: Energy - Coal/Electricity
/// 4: Freight
/// 5: Base metals
/// 6: Precious metals
/// 7: Grains and oilseed
/// 8: Softs and other agriculturals
/// 9: Livestock and dairy
/// 10: Other commodity
/// 11: Carbon trading
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

/// Commodity intra-bucket correlation.
pub const COMMODITY_INTRA_BUCKET_CORRELATION: f64 = 0.55;

/// Commodity inter-bucket correlation.
pub const COMMODITY_INTER_BUCKET_CORRELATION: f64 = 0.20;

/// Commodity vega risk weight after liquidity-horizon scaling.
pub const COMMODITY_VEGA_RISK_WEIGHT: f64 = 1.00;

/// Commodity curvature risk weight scale.
pub const COMMODITY_CURVATURE_RISK_WEIGHT: f64 = 0.5;

/// Look up a commodity risk weight by bucket.
///
/// # Arguments
///
/// * `bucket` - FRTB commodity risk bucket number; unmapped buckets use the
///   regulatory fallback risk weight of 20.0.
#[must_use]
pub fn commodity_risk_weight(bucket: u8) -> f64 {
    COMMODITY_RISK_WEIGHTS
        .iter()
        .find(|(b, _)| *b == bucket)
        .map(|(_, w)| *w)
        .unwrap_or(20.0) // Default for unmapped buckets
}
