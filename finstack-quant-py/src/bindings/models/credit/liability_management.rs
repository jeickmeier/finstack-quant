//! Python bindings for `finstack_quant_models::credit::liability_management`.

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::wire::{serde_label, serde_parse};
use finstack_quant_models::credit::liability_management::{
    self as lm, ExchangeOfferAnalysis, ExchangeType, LeverageImpact, LmeAnalysis, LmeType,
    TENDER_RECOMMENDATION_HURDLE,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis")]
#[pyclass(
    name = "ExchangeOfferAnalysis",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyExchangeOfferAnalysis {
    inner: ExchangeOfferAnalysis,
}

#[pymethods]
impl PyExchangeOfferAnalysis {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.exchange_type")]
    #[getter]
    fn exchange_type(&self) -> PyResult<String> {
        serde_label(&self.inner.exchange_type).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.old_npv")]
    #[getter]
    fn old_npv(&self) -> f64 {
        self.inner.old_npv
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.new_npv")]
    #[getter]
    fn new_npv(&self) -> f64 {
        self.inner.new_npv
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.consent_fee")]
    #[getter]
    fn consent_fee(&self) -> f64 {
        self.inner.consent_fee
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.equity_sweetener_value")]
    #[getter]
    fn equity_sweetener_value(&self) -> f64 {
        self.inner.equity_sweetener_value
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.tender_total")]
    #[getter]
    fn tender_total(&self) -> f64 {
        self.inner.tender_total
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.delta_npv")]
    #[getter]
    fn delta_npv(&self) -> f64 {
        self.inner.delta_npv
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.breakeven_recovery")]
    #[getter]
    fn breakeven_recovery(&self) -> f64 {
        self.inner.breakeven_recovery
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.tender_recommended")]
    #[getter]
    fn tender_recommended(&self) -> bool {
        self.inner.tender_recommended
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // Built explicitly so the column order is fixed; `exchange_type`
        // serializes to its snake_case label.
        let row = serde_json::json!({
            "exchange_type": self.inner.exchange_type,
            "old_npv": self.inner.old_npv,
            "new_npv": self.inner.new_npv,
            "consent_fee": self.inner.consent_fee,
            "equity_sweetener_value": self.inner.equity_sweetener_value,
            "tender_total": self.inner.tender_total,
            "delta_npv": self.inner.delta_npv,
            "breakeven_recovery": self.inner.breakeven_recovery,
            "tender_recommended": self.inner.tender_recommended,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "exchange_type",
                "old_npv",
                "new_npv",
                "consent_fee",
                "equity_sweetener_value",
                "tender_total",
                "delta_npv",
                "breakeven_recovery",
                "tender_recommended",
            ],
        )
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "ExchangeOfferAnalysis(exchange_type='{}', tender_total={}, delta_npv={}, \
             tender_recommended={})",
            self.exchange_type()?,
            self.inner.tender_total,
            self.inner.delta_npv,
            self.inner.tender_recommended
        ))
    }
}

wire_methods!(
    PyExchangeOfferAnalysis,
    ExchangeOfferAnalysis,
    "ExchangeOfferAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);
impl_repr_html_via_dataframe!(PyExchangeOfferAnalysis);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact")]
#[pyclass(
    name = "LeverageImpact",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLeverageImpact {
    inner: LeverageImpact,
}

#[pymethods]
impl PyLeverageImpact {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.pre_total_debt")]
    #[getter]
    fn pre_total_debt(&self) -> f64 {
        self.inner.pre_total_debt
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.post_total_debt")]
    #[getter]
    fn post_total_debt(&self) -> f64 {
        self.inner.post_total_debt
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.pre_leverage")]
    #[getter]
    fn pre_leverage(&self) -> f64 {
        self.inner.pre_leverage
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.post_leverage")]
    #[getter]
    fn post_leverage(&self) -> f64 {
        self.inner.post_leverage
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.leverage_reduction")]
    #[getter]
    fn leverage_reduction(&self) -> f64 {
        self.inner.leverage_reduction
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &[
                "pre_total_debt",
                "post_total_debt",
                "pre_leverage",
                "post_leverage",
                "leverage_reduction",
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "LeverageImpact(pre_leverage={}, post_leverage={}, leverage_reduction={})",
            self.inner.pre_leverage, self.inner.post_leverage, self.inner.leverage_reduction
        )
    }
}

wire_methods!(
    PyLeverageImpact,
    LeverageImpact,
    "LeverageImpact",
    module = "finstack_quant.models.credit.liability_management"
);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis")]
#[pyclass(
    name = "LmeAnalysis",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLmeAnalysis {
    inner: LmeAnalysis,
}

#[pymethods]
impl PyLmeAnalysis {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.lme_type")]
    #[getter]
    fn lme_type(&self) -> PyResult<String> {
        serde_label(&self.inner.lme_type).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.cost")]
    #[getter]
    fn cost(&self) -> f64 {
        self.inner.cost
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.notional_reduction")]
    #[getter]
    fn notional_reduction(&self) -> f64 {
        self.inner.notional_reduction
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.discount_capture")]
    #[getter]
    fn discount_capture(&self) -> f64 {
        self.inner.discount_capture
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.discount_capture_pct")]
    #[getter]
    fn discount_capture_pct(&self) -> f64 {
        self.inner.discount_capture_pct
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.remaining_holder_impact_pct")]
    #[getter]
    fn remaining_holder_impact_pct(&self) -> f64 {
        self.inner.remaining_holder_impact_pct
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.leverage_impact")]
    #[getter]
    fn leverage_impact(&self) -> Option<PyLeverageImpact> {
        self.inner
            .leverage_impact
            .clone()
            .map(|inner| PyLeverageImpact { inner })
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `LmeAnalysis` derives Serialize, but `leverage_impact` is a nested
        // struct that blind serde would drop into a single object-dtype cell,
        // so the row is flattened by hand.
        let leverage = self.inner.leverage_impact.as_ref();
        let row = serde_json::json!({
            "lme_type": self.inner.lme_type,
            "cost": self.inner.cost,
            "notional_reduction": self.inner.notional_reduction,
            "discount_capture": self.inner.discount_capture,
            "discount_capture_pct": self.inner.discount_capture_pct,
            "remaining_holder_impact_pct": self.inner.remaining_holder_impact_pct,
            "pre_total_debt": leverage.map(|l| l.pre_total_debt),
            "post_total_debt": leverage.map(|l| l.post_total_debt),
            "pre_leverage": leverage.map(|l| l.pre_leverage),
            "post_leverage": leverage.map(|l| l.post_leverage),
            "leverage_reduction": leverage.map(|l| l.leverage_reduction),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "lme_type",
                "cost",
                "notional_reduction",
                "discount_capture",
                "discount_capture_pct",
                "remaining_holder_impact_pct",
                "pre_total_debt",
                "post_total_debt",
                "pre_leverage",
                "post_leverage",
                "leverage_reduction",
            ],
        )
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "LmeAnalysis(lme_type='{}', cost={}, notional_reduction={}, discount_capture={})",
            self.lme_type()?,
            self.inner.cost,
            self.inner.notional_reduction,
            self.inner.discount_capture
        ))
    }
}

wire_methods!(
    PyLmeAnalysis,
    LmeAnalysis,
    "LmeAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);
impl_repr_html_via_dataframe!(PyLmeAnalysis);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "analyze_exchange_offer")]
#[pyfunction]
#[pyo3(text_signature = "(old_pv, new_pv, consent_fee, equity_sweetener_value, exchange_type)")]
fn analyze_exchange_offer(
    old_pv: f64,
    new_pv: f64,
    consent_fee: f64,
    equity_sweetener_value: f64,
    exchange_type: &str,
) -> PyResult<PyExchangeOfferAnalysis> {
    let exchange_type = serde_parse::<ExchangeType>(exchange_type).map_err(core_to_py)?;
    lm::analyze_exchange_offer(
        old_pv,
        new_pv,
        consent_fee,
        equity_sweetener_value,
        exchange_type,
    )
    .map(|inner| PyExchangeOfferAnalysis { inner })
    .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "analyze_lme")]
#[pyfunction]
#[pyo3(signature = (lme_type, notional, repurchase_price_pct, opt_acceptance_pct, ebitda=None))]
#[pyo3(
    text_signature = "(lme_type, notional, repurchase_price_pct, opt_acceptance_pct, ebitda=None)"
)]
fn analyze_lme(
    lme_type: &str,
    notional: f64,
    repurchase_price_pct: f64,
    opt_acceptance_pct: f64,
    ebitda: Option<f64>,
) -> PyResult<PyLmeAnalysis> {
    let lme_type = serde_parse::<LmeType>(lme_type).map_err(core_to_py)?;
    lm::analyze_lme(
        lme_type,
        notional,
        repurchase_price_pct,
        opt_acceptance_pct,
        ebitda,
    )
    .map(|inner| PyLmeAnalysis { inner })
    .map_err(core_to_py)
}

/// Build the `finstack_quant.models.credit.liability_management` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "liability_management")?;
    m.setattr(
        "__doc__",
        pydoc!(
            "finstack_quant.models.credit.liability_management",
            "__module__"
        ),
    )?;

    m.add("TENDER_RECOMMENDATION_HURDLE", TENDER_RECOMMENDATION_HURDLE)?;
    m.add_class::<PyExchangeOfferAnalysis>()?;
    m.add_class::<PyLeverageImpact>()?;
    m.add_class::<PyLmeAnalysis>()?;
    m.add_function(wrap_pyfunction!(analyze_exchange_offer, &m)?)?;
    m.add_function(wrap_pyfunction!(analyze_lme, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ExchangeOfferAnalysis",
            "LeverageImpact",
            "LmeAnalysis",
            "TENDER_RECOMMENDATION_HURDLE",
            "analyze_exchange_offer",
            "analyze_lme",
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
