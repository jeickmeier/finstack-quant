//! CDS tranche instruments with base correlation and multiple copula models.
//!
//! CDS tranches (synthetic CDO tranches) provide leveraged credit exposure
//! to specific slices of the default distribution. Pricing supports multiple
//! copula models including Gaussian, Student-t, Random Factor Loading, and
//! Multi-factor, with optional stochastic recovery.
//!
//! # Tranche Structure
//!
//! A tranche covers losses between attachment and detachment points:
//! - **Attachment**: Lower loss threshold (e.g., 3%)
//! - **Detachment**: Upper loss threshold (e.g., 7%)
//! - **Tranche notional**: Detachment - Attachment
//!
//! Example tranches on CDX.IG:
//! - **Equity**: 0-3% (first loss)
//! - **Mezzanine**: 3-7%, 7-10%, 10-15%
//! - **Senior**: 15-30%
//! - **Super senior**: 30-100%
//!
//! # Copula Models
//!
//! ## One-Factor Gaussian (Default)
//!
//! Default correlation modeled via single systematic factor:
//!
//! ```text
//! Asset value = √ρ · Z + √(1-ρ) · εᵢ
//! ```
//!
//! where ρ is base correlation, Z is common factor, εᵢ is idiosyncratic.
//!
//! ## Student-t Copula
//!
//! Captures tail dependence - joint extreme defaults more likely than
//! Gaussian predicts. Use for stress testing or when market implies
//! high correlation during stress.
//!
//! ## Random Factor Loading (RFL)
//!
//! Stochastic correlation - correlation itself is random, typically
//! higher in stressed markets. Important for senior tranches.
//!
//! ## Multi-Factor
//!
//! Sector-specific correlation structure for bespoke portfolios
//! with industry concentration.
//!
//! # Stochastic Recovery
//!
//! Optional recovery model where recovery negatively correlates with
//! the systematic factor (lower recovery in stress). This captures
//! the "double hit" effect seen empirically.
//!
//! # Arbitrage-Free Base Correlation
//!
//! Base correlation curves are validated for arbitrage-free conditions:
//! - Monotonicity: β(K₁) ≤ β(K₂) for K₁ < K₂
//! - Valid bounds: 0 ≤ β(K) ≤ 1
//!
//! Smoothing methods available: Isotonic Regression (PAVA), Strict
//! Monotonic, Weighted Smoothing.
//!
//! # References
//!
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."
//!   *Journal of Fixed Income*, 9(4), 43-54. `docs/REFERENCES.md#li-2000-gaussian-copula`
//!
//! - Andersen, L., & Sidenius, J. (2005). "Extensions to the Gaussian Copula:
//!   Random Recovery and Random Factor Loadings." *Journal of Credit Risk*. `docs/REFERENCES.md#li-2000-gaussian-copula` `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//!
//! - Laurent, J.-P., & Gregory, J. (2005). "Basket Default Swaps, CDOs and
//!   Factor Copulas." *Journal of Risk*, 7(4), 103-122. `docs/REFERENCES.md#laurent-gregory-2005-factor-copulas`
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapter 9: Synthetic CDO Valuation. `docs/REFERENCES.md#o-kane-2008`
//!
//! # See Also
//!
//! - [`CDSTranche`] for instrument struct
//! - [`crate::instruments::credit_derivatives::cds_tranche::TrancheSide`] for buyer vs seller
//! - [`finstack_quant_models::correlation::copula`] for copula model implementations
//! - [`finstack_quant_models::correlation::recovery`] for stochastic recovery models
//! - Base correlation calibration via plan-driven `calibration::api`

mod credit_risk;
pub(crate) mod metrics;
pub(crate) mod parameters;
pub(crate) mod pricing;
mod types;

pub use finstack_quant_models::correlation::copula::CopulaSpec;
pub use finstack_quant_models::correlation::recovery::RecoverySpec;
pub use parameters::CDSTrancheParams;
pub use types::{CDSTranche, CDSTrancheBuilder, TrancheSide};

pub use pricing::{CDSTranchePricer, CDSTranchePricerConfig, HeteroMethod};
