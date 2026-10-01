//! Core types for financial statement modeling.

mod model;
mod node;
mod value;

pub(crate) use model::validated_explicit_value_type;
pub use model::{
    CapitalStructureSpec, DebtInstrumentSpec, FinancialModelSpec, FinancialStatementInstrument,
    FINANCIAL_MODEL_CONTRACT,
};
pub use node::{
    ForecastMethod, ForecastSpec, NodeId, NodeSpec, NodeType, NodeValueType, SeasonalMode,
};
pub use value::{infer_series_value_type, AmountOrScalar};
