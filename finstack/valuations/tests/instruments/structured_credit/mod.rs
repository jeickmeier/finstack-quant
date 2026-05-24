//! Comprehensive test suite for structured credit instruments.
//!
//! # Test Organization
//!
//! ## Unit Tests (`unit/`)
//! - **components/**: Tests for structural building blocks
//!   - pool: AssetPool, PoolAsset, pool statistics
//!   - tranches: Tranche, TrancheStructure, attachment points
//!   - waterfall: Waterfall, payment rules, diversion
//!   - coverage: OC/IC test calculations
//!   - rates: CPR/SMM, CDR/MDR, PSA conversion utilities
//!   - specs: Prepayment, default, recovery model specifications
//! - **metrics/**: Tests for risk and valuation metrics
//!   - pricing: Accrued, dirty/clean prices, WAL calculations
//!   - risk: Duration, spreads (Z-spread, CS01), YTM
//!   - pool: WAM, CPR, CDR, WARF, WAS calculations
//!   - deal_specific: ABS, CMBS, RMBS specific metrics
//! - **utils/**: Utility function tests (dates, rating factors, reinvestment)
//!
//! ## Integration Tests (`integration/`)
//! - **cashflow_generation**: End-to-end waterfall execution
//! - **pricing**: Full NPV and metric computation
//! - **deal_types**: CLO, ABS, CMBS, RMBS specific workflows
//! - **serialization**: JSON roundtrip and wire format stability
//!
//! ## Examples (`examples.rs`)
//! - Preserved as living documentation
//!
//! # Testing Philosophy
//!
//! 1. **Unit tests** focus on individual functions and components
//! 2. **Integration tests** verify end-to-end workflows
//! 3. **Examples** serve as both documentation and smoke tests
//! 4. Follow **AAA pattern**: Arrange, Act, Assert
//! 5. Use **descriptive names**: `test_<component>_<scenario>_<expected_result>`
//! 6. **Deterministic**: No randomness, fixed dates and amounts
//! 7. **Isolated**: Each test is independent

// Unit tests
pub mod unit;

// Integration tests
pub mod integration;

// Examples (documentation)
pub mod examples;

// Golden tests (real-world scenarios)
pub mod waterfall_golden;

// Property tests (invariant verification)
pub mod waterfall_properties;

// Academic golden tests (Fabozzi, PSA, WARF references)
pub mod academic_golden;

// BucketedDv01 smoke tests
pub mod test_bucketed_dv01;

// Cashflow sensitivity sweep tests (CDR/CPR/Recovery/Lag)
pub mod cashflow_sweep_tests;

// End-to-end simulation tests with hand-calculated expectations
pub mod simulation_e2e_tests;

// Feature tests: loss allocation, cleanup call, reserve account, OC/IC cure
pub mod feature_tests;
