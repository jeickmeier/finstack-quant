//! Correlation structures for structured credit.
//!
//! This module provides correlation specifications that capture:
//! - Asset correlation (intra-pool default correlation)
//! - Prepay-default correlation (typically negative)
//! - Sector correlation (intra vs inter-sector)
//!
//! # Industry Standard Calibrations
//!
//! ## RMBS
//! - Asset correlation: 5-10% (diversified mortgage pools)
//! - Prepay-default correlation: -20% to -40% (refi incentive vs credit)
//!
//! ## CLO
//! - Intra-sector correlation: 25-35%
//! - Inter-sector correlation: 10-15%
//! - Prepay-default correlation: -15% to -25%
//!
//! ## CMBS
//! - Asset correlation: 15-25% (concentrated property types)
//! - Prepay-default correlation: -10% to -20%

mod structure;

pub use structure::CorrelationStructure;
