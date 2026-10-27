use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, enum_to_py_string, typed_list_from_py,
};
use crate::errors::core_to_py;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_equity_metrics, CreditModelConfig, DealFees, DealType, LossAllocationPolicy,
    LossRecognition, MarketConditions, Metadata, StructuredCredit, TrancheDraw, TrancheReadvance,
    WaterfallRules,
};

use super::super::convert::{enum_from_str, instrument_expiry};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use super::{
    PyAssetPool, PyCallAssumption, PyCoverageRules, PyEquityMetrics, PyHedgeSwap, PySimulationRun,
    PyStochasticPricingResult, PyTrancheCashflows, PyTrancheStructure, PyWaterfall,
};

use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StructuredCredit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStructuredCredit {
    /// Inner canonical Rust structured-credit deal.
    pub(crate) inner: StructuredCredit,
}

instrument_envelope_methods!(
    PyStructuredCredit(
        "StructuredCredit",
        module = "finstack_quant.valuations.instruments._typed_structured_credit.structured_credit"
    ),
    StructuredCredit,
    builder = PyStructuredCreditBuilder("StructuredCreditBuilder", no_fields),
    seed = StructuredCredit::builder(),
);
instrument_pricing_methods!(
    PyStructuredCredit,
    "StructuredCredit",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.structured_credit"
);

#[pymethods]
impl PyStructuredCredit {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.new_standard")]
    #[staticmethod]
    #[pyo3(text_signature = "(deal_type, params)")]
    fn new_standard(
        deal_type: &str,
        params: PyRef<'_, super::PyStandardDealParams>,
    ) -> PyResult<Self> {
        let deal_type = enum_from_str::<DealType>(deal_type, "DealType")?;
        StructuredCredit::new_standard(deal_type, params.inner.clone())
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.price_stochastic")]
    #[pyo3(signature = (market, as_of, num_paths=None, antithetic=None))]
    #[pyo3(text_signature = "($self, market, as_of, num_paths=None, antithetic=None)")]
    fn price_stochastic(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        num_paths: Option<usize>,
        antithetic: Option<bool>,
    ) -> PyResult<PyStochasticPricingResult> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let inner = py
            .detach(move || deal.price_stochastic(&market, as_of, num_paths, antithetic))
            .map_err(core_to_py)?;
        Ok(PyStochasticPricingResult { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.run_simulation")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn run_simulation(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PySimulationRun> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let inner = py
            .detach(move || deal.run_simulation(&market, as_of))
            .map_err(core_to_py)?;
        Ok(PySimulationRun { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.closing_date")]
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.first_payment_date")]
    #[getter]
    fn first_payment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.first_payment_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.quote_settlement_date")]
    #[getter]
    fn quote_settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .quote_settlement_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.with_standard_fees")]
    #[pyo3(text_signature = "($self)")]
    fn with_standard_fees(&self) -> Self {
        Self {
            inner: self.inner.clone().with_standard_fees(),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.enable_stochastic")]
    #[pyo3(text_signature = "($self)")]
    fn enable_stochastic(&self) -> PyResult<Self> {
        let mut deal = self.inner.clone();
        deal.enable_stochastic().map_err(core_to_py)?;
        Ok(Self { inner: deal })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.create_waterfall")]
    #[pyo3(text_signature = "($self)")]
    fn create_waterfall(&self) -> PyResult<PyWaterfall> {
        let inner = self.inner.create_waterfall().map_err(core_to_py)?;
        Ok(PyWaterfall { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.tranche_cashflows")]
    #[pyo3(text_signature = "($self, tranche_id, market, as_of)")]
    fn tranche_cashflows(
        &self,
        py: Python<'_>,
        tranche_id: &str,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyTrancheCashflows> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let tranche_id = tranche_id.to_string();
        let inner = py
            .detach(move || deal.tranche_cashflows(&tranche_id, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PyTrancheCashflows { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.equity_metrics")]
    #[pyo3(signature = (market, as_of, purchase_price_pct=None))]
    #[pyo3(text_signature = "($self, market, as_of, purchase_price_pct=None)")]
    fn equity_metrics(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        purchase_price_pct: Option<f64>,
    ) -> PyResult<PyEquityMetrics> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let deal = self.inner.clone();
        let inner = py
            .detach(move || calculate_equity_metrics(&deal, &market, as_of, purchase_price_pct))
            .map_err(core_to_py)?;
        Ok(PyEquityMetrics { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<Option<String>> {
        self.inner
            .business_day_convention
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.credit_model")]
    #[getter]
    fn credit_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_model)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.stochastic_prepay_spec")]
    #[getter]
    fn stochastic_prepay_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_prepay_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.stochastic_default_spec")]
    #[getter]
    fn stochastic_default_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_default_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.stochastic_recovery_spec")]
    #[getter]
    fn stochastic_recovery_spec<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .stochastic_recovery_spec
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.correlation_structure")]
    #[getter]
    fn correlation_structure<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .correlation_structure
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.delinquency")]
    #[getter]
    fn delinquency<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .delinquency
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.card")]
    #[getter]
    fn card<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .credit_model
            .card
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.market_conditions")]
    #[getter]
    fn market_conditions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.market_conditions)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.deal_metadata")]
    #[getter]
    fn deal_metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.deal_metadata)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.hedge_swaps")]
    #[getter]
    fn hedge_swaps(&self) -> Vec<PyHedgeSwap> {
        self.inner
            .hedge_swaps
            .iter()
            .map(|hedge| PyHedgeSwap {
                inner: hedge.clone(),
            })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.fees")]
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fees
            .as_ref()
            .map(|fees| serde_to_py(py, fees))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.coverage_triggers")]
    #[getter]
    fn coverage_triggers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.coverage_triggers)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.loss_allocation")]
    #[getter]
    fn loss_allocation(&self) -> PyResult<Option<String>> {
        self.inner
            .loss_allocation
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.tranche_draws")]
    #[getter]
    fn tranche_draws<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tranche_draws)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.tranche_readvance")]
    #[getter]
    fn tranche_readvance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .tranche_readvance
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.loss_recognition")]
    #[getter]
    fn loss_recognition(&self) -> PyResult<Option<String>> {
        self.inner
            .loss_recognition
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.waterfall_rules")]
    #[getter]
    fn waterfall_rules<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .waterfall_rules
            .as_ref()
            .map(|rules| serde_to_py(py, rules))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        StructuredCredit::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.disable_stochastic")]
    #[pyo3(text_signature = "($self)")]
    fn disable_stochastic(&self) -> Self {
        let mut deal = self.inner.clone();
        deal.disable_stochastic();
        Self { inner: deal }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "StructuredCredit(id={:?}, deal_type={:?})",
            self.inner.id.as_str(),
            self.inner.deal_type
        )
    }
}

crate::bindings::macros::py_getters!(PyStructuredCredit |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.deal_type")]
    deal_type -> PyResult<String> = enum_to_py_string(&s.inner.deal_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.pool")]
    pool -> PyAssetPool = PyAssetPool { inner: s.inner.pool.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.tranches")]
    tranches -> PyTrancheStructure = PyTrancheStructure { inner: s.inner.tranches.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.frequency")]
    frequency -> PyTenor = PyTenor { inner: s.inner.frequency };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.prepayment_spec")]
    prepayment_spec -> PyPrepaymentModelSpec =
        PyPrepaymentModelSpec { inner: s.inner.credit_model.prepayment_spec.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.default_spec")]
    default_spec -> PyDefaultModelSpec =
        PyDefaultModelSpec { inner: s.inner.credit_model.default_spec.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.recovery_spec")]
    recovery_spec -> PyRecoveryModelSpec =
        PyRecoveryModelSpec { inner: s.inner.credit_model.recovery_spec.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.cleanup_call_decimal")]
    cleanup_call_decimal -> Option<f64> = s.inner.cleanup_call_decimal;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.call_assumption")]
    call_assumption -> Option<PyCallAssumption> =
        s.inner.call_assumption.clone().map(|inner| PyCallAssumption { inner });

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.liquidation_price_pct")]
    liquidation_price_pct -> Option<f64> = s.inner.liquidation_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.principal_covers_senior_interest")]
    principal_covers_senior_interest -> Option<bool> = s.inner.principal_covers_senior_interest;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.coverage_rules")]
    coverage_rules -> Option<PyCoverageRules> =
        s.inner.coverage_rules.clone().map(|inner| PyCoverageRules { inner });

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.waterfall")]
    waterfall -> Option<PyWaterfall> = s.inner.waterfall.clone().map(|inner| PyWaterfall { inner });

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.is_stochastic")]
    is_stochastic -> bool = s.inner.is_stochastic();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.effective_loss_allocation")]
    effective_loss_allocation -> PyResult<String> =
        enum_to_py_string(&s.inner.effective_loss_allocation());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.effective_loss_recognition")]
    effective_loss_recognition -> PyResult<String> =
        enum_to_py_string(&s.inner.effective_loss_recognition());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCredit.effective_principal_covers_senior_interest")]
    effective_principal_covers_senior_interest -> bool =
        s.inner.effective_principal_covers_senior_interest();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StructuredCreditBuilder",
    skip_from_py_object
)]
pub struct PyStructuredCreditBuilder {
    inner: Option<StructuredCreditBuilder>,
}

#[pymethods]
impl PyStructuredCreditBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.deal_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn deal_type<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let deal_type: DealType = enum_from_str(value, "deal_type")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.deal_type(deal_type))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.pool")]
    #[pyo3(text_signature = "($self, value)")]
    fn pool<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyAssetPool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .pool(value.inner.clone()))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.tranches")]
    #[pyo3(text_signature = "($self, value)")]
    fn tranches<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTrancheStructure>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .tranches(value.inner.clone()))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.closing_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn closing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.closing_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.first_payment_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn first_payment_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.first_payment_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.quote_settlement_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn quote_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .quote_settlement_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frequency = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.frequency(frequency))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .calendar_id(value.into()))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let business_day_convention: BusinessDayConvention =
            crate::bindings::valuations::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .business_day_convention(business_day_convention))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .discount_curve_id(CurveId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.market_conditions")]
    #[pyo3(text_signature = "($self, value)")]
    fn market_conditions<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let market_conditions: MarketConditions =
            crate::bindings::module_utils::py_to_serde(py, value, "market_conditions")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .market_conditions(market_conditions))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.waterfall_rules")]
    #[pyo3(text_signature = "($self, value)")]
    fn waterfall_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let waterfall_rules: WaterfallRules =
            crate::bindings::module_utils::py_to_serde(py, value, "waterfall_rules")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .waterfall_rules(waterfall_rules))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.fees")]
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: DealFees = crate::bindings::module_utils::py_to_serde(py, value, "fees")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.fees(fees))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.credit_model")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: CreditModelConfig =
            crate::bindings::module_utils::py_to_serde(py, value, "credit_model")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.credit_model(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.prepayment_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: PrepaymentModelSpec =
            if let Ok(typed) = value.cast::<PyPrepaymentModelSpec>() {
                typed.borrow().inner.clone()
            } else {
                crate::bindings::module_utils::py_to_serde(py, value, "prepayment_spec")?
            };
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .prepayment_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.default_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: DefaultModelSpec = if let Ok(typed) = value.cast::<PyDefaultModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "default_spec")?
        };
        builder_set!(slf, |b: StructuredCreditBuilder| b.default_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.recovery_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: RecoveryModelSpec = if let Ok(typed) = value.cast::<PyRecoveryModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "recovery_spec")?
        };
        builder_set!(slf, |b: StructuredCreditBuilder| b.recovery_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.stochastic_prepay_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_prepay_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_prepay_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .stochastic_prepay_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.stochastic_default_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_default_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .stochastic_default_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.stochastic_recovery_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn stochastic_recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "stochastic_recovery_spec")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .stochastic_recovery_spec(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.correlation_structure")]
    #[pyo3(text_signature = "($self, value)")]
    fn correlation_structure<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::module_utils::py_to_serde(py, value, "correlation_structure")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .correlation_structure(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.delinquency")]
    #[pyo3(text_signature = "($self, value)")]
    fn delinquency<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::DelinquencyModel =
            crate::bindings::module_utils::py_to_serde(py, value, "delinquency")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.delinquency(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.card")]
    #[pyo3(text_signature = "($self, value)")]
    fn card<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CardPortfolioSpec =
            crate::bindings::module_utils::py_to_serde(py, value, "card")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.card(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.coverage_triggers")]
    #[pyo3(text_signature = "($self, value)")]
    fn coverage_triggers<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: Vec<finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageTestSpec> =
            crate::bindings::module_utils::py_to_serde(py, value, "coverage_triggers")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .coverage_triggers(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.coverage_rules")]
    #[pyo3(text_signature = "($self, value)")]
    fn coverage_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageRules = if let Ok(typed) = value.cast::<PyCoverageRules>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "coverage_rules")?
        };
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .coverage_rules(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.call_assumption")]
    #[pyo3(text_signature = "($self, value)")]
    fn call_assumption<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::CallAssumption = if let Ok(typed) = value.cast::<PyCallAssumption>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "call_assumption")?
        };
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .call_assumption(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.waterfall")]
    #[pyo3(text_signature = "($self, value)")]
    fn waterfall<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: finstack_quant_valuations::instruments::fixed_income::structured_credit::Waterfall = if let Ok(typed) = value.cast::<PyWaterfall>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "waterfall")?
        };
        builder_set!(slf, |b: StructuredCreditBuilder| b.waterfall(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.hedge_swaps")]
    #[pyo3(text_signature = "($self, value)")]
    fn hedge_swaps<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let hedges = typed_list_from_py(py, value, "hedge_swaps", |hedge: &PyHedgeSwap| {
            hedge.inner.clone()
        })?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.hedge_swaps(hedges))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.cleanup_call_decimal")]
    #[pyo3(text_signature = "($self, value)")]
    fn cleanup_call_decimal<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .cleanup_call_decimal(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.liquidation_price_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn liquidation_price_pct<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .liquidation_price_pct(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.loss_allocation")]
    #[pyo3(text_signature = "($self, value)")]
    fn loss_allocation<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let policy: LossAllocationPolicy = enum_from_str(value, "loss_allocation")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.loss_allocation(policy))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.tranche_draws")]
    #[pyo3(text_signature = "($self, value)")]
    fn tranche_draws<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let draws: Vec<TrancheDraw> =
            crate::bindings::module_utils::py_to_serde(py, value, "tranche_draws")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.tranche_draws(draws))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.tranche_readvance")]
    #[pyo3(text_signature = "($self, value)")]
    fn tranche_readvance<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: TrancheReadvance =
            crate::bindings::module_utils::py_to_serde(py, value, "tranche_readvance")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.tranche_readvance(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.loss_recognition")]
    #[pyo3(text_signature = "($self, value)")]
    fn loss_recognition<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let timing: LossRecognition = enum_from_str(value, "loss_recognition")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.loss_recognition(timing))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.principal_covers_senior_interest")]
    #[pyo3(text_signature = "($self, value)")]
    fn principal_covers_senior_interest<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: StructuredCreditBuilder| b
            .principal_covers_senior_interest(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.deal_metadata")]
    #[pyo3(text_signature = "($self, value)")]
    fn deal_metadata<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let metadata: Metadata =
            crate::bindings::module_utils::py_to_serde(py, value, "deal_metadata")?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.deal_metadata(metadata))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit", "StructuredCreditBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(slf, |b: StructuredCreditBuilder| b.attributes(attributes))
    }
}
