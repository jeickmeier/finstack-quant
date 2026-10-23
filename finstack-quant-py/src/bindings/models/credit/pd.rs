//! Python bindings for `finstack_quant_models::credit::pd` (calibration subset).

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pydoc::pydoc;
use finstack_quant_models::credit::pd::{
    apply_basel_irb_pd_floor as core_apply_basel_irb_pd_floor,
    central_tendency as core_central_tendency, pit_to_ttc as core_pit_to_ttc,
    ttc_to_pit as core_ttc_to_pit, MasterScale, MasterScaleGrade, MasterScaleResult, PdCycleParams,
    BASEL_IRB_PD_FLOOR,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use super::scoring::PyScoringResult;
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{core_to_py, kinded_to_py};

/// Column schema shared by `MasterScaleResult.to_dataframe` and
/// `MasterScale.map_pds`.
const RESULT_COLUMNS: &[ColumnSchema<'static>] = &[
    ("grade", "str"),
    ("grade_index", "int64"),
    ("input_pd", "float64"),
    ("central_pd", "float64"),
];

#[doc = pydoc!("finstack_quant.models.credit.pd", "pit_to_ttc")]
#[pyfunction]
#[pyo3(text_signature = "(pd_pit, asset_correlation, cycle_index)")]
fn pit_to_ttc(pd_pit: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_pit_to_ttc(pd_pit, &params).map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.pd", "ttc_to_pit")]
#[pyfunction]
#[pyo3(text_signature = "(pd_ttc, asset_correlation, cycle_index)")]
fn ttc_to_pit(pd_ttc: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_ttc_to_pit(pd_ttc, &params).map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.pd", "central_tendency")]
#[pyfunction]
#[pyo3(text_signature = "(annual_default_rates)")]
fn central_tendency(annual_default_rates: Vec<f64>) -> PyResult<f64> {
    core_central_tendency(&annual_default_rates).map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.pd", "apply_basel_irb_pd_floor")]
#[pyfunction]
#[pyo3(text_signature = "(pd)")]
fn apply_basel_irb_pd_floor(pd: f64) -> f64 {
    core_apply_basel_irb_pd_floor(pd)
}

#[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade")]
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScaleGrade",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyMasterScaleGrade {
    inner: MasterScaleGrade,
}

#[pymethods]
impl PyMasterScaleGrade {
    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade.__init__")]
    #[new]
    #[pyo3(text_signature = "(label, upper_pd, central_pd)")]
    fn new(label: String, upper_pd: f64, central_pd: f64) -> Self {
        Self {
            inner: MasterScaleGrade {
                label,
                upper_pd,
                central_pd,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade.label")]
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade.upper_pd")]
    #[getter]
    fn upper_pd(&self) -> f64 {
        self.inner.upper_pd
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade.central_pd")]
    #[getter]
    fn central_pd(&self) -> f64 {
        self.inner.central_pd
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleGrade.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MasterScaleGrade", &self.inner)
    }
}

wire_methods!(
    PyMasterScaleGrade,
    MasterScaleGrade,
    "MasterScaleGrade",
    module = "finstack_quant.models.credit.pd"
);

#[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult")]
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScaleResult",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyMasterScaleResult {
    inner: MasterScaleResult,
}

#[pymethods]
impl PyMasterScaleResult {
    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.grade")]
    #[getter]
    fn grade(&self) -> &str {
        &self.inner.grade
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.central_pd")]
    #[getter]
    fn central_pd(&self) -> f64 {
        self.inner.central_pd
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.input_pd")]
    #[getter]
    fn input_pd(&self) -> f64 {
        self.inner.input_pd
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.grade_index")]
    #[getter]
    fn grade_index(&self) -> usize {
        self.inner.grade_index
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let columns: Vec<&str> = RESULT_COLUMNS.iter().map(|(name, _)| *name).collect();
        serde_object_to_single_row_dataframe_with_schema(py, &self.inner, &columns)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScaleResult.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("MasterScaleResult", &self.inner)
    }
}

wire_methods!(
    PyMasterScaleResult,
    MasterScaleResult,
    "MasterScaleResult",
    module = "finstack_quant.models.credit.pd"
);
impl_repr_html_via_dataframe!(PyMasterScaleResult);

#[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale")]
#[pyclass(
    module = "finstack_quant.models.credit.pd",
    name = "MasterScale",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMasterScale {
    inner: MasterScale,
}

#[pymethods]
impl PyMasterScale {
    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.__init__")]
    #[new]
    #[pyo3(text_signature = "(grades)")]
    fn new(grades: Vec<PyMasterScaleGrade>) -> PyResult<Self> {
        let grades = grades.into_iter().map(|g| g.inner).collect();
        MasterScale::new(grades)
            .map(|inner| Self { inner })
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.default_calibration")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn default_calibration() -> PyResult<Self> {
        MasterScale::default_calibration()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.from_registry_id")]
    #[staticmethod]
    #[pyo3(text_signature = "(scale_id)")]
    fn from_registry_id(scale_id: &str) -> PyResult<Self> {
        MasterScale::from_registry_id(scale_id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.map_pd")]
    #[pyo3(text_signature = "($self, pd)")]
    fn map_pd(&self, pd: f64) -> PyResult<PyMasterScaleResult> {
        let result = self.inner.map_pd(pd).map_err(kinded_to_py)?;
        Ok(PyMasterScaleResult { inner: result })
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.map_pds")]
    #[pyo3(text_signature = "($self, pds)")]
    fn map_pds<'py>(&self, py: Python<'py>, pds: Vec<f64>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.inner.map_pds(&pds).map_err(kinded_to_py)?;
        serde_rows_to_dataframe_with_schema(py, &rows, RESULT_COLUMNS)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.map_score")]
    #[pyo3(text_signature = "($self, result)")]
    fn map_score(&self, result: &PyScoringResult) -> PyResult<PyMasterScaleResult> {
        self.inner
            .map_score(&result.inner)
            .map(|inner| PyMasterScaleResult { inner })
            .map_err(kinded_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.n_grades")]
    #[getter]
    fn n_grades(&self) -> usize {
        self.inner.n_grades()
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.grades")]
    #[getter]
    fn grades(&self) -> Vec<PyMasterScaleGrade> {
        self.inner
            .grades()
            .iter()
            .cloned()
            .map(|inner| PyMasterScaleGrade { inner })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            self.inner.grades(),
            &[
                ("label", "str"),
                ("upper_pd", "float64"),
                ("central_pd", "float64"),
            ],
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.__len__")]
    fn __len__(&self) -> usize {
        self.inner.n_grades()
    }

    #[doc = pydoc!("finstack_quant.models.credit.pd", "MasterScale.__repr__")]
    fn __repr__(&self) -> String {
        let labels: Vec<String> = self
            .inner
            .grades()
            .iter()
            .map(|g| format!("{:?}", g.label))
            .collect();
        format!(
            "MasterScale(n_grades={}, labels=[{}])",
            self.inner.n_grades(),
            labels.join(", ")
        )
    }
}

wire_methods!(
    PyMasterScale,
    MasterScale,
    "MasterScale",
    module = "finstack_quant.models.credit.pd"
);

/// Build the `finstack_quant.models.credit.pd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "pd")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit.pd", "__module__"),
    )?;

    m.add("BASEL_IRB_PD_FLOOR", BASEL_IRB_PD_FLOOR)?;
    m.add_function(wrap_pyfunction!(apply_basel_irb_pd_floor, &m)?)?;
    m.add_function(wrap_pyfunction!(pit_to_ttc, &m)?)?;
    m.add_function(wrap_pyfunction!(ttc_to_pit, &m)?)?;
    m.add_function(wrap_pyfunction!(central_tendency, &m)?)?;
    m.add_class::<PyMasterScale>()?;
    m.add_class::<PyMasterScaleGrade>()?;
    m.add_class::<PyMasterScaleResult>()?;

    let all = PyList::new(
        py,
        [
            "BASEL_IRB_PD_FLOOR",
            "MasterScale",
            "MasterScaleGrade",
            "MasterScaleResult",
            "apply_basel_irb_pd_floor",
            "central_tendency",
            "pit_to_ttc",
            "ttc_to_pit",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
