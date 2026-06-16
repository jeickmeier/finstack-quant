//! Internal consistency checks.
//!
//! These checks verify that derived metrics within a single model are
//! internally coherent — for example, that growth rates stay within plausible
//! bounds, effective tax rates fall in expected ranges, and working-capital
//! changes on the cash flow statement match balance-sheet deltas.

mod growth_rate;
mod tax_rate;
mod working_capital;

pub use growth_rate::GrowthRateConsistency;
pub use tax_rate::EffectiveTaxRateCheck;
pub use working_capital::WorkingCapitalConsistency;
