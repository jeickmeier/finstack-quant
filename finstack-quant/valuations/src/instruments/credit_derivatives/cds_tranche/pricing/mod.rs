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
//! Simpson with a fixed absolute budget allocated across intervals, factors
//! and tails; exhausted refinement produces an error. Normal tails outside
//! [-10,10] have probability below 1.524e-23. Student-t uses the copula's
//! product Gauss–Laguerre × Gauss–Hermite rule. Conditional convolution grid
//! error is a separate approximation boundary. Base-correlation differences
//! may clamp only noise inside the integration budget; materially negative
//! tranche losses return an error.
//!
//! ## Portfolio Support
//!
//! * Supports both homogeneous and heterogeneous portfolios: per-issuer credit
//!   curves, recovery rates, and weights via `CreditIndexData::issuer_credit_curves`
//! * Automatically detects uniform portfolios and uses the exact conditional
//!   binomial path at every pool size
//! * Uses bounded exact conditional convolution for heterogeneous pools;
//!   exceeding the grid or work limits fails instead of approximating
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
mod sensitivities;

#[cfg(test)]
mod tests;

pub use config::CdsTranchePricer;
pub(crate) use registry::SimpleCdsTrancheHazardPricer;
