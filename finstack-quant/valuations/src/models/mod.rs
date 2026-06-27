//! Option pricing models and numerical methods with academic foundations.
//!
//! Provides reusable pricing models for options and derivatives, including
//! closed-form formulas, tree-based methods, volatility models, and Black-Scholes variants.
//! All implementations cite their academic sources for correctness verification.
//!
//! # Module Organization
//!
//! - `closed_form`: Closed-form and semi-analytical pricing formulas (Black-Scholes Greeks,
//!   Asian, Barrier, Lookback, Quanto, Heston)
//! - `volatility`: Volatility models (SABR) and Black-Scholes helper functions
//! - `trees`: Tree-based methods (Binomial, Trinomial, Multi-factor, Short-rate)
//! - `pde`: Finite difference PDE methods (1D Crank-Nicolson, 2D Craig-Sneyd ADI, Heston, American penalty)
//!
//! Credit copulas, recovery models, and factor models live in [`crate::correlation`].

pub mod closed_form;
pub mod credit;
pub mod pde;
pub mod trees;
pub mod volatility;

// Re-export commonly used items from submodules for convenience
#[allow(unused_imports)]
pub use closed_form::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, barrier_call_continuous,
    barrier_put_continuous, black76_implied_vol, bs_call_delta, bs_call_greeks, bs_call_rho,
    bs_call_theta, bs_gamma, bs_greeks, bs_implied_vol, bs_price, bs_put_delta, bs_put_greeks,
    bs_put_rho, bs_put_theta, bs_vega, down_in_call, down_out_call, fixed_strike_lookback_call,
    fixed_strike_lookback_put, floating_strike_lookback_call, floating_strike_lookback_put,
    geometric_asian_call, geometric_asian_put, heston_call_price_fourier, heston_put_price_fourier,
    quanto_call, quanto_drift_adjustment, quanto_put, up_in_call, up_out_call, AsianGreeks,
    AsianPriceResult, BarrierType, BsGreeks, HestonParams, ONE_PERCENT,
};
#[allow(unused_imports)]
pub(crate) use closed_form::{black76_call, black76_put};
#[allow(unused_imports)]
pub use pde::{
    BlackScholesPde, BoundaryCondition, CraigSneydStepper, Grid1D, Grid2D, HestonPde, PdeProblem1D,
    PdeProblem2D, PdeSolution, PdeSolution2D, Solver1D, Solver2D,
};
#[allow(unused_imports)]
pub use trees::{
    short_rate_keys, single_factor_equity_state, state_keys, two_factor_equity_rates_state,
    BarrierSpec, BarrierStyle, BinomialTree, EvolutionParams, HullWhiteTree, HullWhiteTreeConfig,
    NodeState, ShortRateModel, ShortRateTree, ShortRateTreeConfig, TreeBranching, TreeCompounding,
    TreeGreeks, TreeModel, TreeParameters, TreeType, TreeValuator,
};
pub(crate) use volatility::vega_weight;
#[allow(unused_imports)]
pub use volatility::{
    d1, d1_black76, d1_d2, d1_d2_black76, d2, d2_black76, norm_cdf, norm_pdf, SABRCalibrator,
    SABRModel, SABRParameters, SABRSmile,
};
