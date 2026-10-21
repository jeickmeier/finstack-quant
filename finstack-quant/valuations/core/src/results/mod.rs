//! Valuation result types and output formatting.
//!
//! This module provides the result envelope types returned by pricing operations,
//! encapsulating present values, computed metrics, and execution metadata.
//!
//! # Features
//!
//! - **ValuationResult**: Standard result envelope with PV and metrics
//! - **ResultsMeta**: Metadata tracking config, timing, and FX policy
//! - **Table Export**: Flatten results into `core::table` rows for pandas/Polars consumers
//!
//! # Result Structure
//!
//! Every pricing operation returns a [`crate::results::ValuationResult`] containing:
//!
//! ```text
//! ValuationResult {
//!     value: Money,              // Present value in instrument currency
//!     measures: IndexMap<MetricId, f64>,  // Computed metrics (DV01, Greeks, etc.)
//!     meta: ResultsMeta,         // Execution metadata
//! }
//! ```
//!
//! # Metadata Tracking
//!
//! [`finstack_quant_core::config::ResultsMeta`] captures important context for audit and reproducibility:
//! - **Timestamp**: When the valuation was computed
//! - **Rounding context**: Numeric precision policy applied
//! - **FX policy**: Currency conversion method (if applicable)
//! - **Parallel flag**: Whether parallel execution was used
//!
//! # Quick Example
//!
//! The umbrella crate's `results` module documentation prices a bond and reads
//! its metrics back from the envelope.
//!
//! # See Also
//!
//! - [`crate::results::ValuationResult`] for the main result type
//! - [`finstack_quant_core::config::ResultsMeta`] for execution metadata
//! - [`crate::metrics`] for available metric calculators
//!
//! # References
//!
//! - Metric and sensitivity interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

pub mod composite;
pub(crate) mod dataframe;
mod provenance;
pub mod structured_credit;
mod valuation_result;

pub use dataframe::{ValuationLongRow, ValuationRow};
pub use provenance::{SensitivityBumps, ValuationProvenance};
pub use valuation_result::{
    CreditDerivativeValuationDetails, FxValuationDetails, MonteCarloValuationDetails,
    ValuationDetails, ValuationResult,
};
