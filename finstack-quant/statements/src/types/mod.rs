//! Core types for financial statement modeling.

mod forecast_params;
mod model;
mod node;
mod value;

pub use forecast_params::{
    BootstrapParams, BoundsParams, CurvePctParams, DistributionParams, FadeToTargetParams,
    ForecastBootstrapMode, ForecastFadeShape, ForecastParameters, ForecastTrendMethod,
    GrowthPctParams, MeanRevertingParams, OverrideParams, SeasonalParams, TimeSeriesParams,
};
pub(crate) use model::validated_explicit_value_type;
pub use model::{
    CapitalStructureSpec, DebtInstrumentSpec, FINANCIAL_MODEL_CONTRACT, FinancialModelSpec,
    FinancialStatementInstrument,
};
pub use node::{
    ForecastMethod, ForecastSpec, NodeId, NodeSpec, NodeType, NodeValueType, RollupMethod,
    SeasonalMode,
};
pub use value::{AmountOrScalar, infer_series_value_type};
