//! Core types for financial statement modeling.

mod model;
mod node;
mod value;

pub use model::{CapitalStructureSpec, DebtInstrumentSpec, FinancialModelSpec};
pub use node::{
    ForecastMethod, ForecastSpec, NodeId, NodeSpec, NodeType, NodeValueType, SeasonalMode,
};
pub use value::{infer_series_value_type, AmountOrScalar};
