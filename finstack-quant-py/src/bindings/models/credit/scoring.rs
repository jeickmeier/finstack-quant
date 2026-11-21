//! Python bindings for `finstack_quant_models::credit::scoring`.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
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
use crate::errors::{core_to_py, kinded_to_py};

/// A frozen pyclass over one scoring input struct: a keyword-only constructor
/// and one getter per Rust field, all decimal ratios.
macro_rules! scoring_input {
    (
        $py_type:ident,
        $rust_type:ident,
        $name:literal,
        $text_signature:literal,
        [$($field:ident),+ $(,)?]
    ) => {
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
            #[pyo3(signature = (**fields), text_signature = $text_signature)]
            fn new(
                py: Python<'_>,
                fields: Option<&Bound<'_, pyo3::types::PyDict>>,
            ) -> PyResult<Self> {
                Ok(Self {
                    inner: crate::bindings::module_utils::fields_from_py(py, fields, $name)?,
                })
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
    "(*, working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, market_equity_to_total_liabilities, sales_to_total_assets)",
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
    "(*, working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, sales_to_total_assets)",
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
    "(*, working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities)",
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
    "(*, log_total_assets_adjusted, total_liabilities_to_total_assets, working_capital_to_total_assets, current_liabilities_to_current_assets, liabilities_exceed_assets, net_income_to_total_assets, funds_from_operations_to_total_liabilities, negative_net_income_two_years, net_income_change)",
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
    "(*, net_income_to_total_assets, total_liabilities_to_total_assets, current_assets_to_current_liabilities)",
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

py_getters!(PyScoringResult |s| {
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.score")]
    score -> f64 = s.inner.score;
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.zone")]
    zone -> PyResult<String> = finstack_quant_core::wire::serde_label(&s.inner.zone).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.implied_pd")]
    implied_pd -> Option<f64> = s.inner.implied_pd;
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.model")]
    model -> String = s.inner.model.clone();
});

#[pymethods]
impl PyScoringResult {
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "ScoringResult.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScoringResult", &self.inner)
    }
}

impl PyScoringResult {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("model", vec![self.inner.model.clone()])?
            .f64("score", vec![self.inner.score])?
            .utf8("zone", vec![self.zone()?])?
            .nullable_f64("implied_pd", vec![self.inner.implied_pd])?
            .build()
    }
}

tabular_methods!(
    PyScoringResult,
    "ScoringResult",
    module = "finstack_quant.models.credit.scoring"
);
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
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditScoreReport", &self.inner)
    }
}

py_getters!(PyCreditScoreReport |s| {
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.result")]
    result -> PyScoringResult = PyScoringResult::from_inner(s.inner.result.clone());
    #[doc = pydoc!("finstack_quant.models.credit.scoring", "CreditScoreReport.grade")]
    grade -> Option<PyMasterScaleResult> = s.inner.grade.clone().map(PyMasterScaleResult::from_inner);
});

wire_methods!(
    PyCreditScoreReport,
    CreditScoreReport,
    "CreditScoreReport",
    module = "finstack_quant.models.credit.scoring"
);

macro_rules! scoring_fn {
    ($($fn_name:ident => ($core_fn:ident, $input_ty:ty);)+) => {
        $(
            #[doc = pydoc!("finstack_quant.models.credit.scoring", stringify!($fn_name))]
            #[pyfunction]
            #[pyo3(text_signature = "(input)")]
            fn $fn_name(input: $input_ty) -> PyResult<PyScoringResult> {
                $core_fn(&input.inner)
                    .map(PyScoringResult::from_inner)
                    .map_err(kinded_to_py)
            }
        )+
    };
}

scoring_fn!(
    altman_z_score => (core_altman_z_score, PyAltmanZScoreInput);
    altman_z_prime => (core_altman_z_prime, PyAltmanZPrimeInput);
    altman_z_double_prime => (core_altman_z_double_prime, PyAltmanZDoublePrimeInput);
    altman_em_score => (core_altman_em_score, PyAltmanZDoublePrimeInput);
    ohlson_o_score => (core_ohlson_o_score, PyOhlsonOScoreInput);
    zmijewski_score => (core_zmijewski_score, PyZmijewskiInput);
);

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
