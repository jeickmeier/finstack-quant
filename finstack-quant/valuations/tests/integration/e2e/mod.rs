//! End-to-End Integration Tests
//!
//! Complete workflow tests that exercise multiple system components together:
//!
//! - [`bond_portfolio`]: Multi-currency bond portfolio pricing with metrics
//! - [`fx_settlement`]: FX spot date calculations with joint holiday calendars

pub mod bond_portfolio;
pub mod fx_settlement;
