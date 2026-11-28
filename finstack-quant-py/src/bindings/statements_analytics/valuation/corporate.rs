//! LBO check mappings, WACC, and orchestrated corporate analysis Python bindings.

use super::inputs::{PyDcfInputs, extract_dcf_options, extract_terminal_value};
use super::sensitivity::{PyCorporateValuationResult, PyDcfSensitivityResult};
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::statements_analytics::checks::{
    PyCreditMapping, PyThreeStatementMapping, extract_credit_mapping,
    extract_three_statement_mapping,
};
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis::{
    CorporateAnalysis, CorporateAnalysisOptions, DEFAULT_UFCF_NODE, LboCheckMappings, WaccInputs,
};
use pyo3::prelude::*;

/// Column schema for [`PyCorporateAnalysis`] tabular exports.
const CREDIT_COLUMNS: [(&str, &str); 6] = [
    ("instrument_id", "str"),
    ("period", "str"),
    ("dscr", "float64"),
    ("dscr_total", "float64"),
    ("dscr_incl_fees", "float64"),
    ("interest_coverage", "float64"),
];

/// Node mappings that switch on the LBO model check suite.
///
/// Parameters
/// ----------
/// three_statement : ThreeStatementMapping | dict
///     Balance-sheet / income / cash-flow node mapping.
/// credit : CreditMapping | dict
///     Leverage and coverage node mapping.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CreditMapping, LboCheckMappings, ThreeStatementMapping
/// >>> three = ThreeStatementMapping(
/// ...     cash_node="cash",
/// ...     retained_earnings_node="re",
/// ...     net_income_node="ni",
/// ...     assets_nodes=["assets"],
/// ...     liabilities_nodes=["liabilities"],
/// ...     equity_nodes=["equity"],
/// ... )
/// >>> credit = CreditMapping(debt_node="debt", ebitda_node="ebitda", interest_expense_node="interest")
/// >>> m = LboCheckMappings(three, credit)
/// >>> m.credit.debt_node
/// 'debt'
#[pyclass(
    name = "LboCheckMappings",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyLboCheckMappings {
    pub(crate) inner: LboCheckMappings,
}

#[pymethods]
impl PyLboCheckMappings {
    #[new]
    fn new(
        py: Python<'_>,
        three_statement: &Bound<'_, PyAny>,
        credit: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: LboCheckMappings {
                three_statement: extract_three_statement_mapping(py, three_statement)?,
                credit: extract_credit_mapping(py, credit)?,
            },
        })
    }

    /// Three-statement node mapping.
    #[getter]
    fn three_statement(&self) -> PyThreeStatementMapping {
        PyThreeStatementMapping {
            inner: self.inner.three_statement.clone(),
        }
    }

    /// Credit node mapping.
    #[getter]
    fn credit(&self) -> PyCreditMapping {
        PyCreditMapping {
            inner: self.inner.credit.clone(),
        }
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LboCheckMappings", &self.inner)
    }
}

wire_methods!(PyLboCheckMappings, LboCheckMappings, "LboCheckMappings");

/// Orchestrated statement + equity + credit analysis envelope.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import run_corporate_analysis
/// >>> b = ModelBuilder("m"); b.periods("2024Q1..Q2", None); b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
/// >>> analysis = run_corporate_analysis(b.build())
/// >>> analysis.equity is None
/// True
#[pyclass(
    name = "CorporateAnalysis",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCorporateAnalysis {
    pub(crate) inner: CorporateAnalysis,
}

#[pymethods]
impl PyCorporateAnalysis {
    /// Full statement evaluation.
    #[getter]
    fn statement(&self) -> PyStatementResult {
        PyStatementResult {
            inner: self.inner.statement.clone(),
        }
    }

    /// DCF valuation, or ``None`` when no ``wacc`` was configured.
    #[getter]
    fn equity(&self) -> Option<PyCorporateValuationResult> {
        self.inner
            .equity
            .clone()
            .map(|inner| PyCorporateValuationResult { inner })
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "CorporateAnalysis.credit")]
    fn credit(
        &self,
    ) -> std::collections::HashMap<String, super::super::credit_context::PyCreditContextMetrics>
    {
        self.inner
            .credit
            .iter()
            .map(|(id, metrics)| {
                (
                    id.clone(),
                    super::super::credit_context::PyCreditContextMetrics {
                        inner: metrics.clone(),
                    },
                )
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "CorporateAnalysis(statement_nodes={}, equity={}, credit_instruments={})",
            self.inner.statement.cells.len(),
            if self.inner.equity.is_some() {
                "CorporateValuationResult(...)"
            } else {
                "None"
            },
            self.inner.credit.len()
        )
    }
}

wire_methods!(PyCorporateAnalysis, CorporateAnalysis, "CorporateAnalysis");

impl PyCorporateAnalysis {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut rows = Vec::new();
        for (instrument_id, metrics) in &self.inner.credit {
            for (period, values) in super::super::credit_context::observations(metrics) {
                if values[..4].iter().all(Option::is_none) {
                    continue;
                }
                rows.push(serde_json::json!({
                    "instrument_id": instrument_id,
                    "period": period.to_string(),
                    "dscr": values[0],
                    "dscr_total": values[1],
                    "dscr_incl_fees": values[2],
                    "interest_coverage": values[3],
                }));
            }
        }
        serde_rows_to_arrow_table(&rows, &CREDIT_COLUMNS)
    }
}

tabular_methods!(PyCorporateAnalysis);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "evaluate_dcf")]
#[pyfunction]
#[pyo3(signature = (model, inputs, options=None, market=None, as_of=None))]
pub(super) fn evaluate_dcf<'py>(
    py: Python<'py>,
    model: PyRef<'py, PyFinancialModelSpec>,
    inputs: &Bound<'py, PyAny>,
    options: Option<&Bound<'py, PyAny>>,
    market: Option<PyRef<'py, PyMarketContext>>,
    as_of: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyCorporateValuationResult> {
    let model = model.inner.clone();
    let inputs = match inputs.extract::<PyRef<'_, PyDcfInputs>>() {
        Ok(inputs) => inputs.inner.clone(),
        Err(_) => crate::bindings::module_utils::py_to_serde(py, inputs, "DcfInputs")?,
    };
    let options = extract_dcf_options(py, options)?;
    let market = market.map(|market| market.inner.clone());
    let as_of = as_of
        .map(crate::bindings::date_utils::py_to_date)
        .transpose()?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_dcf(
                &model,
                &inputs,
                &options,
                market.as_ref(),
                as_of,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyCorporateValuationResult { inner })
}

/// Rank the headline DCF assumptions by enterprise-value impact.
///
/// The statement model is evaluated once; each shocked point re-runs only the
/// DCF. Entries are EV deltas versus the baseline, sorted by descending
/// absolute swing.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec
///     A ``FinancialModelSpec``; metadata must include
///     a ``"currency"`` key.
/// wacc : float
///     Baseline WACC in decimal form.
/// terminal_value : TerminalValueSpec | dict
///     Terminal value method; selects whether the growth rate or the exit
///     multiple is shocked.
/// ufcf_node : str
///     Node id containing monetary unlevered free cash flow in model currency.
///     Defaults to the Rust ``DEFAULT_UFCF_NODE`` (``"ufcf"``). Gordon Growth /
///     H-Model terminal cash flow requires a complete contiguous calendar year
///     at the end of the forecast.
/// net_debt_override : float | None
///     Flat net-debt amount used instead of the model-derived bridge.
/// options : dict | None
///     The Rust ``DcfOptions`` serde form (the same document WASM
///     ``dcfSensitivity`` takes); every key is optional and a missing one
///     takes its Rust default: ``wacc_sensitivity_bump`` (``0.01`` = +/-100bp
///     on WACC and terminal growth), ``wacc_denominator_epsilon``
///     (``0.005``), ``max_stable_growth_rate`` (``0.05``),
///     ``exit_multiple_bump`` (``{"absolute": 1.0}`` turns or
///     ``{"relative": 0.10}``), ``mid_year_convention`` (``False``),
///     ``exit_multiple_metric_node`` (statement monetary flow node supplying
///     the complete trailing-year terminal metric), ``equity_bridge``,
///     ``shares_outstanding`` and ``valuation_discounts``. Unknown keys raise
///     ``ValueError``.
///
/// Returns
/// -------
/// DcfSensitivityResult
///     Baseline EV, ranked ``entries`` and the clamped-shock flags;
///     ``to_pandas()`` or ``to_arrow()`` gives the tornado table.
///
/// Raises
/// ------
/// ValueError
///     If a payload or ``options`` is malformed, or the model or DCF inputs
///     are invalid (including an exit-multiple node with insufficient or
///     noncontiguous history).
/// KeyError
///     If ``ufcf_node`` or ``exit_multiple_metric_node`` is missing.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import TerminalValueSpec, dcf_sensitivity
/// >>> b = ModelBuilder("dcf"); b.periods("2025..2026")
/// >>> b.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
/// >>> b.with_meta("currency", '"USD"')
/// >>> sens = dcf_sensitivity(b.build(), 0.10, TerminalValueSpec.gordon_growth(0.02), net_debt_override=0.0)
/// >>> list(sens.to_pandas().columns)
/// ['parameter_id', 'downside', 'upside', 'swing']
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc,
    terminal_value,
    ufcf_node=DEFAULT_UFCF_NODE,
    net_debt_override=None,
    options=None,
))]
pub(super) fn dcf_sensitivity<'py>(
    py: Python<'py>,
    model: PyRef<'py, PyFinancialModelSpec>,
    wacc: f64,
    terminal_value: &Bound<'py, PyAny>,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    options: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyDcfSensitivityResult> {
    let model = model.inner.clone();
    let terminal_value = extract_terminal_value(py, terminal_value)?;
    let ufcf_node = ufcf_node.to_owned();
    let options = extract_dcf_options(py, options)?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::dcf_sensitivity(
                &model,
                wacc,
                terminal_value,
                &ufcf_node,
                net_debt_override,
                &options,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyDcfSensitivityResult { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs")]
#[pyclass(
    name = "WaccInputs",
    module = "finstack_quant.statements_analytics",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyWaccInputs {
    pub(crate) inner: WaccInputs,
}

#[pymethods]
impl PyWaccInputs {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.__init__")]
    #[new]
    #[pyo3(signature = (*, equity_weight, cost_of_equity, debt_weight, cost_of_debt, tax_rate))]
    fn new(
        equity_weight: &Bound<'_, PyAny>,
        cost_of_equity: f64,
        debt_weight: &Bound<'_, PyAny>,
        cost_of_debt: f64,
        tax_rate: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: WaccInputs {
                equity_weight: finstack_quant_core::wire::DecimalWire(
                    crate::bindings::core::money::extract_exact_decimal(
                        equity_weight,
                        "equity_weight must be an exact decimal",
                    )?,
                ),
                cost_of_equity,
                debt_weight: finstack_quant_core::wire::DecimalWire(
                    crate::bindings::core::money::extract_exact_decimal(
                        debt_weight,
                        "debt_weight must be an exact decimal",
                    )?,
                ),
                cost_of_debt,
                tax_rate,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.equity_weight")]
    #[getter]
    fn equity_weight<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.equity_weight.0)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.cost_of_equity")]
    #[getter]
    fn cost_of_equity(&self) -> f64 {
        self.inner.cost_of_equity
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.debt_weight")]
    #[getter]
    fn debt_weight<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.debt_weight.0)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.cost_of_debt")]
    #[getter]
    fn cost_of_debt(&self) -> f64 {
        self.inner.cost_of_debt
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "WaccInputs.tax_rate")]
    #[getter]
    fn tax_rate(&self) -> f64 {
        self.inner.tax_rate
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WaccInputs", &self.inner)
    }
}

wire_methods!(PyWaccInputs, WaccInputs, "WaccInputs");

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "wacc")]
#[pyfunction]
#[pyo3(signature = (inputs))]
pub(super) fn wacc(inputs: PyWaccInputs) -> PyResult<f64> {
    finstack_quant_statements_analytics::analysis::wacc(&inputs.inner).map_err(statements_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.corporate", "run_corporate_analysis")]
#[pyfunction]
#[pyo3(signature = (model, options=None, market=None))]
pub(super) fn run_corporate_analysis<'py>(
    py: Python<'py>,
    model: PyRef<'py, PyFinancialModelSpec>,
    options: Option<&Bound<'py, PyAny>>,
    market: Option<PyRef<'py, PyMarketContext>>,
) -> PyResult<PyCorporateAnalysis> {
    let model = model.inner.clone();
    let options: CorporateAnalysisOptions =
        crate::bindings::module_utils::opt_py_to_serde(py, options, "CorporateAnalysisOptions")?
            .unwrap_or_default();
    let market = market.map(|market| market.inner.clone());

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::run_corporate_analysis(
                model, options, market,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyCorporateAnalysis { inner })
}
