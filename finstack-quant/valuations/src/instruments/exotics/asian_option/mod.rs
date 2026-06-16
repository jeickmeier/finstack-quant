//! Asian option instruments with analytical and Monte Carlo pricing.
//!
//! Asian options (average options) have payoffs based on the average price
//! of the underlying over the option's life, making them less sensitive to
//! spot price manipulation and reducing volatility exposure.
//!
//! # Payoff Structure
//!
//! - **Arithmetic average call**: max(Avg(S) - K, 0)
//! - **Arithmetic average put**: max(K - Avg(S), 0)
//! - **Geometric average call**: max(GeoAvg(S) - K, 0)
//! - **Geometric average put**: max(K - GeoAvg(S), 0)
//!
//! # Pricing Methods
//!
//! - **Geometric average**: Exact closed-form (Kemna & Vorst 1990)
//! - **Arithmetic average**: Turnbull-Wakeman approximation or Monte Carlo
//! - See [`models::closed_form::asian`](crate::models::closed_form::asian) for formulas
//!
//! # References
//!
//! See the analytical module for complete citations to:
//! - Kemna & Vorst (1990) - Geometric average
//! - Turnbull & Wakeman (1991) - Arithmetic approximation
//!
//! # See Also
//!
//! - [`AsianOption`] for instrument struct
//! - [`AveragingMethod`] for geometric vs arithmetic
//! - [`models::closed_form::asian`](crate::models::closed_form::asian) for pricing formulas

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod traits;
pub(crate) mod types;

pub(crate) mod heston_mc_pricer;

pub use pricer::{AsianOptionAnalyticalGeometricPricer, AsianOptionSemiAnalyticalTwPricer};
pub use types::{AsianOption, AsianOptionBuilder, AveragingMethod};
