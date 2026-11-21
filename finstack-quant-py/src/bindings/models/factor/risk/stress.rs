use super::functions::extract_pnl_input;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, value_error};
use finstack_quant_factor::risk::{
    self as model_risk, StressAttribution, StressPositionEntry, TailScenarioBreakdown,
};
use pyo3::prelude::*;

fn stress_position_entries_table(rows: &[StressPositionEntry]) -> PyResult<PyArrowTable> {
    let position_ids: Vec<String> = rows
        .iter()
        .map(|e| e.position_id.as_str().to_owned())
        .collect();
    let avg_tail_pnl: Vec<f64> = rows.iter().map(|e| e.avg_tail_pnl).collect();
    let pct_of_tail_loss: Vec<f64> = rows.iter().map(|e| e.pct_of_tail_loss).collect();
    let worst_scenario_pnl: Vec<f64> = rows.iter().map(|e| e.worst_scenario_pnl).collect();
    TableBuilder::new()
        .utf8("position_id", position_ids)?
        .f64("avg_tail_pnl", avg_tail_pnl)?
        .f64("pct_of_tail_loss", pct_of_tail_loss)?
        .f64("worst_scenario_pnl", worst_scenario_pnl)?
        .build()
}

#[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry")]
#[pyclass(
    name = "StressPositionEntry",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressPositionEntry {
    pub(crate) inner: StressPositionEntry,
}

impl PyStressPositionEntry {
    fn from_inner(inner: StressPositionEntry) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyStressPositionEntry,
    StressPositionEntry,
    "StressPositionEntry",
    module = "finstack_quant.models.factor.risk._stress"
);

py_getters!(PyStressPositionEntry |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry.avg_tail_pnl")]
    avg_tail_pnl -> f64 = s.inner.avg_tail_pnl;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry.pct_of_tail_loss")]
    pct_of_tail_loss -> f64 = s.inner.pct_of_tail_loss;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry.worst_scenario_pnl")]
    worst_scenario_pnl -> f64 = s.inner.worst_scenario_pnl;
});

#[pymethods]
impl PyStressPositionEntry {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressPositionEntry.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("StressPositionEntry", &self.inner)
    }
}

impl PyStressPositionEntry {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        stress_position_entries_table(std::slice::from_ref(&self.inner))
    }
}

tabular_methods!(
    PyStressPositionEntry,
    "StressPositionEntry",
    module = "finstack_quant.models.factor.risk._stress"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown")]
#[pyclass(
    name = "TailScenarioBreakdown",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyTailScenarioBreakdown {
    pub(crate) inner: TailScenarioBreakdown,
    /// Position ordering inherited from the parent ``StressAttribution``;
    /// ``None`` for a breakdown built from JSON on its own.
    position_ids: Option<Vec<String>>,
}

impl PyTailScenarioBreakdown {
    fn from_inner(inner: TailScenarioBreakdown) -> Self {
        Self {
            inner,
            position_ids: None,
        }
    }

    fn with_position_ids(inner: TailScenarioBreakdown, position_ids: Vec<String>) -> Self {
        Self {
            inner,
            position_ids: Some(position_ids),
        }
    }
}

wire_methods!(
    PyTailScenarioBreakdown,
    TailScenarioBreakdown,
    "TailScenarioBreakdown",
    module = "finstack_quant.models.factor.risk._stress",
    from_inner
);

py_getters!(PyTailScenarioBreakdown |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.scenario_index")]
    scenario_index -> usize = s.inner.scenario_index;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.portfolio_pnl")]
    portfolio_pnl -> f64 = s.inner.portfolio_pnl;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.position_pnls")]
    position_pnls -> Vec<f64> = s.inner.position_pnls.clone();
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.position_ids")]
    position_ids -> Option<Vec<String>> = s.position_ids.clone();
});

#[pymethods]
impl PyTailScenarioBreakdown {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.to_arrow")]
    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_arrow(&self, position_ids: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        self.to_arrow_table_with_ids(position_ids)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.to_pandas")]
    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_pandas<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_pandas(&table, py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.to_polars")]
    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_polars<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.to_pyarrow")]
    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_pyarrow<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_pyarrow(&table, py)
    }

    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        let _ = requested_schema;
        self.to_arrow_table_with_ids(None)?.arrow_c_stream(py)
    }

    fn __arrow_c_schema__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        self.to_arrow_table_with_ids(None)?.arrow_c_schema(py)
    }

    fn _repr_html_(&self) -> Option<String> {
        self.to_arrow_table_with_ids(None)
            .ok()
            .map(|t| t.render_html())
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "TailScenarioBreakdown.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "TailScenarioBreakdown(scenario_index={}, portfolio_pnl={}, positions={})",
            self.inner.scenario_index,
            self.inner.portfolio_pnl,
            self.inner.position_pnls.len(),
        )
    }
}

impl PyTailScenarioBreakdown {
    fn to_arrow_table_with_ids(&self, position_ids: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        let Some(position_ids) = position_ids.or_else(|| self.position_ids.clone()) else {
            return Err(value_error(
                "position_ids is required: this TailScenarioBreakdown carries no parent \
                 StressAttribution ordering",
            ));
        };
        let expected = self.inner.position_pnls.len();
        if position_ids.len() != expected {
            return Err(value_error(format!(
                "position_ids length ({}) does not match position_pnls length ({expected})",
                position_ids.len(),
            )));
        }
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("pnl", self.inner.position_pnls.clone())?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution")]
#[pyclass(
    name = "StressAttribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressAttribution {
    pub(crate) inner: StressAttribution,
}

impl PyStressAttribution {
    pub(super) fn from_inner(inner: StressAttribution) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyStressAttribution,
    StressAttribution,
    "StressAttribution",
    module = "finstack_quant.models.factor.risk._stress"
);

py_getters!(PyStressAttribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.var_threshold")]
    var_threshold -> f64 = s.inner.var_threshold;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.n_tail_scenarios")]
    n_tail_scenarios -> usize = s.inner.n_tail_scenarios;
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.position_ids")]
    position_ids -> Vec<String> = s
        .inner
        .position_ids
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.position_contributions")]
    position_contributions -> Vec<PyStressPositionEntry> = s
        .inner
        .position_contributions
        .iter()
        .cloned()
        .map(PyStressPositionEntry::from_inner)
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.tail_scenarios")]
    tail_scenarios -> Vec<PyTailScenarioBreakdown> = {
        let ids: Vec<String> = s
            .inner
            .position_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        s.inner
            .tail_scenarios
            .iter()
            .cloned()
            .map(|scenario| PyTailScenarioBreakdown::with_position_ids(scenario, ids.clone()))
            .collect()
    };
});

#[pymethods]
impl PyStressAttribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.to_scenario_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_scenario_table(&self) -> PyResult<PyArrowTable> {
        let n_positions = self.inner.position_ids.len();
        for scenario in &self.inner.tail_scenarios {
            if scenario.position_pnls.len() != n_positions {
                return Err(value_error(format!(
                    "tail scenario {} has {} position P&Ls but {n_positions} position ids",
                    scenario.scenario_index,
                    scenario.position_pnls.len(),
                )));
            }
        }
        let scenario_indices: Vec<i64> = self
            .inner
            .tail_scenarios
            .iter()
            .map(|scenario| {
                i64::try_from(scenario.scenario_index)
                    .map_err(|e| value_error(format!("scenario_index overflow: {e}")))
            })
            .collect::<PyResult<_>>()?;
        let mut builder = TableBuilder::new().i64("scenario_index", scenario_indices)?;
        for (i, position_id) in self.inner.position_ids.iter().enumerate() {
            let column: Vec<f64> = self
                .inner
                .tail_scenarios
                .iter()
                .map(|scenario| scenario.position_pnls[i])
                .collect();
            builder = builder.f64(position_id.as_str(), column)?;
        }
        builder.build()
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._stress", "StressAttribution.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "StressAttribution(var_threshold={}, n_tail_scenarios={}, position_contributions={}, tail_scenarios={})",
            self.inner.var_threshold,
            self.inner.n_tail_scenarios,
            self.inner.position_contributions.len(),
            self.inner.tail_scenarios.len(),
        )
    }
}

impl PyStressAttribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        stress_position_entries_table(&self.inner.position_contributions)
    }
}

tabular_methods!(
    PyStressAttribution,
    "StressAttribution",
    module = "finstack_quant.models.factor.risk._stress"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._stress", "build_stress_attribution")]
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = None))]
pub(super) fn build_stress_attribution(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
    confidence: Option<f64>,
) -> PyResult<PyStressAttribution> {
    let (position_ids, position_pnls) = extract_pnl_input(py, position_ids, position_pnls)?;
    let n_positions = position_ids.len();
    let n_scenarios = position_pnls.n_scenarios();
    let result = py
        .detach(move || {
            let flat = position_pnls.into_scenario_major(n_positions);
            model_risk::build_stress_attribution(&position_ids, &flat, n_scenarios, confidence)
        })
        .map_err(core_to_py)?;
    Ok(PyStressAttribution::from_inner(result))
}
