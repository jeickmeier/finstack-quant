//! Python bindings for structural credit model specifications.

mod lgd;
mod liability_management;
mod migration;
mod pd;
mod recovery_waterfall;
mod scoring;

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use std::sync::Arc;

use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::core::market_data::curves::PyHazardCurve;
use crate::bindings::core::types::extract_credit_rating;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe, serde_to_py, values_to_series,
};
use crate::errors::core_to_py;
use finstack_quant_core::wire::serde_parse;
use finstack_quant_models::credit::{
    moodys_warf_factor as rust_moodys_warf_factor, AssetDynamics, CreditState, CreditStateVariable,
    DynamicRecoverySpec, EndogenousHazardSpec, MertonBarrierType, MertonModel, RatingFactorTable,
    SimulatedPaths, ThresholdDirection, ToggleExerciseModel,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

#[doc = pydoc!("finstack_quant.models.credit", "moodys_warf_factor")]
#[pyfunction]
#[pyo3(text_signature = "(rating)")]
fn moodys_warf_factor(rating: &Bound<'_, PyAny>) -> PyResult<f64> {
    let rating = extract_credit_rating(rating)?;
    rust_moodys_warf_factor(rating).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable")]
#[pyclass(
    name = "RatingFactorTable",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRatingFactorTable {
    pub(crate) inner: RatingFactorTable,
}

#[pymethods]
impl PyRatingFactorTable {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.default_calibration")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn default_calibration() -> PyResult<Self> {
        RatingFactorTable::default_calibration()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.from_registry_id")]
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn from_registry_id(id: &str) -> PyResult<Self> {
        RatingFactorTable::from_registry_id(id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.get_factor")]
    #[pyo3(text_signature = "($self, rating)")]
    fn get_factor(&self, rating: &Bound<'_, PyAny>) -> PyResult<f64> {
        let rating = extract_credit_rating(rating)?;
        self.inner.get_factor(rating).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.agency")]
    #[getter]
    fn agency(&self) -> String {
        self.inner.agency().to_string()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.methodology")]
    #[getter]
    fn methodology(&self) -> String {
        self.inner.methodology().to_string()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.default_factor")]
    #[getter]
    fn default_factor(&self) -> f64 {
        self.inner.default_factor()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "RatingFactorTable.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RatingFactorTable", &self.inner)
    }
}

wire_methods!(
    PyRatingFactorTable,
    RatingFactorTable,
    "RatingFactorTable",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "MertonBarrierType")]
#[pyclass(
    name = "MertonBarrierType",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyBarrierType {
    pub(crate) inner: MertonBarrierType,
}

impl PyBarrierType {
    pub(crate) fn from_inner(inner: MertonBarrierType) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBarrierType {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonBarrierType.terminal")]
    #[staticmethod]
    fn terminal() -> Self {
        Self {
            inner: MertonBarrierType::Terminal,
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonBarrierType.first_passage")]
    #[staticmethod]
    fn first_passage(barrier_growth_rate: f64) -> Self {
        Self {
            inner: MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            },
        }
    }

    fn __repr__(&self) -> String {
        variant_repr("MertonBarrierType", &self.inner)
    }
}

wire_methods!(
    PyBarrierType,
    MertonBarrierType,
    "MertonBarrierType",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "AssetDynamics")]
#[pyclass(
    name = "AssetDynamics",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyAssetDynamics {
    pub(crate) inner: AssetDynamics,
}

impl PyAssetDynamics {
    pub(crate) fn from_inner(inner: AssetDynamics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyAssetDynamics {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "AssetDynamics.geometric_brownian")]
    #[staticmethod]
    fn geometric_brownian() -> Self {
        Self {
            inner: AssetDynamics::GeometricBrownian,
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "AssetDynamics.jump_diffusion")]
    #[staticmethod]
    fn jump_diffusion(jump_intensity: f64, jump_mean: f64, jump_vol: f64) -> Self {
        Self {
            inner: AssetDynamics::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "AssetDynamics.credit_grades")]
    #[staticmethod]
    fn credit_grades(barrier_uncertainty: f64, mean_recovery: f64) -> Self {
        Self {
            inner: AssetDynamics::CreditGrades {
                barrier_uncertainty,
                mean_recovery,
            },
        }
    }

    fn __repr__(&self) -> String {
        variant_repr("AssetDynamics", &self.inner)
    }
}

wire_methods!(
    PyAssetDynamics,
    AssetDynamics,
    "AssetDynamics",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel")]
#[pyclass(
    name = "MertonModel",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyMertonModel {
    pub(crate) inner: MertonModel,
}

#[pymethods]
impl PyMertonModel {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.__init__")]
    #[new]
    #[pyo3(text_signature = "(asset_value, asset_vol, debt_barrier, risk_free_rate)")]
    fn new(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::new(asset_value, asset_vol, debt_barrier, risk_free_rate)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.from_equity")]
    #[staticmethod]
    fn from_equity(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        maturity: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_equity(
                equity_value,
                equity_vol,
                total_debt,
                risk_free_rate,
                payout_rate,
                maturity,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.from_cds_spread")]
    #[staticmethod]
    fn from_cds_spread(
        cds_spread_bp: f64,
        recovery: f64,
        total_debt: f64,
        risk_free_rate: f64,
        maturity: f64,
        asset_value: f64,
        payout_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_cds_spread(
                cds_spread_bp,
                recovery,
                total_debt,
                risk_free_rate,
                maturity,
                asset_value,
                payout_rate,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.from_target_pd")]
    #[staticmethod]
    fn from_target_pd(
        asset_value: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        target_pd: f64,
        maturity: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::from_target_pd(
                asset_value,
                asset_vol,
                risk_free_rate,
                payout_rate,
                target_pd,
                maturity,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.kmv_default_point")]
    #[staticmethod]
    fn kmv_default_point(short_term_debt: f64, long_term_debt: f64) -> PyResult<f64> {
        MertonModel::kmv_default_point(short_term_debt, long_term_debt).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.new_with_dynamics")]
    #[staticmethod]
    #[allow(clippy::too_many_arguments)]
    fn new_with_dynamics(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        barrier_type: PyRef<'_, PyBarrierType>,
        dynamics: PyRef<'_, PyAssetDynamics>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::new_with_dynamics(
                asset_value,
                asset_vol,
                debt_barrier,
                risk_free_rate,
                payout_rate,
                barrier_type.inner,
                dynamics.inner,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.credit_grades")]
    #[staticmethod]
    fn credit_grades(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        barrier_uncertainty: f64,
        mean_recovery: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::credit_grades(
                equity_value,
                equity_vol,
                total_debt,
                risk_free_rate,
                barrier_uncertainty,
                mean_recovery,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.asset_value")]
    #[getter]
    fn asset_value(&self) -> f64 {
        self.inner.asset_value()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.asset_vol")]
    #[getter]
    fn asset_vol(&self) -> f64 {
        self.inner.asset_vol()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.debt_barrier")]
    #[getter]
    fn debt_barrier(&self) -> f64 {
        self.inner.debt_barrier()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.risk_free_rate")]
    #[getter]
    fn risk_free_rate(&self) -> f64 {
        self.inner.risk_free_rate()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.payout_rate")]
    #[getter]
    fn payout_rate(&self) -> f64 {
        self.inner.payout_rate()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.barrier_type")]
    #[getter]
    fn barrier_type(&self) -> PyBarrierType {
        PyBarrierType::from_inner(*self.inner.barrier_type())
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.dynamics")]
    #[getter]
    fn dynamics(&self) -> PyAssetDynamics {
        PyAssetDynamics::from_inner(*self.inner.dynamics())
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.distance_to_default")]
    fn distance_to_default(&self, horizon: f64) -> f64 {
        self.inner.distance_to_default(horizon)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.distance_to_default_with_drift")]
    fn distance_to_default_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .distance_to_default_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.default_probability")]
    fn default_probability(&self, horizon: f64) -> f64 {
        self.inner.default_probability(horizon)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.default_probabilities")]
    #[pyo3(text_signature = "($self, horizons)")]
    fn default_probabilities<'py>(
        &self,
        py: Python<'py>,
        horizons: Vec<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = horizons.iter().map(|h| h.to_string()).collect();
        let values = self.inner.default_probabilities(&horizons);
        values_to_series(py, values, &labels, "default_probability")
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.default_probability_with_drift")]
    fn default_probability_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .default_probability_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.implied_spread")]
    fn implied_spread(&self, horizon: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .implied_spread(horizon, recovery)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.debt_spread")]
    fn debt_spread(&self, horizon: f64) -> PyResult<f64> {
        self.inner.debt_spread(horizon).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.cds_par_spread")]
    fn cds_par_spread(&self, maturity: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .cds_par_spread(maturity, recovery)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.implied_equity")]
    fn implied_equity(&self, horizon: f64) -> PyResult<(f64, f64)> {
        self.inner.implied_equity(horizon).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.to_hazard_curve")]
    #[pyo3(text_signature = "($self, id, base_date, tenors, recovery, day_count)")]
    fn to_hazard_curve(
        &self,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        tenors: Vec<f64>,
        recovery: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<PyHazardCurve> {
        let base_date = py_to_date(base_date)?;
        let day_count = extract_day_count(day_count)?;
        let curve = self
            .inner
            .to_hazard_curve(id, base_date, &tenors, recovery, day_count)
            .map_err(core_to_py)?;
        Ok(PyHazardCurve::from_inner(Arc::new(curve)))
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "MertonModel.simulate_paths")]
    #[pyo3(text_signature = "($self, num_paths, num_steps, horizon, seed, antithetic)")]
    fn simulate_paths(
        &self,
        num_paths: usize,
        num_steps: usize,
        horizon: f64,
        seed: u64,
        antithetic: bool,
    ) -> PyResult<PySimulatedPaths> {
        let paths = self
            .inner
            .simulate_paths(num_paths, num_steps, horizon, seed, antithetic)
            .map_err(core_to_py)?;
        Ok(PySimulatedPaths::from_inner(paths))
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MertonModel", &self.inner)
    }
}

wire_methods!(
    PyMertonModel,
    MertonModel,
    "MertonModel",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths")]
#[pyclass(
    name = "SimulatedPaths",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PySimulatedPaths {
    pub(crate) inner: SimulatedPaths,
}

impl PySimulatedPaths {
    pub(crate) fn from_inner(inner: SimulatedPaths) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySimulatedPaths {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.times")]
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.asset_values")]
    #[getter]
    fn asset_values(&self) -> Vec<f64> {
        self.inner.asset_values.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.num_steps")]
    #[getter]
    fn num_steps(&self) -> usize {
        self.inner.num_steps
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.values_per_path")]
    #[getter]
    fn values_per_path(&self) -> usize {
        self.inner.values_per_path()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let per_path = self.inner.values_per_path();
        let n = self.inner.num_paths * per_path;
        let mut path_col = Vec::with_capacity(n);
        let mut time_col = Vec::with_capacity(n);
        for p in 0..self.inner.num_paths {
            for &time in &self.inner.times {
                path_col.push(p as u64);
                time_col.push(time);
            }
        }
        let columns = PyDict::new(py);
        columns.set_item("path", path_col)?;
        columns.set_item("time", time_col)?;
        columns.set_item("asset_value", self.inner.asset_values.clone())?;
        dict_to_dataframe(py, &columns, None)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.get")]
    fn get(&self, path_idx: usize, time_idx: usize) -> Option<f64> {
        self.inner.get(path_idx, time_idx)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.path")]
    fn path(&self, path_idx: usize) -> Option<Vec<f64>> {
        self.inner.path(path_idx).map(<[f64]>::to_vec)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "SimulatedPaths.to_nested")]
    fn to_nested(&self) -> Vec<Vec<f64>> {
        self.inner.to_nested()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SimulatedPaths", &self.inner)
    }
}

wire_methods!(
    PySimulatedPaths,
    SimulatedPaths,
    "SimulatedPaths",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec")]
#[pyclass(
    name = "DynamicRecoverySpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyDynamicRecoverySpec {
    pub(crate) inner: DynamicRecoverySpec,
}

#[pymethods]
impl PyDynamicRecoverySpec {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.constant")]
    #[staticmethod]
    #[pyo3(text_signature = "(recovery)")]
    fn constant(recovery: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::constant(recovery).map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.inverse_linear")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional)")]
    fn inverse_linear(base_recovery: f64, base_notional: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_linear(base_recovery, base_notional)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.inverse_power")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, exponent)")]
    fn inverse_power(base_recovery: f64, base_notional: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::inverse_power(base_recovery, base_notional, exponent)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.floored_inverse")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, floor)")]
    fn floored_inverse(base_recovery: f64, base_notional: f64, floor: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::floored_inverse(base_recovery, base_notional, floor)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.linear_decline")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_recovery, base_notional, sensitivity, floor)")]
    fn linear_decline(
        base_recovery: f64,
        base_notional: f64,
        sensitivity: f64,
        floor: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::linear_decline(
                base_recovery,
                base_notional,
                sensitivity,
                floor,
            )
            .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(self.inner.model()).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.recovery_at_notional")]
    #[pyo3(text_signature = "($self, notional)")]
    fn recovery_at_notional(&self, notional: f64) -> f64 {
        self.inner.recovery_at_notional(notional)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.base_recovery")]
    #[getter]
    fn base_recovery(&self) -> f64 {
        self.inner.base_recovery()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.base_notional")]
    #[getter]
    fn base_notional(&self) -> f64 {
        self.inner.base_notional()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.model")]
    #[getter]
    fn model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.model())
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "DynamicRecoverySpec.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DynamicRecoverySpec", &self.inner)
    }
}

wire_methods!(
    PyDynamicRecoverySpec,
    DynamicRecoverySpec,
    "DynamicRecoverySpec",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec")]
#[pyclass(
    name = "EndogenousHazardSpec",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyEndogenousHazardSpec {
    pub(crate) inner: EndogenousHazardSpec,
}

#[pymethods]
impl PyEndogenousHazardSpec {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.power_law")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, exponent)")]
    fn power_law(base_hazard: f64, base_leverage: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.exponential")]
    #[staticmethod]
    #[pyo3(text_signature = "(base_hazard, base_leverage, sensitivity)")]
    fn exponential(base_hazard: f64, base_leverage: f64, sensitivity: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::exponential(base_hazard, base_leverage, sensitivity)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.tabular")]
    #[staticmethod]
    #[pyo3(text_signature = "(leverage_points, hazard_points)")]
    fn tabular(leverage_points: Vec<f64>, hazard_points: Vec<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::tabular(leverage_points, hazard_points)
                .map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(self.inner.leverage_hazard_map()).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.hazard_at_leverage")]
    #[pyo3(text_signature = "($self, leverage)")]
    fn hazard_at_leverage(&self, leverage: f64) -> f64 {
        self.inner.hazard_at_leverage(leverage)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.hazard_after_pik_accrual")]
    #[pyo3(text_signature = "($self, accreted_notional, asset_value)")]
    fn hazard_after_pik_accrual(&self, accreted_notional: f64, asset_value: f64) -> f64 {
        self.inner
            .hazard_after_pik_accrual(accreted_notional, asset_value)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.base_hazard_rate")]
    #[getter]
    fn base_hazard_rate(&self) -> f64 {
        self.inner.base_hazard_rate()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.base_leverage")]
    #[getter]
    fn base_leverage(&self) -> f64 {
        self.inner.base_leverage()
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.leverage_hazard_map")]
    #[getter]
    fn leverage_hazard_map<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.leverage_hazard_map())
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "EndogenousHazardSpec.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EndogenousHazardSpec", &self.inner)
    }
}

wire_methods!(
    PyEndogenousHazardSpec,
    EndogenousHazardSpec,
    "EndogenousHazardSpec",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState")]
#[pyclass(
    name = "CreditState",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditState {
    pub(crate) inner: CreditState,
}

#[pymethods]
impl PyCreditState {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.__init__")]
    #[new]
    #[pyo3(signature = (hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None))]
    #[pyo3(
        text_signature = "(hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None)"
    )]
    fn new(
        hazard_rate: f64,
        distance_to_default: Option<f64>,
        leverage: f64,
        accreted_notional: f64,
        coupon_due: f64,
        asset_value: Option<f64>,
    ) -> PyResult<Self> {
        CreditState::new(
            hazard_rate,
            distance_to_default,
            leverage,
            accreted_notional,
            coupon_due,
            asset_value,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.hazard_rate")]
    #[getter]
    fn hazard_rate(&self) -> f64 {
        self.inner.hazard_rate
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.distance_to_default")]
    #[getter]
    fn distance_to_default(&self) -> Option<f64> {
        self.inner.distance_to_default
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.leverage")]
    #[getter]
    fn leverage(&self) -> f64 {
        self.inner.leverage
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.accreted_notional")]
    #[getter]
    fn accreted_notional(&self) -> f64 {
        self.inner.accreted_notional
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.coupon_due")]
    #[getter]
    fn coupon_due(&self) -> f64 {
        self.inner.coupon_due
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.asset_value")]
    #[getter]
    fn asset_value(&self) -> Option<f64> {
        self.inner.asset_value
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "CreditState.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditState", &self.inner)
    }
}

wire_methods!(
    PyCreditState,
    CreditState,
    "CreditState",
    module = "finstack_quant.models.credit._structural"
);

#[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel")]
#[pyclass(
    name = "ToggleExerciseModel",
    module = "finstack_quant.models.credit",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyToggleExerciseModel {
    pub(crate) inner: ToggleExerciseModel,
}

#[pymethods]
impl PyToggleExerciseModel {
    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.threshold")]
    #[staticmethod]
    #[pyo3(text_signature = "(variable, threshold, direction)")]
    fn threshold(variable: &str, threshold: f64, direction: &str) -> PyResult<Self> {
        let variable = serde_parse::<CreditStateVariable>(variable).map_err(core_to_py)?;
        let direction = serde_parse::<ThresholdDirection>(direction).map_err(core_to_py)?;
        ToggleExerciseModel::threshold(variable, threshold, direction)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.stochastic")]
    #[staticmethod]
    #[pyo3(text_signature = "(variable, intercept, sensitivity)")]
    fn stochastic(variable: &str, intercept: f64, sensitivity: f64) -> PyResult<Self> {
        let variable = serde_parse::<CreditStateVariable>(variable).map_err(core_to_py)?;
        ToggleExerciseModel::stochastic(variable, intercept, sensitivity)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.should_pik")]
    #[pyo3(text_signature = "($self, state, u)")]
    fn should_pik(&self, state: &PyCreditState, u: f64) -> bool {
        self.inner.should_pik(&state.inner, u)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.optimal")]
    #[staticmethod]
    #[pyo3(text_signature = "(nested_paths, asset_vol, risk_free_rate, horizon)")]
    fn optimal(
        nested_paths: usize,
        asset_vol: f64,
        risk_free_rate: f64,
        horizon: f64,
    ) -> PyResult<Self> {
        ToggleExerciseModel::optimal(nested_paths, asset_vol, risk_free_rate, horizon)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._structural", "ToggleExerciseModel.params")]
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.inner {
            ToggleExerciseModel::Threshold(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::Stochastic(spec) => serde_to_py(py, spec),
            ToggleExerciseModel::OptimalExercise(spec) => serde_to_py(py, spec),
        }
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ToggleExerciseModel", &self.inner)
    }
}

wire_methods!(
    PyToggleExerciseModel,
    ToggleExerciseModel,
    "ToggleExerciseModel",
    module = "finstack_quant.models.credit._structural"
);

/// Render an externally-tagged enum as its Python constructor call:
/// `"terminal"` -> `Name.terminal()`, `{"first_passage": {"k": v}}` ->
/// `Name.first_passage(k=v)`. Never fails; falls back to `Name(...)`.
fn variant_repr<T: serde::Serialize>(type_name: &str, value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(tag)) => format!("{type_name}.{tag}()"),
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => {
            let Some((tag, fields)) = map.iter().next() else {
                return format!("{type_name}(...)");
            };
            let inner = crate::bindings::repr_support::repr_from_serde("", fields);
            format!("{type_name}.{tag}{inner}")
        }
        _ => format!("{type_name}(...)"),
    }
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "credit")?;
    module.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit", "__module__"),
    )?;
    module.add_class::<PyAssetDynamics>()?;
    module.add_class::<PyBarrierType>()?;
    module.add_class::<PyMertonModel>()?;
    module.add_class::<PySimulatedPaths>()?;
    module.add_class::<PyDynamicRecoverySpec>()?;
    module.add_class::<PyEndogenousHazardSpec>()?;
    module.add_class::<PyCreditState>()?;
    module.add_class::<PyToggleExerciseModel>()?;
    module.add_class::<PyRatingFactorTable>()?;
    module.add_function(wrap_pyfunction!(moodys_warf_factor, &module)?)?;
    scoring::register(py, &module)?;
    pd::register(py, &module)?;
    lgd::register(py, &module)?;
    migration::register(py, &module)?;
    recovery_waterfall::register(py, &module)?;
    liability_management::register(py, &module)?;
    let all = PyList::new(
        py,
        [
            "AssetDynamics",
            "CreditState",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "MertonBarrierType",
            "MertonModel",
            "RatingFactorTable",
            "SimulatedPaths",
            "ToggleExerciseModel",
            "lgd",
            "liability_management",
            "migration",
            "moodys_warf_factor",
            "pd",
            "recovery_waterfall",
            "scoring",
        ],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
