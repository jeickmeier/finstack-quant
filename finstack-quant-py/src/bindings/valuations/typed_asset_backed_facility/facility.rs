//! `AssetBackedFacility`, `AssetBackedFacilityBuilder` and `FacilityProjection`.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, day_count_from_py, money_from_py, money_to_py,
};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::bindings::valuations::typed_structured_credit::{
    PyAssetPool, PySimulationDiagnostics, PyStructuredCredit, PyTrancheCashflows,
};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, AssetBackedFacility, FacilityProjection, TermOutSpec,
};
use finstack_quant_valuations::instruments::fixed_income::loan_terms::DrawEvent;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::BorrowingBaseRules;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CreditModelConfig;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::DealFees;

use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacilityBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetBackedFacility",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAssetBackedFacility {
    pub(crate) inner: AssetBackedFacility,
}

instrument_envelope_methods!(
    PyAssetBackedFacility("AssetBackedFacility"),
    AssetBackedFacility,
    builder = PyAssetBackedFacilityBuilder("AssetBackedFacilityBuilder", no_fields),
    seed = AssetBackedFacility::builder(),
);
instrument_pricing_methods!(PyAssetBackedFacility, "AssetBackedFacility");

#[pymethods]
impl PyAssetBackedFacility {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        AssetBackedFacility::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.borrowing_base")]
    #[pyo3(text_signature = "($self)")]
    fn borrowing_base<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let report = self.inner.borrowing_base().map_err(core_to_py)?;
        serde_to_py(py, &report)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.synthesized_deal")]
    #[pyo3(text_signature = "($self)")]
    fn synthesized_deal(&self) -> PyResult<PyStructuredCredit> {
        let inner = self.inner.synthesized_deal().map_err(core_to_py)?;
        Ok(PyStructuredCredit { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.project")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn project(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyFacilityProjection> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let facility = self.inner.clone();
        let inner = py
            .detach(move || facility.project(&market, as_of))
            .map_err(core_to_py)?;
        Ok(PyFacilityProjection { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.facility_irr")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn facility_irr(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner.facility_irr(&market, as_of).map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.borrowing_base_rules")]
    #[getter]
    fn borrowing_base_rules<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.borrowing_base_rules)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.rate")]
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.closing_date")]
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.revolving_end")]
    #[getter]
    fn revolving_end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.revolving_end)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.effective_revolving_end")]
    #[getter]
    fn effective_revolving_end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.effective_revolving_end())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.repayment_date")]
    #[getter]
    fn repayment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.repayment_date().map_err(core_to_py)?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.amortization_events")]
    #[getter]
    fn amortization_events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.amortization_events)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.term_out")]
    #[getter]
    fn term_out<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .term_out
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.fees")]
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fees
            .as_ref()
            .map(|fees| serde_to_py(py, fees))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.draws")]
    #[getter]
    fn draws<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.draws)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.credit_model")]
    #[getter]
    fn credit_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_model)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetBackedFacility(id={:?}, commitment={}, drawn={}, rate={}, revolving_end={})",
            self.inner.id.as_str(),
            self.inner.commitment.amount(),
            self.inner.drawn.amount(),
            serde_json::to_string(&self.inner.rate).unwrap_or_default(),
            self.inner.revolving_end
        )
    }
}

crate::bindings::macros::py_getters!(PyAssetBackedFacility |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.collateral")]
    collateral -> PyAssetPool = PyAssetPool { inner: s.inner.collateral.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.commitment")]
    commitment -> PyMoney = money_to_py(s.inner.commitment);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.drawn")]
    drawn -> PyMoney = money_to_py(s.inner.drawn);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.undrawn")]
    undrawn -> PyResult<PyMoney> = s.inner.undrawn().map(money_to_py).map_err(core_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.commitment_fee_bp")]
    commitment_fee_bp -> f64 =
        rust_decimal::prelude::ToPrimitive::to_f64(&s.inner.commitment_fee_bp).unwrap_or_default();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.frequency")]
    frequency -> PyTenor = PyTenor { inner: s.inner.frequency };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.day_count")]
    day_count -> PyDayCount = PyDayCount { inner: s.inner.day_count };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.liquidation_price_pct")]
    liquidation_price_pct -> Option<f64> = s.inner.liquidation_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.readvance_to_borrowing_base")]
    readvance_to_borrowing_base -> bool = s.inner.readvance_to_borrowing_base;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.prepayment_spec")]
    prepayment_spec -> PyPrepaymentModelSpec =
        PyPrepaymentModelSpec { inner: s.inner.credit_model.prepayment_spec.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.default_spec")]
    default_spec -> PyDefaultModelSpec =
        PyDefaultModelSpec { inner: s.inner.credit_model.default_spec.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacility.recovery_spec")]
    recovery_spec -> PyRecoveryModelSpec =
        PyRecoveryModelSpec { inner: s.inner.credit_model.recovery_spec.clone() };
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FacilityProjection",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFacilityProjection {
    pub(crate) inner: FacilityProjection,
}

#[pymethods]
impl PyFacilityProjection {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid FacilityProjection JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.commitment_fees")]
    #[getter]
    fn commitment_fees<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .commitment_fees
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.draws")]
    #[getter]
    fn draws<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .draws
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.lender_cashflows")]
    #[getter]
    fn lender_cashflows<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .lender_cashflows()
            .map_err(core_to_py)?
            .into_iter()
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FacilityProjection(payments={}, total_interest={}, total_principal={}, commitment_fees={})",
            self.inner.facility.cashflows.len(),
            self.inner.facility.total_interest.amount(),
            self.inner.facility.total_principal.amount(),
            self.inner.commitment_fees.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyFacilityProjection |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.facility")]
    facility -> PyTrancheCashflows = PyTrancheCashflows { inner: s.inner.facility.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.residual")]
    residual -> PyTrancheCashflows = PyTrancheCashflows { inner: s.inner.residual.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "FacilityProjection.diagnostics")]
    diagnostics -> PySimulationDiagnostics =
        PySimulationDiagnostics { inner: s.inner.diagnostics.clone() };
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetBackedFacilityBuilder",
    skip_from_py_object
)]
pub struct PyAssetBackedFacilityBuilder {
    inner: Option<AssetBackedFacilityBuilder>,
}

#[pymethods]
impl PyAssetBackedFacilityBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let converted = InstrumentId::new(value.to_string());
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.id(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.collateral")]
    #[pyo3(text_signature = "($self, value)")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyAssetPool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .collateral(value.inner.clone()))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.borrowing_base_rules")]
    #[pyo3(text_signature = "($self, value)")]
    fn borrowing_base_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rules: BorrowingBaseRules =
            crate::bindings::module_utils::py_to_serde(py, value, "borrowing_base_rules")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .borrowing_base_rules(rules))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.commitment")]
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn commitment<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "commitment")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.commitment(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.drawn")]
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn drawn<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "drawn")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.drawn(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: RateSpec =
            if value.extract::<&str>().is_ok() || value.cast::<pyo3::types::PyDict>().is_ok() {
                crate::bindings::module_utils::py_to_serde(py, value, "rate")?
            } else {
                let rate = super::super::convert::rate_from_py(value, "rate")?;
                RateSpec::fixed_rate(rate)
            };
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.rate(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.commitment_fee_bp")]
    #[pyo3(text_signature = "($self, value)")]
    fn commitment_fee_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::valuations::convert::decimal_from_f64(value, "commitment_fee_bp")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .commitment_fee_bp(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.closing_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn closing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.closing_date(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.revolving_end")]
    #[pyo3(text_signature = "($self, value)")]
    fn revolving_end<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.revolving_end(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.maturity(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.frequency(tenor))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.day_count(day_count))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value.to_string();
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .calendar_id(converted.into()))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.amortization_events")]
    #[pyo3(text_signature = "($self, value)")]
    fn amortization_events<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let events: Vec<AmortizationEvent> =
            crate::bindings::module_utils::py_to_serde(py, value, "amortization_events")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .amortization_events(events))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.term_out")]
    #[pyo3(text_signature = "($self, value)")]
    fn term_out<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted: TermOutSpec = if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "term_out: expected int or TermOutSpec, got bool",
            ));
        } else if let Ok(months) = value.extract::<u32>() {
            TermOutSpec { months }
        } else if let Ok(spec) = value.cast::<super::facility_terms::PyTermOutSpec>() {
            spec.borrow().inner
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "term_out")?
        };
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.term_out(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.fees")]
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: DealFees = crate::bindings::module_utils::py_to_serde(py, value, "fees")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.fees(fees))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.draws")]
    #[pyo3(text_signature = "($self, value)")]
    fn draws<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let draws: Vec<DrawEvent> = crate::bindings::module_utils::py_to_serde(py, value, "draws")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.draws(draws))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.readvance_to_borrowing_base")]
    #[pyo3(text_signature = "($self, value)")]
    fn readvance_to_borrowing_base<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .readvance_to_borrowing_base(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.liquidation_price_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn liquidation_price_pct<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .liquidation_price_pct(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = CurveId::new(value.to_string());
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .discount_curve_id(converted))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.credit_model")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let model: CreditModelConfig =
            crate::bindings::module_utils::py_to_serde(py, value, "credit_model")?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.credit_model(model))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.prepayment_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: PrepaymentModelSpec = if let Ok(typed) = value.cast::<PyPrepaymentModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "prepayment_spec")?
        };
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.prepayment_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.default_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: DefaultModelSpec = if let Ok(typed) = value.cast::<PyDefaultModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "default_spec")?
        };
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.default_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.recovery_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: RecoveryModelSpec = if let Ok(typed) = value.cast::<PyRecoveryModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "recovery_spec")?
        };
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b.recovery_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetBackedFacilityBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(slf, |b: AssetBackedFacilityBuilder| b
            .attributes(attributes))
    }
}
