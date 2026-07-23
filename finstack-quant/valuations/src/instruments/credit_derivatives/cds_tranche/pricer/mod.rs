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
//!   robust integration using Gauss-Hermite quadrature.
//! * **ISDA Compliance**: Mid-period protection timing, proper settlement lag handling,
//!   and standard day count conventions.
//!
//! ## Mathematical Approach
//!
//! The model decomposes tranche [A,D] expected loss as:
//! `EL_[A,D](t) = [EL_eq(0,D,t) - EL_eq(0,A,t)] / [(D-A)/100]`
//!
//! Where `EL_eq(0,K,t)` is the expected loss of equity tranche [0,K] at time t,
//! calculated using base correlation ρ(K) for detachment point K.
//!
//! ### Premium Leg PV
//! `PV_prem = Σ c * Δt_i * DF(t_i) * [N_outstanding(t_{i-1}) - 0.5 * N_incremental_loss(t_i)]`
//!
//! ### Protection Leg PV
//! `PV_prot = Σ DF(t_i) * N_tr * [EL_fraction(t_i) - EL_fraction(t_{i-1})]`
//!
//! ## Integration Near Correlation Boundaries
//!
//! When correlation falls outside [0.05, 0.95] the pricer routes through
//! `GaussHermiteQuadrature::integrate_adaptive`, which performs at most one
//! order-promotion step and is a **no-op at the default quadrature order of
//! 20** (the maximum tabulated order). Boundary-correlation accuracy at the
//! default configuration therefore rests on the correlation clamp into
//! (0.01, 0.99) plus the order-20 rule itself — not on adaptive refinement.
//! Configuring a lower `quadrature_order` (5/7/10/15) enables the single
//! promotion step near the boundaries.
//!
//! ## Portfolio Support
//!
//! * Supports both homogeneous and heterogeneous portfolios: per-issuer credit
//!   curves, recovery rates, and weights via `CreditIndexData::issuer_credit_curves`
//! * Automatically detects uniform portfolios and uses the faster binomial path
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
mod registry;
mod saddlepoint;
mod sensitivities;

#[cfg(test)]
mod tests;

pub use config::{CDSTranchePricer, CDSTranchePricerConfig, HeteroMethod};
pub use registry::JumpToDefaultResult;
pub(crate) use registry::SimpleCDSTrancheHazardPricer;
