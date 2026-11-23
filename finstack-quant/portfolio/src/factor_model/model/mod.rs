//! Factor-model orchestration for portfolio-level risk decomposition.
//!
//! The runtime model connects:
//!
//! - declarative factor definitions and covariance inputs
//! - dependency-to-factor matching
//! - sensitivity generation
//! - downstream decomposition engines
//!
//! Credit hierarchy factors shock stored CDS spread quotes and require a lossless
//! calibration recipe on every matched hazard curve. Intensity-only curves cannot
//! supply spread-factor risk. All monetary exposures use portfolio base currency.
//!
//! # References
//!
//! - Factor-model portfolio construction: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//!
//! - Euler-style capital allocation background: `docs/REFERENCES.md#tasche-2008-capital-allocation`
//!
//! - Parametric VaR conventions: `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
//!

pub(crate) mod engine;
#[cfg(test)]
pub(super) mod tests;

pub(crate) use engine::FactorModel;
