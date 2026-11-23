//! Internal consistency checks.
//!
//! These checks verify that derived metrics within a single model are
//! internally coherent — for example, that growth rates stay within plausible
//! bounds, effective tax rates fall in expected ranges, and working-capital
//! changes on the cash flow statement match balance-sheet deltas.

pub(crate) mod growth_rate;
pub(crate) mod tax_rate;
pub(crate) mod working_capital;

pub(crate) use growth_rate::GrowthRateConsistency;
pub(crate) use tax_rate::EffectiveTaxRateCheck;
pub(crate) use working_capital::WorkingCapitalConsistency;
