//! Interest rate swap metrics and risk calculations.
//!
//! Provides specialized metric calculators for IRS instruments including:
//! - **Par Rate**: The fixed rate that makes the swap's NPV zero
//! - **Annuity**: Sum of discounted accrual factors (fixed leg)
//! - **DV01**: Dollar value of 1bp parallel curve shift
//! - **Bucketed DV01**: Key rate sensitivities at curve vertices
//! - **IR Convexity**: Second-order parallel rate sensitivity (gamma)
//! - **IR Cross-Gamma**: Mixed second derivative (discount vs forward curve)
//! - **PV Fixed/Float**: Individual leg present values
//!
//! # Module Organization
//!
//! Par rate and the leg PVs are defined in this file; the annuity, DV01 and
//! IR convexity calculators have their own files. DV01 uses the unified
//! calculator framework from `finstack_quant_valuations_core::metrics`.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_valuations_rates::irs::InterestRateSwap;
//! use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
//! use finstack_quant_valuations_core::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! # use time::Month;
//!
//! # fn example() -> finstack_quant_core::Result<()> {
//! let irs = InterestRateSwap::example()?;
//! // Build market context with required curves
//! let mut context = MarketContext::new();
//! // ... add USD-OIS and USD-SOFR-3M curves ...
//!
//! let as_of = Date::from_calendar_date(2024, Month::January, 1)
//!     .map_err(|e| finstack_quant_core::Error::Validation(format!("{}", e)))?;
//!
//! // Compute par rate and DV01
//! let metrics = vec![MetricId::ParRate, MetricId::Dv01];
//! let result = irs.price_with_metrics(&context, as_of, &metrics, PricingOptions::default())?;
//!
//! let par_rate = result.measures.get(MetricId::ParRate.as_str()).copied();
//! let dv01 = result.measures.get(MetricId::Dv01.as_str()).copied();
//! # let _ = (par_rate, dv01);
//! # Ok(())
//! # }
//! ```

pub(crate) mod annuity;
pub(crate) mod ir_convexity;
mod register;

pub(crate) use register::register_irs_metrics;
