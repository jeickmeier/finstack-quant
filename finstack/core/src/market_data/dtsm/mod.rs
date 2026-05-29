//! Dynamic term structure models (Diebold-Li, PCA).
//!
//! This module extends the static parametric curve infrastructure with
//! time-series models that capture how the yield curve evolves over time.
//! It provides two complementary approaches:
//!
//! 1. **Diebold-Li**: Treats Nelson-Siegel factors as time-varying, enabling
//!    yield curve forecasting via VAR(1) dynamics.
//! 2. **PCA**: Decomposes yield curve changes into orthogonal principal
//!    components for risk decomposition and scenario generation.
//!
//! # Architecture
//!
//! Both models operate on a [`YieldPanel`][crate::market_data::dtsm::YieldPanel]
//! -- a matrix of yield observations across dates and tenors. The Diebold-Li
//! model extracts interpretable Nelson-Siegel factors (level, slope, curvature)
//! and models their dynamics. PCA provides a purely statistical decomposition of
//! yield changes.
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_core::market_data::dtsm::{DieboldLi, YieldPca, YieldPanel};
//! use nalgebra::DMatrix;
//!
//! // Build a yield panel with time-varying NS-shaped curves.
//! // Years-convention lambda (12 × Diebold-Li's 0.0609 months value, matching
//! // the builder default) so the curvature peak sits at ≈2.45 years given the
//! // year-denominated tenors below.
//! let lambda = 0.7308_f64;
//! let tenors = vec![0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
//! let n = tenors.len();
//! let t = 50;
//! let yields = DMatrix::from_fn(t, n, |i, j| {
//!     let tau = tenors[j];
//!     let lt = lambda * tau;
//!     let slope = (1.0 - (-lt).exp()) / lt;
//!     let b0 = 0.06 + 0.002 * ((i as f64) * 0.3).sin();
//!     let b1 = -0.02 + 0.001 * ((i as f64) * 0.5).cos();
//!     let b2 = 0.01 + 0.001 * ((i as f64) * 0.7).sin();
//!     b0 + b1 * slope + b2 * (slope - (-lt).exp())
//! });
//! let panel = YieldPanel::new(yields, tenors, None).unwrap();
//!
//! // Diebold-Li: extract factors, fit VAR, forecast
//! let model = DieboldLi::builder()
//!     .lambda(lambda)
//!     .build()
//!     .unwrap()
//!     .extract_factors(&panel)
//!     .unwrap()
//!     .fit_var()
//!     .unwrap();
//!
//! let _forecast = model.forecast(12).unwrap();
//!
//! // PCA: decompose yield changes
//! let pca = YieldPca::fit(&panel).unwrap();
//! let _scenario_shock = pca.scenario(&[2.0, -1.0]).unwrap();
//! ```
//!
//! # References
//!
//! - Diebold, F. X., & Li, C. (2006). "Forecasting the Term Structure of
//!   Government Bond Yields." *Journal of Econometrics*, 130(2), 337-364.
//! - Litterman, R., & Scheinkman, J. (1991). "Common Factors Affecting
//!   Bond Returns." *Journal of Fixed Income*, 1(1), 54-61.

/// Diebold-Li dynamic Nelson-Siegel model.
pub mod diebold_li;
/// PCA-based yield curve analysis.
pub mod pca;
/// Shared types: YieldPanel, FactorTimeSeries, YieldForecast.
pub mod types;

// Re-export primary types for convenience
pub use diebold_li::{DieboldLi, DieboldLiBuilder};
pub use pca::YieldPca;
pub use types::{FactorTimeSeries, YieldForecast, YieldPanel};
