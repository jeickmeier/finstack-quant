//! FX Option comprehensive test suite.
//!
//! This module provides test coverage (>80%) for the FX option
//! implementation, organized by concern:
//!
//! - `helpers`: Shared test utilities and fixtures
//! - `test_calculator`: Core calculator unit tests (npv, inputs, validation)
//! - `test_greeks`: Greek calculations with finite difference validation
//! - `test_implied_vol`: Implied volatility solver tests
//! - `test_put_call_parity`: Market standard put-call parity validation
//! - `test_edge_cases`: Edge cases, boundaries, and error handling
//! - `test_instrument`: Instrument construction and trait implementations
//!
//! ## Test Philosophy
//!
//! 1. **Arrange-Act-Assert (AAA)**: All tests follow this pattern
//! 2. **Isolation**: Tests use mocks and don't depend on external systems
//! 3. **Determinism**: Results are reproducible
//! 4. **Market Standards**: Include validation of no-arbitrage relationships
//! 5. **Edge Coverage**: Comprehensive boundary and error testing

mod helpers;

mod test_calculator;
mod test_conventions;
mod test_edge_cases;
mod test_greeks;
mod test_implied_vol;
mod test_instrument;
mod test_put_call_parity;
