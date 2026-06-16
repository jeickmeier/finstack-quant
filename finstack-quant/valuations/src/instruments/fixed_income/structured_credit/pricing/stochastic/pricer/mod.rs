//! Stochastic pricing engine for structured credit.
//!
//! This module provides tree-based and Monte Carlo pricing for structured credit
//! instruments with stochastic prepayment and default models.
//!
//! # Pricing Modes
//!
//! - **Tree-based**: Exact pricing using the recombining scenario lattice
//! - **Monte Carlo**: Statistical re-sampling of the tree distribution (with optional variance reduction)
//!
//! # Features
//!
//! - Path-dependent state tracking (burnout, cumulative losses)
//! - Tranche-level cashflow generation
//! - Multi-currency support via FX rates
//! - Risk metric computation (EL, UL, ES)
//!
//! # Example
//!
//! ```text
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_valuations::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::{
//!     StochasticPricer, StochasticPricerConfig,
//! };
//! use std::sync::Arc;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let valuation_date = Date::from_calendar_date(2025, Month::January, 1)?;
//! let discount_curve = Arc::new(
//!     DiscountCurve::builder("USD-OIS")
//!         .base_date(valuation_date)
//!         .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
//!         .build()
//!         .expect("discount curve should build"),
//! );
//! let pool_coupon = 0.045;
//! let horizon_years = 5.0;
//!
//! let config = StochasticPricerConfig::rmbs_standard(
//!     valuation_date,
//!     discount_curve,
//!     pool_coupon,
//!     horizon_years,
//! );
//!
//! let pricer = StochasticPricer::new(config);
//! let result = pricer.price(1_000_000.0, Currency::USD).expect("pricing should succeed");
//!
//! println!("NPV: {}", result.npv);
//! println!("Expected Loss: {}", result.expected_loss);
//! println!("Unexpected Loss: {}", result.unexpected_loss);
//! # Ok(())
//! # }
//! ```

mod config;
mod engine;
mod result;

pub use config::PricingMode;
pub(crate) use config::StochasticPricerConfig;
pub(crate) use engine::StochasticPricer;
pub use result::{StochasticPricingResult, TranchePricingResult};
