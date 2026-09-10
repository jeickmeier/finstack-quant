//! Python bindings for DCF, LBO and the orchestrated corporate analysis.
//!
//! Typed inputs (`TerminalValueSpec`, `EquityBridge`, `ValuationDiscounts`,
//! `LboCheckMappings`) and typed results (`CorporateValuationResult`,
//! `DcfSensitivityResult`, `LboResult`, `CorporateAnalysis`) wrap the Rust
//! types one-to-one. Every entry point also accepts the canonical JSON string
//! where a typed input is taken.

use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_opt, extract_model_ref};
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py, ColumnSchema,
};
use crate::bindings::statements::checks::PyCheckReport;
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements_analytics::checks::{
    extract_check_suite_spec, extract_credit_mapping, extract_three_statement_mapping,
    PyCreditMapping, PyThreeStatementMapping,
};
use crate::bindings::statements_analytics::extract_serde_any;
use crate::bindings::statements_analytics::typed::PyTornadoEntry;
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::{
    CorporateAnalysis, CorporateValuationResult, DcfOptions, DcfSensitivityResult,
    ExitMultipleBump, LboCheckMappings, LboConfig, LboResult, LboTranche,
};
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    EquityBridge, TerminalValueSpec, ValuationDiscounts,
};
use pyo3::prelude::*;

/// Column schema for `DcfSensitivityResult.to_dataframe`.
const TORNADO_COLUMNS: [ColumnSchema<'static>; 4] = [
    ("parameter_id", "str"),
    ("downside", "float64"),
    ("upside", "float64"),
    ("swing", "float64"),
];

/// Column schema for `CorporateAnalysis.to_dataframe`.
const CREDIT_COLUMNS: [ColumnSchema<'static>; 6] = [
    ("instrument_id", "str"),
    ("period", "str"),
    ("dscr", "float64"),
    ("dscr_total", "float64"),
    ("dscr_incl_fees", "float64"),
    ("interest_coverage", "float64"),
];

/// Terminal value method for a DCF.
///
/// Build with one of the constructors; the wire form is the tagged serde enum
/// (``{"type": "gordon_growth", "growth_rate": 0.02}``).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import TerminalValueSpec
/// >>> TerminalValueSpec.gordon_growth(0.02).kind
/// 'gordon_growth'
/// >>> TerminalValueSpec.exit_multiple(9.0).params["multiple"]
/// 9.0
#[pyclass(
    name = "TerminalValueSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyTerminalValueSpec {
    pub(crate) inner: TerminalValueSpec,
}

#[pymethods]
impl PyTerminalValueSpec {
    /// Gordon growth: ``TV = FCF_terminal * (1 + g) / (WACC - g)``.
    ///
    /// Parameters
    /// ----------
    /// growth_rate : float
    ///     Perpetual growth rate ``g`` in decimal form (``0.02`` = 2%).
    #[staticmethod]
    #[pyo3(text_signature = "(growth_rate)")]
    fn gordon_growth(growth_rate: f64) -> Self {
        Self {
            inner: TerminalValueSpec::GordonGrowth { growth_rate },
        }
    }

    /// Exit multiple: ``TV = terminal_metric * multiple``.
    ///
    /// Parameters
    /// ----------
    /// multiple : float
    ///     Exit multiple in turns (``9.0`` = 9.0x).
    /// terminal_metric : float
    ///     Terminal-year metric (EBITDA, revenue, ...) in model currency.
    ///     Default ``0.0``; pass ``exit_multiple_metric_node`` to
    ///     ``evaluate_dcf`` to read it from the statement model instead.
    #[staticmethod]
    #[pyo3(signature = (multiple, terminal_metric=0.0))]
    fn exit_multiple(multiple: f64, terminal_metric: f64) -> Self {
        Self {
            inner: TerminalValueSpec::ExitMultiple {
                terminal_metric,
                multiple,
            },
        }
    }

    /// H-model: growth decays linearly from ``high_growth_rate`` to
    /// ``stable_growth_rate`` over ``2 * half_life_years``.
    ///
    /// Parameters
    /// ----------
    /// high_growth_rate : float
    ///     Initial growth rate in decimal form.
    /// stable_growth_rate : float
    ///     Long-run growth rate in decimal form.
    /// half_life_years : float
    ///     Half-life of the decay in years.
    #[staticmethod]
    #[pyo3(text_signature = "(high_growth_rate, stable_growth_rate, half_life_years)")]
    fn h_model(high_growth_rate: f64, stable_growth_rate: f64, half_life_years: f64) -> Self {
        Self {
            inner: TerminalValueSpec::HModel {
                high_growth_rate,
                stable_growth_rate,
                half_life_years,
            },
        }
    }

    /// Serde tag: ``"gordon_growth"``, ``"exit_multiple"`` or ``"h_model"``.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        let value = serde_json::to_value(&self.inner)
            .map_err(|e| serde_json_to_py(e, "TerminalValueSpec"))?;
        Ok(value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// Method parameters as a dict (the wire form without the ``type`` tag).
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut value = serde_json::to_value(&self.inner)
            .map_err(|e| serde_json_to_py(e, "TerminalValueSpec"))?;
        if let Some(map) = value.as_object_mut() {
            map.remove("type");
        }
        serde_to_py(py, &value)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "TerminalValueSpec"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid tagged ``TerminalValueSpec`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid TerminalValueSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TerminalValueSpec", &self.inner)
    }
}

/// Structured enterprise-to-equity bridge.
///
/// ``net_adjustment = total_debt - cash + preferred_equity + minority_interest
/// - non_operating_assets + sum(other_adjustments)``; all amounts in the model
/// currency.
///
/// Parameters
/// ----------
/// total_debt : float
///     Interest-bearing debt. Default ``0.0``.
/// cash : float
///     Cash and equivalents. Default ``0.0``.
/// preferred_equity : float
///     Preferred equity claims. Default ``0.0``.
/// minority_interest : float
///     Non-controlling interests. Default ``0.0``.
/// non_operating_assets : float
///     Non-operating assets added back. Default ``0.0``.
/// other_adjustments : list[tuple[str, float]]
///     Labelled additional claims (positive reduces equity). Default ``[]``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import EquityBridge
/// >>> EquityBridge(total_debt=500.0, cash=100.0).net_adjustment
/// 400.0
#[pyclass(
    name = "EquityBridge",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyEquityBridge {
    pub(crate) inner: EquityBridge,
}

#[pymethods]
impl PyEquityBridge {
    #[new]
    #[pyo3(signature = (
        total_debt=0.0,
        cash=0.0,
        preferred_equity=0.0,
        minority_interest=0.0,
        non_operating_assets=0.0,
        other_adjustments=Vec::new(),
    ))]
    fn new(
        total_debt: f64,
        cash: f64,
        preferred_equity: f64,
        minority_interest: f64,
        non_operating_assets: f64,
        other_adjustments: Vec<(String, f64)>,
    ) -> Self {
        Self {
            inner: EquityBridge {
                total_debt,
                cash,
                preferred_equity,
                minority_interest,
                non_operating_assets,
                other_adjustments,
            },
        }
    }

    /// Interest-bearing debt.
    #[getter]
    fn total_debt(&self) -> f64 {
        self.inner.total_debt
    }

    /// Cash and equivalents.
    #[getter]
    fn cash(&self) -> f64 {
        self.inner.cash
    }

    /// Preferred equity claims.
    #[getter]
    fn preferred_equity(&self) -> f64 {
        self.inner.preferred_equity
    }

    /// Non-controlling interests.
    #[getter]
    fn minority_interest(&self) -> f64 {
        self.inner.minority_interest
    }

    /// Non-operating assets.
    #[getter]
    fn non_operating_assets(&self) -> f64 {
        self.inner.non_operating_assets
    }

    /// Labelled additional claims.
    #[getter]
    fn other_adjustments(&self) -> Vec<(String, f64)> {
        self.inner.other_adjustments.clone()
    }

    /// Net amount subtracted from enterprise value.
    #[getter]
    fn net_adjustment(&self) -> f64 {
        self.inner.net_adjustment()
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "EquityBridge"))
    }

    /// Deserialize from canonical JSON (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``EquityBridge`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid EquityBridge JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EquityBridge", &self.inner)
    }
}

/// Equity-level valuation discounts (DLOM, DLOC, other).
///
/// Each discount is a decimal fraction in ``[0, 1]`` applied multiplicatively
/// to the pre-discount equity value.
///
/// Parameters
/// ----------
/// dlom : float | None
///     Discount for lack of marketability.
/// dloc : float | None
///     Discount for lack of control.
/// other_discount : float | None
///     Any additional discount.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import ValuationDiscounts
/// >>> ValuationDiscounts(dlom=0.25).dlom
/// 0.25
#[pyclass(
    name = "ValuationDiscounts",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyValuationDiscounts {
    pub(crate) inner: ValuationDiscounts,
}

#[pymethods]
impl PyValuationDiscounts {
    #[new]
    #[pyo3(signature = (dlom=None, dloc=None, other_discount=None))]
    fn new(dlom: Option<f64>, dloc: Option<f64>, other_discount: Option<f64>) -> PyResult<Self> {
        let inner = ValuationDiscounts {
            dlom,
            dloc,
            other_discount,
        };
        inner.validate().map_err(crate::errors::core_to_py)?;
        Ok(Self { inner })
    }

    /// Discount for lack of marketability, or ``None``.
    #[getter]
    fn dlom(&self) -> Option<f64> {
        self.inner.dlom
    }

    /// Discount for lack of control, or ``None``.
    #[getter]
    fn dloc(&self) -> Option<f64> {
        self.inner.dloc
    }

    /// Additional discount, or ``None``.
    #[getter]
    fn other_discount(&self) -> Option<f64> {
        self.inner.other_discount
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ValuationDiscounts"))
    }

    /// Deserialize from canonical JSON (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``ValuationDiscounts`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ValuationDiscounts JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ValuationDiscounts", &self.inner)
    }
}

fn extract_terminal_value(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<TerminalValueSpec> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyTerminalValueSpec>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "terminal_value")
}

fn extract_equity_bridge(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<EquityBridge> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyEquityBridge>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "equity_bridge")
}

fn extract_valuation_discounts(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<ValuationDiscounts> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyValuationDiscounts>>() {
        return Ok(typed.inner.clone());
    }
    extract_serde_any(py, obj, "valuation_discounts")
}

/// DCF outputs in the model currency.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CorporateValuationResult
/// >>> r = CorporateValuationResult.from_json(
/// ...     '{"equity_value":{"amount":"90","currency":"USD"},"enterprise_value":{"amount":"100","currency":"USD"},'
/// ...     '"net_debt":{"amount":"10","currency":"USD"},"terminal_value_pv":{"amount":"60","currency":"USD"},'
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

    /// Net debt (or effective bridge amount) subtracted from EV.
    #[getter]
    fn net_debt(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.net_debt)
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

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``currency``, ``equity_value``, ``enterprise_value``,
    /// ``net_debt``, ``terminal_value_pv`` (float amounts in ``currency``),
    /// ``equity_value_per_share``, ``diluted_shares`` (``None`` when absent).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "currency": self.inner.equity_value.currency().to_string(),
            "equity_value": self.inner.equity_value.amount(),
            "enterprise_value": self.inner.enterprise_value.amount(),
            "net_debt": self.inner.net_debt.amount(),
            "terminal_value_pv": self.inner.terminal_value_pv.amount(),
            "equity_value_per_share": self.inner.equity_value_per_share,
            "diluted_shares": self.inner.diluted_shares,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "currency",
                "equity_value",
                "enterprise_value",
                "net_debt",
                "terminal_value_pv",
                "equity_value_per_share",
                "diluted_shares",
            ],
        )
    }

    /// Serialize to canonical JSON (``Money`` fields as ``{"amount", "currency"}``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "CorporateValuationResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``CorporateValuationResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CorporateValuationResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CorporateValuationResult", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Tornado ranking of the headline DCF assumptions by enterprise-value impact.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import DcfSensitivityResult
/// >>> r = DcfSensitivityResult.from_json(
/// ...     '{"baseline_enterprise_value":{"amount":"100","currency":"USD"},'
/// ...     '"entries":[{"parameter_id":"wacc","downside":-5.0,"upside":6.0}],'
/// ...     '"wacc_down":0.09,"wacc_down_clamped":false,"terminal_growth_up":0.03,"terminal_growth_up_clamped":false}')
/// >>> list(r.to_dataframe()["swing"])
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

    /// Export the tornado table as a pandas ``DataFrame``.
    ///
    /// Columns: ``parameter_id``, ``downside``, ``upside``, ``swing`` (EV
    /// deltas in the baseline currency). One row per entry, in ranked order.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        serde_rows_to_dataframe_with_schema(py, &rows, &TORNADO_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "DcfSensitivityResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``DcfSensitivityResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid DcfSensitivityResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DcfSensitivityResult", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Node mappings that switch on the LBO model check suite.
///
/// Parameters
/// ----------
/// three_statement : ThreeStatementMapping | dict | str
///     Balance-sheet / income / cash-flow node mapping.
/// credit : CreditMapping | dict | str
///     Leverage and coverage node mapping.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CreditMapping, LboCheckMappings, ThreeStatementMapping
/// >>> m = LboCheckMappings(ThreeStatementMapping("cash", "re", "ni"), CreditMapping("debt", "ebitda", "interest"))
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

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "LboCheckMappings"))
    }

    /// Deserialize from canonical JSON (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``LboCheckMappings`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid LboCheckMappings JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LboCheckMappings", &self.inner)
    }
}

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

    /// Entry metric read from the model's first period.
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

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``currency``, ``entry_enterprise_value``, ``entry_metric``,
    /// ``debt_total``, ``equity_check``, ``sources_total``, ``uses_total``,
    /// ``sources_uses_balanced``, ``exit_enterprise_value``, ``exit_metric``,
    /// ``exit_net_debt``, ``exit_equity_proceeds``, ``moic``. Money columns are
    /// float amounts in ``currency``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "currency",
                "entry_enterprise_value",
                "entry_metric",
                "debt_total",
                "equity_check",
                "sources_total",
                "uses_total",
                "sources_uses_balanced",
                "exit_enterprise_value",
                "exit_metric",
                "exit_net_debt",
                "exit_equity_proceeds",
                "moic",
            ],
        )
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "LboResult"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``LboResult`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid LboResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LboResult", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Orchestrated statement + equity + credit analysis envelope.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import run_corporate_analysis
/// >>> b = ModelBuilder("m"); b.periods("2024Q1..Q2", None); b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
/// >>> analysis = run_corporate_analysis(b.build())
/// >>> analysis.equity is None, analysis.ev_suppressed_non_positive
/// (True, False)
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

    /// Per-instrument credit metrics as ``{instrument_id: CreditContextMetrics dict}``
    /// (serde form, including ``dscr_incl_fees`` / ``dscr_incl_fees_min``).
    #[getter]
    fn credit<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit)
    }

    /// Whether a non-positive DCF enterprise value was excluded from LTV.
    #[getter]
    fn ev_suppressed_non_positive(&self) -> bool {
        self.inner.ev_suppressed_non_positive
    }

    /// Export the per-instrument credit metrics as a long pandas ``DataFrame``.
    ///
    /// Columns: ``instrument_id``, ``period`` (period-id string), ``dscr``,
    /// ``dscr_total``, ``dscr_incl_fees``, ``interest_coverage`` (turns;
    /// ``NaN`` where a metric is not available for that period). One row per
    /// (instrument, period) present on any metric series.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use std::collections::BTreeMap;
        let mut rows = Vec::new();
        for (instrument_id, metrics) in &self.inner.credit {
            let mut by_period: BTreeMap<finstack_quant_core::dates::PeriodId, [Option<f64>; 4]> =
                BTreeMap::new();
            let series = [
                (&metrics.dscr, 0usize),
                (&metrics.dscr_total, 1),
                (&metrics.dscr_incl_fees, 2),
                (&metrics.interest_coverage, 3),
            ];
            for (values, slot) in series {
                for (period, value) in values {
                    by_period.entry(*period).or_default()[slot] = Some(*value);
                }
            }
            for (period, values) in by_period {
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
        serde_rows_to_dataframe_with_schema(py, &rows, &CREDIT_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "CorporateAnalysis"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``CorporateAnalysis`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CorporateAnalysis JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CorporateAnalysis(statement_nodes={}, equity={}, credit_instruments={}, ev_suppressed_non_positive={})",
            self.inner.statement.nodes.len(),
            if self.inner.equity.is_some() {
                "CorporateValuationResult(...)"
            } else {
                "None"
            },
            self.inner.credit.len(),
            crate::bindings::statements_analytics::py_bool(self.inner.ev_suppressed_non_positive)
        )
    }
}

/// Evaluate DCF valuation on a financial model.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string; metadata must contain
///     a ``"currency"`` key.
/// wacc : float
///     Weighted average cost of capital in decimal form (``0.10`` = 10%).
/// terminal_value : TerminalValueSpec | dict | str
///     Terminal value method (typed, serde dict, or tagged JSON such as
///     ``{"type": "gordon_growth", "growth_rate": 0.02}``).
/// ufcf_node : str
///     Node id containing unlevered free cash flow. Default ``"ufcf"``.
/// net_debt_override : float | None
///     Flat net-debt amount used instead of the model-derived bridge.
/// mid_year_convention : bool
///     Mid-year discounting. Default ``False`` (year-end).
/// max_stable_growth_rate : float | None
///     Maximum perpetual growth accepted for Gordon Growth / H-Model
///     (decimal); ``None`` uses the canonical 5% default.
/// shares_outstanding : float | None
///     Basic shares outstanding for per-share equity value.
/// equity_bridge : EquityBridge | dict | str | None
///     Structured EV-to-equity bridge.
/// valuation_discounts : ValuationDiscounts | dict | str | None
///     DLOM / DLOC / other discounts.
/// market : MarketContext | str | None
///     Market context used for statement evaluation (capital-structure curve
///     lookups); DCF discounting stays WACC-only. Requires ``as_of``.
/// as_of : datetime.date | str | None
///     DCF valuation date and, with ``market``, the statement visibility and
///     market-data date. Defaults to the first forecast boundary.
/// exit_multiple_metric_node : str | None
///     Statement monetary flow node whose complete trailing-year sum replaces
///     ``terminal_metric`` on an exit-multiple terminal. Actual period boundaries must cover one complete contiguous calendar year; otherwise omit this node and supply an explicit annual metric.
///
/// Returns
/// -------
/// CorporateValuationResult
///     ``equity_value``, ``enterprise_value``, ``net_debt`` and
///     ``terminal_value_pv`` as ``Money``; per-share values as floats.
///
/// Raises
/// ------
/// ValueError
///     If ``market`` is set without ``as_of``, a payload is malformed, or the
///     model, cash-flow node, exit-multiple node or DCF inputs are invalid.
/// KeyError
///     If ``ufcf_node`` or ``exit_multiple_metric_node`` is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import TerminalValueSpec, evaluate_dcf
/// >>> b = ModelBuilder("dcf"); b.periods("2025..2026")
/// >>> b.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
/// >>> b.with_meta("currency", '"USD"')
/// >>> result = evaluate_dcf(b.build(), 0.10, TerminalValueSpec.gordon_growth(0.02), net_debt_override=0.0)
/// >>> result.enterprise_value.currency.code
/// 'USD'
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc,
    terminal_value,
    ufcf_node="ufcf",
    net_debt_override=None,
    mid_year_convention=false,
    max_stable_growth_rate=None,
    shares_outstanding=None,
    equity_bridge=None,
    valuation_discounts=None,
    market=None,
    as_of=None,
    exit_multiple_metric_node=None,
))]
#[allow(clippy::too_many_arguments)]
fn evaluate_dcf<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: f64,
    terminal_value: &Bound<'py, PyAny>,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    mid_year_convention: bool,
    max_stable_growth_rate: Option<f64>,
    shares_outstanding: Option<f64>,
    equity_bridge: Option<&Bound<'py, PyAny>>,
    valuation_discounts: Option<&Bound<'py, PyAny>>,
    market: Option<&Bound<'py, PyAny>>,
    as_of: Option<&Bound<'py, PyAny>>,
    exit_multiple_metric_node: Option<&str>,
) -> PyResult<PyCorporateValuationResult> {
    let model = extract_model_ref(model)?.into_owned();
    let terminal_value = extract_terminal_value(py, terminal_value)?;
    let ufcf_node = ufcf_node.to_owned();
    let equity_bridge = equity_bridge
        .map(|obj| extract_equity_bridge(py, obj))
        .transpose()?;
    let valuation_discounts = valuation_discounts
        .map(|obj| extract_valuation_discounts(py, obj))
        .transpose()?;

    let options = DcfOptions {
        mid_year_convention,
        max_stable_growth_rate: max_stable_growth_rate
            .unwrap_or_else(|| DcfOptions::default().max_stable_growth_rate),
        equity_bridge,
        shares_outstanding,
        valuation_discounts,
        exit_multiple_metric_node: exit_multiple_metric_node.map(str::to_owned),
        ..Default::default()
    };

    let market = extract_market_opt(py, market)?;
    let as_of = as_of
        .map(crate::bindings::date_utils::extract_date)
        .transpose()?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_dcf_with_market(
                &model,
                wacc,
                terminal_value,
                &ufcf_node,
                net_debt_override,
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
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string; metadata must include
///     a ``"currency"`` key.
/// wacc : float
///     Baseline WACC in decimal form.
/// terminal_value : TerminalValueSpec | dict | str
///     Terminal value method; selects whether the growth rate or the exit
///     multiple is shocked.
/// ufcf_node : str
///     Node id containing unlevered free cash flow. Default ``"ufcf"``.
/// net_debt_override : float | None
///     Flat net-debt amount used instead of the model-derived bridge.
/// wacc_sensitivity_bump : float | None
///     Absolute shock to WACC and terminal growth in decimal (``0.01`` =
///     +/-100bp); ``None`` uses the Rust ``DcfOptions`` default.
/// wacc_denominator_epsilon : float | None
///     Minimum ``wacc - g`` spread preserved, in decimal; ``None`` uses the
///     Rust default.
/// max_stable_growth_rate : float | None
///     Maximum perpetual growth (decimal); ``None`` uses the 5% default.
/// exit_multiple_bump : float | None
///     Absolute exit-multiple shock in turns; ``None`` uses the Rust default.
/// mid_year_convention : bool
///     Mid-year discounting for every re-run. Default ``False``.
/// market : MarketContext | str | None
///     Market context for statement evaluation (not WACC discounting).
/// exit_multiple_metric_node : str | None
///     Statement monetary flow node supplying the complete trailing-year terminal metric; insufficient or noncontiguous history raises ValueError.
///
/// Returns
/// -------
/// DcfSensitivityResult
///     Baseline EV, ranked ``entries`` and the clamped-shock flags;
///     ``to_dataframe()`` gives the tornado table.
///
/// Raises
/// ------
/// ValueError
///     If a payload is malformed or the model or DCF inputs are invalid.
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
/// >>> list(sens.to_dataframe().columns)
/// ['parameter_id', 'downside', 'upside', 'swing']
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc,
    terminal_value,
    ufcf_node="ufcf",
    net_debt_override=None,
    wacc_sensitivity_bump=None,
    wacc_denominator_epsilon=None,
    max_stable_growth_rate=None,
    exit_multiple_bump=None,
    mid_year_convention=false,
    market=None,
    exit_multiple_metric_node=None,
))]
#[allow(clippy::too_many_arguments)]
fn dcf_sensitivity<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: f64,
    terminal_value: &Bound<'py, PyAny>,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    wacc_sensitivity_bump: Option<f64>,
    wacc_denominator_epsilon: Option<f64>,
    max_stable_growth_rate: Option<f64>,
    exit_multiple_bump: Option<f64>,
    mid_year_convention: bool,
    market: Option<&Bound<'py, PyAny>>,
    exit_multiple_metric_node: Option<&str>,
) -> PyResult<PyDcfSensitivityResult> {
    let model = extract_model_ref(model)?.into_owned();
    let terminal_value = extract_terminal_value(py, terminal_value)?;
    let ufcf_node = ufcf_node.to_owned();
    let market = extract_market_opt(py, market)?;

    // Defaults come from the canonical Rust `DcfOptions` at runtime rather
    // than duplicated signature literals.
    let defaults = DcfOptions::default();
    let options = DcfOptions {
        mid_year_convention,
        wacc_sensitivity_bump: wacc_sensitivity_bump.unwrap_or(defaults.wacc_sensitivity_bump),
        wacc_denominator_epsilon: wacc_denominator_epsilon
            .unwrap_or(defaults.wacc_denominator_epsilon),
        max_stable_growth_rate: max_stable_growth_rate.unwrap_or(defaults.max_stable_growth_rate),
        exit_multiple_bump: exit_multiple_bump
            .map_or(defaults.exit_multiple_bump, ExitMultipleBump::Absolute),
        exit_multiple_metric_node: exit_multiple_metric_node.map(str::to_owned),
        ..DcfOptions::default()
    };

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::dcf_sensitivity(
                &model,
                wacc,
                terminal_value,
                &ufcf_node,
                net_debt_override,
                &options,
                market.as_ref(),
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyDcfSensitivityResult { inner })
}

/// Evaluate a leveraged-buyout transaction against a statement model.
///
/// Entry enterprise value is priced at the model's first period, the sponsor
/// equity check is the sources-and-uses residual, and exit proceeds are the
/// exit enterprise value less modelled net debt at ``exit_period``. IRR is out
/// of scope: pair ``exit_equity_proceeds`` with the equity outflow at close and
/// call ``finstack_quant.portfolio.mwr_xirr``.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string; metadata must include
///     a ``"currency"`` key.
/// entry_multiple : float
///     Entry multiple in turns (``8.5`` = 8.5x).
/// entry_metric_node : str
///     Node supplying the entry metric at the first period (e.g. ``"ebitda"``).
/// exit_multiple : float
///     Exit multiple in turns.
/// exit_metric_node : str
///     Node supplying the exit metric at ``exit_period``.
/// exit_net_debt_node : str
///     Node supplying net debt at ``exit_period``.
/// exit_period : str
///     Exit period label (``"2029"`` or ``"2029Q4"``).
/// sources : list[tuple[str, float]]
///     Funded debt tranches at close as ``(name, amount)`` in model currency.
/// transaction_fees : float
///     Fees funded at close, in model currency. Default ``0.0``.
/// check_mappings : LboCheckMappings | dict | str | None
///     When supplied, runs the LBO model check suite against the same
///     evaluation and populates ``LboResult.checks``.
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
///     If a tranche amount is invalid, ``exit_period`` does not parse, sources
///     and uses cannot balance, or the model fails to evaluate.
/// KeyError
///     If a metric or net-debt node is missing at the required period.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import evaluate_lbo
/// >>> b = ModelBuilder("m")
/// >>> _ = b.periods("2025Q1..2026Q1", None)
/// >>> _ = b.with_meta("currency", "USD")
/// >>> _ = b.value("ebitda", [("2025Q1", 100.0), ("2025Q2", 100.0), ("2025Q3", 100.0), ("2025Q4", 100.0), ("2026Q1", 120.0)])
/// >>> _ = b.value("net_debt", [("2025Q1", 300.0), ("2025Q2", 300.0), ("2025Q3", 300.0), ("2025Q4", 300.0), ("2026Q1", 200.0)])
/// >>> lbo = evaluate_lbo(b.build(), 8.0, "ebitda", 9.0, "ebitda", "net_debt", "2026Q1", [("debt", 500.0)])
/// >>> round(lbo.moic, 4)
/// 2.9333
#[pyfunction]
#[pyo3(signature = (
    model,
    entry_multiple,
    entry_metric_node,
    exit_multiple,
    exit_metric_node,
    exit_net_debt_node,
    exit_period,
    sources,
    transaction_fees=0.0,
    check_mappings=None,
))]
#[allow(clippy::too_many_arguments)]
fn evaluate_lbo<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    entry_multiple: f64,
    entry_metric_node: &str,
    exit_multiple: f64,
    exit_metric_node: &str,
    exit_net_debt_node: &str,
    exit_period: &str,
    sources: Vec<(String, f64)>,
    transaction_fees: f64,
    check_mappings: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyLboResult> {
    let model = extract_model_ref(model)?.into_owned();
    let exit_period: finstack_quant_core::dates::PeriodId =
        exit_period.parse().map_err(crate::errors::display_to_py)?;
    let check_mappings = check_mappings
        .map(|obj| {
            if let Ok(typed) = obj.extract::<PyRef<'_, PyLboCheckMappings>>() {
                Ok(typed.inner.clone())
            } else {
                extract_serde_any(py, obj, "check_mappings")
            }
        })
        .transpose()?;

    let config = LboConfig {
        entry_multiple,
        entry_metric_node: entry_metric_node.to_owned(),
        transaction_fees,
        sources: sources
            .into_iter()
            .map(|(name, amount)| LboTranche { name, amount })
            .collect(),
        exit_multiple,
        exit_metric_node: exit_metric_node.to_owned(),
        exit_net_debt_node: exit_net_debt_node.to_owned(),
        exit_period,
        check_mappings,
    };

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_lbo(&model, &config)
        })
        .map_err(statements_to_py)?;
    Ok(PyLboResult { inner })
}

/// Weighted-average cost of capital (WACC).
///
/// ``WACC = w_E * r_E + w_D * r_D * (1 - T)``.
///
/// Parameters
/// ----------
/// equity_weight : float
///     Equity share of total capital as a decimal fraction; non-negative.
/// cost_of_equity : float
///     Required return on equity in decimal form (``0.115`` = 11.5%).
/// debt_weight : float
///     Debt share of total capital as a decimal fraction; non-negative and
///     summing with ``equity_weight`` to ``1.0``.
/// cost_of_debt : float
///     Pre-tax marginal borrowing yield in decimal form.
/// tax_rate : float
///     Marginal corporate tax rate as a decimal in ``[0, 1]``.
///
/// Returns
/// -------
/// float
///     Blended discount rate as a decimal fraction.
///
/// Raises
/// ------
/// ValueError
///     If a weight is negative, the weights do not sum to ``1.0``, or the tax
///     rate is outside ``[0, 1]``.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import wacc
/// >>> round(wacc(0.6, 0.10, 0.4, 0.05, 0.25), 4)
/// 0.075
#[pyfunction]
#[pyo3(signature = (equity_weight, cost_of_equity, debt_weight, cost_of_debt, tax_rate))]
fn wacc(
    equity_weight: f64,
    cost_of_equity: f64,
    debt_weight: f64,
    cost_of_debt: f64,
    tax_rate: f64,
) -> PyResult<f64> {
    finstack_quant_statements_analytics::analysis::wacc(
        equity_weight,
        cost_of_equity,
        debt_weight,
        cost_of_debt,
        tax_rate,
    )
    .map_err(statements_to_py)
}

/// Run the full corporate analysis pipeline.
///
/// Evaluates statements and optionally runs DCF equity valuation plus credit
/// context through the Rust ``CorporateAnalysisBuilder``.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string.
/// wacc : float | None
///     Enables DCF valuation at this decimal discount rate when set.
/// terminal_value : TerminalValueSpec | dict | str | None
///     Terminal value method; required when ``wacc`` is set.
/// net_debt_override : float | None
///     Flat net debt for the equity bridge.
/// cfads_node : str | None
///     CFADS numerator required when the model has capital-structure credit
///     analytics; no EBITDA fallback is applied.
/// interest_coverage_node : str
///     Earnings numerator used for interest coverage. Default ``"ebitda"``.
/// check_suite : CheckSuiteSpec | dict | str | None
///     Check suite required for DCF or credit analysis; must include
///     ``NonFiniteCheck``.
/// market : MarketContext | str | None
///     Market context for statement evaluation (not WACC discounting).
/// as_of : datetime.date | str | None
///     Valuation date; required when ``market`` is set.
/// ltv_value_node : str | None
///     Statement node supplying a per-period LTV denominator. When omitted, a
///     positive DCF enterprise value is broadcast as a constant denominator.
///
/// Returns
/// -------
/// CorporateAnalysis
///     ``statement`` (``StatementResult``), ``equity``
///     (``CorporateValuationResult`` or ``None``), ``credit`` (per-instrument
///     metrics) and ``ev_suppressed_non_positive``.
///
/// Raises
/// ------
/// ValueError
///     If ``terminal_value`` is missing while ``wacc`` is set, ``market`` is
///     set without ``as_of``, a payload is malformed, or the pipeline fails.
/// KeyError
///     If a referenced node is missing from the model.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder
/// >>> from finstack_quant.statements_analytics import run_corporate_analysis
/// >>> b = ModelBuilder("m"); b.periods("2024Q1..Q2", None); b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
/// >>> run_corporate_analysis(b.build()).statement.node_count
/// 1
#[pyfunction]
#[pyo3(signature = (
    model,
    wacc=None,
    terminal_value=None,
    net_debt_override=None,
    cfads_node=None,
    interest_coverage_node="ebitda",
    check_suite=None,
    market=None,
    as_of=None,
    ltv_value_node=None,
))]
#[allow(clippy::too_many_arguments)]
fn run_corporate_analysis<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: Option<f64>,
    terminal_value: Option<&Bound<'py, PyAny>>,
    net_debt_override: Option<f64>,
    cfads_node: Option<&str>,
    interest_coverage_node: &str,
    check_suite: Option<&Bound<'py, PyAny>>,
    market: Option<&Bound<'py, PyAny>>,
    as_of: Option<&Bound<'py, PyAny>>,
    ltv_value_node: Option<&str>,
) -> PyResult<PyCorporateAnalysis> {
    let model = extract_model_ref(model)?.into_owned();
    let mut builder =
        finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder::new(model)
            .interest_coverage_node(interest_coverage_node);
    if let Some(node) = cfads_node {
        builder = builder.cfads_node(node);
    }
    if let Some(spec) = check_suite {
        let spec = extract_check_suite_spec(py, spec)?;
        builder = builder.checks(spec.resolve().map_err(statements_to_py)?);
    }

    if let Some(w) = wacc {
        let terminal_value = terminal_value.ok_or_else(|| {
            crate::errors::value_error("terminal_value is required when wacc is set")
        })?;
        let tv = extract_terminal_value(py, terminal_value)?;
        builder = builder.dcf(w, tv);
        if let Some(nd) = net_debt_override {
            builder = builder.net_debt_override(nd);
        }
    }

    if let Some(mkt) = extract_market_opt(py, market)? {
        builder = builder.market(mkt);
    }

    if let Some(as_of) = as_of {
        let date = crate::bindings::date_utils::extract_date(as_of)?;
        builder = builder.as_of(date);
    }

    if let Some(node) = ltv_value_node {
        builder = builder.ltv_value_node(node);
    }

    let inner = py
        .detach(move || builder.analyze())
        .map_err(statements_to_py)?;
    Ok(PyCorporateAnalysis { inner })
}

/// Register valuation types and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTerminalValueSpec>()?;
    m.add_class::<PyEquityBridge>()?;
    m.add_class::<PyValuationDiscounts>()?;
    m.add_class::<PyCorporateValuationResult>()?;
    m.add_class::<PyDcfSensitivityResult>()?;
    m.add_class::<PyLboCheckMappings>()?;
    m.add_class::<PyLboResult>()?;
    m.add_class::<PyCorporateAnalysis>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_lbo, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(wacc, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_corporate_analysis, m)?)?;
    Ok(())
}
