//! DCF valuation results, sensitivity grids, and Monte Carlo distributions.

use super::inputs::{PyCorporateDcfRequest, PyEquityBridge};
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::extract_model_ref;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements_analytics::typed::PyTornadoEntry;
use crate::bindings::table_builders::{TableBuilder, serde_rows_to_arrow_table};
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis::DcfDistributionResult;
use finstack_quant_statements_analytics::analysis::{
    CorporateValuationResult, DcfSensitivityResult,
};
use finstack_quant_statements_analytics::analysis::{
    DcfSensitivityEndpoint, DcfSensitivityGridResult, DcfSensitivityGridSpec,
};
use pyo3::prelude::*;

/// Column schema for [`PyDcfSensitivityResult`] tabular exports.
const TORNADO_COLUMNS: [(&str, &str); 4] = [
    ("parameter_id", "str"),
    ("downside", "float64"),
    ("upside", "float64"),
    ("swing", "float64"),
];

#[pyclass(
    name = "DcfSensitivityGridSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfSensitivityGridSpec")]
#[derive(Clone)]
pub(super) struct PyDcfSensitivityGridSpec {
    inner: DcfSensitivityGridSpec,
}

#[pymethods]
impl PyDcfSensitivityGridSpec {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfSensitivityGridSpec.__init__")]
    fn new(wacc_values: Vec<f64>, terminal_values: Vec<f64>) -> PyResult<Self> {
        let inner = DcfSensitivityGridSpec {
            wacc_values,
            terminal_values,
        };
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }
}

#[pyclass(
    name = "DcfSensitivityGridResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfSensitivityGridResult")]
#[derive(Clone)]
pub(super) struct PyDcfSensitivityGridResult {
    inner: DcfSensitivityGridResult,
}

#[pymethods]
impl PyDcfSensitivityGridResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfSensitivityGridResult.baseline_enterprise_value")]
    fn baseline_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.baseline_enterprise_value)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfSensitivityGridResult.to_pandas")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        DcfSensitivityEndpoint::Priced {
                            enterprise_value, ..
                        } => Some(enterprise_value.amount()),
                        DcfSensitivityEndpoint::Unavailable { .. } => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs.set_item("index", &self.inner.axes.wacc_values)?;
        kwargs.set_item("columns", &self.inner.axes.terminal_values)?;
        py.import("pandas")?
            .getattr("DataFrame")?
            .call((rows,), Some(&kwargs))
    }
}

impl PyDcfSensitivityGridResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut builder = TableBuilder::new().f64("wacc", self.inner.axes.wacc_values.clone())?;
        for (col_idx, term_val) in self.inner.axes.terminal_values.iter().enumerate() {
            let col_vals: Vec<Option<f64>> = self
                .inner
                .cells
                .iter()
                .map(|row| match row.get(col_idx) {
                    Some(DcfSensitivityEndpoint::Priced {
                        enterprise_value, ..
                    }) => Some(enterprise_value.amount()),
                    _ => None,
                })
                .collect();
            builder = builder.nullable_f64(&term_val.to_string(), col_vals)?;
        }
        builder.build()
    }
}

tabular_methods!(
    PyDcfSensitivityGridResult,
    "DcfSensitivityGridResult",
    module = "finstack_quant.statements_analytics._valuation.sensitivity",
    custom_pandas
);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "dcf_sensitivity_on_results")]
#[pyfunction]
pub(super) fn dcf_sensitivity_on_results<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    results: &Bound<'py, PyAny>,
    request: &PyCorporateDcfRequest,
    shocks: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let model = extract_model_ref(model)?.into_owned();
    let results = crate::bindings::extract::extract_results_ref(results)?.into_owned();
    let request = request.inner.clone();
    let shocks = crate::bindings::module_utils::spec_from_py(py, shocks, "DCF shocks")?;
    let report = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::dcf_sensitivity_on_results(
                &model, &results, &request, &shocks,
            )
        })
        .map_err(statements_to_py)?;
    crate::bindings::json_bridge::serde_to_py(py, &report)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "dcf_sensitivity_grid")]
#[pyfunction]
pub(super) fn dcf_sensitivity_grid<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    results: &Bound<'py, PyAny>,
    request: &PyCorporateDcfRequest,
    axes: &PyDcfSensitivityGridSpec,
) -> PyResult<PyDcfSensitivityGridResult> {
    let model = extract_model_ref(model)?.into_owned();
    let results = crate::bindings::extract::extract_results_ref(results)?.into_owned();
    let request = request.inner.clone();
    let axes = axes.inner.clone();
    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::dcf_sensitivity_grid(
                &model, &results, &request, &axes,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyDcfSensitivityGridResult { inner })
}

#[pyclass(
    name = "DcfDistributionResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfDistributionResult")]
#[derive(Clone)]
pub(super) struct PyDcfDistributionResult {
    inner: DcfDistributionResult,
}

#[pymethods]
impl PyDcfDistributionResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "DcfDistributionResult.quantiles_to_table")]
    fn quantiles_to_table(&self) -> PyResult<PyArrowTable> {
        let rows = self
            .inner
            .quantiles
            .iter()
            .map(|row| {
                serde_json::json!({
                    "probability": row.probability,
                    "enterprise_value": row.enterprise_value.amount(),
                    "equity_value": row.equity_value.amount(),
                    "currency": row.enterprise_value.currency().to_string(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_arrow_table(
            &rows,
            &[
                ("probability", "float64"),
                ("enterprise_value", "float64"),
                ("equity_value", "float64"),
                ("currency", "str"),
            ],
        )
    }
}

impl PyDcfDistributionResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows = self
            .inner
            .paths
            .iter()
            .map(|row| {
                serde_json::json!({
                    "path_id": row.path_id,
                    "enterprise_value": row.enterprise_value.amount(),
                    "equity_value": row.equity_value.amount(),
                    "currency": row.enterprise_value.currency().to_string(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_arrow_table(
            &rows,
            &[
                ("path_id", "uint32"),
                ("enterprise_value", "float64"),
                ("equity_value", "float64"),
                ("currency", "str"),
            ],
        )
    }
}

tabular_methods!(
    PyDcfDistributionResult,
    "DcfDistributionResult",
    module = "finstack_quant.statements_analytics._valuation.sensitivity"
);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "evaluate_dcf_monte_carlo")]
#[pyfunction]
pub(super) fn evaluate_dcf_monte_carlo<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    results: &Bound<'py, PyAny>,
    paths: &crate::bindings::statements::monte_carlo::PyMonteCarloResults,
    request: &PyCorporateDcfRequest,
) -> PyResult<PyDcfDistributionResult> {
    let model = extract_model_ref(model)?.into_owned();
    let results = crate::bindings::extract::extract_results_ref(results)?.into_owned();
    let paths = paths.inner.clone();
    let request = request.inner.clone();
    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_dcf_monte_carlo(
                &model, &results, &paths, &request,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyDcfDistributionResult { inner })
}

/// DCF outputs in the model currency.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CorporateValuationResult
/// >>> r = CorporateValuationResult.from_json(
/// ...     '{"equity_value":{"amount":"90","currency":"USD"},"enterprise_value":{"amount":"100","currency":"USD"},'
/// ...     '"net_debt":{"amount":"10","currency":"USD"},"other_equity_claims":{"amount":"0","currency":"USD"},"valuation_discount":{"amount":"0","currency":"USD"},'
/// ...     '"terminal_value_pv":{"amount":"60","currency":"USD"},'
/// ...     '"equity_value_per_share":null,"diluted_shares":null}')
/// >>> r.equity_value.amount
/// 90.0
#[pyclass(
    name = "CorporateValuationResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorporateValuationResult {
    pub(crate) inner: CorporateValuationResult,
}

#[pymethods]
impl PyCorporateValuationResult {
    /// Equity value (EV less net debt, after discounts).
    #[getter]
    fn equity_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.equity_value)
    }

    /// Enterprise value (PV of forecast cash flows plus terminal value).
    #[getter]
    fn enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.enterprise_value)
    }

    /// Interest-bearing debt less cash in model currency.
    #[getter]
    fn net_debt(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.net_debt)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.sensitivity", "CorporateValuationResult.other_equity_claims")]
    fn other_equity_claims(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.other_equity_claims)
    }

    /// Total valuation discount (DLOM, DLOC, other) deducted after the
    /// EV-to-equity bridge; zero without ``valuation_discounts``.
    /// ``enterprise_value - net_debt - other_equity_claims - valuation_discount`` equals
    /// ``equity_value``.
    #[getter]
    fn valuation_discount(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.valuation_discount)
    }

    /// Present value of the terminal value.
    #[getter]
    fn terminal_value_pv(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.terminal_value_pv)
    }

    /// Equity value per diluted share, or ``None`` without ``shares_outstanding``.
    #[getter]
    fn equity_value_per_share(&self) -> Option<f64> {
        self.inner.equity_value_per_share
    }

    /// Diluted share count, or ``None`` without ``shares_outstanding``.
    #[getter]
    fn diluted_shares(&self) -> Option<f64> {
        self.inner.diluted_shares
    }

    /// Discount rate (WACC) applied to every flow, as an annually compounded
    /// decimal; ``None`` only for a result loaded from JSON written before
    /// the DCF detail was recorded.
    #[getter]
    fn wacc(&self) -> Option<f64> {
        self.inner.wacc
    }

    /// Explicit forecast periods as a list of dicts with keys ``period_id``,
    /// ``date``, ``free_cash_flow``, ``discount_years``, ``discount_factor``
    /// and ``present_value`` (unrounded floats in the model currency).
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    /// Present value of the explicit forecast flows (the sum of the
    /// ``present_value`` of ``periods``).
    #[getter]
    fn pv_explicit(&self) -> Option<PyMoney> {
        self.inner.pv_explicit.map(PyMoney::from_inner)
    }

    /// Terminal value at the horizon date, before discounting.
    #[getter]
    fn terminal_value(&self) -> Option<PyMoney> {
        self.inner.terminal_value.map(PyMoney::from_inner)
    }

    /// Discounting tenor of the terminal value in years (ACT/365.25).
    #[getter]
    fn terminal_discount_years(&self) -> Option<f64> {
        self.inner.terminal_discount_years
    }

    /// EV-to-equity bridge behind ``net_debt``: gross debt and cash
    /// separately, plus the other bridge components.
    #[getter]
    fn equity_bridge(&self) -> Option<PyEquityBridge> {
        self.inner
            .equity_bridge
            .clone()
            .map(|inner| PyEquityBridge { inner })
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CorporateValuationResult", &self.inner)
    }
}

wire_methods!(
    PyCorporateValuationResult,
    CorporateValuationResult,
    "CorporateValuationResult"
);

impl PyCorporateValuationResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 15] = [
            ("currency", "str"),
            ("equity_value", "float64"),
            ("enterprise_value", "float64"),
            ("net_debt", "float64"),
            ("other_equity_claims", "float64"),
            ("valuation_discount", "float64"),
            ("terminal_value_pv", "float64"),
            ("equity_value_per_share", "float64"),
            ("diluted_shares", "float64"),
            ("wacc", "float64"),
            ("pv_explicit", "float64"),
            ("terminal_value", "float64"),
            ("total_debt", "float64"),
            ("cash", "float64"),
            ("terminal_discount_years", "float64"),
        ];
        let bridge = self.inner.equity_bridge.as_ref();
        let row = serde_json::json!({
            "currency": self.inner.equity_value.currency().to_string(),
            "equity_value": self.inner.equity_value.amount(),
            "enterprise_value": self.inner.enterprise_value.amount(),
            "net_debt": self.inner.net_debt.amount(),
            "other_equity_claims": self.inner.other_equity_claims.amount(),
            "valuation_discount": self.inner.valuation_discount.amount(),
            "terminal_value_pv": self.inner.terminal_value_pv.amount(),
            "equity_value_per_share": self.inner.equity_value_per_share,
            "diluted_shares": self.inner.diluted_shares,
            "wacc": self.inner.wacc,
            "pv_explicit": self.inner.pv_explicit.map(|money| money.amount()),
            "terminal_value": self.inner.terminal_value.map(|money| money.amount()),
            "total_debt": bridge.map(|bridge| bridge.total_debt),
            "cash": bridge.map(|bridge| bridge.cash),
            "terminal_discount_years": self.inner.terminal_discount_years,
        });
        serde_rows_to_arrow_table(&[row], &COLUMNS)
    }
}

tabular_methods!(PyCorporateValuationResult);

/// Tornado ranking of the headline DCF assumptions by enterprise-value impact.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import DcfSensitivityResult
/// >>> r = DcfSensitivityResult.from_json(
/// ...     '{"baseline_enterprise_value":{"amount":"100","currency":"USD"},'
/// ...     '"entries":[{"parameter_id":"wacc","downside":-5.0,"upside":6.0}],'
/// ...     '"wacc_down":0.09,"wacc_down_clamped":false,"terminal_growth_up":0.03,"terminal_growth_up_clamped":false}')
/// >>> list(r.to_pandas()["swing"])
/// [11.0]
#[pyclass(
    name = "DcfSensitivityResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyDcfSensitivityResult {
    pub(crate) inner: DcfSensitivityResult,
}

#[pymethods]
impl PyDcfSensitivityResult {
    /// Unshocked enterprise value.
    #[getter]
    fn baseline_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.baseline_enterprise_value)
    }

    /// Tornado entries sorted by descending absolute swing (EV deltas versus
    /// the baseline).
    #[getter]
    fn entries(&self) -> Vec<PyTornadoEntry> {
        self.inner
            .entries
            .iter()
            .cloned()
            .map(PyTornadoEntry::from_inner)
            .collect()
    }

    /// WACC used for the downside shock, in decimal form.
    #[getter]
    fn wacc_down(&self) -> f64 {
        self.inner.wacc_down
    }

    /// Whether the WACC downside was clamped to keep ``wacc - g`` positive.
    #[getter]
    fn wacc_down_clamped(&self) -> bool {
        self.inner.wacc_down_clamped
    }

    /// Terminal growth used for the upside shock (decimal), or ``None`` for
    /// an exit-multiple terminal.
    #[getter]
    fn terminal_growth_up(&self) -> Option<f64> {
        self.inner.terminal_growth_up
    }

    /// Whether the terminal-growth upside was clamped.
    #[getter]
    fn terminal_growth_up_clamped(&self) -> bool {
        self.inner.terminal_growth_up_clamped
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DcfSensitivityResult", &self.inner)
    }
}

wire_methods!(
    PyDcfSensitivityResult,
    DcfSensitivityResult,
    "DcfSensitivityResult"
);

impl PyDcfSensitivityResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "parameter_id": entry.parameter_id,
                    "downside": entry.downside,
                    "upside": entry.upside,
                    "swing": entry.swing(),
                })
            })
            .collect();
        serde_rows_to_arrow_table(&rows, &TORNADO_COLUMNS)
    }
}

tabular_methods!(PyDcfSensitivityResult);
