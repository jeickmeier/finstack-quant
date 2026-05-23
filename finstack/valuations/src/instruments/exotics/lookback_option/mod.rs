//! Lookback option instruments with path extremum payoffs.
//!
//! Lookback options have payoffs based on the maximum or minimum price
//! reached over the option's life, providing optimal execution without
//! timing risk. Popular for guaranteed best-price execution.
//!
//! # Lookback Types
//!
//! - **Fixed strike lookback call**: max(S_max - K, 0)
//! - **Fixed strike lookback put**: max(K - S_min, 0)
//! - **Floating strike lookback call**: S_T - S_min
//! - **Floating strike lookback put**: S_max - S_T
//!
//! where S_max and S_min are the maximum and minimum spot prices
//! over the monitoring period.
//!
//! # Pricing Methods
//!
//! - **Continuous monitoring**: Analytical formulas (Conze & Viswanathan 1991)
//! - **Discrete monitoring**: Monte Carlo simulation
//! - See [`models::closed_form::lookback`](crate::models::closed_form::lookback)
//!
//! # References
//!
//! - Conze, A., & Viswanathan (1991). "Path Dependent Options: The Case of
//!   Lookback Options." *Journal of Finance*, 46(5), 1893-1907.
//!
//! - Goldman, M. B., Sosin, H. B., & Gatto, M. A. (1979). "Path Dependent Options:
//!   Buy at the Low, Sell at the High." *Journal of Finance*, 34(5), 1111-1127.
//!
//! # See Also
//!
//! - [`LookbackOption`] for instrument struct
//! - [`LookbackType`] for fixed vs floating strike
//! - [`models::closed_form::lookback`](crate::models::closed_form::lookback) for formulas

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod traits;
pub(crate) mod types;

pub use types::{LookbackOption, LookbackOptionBuilder, LookbackType};
