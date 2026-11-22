//! Python bindings for the `finstack-quant-models::liquidity` submodule.
//!
//! Exposes market microstructure liquidity modeling: spread estimation
//! (Roll, Amihud), liquidity-adjusted VaR (Bangia et al.), market impact
//! (Almgren-Chriss, Kyle) and tier classification.
//!
//! Series inputs are plain `Vec<f64>` so callers can pass numpy arrays or
//! lists directly. Results are typed wrappers over the canonical Rust structs
//! (`LvarBangiaScalar`, `ImpactEstimate`, `ExecutionTrajectory`) with JSON,
//! pickle and pandas exits.

use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_models::liquidity::{
    self, AlmgrenChrissModel, ExecutionTrajectory, ImpactEstimate, KyleLambdaModel,
    LiquidityProfile, LvarBangiaScalar, TradeParams,
};
use pyo3::prelude::*;
use pyo3::types::PyType;

#[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile")]
#[pyclass(
    name = "LiquidityProfile",
    module = "finstack_quant.models.liquidity",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLiquidityProfile {
    pub(crate) inner: LiquidityProfile,
}

impl PyLiquidityProfile {
    pub(crate) fn from_inner(inner: LiquidityProfile) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLiquidityProfile {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, instrument_id, mid, bid, ask, avg_daily_volume, avg_trade_size, spread_volatility, spread_volatility_kind='relative', observation_days=20)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: LiquidityProfile =
            crate::bindings::module_utils::fields_from_py(py, fields, "LiquidityProfile")?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LiquidityProfile", &self.inner)
    }
}

py_getters!(PyLiquidityProfile |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.clone();

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.mid")]
    mid -> f64 = s.inner.mid;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.bid")]
    bid -> f64 = s.inner.bid;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.ask")]
    ask -> f64 = s.inner.ask;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.avg_daily_volume")]
    avg_daily_volume -> f64 = s.inner.avg_daily_volume;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.avg_trade_size")]
    avg_trade_size -> f64 = s.inner.avg_trade_size;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread_volatility")]
    spread_volatility -> f64 = s.inner.spread_volatility;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread_volatility_kind")]
    spread_volatility_kind -> PyResult<String> = finstack_quant_core::wire::serde_label(&s.inner.spread_volatility_kind).map_err(core_to_py);

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.observation_days")]
    observation_days -> u32 = s.inner.observation_days;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread")]
    spread -> f64 = s.inner.spread();

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.relative_spread")]
    relative_spread -> f64 = s.inner.relative_spread();

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.half_spread")]
    half_spread -> f64 = s.inner.half_spread();

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.relative_spread_volatility")]
    relative_spread_volatility -> f64 = s.inner.relative_spread_volatility();
});

wire_methods!(
    PyLiquidityProfile,
    LiquidityProfile,
    "LiquidityProfile",
    module = "finstack_quant.models.liquidity"
);

#[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams")]
#[pyclass(
    name = "TradeParams",
    module = "finstack_quant.models.liquidity",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyTradeParams {
    pub(crate) inner: TradeParams,
}

#[pymethods]
impl PyTradeParams {
    #[new]
    #[pyo3(signature = (quantity, horizon_days, daily_volatility, profile, risk_aversion = None, reference_price = None))]
    #[pyo3(
        text_signature = "(quantity, horizon_days, daily_volatility, profile, risk_aversion=None, reference_price=None)"
    )]
    fn new(
        quantity: f64,
        horizon_days: f64,
        daily_volatility: f64,
        profile: PyLiquidityProfile,
        risk_aversion: Option<f64>,
        reference_price: Option<f64>,
    ) -> Self {
        Self {
            inner: TradeParams {
                quantity,
                horizon_days,
                daily_volatility,
                profile: profile.inner,
                risk_aversion,
                reference_price,
            },
        }
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TradeParams", &self.inner)
    }
}

py_getters!(PyTradeParams |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.quantity")]
    quantity -> f64 = s.inner.quantity;

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.horizon_days")]
    horizon_days -> f64 = s.inner.horizon_days;

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.daily_volatility")]
    daily_volatility -> f64 = s.inner.daily_volatility;

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.profile")]
    profile -> PyLiquidityProfile = PyLiquidityProfile::from_inner(s.inner.profile.clone());

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.risk_aversion")]
    risk_aversion -> Option<f64> = s.inner.risk_aversion;

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.reference_price")]
    reference_price -> Option<f64> = s.inner.reference_price;

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.effective_reference_price")]
    effective_reference_price -> f64 = s.inner.effective_reference_price();
});

wire_methods!(
    PyTradeParams,
    TradeParams,
    "TradeParams",
    module = "finstack_quant.models.liquidity"
);

const IMPACT_ESTIMATE_COLUMNS: [&str; 5] = [
    "permanent_impact",
    "temporary_impact",
    "total_cost",
    "cost_bp",
    "execution_risk",
];

#[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate")]
#[pyclass(
    name = "ImpactEstimate",
    module = "finstack_quant.models.liquidity",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyImpactEstimate {
    pub(crate) inner: ImpactEstimate,
}

impl PyImpactEstimate {
    pub(crate) fn from_inner(inner: ImpactEstimate) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyImpactEstimate {
    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.to_series")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = IMPACT_ESTIMATE_COLUMNS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        Ok(PyLabeledSeries::from_f64(
            "impact",
            labels,
            vec![
                self.inner.permanent_impact,
                self.inner.temporary_impact,
                self.inner.total_cost,
                self.inner.cost_bp,
                self.inner.execution_risk,
            ],
        )?
        .with_index_name("metric"))
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ImpactEstimate", &self.inner)
    }
}

py_getters!(PyImpactEstimate |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.permanent_impact")]
    permanent_impact -> f64 = s.inner.permanent_impact;

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.temporary_impact")]
    temporary_impact -> f64 = s.inner.temporary_impact;

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.total_cost")]
    total_cost -> f64 = s.inner.total_cost;

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.cost_bp")]
    cost_bp -> f64 = s.inner.cost_bp;

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.execution_risk")]
    execution_risk -> f64 = s.inner.execution_risk;
});

impl PyImpactEstimate {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("permanent_impact", vec![self.inner.permanent_impact])?
            .f64("temporary_impact", vec![self.inner.temporary_impact])?
            .f64("total_cost", vec![self.inner.total_cost])?
            .f64("cost_bp", vec![self.inner.cost_bp])?
            .f64("execution_risk", vec![self.inner.execution_risk])?
            .build()
    }
}

tabular_methods!(
    PyImpactEstimate,
    "ImpactEstimate",
    module = "finstack_quant.models.liquidity"
);
wire_methods!(
    PyImpactEstimate,
    ImpactEstimate,
    "ImpactEstimate",
    module = "finstack_quant.models.liquidity"
);

#[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory")]
#[pyclass(
    name = "ExecutionTrajectory",
    module = "finstack_quant.models.liquidity",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyExecutionTrajectory {
    pub(crate) inner: ExecutionTrajectory,
}

impl PyExecutionTrajectory {
    pub(crate) fn from_inner(inner: ExecutionTrajectory) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyExecutionTrajectory {
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExecutionTrajectory", &self.inner)
    }
}

py_getters!(PyExecutionTrajectory |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.quantities")]
    quantities -> Vec<f64> = s.inner.quantities.clone();

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.remaining")]
    remaining -> Vec<f64> = s.inner.remaining.clone();

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.time_points")]
    time_points -> Vec<f64> = s.inner.time_points.clone();

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.expected_cost")]
    expected_cost -> f64 = s.inner.expected_cost;

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.cost_variance")]
    cost_variance -> f64 = s.inner.cost_variance;
});

impl PyExecutionTrajectory {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut trade = Vec::with_capacity(self.inner.remaining.len());
        trade.push(0.0);
        trade.extend(self.inner.quantities.iter().copied());
        TableBuilder::new()
            .f64("t", self.inner.time_points.clone())?
            .f64("holdings", self.inner.remaining.clone())?
            .f64("trade", trade)?
            .build()
    }
}

tabular_methods!(
    PyExecutionTrajectory,
    "ExecutionTrajectory",
    module = "finstack_quant.models.liquidity"
);
wire_methods!(
    PyExecutionTrajectory,
    ExecutionTrajectory,
    "ExecutionTrajectory",
    module = "finstack_quant.models.liquidity"
);

const LVAR_COLUMNS: [&str; 4] = ["var", "spread_cost", "lvar", "lvar_ratio"];

#[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar")]
#[pyclass(
    name = "LvarBangiaScalar",
    module = "finstack_quant.models.liquidity",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLvarBangiaScalar {
    pub(crate) inner: LvarBangiaScalar,
}

impl PyLvarBangiaScalar {
    pub(crate) fn from_inner(inner: LvarBangiaScalar) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLvarBangiaScalar {
    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.to_series")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = LVAR_COLUMNS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        Ok(PyLabeledSeries::from_f64(
            "lvar",
            labels,
            vec![
                self.inner.var,
                self.inner.spread_cost,
                self.inner.lvar,
                self.inner.lvar_ratio,
            ],
        )?
        .with_index_name("metric"))
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LvarBangiaScalar", &self.inner)
    }
}

py_getters!(PyLvarBangiaScalar |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.var")]
    var -> f64 = s.inner.var;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.spread_cost")]
    spread_cost -> f64 = s.inner.spread_cost;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.lvar")]
    lvar -> f64 = s.inner.lvar;

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.lvar_ratio")]
    lvar_ratio -> f64 = s.inner.lvar_ratio;
});

impl PyLvarBangiaScalar {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("var", vec![self.inner.var])?
            .f64("spread_cost", vec![self.inner.spread_cost])?
            .f64("lvar", vec![self.inner.lvar])?
            .f64("lvar_ratio", vec![self.inner.lvar_ratio])?
            .build()
    }
}

tabular_methods!(
    PyLvarBangiaScalar,
    "LvarBangiaScalar",
    module = "finstack_quant.models.liquidity"
);
wire_methods!(
    PyLvarBangiaScalar,
    LvarBangiaScalar,
    "LvarBangiaScalar",
    module = "finstack_quant.models.liquidity"
);

#[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel")]
#[pyclass(
    name = "AlmgrenChrissModel",
    module = "finstack_quant.models.liquidity",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAlmgrenChrissModel {
    pub(crate) inner: AlmgrenChrissModel,
}

impl PyAlmgrenChrissModel {
    pub(crate) fn from_inner(inner: AlmgrenChrissModel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyAlmgrenChrissModel {
    #[new]
    #[pyo3(text_signature = "(gamma, eta, delta)")]
    fn new(gamma: f64, eta: f64, delta: f64) -> PyResult<Self> {
        AlmgrenChrissModel::new(gamma, eta, delta)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.from_profile")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, profile, daily_volatility)")]
    fn from_profile(
        _cls: &Bound<'_, PyType>,
        profile: PyLiquidityProfile,
        daily_volatility: f64,
    ) -> PyResult<Self> {
        AlmgrenChrissModel::from_profile(&profile.inner, daily_volatility)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.estimate_cost")]
    #[pyo3(text_signature = "(self, params)")]
    fn estimate_cost(&self, py: Python<'_>, params: PyTradeParams) -> PyResult<PyImpactEstimate> {
        py.detach(|| self.inner.estimate_cost(&params.inner))
            .map(PyImpactEstimate::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.optimal_trajectory")]
    #[pyo3(text_signature = "(self, params, num_buckets)")]
    fn optimal_trajectory(
        &self,
        py: Python<'_>,
        params: PyTradeParams,
        num_buckets: usize,
    ) -> PyResult<PyExecutionTrajectory> {
        py.detach(|| self.inner.optimal_trajectory(&params.inner, num_buckets))
            .map(PyExecutionTrajectory::from_inner)
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("AlmgrenChrissModel", &self.inner)
    }
}

py_getters!(PyAlmgrenChrissModel |s| {
    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.gamma")]
    gamma -> f64 = s.inner.gamma();

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.eta")]
    eta -> f64 = s.inner.eta();

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.delta")]
    delta -> f64 = s.inner.delta();
});

wire_methods!(
    PyAlmgrenChrissModel,
    AlmgrenChrissModel,
    "AlmgrenChrissModel",
    module = "finstack_quant.models.liquidity"
);

#[doc = pydoc!("finstack_quant.models.liquidity", "KyleLambdaModel")]
#[pyclass(
    name = "KyleLambdaModel",
    module = "finstack_quant.models.liquidity",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyKyleLambdaModel {
    pub(crate) inner: KyleLambdaModel,
}

impl PyKyleLambdaModel {
    pub(crate) fn from_inner(inner: KyleLambdaModel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyKyleLambdaModel {
    #[new]
    #[pyo3(text_signature = "(lambda_)")]
    fn new(lambda_: f64) -> PyResult<Self> {
        KyleLambdaModel::new(lambda_)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "KyleLambdaModel.from_amihud")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, amihud_ratio, reference_price)")]
    fn from_amihud(
        _cls: &Bound<'_, PyType>,
        amihud_ratio: f64,
        reference_price: f64,
    ) -> PyResult<Self> {
        KyleLambdaModel::from_amihud(amihud_ratio, reference_price)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "KyleLambdaModel.lambda_")]
    #[getter]
    fn lambda_(&self) -> f64 {
        self.inner.lambda()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "KyleLambdaModel.estimate_cost")]
    #[pyo3(text_signature = "(self, params)")]
    fn estimate_cost(&self, py: Python<'_>, params: PyTradeParams) -> PyResult<PyImpactEstimate> {
        py.detach(|| self.inner.estimate_cost(&params.inner))
            .map(PyImpactEstimate::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "KyleLambdaModel.optimal_trajectory")]
    #[pyo3(text_signature = "(self, params, num_buckets)")]
    fn optimal_trajectory(
        &self,
        py: Python<'_>,
        params: PyTradeParams,
        num_buckets: usize,
    ) -> PyResult<PyExecutionTrajectory> {
        py.detach(|| self.inner.optimal_trajectory(&params.inner, num_buckets))
            .map(PyExecutionTrajectory::from_inner)
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!("KyleLambdaModel(lambda_={:?})", self.inner.lambda())
    }
}

wire_methods!(
    PyKyleLambdaModel,
    KyleLambdaModel,
    "KyleLambdaModel",
    module = "finstack_quant.models.liquidity"
);

#[doc = pydoc!("finstack_quant.models.liquidity", "roll_effective_spread")]
#[pyfunction]
fn roll_effective_spread(py: Python<'_>, returns: Vec<f64>) -> Option<f64> {
    py.detach(move || liquidity::roll_effective_spread(&returns))
}

#[doc = pydoc!("finstack_quant.models.liquidity", "amihud_illiquidity")]
#[pyfunction]
fn amihud_illiquidity(py: Python<'_>, returns: Vec<f64>, volumes: Vec<f64>) -> Option<f64> {
    py.detach(move || liquidity::amihud_illiquidity(&returns, &volumes))
}

#[doc = pydoc!("finstack_quant.models.liquidity", "days_to_liquidate")]
#[pyfunction]
fn days_to_liquidate(position_quantity: f64, adv: f64, participation_rate: f64) -> f64 {
    liquidity::days_to_liquidate(position_quantity, adv, participation_rate)
}

#[doc = pydoc!("finstack_quant.models.liquidity", "liquidity_tier")]
#[pyfunction]
#[pyo3(signature = (days_to_liquidate, thresholds = None))]
#[pyo3(text_signature = "(days_to_liquidate, thresholds=None)")]
fn liquidity_tier(days_to_liquidate: f64, thresholds: Option<[f64; 4]>) -> PyResult<String> {
    liquidity::liquidity_tier(days_to_liquidate, thresholds)
        .and_then(|tier| finstack_quant_core::wire::serde_label(&tier))
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.liquidity", "lvar_bangia")]
#[pyfunction]
#[pyo3(signature = (var, spread_mean, spread_vol, confidence, position_value))]
fn lvar_bangia(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> PyResult<PyLvarBangiaScalar> {
    liquidity::lvar_bangia(var, spread_mean, spread_vol, confidence, position_value)
        .map(PyLvarBangiaScalar::from_inner)
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.liquidity", "almgren_chriss_impact")]
#[pyfunction]
#[pyo3(signature = (
    position_size,
    avg_daily_volume,
    volatility,
    execution_horizon_days,
    permanent_impact_coef,
    temporary_impact_coef,
    reference_price = None,
))]
fn almgren_chriss_impact(
    position_size: f64,
    avg_daily_volume: f64,
    volatility: f64,
    execution_horizon_days: f64,
    permanent_impact_coef: f64,
    temporary_impact_coef: f64,
    reference_price: Option<f64>,
) -> PyResult<PyImpactEstimate> {
    liquidity::almgren_chriss_impact(
        position_size,
        avg_daily_volume,
        volatility,
        execution_horizon_days,
        permanent_impact_coef,
        temporary_impact_coef,
        reference_price,
    )
    .map(PyImpactEstimate::from_inner)
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.liquidity", "kyle_lambda")]
#[pyfunction]
fn kyle_lambda(
    py: Python<'_>,
    returns: Vec<f64>,
    volumes: Vec<f64>,
    reference_price: f64,
) -> Option<f64> {
    py.detach(move || liquidity::kyle_lambda(&returns, &volumes, reference_price))
}

/// Register the `models.liquidity` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "liquidity")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.liquidity", "__module__"),
    )?;
    m.add_class::<PyAlmgrenChrissModel>()?;
    m.add_class::<PyExecutionTrajectory>()?;
    m.add_class::<PyImpactEstimate>()?;
    m.add_class::<PyKyleLambdaModel>()?;
    m.add_class::<PyLiquidityProfile>()?;
    m.add_class::<PyLvarBangiaScalar>()?;
    m.add_class::<PyTradeParams>()?;
    m.add_function(wrap_pyfunction!(roll_effective_spread, &m)?)?;
    m.add_function(wrap_pyfunction!(amihud_illiquidity, &m)?)?;
    m.add_function(wrap_pyfunction!(days_to_liquidate, &m)?)?;
    m.add_function(wrap_pyfunction!(liquidity_tier, &m)?)?;
    m.add_function(wrap_pyfunction!(lvar_bangia, &m)?)?;
    m.add_function(wrap_pyfunction!(almgren_chriss_impact, &m)?)?;
    m.add_function(wrap_pyfunction!(kyle_lambda, &m)?)?;
    let all = pyo3::types::PyList::new(
        py,
        [
            "AlmgrenChrissModel",
            "ExecutionTrajectory",
            "ImpactEstimate",
            "KyleLambdaModel",
            "LiquidityProfile",
            "LvarBangiaScalar",
            "TradeParams",
            "almgren_chriss_impact",
            "amihud_illiquidity",
            "days_to_liquidate",
            "kyle_lambda",
            "liquidity_tier",
            "lvar_bangia",
            "roll_effective_spread",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
