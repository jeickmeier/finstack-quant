//! Python bindings for DCF, LBO and the orchestrated corporate analysis.
//!
//! Typed inputs (`TerminalValueSpec`, `EquityBridge`, `ValuationDiscounts`,
//! `LboCheckMappings`) and typed results (`CorporateValuationResult`,
//! `DcfSensitivityResult`, `LboResult`, `CorporateAnalysis`) wrap the Rust
//! types one-to-one. The Rust config types (`DcfOptions`, `LboConfig`) are
//! taken in their serde form (dict or JSON string), where a typed input may
//! sit in place of its nested dict; every other structured input also accepts
//! the canonical JSON string.

use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_opt, extract_model_ref};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    ColumnSchema, serde_object_to_single_row_dataframe_with_schema,
    serde_rows_to_dataframe_with_schema, serde_to_py,
};
use crate::bindings::statements::checks::PyCheckReport;
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements_analytics::checks::{
    PyCreditMapping, PyThreeStatementMapping, extract_credit_mapping,
    extract_three_statement_mapping,
};
use crate::bindings::statements_analytics::typed::PyTornadoEntry;
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::DcfDistributionResult;
use finstack_quant_statements_analytics::analysis::{
    CorporateAnalysis, CorporateAnalysisOptions, CorporateValuationResult, DEFAULT_UFCF_NODE,
    DcfInputs, DcfOptions, DcfSensitivityResult, LboCheckMappings, LboConfig, LboResult,
    WaccInputs,
};
use finstack_quant_statements_analytics::analysis::{
    CorporateDcfRequest, DcfProjection, DcfSensitivityEndpoint, DcfSensitivityGridResult,
    DcfSensitivityGridSpec, StatementValuationCut, TerminalFcffBasis,
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
/// (``{"type": "gordon_growth", "stable_growth_rate": 0.02}``).
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import TerminalValueSpec
/// >>> TerminalValueSpec.gordon_growth(0.02).kind
/// 'gordon_growth'
/// >>> TerminalValueSpec.exit_multiple(9.0, 120.0).params["multiple"]
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
    /// stable_growth_rate : float
    ///     Perpetual stable growth rate ``g`` in decimal form (``0.02`` = 2%).
    #[staticmethod]
    #[pyo3(text_signature = "(stable_growth_rate)")]
    fn gordon_growth(stable_growth_rate: f64) -> Self {
        Self {
            inner: TerminalValueSpec::GordonGrowth { stable_growth_rate },
        }
    }

    /// Exit multiple: ``TV = terminal_metric * multiple``.
    ///
    /// Parameters
    /// ----------
    /// multiple : float
    ///     Exit multiple in turns (``9.0`` = 9.0x).
    /// terminal_metric : float or None
    ///     Terminal-year metric (EBITDA, revenue, ...) in model currency.
    ///     Omit when
    ///     ``DcfOptions.exit_multiple_metric_node`` supplies the metric from
    ///     the statement model. Without that node, ``evaluate_dcf`` raises
    ///     ``ValueError`` unless the metric is positive and finite.
    #[staticmethod]
    #[pyo3(signature = (multiple, terminal_metric=None))]
    fn exit_multiple(multiple: f64, terminal_metric: Option<f64>) -> Self {
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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TerminalValueSpec", &self.inner)
    }
}

wire_methods!(PyTerminalValueSpec, TerminalValueSpec, "TerminalValueSpec");

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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("EquityBridge", &self.inner)
    }
}

wire_methods!(PyEquityBridge, EquityBridge, "EquityBridge");

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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ValuationDiscounts", &self.inner)
    }
}

wire_methods!(
    PyValuationDiscounts,
    ValuationDiscounts,
    "ValuationDiscounts"
);

#[pyclass(
    name = "DcfInputs",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfInputs")]
#[derive(Clone)]
struct PyDcfInputs {
    inner: DcfInputs,
}

#[pymethods]
impl PyDcfInputs {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfInputs.__init__")]
    #[pyo3(signature = (*, wacc, terminal_value, ufcf_node=DEFAULT_UFCF_NODE, net_debt_override=None))]
    fn new(
        wacc: f64,
        terminal_value: &PyTerminalValueSpec,
        ufcf_node: &str,
        net_debt_override: Option<f64>,
    ) -> PyResult<Self> {
        let inner = DcfInputs {
            wacc,
            terminal_value: terminal_value.inner.clone(),
            ufcf_node: ufcf_node.to_owned(),
            net_debt_override,
        };
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }
}

#[pyclass(
    name = "DcfOptions",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfOptions")]
#[derive(Clone)]
struct PyDcfOptions {
    inner: DcfOptions,
}

#[pymethods]
impl PyDcfOptions {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfOptions.__init__")]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, mid_year_convention=False, equity_bridge=None, shares_outstanding=None, valuation_discounts=None, exit_multiple_metric_node=None)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner = crate::bindings::module_utils::fields_from_py(py, fields, "DcfOptions")?;
        Ok(Self { inner })
    }
}

#[pyclass(
    name = "StatementValuationCut",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "StatementValuationCut")]
#[derive(Clone)]
pub(crate) struct PyStatementValuationCut {
    pub(crate) inner: StatementValuationCut,
}

#[pymethods]
impl PyStatementValuationCut {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "StatementValuationCut.__init__")]
    fn new(
        as_of: &Bound<'_, PyAny>,
        observed_through: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        use crate::bindings::date_utils::py_to_date;
        Ok(Self {
            inner: StatementValuationCut {
                as_of: py_to_date(as_of)?,
                observed_through: py_to_date(observed_through)?,
                currency: crate::bindings::core::currency::extract_currency(currency)?,
            },
        })
    }
}

#[pyclass(
    name = "DcfProjection",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfProjection")]
#[derive(Clone)]
struct PyDcfProjection {
    inner: DcfProjection,
}

#[pymethods]
impl PyDcfProjection {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfProjection.__init__")]
    #[pyo3(signature = (*, terminal_date, normalized_annual_fcff=None, forward_remainder_node=None, stable_growth_ceiling=None))]
    fn new(
        terminal_date: &Bound<'_, PyAny>,
        normalized_annual_fcff: Option<PyRef<'_, PyTerminalFcffBasis>>,
        forward_remainder_node: Option<String>,
        stable_growth_ceiling: Option<f64>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: DcfProjection {
                terminal_date: crate::bindings::date_utils::py_to_date(terminal_date)?,
                normalized_annual_fcff: normalized_annual_fcff.map(|basis| basis.inner.clone()),
                forward_remainder_node,
                stable_growth_ceiling,
            },
        })
    }
}

#[pyclass(
    name = "TerminalFcffBasis",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "TerminalFcffBasis")]
#[derive(Clone)]
struct PyTerminalFcffBasis {
    inner: TerminalFcffBasis,
}

#[pymethods]
impl PyTerminalFcffBasis {
    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "TerminalFcffBasis.authored")]
    fn authored(amount: &PyMoney) -> Self {
        Self {
            inner: TerminalFcffBasis::Authored {
                amount: amount.inner,
            },
        }
    }

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "TerminalFcffBasis.trailing_additive")]
    fn trailing_additive() -> Self {
        Self {
            inner: TerminalFcffBasis::TrailingAdditive,
        }
    }
}

#[pyclass(
    name = "CorporateDcfRequest",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "CorporateDcfRequest")]
#[derive(Clone)]
struct PyCorporateDcfRequest {
    inner: CorporateDcfRequest,
}

#[pymethods]
impl PyCorporateDcfRequest {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "CorporateDcfRequest.__init__")]
    #[pyo3(signature = (*, cut, inputs, projection, options=None))]
    fn new(
        cut: &PyStatementValuationCut,
        inputs: &PyDcfInputs,
        projection: &PyDcfProjection,
        options: Option<PyRef<'_, PyDcfOptions>>,
    ) -> Self {
        Self {
            inner: CorporateDcfRequest {
                cut: cut.inner.clone(),
                inputs: inputs.inner.clone(),
                projection: projection.inner.clone(),
                options: options
                    .map(|options| options.inner.clone())
                    .unwrap_or_default(),
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "evaluate_corporate_dcf")]
#[pyfunction]
#[pyo3(signature = (model, request, market=None))]
fn evaluate_corporate_dcf<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    request: &PyCorporateDcfRequest,
    market: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyCorporateValuationResult> {
    let model = extract_model_ref(model)?.into_owned();
    let request = request.inner.clone();
    let market = extract_market_opt(py, market)?;
    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_corporate_dcf(
                &model,
                &request,
                market.as_ref(),
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyCorporateValuationResult { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "evaluate_dcf_from_results")]
#[pyfunction]
fn evaluate_dcf_from_results<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    results: &Bound<'py, PyAny>,
    request: &PyCorporateDcfRequest,
) -> PyResult<PyCorporateValuationResult> {
    let model = extract_model_ref(model)?.into_owned();
    let results = crate::bindings::extract::extract_results_ref(results)?.into_owned();
    let request = request.inner.clone();
    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_dcf_from_results(
                &model, &results, &request,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyCorporateValuationResult { inner })
}

#[pyclass(
    name = "DcfSensitivityGridSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfSensitivityGridSpec")]
#[derive(Clone)]
struct PyDcfSensitivityGridSpec {
    inner: DcfSensitivityGridSpec,
}

#[pymethods]
impl PyDcfSensitivityGridSpec {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfSensitivityGridSpec.__init__")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfSensitivityGridResult")]
#[derive(Clone)]
struct PyDcfSensitivityGridResult {
    inner: DcfSensitivityGridResult,
}

#[pymethods]
impl PyDcfSensitivityGridResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfSensitivityGridResult.baseline_enterprise_value")]
    fn baseline_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.baseline_enterprise_value)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfSensitivityGridResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "dcf_sensitivity_on_results")]
#[pyfunction]
fn dcf_sensitivity_on_results<'py>(
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
    crate::bindings::pandas_utils::serde_to_py(py, &report)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "dcf_sensitivity_grid")]
#[pyfunction]
fn dcf_sensitivity_grid<'py>(
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfDistributionResult")]
#[derive(Clone)]
struct PyDcfDistributionResult {
    inner: DcfDistributionResult,
}

#[pymethods]
impl PyDcfDistributionResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfDistributionResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("path_id", "uint32"),
                ("enterprise_value", "float64"),
                ("equity_value", "float64"),
                ("currency", "str"),
            ],
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "DcfDistributionResult.quantiles_to_dataframe")]
    fn quantiles_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        serde_rows_to_dataframe_with_schema(
            py,
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "evaluate_dcf_monte_carlo")]
#[pyfunction]
fn evaluate_dcf_monte_carlo<'py>(
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

fn extract_terminal_value(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<TerminalValueSpec> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyTerminalValueSpec>>() {
        return Ok(typed.inner.clone());
    }
    crate::bindings::module_utils::spec_from_py(py, obj, "terminal_value")
}

/// Rust `DcfOptions` from its serde form (a dict or JSON string, missing
/// fields taking their Rust defaults); `None` is `DcfOptions::default()`.
fn extract_dcf_options(py: Python<'_>, obj: Option<&Bound<'_, PyAny>>) -> PyResult<DcfOptions> {
    if let Some(bound) = obj
        && let Ok(options) = bound.extract::<PyRef<'_, PyDcfOptions>>()
    {
        return Ok(options.inner.clone());
    }
    obj.map(|obj| crate::bindings::module_utils::spec_from_py(py, obj, "options"))
        .transpose()
        .map(Option::unwrap_or_default)
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "CorporateValuationResult.other_equity_claims")]
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

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``currency``, ``equity_value``, ``enterprise_value``,
    /// ``net_debt``, ``other_equity_claims``, ``valuation_discount``, ``terminal_value_pv`` (float
    /// amounts in ``currency``),
    /// ``equity_value_per_share``, ``diluted_shares`` (``None`` when absent),
    /// then the DCF working: ``wacc`` (decimal), ``pv_explicit``,
    /// ``terminal_value``, ``total_debt``, ``cash`` (float amounts in
    /// ``currency``) and ``terminal_discount_years``; these six are ``None``
    /// for a result loaded from JSON written before they were recorded.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "currency",
                "equity_value",
                "enterprise_value",
                "net_debt",
                "other_equity_claims",
                "valuation_discount",
                "terminal_value_pv",
                "equity_value_per_share",
                "diluted_shares",
                "wacc",
                "pv_explicit",
                "terminal_value",
                "total_debt",
                "cash",
                "terminal_discount_years",
            ],
        )
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
impl_repr_html_via_dataframe!(PyCorporateValuationResult);

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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("DcfSensitivityResult", &self.inner)
    }
}

wire_methods!(
    PyDcfSensitivityResult,
    DcfSensitivityResult,
    "DcfSensitivityResult"
);
impl_repr_html_via_dataframe!(PyDcfSensitivityResult);

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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LboResult", &self.inner)
    }
}

wire_methods!(PyLboResult, LboResult, "LboResult");
impl_repr_html_via_dataframe!(PyLboResult);

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

    fn __repr__(&self) -> String {
        format!(
            "CorporateAnalysis(statement_nodes={}, equity={}, credit_instruments={}, ev_suppressed_non_positive={})",
            self.inner.statement.cells.len(),
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

wire_methods!(PyCorporateAnalysis, CorporateAnalysis, "CorporateAnalysis");

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "evaluate_dcf")]
#[pyfunction]
#[pyo3(signature = (model, inputs, options=None, market=None, as_of=None))]
fn evaluate_dcf<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    inputs: &Bound<'py, PyAny>,
    options: Option<&Bound<'py, PyAny>>,
    market: Option<&Bound<'py, PyAny>>,
    as_of: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyCorporateValuationResult> {
    let model = extract_model_ref(model)?.into_owned();
    let inputs = match inputs.extract::<PyRef<'_, PyDcfInputs>>() {
        Ok(inputs) => inputs.inner.clone(),
        Err(_) => crate::bindings::module_utils::spec_from_py(py, inputs, "DcfInputs")?,
    };
    let options = extract_dcf_options(py, options)?;
    let market = extract_market_opt(py, market)?;
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
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string; metadata must include
///     a ``"currency"`` key.
/// wacc : float
///     Baseline WACC in decimal form.
/// terminal_value : TerminalValueSpec | dict | str
///     Terminal value method; selects whether the growth rate or the exit
///     multiple is shocked.
/// ufcf_node : str
///     Node id containing monetary unlevered free cash flow in model currency.
///     Defaults to the Rust ``DEFAULT_UFCF_NODE`` (``"ufcf"``). Gordon Growth /
///     H-Model terminal cash flow requires a complete contiguous calendar year
///     at the end of the forecast.
/// net_debt_override : float | None
///     Flat net-debt amount used instead of the model-derived bridge.
/// options : dict | str | None
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
///     ``to_dataframe()`` gives the tornado table.
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
/// >>> list(sens.to_dataframe().columns)
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
fn dcf_sensitivity<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    wacc: f64,
    terminal_value: &Bound<'py, PyAny>,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    options: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyDcfSensitivityResult> {
    let model = extract_model_ref(model)?.into_owned();
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
/// model : FinancialModelSpec | str
///     A ``FinancialModelSpec`` object or a JSON string; metadata must include
///     a ``"currency"`` key.
/// config : dict | str
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
fn evaluate_lbo<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    config: &Bound<'py, PyAny>,
) -> PyResult<PyLboResult> {
    let model = extract_model_ref(model)?.into_owned();
    let config: LboConfig = crate::bindings::module_utils::spec_from_py(py, config, "config")?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::evaluate_lbo(&model, &config)
        })
        .map_err(statements_to_py)?;
    Ok(PyLboResult { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.__init__")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.equity_weight")]
    #[getter]
    fn equity_weight<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.equity_weight.0)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.cost_of_equity")]
    #[getter]
    fn cost_of_equity(&self) -> f64 {
        self.inner.cost_of_equity
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.debt_weight")]
    #[getter]
    fn debt_weight<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.debt_weight.0)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.cost_of_debt")]
    #[getter]
    fn cost_of_debt(&self) -> f64 {
        self.inner.cost_of_debt
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "WaccInputs.tax_rate")]
    #[getter]
    fn tax_rate(&self) -> f64 {
        self.inner.tax_rate
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("WaccInputs", &self.inner)
    }
}

wire_methods!(PyWaccInputs, WaccInputs, "WaccInputs");

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "wacc")]
#[pyfunction]
#[pyo3(signature = (inputs))]
fn wacc(inputs: PyWaccInputs) -> PyResult<f64> {
    finstack_quant_statements_analytics::analysis::wacc(&inputs.inner).map_err(statements_to_py)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation", "run_corporate_analysis")]
#[pyfunction]
#[pyo3(signature = (model, options=None, market=None))]
fn run_corporate_analysis<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    options: Option<&Bound<'py, PyAny>>,
    market: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyCorporateAnalysis> {
    let model = extract_model_ref(model)?.into_owned();
    let options: CorporateAnalysisOptions =
        crate::bindings::module_utils::opt_spec_from_py(py, options, "CorporateAnalysisOptions")?
            .unwrap_or_default();
    let market = extract_market_opt(py, market)?;

    let inner = py
        .detach(move || {
            finstack_quant_statements_analytics::analysis::run_corporate_analysis(
                model, options, market,
            )
        })
        .map_err(statements_to_py)?;
    Ok(PyCorporateAnalysis { inner })
}

/// Register valuation types and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDcfDistributionResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf_monte_carlo, m)?)?;
    m.add_class::<PyDcfInputs>()?;
    m.add_class::<PyDcfOptions>()?;
    m.add_class::<PyStatementValuationCut>()?;
    m.add_class::<PyTerminalFcffBasis>()?;
    m.add_class::<PyDcfProjection>()?;
    m.add_class::<PyCorporateDcfRequest>()?;
    m.add_class::<PyDcfSensitivityGridSpec>()?;
    m.add_class::<PyDcfSensitivityGridResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_corporate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf_from_results, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity_grid, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity_on_results, m)?)?;
    m.add_class::<PyTerminalValueSpec>()?;
    m.add_class::<PyEquityBridge>()?;
    m.add_class::<PyValuationDiscounts>()?;
    m.add_class::<PyCorporateValuationResult>()?;
    m.add_class::<PyDcfSensitivityResult>()?;
    m.add_class::<PyLboCheckMappings>()?;
    m.add_class::<PyLboResult>()?;
    m.add_class::<PyCorporateAnalysis>()?;
    m.add_class::<PyWaccInputs>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_lbo, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(wacc, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_corporate_analysis, m)?)?;
    Ok(())
}
