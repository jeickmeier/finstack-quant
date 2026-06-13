//! FX risk prescribed parameters per BCBS d457.

/// FX delta risk weight (uniform across all currency pairs).
pub const FX_DELTA_RISK_WEIGHT: f64 = 15.0;

/// FX vega risk weight after liquidity-horizon scaling.
pub const FX_VEGA_RISK_WEIGHT: f64 = 1.00;

/// FX curvature risk weight scale.
pub const FX_CURVATURE_RISK_WEIGHT: f64 = 0.5;

/// FX inter-bucket (cross-pair) correlation.
pub const FX_INTER_PAIR_CORRELATION: f64 = 0.60;
