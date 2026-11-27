//! DCF specification, bridge, discount, and request Python bindings.

use super::sensitivity::PyCorporateValuationResult;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::extract_model_ref;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::wire_methods;
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements_analytics::analysis::{
    CorporateDcfRequest, DcfProjection, StatementValuationCut, TerminalFcffBasis,
};
use finstack_quant_statements_analytics::analysis::{DEFAULT_UFCF_NODE, DcfInputs, DcfOptions};
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    EquityBridge, TerminalValueSpec, ValuationDiscounts,
};
use pyo3::prelude::*;

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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfInputs")]
#[derive(Clone)]
pub(super) struct PyDcfInputs {
    pub(super) inner: DcfInputs,
}

#[pymethods]
impl PyDcfInputs {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfInputs.__init__")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfOptions")]
#[derive(Clone)]
pub(super) struct PyDcfOptions {
    inner: DcfOptions,
}

#[pymethods]
impl PyDcfOptions {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfOptions.__init__")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "StatementValuationCut")]
#[derive(Clone)]
pub(crate) struct PyStatementValuationCut {
    pub(crate) inner: StatementValuationCut,
}

#[pymethods]
impl PyStatementValuationCut {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "StatementValuationCut.__init__")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfProjection")]
#[derive(Clone)]
pub(super) struct PyDcfProjection {
    inner: DcfProjection,
}

#[pymethods]
impl PyDcfProjection {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "DcfProjection.__init__")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "TerminalFcffBasis")]
#[derive(Clone)]
pub(super) struct PyTerminalFcffBasis {
    inner: TerminalFcffBasis,
}

#[pymethods]
impl PyTerminalFcffBasis {
    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "TerminalFcffBasis.authored")]
    fn authored(amount: &PyMoney) -> Self {
        Self {
            inner: TerminalFcffBasis::Authored {
                amount: amount.inner,
            },
        }
    }

    #[staticmethod]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "TerminalFcffBasis.trailing_additive")]
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
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "CorporateDcfRequest")]
#[derive(Clone)]
pub(super) struct PyCorporateDcfRequest {
    pub(super) inner: CorporateDcfRequest,
}

#[pymethods]
impl PyCorporateDcfRequest {
    #[new]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "CorporateDcfRequest.__init__")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "evaluate_corporate_dcf")]
#[pyfunction]
#[pyo3(signature = (model, request, market=None))]
pub(super) fn evaluate_corporate_dcf<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    request: &PyCorporateDcfRequest,
    market: Option<PyRef<'py, PyMarketContext>>,
) -> PyResult<PyCorporateValuationResult> {
    let model = extract_model_ref(model)?.into_owned();
    let request = request.inner.clone();
    let market = market.map(|market| market.inner.clone());
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._valuation.inputs", "evaluate_dcf_from_results")]
#[pyfunction]
pub(super) fn evaluate_dcf_from_results<'py>(
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

pub(super) fn extract_terminal_value(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<TerminalValueSpec> {
    if let Ok(typed) = obj.extract::<PyRef<'_, PyTerminalValueSpec>>() {
        return Ok(typed.inner.clone());
    }
    crate::bindings::module_utils::spec_from_py(py, obj, "terminal_value")
}

/// Rust `DcfOptions` from its serde form (a dict or JSON string, missing
/// fields taking their Rust defaults); `None` is `DcfOptions::default()`.
pub(super) fn extract_dcf_options(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<DcfOptions> {
    if let Some(bound) = obj
        && let Ok(options) = bound.extract::<PyRef<'_, PyDcfOptions>>()
    {
        return Ok(options.inner.clone());
    }
    obj.map(|obj| crate::bindings::module_utils::spec_from_py(py, obj, "options"))
        .transpose()
        .map(Option::unwrap_or_default)
}
