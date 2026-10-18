//! Comprehensive unit tests for common instruments module.
//!
//! Organized by:
//! - parameters: Parameter types and conventions
//! - pricer: Registry and instrument-type contract tests
//!
//! Curve builders, tolerances and parity helpers live in
//! `finstack_quant_valuations_test_support`.

pub mod parameters;
pub mod pricer;
pub mod test_callable_credit_baseline;
pub mod test_callable_floating_resets;
pub mod test_discountable;
