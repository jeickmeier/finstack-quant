//! Position-level VaR and ES decomposition via Euler allocation.
//!
//! This module provides parametric and historical decomposition of portfolio
//! VaR and Expected Shortfall into per-position contributions. The parametric
//! engine uses covariance-based Euler allocation (exact under normality); the
//! historical engine attributes tail losses from scenario P&L matrices.
//!
//! # Euler Decomposition Property
//!
//! Under the parametric (normal) assumption:
//! ```text
//! sum(component_var_i) == portfolio_var  (exact)
//! sum(component_es_i)  == portfolio_es   (exact)
//! ```
//!
//! Under historical simulation the Euler property holds approximately.
//!
//! # References
//!
//! - `docs/REFERENCES.md#tasche-2008-capital-allocation`
//! - `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - `docs/REFERENCES.md#litterman-1996-hotspots`

pub(super) mod engine;
#[cfg(test)]
mod tests;
