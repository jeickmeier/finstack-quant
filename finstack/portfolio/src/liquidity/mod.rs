//! Liquidity risk metrics, spread estimation, and portfolio scoring.
//!
//! This module provides market microstructure liquidity modeling for traded
//! positions. It is orthogonal to the balance-sheet liquidity ratios in
//! `finstack-statements-analytics` and focuses on:
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
//! 1. **Types** (`types`): `LiquidityProfile`, `LiquidityTier`, `LiquidityConfig`
//! 2. **Estimators** (`estimators`): Pure functions on `&[f64]` slices
//! 3. **LVaR** (`lvar`): Composes with existing VaR numbers
//! 4. **Impact** (`impact`, `almgren_chriss`, `kyle`): Trade execution cost models
//! 5. **Scoring** (`scoring`): Portfolio-level aggregation
//!
//! # Usage
//!
//! ```ignore
//! use finstack_portfolio::liquidity::{
//!     LiquidityProfile, LiquidityConfig, LvarCalculator,
//!     score_portfolio_liquidity, roll_effective_spread,
//! };
//! ```

mod almgren_chriss;
mod estimators;
mod impact;
mod kyle;
mod lvar;
mod scoring;
mod types;

// Re-export core types
pub use types::{
    classify_tier, days_to_liquidate, LiquidityConfig, LiquidityProfile, LiquidityTier,
    SpreadVolatilityKind, TierAllocation,
};

// Re-export estimators
pub use estimators::{amihud_illiquidity, roll_effective_spread};

// Re-export LVaR
pub use lvar::{
    lvar_bangia_scalar, LvarBangiaScalar, LvarCalculator, LvarResult, PortfolioLvarReport,
};

// Re-export impact models
pub use almgren_chriss::AlmgrenChrissModel;
pub use impact::{ExecutionTrajectory, ImpactEstimate, MarketImpactModel, TradeParams};
pub use kyle::KyleLambdaModel;

// Re-export scoring
pub use scoring::{score_portfolio_liquidity, PortfolioLiquidityReport, PositionLiquidityScore};
