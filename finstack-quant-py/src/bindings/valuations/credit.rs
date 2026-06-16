//! Python bindings for structural credit model specifications.

use crate::errors::display_to_py;
use finstack_quant_valuations::models::credit::{
    CreditState, CreditStateVariable, DynamicRecoverySpec, EndogenousHazardSpec, MertonModel,
    OptimalToggle, ThresholdDirection, ToggleExerciseModel,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

#[pyclass(
    name = "MertonModel",
    module = "finstack_quant.valuations.credit",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyMertonModel {
    inner: MertonModel,
}

#[pymethods]
impl PyMertonModel {
    #[new]
    fn new(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: MertonModel::new(asset_value, asset_vol, debt_barrier, risk_free_rate)
                .map_err(display_to_py)?,
        })
    }

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
            .map_err(display_to_py)?,
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(display_to_py)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    fn distance_to_default(&self, horizon: f64) -> f64 {
        self.inner.distance_to_default(horizon)
    }

    fn default_probability(&self, horizon: f64) -> f64 {
        self.inner.default_probability(horizon)
    }

    fn implied_spread(&self, horizon: f64, recovery: f64) -> PyResult<f64> {
        self.inner
            .implied_spread(horizon, recovery)
            .map_err(display_to_py)
    }
}

#[pyclass(
    name = "DynamicRecoverySpec",
    module = "finstack_quant.valuations.credit",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyDynamicRecoverySpec {
    inner: DynamicRecoverySpec,
}

#[pymethods]
impl PyDynamicRecoverySpec {
    #[staticmethod]
    fn constant(recovery: f64) -> PyResult<Self> {
        Ok(Self {
            inner: DynamicRecoverySpec::constant(recovery).map_err(display_to_py)?,
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(display_to_py)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    fn recovery_at_notional(&self, notional: f64) -> f64 {
        self.inner.recovery_at_notional(notional)
    }
}

#[pyclass(
    name = "EndogenousHazardSpec",
    module = "finstack_quant.valuations.credit",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyEndogenousHazardSpec {
    inner: EndogenousHazardSpec,
}

#[pymethods]
impl PyEndogenousHazardSpec {
    #[staticmethod]
    fn power_law(base_hazard: f64, base_leverage: f64, exponent: f64) -> PyResult<Self> {
        Ok(Self {
            inner: EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent)
                .map_err(display_to_py)?,
        })
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(display_to_py)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    fn hazard_at_leverage(&self, leverage: f64) -> f64 {
        self.inner.hazard_at_leverage(leverage)
    }

    fn hazard_after_pik_accrual(&self, accreted_notional: f64, asset_value: f64) -> f64 {
        self.inner
            .hazard_after_pik_accrual(accreted_notional, asset_value)
    }
}

#[pyclass(
    name = "CreditState",
    module = "finstack_quant.valuations.credit",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyCreditState {
    inner: CreditState,
}

#[pymethods]
impl PyCreditState {
    #[new]
    #[pyo3(signature = (hazard_rate=0.0, distance_to_default=None, leverage=0.0, accreted_notional=0.0, coupon_due=0.0, asset_value=None))]
    fn new(
        hazard_rate: f64,
        distance_to_default: Option<f64>,
        leverage: f64,
        accreted_notional: f64,
        coupon_due: f64,
        asset_value: Option<f64>,
    ) -> Self {
        Self {
            inner: CreditState {
                hazard_rate,
                distance_to_default,
                leverage,
                accreted_notional,
                coupon_due,
                asset_value,
            },
        }
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }
}

#[pyclass(
    name = "ToggleExerciseModel",
    module = "finstack_quant.valuations.credit",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyToggleExerciseModel {
    inner: ToggleExerciseModel,
}

#[pymethods]
impl PyToggleExerciseModel {
    #[staticmethod]
    fn threshold(variable: &str, threshold: f64, direction: &str) -> PyResult<Self> {
        let variable = variable
            .parse::<CreditStateVariable>()
            .map_err(display_to_py)?;
        let direction = direction
            .parse::<ThresholdDirection>()
            .map_err(display_to_py)?;
        Ok(Self {
            inner: ToggleExerciseModel::threshold(variable, threshold, direction),
        })
    }

    #[staticmethod]
    fn optimal(
        nested_paths: usize,
        equity_discount_rate: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        horizon: f64,
    ) -> Self {
        Self {
            inner: ToggleExerciseModel::OptimalExercise(OptimalToggle {
                nested_paths,
                equity_discount_rate,
                asset_vol,
                risk_free_rate,
                horizon,
            }),
        }
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(display_to_py)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "credit")?;
    module.add_class::<PyMertonModel>()?;
    module.add_class::<PyDynamicRecoverySpec>()?;
    module.add_class::<PyEndogenousHazardSpec>()?;
    module.add_class::<PyCreditState>()?;
    module.add_class::<PyToggleExerciseModel>()?;
    let all = PyList::new(
        py,
        [
            "MertonModel",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "CreditState",
            "ToggleExerciseModel",
        ],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "credit",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
