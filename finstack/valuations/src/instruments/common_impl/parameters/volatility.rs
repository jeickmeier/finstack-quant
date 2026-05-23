//! Volatility model and volatility-parameter types shared across instruments.

pub use crate::models::volatility::SABRParameters;

/// Volatility model for option pricing.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VolatilityModel {
    /// Black (lognormal).
    #[default]
    Black,
    /// Bachelier / normal model.
    Normal,
}
