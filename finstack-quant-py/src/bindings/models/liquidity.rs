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

use crate::bindings::pandas_utils::{
    dict_to_dataframe, labeled_values_to_series, serde_object_to_single_row_dataframe_with_schema,
};
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_models::liquidity::{
    self, AlmgrenChrissModel, ExecutionTrajectory, ImpactEstimate, KyleLambdaModel,
    LiquidityProfile, LvarBangiaScalar, SpreadVolatilityKind, TradeParams,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

fn parse_spread_volatility_kind(kind: &str) -> PyResult<SpreadVolatilityKind> {
    serde_json::from_value(serde_json::Value::String(kind.to_string())).map_err(|_| {
        value_error(format!(
            "unknown spread_volatility_kind {kind:?}; expected \"relative\" or \"absolute\""
        ))
    })
}

fn spread_volatility_kind_str(kind: SpreadVolatilityKind) -> &'static str {
    match kind {
        SpreadVolatilityKind::Relative => "relative",
        SpreadVolatilityKind::Absolute => "absolute",
    }
}

/// Market microstructure snapshot for one instrument.
///
/// Prices are in the instrument's native currency, ``avg_daily_volume`` and
/// ``avg_trade_size`` in shares/contracts, and ``spread_volatility`` is the
/// standard deviation of the bid-ask spread interpreted according to
/// ``spread_volatility_kind`` (``"relative"`` = spread / mid, the Bangia
/// convention; ``"absolute"`` = ask - bid in price units).
///
/// Parameters
/// ----------
/// instrument_id : str
///     Identifier; must match the position's instrument id when used with
///     portfolio liquidity analytics.
/// mid : float
///     Positive mid price.
/// bid : float
///     Positive best bid; must not exceed ``ask``.
/// ask : float
///     Positive best ask.
/// avg_daily_volume : float
///     Non-negative average daily volume in shares/contracts.
/// avg_trade_size : float
///     Non-negative average trade size in shares/contracts.
/// spread_volatility : float
///     Non-negative spread standard deviation; ``0.0`` when unavailable.
/// spread_volatility_kind : str, default ``"relative"``
///     ``"relative"`` or ``"absolute"``.
/// observation_days : int, default ``20``
///     Trading-day window behind the volume and spread statistics.
///
/// Raises
/// ------
/// ValueError
///     If a price is non-positive, the market is crossed, a statistic is
///     negative/non-finite, or ``spread_volatility_kind`` is not recognised.
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
        spread_volatility_kind = "relative",
        observation_days = 20,
    ))]
    #[pyo3(
        text_signature = "(instrument_id, mid, bid, ask, avg_daily_volume, avg_trade_size, spread_volatility, spread_volatility_kind=\"relative\", observation_days=20)"
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
        spread_volatility_kind: &str,
        observation_days: u32,
    ) -> PyResult<Self> {
        let kind = parse_spread_volatility_kind(spread_volatility_kind)?;
        let mut inner = LiquidityProfile::new(
            instrument_id,
            mid,
            bid,
            ask,
            avg_daily_volume,
            avg_trade_size,
            spread_volatility,
        )
        .map_err(core_to_py)?
        .with_spread_volatility_kind(kind);
        inner.observation_days = observation_days;
        Ok(Self { inner })
    }

    /// Instrument identifier.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    /// Mid price.
    #[getter]
    fn mid(&self) -> f64 {
        self.inner.mid
    }

    /// Best bid price.
    #[getter]
    fn bid(&self) -> f64 {
        self.inner.bid
    }

    /// Best ask price.
    #[getter]
    fn ask(&self) -> f64 {
        self.inner.ask
    }

    /// Average daily volume in shares/contracts.
    #[getter]
    fn avg_daily_volume(&self) -> f64 {
        self.inner.avg_daily_volume
    }

    /// Average trade size in shares/contracts.
    #[getter]
    fn avg_trade_size(&self) -> f64 {
        self.inner.avg_trade_size
    }

    /// Spread standard deviation in the units named by ``spread_volatility_kind``.
    #[getter]
    fn spread_volatility(&self) -> f64 {
        self.inner.spread_volatility
    }

    /// ``"relative"`` or ``"absolute"``.
    #[getter]
    fn spread_volatility_kind(&self) -> &'static str {
        spread_volatility_kind_str(self.inner.spread_volatility_kind)
    }

    /// Trading-day observation window behind the statistics.
    #[getter]
    fn observation_days(&self) -> u32 {
        self.inner.observation_days
    }

    /// Absolute bid-ask spread ``ask - bid`` in price units.
    #[getter]
    fn spread(&self) -> f64 {
        self.inner.spread()
    }

    /// Relative spread ``(ask - bid) / mid``.
    #[getter]
    fn relative_spread(&self) -> f64 {
        self.inner.relative_spread()
    }

    /// Half the absolute spread in price units.
    #[getter]
    fn half_spread(&self) -> f64 {
        self.inner.half_spread()
    }

    /// Spread volatility normalised to relative (fraction-of-mid) units.
    #[getter]
    fn relative_spread_volatility(&self) -> f64 {
        self.inner.relative_spread_volatility()
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "LiquidityProfile serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid LiquidityProfile JSON"))
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LiquidityProfile", &self.inner)
    }
}

/// Inputs to a market-impact calculation.
///
/// Parameters
/// ----------
/// quantity : float
///     Signed quantity to execute (positive = buy, negative = sell) in
///     shares/contracts.
/// horizon_days : float
///     Positive execution horizon in trading days.
/// daily_volatility : float
///     Positive daily return volatility as a decimal (``0.02`` for 2%).
/// profile : LiquidityProfile
///     Market microstructure snapshot of the instrument.
/// risk_aversion : float | None, default ``None``
///     Trajectory risk-aversion; ``None`` uses the model default (``1e-6``).
/// reference_price : float | None, default ``None``
///     Arrival/decision price converting return-space volatility into
///     currency; ``None`` falls back to ``profile.mid``.
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

impl PyTradeParams {
    pub(crate) fn from_inner(inner: TradeParams) -> Self {
        Self { inner }
    }
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

    /// Signed quantity to execute.
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    /// Execution horizon in trading days.
    #[getter]
    fn horizon_days(&self) -> f64 {
        self.inner.horizon_days
    }

    /// Daily return volatility as a decimal.
    #[getter]
    fn daily_volatility(&self) -> f64 {
        self.inner.daily_volatility
    }

    /// Liquidity profile of the instrument.
    #[getter]
    fn profile(&self) -> PyLiquidityProfile {
        PyLiquidityProfile::from_inner(self.inner.profile.clone())
    }

    /// Trajectory risk-aversion override, or ``None`` for the model default.
    #[getter]
    fn risk_aversion(&self) -> Option<f64> {
        self.inner.risk_aversion
    }

    /// Explicit reference price, or ``None`` when ``profile.mid`` applies.
    #[getter]
    fn reference_price(&self) -> Option<f64> {
        self.inner.reference_price
    }

    /// Reference price actually used: ``reference_price`` or ``profile.mid``.
    #[getter]
    fn effective_reference_price(&self) -> f64 {
        self.inner.effective_reference_price()
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "TradeParams serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid TradeParams JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TradeParams", &self.inner)
    }
}

const IMPACT_ESTIMATE_COLUMNS: [&str; 5] = [
    "permanent_impact",
    "temporary_impact",
    "total_cost",
    "cost_bp",
    "execution_risk",
];

/// Expected market-impact execution costs of one trade.
///
/// All ``*_impact`` and ``total_cost`` values are costs in currency units
/// (impact integrated over the executed quantity), not per-share price
/// displacements; ``cost_bp`` is the total cost in basis points of notional
/// and ``execution_risk`` the standard deviation of the cost.
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
    /// Permanent-impact cost component, in currency units.
    #[getter]
    fn permanent_impact(&self) -> f64 {
        self.inner.permanent_impact
    }

    /// Temporary-impact cost component, in currency units.
    #[getter]
    fn temporary_impact(&self) -> f64 {
        self.inner.temporary_impact
    }

    /// Total expected execution cost, in currency units.
    #[getter]
    fn total_cost(&self) -> f64 {
        self.inner.total_cost
    }

    /// Total cost in basis points of notional.
    #[getter]
    fn cost_bp(&self) -> f64 {
        self.inner.cost_bp
    }

    /// Standard deviation of the execution cost, in currency units.
    #[getter]
    fn execution_risk(&self) -> f64 {
        self.inner.execution_risk
    }

    /// The five cost fields as a float ``pandas.Series`` named ``impact``.
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

    /// Single-row ``pandas.DataFrame`` with the five cost columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &IMPACT_ESTIMATE_COLUMNS)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "ImpactEstimate serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid ImpactEstimate JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ImpactEstimate", &self.inner)
    }
}

/// Optimal execution schedule for a trade.
///
/// ``time_points`` holds the ``num_buckets + 1`` bucket boundaries in trading
/// days (starting at ``0.0``), ``remaining`` the inventory at each boundary
/// and ``quantities`` the ``num_buckets`` per-bucket trades.
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
    /// Quantity traded in each bucket (length ``num_buckets``).
    #[getter]
    fn quantities(&self) -> Vec<f64> {
        self.inner.quantities.clone()
    }

    /// Remaining inventory at each bucket boundary (length ``num_buckets + 1``).
    #[getter]
    fn remaining(&self) -> Vec<f64> {
        self.inner.remaining.clone()
    }

    /// Bucket boundaries in trading days (length ``num_buckets + 1``).
    #[getter]
    fn time_points(&self) -> Vec<f64> {
        self.inner.time_points.clone()
    }

    /// Expected cost of the schedule, in currency units.
    #[getter]
    fn expected_cost(&self) -> f64 {
        self.inner.expected_cost
    }

    /// Variance of the cost under the schedule, in currency units squared.
    #[getter]
    fn cost_variance(&self) -> f64 {
        self.inner.cost_variance
    }

    /// Schedule as a ``pandas.DataFrame`` with columns ``t``, ``holdings``,
    /// ``trade``.
    ///
    /// One row per bucket boundary: ``t`` is the boundary time in trading
    /// days, ``holdings`` the inventory at that boundary and ``trade`` the
    /// quantity executed in the bucket ending at ``t`` (``0.0`` on the first
    /// row).
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

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "ExecutionTrajectory serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid ExecutionTrajectory JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExecutionTrajectory", &self.inner)
    }
}

const LVAR_COLUMNS: [&str; 4] = ["var", "spread_cost", "lvar", "lvar_ratio"];

/// Bangia liquidity-adjusted VaR for one position (loss sign convention).
///
/// ``var`` and ``lvar`` are non-positive loss numbers with ``lvar <= var``,
/// ``spread_cost`` is the non-negative liquidity add-on and ``lvar_ratio``
/// is ``lvar / var`` (``NaN`` when ``var == 0``).
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
    /// Input market VaR (non-positive), echoed back.
    #[getter]
    fn var(&self) -> f64 {
        self.inner.var
    }

    /// Non-negative spread-cost add-on.
    #[getter]
    fn spread_cost(&self) -> f64 {
        self.inner.spread_cost
    }

    /// Liquidity-adjusted VaR (non-positive, ``lvar <= var``).
    #[getter]
    fn lvar(&self) -> f64 {
        self.inner.lvar
    }

    /// ``lvar / var``; ``NaN`` when ``var`` is zero.
    #[getter]
    fn lvar_ratio(&self) -> f64 {
        self.inner.lvar_ratio
    }

    /// The four fields as a float ``pandas.Series`` named ``lvar``.
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

    /// Single-row ``pandas.DataFrame`` with the four columns.
    ///
    /// Read the VaR column as ``df["var"]``: attribute access resolves to
    /// ``DataFrame.var`` (the variance method).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &LVAR_COLUMNS)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "LvarBangiaScalar serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid LvarBangiaScalar JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LvarBangiaScalar", &self.inner)
    }
}

/// Almgren-Chriss (2000) market-impact model.
///
/// Permanent impact is linear (``g(v) = gamma * v``); temporary impact
/// follows the power law ``h(v) = eta * sign(v) * |v|^delta``.
///
/// Parameters
/// ----------
/// gamma : float
///     Non-negative permanent impact coefficient in price units per share.
/// eta : float
///     Positive temporary impact coefficient in price units per share.
/// delta : float
///     Power-law exponent in ``(0, 1]``; ``0.5``-``0.6`` is typical for
///     equities and ``1.0`` selects the linear model required by
///     ``optimal_trajectory``.
///
/// Raises
/// ------
/// ValueError
///     If a coefficient is outside its documented range or non-finite.
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

    /// Calibrate coefficients from a liquidity profile.
    ///
    /// ``gamma = spread / (2 * ADV)``, ``eta = daily_volatility * mid / sqrt(ADV)``
    /// and ``delta = 0.5``.
    ///
    /// Raises ``ValueError`` if ``daily_volatility`` is non-positive or the
    /// profile has zero average daily volume.
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

    /// Permanent impact coefficient.
    #[getter]
    fn gamma(&self) -> f64 {
        self.inner.gamma()
    }

    /// Temporary impact coefficient.
    #[getter]
    fn eta(&self) -> f64 {
        self.inner.eta()
    }

    /// Temporary-impact power-law exponent.
    #[getter]
    fn delta(&self) -> f64 {
        self.inner.delta()
    }

    /// Model name for diagnostics.
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    /// Expected execution cost of ``params`` under uniform execution.
    ///
    /// Raises ``ValueError`` for non-finite or non-positive trade inputs.
    #[pyo3(text_signature = "(self, params)")]
    fn estimate_cost(&self, py: Python<'_>, params: PyTradeParams) -> PyResult<PyImpactEstimate> {
        py.detach(|| self.inner.estimate_cost(&params.inner))
            .map(PyImpactEstimate::from_inner)
            .map_err(core_to_py)
    }

    /// Cost-plus-risk optimal schedule over ``num_buckets`` intervals.
    ///
    /// Only defined for ``delta == 1.0`` (linear temporary impact).
    ///
    /// Raises ``ValueError`` when ``num_buckets`` is zero, the trade inputs
    /// are invalid, or the model's ``delta`` is not ``1.0``.
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

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "AlmgrenChrissModel serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed or the impact
    /// parameters violate the constructor's finiteness and range requirements.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid AlmgrenChrissModel JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("AlmgrenChrissModel", &self.inner)
    }
}

/// Kyle (1985) linear price-impact model ``dP = lambda * signed_volume``.
///
/// Parameters
/// ----------
/// lambda_ : float
///     Non-negative price impact per unit of order flow, in price units per
///     share/contract. Named ``lambda_`` because ``lambda`` is a Python
///     keyword.
///
/// Raises
/// ------
/// ValueError
///     If ``lambda_`` is negative or non-finite.
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

    /// Build from an Amihud ratio: ``lambda = amihud_ratio * reference_price``.
    ///
    /// Raises ``ValueError`` if the ratio is negative/non-finite or the
    /// reference price is non-positive.
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

    /// Price impact per unit of order flow.
    #[getter]
    fn lambda_(&self) -> f64 {
        self.inner.lambda()
    }

    /// Model name for diagnostics.
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    /// Expected execution cost ``0.5 * lambda * quantity^2`` plus timing risk.
    ///
    /// Raises ``ValueError`` for non-finite or non-positive trade inputs.
    #[pyo3(text_signature = "(self, params)")]
    fn estimate_cost(&self, py: Python<'_>, params: PyTradeParams) -> PyResult<PyImpactEstimate> {
        py.detach(|| self.inner.estimate_cost(&params.inner))
            .map(PyImpactEstimate::from_inner)
            .map_err(core_to_py)
    }

    /// Uniform execution schedule over ``num_buckets`` intervals.
    ///
    /// Raises ``ValueError`` when ``num_buckets`` is zero or trade inputs are
    /// invalid.
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

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "KyleLambdaModel serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed or ``lambda`` is
    /// negative or non-finite.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid KyleLambdaModel JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!("KyleLambdaModel(lambda_={:?})", self.inner.lambda())
    }
}

/// Estimate the effective bid-ask spread via Roll's (1984) serial covariance
/// estimator.
///
/// Under Roll's model, observed returns are the sum of an efficient-price
/// innovation and a bid-ask bounce component, giving
/// ``effective_spread = 2 * sqrt(-Cov(r_t, r_{t-1}))``.
///
/// Parameters
/// ----------
/// returns : list[float]
///     Log or arithmetic returns, length >= 2.
///
/// Returns
/// -------
/// float | None
///     Effective spread in the same units as the returns, or ``None`` when
///     the serial covariance is non-negative (violates the Roll assumption)
///     or when ``len(returns) < 2``. ``None`` (rather than ``NaN``) forces
///     callers to handle the unestimable case explicitly instead of letting
///     it propagate silently through downstream arithmetic.
///
/// Sources
/// -------
/// - Roll (1984): see docs/REFERENCES.md#roll-1984
#[pyfunction]
fn roll_effective_spread(py: Python<'_>, returns: Vec<f64>) -> Option<f64> {
    py.detach(move || liquidity::roll_effective_spread(&returns))
}

/// Compute the Amihud (2002) illiquidity ratio from returns and volumes.
///
/// ``ILLIQ = mean(|r_t| / Volume_t)``. Higher values indicate less liquid
/// instruments (more price impact per unit of volume).
///
/// Parameters
/// ----------
/// returns : list[float]
///     Period returns (absolute value taken internally).
/// volumes : list[float]
///     Period trading volumes, same length as ``returns``. All entries must
///     be strictly positive.
///
/// Returns
/// -------
/// float | None
///     Average daily illiquidity ratio, or ``None`` if inputs are empty,
///     mismatched in length, non-finite, or contain a zero/negative volume.
///
/// Sources
/// -------
/// - Amihud (2002): see docs/REFERENCES.md#amihud-2002
#[pyfunction]
fn amihud_illiquidity(py: Python<'_>, returns: Vec<f64>, volumes: Vec<f64>) -> Option<f64> {
    py.detach(move || liquidity::amihud_illiquidity(&returns, &volumes))
}

/// Trading days required to liquidate a position at the given participation
/// rate.
///
/// ``days = position_quantity / (adv * participation_rate)``. Both
/// ``position_quantity`` and ``adv`` are in **share/contract space** — the
/// same units the Rust ``days_to_liquidate`` contract defines. Passing a
/// currency notional against a share-count ADV (or vice versa) silently
/// mis-scales the result by the share price.
///
/// Parameters
/// ----------
/// position_quantity : float
///     Number of shares/contracts to liquidate (absolute value used).
/// adv : float
///     Average daily traded volume in shares/contracts.
/// participation_rate : float
///     Fraction of ADV that can be traded per day, typically 0.05 to 0.25.
///
/// Returns
/// -------
/// float
///     Trading days to fully liquidate. ``inf`` if ADV or participation rate
///     is non-positive.
///
/// Notes
/// -----
/// This helper does not raise; non-positive ADV or participation rate returns
/// ``inf`` rather than an exception.
#[pyfunction]
fn days_to_liquidate(position_quantity: f64, adv: f64, participation_rate: f64) -> f64 {
    liquidity::days_to_liquidate(position_quantity, adv, participation_rate)
}

/// Classify a position into a liquidity tier from its days-to-liquidate.
///
/// Parameters
/// ----------
/// days_to_liquidate : float
///     Estimated trading days required to fully unwind the position.
/// thresholds : tuple[float, float, float, float] | None, default ``None``
///     Ascending tier boundaries ``(tier1_max, tier2_max, tier3_max,
///     tier4_max)`` in trading days. ``None`` uses the Rust
///     ``LiquidityConfig`` default ``(1.0, 5.0, 20.0, 60.0)``.
///
/// Returns
/// -------
/// str
///     One of ``"tier1"``, ``"tier2"``, ``"tier3"``, ``"tier4"``, ``"tier5"``
///     with Tier 1 most liquid and Tier 5 least liquid.
///
/// Raises
/// ------
/// ValueError
///     If ``thresholds`` is given but is not strictly ascending or contains a
///     non-finite value.
#[pyfunction]
#[pyo3(signature = (days_to_liquidate, thresholds = None))]
#[pyo3(text_signature = "(days_to_liquidate, thresholds=None)")]
fn liquidity_tier(days_to_liquidate: f64, thresholds: Option<[f64; 4]>) -> PyResult<&'static str> {
    let thresholds = match thresholds {
        Some(values) => {
            if values.iter().any(|value| !value.is_finite())
                || values.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(value_error(format!(
                    "thresholds must be four finite, strictly ascending day counts, got {values:?}"
                )));
            }
            values
        }
        None => liquidity::LiquidityConfig::default().tier_thresholds,
    };
    Ok(liquidity::classify_tier(days_to_liquidate, &thresholds).as_binding_str())
}

/// Liquidity-adjusted VaR following Bangia, Diebold, Schuermann & Stroughair (1999).
///
/// Uses the loss sign convention: VaR and LVaR are non-positive numbers.
///
/// ``LVaR = VaR - (0.5 * spread_mean + z_alpha * 0.5 * spread_vol) * position_value``
///
/// The ``spread_cost`` add-on is returned as a non-negative magnitude.
///
/// Parameters
/// ----------
/// var : float
///     Standard VaR for the position following the loss sign convention
///     (non-positive number; ``-10_000.0`` means a $10,000 loss). ``0.0`` is
///     accepted for a zero-risk position.
/// spread_mean : float
///     Mean relative bid-ask spread over the lookback window, e.g. ``0.001``
///     for 10bp.
/// spread_vol : float
///     Relative spread volatility (standard deviation of relative spread).
/// confidence : float
///     Confidence level strictly inside ``(0.5, 1)``, e.g. ``0.99``.
/// position_value : float
///     Market value of the position (sign ignored; only magnitude is used).
///
/// Returns
/// -------
/// LvarBangiaScalar
///     Typed result with ``var``, ``spread_cost``, ``lvar`` and
///     ``lvar_ratio`` attributes plus ``to_series()`` / ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If ``var`` is positive or non-finite, a spread statistic is negative,
///     ``confidence`` is outside ``(0.5, 1)`` or ``position_value`` is
///     non-finite.
///
/// Sources
/// -------
/// - Bangia, Diebold, Schuermann, and Stroughair (1999): see
///   docs/REFERENCES.md#bangia-1999-lvar
#[pyfunction]
#[pyo3(signature = (var, spread_mean, spread_vol, confidence, position_value))]
fn lvar_bangia(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> PyResult<PyLvarBangiaScalar> {
    liquidity::lvar_bangia_scalar(var, spread_mean, spread_vol, confidence, position_value)
        .map(PyLvarBangiaScalar::from_inner)
        .map_err(core_to_py)
}

/// Almgren-Chriss (2000) impact-cost decomposition for a uniform execution
/// over a fixed horizon.
///
/// The impact coefficients are derived from ``avg_daily_volume`` with the
/// same calibration as ``AlmgrenChrissModel.from_profile`` (20 bp
/// proportional spread), and ``permanent_impact_coef`` /
/// ``temporary_impact_coef`` scale that base multiplicatively. Callers with
/// externally calibrated absolute ``gamma`` / ``eta`` should build
/// ``AlmgrenChrissModel`` directly.
///
/// Parameters
/// ----------
/// position_size : float
///     Total quantity to execute in shares/contracts (sign is preserved but
///     cost is symmetric in size).
/// avg_daily_volume : float
///     Average daily volume in shares/contracts (must be positive).
/// volatility : float
///     Daily return volatility (e.g., ``0.02`` for 2%).
/// execution_horizon_days : float
///     Execution horizon in trading days (must be positive).
/// permanent_impact_coef : float
///     Dimensionless multiplier on the ADV-derived permanent impact. Non-negative.
/// temporary_impact_coef : float
///     Dimensionless multiplier on the ADV-derived temporary impact. Strictly positive.
/// reference_price : float | None, default ``None``
///     Optional arrival/decision price used for notional and cost-bp scaling.
///     When omitted, the helper keeps the unit-price convention.
///
/// Returns
/// -------
/// ImpactEstimate
///     Typed result with ``permanent_impact``, ``temporary_impact``,
///     ``total_cost``, ``cost_bp`` and ``execution_risk`` attributes plus
///     ``to_series()`` / ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If an input violates its finiteness, sign or range contract.
///
/// Sources
/// -------
/// - Almgren and Chriss (2000): see docs/REFERENCES.md#almgren-chriss-2000
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
    liquidity::almgren_chriss_uniform_impact(
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

/// Estimate Kyle's (1985) linear price impact coefficient lambda from
/// observed returns and volumes.
///
/// Uses the Amihud-ratio proxy:
/// ``lambda = mean(|r_t| / V_t) * reference_price``.
/// Under the Kyle model, price impact per trade is ``lambda * signed_volume``.
/// Argument order matches ``amihud_illiquidity``: returns first, then volumes.
///
/// Parameters
/// ----------
/// returns : list[float]
///     Period returns, same length as ``volumes``.
/// volumes : list[float]
///     Period trading volumes, strictly positive.
/// reference_price : float
///     Positive price per share or contract used to convert the return-space
///     ratio into price-space lambda.
///
/// Returns
/// -------
/// float | None
///     Estimated price-space Kyle lambda, or ``None`` if inputs are invalid
///     (empty, mismatched length, non-finite, contain zero volumes, or have a
///     non-positive reference price).
///
/// Sources
/// -------
/// - Kyle (1985): see docs/REFERENCES.md#kyle-1985
#[pyfunction]
fn kyle_lambda(
    py: Python<'_>,
    returns: Vec<f64>,
    volumes: Vec<f64>,
    reference_price: f64,
) -> Option<f64> {
    py.detach(move || KyleLambdaModel::lambda_from_series(&returns, &volumes, reference_price))
}

/// Register the `models.liquidity` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "liquidity")?;
    m.setattr(
        "__doc__",
        "Product-independent liquidity estimation, risk, and market-impact models.",
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
