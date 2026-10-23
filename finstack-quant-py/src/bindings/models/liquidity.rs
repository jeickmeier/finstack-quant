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

use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::{
    dict_to_dataframe, labeled_values_to_series, serde_object_to_single_row_dataframe_with_schema,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::liquidity::{
    self, AlmgrenChrissModel, ExecutionTrajectory, ImpactEstimate, KyleLambdaModel,
    LiquidityProfile, LvarBangiaScalar, SpreadVolatilityKind, TradeParams,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

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
    #[pyo3(signature = (
        instrument_id,
        mid,
        bid,
        ask,
        avg_daily_volume,
        avg_trade_size,
        spread_volatility,
        spread_volatility_kind = None,
        observation_days = None,
    ))]
    #[pyo3(
        text_signature = "(instrument_id, mid, bid, ask, avg_daily_volume, avg_trade_size, spread_volatility, spread_volatility_kind=None, observation_days=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        instrument_id: String,
        mid: f64,
        bid: f64,
        ask: f64,
        avg_daily_volume: f64,
        avg_trade_size: f64,
        spread_volatility: f64,
        spread_volatility_kind: Option<&str>,
        observation_days: Option<u32>,
    ) -> PyResult<Self> {
        let mut inner = LiquidityProfile::new(
            instrument_id,
            mid,
            bid,
            ask,
            avg_daily_volume,
            avg_trade_size,
            spread_volatility,
        )
        .map_err(core_to_py)?;
        if let Some(kind) = spread_volatility_kind {
            inner = inner.with_spread_volatility_kind(
                finstack_quant_core::wire::serde_parse::<SpreadVolatilityKind>(kind)
                    .map_err(core_to_py)?,
            );
        }
        if let Some(days) = observation_days {
            inner = inner.with_observation_days(days).map_err(core_to_py)?;
        }
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.instrument_id")]
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.mid")]
    #[getter]
    fn mid(&self) -> f64 {
        self.inner.mid
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.bid")]
    #[getter]
    fn bid(&self) -> f64 {
        self.inner.bid
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.ask")]
    #[getter]
    fn ask(&self) -> f64 {
        self.inner.ask
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.avg_daily_volume")]
    #[getter]
    fn avg_daily_volume(&self) -> f64 {
        self.inner.avg_daily_volume
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.avg_trade_size")]
    #[getter]
    fn avg_trade_size(&self) -> f64 {
        self.inner.avg_trade_size
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread_volatility")]
    #[getter]
    fn spread_volatility(&self) -> f64 {
        self.inner.spread_volatility
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread_volatility_kind")]
    #[getter]
    fn spread_volatility_kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.spread_volatility_kind)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.observation_days")]
    #[getter]
    fn observation_days(&self) -> u32 {
        self.inner.observation_days
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.spread")]
    #[getter]
    fn spread(&self) -> f64 {
        self.inner.spread()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.relative_spread")]
    #[getter]
    fn relative_spread(&self) -> f64 {
        self.inner.relative_spread()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.half_spread")]
    #[getter]
    fn half_spread(&self) -> f64 {
        self.inner.half_spread()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LiquidityProfile.relative_spread_volatility")]
    #[getter]
    fn relative_spread_volatility(&self) -> f64 {
        self.inner.relative_spread_volatility()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LiquidityProfile", &self.inner)
    }
}

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

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.horizon_days")]
    #[getter]
    fn horizon_days(&self) -> f64 {
        self.inner.horizon_days
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.daily_volatility")]
    #[getter]
    fn daily_volatility(&self) -> f64 {
        self.inner.daily_volatility
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.profile")]
    #[getter]
    fn profile(&self) -> PyLiquidityProfile {
        PyLiquidityProfile::from_inner(self.inner.profile.clone())
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.risk_aversion")]
    #[getter]
    fn risk_aversion(&self) -> Option<f64> {
        self.inner.risk_aversion
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.reference_price")]
    #[getter]
    fn reference_price(&self) -> Option<f64> {
        self.inner.reference_price
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "TradeParams.effective_reference_price")]
    #[getter]
    fn effective_reference_price(&self) -> f64 {
        self.inner.effective_reference_price()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TradeParams", &self.inner)
    }
}

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
    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.permanent_impact")]
    #[getter]
    fn permanent_impact(&self) -> f64 {
        self.inner.permanent_impact
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.temporary_impact")]
    #[getter]
    fn temporary_impact(&self) -> f64 {
        self.inner.temporary_impact
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.total_cost")]
    #[getter]
    fn total_cost(&self) -> f64 {
        self.inner.total_cost
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.cost_bp")]
    #[getter]
    fn cost_bp(&self) -> f64 {
        self.inner.cost_bp
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.execution_risk")]
    #[getter]
    fn execution_risk(&self) -> f64 {
        self.inner.execution_risk
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = IMPACT_ESTIMATE_COLUMNS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        labeled_values_to_series(
            py,
            &labels,
            vec![
                self.inner.permanent_impact,
                self.inner.temporary_impact,
                self.inner.total_cost,
                self.inner.cost_bp,
                self.inner.execution_risk,
            ],
            "impact",
        )
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ImpactEstimate.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &IMPACT_ESTIMATE_COLUMNS)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ImpactEstimate", &self.inner)
    }
}

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
    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.quantities")]
    #[getter]
    fn quantities(&self) -> Vec<f64> {
        self.inner.quantities.clone()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.remaining")]
    #[getter]
    fn remaining(&self) -> Vec<f64> {
        self.inner.remaining.clone()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.time_points")]
    #[getter]
    fn time_points(&self) -> Vec<f64> {
        self.inner.time_points.clone()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.expected_cost")]
    #[getter]
    fn expected_cost(&self) -> f64 {
        self.inner.expected_cost
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.cost_variance")]
    #[getter]
    fn cost_variance(&self) -> f64 {
        self.inner.cost_variance
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "ExecutionTrajectory.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut trade = Vec::with_capacity(self.inner.remaining.len());
        trade.push(0.0);
        trade.extend(self.inner.quantities.iter().copied());
        let data = PyDict::new(py);
        data.set_item("t", self.inner.time_points.clone())?;
        data.set_item("holdings", self.inner.remaining.clone())?;
        data.set_item("trade", trade)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExecutionTrajectory", &self.inner)
    }
}

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
    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.var")]
    #[getter]
    fn var(&self) -> f64 {
        self.inner.var
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.spread_cost")]
    #[getter]
    fn spread_cost(&self) -> f64 {
        self.inner.spread_cost
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.lvar")]
    #[getter]
    fn lvar(&self) -> f64 {
        self.inner.lvar
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.lvar_ratio")]
    #[getter]
    fn lvar_ratio(&self) -> f64 {
        self.inner.lvar_ratio
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = LVAR_COLUMNS
            .iter()
            .map(|label| (*label).to_string())
            .collect();
        labeled_values_to_series(
            py,
            &labels,
            vec![
                self.inner.var,
                self.inner.spread_cost,
                self.inner.lvar,
                self.inner.lvar_ratio,
            ],
            "lvar",
        )
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "LvarBangiaScalar.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &LVAR_COLUMNS)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LvarBangiaScalar", &self.inner)
    }
}

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

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.gamma")]
    #[getter]
    fn gamma(&self) -> f64 {
        self.inner.gamma()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.eta")]
    #[getter]
    fn eta(&self) -> f64 {
        self.inner.eta()
    }

    #[doc = pydoc!("finstack_quant.models.liquidity", "AlmgrenChrissModel.delta")]
    #[getter]
    fn delta(&self) -> f64 {
        self.inner.delta()
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
#[allow(clippy::too_many_arguments)]
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
