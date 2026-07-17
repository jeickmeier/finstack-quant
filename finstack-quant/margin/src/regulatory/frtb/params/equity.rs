//! Equity risk prescribed parameters per BCBS d457.

/// Equity delta risk weights by bucket (percentage).
///
/// Buckets 1-13 per FRTB specification:
/// 1-4: Large-cap developed markets
/// 5-8: Large-cap emerging markets
/// 9: Small-cap developed markets
/// 10: Small-cap emerging markets
/// 11: Other sector
/// 12: Indices, ETFs, and funds
/// 13: Other equity (volatility indices, etc.)
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

/// Equity intra-bucket correlation.
pub const EQUITY_INTRA_BUCKET_CORRELATION: f64 = 0.15;

/// Equity inter-bucket correlation.
pub const EQUITY_INTER_BUCKET_CORRELATION: f64 = 0.15;

/// Equity vega risk weight.
pub const EQUITY_VEGA_RISK_WEIGHT: f64 = 0.78;

/// Equity curvature risk weight scale.
pub const EQUITY_CURVATURE_RISK_WEIGHT: f64 = 0.5;

use std::sync::LazyLock;

use finstack_quant_core::HashMap;

static EQUITY_RW_BY_BUCKET: LazyLock<HashMap<u8, f64>> =
    LazyLock::new(|| EQUITY_RISK_WEIGHTS.iter().copied().collect());

/// Look up an equity risk weight by bucket.
///
/// # Arguments
///
/// * `bucket` - FRTB equity risk bucket number; unmapped buckets use the
///   regulatory fallback risk weight of 55.0.
#[must_use]
pub fn equity_risk_weight(bucket: u8) -> f64 {
    EQUITY_RW_BY_BUCKET.get(&bucket).copied().unwrap_or(55.0)
}
