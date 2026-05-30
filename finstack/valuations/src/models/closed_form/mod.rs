//! Closed-form and semi-analytical pricing formulas with academic citations.
//!
//! This module provides closed-form and semi-analytical pricing formulas for
//! options and derivatives. These implementations serve dual purposes:
//! 1. **Production pricing** when analytical methods are appropriate
//! 2. **Validation** of Monte Carlo and numerical implementations
//!
//! All formulas are implemented with academic citations to ensure correctness
//! and traceability to authoritative sources.
//!
//! # Supported Models
//!
//! ## Black-Scholes-Merton Framework
//!
//! - **Vanilla options**: European calls and puts with dividends
//! - **Greeks**: Delta, gamma, vega, theta, rho with analytical formulas
//! - **Base model**: Black & Scholes (1973), Merton (1973)
//!
//! ## Path-Dependent Options
//!
//! - **Asian options**: Geometric (Kemna & Vorst 1990), Arithmetic approximation (Turnbull & Wakeman 1991)
//! - **Barrier options**: Continuous monitoring (Reiner & Rubinstein 1991)
//! - **Lookback options**: Fixed and floating strike (Conze & Viswanathan 1991)
//!
//! ## Multi-Asset and Stochastic Volatility
//!
//! - **Quanto options**: Cross-currency adjustments (Garman & Kohlhagen 1983)
//! - **Heston model**: Stochastic volatility via Fourier transform (Heston 1993, Carr & Madan 1999)
//!
//! # Academic References
//!
//! ## Foundational Papers
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities."
//!   *Journal of Political Economy*, 81(3), 637-654.
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   *Bell Journal of Economics and Management Science*, 4(1), 141-183.
//!
//! ## Asian Options
//!
//! - Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options Based on
//!   Average Asset Values." *Journal of Banking & Finance*, 14(1), 113-129.
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing European
//!   Average Options." *Journal of Financial and Quantitative Analysis*, 26(3), 377-389.
//!
//! ## Barrier Options
//!
//! - Reiner, E., & Rubinstein, M. (1991). "Breaking Down the Barriers."
//!   *Risk Magazine*, 4(8), 28-35.
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   (Also covers barrier option fundamentals)
//!
//! ## Lookback Options
//!
//! - Conze, A., & Viswanathan (1991). "Path Dependent Options: The Case of Lookback Options."
//!   *Journal of Finance*, 46(5), 1893-1907.
//! - Goldman, M. B., Sosin, H. B., & Gatto, M. A. (1979). "Path Dependent Options:
//!   Buy at the Low, Sell at the High." *Journal of Finance*, 34(5), 1111-1127.
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.).
//!   McGraw-Hill. Chapter 4.
//!
//! ## Quanto Options
//!
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."
//!   *Journal of International Money and Finance*, 2(3), 231-237.
//! - Derman, E., Karasinski, P., & Wecker, J. (1990). "Understanding Guaranteed
//!   Exchange-Rate Contracts in Foreign Stock Investments." Goldman Sachs Quantitative
//!   Strategies Research Notes.
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Section 13.16.
//!
//! ## Stochastic Volatility (Heston)
//!
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility
//!   with Applications to Bond and Currency Options." *Review of Financial Studies*, 6(2), 327-343.
//! - Carr, P., & Madan, D. (1999). "Option Valuation Using the Fast Fourier Transform."
//!   *Journal of Computational Finance*, 2(4), 61-73.
//! - Albrecher, H., Mayer, P., Schoutens, W., & Tistaert, J. (2007). "The Little Heston Trap."
//!   *Wilmott Magazine*, January, 83-92.
//! - Lord, R., & Kahl, C. (2010). "Complex Logarithms in Heston-Like Models."
//!   *Mathematical Finance*, 20(4), 671-694.
//!
//! # Implementation Notes
//!
//! - All formulas use **continuous compounding** and **continuous dividends**
//! - Greeks are computed analytically from first principles
//! - Numerical stability is prioritized (avoiding division by zero, handling edge cases)
//! - Edge cases (zero time, zero volatility) are handled explicitly
//!
//! # Examples
//!
//! ## Black-Scholes Greeks
//!
//! ```ignore
//! use finstack_valuations::models::closed_form::greeks::{
//!     bs_call_delta, bs_gamma, bs_vega
//! };
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let time = 1.0;
//! let rate = 0.05;
//! let div_yield = 0.02;
//! let vol = 0.20;
//!
//! let delta = bs_call_delta(spot, strike, time, rate, div_yield, vol);
//! let gamma = bs_gamma(spot, strike, time, rate, div_yield, vol);
//! let vega = bs_vega(spot, strike, time, rate, div_yield, vol);
//!
//! assert!((delta - 0.5).abs() < 0.1);
//! ```
//!
//! ## Barrier Option
//!
//! ```ignore
//! use finstack_valuations::models::closed_form::barrier::down_out_call;
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let barrier = 90.0;
//! let time = 1.0;
//! let rate = 0.05;
//! let div_yield = 0.02;
//! let vol = 0.20;
//!
//! let price = down_out_call(spot, strike, barrier, time, rate, div_yield, vol);
//! assert!(price > 0.0);
//! ```
//!
//! # See Also
//!
//! - [`greeks`] for Black-Scholes Greeks (delta, gamma, vega, theta, rho)
//! - [`asian`] for Asian option pricing
//! - [`barrier`] for barrier option pricing
//! - [`lookback`] for lookback option pricing
//! - [`quanto`] for quanto option pricing
//! - [`heston`] for stochastic volatility pricing

pub mod asian;
pub mod barrier;
pub mod greeks;
pub mod heston;
pub mod implied_vol;
pub mod lookback;
pub mod quanto;
pub mod vanilla;

// Re-export commonly used functions
pub use asian::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, geometric_asian_call, geometric_asian_put,
    AsianGreeks, AsianPriceResult,
};
pub use barrier::{
    barrier_call_continuous, barrier_put_continuous, down_in_call, down_out_call, up_in_call,
    up_out_call, BarrierType,
};
pub use greeks::{
    bs_call_delta, bs_call_greeks, bs_call_rho, bs_call_theta, bs_gamma, bs_put_delta,
    bs_put_greeks, bs_put_rho, bs_put_theta, bs_vega,
};
pub use heston::{heston_call_price_fourier, heston_put_price_fourier, HestonParams};
pub use implied_vol::{black76_implied_vol, bs_implied_vol};
pub use lookback::{
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
    floating_strike_lookback_put,
};
pub use quanto::{
    quanto_call, quanto_call_simple, quanto_drift_adjustment, quanto_put, quanto_put_simple,
};
pub(crate) use vanilla::{black76_call, black76_put};
pub use vanilla::{
    bs_greeks, bs_price, bs_price_checked, checked_closed_form_value, option_type_from_bool,
    BsGreeks, ONE_PERCENT,
};
