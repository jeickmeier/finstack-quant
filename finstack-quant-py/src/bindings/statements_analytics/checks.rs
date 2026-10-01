//! Python bindings for the statement check suites.
//!
//! Exposes the typed node mappings (`ThreeStatementMapping`, `CreditMapping`)
//! that drive the built-in three-statement and credit-underwriting suites, the
//! suite runners (`run_checks`, `run_three_statement_checks`,
//! `run_credit_underwriting_checks`) and the report renderers. Every entry
//! point accepts the typed object or its canonical JSON string.

use crate::bindings::extract::{extract_model_ref, extract_results_ref};
use crate::bindings::statements::checks::PyCheckReport;
use crate::bindings::statements_analytics::extract_serde_any;
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements::checks::{CheckReport, CheckSuite, CheckSuiteSpec};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements_analytics::analysis::{CreditMapping, ThreeStatementMapping};
use pyo3::prelude::*;

type NodePair = (String, Option<String>);

fn node_ids(values: Vec<String>) -> Vec<NodeId> {
    values.into_iter().map(NodeId::from).collect()
}

fn node_strings(values: &[NodeId]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn opt_node_str(value: &Option<NodeId>) -> Option<&str> {
    value.as_ref().map(NodeId::as_str)
}

/// Node-id mapping for the three-statement check suite.
///
/// Required ids name the balance-sheet articulation nodes; optional ids switch
/// on the reconciliation checks that need them (depreciation, interest, tax,
/// cash-flow subtotals, capex, dividends, working capital, debt roll-forwards).
///
/// Parameters
/// ----------
/// cash_node : str
///     Cash balance node.
/// retained_earnings_node : str
///     Retained-earnings balance node.
/// net_income_node : str
///     Net-income node.
/// assets_nodes : list[str]
///     Nodes summed to total assets.
/// liabilities_nodes : list[str]
///     Nodes summed to total liabilities.
/// equity_nodes : list[str]
///     Nodes summed to total equity.
/// ppe_node : str | None
///     Net PP&E balance node.
/// depreciation_node : str | None
///     Depreciation-expense node.
/// interest_expense_node : str | None
///     Interest-expense node.
/// tax_expense_node : str | None
///     Tax-expense node.
/// pretax_income_node : str | None
///     Pre-tax income node.
/// cfo_node : str | None
///     Cash from operations node.
/// cfi_node : str | None
///     Cash from investing node.
/// cff_node : str | None
///     Cash from financing node.
/// total_cf_node : str | None
///     Net change in cash node.
/// capex_node : str | None
///     Capital-expenditure node.
/// dividends_node : str | None
///     Dividends-paid node.
/// ppe_additions_node : str | None
///     PP&E additions node (capex reconciliation).
/// intangible_additions_node : str | None
///     Intangible additions node (capex reconciliation).
/// dividends_equity_node : str | None
///     Dividends recorded in the equity roll-forward.
/// debt_balance_nodes : list[tuple[str, str | None]]
///     ``(balance_node, rate_node)`` pairs for interest reconciliation.
/// cs_interest_node : str | None
///     Capital-structure interest node.
/// wc_change_cf_node : str | None
///     Working-capital change node on the cash-flow statement.
/// current_assets_nodes : list[str]
///     Current-asset nodes for the working-capital check.
/// current_liabilities_nodes : list[str]
///     Current-liability nodes for the working-capital check.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ThreeStatementMapping
/// >>> m = ThreeStatementMapping("cash", "retained_earnings", "net_income", ["cash"], [], ["retained_earnings"])
/// >>> m.cash_node
/// 'cash'
#[pyclass(
    name = "ThreeStatementMapping",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyThreeStatementMapping {
    pub(crate) inner: ThreeStatementMapping,
}

#[pymethods]
impl PyThreeStatementMapping {
    #[new]
    #[pyo3(signature = (
        cash_node,
        retained_earnings_node,
        net_income_node,
        assets_nodes=Vec::new(),
        liabilities_nodes=Vec::new(),
        equity_nodes=Vec::new(),
        ppe_node=None,
        depreciation_node=None,
        interest_expense_node=None,
        tax_expense_node=None,
        pretax_income_node=None,
        cfo_node=None,
        cfi_node=None,
        cff_node=None,
        total_cf_node=None,
        capex_node=None,
        dividends_node=None,
        ppe_additions_node=None,
        intangible_additions_node=None,
        dividends_equity_node=None,
        debt_balance_nodes=Vec::new(),
        cs_interest_node=None,
        wc_change_cf_node=None,
        current_assets_nodes=Vec::new(),
        current_liabilities_nodes=Vec::new(),
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        cash_node: String,
        retained_earnings_node: String,
        net_income_node: String,
        assets_nodes: Vec<String>,
        liabilities_nodes: Vec<String>,
        equity_nodes: Vec<String>,
        ppe_node: Option<String>,
        depreciation_node: Option<String>,
        interest_expense_node: Option<String>,
        tax_expense_node: Option<String>,
        pretax_income_node: Option<String>,
        cfo_node: Option<String>,
        cfi_node: Option<String>,
        cff_node: Option<String>,
        total_cf_node: Option<String>,
        capex_node: Option<String>,
        dividends_node: Option<String>,
        ppe_additions_node: Option<String>,
        intangible_additions_node: Option<String>,
        dividends_equity_node: Option<String>,
        debt_balance_nodes: Vec<NodePair>,
        cs_interest_node: Option<String>,
        wc_change_cf_node: Option<String>,
        current_assets_nodes: Vec<String>,
        current_liabilities_nodes: Vec<String>,
    ) -> Self {
        Self {
            inner: ThreeStatementMapping {
                assets_nodes: node_ids(assets_nodes),
                liabilities_nodes: node_ids(liabilities_nodes),
                equity_nodes: node_ids(equity_nodes),
                cash_node: cash_node.into(),
                retained_earnings_node: retained_earnings_node.into(),
                ppe_node: ppe_node.map(NodeId::from),
                net_income_node: net_income_node.into(),
                depreciation_node: depreciation_node.map(NodeId::from),
                interest_expense_node: interest_expense_node.map(NodeId::from),
                tax_expense_node: tax_expense_node.map(NodeId::from),
                pretax_income_node: pretax_income_node.map(NodeId::from),
                cfo_node: cfo_node.map(NodeId::from),
                cfi_node: cfi_node.map(NodeId::from),
                cff_node: cff_node.map(NodeId::from),
                total_cf_node: total_cf_node.map(NodeId::from),
                capex_node: capex_node.map(NodeId::from),
                dividends_node: dividends_node.map(NodeId::from),
                ppe_additions_node: ppe_additions_node.map(NodeId::from),
                intangible_additions_node: intangible_additions_node.map(NodeId::from),
                dividends_equity_node: dividends_equity_node.map(NodeId::from),
                debt_balance_nodes: debt_balance_nodes
                    .into_iter()
                    .map(|(balance, rate)| (NodeId::from(balance), rate.map(NodeId::from)))
                    .collect(),
                cs_interest_node: cs_interest_node.map(NodeId::from),
                wc_change_cf_node: wc_change_cf_node.map(NodeId::from),
                current_assets_nodes: node_ids(current_assets_nodes),
                current_liabilities_nodes: node_ids(current_liabilities_nodes),
            },
        }
    }

    /// Cash balance node.
    #[getter]
    fn cash_node(&self) -> &str {
        self.inner.cash_node.as_str()
    }

    /// Retained-earnings balance node.
    #[getter]
    fn retained_earnings_node(&self) -> &str {
        self.inner.retained_earnings_node.as_str()
    }

    /// Net-income node.
    #[getter]
    fn net_income_node(&self) -> &str {
        self.inner.net_income_node.as_str()
    }

    /// Nodes summed to total assets.
    #[getter]
    fn assets_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.assets_nodes)
    }

    /// Nodes summed to total liabilities.
    #[getter]
    fn liabilities_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.liabilities_nodes)
    }

    /// Nodes summed to total equity.
    #[getter]
    fn equity_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.equity_nodes)
    }

    /// Net PP&E node, or ``None``.
    #[getter]
    fn ppe_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.ppe_node)
    }

    /// Depreciation-expense node, or ``None``.
    #[getter]
    fn depreciation_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.depreciation_node)
    }

    /// Interest-expense node, or ``None``.
    #[getter]
    fn interest_expense_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.interest_expense_node)
    }

    /// Tax-expense node, or ``None``.
    #[getter]
    fn tax_expense_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.tax_expense_node)
    }

    /// Pre-tax income node, or ``None``.
    #[getter]
    fn pretax_income_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.pretax_income_node)
    }

    /// Cash from operations node, or ``None``.
    #[getter]
    fn cfo_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cfo_node)
    }

    /// Cash from investing node, or ``None``.
    #[getter]
    fn cfi_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cfi_node)
    }

    /// Cash from financing node, or ``None``.
    #[getter]
    fn cff_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cff_node)
    }

    /// Net change in cash node, or ``None``.
    #[getter]
    fn total_cf_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.total_cf_node)
    }

    /// Capital-expenditure node, or ``None``.
    #[getter]
    fn capex_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.capex_node)
    }

    /// Dividends-paid node, or ``None``.
    #[getter]
    fn dividends_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.dividends_node)
    }

    /// PP&E additions node, or ``None``.
    #[getter]
    fn ppe_additions_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.ppe_additions_node)
    }

    /// Intangible additions node, or ``None``.
    #[getter]
    fn intangible_additions_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.intangible_additions_node)
    }

    /// Dividends node in the equity roll-forward, or ``None``.
    #[getter]
    fn dividends_equity_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.dividends_equity_node)
    }

    /// ``(balance_node, rate_node)`` pairs for interest reconciliation.
    #[getter]
    fn debt_balance_nodes(&self) -> Vec<NodePair> {
        self.inner
            .debt_balance_nodes
            .iter()
            .map(|(balance, rate)| (balance.to_string(), rate.as_ref().map(ToString::to_string)))
            .collect()
    }

    /// Capital-structure interest node, or ``None``.
    #[getter]
    fn cs_interest_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cs_interest_node)
    }

    /// Working-capital change node on the cash-flow statement, or ``None``.
    #[getter]
    fn wc_change_cf_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.wc_change_cf_node)
    }

    /// Current-asset nodes.
    #[getter]
    fn current_assets_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.current_assets_nodes)
    }

    /// Current-liability nodes.
    #[getter]
    fn current_liabilities_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.current_liabilities_nodes)
    }

    /// Every node id referenced by the mapping.
    #[getter]
    fn all_nodes(&self) -> Vec<String> {
        node_strings(&self.inner.all_nodes())
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ThreeStatementMapping"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``ThreeStatementMapping`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ThreeStatementMapping JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ThreeStatementMapping", &self.inner)
    }
}

/// Node-id mapping for the credit-underwriting check suite.
///
/// Parameters
/// ----------
/// debt_node : str
///     Total-debt node.
/// ebitda_node : str
///     EBITDA node.
/// interest_expense_node : str
///     Interest-expense node.
/// fcf_node : str | None
///     Free-cash-flow node (enables the FCF sign check).
/// cash_node : str | None
///     Cash balance node (enables the liquidity check).
/// cash_burn_node : str | None
///     Cash-burn node (liquidity runway).
/// leverage_warn : tuple[float, float] | None
///     ``(warn, error)`` debt/EBITDA thresholds in turns.
/// coverage_min_warn : float | None
///     Minimum EBITDA/interest coverage in turns before a warning.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CreditMapping
/// >>> CreditMapping("total_debt", "ebitda", "interest_expense", leverage_warn=(4.0, 6.0)).leverage_warn
/// (4.0, 6.0)
#[pyclass(
    name = "CreditMapping",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyCreditMapping {
    pub(crate) inner: CreditMapping,
}

#[pymethods]
impl PyCreditMapping {
    #[new]
    #[pyo3(signature = (
        debt_node,
        ebitda_node,
        interest_expense_node,
        fcf_node=None,
        cash_node=None,
        cash_burn_node=None,
        leverage_warn=None,
        coverage_min_warn=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        debt_node: String,
        ebitda_node: String,
        interest_expense_node: String,
        fcf_node: Option<String>,
        cash_node: Option<String>,
        cash_burn_node: Option<String>,
        leverage_warn: Option<(f64, f64)>,
        coverage_min_warn: Option<f64>,
    ) -> Self {
        Self {
            inner: CreditMapping {
                debt_node: debt_node.into(),
                ebitda_node: ebitda_node.into(),
                interest_expense_node: interest_expense_node.into(),
                fcf_node: fcf_node.map(NodeId::from),
                cash_node: cash_node.map(NodeId::from),
                cash_burn_node: cash_burn_node.map(NodeId::from),
                leverage_warn,
                coverage_min_warn,
            },
        }
    }

    /// Total-debt node.
    #[getter]
    fn debt_node(&self) -> &str {
        self.inner.debt_node.as_str()
    }

    /// EBITDA node.
    #[getter]
    fn ebitda_node(&self) -> &str {
        self.inner.ebitda_node.as_str()
    }

    /// Interest-expense node.
    #[getter]
    fn interest_expense_node(&self) -> &str {
        self.inner.interest_expense_node.as_str()
    }

    /// Free-cash-flow node, or ``None``.
    #[getter]
    fn fcf_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.fcf_node)
    }

    /// Cash balance node, or ``None``.
    #[getter]
    fn cash_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cash_node)
    }

    /// Cash-burn node, or ``None``.
    #[getter]
    fn cash_burn_node(&self) -> Option<&str> {
        opt_node_str(&self.inner.cash_burn_node)
    }

    /// ``(warn, error)`` leverage thresholds in turns, or ``None``.
    #[getter]
    fn leverage_warn(&self) -> Option<(f64, f64)> {
        self.inner.leverage_warn
    }

    /// Minimum coverage in turns before a warning, or ``None``.
    #[getter]
    fn coverage_min_warn(&self) -> Option<f64> {
        self.inner.coverage_min_warn
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "CreditMapping"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``CreditMapping`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CreditMapping JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditMapping", &self.inner)
    }
}

/// Extract a `ThreeStatementMapping` from the typed object, a dict, or JSON.
pub(crate) fn extract_three_statement_mapping(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<ThreeStatementMapping> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyThreeStatementMapping>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "ThreeStatementMapping")
}

/// Extract a `CreditMapping` from the typed object, a dict, or JSON.
pub(crate) fn extract_credit_mapping(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CreditMapping> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyCreditMapping>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "CreditMapping")
}

/// Extract a `CheckSuiteSpec` from the typed `finstack_quant.statements.CheckSuiteSpec`,
/// a dict, or JSON. The typed wrapper is read through its own ``to_json`` so
/// this module never touches the statements binding internals.
pub(crate) fn extract_check_suite_spec(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CheckSuiteSpec> {
    if obj
        .get_type()
        .name()
        .map(|name| name == "CheckSuiteSpec")
        .unwrap_or(false)
    {
        let json: String = obj.call_method0("to_json")?.extract()?;
        return serde_json::from_str(&json)
            .map_err(|e| serde_json_to_py(e, "invalid CheckSuiteSpec JSON"));
    }
    extract_serde_any(py, obj, "CheckSuiteSpec")
}

/// Extract a `CheckReport` from the typed wrapper, a dict, or JSON.
pub(crate) fn extract_check_report(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<CheckReport> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyCheckReport>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "CheckReport")
}

fn run_check_suite(
    py: Python<'_>,
    model: finstack_quant_statements::FinancialModelSpec,
    suite: CheckSuite,
    results: Option<finstack_quant_statements::evaluator::StatementResult>,
) -> PyResult<PyCheckReport> {
    py.detach(move || {
        suite
            .run_model(&model, results.as_ref())
            .map(PyCheckReport::from_inner)
            .map_err(statements_to_py)
    })
}

fn optional_results(
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_statements::evaluator::StatementResult>> {
    results
        .map(extract_results_ref)
        .transpose()
        .map(|results| results.map(|results| results.into_owned()))
}

/// Run checks from a suite spec against a model.
///
/// Resolves both built-in and formula checks from the spec, evaluates the
/// model (unless ``results`` is supplied), and returns a full check report.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// spec : CheckSuiteSpec | dict | str
///     Typed ``finstack_quant.statements.CheckSuiteSpec``, its serde dict, or
///     JSON string.
/// results : StatementResult | str | None
///     Pre-computed evaluation results; when provided the model is not
///     re-evaluated. Accounting identity checks require supplied node-unit
///     metadata to agree with the model's declarations or explicit monetary values.
///
/// Returns
/// -------
/// CheckReport
///     Typed report with summary, findings, JSON, and DataFrame accessors.
///
/// Raises
/// ------
/// ValueError
///     If a model, spec, or results payload is malformed, a check configuration
///     is invalid, formula parsing or evaluation fails, accounting operands
///     have incompatible currencies/units, or supplied result units conflict
///     with the model's declarations or explicit monetary values.
/// KeyError
///     If a check references a node missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import run_checks
/// >>> b = ModelBuilder("m"); b.periods("2024Q1..Q2", None); b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
/// >>> spec = {"name": "s", "builtin_checks": [], "formula_checks": [{"id": "pos", "name": "positive",
/// ...   "category": "internal_consistency", "severity": "error", "formula": "revenue > 0",
/// ...   "message_template": "bad {period}"}]}
/// >>> run_checks(b.build(), spec).passed
/// True
#[pyfunction]
#[pyo3(signature = (model, spec, results=None))]
fn run_checks(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    spec: &Bound<'_, PyAny>,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCheckReport> {
    let model = extract_model_ref(model)?.into_owned();
    let spec = extract_check_suite_spec(py, spec)?;
    let suite = spec.resolve().map_err(statements_to_py)?;
    let results = optional_results(results)?;
    run_check_suite(py, model, suite, results)
}

/// Run the built-in three-statement check suite.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// mapping : ThreeStatementMapping | dict | str
///     Typed node mapping, its serde dict, or JSON string.
/// results : StatementResult | str | None
///     Pre-computed evaluation results; skips re-evaluation when provided.
///     Supplied accounting-node units must agree with the model's declarations
///     or explicit monetary values.
///
/// Returns
/// -------
/// CheckReport
///     Typed report with summary, findings, JSON, and DataFrame accessors.
///
/// Raises
/// ------
/// ValueError
///     If a model, mapping, or results payload is malformed, evaluation fails,
///     accounting operands have incompatible currencies/units, or supplied
///     result units conflict with model declarations or explicit monetary values.
/// KeyError
///     If a mapped node is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import ThreeStatementMapping, run_three_statement_checks
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("cash", [("2025Q1", 100.0)])
/// >>> _ = b.value("debt", [("2025Q1", 40.0)])
/// >>> _ = b.value("equity", [("2025Q1", 60.0)])
/// >>> _ = b.value("retained_earnings", [("2025Q1", 60.0)])
/// >>> _ = b.value("net_income", [("2025Q1", 0.0)])
/// >>> mapping = ThreeStatementMapping("cash", "retained_earnings", "net_income", ["cash"], ["debt"], ["equity"])
/// >>> run_three_statement_checks(b.build(), mapping).passed
/// True
#[pyfunction]
#[pyo3(signature = (model, mapping, results=None))]
fn run_three_statement_checks(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    mapping: &Bound<'_, PyAny>,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCheckReport> {
    let model = extract_model_ref(model)?.into_owned();
    let mapping = extract_three_statement_mapping(py, mapping)?;
    let suite = finstack_quant_statements_analytics::analysis::three_statement_checks(mapping);
    let results = optional_results(results)?;
    run_check_suite(py, model, suite, results)
}

/// Run the built-in credit-underwriting check suite.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// mapping : CreditMapping | dict | str
///     Typed node mapping, its serde dict, or JSON string.
/// results : StatementResult | str | None
///     Pre-computed evaluation results; skips re-evaluation when provided.
///
/// Returns
/// -------
/// CheckReport
///     Typed report with summary, findings, JSON, and DataFrame accessors.
///
/// Raises
/// ------
/// ValueError
///     If the mapping is malformed or the evaluation fails.
/// KeyError
///     If a mapped node is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import CreditMapping, run_credit_underwriting_checks
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("total_debt", [("2025Q1", 100.0)])
/// >>> _ = b.value("ebitda", [("2025Q1", 40.0)])
/// >>> _ = b.value("interest_expense", [("2025Q1", 10.0)])
/// >>> report = run_credit_underwriting_checks(b.build(), CreditMapping("total_debt", "ebitda", "interest_expense"))
/// >>> report.passed, report.total_findings
/// (True, 0)
#[pyfunction]
#[pyo3(signature = (model, mapping, results=None))]
fn run_credit_underwriting_checks(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    mapping: &Bound<'_, PyAny>,
    results: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyCheckReport> {
    let model = extract_model_ref(model)?.into_owned();
    let mapping = extract_credit_mapping(py, mapping)?;
    let suite = finstack_quant_statements_analytics::analysis::credit_underwriting_checks(mapping);
    let results = optional_results(results)?;
    run_check_suite(py, model, suite, results)
}

/// Render a check report as plain text.
///
/// Parameters
/// ----------
/// report : CheckReport | dict | str
///     Typed report, its serde dict, or JSON string.
///
/// Returns
/// -------
/// str
///     Human-readable plain-text report.
///
/// Raises
/// ------
/// ValueError
///     If ``report`` is not a valid ``CheckReport`` payload.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import CreditMapping, render_check_report_text, run_credit_underwriting_checks
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("total_debt", [("2025Q1", 100.0)])
/// >>> _ = b.value("ebitda", [("2025Q1", 40.0)])
/// >>> _ = b.value("interest_expense", [("2025Q1", 10.0)])
/// >>> report = run_credit_underwriting_checks(b.build(), CreditMapping("total_debt", "ebitda", "interest_expense"))
/// >>> render_check_report_text(report).splitlines()[0]
/// '══ Check Report ══'
#[pyfunction]
fn render_check_report_text(py: Python<'_>, report: &Bound<'_, PyAny>) -> PyResult<String> {
    let report = extract_check_report(py, report)?;
    Ok(finstack_quant_statements_analytics::analysis::CheckReportRenderer::render_text(&report))
}

/// Render a check report as HTML with inline styles.
///
/// Parameters
/// ----------
/// report : CheckReport | dict | str
///     Typed report, its serde dict, or JSON string.
///
/// Returns
/// -------
/// str
///     HTML-formatted report suitable for Jupyter notebooks.
///
/// Raises
/// ------
/// ValueError
///     If ``report`` is not a valid ``CheckReport`` payload.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import CreditMapping, render_check_report_html, run_credit_underwriting_checks
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..Q1", None)
/// >>> _ = b.value("total_debt", [("2025Q1", 100.0)])
/// >>> _ = b.value("ebitda", [("2025Q1", 40.0)])
/// >>> _ = b.value("interest_expense", [("2025Q1", 10.0)])
/// >>> report = run_credit_underwriting_checks(b.build(), CreditMapping("total_debt", "ebitda", "interest_expense"))
/// >>> render_check_report_html(report).startswith("<div")
/// True
#[pyfunction]
fn render_check_report_html(py: Python<'_>, report: &Bound<'_, PyAny>) -> PyResult<String> {
    let report = extract_check_report(py, report)?;
    Ok(finstack_quant_statements_analytics::analysis::CheckReportRenderer::render_html(&report))
}

/// Register check mappings and runners.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyThreeStatementMapping>()?;
    m.add_class::<PyCreditMapping>()?;
    m.add_function(pyo3::wrap_pyfunction!(run_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_three_statement_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_credit_underwriting_checks, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(render_check_report_text, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(render_check_report_html, m)?)?;
    Ok(())
}
