//! Python bindings for Merton Monte Carlo PIK bond pricing types.

use crate::bindings::models::credit::{
    PyDynamicRecoverySpec, PyEndogenousHazardSpec, PyMertonModel, PyToggleExerciseModel,
};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::merton_mc::{
    MertonMcResult, PathStatistics,
};
use finstack_quant_valuations::instruments::{
    BarrierCrossing, MertonMcCalibrationSpec, MertonMcConfig, PikMode, PikSchedule,
};
use pyo3::prelude::*;
use pyo3::types::PyList;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode")]
#[pyclass(
    name = "PikMode",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub(crate) struct PyPikMode {
    pub(crate) inner: PikMode,
}

impl PyPikMode {
    pub(crate) fn from_inner(inner: PikMode) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPikMode {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.cash")]
    #[staticmethod]
    fn cash() -> Self {
        Self {
            inner: PikMode::Cash,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.pik")]
    #[staticmethod]
    fn pik() -> Self {
        Self {
            inner: PikMode::Pik,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.split")]
    #[staticmethod]
    fn split(cash_fraction: f64, pik_fraction: f64) -> Self {
        Self {
            inner: PikMode::Split {
                cash_fraction,
                pik_fraction,
            },
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.toggle")]
    #[staticmethod]
    fn toggle() -> Self {
        Self {
            inner: PikMode::Toggle,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PikMode JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.to_json")]
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match self.inner {
            PikMode::Cash => "PikMode.cash()".to_string(),
            PikMode::Pik => "PikMode.pik()".to_string(),
            PikMode::Toggle => "PikMode.toggle()".to_string(),
            PikMode::Split {
                cash_fraction,
                pik_fraction,
            } => {
                format!("PikMode.split(cash_fraction={cash_fraction}, pik_fraction={pik_fraction})")
            }
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikMode.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule")]
#[pyclass(
    name = "PikSchedule",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPikSchedule {
    pub(crate) inner: PikSchedule,
}

#[pymethods]
impl PyPikSchedule {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.uniform")]
    #[staticmethod]
    fn uniform(mode: PyRef<'_, PyPikMode>) -> Self {
        Self {
            inner: PikSchedule::Uniform(mode.inner),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.stepped")]
    #[staticmethod]
    fn stepped(steps: &Bound<'_, PyAny>) -> PyResult<Self> {
        let list = steps.cast::<PyList>()?;
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            let (t, mode): (f64, PyRef<'_, PyPikMode>) = item.extract()?;
            out.push((t, mode.inner));
        }
        Ok(Self {
            inner: PikSchedule::Stepped(out),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.mode_at")]
    fn mode_at(&self, t: f64) -> PyPikMode {
        PyPikMode::from_inner(self.inner.mode_at(t))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PikSchedule JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            PikSchedule::Uniform(mode) => format!(
                "PikSchedule.uniform({})",
                PyPikMode::from_inner(*mode).__repr__()
            ),
            PikSchedule::Stepped(steps) => {
                let items: Vec<String> = steps
                    .iter()
                    .map(|(t, mode)| format!("({t}, {})", PyPikMode::from_inner(*mode).__repr__()))
                    .collect();
                format!("PikSchedule.stepped([{}])", items.join(", "))
            }
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PikSchedule.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing")]
#[pyclass(
    name = "BarrierCrossing",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub(crate) struct PyBarrierCrossing {
    pub(crate) inner: BarrierCrossing,
}

#[pymethods]
impl PyBarrierCrossing {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing.discrete")]
    #[staticmethod]
    fn discrete() -> Self {
        Self {
            inner: BarrierCrossing::Discrete,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing.brownian_bridge")]
    #[staticmethod]
    fn brownian_bridge() -> Self {
        Self {
            inner: BarrierCrossing::BrownianBridge,
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BarrierCrossing JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing.to_json")]
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match self.inner {
            BarrierCrossing::Discrete => "BarrierCrossing.discrete()".to_string(),
            BarrierCrossing::BrownianBridge => "BarrierCrossing.brownian_bridge()".to_string(),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "BarrierCrossing.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics")]
#[pyclass(
    name = "PathStatistics",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPathStatistics {
    pub(crate) inner: PathStatistics,
}

impl PyPathStatistics {
    pub(crate) fn from_inner(inner: PathStatistics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPathStatistics {
    /// Identify this value in notebooks and logs.
    ///
    /// The inner type is not `Serialize`, so the headline statistics are
    /// rendered directly.
    fn __repr__(&self) -> String {
        format!(
            "PathStatistics(default_rate={}, avg_recovery_rate={}, pik_exercise_rate={})",
            self.inner.default_rate, self.inner.avg_recovery_rate, self.inner.pik_exercise_rate
        )
    }
}

crate::bindings::macros::py_getters!(PyPathStatistics |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics.default_rate")]
    default_rate -> f64 = s.inner.default_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics.avg_default_time")]
    avg_default_time -> f64 = s.inner.avg_default_time;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics.avg_terminal_notional")]
    avg_terminal_notional -> f64 = s.inner.avg_terminal_notional;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics.avg_recovery_rate")]
    avg_recovery_rate -> f64 = s.inner.avg_recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PathStatistics.pik_exercise_rate")]
    pik_exercise_rate -> f64 = s.inner.pik_exercise_rate;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult")]
#[pyclass(
    name = "MertonMcResult",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcResult {
    pub(crate) inner: MertonMcResult,
}

impl PyMertonMcResult {
    pub(crate) fn from_inner(inner: MertonMcResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMertonMcResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult._repr_html_")]
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Identify this value in notebooks and logs.
    ///
    /// The inner type is not `Serialize`, so the headline results are
    /// rendered directly.
    fn __repr__(&self) -> String {
        format!(
            "MertonMcResult(clean_price_pct={}, effective_spread_bp={}, expected_loss={})",
            self.inner.clean_price_pct, self.inner.effective_spread_bp, self.inner.expected_loss
        )
    }
}

crate::bindings::macros::py_getters!(PyMertonMcResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.clean_price_pct")]
    clean_price_pct -> f64 = s.inner.clean_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.dirty_price_pct")]
    dirty_price_pct -> f64 = s.inner.dirty_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.expected_loss")]
    expected_loss -> f64 = s.inner.expected_loss;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.unexpected_loss")]
    unexpected_loss -> f64 = s.inner.unexpected_loss;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.expected_shortfall_95")]
    expected_shortfall_95 -> f64 = s.inner.expected_shortfall_95;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.average_pik_fraction")]
    average_pik_fraction -> f64 = s.inner.average_pik_fraction;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.effective_spread_bp")]
    effective_spread_bp -> f64 = s.inner.effective_spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.path_statistics")]
    path_statistics -> PyPathStatistics =
        PyPathStatistics::from_inner(s.inner.path_statistics.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.num_paths")]
    num_paths -> usize = s.inner.num_paths;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcResult.standard_error")]
    standard_error -> f64 = s.inner.standard_error;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec")]
#[pyclass(
    name = "MertonMcCalibrationSpec",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcCalibrationSpec {
    pub(crate) inner: MertonMcCalibrationSpec,
}

#[pymethods]
impl PyMertonMcCalibrationSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.__init__")]
    #[new]
    #[pyo3(signature = (target, parameter, low_paths=None, max_iterations=None, tolerance_pv=None, bracket=None, seed=None))]
    #[pyo3(
        text_signature = "(target, parameter, low_paths=None, max_iterations=None, tolerance_pv=None, bracket=None, seed=None)"
    )]
    // PyO3 binding: the argument list mirrors the Rust struct fields.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        target: &Bound<'_, PyAny>,
        parameter: &str,
        low_paths: Option<usize>,
        max_iterations: Option<usize>,
        tolerance_pv: Option<f64>,
        bracket: Option<(f64, f64)>,
        seed: Option<u64>,
    ) -> PyResult<Self> {
        let target = match target.extract::<String>() {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BondQuoteInput"))?,
            Err(_) => crate::bindings::module_utils::py_to_serde(py, target, "BondQuoteInput")?,
        };
        let defaults = MertonMcCalibrationSpec::default();
        Ok(Self {
            inner: MertonMcCalibrationSpec {
                target,
                parameter: super::convert::enum_from_str(parameter, "calibration parameter")?,
                low_paths: low_paths.unwrap_or(defaults.low_paths),
                max_iterations: max_iterations.unwrap_or(defaults.max_iterations),
                tolerance_pv: tolerance_pv.unwrap_or(defaults.tolerance_pv),
                bracket: bracket.or(defaults.bracket),
                seed: seed.or(defaults.seed),
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.target")]
    #[getter]
    fn target<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.target)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MertonMcCalibrationSpec JSON"))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "MertonMcCalibrationSpec(target={}, parameter={:?}, low_paths={}, max_iterations={})",
            serde_json::to_string(&self.inner.target).unwrap_or_default(),
            super::convert::enum_to_py_string(&self.inner.parameter).unwrap_or_default(),
            self.inner.low_paths,
            self.inner.max_iterations
        )
    }
}

crate::bindings::macros::py_getters!(PyMertonMcCalibrationSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.parameter")]
    parameter -> PyResult<String> = super::convert::enum_to_py_string(&s.inner.parameter);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.low_paths")]
    low_paths -> usize = s.inner.low_paths;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.max_iterations")]
    max_iterations -> usize = s.inner.max_iterations;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.tolerance_pv")]
    tolerance_pv -> f64 = s.inner.tolerance_pv;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.bracket")]
    bracket -> Option<(f64, f64)> = s.inner.bracket;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcCalibrationSpec.seed")]
    seed -> Option<u64> = s.inner.seed;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig")]
#[pyclass(
    name = "MertonMcConfig",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcConfig {
    pub(crate) inner: MertonMcConfig,
}

#[pymethods]
impl PyMertonMcConfig {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.__init__")]
    #[new]
    #[pyo3(text_signature = "(merton, recovery_rate)")]
    fn new(merton: PyRef<'_, PyMertonModel>, recovery_rate: f64) -> PyResult<Self> {
        Ok(Self {
            inner: MertonMcConfig::new(merton.inner.clone(), recovery_rate).map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.pik_schedule")]
    #[pyo3(text_signature = "($self, s)")]
    fn pik_schedule(&self, s: PyRef<'_, PyPikSchedule>) -> Self {
        Self {
            inner: self.inner.clone().pik_schedule(s.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.steps_per_year")]
    #[pyo3(text_signature = "($self, n)")]
    fn steps_per_year(&self, n: usize) -> Self {
        Self {
            inner: self.inner.clone().steps_per_year(n),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.barrier_crossing")]
    #[pyo3(text_signature = "($self, p)")]
    fn barrier_crossing(&self, p: PyRef<'_, PyBarrierCrossing>) -> Self {
        Self {
            inner: self.inner.clone().barrier_crossing(p.inner),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.recovery_rate")]
    #[pyo3(text_signature = "($self, r)")]
    fn recovery_rate(&self, r: f64) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().recovery_rate(r).map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.endogenous_hazard")]
    #[pyo3(text_signature = "($self, h)")]
    fn endogenous_hazard(&self, h: PyRef<'_, PyEndogenousHazardSpec>) -> Self {
        Self {
            inner: self.inner.clone().endogenous_hazard(h.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.dynamic_recovery")]
    #[pyo3(text_signature = "($self, r)")]
    fn dynamic_recovery(&self, r: PyRef<'_, PyDynamicRecoverySpec>) -> Self {
        Self {
            inner: self.inner.clone().dynamic_recovery(r.inner),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.toggle_model")]
    #[pyo3(text_signature = "($self, t)")]
    fn toggle_model(&self, t: PyRef<'_, PyToggleExerciseModel>) -> Self {
        Self {
            inner: self.inner.clone().toggle_model(t.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.calibration")]
    #[pyo3(text_signature = "($self, spec)")]
    fn calibration(&self, spec: PyRef<'_, PyMertonMcCalibrationSpec>) -> Self {
        Self {
            inner: self.inner.clone().calibration(spec.inner.clone()),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.cashflow_dfs")]
    #[pyo3(text_signature = "($self, dfs)")]
    fn cashflow_dfs(&self, dfs: Vec<(f64, f64)>) -> Self {
        Self {
            inner: self.inner.clone().cashflow_dfs(dfs),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MertonMcConfig JSON"))?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        let bool_repr = super::convert::bool_repr;
        format!(
            "MertonMcConfig(steps_per_year={}, barrier_crossing={}, recovery_rate={}, pik_schedule={}, endogenous_hazard={}, dynamic_recovery={}, toggle_model={}, calibration={}, cashflow_dfs={})",
            self.inner.steps_per_year,
            PyBarrierCrossing {
                inner: self.inner.barrier_crossing
            }
            .__repr__(),
            self.inner.recovery_rate,
            PyPikSchedule {
                inner: self.inner.pik_schedule.clone()
            }
            .__repr__(),
            bool_repr(self.inner.endogenous_hazard.is_some()),
            bool_repr(self.inner.dynamic_recovery.is_some()),
            bool_repr(self.inner.toggle_model.is_some()),
            bool_repr(self.inner.calibration.is_some()),
            bool_repr(self.inner.cashflow_dfs.is_some()),
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "MertonMcConfig.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

pub(crate) const EXPORTS: &[&str] = &[
    "BarrierCrossing",
    "MertonMcCalibrationSpec",
    "MertonMcConfig",
    "MertonMcResult",
    "PathStatistics",
    "PikMode",
    "PikSchedule",
];

/// Register Merton MC types on the instruments submodule.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBarrierCrossing>()?;
    m.add_class::<PyMertonMcCalibrationSpec>()?;
    m.add_class::<PyMertonMcConfig>()?;
    m.add_class::<PyMertonMcResult>()?;
    m.add_class::<PyPathStatistics>()?;
    m.add_class::<PyPikMode>()?;
    m.add_class::<PyPikSchedule>()?;
    Ok(())
}
