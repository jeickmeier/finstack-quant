//! Autocallable test suite.
//!
//! This module organizes tests for autocallable structured product pricing:
//! - Day count basis: Validates consistent use of discount curve DC for all time calculations
//! - Determinism: MC seeding produces reproducible results
//! - Pricing bounds: Reasonable value ranges for various market conditions

mod helpers;
mod test_day_count_basis;
mod test_seasoned;
mod test_settlement_gap;
