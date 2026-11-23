//! Merton structural credit model and report Python bindings.

use super::paths::PySimulatedPaths;
use super::pd;
use super::toggle::variant_repr;
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::core::market_data::curves::PyHazardCurve;
use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::date_utils::py_to_date;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::{TableBuilder, serde_object_to_single_row_arrow_table};
use crate::errors::core_to_py;
use finstack_quant_models::credit::{
    AssetDynamics, MertonBarrierType, MertonModel, MertonReport, MertonReportRequest,
    merton_report as rust_merton_report,
};
use pyo3::prelude::*;
use std::sync::Arc;

#[doc = pydoc!("finstack_quant.models.credit._merton", "MertonBarrierType")]
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
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonBarrierType.terminal")]
    #[staticmethod]
    fn terminal() -> Self {
        Self {
            inner: MertonBarrierType::Terminal,
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonBarrierType.first_passage")]
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
    module = "finstack_quant.models.credit._merton"
);

#[doc = pydoc!("finstack_quant.models.credit._merton", "AssetDynamics")]
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
    #[doc = pydoc!("finstack_quant.models.credit._merton", "AssetDynamics.geometric_brownian")]
    #[staticmethod]
    fn geometric_brownian() -> Self {
        Self {
            inner: AssetDynamics::GeometricBrownian,
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "AssetDynamics.jump_diffusion")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "AssetDynamics.credit_grades")]
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
    module = "finstack_quant.models.credit._merton"
);

#[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel")]
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
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.__init__")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.from_equity")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.from_cds_spread")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.from_target_pd")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.kmv_default_point")]
    #[staticmethod]
    fn kmv_default_point(short_term_debt: f64, long_term_debt: f64) -> PyResult<f64> {
        MertonModel::kmv_default_point(short_term_debt, long_term_debt).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.new_with_dynamics")]
    #[staticmethod]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.credit_grades")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.distance_to_default")]
    fn distance_to_default(&self, horizon: f64) -> f64 {
        self.inner.distance_to_default(horizon)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.distance_to_default_with_drift")]
    fn distance_to_default_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .distance_to_default_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.default_probability")]
    fn default_probability(&self, horizon: f64) -> PyResult<f64> {
        self.inner.default_probability(horizon).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.default_probabilities")]
    #[pyo3(text_signature = "($self, horizons)")]
    fn default_probabilities(&self, horizons: Vec<f64>) -> PyResult<PyLabeledSeries> {
        let labels: Vec<String> = horizons.iter().map(|h| h.to_string()).collect();
        let values = self
            .inner
            .default_probabilities(&horizons)
            .map_err(core_to_py)?;
        Ok(
            PyLabeledSeries::from_f64("default_probability", labels, values)?
                .with_index_name("horizon"),
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.default_probability_with_drift")]
    fn default_probability_with_drift(&self, asset_drift: f64, horizon: f64) -> PyResult<f64> {
        self.inner
            .default_probability_with_drift(asset_drift, horizon)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.implied_spread")]
    fn implied_spread(&self, horizon: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .implied_spread(horizon, recovery)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.debt_spread")]
    fn debt_spread(&self, horizon: f64) -> PyResult<f64> {
        self.inner.debt_spread(horizon).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.cds_par_spread")]
    fn cds_par_spread(&self, maturity: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .cds_par_spread(maturity, recovery)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.implied_equity")]
    fn implied_equity(&self, horizon: f64) -> PyResult<(f64, f64)> {
        self.inner.implied_equity(horizon).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.to_hazard_curve")]
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

    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.simulate_paths")]
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

py_getters!(PyMertonModel |s| {
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.asset_value")]
    asset_value -> f64 = s.inner.asset_value();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.asset_vol")]
    asset_vol -> f64 = s.inner.asset_vol();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.debt_barrier")]
    debt_barrier -> f64 = s.inner.debt_barrier();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.risk_free_rate")]
    risk_free_rate -> f64 = s.inner.risk_free_rate();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.payout_rate")]
    payout_rate -> f64 = s.inner.payout_rate();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.barrier_type")]
    barrier_type -> PyBarrierType = PyBarrierType::from_inner(*s.inner.barrier_type());
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonModel.dynamics")]
    dynamics -> PyAssetDynamics = PyAssetDynamics::from_inner(*s.inner.dynamics());
});

impl PyMertonModel {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(
            &self.inner,
            &[
                ("asset_value", "float64"),
                ("asset_vol", "float64"),
                ("debt_barrier", "float64"),
                ("risk_free_rate", "float64"),
                ("payout_rate", "float64"),
                ("barrier_type", "str"),
                ("dynamics", "str"),
            ],
        )
    }
}

tabular_methods!(
    PyMertonModel,
    "MertonModel",
    module = "finstack_quant.models.credit._merton"
);
wire_methods!(
    PyMertonModel,
    MertonModel,
    "MertonModel",
    module = "finstack_quant.models.credit._merton"
);

#[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport")]
#[pyclass(
    name = "MertonReport",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonReport {
    pub(crate) inner: MertonReport,
}

#[pymethods]
impl PyMertonReport {
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MertonReport", &self.inner)
    }
}

py_getters!(PyMertonReport |s| {
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.model")]
    model -> PyMertonModel = PyMertonModel {
        inner: s.inner.model.clone(),
    };
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.default_point")]
    default_point -> Option<f64> = s.inner.default_point;
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.horizons")]
    horizons -> Vec<f64> = s.inner.horizons.clone();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.default_probabilities")]
    default_probabilities -> Vec<f64> = s.inner.default_probabilities.clone();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.distance_to_default")]
    distance_to_default -> Vec<f64> = s.inner.distance_to_default.clone();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.grades")]
    grades -> Vec<pd::PyMasterScaleResult> = s
        .inner
        .grades
        .iter()
        .cloned()
        .map(pd::PyMasterScaleResult::from_inner)
        .collect();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.implied_spread")]
    implied_spread -> Option<Vec<f64>> = s.inner.implied_spread.clone();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.debt_spread")]
    debt_spread -> Option<Vec<f64>> = s.inner.debt_spread.clone();
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.maturity")]
    maturity -> f64 = s.inner.maturity;
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.recovery")]
    recovery -> f64 = s.inner.recovery;
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.cds_par_spread")]
    cds_par_spread -> Option<f64> = s.inner.cds_par_spread;
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.implied_equity")]
    implied_equity -> Option<(f64, f64)> = s
        .inner
        .implied_equity
        .map(|equity| (equity.value, equity.vol));
    #[doc = pydoc!("finstack_quant.models.credit._merton", "MertonReport.errors")]
    errors -> std::collections::BTreeMap<&'static str, String> = s
        .inner
        .errors
        .iter()
        .map(|(measure, message)| (measure.as_str(), message.clone()))
        .collect();
});

impl PyMertonReport {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let per_horizon = |measure: &Option<Vec<f64>>| -> Vec<Option<f64>> {
            match measure {
                Some(values) => values.iter().copied().map(Some).collect(),
                None => vec![None; self.inner.horizons.len()],
            }
        };
        let grades: Vec<String> = self
            .inner
            .grades
            .iter()
            .map(|row| row.grade.clone())
            .collect();
        let central_pds: Vec<f64> = self.inner.grades.iter().map(|row| row.central_pd).collect();
        TableBuilder::new()
            .f64("horizon", self.inner.horizons.clone())?
            .f64(
                "default_probability",
                self.inner.default_probabilities.clone(),
            )?
            .f64(
                "distance_to_default",
                self.inner.distance_to_default.clone(),
            )?
            .nullable_f64("implied_spread", per_horizon(&self.inner.implied_spread))?
            .nullable_f64("debt_spread", per_horizon(&self.inner.debt_spread))?
            .utf8("grade", grades)?
            .f64("central_pd", central_pds)?
            .build()
    }
}

tabular_methods!(
    PyMertonReport,
    "MertonReport",
    module = "finstack_quant.models.credit._merton"
);
wire_methods!(
    PyMertonReport,
    MertonReport,
    "MertonReport",
    module = "finstack_quant.models.credit._merton"
);

#[doc = pydoc!("finstack_quant.models.credit._merton", "merton_report")]
#[pyfunction]
#[pyo3(text_signature = "(request)")]
pub(super) fn merton_report(request: &Bound<'_, PyAny>) -> PyResult<PyMertonReport> {
    let request: MertonReportRequest =
        crate::bindings::module_utils::spec_from_py(request.py(), request, "MertonReportRequest")?;
    Ok(PyMertonReport {
        inner: rust_merton_report(&request).map_err(core_to_py)?,
    })
}
