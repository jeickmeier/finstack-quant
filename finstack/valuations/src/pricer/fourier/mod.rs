//! Fourier pricing methods for European options.
//!
//! This module provides model-agnostic Fourier pricing engines that work
//! with any model implementing the [`CharacteristicFunction`] trait from
//! `finstack-core`. One method is currently available:
//!
//! - **COS method** ([`cos::CosPricer`]): Fang-Oosterlee (2008) cosine series
//!   expansion. O(N) per strike, fastest single-strike Fourier method.
//!   Automatic truncation range from cumulants. No external dependencies.
//!
//! # Method Selection Guide
//!
//! | Use Case | Recommended Method |
//! |----------|-------------------|
//! | Single-strike pricing | COS (fastest) |
//! | Strip pricing (5-50 strikes) | COS with strip reuse |
//! | Arbitrary strike, no grid | COS (recommended) |
//! | WASM / latency-critical | COS (smallest N) |
//!
//! # Example
//!
//! ```ignore
//! use finstack_core::math::characteristic_function::BlackScholesCf;
//! use finstack_valuations::pricer::fourier::cos::{CosPricer, CosConfig};
//!
//! let cf = BlackScholesCf { r: 0.05, q: 0.0, sigma: 0.2 };
//!
//! // COS method (r encodes the drift already present in the CF)
//! let cos = CosPricer::new(&cf, CosConfig::default());
//! let call = cos.price_call(100.0, 100.0, 0.05, 1.0);
//! ```
//!
//! # Removed: Lewis (2001) pricer
//!
//! A `LewisPricer` was previously exposed here but was known-divergent
//! off-ATM and silently dropped non-finite integrand panels behind a
//! `max(0.0)` clamp. Because no internal pricer consumed it and callers
//! had no way to distinguish its correct ATM behavior from its off-ATM
//! collapse, it has been removed. Use [`cos::CosPricer`] for all
//! Fourier pricing; it handles arbitrary strikes with Fang-Oosterlee
//! truncation from cumulants.
//!
//! [`CharacteristicFunction`]: finstack_core::math::characteristic_function::CharacteristicFunction

pub mod cos;

pub use cos::{CosConfig, CosPricer};
