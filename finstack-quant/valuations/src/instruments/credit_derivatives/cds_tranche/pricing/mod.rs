//! Gaussian Copula pricing model for CDS tranches.
//!
//! Implements the industry-standard base correlation approach for pricing
//! synthetic CDO tranches using a one-factor Gaussian Copula model.
//!
//! ## Key Features
//!
//! * **Time-dependent Expected Loss**: Calculates expected loss at each payment date
//!   rather than using linear approximation from maturity values.
//! * **Accrual-on-Default (AoD)**: Premium leg includes proper AoD adjustment using
//!   half of incremental loss within each period.
//! * **Market-standard Scheduling**: Uses canonical schedule builders with business
//!   day conventions and holiday calendar support.
//! * **Risk Metrics**: Full implementation of CS01, Correlation Delta, and Jump-to-Default
//!   using central-difference bumping for accurate hedge ratios.
//! * **Numerical Stability**: Correlation clamping, monotonicity enforcement, and
//!   adaptive integration with explicit error and factor-tail budgets.
//! * **ISDA Compliance**: Mid-period protection timing, proper settlement lag handling,
//!   and standard day count conventions.
//!
//! ## Mathematical Approach
//!
//! The model decomposes tranche `[A,D]` expected loss as:
//! `EL_[A,D](t) = [EL_eq(0,D,t) - EL_eq(0,A,t)] / [(D-A)/100]`
//!
//! Where `EL_eq(0,K,t)` is the expected loss of equity tranche `[0,K]` at time t,
//! calculated using base correlation ρ(K) for detachment point K.
//!
//! ### Premium Leg PV
//! `PV_prem = Σ c * Δt_i * DF(t_i) * [N_outstanding(t_{i-1}) - 0.5 * N_incremental_loss(t_i)]`
//!
//! ### Protection Leg PV
//! `PV_prot = Σ DF(t_i) * N_tr * [EL_fraction(t_i) - EL_fraction(t_{i-1})]`
//!
//! ## Adaptive Integration
//!
//! Gaussian, RFL, and multi-factor conditioning use partitioned adaptive
//! Simpson. Normal tails outside [-10,10] have probability below 1.524e-23.
//! Student-t uses the copula's product Gauss–Laguerre × Gauss–Hermite rule
//! by default; nested adaptive Simpson over the mixing variable is opt-in
//! via `CDSTranchePricerConfig::with_adaptive_student_t_integration`.
//! When adaptive Student-t is enabled, mixing tails use explicit quantile
//! bounds and the configured absolute integration tolerance is allocated
//! across intervals, factors and tails; exhausted refinement produces an
//! error. Conditional convolution grid error and the large-pool
//! approximation remain separate approximation boundaries.
//! Base-correlation differences may clamp only noise inside the combined
//! integration budget; materially negative tranche losses return an error.
//!
//! ## Portfolio Support
//!
//! * Supports both homogeneous and heterogeneous portfolios: per-issuer credit
//!   curves, recovery rates, and weights via `CreditIndexData::issuer_credit_curves`
//! * Automatically detects uniform portfolios and uses the faster binomial
//!   path, or the large-homogeneous-pool closed form above
//!   `credit::SMALL_POOL_THRESHOLD` names
//! * Falls back to heterogeneous exact convolution (pools ≤ 64 names) or the
//!   moment-matched normal approximation for large diversified portfolios
//!
//! ## Limitations
//!
//! * Base correlation model can have small arbitrage inconsistencies at curve knots

mod config;
mod engine;
mod expected_loss;
mod heterogeneous;
mod integration;
mod registry;
mod saddlepoint;
mod sensitivities;

#[cfg(test)]
mod tests;

pub use config::{CDSTranchePricer, CDSTranchePricerConfig, HeteroMethod};
pub use registry::JumpToDefaultResult;
pub(crate) use registry::SimpleCDSTrancheHazardPricer;
