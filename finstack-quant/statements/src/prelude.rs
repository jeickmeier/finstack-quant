//! Commonly used types and traits.
//!
//! Import this module to get quick access to the most common types:
//!
//! ```rust
//! use finstack_quant_statements::prelude::*;
//! ```
//!
//! This prelude re-exports the most common `finstack-quant-statements` types plus
//! the core money/date primitives that statements-centric models typically
//! need. Prefer importing from the source module directly when you want a
//! narrower API boundary in libraries or examples aimed at teaching a
//! specific subsystem.

pub use crate::builder::{MixedNodeBuilder, ModelBuilder};
pub use crate::checks::builtins::{
    BalanceSheetArticulation, CashReconciliation, MissingValueCheck, NonFiniteCheck,
    RetainedEarningsReconciliation, SignConventionCheck,
};
pub use crate::checks::{
    Check, CheckCategory, CheckConfig, CheckFinding, CheckReport, CheckResult, CheckSuite,
    CheckSuiteSpec, CheckSummary, Materiality, PeriodScope, Severity,
};
pub use crate::error::{Error, Result};
pub use crate::evaluator::{Evaluator, NumericMode, PreparedEvaluation, StatementResult};
pub use crate::registry::Registry;
pub use crate::types::{
    AmountOrScalar, FinancialModelSpec, ForecastMethod, ForecastSpec, NodeId, NodeSpec, NodeType,
    NodeValueType, SeasonalMode,
};

pub use finstack_quant_core::currency::Currency;
pub use finstack_quant_core::dates::{
    adjust, build_periods, BusinessDayConvention, Calendar, Date, DayCount, Period, PeriodId,
    PeriodKind, ScheduleBuilder, Tenor,
};
pub use finstack_quant_core::money::{
    fx::{FxConversionPolicy, FxMatrix, FxProvider},
    Money,
};
