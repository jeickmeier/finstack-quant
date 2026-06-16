//! CMS Spread Option - option on the spread between two CMS rates.
//!
//! CMS spread options are European-style options whose payoff depends on
//! the difference between two constant maturity swap rates (e.g., 10Y CMS
//! minus 2Y CMS). They are widely used for curve steepener/flattener views.
//!
//! # Pricing
//!
//! Standard approach uses SABR marginals for each CMS rate combined via
//! Gaussian copula. CMS convexity adjustments are applied via static
//! replication.
//!
//! # See Also
//!
//! - [`CmsSpreadOption`] for instrument definition
//! - [`CmsSpreadOptionType`] for call/put selection

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::CmsSpreadOptionPricer;
pub use types::{CmsSpreadOption, CmsSpreadOptionType};
