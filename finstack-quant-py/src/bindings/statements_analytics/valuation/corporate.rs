//! LBO, WACC, and orchestrated corporate analysis Python bindings.

use super::inputs::{PyDcfInputs, extract_dcf_options, extract_terminal_value};
use super::sensitivity::{PyCorporateValuationResult, PyDcfSensitivityResult};
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::statements::checks::PyCheckReport;
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::statements_analytics::checks::{
    PyCreditMapping, PyThreeStatementMapping, extract_credit_mapping,
    extract_three_statement_mapping,
};
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis::{
    CorporateAnalysis, CorporateAnalysisOptions, DEFAULT_UFCF_NODE, LboCheckMappings, LboConfig,
    LboResult, WaccInputs,
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

/// Outputs of an LBO evaluation in the model currency.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import LboResult
/// >>> m = '{"amount":"100","currency":"USD"}'
/// >>> r = LboResult.from_json('{"entry_enterprise_value":' + m + ',"entry_metric":10.0,"debt_total":' + m
/// ...     + ',"equity_check":' + m + ',"sources_total":' + m + ',"uses_total":' + m + ',"sources_uses_balanced":true,'
/// ...     + '"exit_enterprise_value":' + m + ',"exit_metric":12.0,"exit_net_debt":' + m + ',"exit_equity_proceeds":' + m
/// ...     + ',"moic":2.0,"checks":null}')
/// >>> r.moic
/// 2.0
#[pyclass(
    name = "LboResult",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyLboResult {
    pub(crate) inner: LboResult,
}

#[pymethods]
impl PyLboResult {
    /// Entry enterprise value (``entry_multiple * entry_metric``).
    #[getter]
    fn entry_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.entry_enterprise_value)
    }

    /// Entry metric read from the explicitly selected entry period.
    #[getter]
    fn entry_metric(&self) -> f64 {
        self.inner.entry_metric
    }

    /// Total funded debt at close.
    #[getter]
    fn debt_total(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.debt_total)
    }

    /// Sponsor equity check (sources-and-uses residual).
    #[getter]
    fn equity_check(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.equity_check)
    }

    /// Total sources at close.
    #[getter]
    fn sources_total(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.sources_total)
    }

    /// Total uses at close (entry EV plus fees).
    #[getter]
    fn uses_total(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.uses_total)
    }

    /// Whether sources equal uses within tolerance.
    #[getter]
    fn sources_uses_balanced(&self) -> bool {
        self.inner.sources_uses_balanced
    }

    /// Exit enterprise value.
    #[getter]
    fn exit_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.exit_enterprise_value)
    }

    /// Exit metric read at ``exit_period``.
    #[getter]
    fn exit_metric(&self) -> f64 {
        self.inner.exit_metric
    }

    /// Net debt at exit.
    #[getter]
    fn exit_net_debt(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.exit_net_debt)
    }

    /// Equity proceeds at exit.
    #[getter]
    fn exit_equity_proceeds(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.exit_equity_proceeds)
    }

    /// Multiple of invested capital (``2.4`` = 2.4x).
    #[getter]
    fn moic(&self) -> f64 {
        self.inner.moic
    }

    /// LBO model check report, or ``None`` when no ``check_mappings`` were
    /// supplied.
    #[getter]
    fn checks(&self) -> Option<PyCheckReport> {
        self.inner.checks.clone().map(PyCheckReport::from_inner)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LboResult", &self.inner)
    }
}

wire_methods!(PyLboResult, LboResult, "LboResult");

impl PyLboResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 13] = [
            ("currency", "str"),
            ("entry_enterprise_value", "float64"),
            ("entry_metric", "float64"),
            ("debt_total", "float64"),
            ("equity_check", "float64"),
            ("sources_total", "float64"),
            ("uses_total", "float64"),
            ("sources_uses_balanced", "bool"),
            ("exit_enterprise_value", "float64"),
            ("exit_metric", "float64"),
            ("exit_net_debt", "float64"),
            ("exit_equity_proceeds", "float64"),
            ("moic", "float64"),
        ];
        let row = serde_json::json!({
            "currency": self.inner.entry_enterprise_value.currency().to_string(),
            "entry_enterprise_value": self.inner.entry_enterprise_value.amount(),
            "entry_metric": self.inner.entry_metric,
            "debt_total": self.inner.debt_total.amount(),
            "equity_check": self.inner.equity_check.amount(),
            "sources_total": self.inner.sources_total.amount(),
            "uses_total": self.inner.uses_total.amount(),
            "sources_uses_balanced": self.inner.sources_uses_balanced,
            "exit_enterprise_value": self.inner.exit_enterprise_value.amount(),
            "exit_metric": self.inner.exit_metric,
            "exit_net_debt": self.inner.exit_net_debt.amount(),
            "exit_equity_proceeds": self.inner.exit_equity_proceeds.amount(),
            "moic": self.inner.moic,
        });
        serde_rows_to_arrow_table(&[row], &COLUMNS)
    }
}

tabular_methods!(PyLboResult);

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

/// Evaluate a leveraged-buyout transaction against a statement model.
///
/// Entry enterprise value is priced at the explicitly selected entry period, the sponsor
/// equity check is the sources-and-uses residual, and exit proceeds are the
/// exit enterprise value less modelled net debt at ``exit_period``. IRR is out
/// of scope: pair ``exit_equity_proceeds`` with the equity outflow at close and
/// call ``finstack_quant.portfolio.mwr_xirr``.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec
///     A ``FinancialModelSpec``; metadata must include
///     a ``"currency"`` key.
/// config : dict
///     The Rust ``LboConfig`` serde form (the same document WASM
///     ``evaluateLbo`` takes): ``entry_multiple`` (turns, ``8.5`` = 8.5x),
///     ``entry_metric_node``, ``transaction_fees`` (model currency),
///     ``sources`` (``[{"name": ..., "amount": ...}]`` funded at close, model
///     currency), ``exit_multiple``, ``exit_metric_node``,
///     ``exit_net_debt_node``, ``exit_period`` (``"2029"`` or ``"2029Q4"``)
///     and the optional ``check_mappings`` (a serde dict or a typed
///     ``LboCheckMappings``), which runs the LBO model check suite against the
///     same evaluation and populates ``LboResult.checks``. Every key except
///     ``check_mappings`` is required; unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// LboResult
///     Money outputs in the model currency, ``moic`` as a scalar, and
///     ``checks`` (``CheckReport`` or ``None``).
///
/// Raises
/// ------
/// ValueError
///     If ``config`` is malformed, a tranche amount is invalid,
///     ``exit_period`` does not parse, sources and uses cannot balance, or
///     the model fails to evaluate.
/// KeyError
///     If a metric or net-debt node is missing at the required period.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import evaluate_lbo
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..2026Q1", None)
/// >>> _ = b.with_meta("currency", '"USD"')
/// >>> quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4", "2026Q1"]
/// >>> _ = b.value_money("ebitda", [(q, Money(v, "USD")) for q, v in zip(quarters, [100.0] * 4 + [120.0])])
/// >>> _ = b.value_money("net_debt", [(q, Money(v, "USD")) for q, v in zip(quarters, [300.0] * 4 + [200.0])])
/// >>> config = {"entry_multiple": 8.0, "entry_metric_node": "ebitda", "entry_period": "2025Q1", "transaction_fees": 0.0,
/// ...     "sources": [{"name": "debt", "amount": 500.0}], "exit_multiple": 9.0,
/// ...     "exit_metric_node": "ebitda", "exit_net_debt_node": "net_debt", "exit_period": "2026Q1"}
/// >>> round(evaluate_lbo(b.build(), config).moic, 4)
/// 2.9333
#[pyfunction]
#[pyo3(signature = (model, config))]
pub(super) fn evaluate_lbo<'py>(
    py: Python<'py>,
    model: PyRef<'py, PyFinancialModelSpec>,
    config: &Bound<'py, PyAny>,
) -> PyResult<PyLboResult> {
    let model = model.inner.clone();
    let config: LboConfig = crate::bindings::module_utils::py_to_serde(py, config, "config")?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_lbo(&model, &config)
        })
        .map_err(statements_to_py)?;
    Ok(PyLboResult { inner })
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
