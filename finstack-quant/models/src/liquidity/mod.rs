//! Product-independent liquidity risk metrics and market-impact models.
//!
//! This module provides market microstructure liquidity modeling for traded
//! positions. It is orthogonal to the balance-sheet liquidity ratios in
//! `finstack-quant-statements-analytics` and focuses on:
//!
//! - **Spread estimation**: Roll (1984) effective spread and Amihud (2002)
//!   illiquidity ratio from return/volume data.
//! - **Liquidity-adjusted VaR (LVaR)**: Bangia et al. (1999) framework
//!   combining exogenous spread costs, endogenous position-size effects,
//!   and time-to-liquidation horizon adjustments.
//! - **Market impact models**: Almgren-Chriss (2001) optimal execution with
//!   permanent/temporary impact decomposition, and Kyle (1985) linear lambda.
//! - **Portfolio liquidity scoring**: Position-level days-to-liquidate, tier
//!   classification, and aggregate portfolio liquidity reports.
//!
//! # Architecture
//!
//! The module is structured in layers:
//!
//! 1. **Types** (`types`): `LiquidityProfile`, `LiquidityTier`
//! 2. **Estimators** (`estimators`): Pure functions on `&[f64]` slices
//! 3. **LVaR** (`lvar`): Composes with existing VaR numbers
//! 4. **Impact** (`impact`, `almgren_chriss`, `kyle`): Trade execution cost models
//!
//! # Usage
//!
//! Estimate the effective spread from a return series, then charge that spread
//! against an existing VaR number:
//!
//! ```
//! use finstack_quant_models::liquidity::{lvar_bangia, roll_effective_spread};
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let returns = [0.004, -0.005, 0.006, -0.004, 0.005, -0.006];
//! let spread = roll_effective_spread(&returns).expect("at least two returns");
//! assert!(spread > 0.0);
//!
//! // VaR uses the loss-sign convention, so it is non-positive.
//! let result = lvar_bangia(-100_000.0, spread, 0.25 * spread, 0.99, 1_000_000.0)?;
//! assert!(result.spread_cost >= 0.0);
//! assert!(result.lvar <= result.var);
//! # Ok(())
//! # }
//! ```

mod almgren_chriss;
mod estimators;
mod impact;
mod kyle;
mod lvar;
mod types;

use crate::invalid_input;

pub use types::{
    LiquidityProfile, LiquidityTier, SpreadVolatilityKind, days_to_liquidate, liquidity_tier,
};

pub use estimators::{amihud_illiquidity, roll_effective_spread};

pub use lvar::{LvarBangiaScalar, lvar_bangia};

pub use almgren_chriss::{AlmgrenChrissModel, almgren_chriss_impact};
pub use impact::{ExecutionTrajectory, ImpactEstimate, TradeParams};
pub use kyle::{KyleLambdaModel, kyle_lambda};

#[cfg(test)]
mod tests;
