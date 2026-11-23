//! Error types for financial computation and validation failures.
//!
//! This module defines the unified error hierarchy used throughout Finstack Quant's
//! core library. All errors bubble up through the [`enum@Error`] enum, which wraps
//! domain-specific failures like input validation, interpolation bounds,
//! currency mismatches, and calibration failures.
//!
//! # Design Philosophy
//!
//! - **Actionable errors**: Each variant includes enough context for callers to
//!   diagnose and potentially recover from failures
//! - **Non-exhaustive**: Error variants may expand in minor releases; always
//!   match with a catch-all `_` pattern for forward compatibility
//! - **Fuzzy suggestions**: Missing curve errors include similar IDs based on
//!   edit distance to guide users toward corrections
//! - **Serializable**: All error types support serde wire formats
//!
//! # Error Categories
//!
//! - **Input validation** ([`InputError`]): User-supplied data fails constraints
//!   (e.g., non-monotonic knots, invalid dates, missing curves)
//! - **Currency safety** ([`Error::CurrencyMismatch`]): Attempted cross-currency
//!   arithmetic without explicit conversion
//! - **Calibration** ([`Error::Calibration`]): Numerical solver or fitting
//!   procedure failed to converge
//! - **Validation** ([`Error::Validation`]): Market data fails no-arbitrage or
//!   structural checks
//!
//! # Examples
//!
//! ## Handling common input errors
//!
//! ```rust
//! use finstack_quant_core::{Error, InputError};
//!
//! fn parse_knots(data: &[(f64, f64)]) -> Result<(), Error> {
//!     if data.len() < 2 {
//!         return Err(InputError::TooFewPoints.into());
//!     }
//!
//!     // Check monotonicity
//!     for window in data.windows(2) {
//!         if window[1].0 <= window[0].0 {
//!             return Err(InputError::NonMonotonicKnots.into());
//!         }
//!     }
//!
//!     Ok(())
//! }
//!
//! let invalid_data = vec![(1.0, 0.95), (0.5, 0.90)]; // Non-monotonic
//! assert!(parse_knots(&invalid_data).is_err());
//! ```
//!
//! ## Currency mismatch detection
//!
//! ```rust
//! use finstack_quant_core::Error;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//!
//! let usd = Money::from((100_i64, Currency::USD));
//! let eur = Money::from((85_i64, Currency::EUR));
//!
//! // Attempting to add different currencies returns CurrencyMismatch
//! let result = usd.checked_add(eur);
//! assert!(result.is_err());
//!
//! match result {
//!     Err(Error::CurrencyMismatch { expected, actual }) => {
//!         assert_eq!(expected, Currency::USD);
//!         assert_eq!(actual, Currency::EUR);
//!     }
//!     _ => panic!("Expected currency mismatch"),
//! }
//! ```
//!
//! ## Using error suggestions for missing curves
//!
//! ```rust
//! use finstack_quant_core::Error;
//!
//! let available = vec![
//!     "USD_OIS".to_string(),
//!     "EUR_OIS".to_string(),
//!     "GBP_GILT".to_string(),
//! ];
//!
//! // Typo in curve name
//! let err = Error::missing_curve_with_suggestions("USD_OS", &available);
//! let msg = format!("{}", err);
//!
//! // Error message includes suggestions
//! assert!(msg.contains("USD_OIS") || msg.contains("Did you mean"));
//! ```
//!
//! # See Also
//!
//! - [`crate::Result`] - Type alias for `Result<T, Error>`
//! - [`InputError`] - Specific validation failure modes
//!
//! # References
//!
//! The fuzzy matching algorithm uses Levenshtein edit distance: `docs/REFERENCES.md#levenshtein-1966`
//! - Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
//!   insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707-710. `docs/REFERENCES.md#levenshtein-1966`

pub(crate) mod inputs;
mod suggestions;
#[cfg(test)]
mod tests;
pub(crate) mod types;

pub(crate) use inputs::{InputError, NonFiniteKind};
#[cfg(test)]
pub(super) use types::{Error, ErrorKind, format_chain};
