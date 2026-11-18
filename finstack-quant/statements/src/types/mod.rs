//! Core types for financial statement modeling.

mod edit;
mod forecast_params;
mod model;
mod node;
mod value;

pub use edit::ModelEdit;
pub(crate) use forecast_params::seed as forecast_seed;
pub use forecast_params::{
    BootstrapParams, BoundsParams, CurvePctParams, DistributionParams, FadeToTargetParams,
    ForecastBootstrapMode, ForecastFadeShape, ForecastSpec, ForecastTrendMethod, GrowthPctParams,
    MeanRevertingParams, OverrideParams, SeasonalParams, TimeSeriesParams,
};
pub(crate) use model::validated_explicit_value_type;
pub use model::{
    CapitalFxPolicy, CapitalStructureSpec, DebtInstrumentSpec, FINANCIAL_MODEL_CONTRACT,
    FinancialModelSpec, FinancialStatementInstrument,
};
pub use node::{
    ForecastMethod, NodeId, NodeSpec, NodeType, NodeValueType, RollupMethod, SeasonalMode,
};
pub use value::{AmountOrScalar, infer_series_value_type};
