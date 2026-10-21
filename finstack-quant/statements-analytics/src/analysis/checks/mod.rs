//! Domain-level validation checks for financial statement models.
//!
//! These checks go beyond the core accounting-identity and data-quality checks
//! in [`finstack_quant_statements::checks::builtins`] and test cross-statement
//! reconciliation, internal consistency, and credit reasonableness.
//!
//! Higher-level conveniences:
//!
//! - [`CreditMapping`] and [`ThreeStatementMapping`] — typed node-id mappings for common model patterns
//! - [`three_statement_checks`], [`credit_underwriting_checks`], and [`lbo_model_checks`] — pre-built check suites
//! - [`render_check_report_text`] and [`render_check_report_html`] — render [`finstack_quant_statements::checks::CheckReport`] as
//!   text or HTML

pub(crate) mod consistency;
pub(crate) mod credit;
pub(crate) mod mappings;
pub(crate) mod reconciliation;
pub(crate) mod renderer;
pub(crate) mod suites;

pub use consistency::{EffectiveTaxRateCheck, GrowthRateConsistency, WorkingCapitalConsistency};
pub use credit::{
    CoverageFloorCheck, FcfSignCheck, LeverageRangeCheck, LiquidityRunwayCheck, TrendCheck,
    TrendDirection,
};
pub use reconciliation::{
    CapexReconciliation, DepreciationReconciliation, DividendReconciliation,
    InterestExpenseReconciliation,
};

pub use mappings::{CreditMapping, ThreeStatementMapping};
pub use renderer::{render_check_report_html, render_check_report_text};
pub use suites::{credit_underwriting_checks, lbo_model_checks, three_statement_checks};
