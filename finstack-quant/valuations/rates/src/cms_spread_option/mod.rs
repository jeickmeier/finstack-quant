//! CMS Spread Option - option on the spread between two CMS rates.
//!
//! CMS spread options are European-style options whose payoff depends on
//! the difference between two constant maturity swap rates (e.g., 10Y CMS
//! minus 2Y CMS). They are widely used for curve steepener/flattener views.
//!
//! # Pricing
//!
//! The implemented approximation combines lognormal CMS marginals using a
//! Gaussian copula. Flat-strike Black volatility surfaces (including tenor-axis
//! ATM surfaces) are supported; SABR cubes and nonflat smiles are rejected.
//! Each marginal mean uses the shared first-order CMS convexity adjustment.
//!
//! # See Also
//!
//! - [`CmsSpreadOption`] for instrument definition

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::CmsSpreadOptionPricer;
pub use types::CmsSpreadOption;
