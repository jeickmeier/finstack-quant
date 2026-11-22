//! Python bindings for `finstack_quant_models::credit::liability_management`.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_core::wire::{serde_label, serde_parse};
use finstack_quant_models::credit::liability_management::{
    self as lm, ExchangeOffer, ExchangeOfferAnalysis, ExchangeType, LeverageImpact, LmeAnalysis,
    LmeTerms, LmeType, TENDER_RECOMMENDATION_HURDLE,
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

py_getters!(PyExchangeOfferAnalysis |s| {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.exchange_type")]
    exchange_type -> PyResult<String> = serde_label(&s.inner.exchange_type).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.old_npv")]
    old_npv -> f64 = s.inner.old_npv;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.new_npv")]
    new_npv -> f64 = s.inner.new_npv;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.consent_fee")]
    consent_fee -> f64 = s.inner.consent_fee;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.equity_sweetener_value")]
    equity_sweetener_value -> f64 = s.inner.equity_sweetener_value;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.tender_total")]
    tender_total -> f64 = s.inner.tender_total;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.delta_npv")]
    delta_npv -> f64 = s.inner.delta_npv;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.breakeven_recovery")]
    breakeven_recovery -> f64 = s.inner.breakeven_recovery;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOfferAnalysis.tender_recommended")]
    tender_recommended -> bool = s.inner.tender_recommended;
});

impl PyExchangeOfferAnalysis {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("exchange_type", vec![self.exchange_type()?])?
            .f64("old_npv", vec![self.inner.old_npv])?
            .f64("new_npv", vec![self.inner.new_npv])?
            .f64("consent_fee", vec![self.inner.consent_fee])?
            .f64(
                "equity_sweetener_value",
                vec![self.inner.equity_sweetener_value],
            )?
            .f64("tender_total", vec![self.inner.tender_total])?
            .f64("delta_npv", vec![self.inner.delta_npv])?
            .f64("breakeven_recovery", vec![self.inner.breakeven_recovery])?
            .bool("tender_recommended", vec![self.inner.tender_recommended])?
            .build()
    }
}

tabular_methods!(
    PyExchangeOfferAnalysis,
    "ExchangeOfferAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);
wire_methods!(
    PyExchangeOfferAnalysis,
    ExchangeOfferAnalysis,
    "ExchangeOfferAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);

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
    fn __repr__(&self) -> String {
        format!(
            "LeverageImpact(pre_leverage={}, post_leverage={}, leverage_reduction={})",
            self.inner.pre_leverage, self.inner.post_leverage, self.inner.leverage_reduction
        )
    }
}

py_getters!(PyLeverageImpact |s| {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.pre_total_debt")]
    pre_total_debt -> f64 = s.inner.pre_total_debt;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.post_total_debt")]
    post_total_debt -> f64 = s.inner.post_total_debt;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.pre_leverage")]
    pre_leverage -> f64 = s.inner.pre_leverage;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.post_leverage")]
    post_leverage -> f64 = s.inner.post_leverage;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LeverageImpact.leverage_reduction")]
    leverage_reduction -> f64 = s.inner.leverage_reduction;
});

impl PyLeverageImpact {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("pre_total_debt", vec![self.inner.pre_total_debt])?
            .f64("post_total_debt", vec![self.inner.post_total_debt])?
            .f64("pre_leverage", vec![self.inner.pre_leverage])?
            .f64("post_leverage", vec![self.inner.post_leverage])?
            .f64("leverage_reduction", vec![self.inner.leverage_reduction])?
            .build()
    }
}

tabular_methods!(
    PyLeverageImpact,
    "LeverageImpact",
    module = "finstack_quant.models.credit.liability_management"
);
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

py_getters!(PyLmeAnalysis |s| {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.lme_type")]
    lme_type -> PyResult<String> = serde_label(&s.inner.lme_type).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.cost")]
    cost -> f64 = s.inner.cost;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.notional_reduction")]
    notional_reduction -> f64 = s.inner.notional_reduction;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.discount_capture")]
    discount_capture -> f64 = s.inner.discount_capture;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.discount_capture_pct")]
    discount_capture_pct -> f64 = s.inner.discount_capture_pct;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.remaining_holder_impact_pct")]
    remaining_holder_impact_pct -> f64 = s.inner.remaining_holder_impact_pct;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeAnalysis.leverage_impact")]
    leverage_impact -> Option<PyLeverageImpact> = s
        .inner
        .leverage_impact
        .clone()
        .map(|inner| PyLeverageImpact { inner });
});

impl PyLmeAnalysis {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let leverage = self.inner.leverage_impact.as_ref();
        TableBuilder::new()
            .utf8("lme_type", vec![self.lme_type()?])?
            .f64("cost", vec![self.inner.cost])?
            .f64("notional_reduction", vec![self.inner.notional_reduction])?
            .f64("discount_capture", vec![self.inner.discount_capture])?
            .f64(
                "discount_capture_pct",
                vec![self.inner.discount_capture_pct],
            )?
            .f64(
                "remaining_holder_impact_pct",
                vec![self.inner.remaining_holder_impact_pct],
            )?
            .nullable_f64("pre_total_debt", vec![leverage.map(|l| l.pre_total_debt)])?
            .nullable_f64("post_total_debt", vec![leverage.map(|l| l.post_total_debt)])?
            .nullable_f64("pre_leverage", vec![leverage.map(|l| l.pre_leverage)])?
            .nullable_f64("post_leverage", vec![leverage.map(|l| l.post_leverage)])?
            .nullable_f64(
                "leverage_reduction",
                vec![leverage.map(|l| l.leverage_reduction)],
            )?
            .build()
    }
}

tabular_methods!(
    PyLmeAnalysis,
    "LmeAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);
wire_methods!(
    PyLmeAnalysis,
    LmeAnalysis,
    "LmeAnalysis",
    module = "finstack_quant.models.credit.liability_management"
);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer")]
#[pyclass(
    name = "ExchangeOffer",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyExchangeOffer {
    inner: ExchangeOffer,
}

#[pymethods]
impl PyExchangeOffer {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.__init__")]
    #[new]
    #[pyo3(signature = (*, old_pv, new_pv, consent_fee, equity_sweetener_value, exchange_type))]
    fn new(
        old_pv: f64,
        new_pv: f64,
        consent_fee: f64,
        equity_sweetener_value: f64,
        exchange_type: &str,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ExchangeOffer {
                old_pv,
                new_pv,
                consent_fee,
                equity_sweetener_value,
                exchange_type: serde_parse::<ExchangeType>(exchange_type).map_err(core_to_py)?,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ExchangeOffer", &self.inner)
    }
}

py_getters!(PyExchangeOffer |s| {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.old_pv")]
    old_pv -> f64 = s.inner.old_pv;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.new_pv")]
    new_pv -> f64 = s.inner.new_pv;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.consent_fee")]
    consent_fee -> f64 = s.inner.consent_fee;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.equity_sweetener_value")]
    equity_sweetener_value -> f64 = s.inner.equity_sweetener_value;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "ExchangeOffer.exchange_type")]
    exchange_type -> PyResult<String> = serde_label(&s.inner.exchange_type).map_err(core_to_py);
});

wire_methods!(
    PyExchangeOffer,
    ExchangeOffer,
    "ExchangeOffer",
    module = "finstack_quant.models.credit.liability_management"
);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms")]
#[pyclass(
    name = "LmeTerms",
    module = "finstack_quant.models.credit.liability_management",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyLmeTerms {
    inner: LmeTerms,
}

#[pymethods]
impl PyLmeTerms {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.__init__")]
    #[new]
    #[pyo3(signature = (*, lme_type, notional, repurchase_price_pct, opt_acceptance_pct, ebitda=None))]
    fn new(
        lme_type: &str,
        notional: f64,
        repurchase_price_pct: f64,
        opt_acceptance_pct: f64,
        ebitda: Option<f64>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: LmeTerms {
                lme_type: serde_parse::<LmeType>(lme_type).map_err(core_to_py)?,
                notional,
                repurchase_price_pct,
                opt_acceptance_pct,
                ebitda,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LmeTerms", &self.inner)
    }
}

py_getters!(PyLmeTerms |s| {
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.lme_type")]
    lme_type -> PyResult<String> = serde_label(&s.inner.lme_type).map_err(core_to_py);
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.notional")]
    notional -> f64 = s.inner.notional;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.repurchase_price_pct")]
    repurchase_price_pct -> f64 = s.inner.repurchase_price_pct;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.opt_acceptance_pct")]
    opt_acceptance_pct -> f64 = s.inner.opt_acceptance_pct;
    #[doc = pydoc!("finstack_quant.models.credit.liability_management", "LmeTerms.ebitda")]
    ebitda -> Option<f64> = s.inner.ebitda;
});

wire_methods!(
    PyLmeTerms,
    LmeTerms,
    "LmeTerms",
    module = "finstack_quant.models.credit.liability_management"
);

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "analyze_exchange_offer")]
#[pyfunction]
#[pyo3(text_signature = "(offer)")]
fn analyze_exchange_offer(offer: PyExchangeOffer) -> PyResult<PyExchangeOfferAnalysis> {
    lm::analyze_exchange_offer(&offer.inner)
        .map(|inner| PyExchangeOfferAnalysis { inner })
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit.liability_management", "analyze_lme")]
#[pyfunction]
#[pyo3(text_signature = "(terms)")]
fn analyze_lme(terms: PyLmeTerms) -> PyResult<PyLmeAnalysis> {
    lm::analyze_lme(&terms.inner)
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
    m.add_class::<PyExchangeOffer>()?;
    m.add_class::<PyExchangeOfferAnalysis>()?;
    m.add_class::<PyLeverageImpact>()?;
    m.add_class::<PyLmeAnalysis>()?;
    m.add_class::<PyLmeTerms>()?;
    m.add_function(wrap_pyfunction!(analyze_exchange_offer, &m)?)?;
    m.add_function(wrap_pyfunction!(analyze_lme, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ExchangeOffer",
            "ExchangeOfferAnalysis",
            "LeverageImpact",
            "LmeAnalysis",
            "LmeTerms",
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
