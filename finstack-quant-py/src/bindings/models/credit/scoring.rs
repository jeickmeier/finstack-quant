//! Python bindings for `finstack_quant_models::credit::scoring`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_models::credit::scoring::{
    AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, CreditScoreReport,
    CreditScoreRequest, OhlsonOScoreInput, ScoringResult, ZmijewskiInput,
    altman_em_score as core_altman_em_score, altman_z_double_prime as core_altman_z_double_prime,
    altman_z_prime as core_altman_z_prime, altman_z_score as core_altman_z_score,
    credit_score_report as core_credit_score_report, ohlson_o_score as core_ohlson_o_score,
    zmijewski_score as core_zmijewski_score,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use super::pd::PyMasterScaleResult;
use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, kinded_to_py};

/// A frozen pyclass over one scoring input struct: a keyword-only constructor
/// and one getter per Rust field, all decimal ratios.
macro_rules! scoring_input {
    ($py_type:ident, $rust_type:ident, $name:literal, [$($field:ident),+ $(,)?]) => {
        #[doc = pydoc!("finstack_quant.models.credit.scoring", $name)]
        #[pyclass(
            module = "finstack_quant.models.credit.scoring",
            name = $name,
            frozen,
            eq,
            from_py_object
        )]
        #[derive(Clone, PartialEq)]
        pub(crate) struct $py_type {
            pub(crate) inner: $rust_type,
        }

        #[pymethods]
        impl $py_type {
            #[doc = pydoc!("finstack_quant.models.credit.scoring", concat!($name, ".__init__"))]
            #[new]
            #[pyo3(signature = (*, $($field),+))]
            #[allow(clippy::too_many_arguments)]
            fn new($($field: f64),+) -> Self {
                Self {
                    inner: $rust_type { $($field),+ },
                }
            }

            $(
                #[doc = pydoc!(
                    "finstack_quant.models.credit.scoring",
                    concat!($name, ".", stringify!($field))
                )]
                #[getter]
                fn $field(&self) -> f64 {
                    self.inner.$field
                }
            )+

            #[doc = pydoc!("finstack_quant.models.credit.scoring", concat!($name, ".__repr__"))]
            fn __repr__(&self) -> String {
                crate::bindings::repr_support::repr_from_serde($name, &self.inner)
            }
        }

        wire_methods!(
            $py_type,
            $rust_type,
            $name,
            module = "finstack_quant.models.credit.scoring"
        );
    };
}

scoring_input!(
    PyAltmanZScoreInput,
    AltmanZScoreInput,
    "AltmanZScoreInput",
    [
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        market_equity_to_total_liabilities,
        sales_to_total_assets,
    ]
);

scoring_input!(
    PyAltmanZPrimeInput,
    AltmanZPrimeInput,
    "AltmanZPrimeInput",
    [
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
        sales_to_total_assets,
    ]
);

scoring_input!(
    PyAltmanZDoublePrimeInput,
    AltmanZDoublePrimeInput,
    "AltmanZDoublePrimeInput",
    [
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    ]
);

scoring_input!(
    PyOhlsonOScoreInput,
    OhlsonOScoreInput,
    "OhlsonOScoreInput",
    [
        log_total_assets_adjusted,
        total_liabilities_to_total_assets,
        working_capital_to_total_assets,
        current_liabilities_to_current_assets,
        liabilities_exceed_assets,
        net_income_to_total_assets,
        funds_from_operations_to_total_liabilities,
        negative_net_income_two_years,
        net_income_change,
    ]
);

scoring_input!(
    PyZmijewskiInput,
    ZmijewskiInput,
    "ZmijewskiInput",
    [
        net_income_to_total_assets,
        total_liabilities_to_total_assets,
        current_assets_to_current_liabilities,
    ]
);

#[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult")]
#[pyclass(
    module = "finstack_quant.models.credit.scoring",
    name = "ScoringResult",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyScoringResult {
    pub(crate) inner: ScoringResult,
}

impl PyScoringResult {
    pub(crate) fn from_inner(inner: ScoringResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyScoringResult {
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.score")]
    #[getter]
    fn score(&self) -> f64 {
        self.inner.score
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.zone")]
    #[getter]
    fn zone(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.zone).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.implied_pd")]
    #[getter]
    fn implied_pd(&self) -> Option<f64> {
        self.inner.implied_pd
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.model")]
    #[getter]
    fn model(&self) -> String {
        self.inner.model.clone()
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["model", "score", "zone", "implied_pd"],
        )
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScoringResult", &self.inner)
    }
}

wire_methods!(
    PyScoringResult,
    ScoringResult,
    "ScoringResult",
    module = "finstack_quant.models.credit.scoring"
);

#[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport")]
#[pyclass(
    module = "finstack_quant.models.credit.scoring",
    name = "CreditScoreReport",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyCreditScoreReport {
    pub(crate) inner: CreditScoreReport,
}

#[pymethods]
impl PyCreditScoreReport {
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.result")]
    #[getter]
    fn result(&self) -> PyScoringResult {
        PyScoringResult::from_inner(self.inner.result.clone())
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.grade")]
    #[getter]
    fn grade(&self) -> Option<PyMasterScaleResult> {
        self.inner
            .grade
            .clone()
            .map(PyMasterScaleResult::from_inner)
    }

    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditScoreReport", &self.inner)
    }
}

wire_methods!(
    PyCreditScoreReport,
    CreditScoreReport,
    "CreditScoreReport",
    module = "finstack_quant.models.credit.scoring"
);

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_score")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn altman_z_score(input: PyAltmanZScoreInput) -> PyResult<PyScoringResult> {
    core_altman_z_score(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_prime")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn altman_z_prime(input: PyAltmanZPrimeInput) -> PyResult<PyScoringResult> {
    core_altman_z_prime(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_double_prime")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn altman_z_double_prime(input: PyAltmanZDoublePrimeInput) -> PyResult<PyScoringResult> {
    core_altman_z_double_prime(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_em_score")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn altman_em_score(input: PyAltmanZDoublePrimeInput) -> PyResult<PyScoringResult> {
    core_altman_em_score(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "ohlson_o_score")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn ohlson_o_score(input: PyOhlsonOScoreInput) -> PyResult<PyScoringResult> {
    core_ohlson_o_score(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "zmijewski_score")]
#[pyfunction]
#[pyo3(text_signature = "(input)")]
fn zmijewski_score(input: PyZmijewskiInput) -> PyResult<PyScoringResult> {
    core_zmijewski_score(&input.inner)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "credit_score_report")]
#[pyfunction]
#[pyo3(text_signature = "(request)")]
fn credit_score_report(request: &Bound<'_, PyAny>) -> PyResult<PyCreditScoreReport> {
    let request: CreditScoreRequest =
        crate::bindings::module_utils::spec_from_py(request.py(), request, "CreditScoreRequest")?;
    Ok(PyCreditScoreReport {
        inner: core_credit_score_report(&request).map_err(core_to_py)?,
    })
}

/// Build the `finstack_quant.models.credit.scoring` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "scoring")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit.scoring", "__module__"),
    )?;

    m.add_class::<PyAltmanZDoublePrimeInput>()?;
    m.add_class::<PyAltmanZPrimeInput>()?;
    m.add_class::<PyAltmanZScoreInput>()?;
    m.add_class::<PyCreditScoreReport>()?;
    m.add_class::<PyOhlsonOScoreInput>()?;
    m.add_class::<PyScoringResult>()?;
    m.add_class::<PyZmijewskiInput>()?;
    m.add_function(wrap_pyfunction!(altman_z_score, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_double_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_em_score, &m)?)?;
    m.add_function(wrap_pyfunction!(credit_score_report, &m)?)?;
    m.add_function(wrap_pyfunction!(ohlson_o_score, &m)?)?;
    m.add_function(wrap_pyfunction!(zmijewski_score, &m)?)?;

    let all = PyList::new(
        py,
        [
            "AltmanZDoublePrimeInput",
            "AltmanZPrimeInput",
            "AltmanZScoreInput",
            "CreditScoreReport",
            "OhlsonOScoreInput",
            "ScoringResult",
            "ZmijewskiInput",
            "altman_em_score",
            "altman_z_double_prime",
            "altman_z_prime",
            "altman_z_score",
            "credit_score_report",
            "ohlson_o_score",
            "zmijewski_score",
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
