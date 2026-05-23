//! Inflation indexation specification for inflation-linked instruments.

use finstack_core::dates::Date;
use finstack_core::market_data::scalars::InflationLag;

use crate::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod,
};

/// Inflation indexation parameters that describe how an instrument's
/// cashflows are adjusted for inflation.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct IndexationSpec {
    /// Base CPI/index value at issue.
    pub base_index: f64,
    /// Base date for index (may differ from issue date).
    #[schemars(with = "String")]
    pub base_date: Date,
    /// Indexation method (TIPS, Canadian, UK, French, Japanese).
    pub indexation_method: IndexationMethod,
    /// Inflation lag (e.g., 3 months for TIPS, 8 months for legacy UK).
    pub lag: InflationLag,
    /// Deflation protection policy.
    pub deflation_protection: DeflationProtection,
}
