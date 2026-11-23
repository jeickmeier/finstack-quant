//! Credit scorecard analysis extension.
//!
//! Credit rating assignment from financial metrics and configurable
//! thresholds on a caller-supplied scorecard scale, with weighted scoring and
//! optional minimum-rating compliance checks. The scale is resolved once from
//! the supplied [`RatingScaleRegistry`] and kept as an immutable snapshot;
//! there is no embedded or default scale (record CORE-CONVENTIONS-019).
//!
//! # Configuration Schema
//!
//! ```json
//! {
//!   "rating_scale": "internal",
//!   "metrics": [
//!     {
//!       "name": "debt_to_ebitda",
//!       "formula": "total_debt / ttm(ebitda)",
//!       "weight": 0.3,
//!       "thresholds": {
//!         "AAA": [0.0, 1.0],
//!         "AA": [1.0, 2.0],
//!         "A": [2.0, 3.0],
//!         "BBB": [3.0, 4.0],
//!         "BB": [4.0, 5.0],
//!         "B": [5.0, 6.0],
//!         "CCC": [6.0, 999.0]
//!       }
//!     },
//!     {
//!       "name": "interest_coverage",
//!       "formula": "ebitda / interest_expense",
//!       "weight": 0.25,
//!       "thresholds": {
//!         "AAA": [8.0, 999.0],
//!         "AA": [6.0, 8.0],
//!         "A": [4.5, 6.0],
//!         "BBB": [3.0, 4.5],
//!         "BB": [2.0, 3.0],
//!         "B": [1.0, 2.0],
//!         "CCC": [0.0, 1.0]
//!       }
//!     }
//!   ]
//! }
//! ```
//!
//! # Example Usage
//!
//! ```no_run
//! use finstack_quant_core::rating_scales::RatingScaleRegistry;
//! use finstack_quant_statements_analytics::extensions::{
//!     CreditScorecardExtension, ScorecardConfig, ScorecardMetric,
//! };
//! use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_quant_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_quant_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! # let registry_json: &str = unimplemented!("a finstack_quant.rating_scales/2 document");
//! let registry = RatingScaleRegistry::from_json(registry_json)?;
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = ScorecardConfig {
//!     rating_scale: "internal".into(),
//!     metrics: vec![ScorecardMetric {
//!         name: "debt_to_ebitda".into(),
//!         formula: "total_debt / ttm(ebitda)".into(),
//!         weight: 1.0,
//!         thresholds: indexmap::IndexMap::new(),
//!         description: None,
//!     }],
//!     min_rating: None,
//!     period: None,
//! };
//!
//! let extension = CreditScorecardExtension::new(config, &registry)?;
//! let report = extension.execute(&model, &results)?;
//! # let _ = report;
//! # Ok(())
//! # }
//! ```

pub(crate) mod extension;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) use extension::{CreditScorecardExtension, ScorecardConfig, ScorecardMetric};
