//! CDS tranche Python wrappers: `CdsTranche`, its fluent builder and the
//! `CdsTrancheParams` descriptor used by `CdsTranche.standard`.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::PyRollRule;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;

use super::super::convert::{
    attributes_from_py, bdc_from_py, date_repr, dated_money_from_py, day_count_from_py,
    enum_to_py_string, float_repr, money_from_py, money_repr, money_to_py,
};
use super::super::convert::{enum_from_str, stub_kind_from_py};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::bindings::extract::extract_basis_points;

use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsTrancheParams",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsTrancheParams {
    /// Inner canonical Rust tranche parameters.
    pub(crate) inner: CdsTrancheParams,
}

#[pymethods]
impl PyCdsTrancheParams {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.__init__")]
    #[new]
    #[pyo3(signature = (index_name, series, attach_pct, detach_pct, notional, maturity, coupon_bp, realized_loss=0.0))]
    #[pyo3(
        text_signature = "(index_name, series, attach_pct, detach_pct, notional, maturity, coupon_bp, realized_loss=0.0)"
    )]
    // PyO3 binding: the argument list mirrors the Rust constructor one-for-one.
    #[allow(clippy::too_many_arguments)]
    fn new(
        index_name: &str,
        series: u16,
        attach_pct: f64,
        detach_pct: f64,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
        realized_loss: f64,
    ) -> PyResult<Self> {
        let inner = CdsTrancheParams::new(
            index_name,
            series,
            attach_pct,
            detach_pct,
            money_from_py(notional, None, "notional")?,
            py_to_date(maturity)?,
            extract_basis_points(coupon_bp, "coupon_bp")?,
        )
        .with_realized_loss(realized_loss)
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.equity_tranche")]
    #[staticmethod]
    #[pyo3(text_signature = "(index_name, series, notional, maturity, coupon_bp)")]
    fn equity_tranche(
        index_name: &str,
        series: u16,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CdsTrancheParams::equity_tranche(
                index_name,
                series,
                money_from_py(notional, None, "notional")?,
                py_to_date(maturity)?,
                extract_basis_points(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.mezzanine_tranche")]
    #[staticmethod]
    #[pyo3(text_signature = "(index_name, series, notional, maturity, coupon_bp)")]
    fn mezzanine_tranche(
        index_name: &str,
        series: u16,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CdsTrancheParams::mezzanine_tranche(
                index_name,
                series,
                money_from_py(notional, None, "notional")?,
                py_to_date(maturity)?,
                extract_basis_points(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsTrancheParams(index_name={:?}, series={}, attach_pct={}, detach_pct={}, notional={}, maturity={}, coupon_bp={}, realized_loss={})",
            self.inner.index_name,
            self.inner.series,
            float_repr(self.inner.attach_pct),
            float_repr(self.inner.detach_pct),
            money_repr(self.inner.notional),
            date_repr(self.inner.maturity),
            float_repr(self.inner.coupon_bp),
            float_repr(self.inner.realized_loss),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsTrancheParams |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.index_name")]
    index_name -> String = s.inner.index_name.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.series")]
    series -> u16 = s.inner.series;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.attach_pct")]
    attach_pct -> f64 = s.inner.attach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.detach_pct")]
    detach_pct -> f64 = s.inner.detach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.coupon_bp")]
    coupon_bp -> f64 = s.inner.coupon_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheParams.realized_loss")]
    realized_loss -> f64 = s.inner.realized_loss;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsTranche",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsTranche {
    /// Inner canonical Rust CDS tranche.
    pub(crate) inner:
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche,
}

instrument_envelope_methods!(
    PyCdsTranche("CdsTranche", module = "finstack_quant.valuations.instruments._typed_credit.cds_tranche"),
    CdsTranche,
    builder = PyCdsTrancheBuilder("CdsTrancheBuilder", fields),
    seed = finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche::builder().realized_loss(0.0),
);
instrument_pricing_methods!(
    PyCdsTranche,
    "CdsTranche",
    module = "finstack_quant.valuations.instruments._typed_credit.cds_tranche"
);

#[pymethods]
impl PyCdsTranche {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.standard")]
    #[staticmethod]
    #[pyo3(text_signature = "(id, params, discount_curve_id, credit_index_id, side)")]
    fn standard(
        id: &str,
        params: PyRef<'_, PyCdsTrancheParams>,
        discount_curve_id: &str,
        credit_index_id: &str,
        side: &str,
    ) -> PyResult<Self> {
        let side: PayReceive = enum_from_str(side, "side")?;
        let inner = finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche::standard(
            InstrumentId::new(id.to_string()),
            &params.inner,
            CurveId::new(discount_curve_id.to_string()),
            CurveId::new(credit_index_id.to_string()),
            side,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.start_date.map(|d| date_to_py(py, d)).transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.upfront")]
    #[getter]
    fn upfront<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .upfront
            .map(|(date, money)| Ok((date_to_py(py, date)?, money_to_py(money))))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsTranche(id={:?}, index_name={:?}, series={}, attach_pct={}, detach_pct={}, side={:?}, notional={}, coupon_bp={}, maturity={})",
            self.inner.id.as_str(),
            self.inner.index_name,
            self.inner.series,
            float_repr(self.inner.attach_pct),
            float_repr(self.inner.detach_pct),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            float_repr(self.inner.coupon_bp),
            date_repr(self.inner.maturity),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsTranche |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.index_name")]
    index_name -> String = s.inner.index_name.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.series")]
    series -> u16 = s.inner.series;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.attach_pct")]
    attach_pct -> f64 = s.inner.attach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.detach_pct")]
    detach_pct -> f64 = s.inner.detach_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.coupon_bp")]
    coupon_bp -> f64 = s.inner.coupon_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.credit_index_id")]
    credit_index_id -> String = s.inner.credit_index_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.side")]
    side -> PyResult<String> = enum_to_py_string(&s.inner.side);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.realized_loss")]
    realized_loss -> f64 = s.inner.realized_loss;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.roll_rule")]
    roll_rule -> PyRollRule = PyRollRule { inner: s.inner.roll_rule };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTranche.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsTrancheBuilder",
    skip_from_py_object
)]
pub struct PyCdsTrancheBuilder {
    inner: Option<CdsTrancheBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyCdsTrancheBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CdsTrancheBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.index_name")]
    #[pyo3(text_signature = "($self, value)")]
    fn index_name<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            index_name,
            format!("{value:?}"),
            |b: CdsTrancheBuilder| b.index_name(value.to_string())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.series")]
    #[pyo3(text_signature = "($self, value)")]
    fn series<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, series, value.to_string(), |b: CdsTrancheBuilder| b
            .series(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.attach_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn attach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            attach_pct,
            float_repr(value),
            |b: CdsTrancheBuilder| b.attach_pct(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.detach_pct")]
    #[pyo3(text_signature = "($self, value)")]
    fn detach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            detach_pct,
            float_repr(value),
            |b: CdsTrancheBuilder| b.detach_pct(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(slf, notional, money_repr(money), |b: CdsTrancheBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(
            slf,
            maturity,
            date_repr(maturity),
            |b: CdsTrancheBuilder| b.maturity(maturity)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.coupon_bp")]
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let bp = extract_basis_points(value, "coupon_bp")?;
        builder_set!(slf, coupon_bp, float_repr(bp), |b: CdsTrancheBuilder| b
            .coupon_bp(bp))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(
            slf,
            frequency,
            format!("Tenor({:?})", tenor.to_string()),
            |b: CdsTrancheBuilder| b.frequency(tenor)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            format!("DayCount('{day_count}')"),
            |b: CdsTrancheBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = bdc_from_py(value, "business_day_convention")?;
        let shown = enum_to_py_string(&convention).map(|s| format!("{s:?}"))?;
        builder_set!(
            slf,
            business_day_convention,
            shown,
            |b: CdsTrancheBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilder| b.calendar_id(value.into())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.credit_index_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_index_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilder| b.credit_index_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.side")]
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side: PayReceive = enum_from_str(value, "side")?;
        builder_set!(slf, side, format!("{value:?}"), |b: CdsTrancheBuilder| b
            .side(side))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.start_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let start_date = py_to_date(value)?;
        builder_set!(
            slf,
            start_date,
            date_repr(start_date),
            |b: CdsTrancheBuilder| b.start_date(start_date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.realized_loss")]
    #[pyo3(text_signature = "($self, value)")]
    fn realized_loss<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            realized_loss,
            float_repr(value),
            |b: CdsTrancheBuilder| b.realized_loss(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.roll_rule")]
    #[pyo3(text_signature = "($self, value)")]
    fn roll_rule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyRollRule>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rule = value.inner;
        builder_set!(
            slf,
            roll_rule,
            format!("{rule:?}"),
            |b: CdsTrancheBuilder| b.roll_rule(rule)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.stub")]
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        builder_set!(slf, stub, format!("{stub:?}"), |b: CdsTrancheBuilder| b
            .stub(stub))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.upfront")]
    #[pyo3(text_signature = "($self, value)")]
    fn upfront<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (date, money) = dated_money_from_py(value, None, "upfront")?;
        let shown = format!("({}, {})", date_repr(date), money_repr(money));
        builder_set!(slf, upfront, shown, |b: CdsTrancheBuilder| b
            .upfront((date, money)))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_credit.cds_tranche", "CdsTrancheBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: CdsTrancheBuilder| b
            .attributes(attrs))
    }
}

/// Register the tranche helper classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCdsTrancheParams>()?;
    Ok(())
}
