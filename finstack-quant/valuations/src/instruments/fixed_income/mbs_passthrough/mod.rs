//! Agency MBS passthrough instrument module.
//!
//! This module provides the [`AgencyMbsPassthrough`] instrument for modeling
//! agency mortgage-backed securities (FNMA, FHLMC, GNMA) with prepayment
//! modeling, servicing fees, and payment delay conventions.
//!
//! # Overview
//!
//! Agency MBS passthroughs are securities backed by pools of conforming
//! mortgages, guaranteed by government-sponsored enterprises (GSEs) or
//! government agencies. Cashflows from the underlying mortgages (principal
//! and interest) are "passed through" to investors, net of servicing and
//! guarantee fees.
//!
//! # Key Features
//!
//! - **Prepayment modeling**: PSA curves, constant CPR, and stochastic models
//! - **Payment delays**: Agency-specific conventions (FNMA 25d, FHLMC/GNMA 45d)
//! - **Fee decomposition**: Servicing fees and guarantee fees
//! - **Risk metrics**: OAS, effective duration/convexity, key-rate DV01
//!
//! # Agency Programs
//!
//! - **FNMA (Fannie Mae)**: Federal National Mortgage Association
//! - **FHLMC (Freddie Mac)**: Federal Home Loan Mortgage Corporation
//! - **GNMA (Ginnie Mae)**: Government National Mortgage Association (government-backed)
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
//!     AgencyMbsPassthrough, AgencyProgram, PoolType,
//! };
//! use finstack_quant_cashflows::builder::specs::PrepaymentModelSpec;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::Month;
//!
//! // Create a FNMA 30-year passthrough
//! let mbs = AgencyMbsPassthrough::builder()
//!     .id(InstrumentId::new("FN-MA1234"))
//!     .pool_id("MA1234".into())
//!     .agency(AgencyProgram::Fnma)
//!     .pool_type(PoolType::Generic)
//!     .original_face(Money::new(1_000_000.0, Currency::USD))
//!     .current_face(Money::new(950_000.0, Currency::USD))
//!     .current_factor(0.95)
//!     .wac(0.045)
//!     .pass_through_rate(0.04)
//!     .servicing_fee_rate(0.0025)
//!     .guarantee_fee_rate(0.0025)
//!     .wam(348)
//!     .issue_date(Date::from_calendar_date(2022, Month::January, 1).unwrap())
//!     .maturity(Date::from_calendar_date(2052, Month::January, 1).unwrap())
//!     .prepayment_model(PrepaymentModelSpec::psa(1.0))
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .day_count(finstack_quant_core::dates::DayCount::Thirty360)
//!     .build()
//!     .expect("Valid MBS");
//!
//! // Use the example constructor for quick testing
//! let example_mbs = AgencyMbsPassthrough::example().unwrap();
//! ```

pub mod delay;
pub(crate) mod metrics;
pub mod prepayment;
pub(crate) mod pricer;
pub mod servicing;
mod types;

pub use pricer::{generate_cashflows, MbsCashflow};
pub use types::{AgencyMbsPassthrough, AgencyProgram, PoolType};
