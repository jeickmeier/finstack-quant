//! Stochastic risk metrics for structured credit.
//!
//! This module provides risk metrics for stochastic
//! structured credit analysis including:
//!
//! - **Expected Loss (EL)**: Probability-weighted average loss
//! - **Unexpected Loss (UL)**: Loss standard deviation (Credit VaR proxy)
//! - **Expected Shortfall (ES)**: Tail risk metric (CVaR)
//! - **Correlation01**: Sensitivity to 1% asset correlation bump
//! - **RecoveryCorrelation01**: Sensitivity to recovery-default correlation
//! - **PrepaymentVol01**: Sensitivity to prepayment volatility
//!
//! # Risk Metric Definitions
//!
//! ## Expected Loss (EL)
//! ```text
//! EL = E[Loss] = Σᵢ pᵢ × Lossᵢ
//! ```
//!
//! ## Unexpected Loss (UL)
//! ```text
//! UL = σ(Loss) = √(E[Loss²] - E[Loss]²)
//! ```
//!
//! ## Expected Shortfall (ES) at confidence α
//! ```text
//! ES_α = E[Loss | Loss > VaR_α]
//! ```
//!
//! ## Correlation01
//! ```text
//! Corr01 = (NPV(ρ + 0.01) - NPV(ρ)) / 0.01
//! ```
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::metrics::{
//!     StochasticMetricsCalculator,
//! };
//! use finstack_quant_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
//!     ScenarioTree, ScenarioTreeConfig,
//! };
//!
//! // Build a simple scenario tree configuration (e.g., RMBS-style defaults/prepay)
//! let config = ScenarioTreeConfig::rmbs_standard(5.0, 0.045);
//! let tree = ScenarioTree::build(&config).expect("tree build should succeed");
//!
//! // Compute metrics directly from the tree (no market data needed)
//! let calc = StochasticMetricsCalculator::new(1.0);
//! let metrics = calc.compute_from_tree(&tree);
//!
//! println!("Expected Loss: ${:.0}", metrics.expected_loss);
//! // Tail-risk fields (`unexpected_loss`, `var_*`, `expected_shortfall_*`) are
//! // `Option` and are `None` from a recombining tree — it collapses the loss
//! // dispersion they measure. Price in Monte Carlo mode for tail risk.
//! // Correlation sensitivities are computed separately via `CorrelationSensitivities`.
//! ```

mod calculator;
mod sensitivities;

#[allow(unused_imports)]
pub(crate) use calculator::{StochasticMetrics, StochasticMetricsCalculator};
#[allow(unused_imports)]
pub(crate) use sensitivities::{CorrelationSensitivities, SensitivityConfig};
