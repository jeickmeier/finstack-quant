//! Python bindings for `finstack_quant_models::credit::scoring`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_models::credit::scoring::{
    altman_em_score as core_altman_em_score, altman_z_double_prime as core_altman_z_double_prime,
    altman_z_prime as core_altman_z_prime, altman_z_score as core_altman_z_score,
    ohlson_o_score as core_ohlson_o_score, zmijewski_score as core_zmijewski_score,
    AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, OhlsonOScoreInput,
    ScoringResult, ZmijewskiInput,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, kinded_to_py};

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

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_score")]
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, market_equity_to_total_liabilities, sales_to_total_assets)"
)]
fn altman_z_score(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    market_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZScoreInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        market_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    core_altman_z_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_prime")]
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, sales_to_total_assets)"
)]
fn altman_z_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZPrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    core_altman_z_prime(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_z_double_prime")]
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities)"
)]
fn altman_z_double_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZDoublePrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    };
    core_altman_z_double_prime(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "altman_em_score")]
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities)"
)]
fn altman_em_score(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZDoublePrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    };
    core_altman_em_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "ohlson_o_score")]
#[pyfunction]
#[pyo3(
    text_signature = "(log_total_assets_adjusted, total_liabilities_to_total_assets, working_capital_to_total_assets, current_liabilities_to_current_assets, liabilities_exceed_assets, net_income_to_total_assets, funds_from_operations_to_total_liabilities, negative_net_income_two_years, net_income_change)"
)]
#[allow(clippy::too_many_arguments)]
fn ohlson_o_score(
    log_total_assets_adjusted: f64,
    total_liabilities_to_total_assets: f64,
    working_capital_to_total_assets: f64,
    current_liabilities_to_current_assets: f64,
    liabilities_exceed_assets: f64,
    net_income_to_total_assets: f64,
    funds_from_operations_to_total_liabilities: f64,
    negative_net_income_two_years: f64,
    net_income_change: f64,
) -> PyResult<PyScoringResult> {
    let input = OhlsonOScoreInput {
        log_total_assets_adjusted,
        total_liabilities_to_total_assets,
        working_capital_to_total_assets,
        current_liabilities_to_current_assets,
        liabilities_exceed_assets,
        net_income_to_total_assets,
        funds_from_operations_to_total_liabilities,
        negative_net_income_two_years,
        net_income_change,
    };
    core_ohlson_o_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.scoring", "zmijewski_score")]
#[pyfunction]
#[pyo3(
    text_signature = "(net_income_to_total_assets, total_liabilities_to_total_assets, current_assets_to_current_liabilities)"
)]
fn zmijewski_score(
    net_income_to_total_assets: f64,
    total_liabilities_to_total_assets: f64,
    current_assets_to_current_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = ZmijewskiInput {
        net_income_to_total_assets,
        total_liabilities_to_total_assets,
        current_assets_to_current_liabilities,
    };
    core_zmijewski_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(kinded_to_py)
}

/// Build the `finstack_quant.models.credit.scoring` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "scoring")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit.scoring", "__module__"),
    )?;

    m.add_class::<PyScoringResult>()?;
    m.add_function(wrap_pyfunction!(altman_z_score, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_double_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_em_score, &m)?)?;
    m.add_function(wrap_pyfunction!(ohlson_o_score, &m)?)?;
    m.add_function(wrap_pyfunction!(zmijewski_score, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ScoringResult",
            "altman_em_score",
            "altman_z_double_prime",
            "altman_z_prime",
            "altman_z_score",
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
