//! CDS index Python wrappers: `CdsIndex`, its fluent builder, the
//! `CdsIndexParams` preset descriptor and the `CdsIndexConstituent` row.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CdsIndexConstituent, CdsIndexParams, IndexPricing,
};
use finstack_quant_valuations::instruments::{CreditParams, Instrument};
use finstack_quant_valuations::market::conventions::CdsConvention;
use rust_decimal::prelude::ToPrimitive;

use super::super::convert::enum_from_str;
use super::super::convert::{
    attributes_from_py, bool_repr, date_repr, dated_money_from_py, enum_to_py_string, float_repr,
    money_from_py, money_repr, money_to_py, opt_repr, typed_list_from_py,
};
use super::super::typed_legs::{PyPremiumLegSpec, PyProtectionLegSpec};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::bindings::extract::extract_basis_points;

use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsIndexParams",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCdsIndexParams {
    /// Inner canonical Rust preset.
    pub(crate) inner: CdsIndexParams,
}

#[pymethods]
impl PyCdsIndexParams {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.__init__")]
    #[new]
    #[pyo3(signature = (index_name, series, version, coupon_bp, convention="isda_na", num_constituents=None))]
    #[pyo3(
        text_signature = "(index_name, series, version, coupon_bp, convention='isda_na', num_constituents=None)"
    )]
    fn new(
        index_name: &str,
        series: u16,
        version: u16,
        coupon_bp: &Bound<'_, PyAny>,
        convention: &str,
        num_constituents: Option<u32>,
    ) -> PyResult<Self> {
        let mut inner = CdsIndexParams::new(
            index_name,
            series,
            version,
            extract_basis_points(coupon_bp, "coupon_bp")?,
            enum_from_str::<CdsConvention>(convention, "convention")?,
        );
        if let Some(n) = num_constituents {
            inner = inner.with_num_constituents(n);
        }
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.cdx_na_ig")]
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, coupon_bp)")]
    fn cdx_na_ig(series: u16, version: u16, coupon_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CdsIndexParams::cdx_na_ig(
                series,
                version,
                extract_basis_points(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.cdx_na_hy")]
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, coupon_bp)")]
    fn cdx_na_hy(series: u16, version: u16, coupon_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CdsIndexParams::cdx_na_hy(
                series,
                version,
                extract_basis_points(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.itraxx_europe")]
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, coupon_bp)")]
    fn itraxx_europe(series: u16, version: u16, coupon_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CdsIndexParams::itraxx_europe(
                series,
                version,
                extract_basis_points(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsIndexParams(index_name={:?}, series={}, version={}, coupon_bp={}, convention={:?}, num_constituents={})",
            self.inner.index_name,
            self.inner.series,
            self.inner.version,
            float_repr(self.inner.coupon_bp),
            enum_to_py_string(&self.inner.convention).unwrap_or_default(),
            opt_repr(self.inner.num_constituents),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsIndexParams |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.index_name")]
    index_name -> String = s.inner.index_name.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.series")]
    series -> u16 = s.inner.series;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.version")]
    version -> u16 = s.inner.version;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.coupon_bp")]
    coupon_bp -> f64 = s.inner.coupon_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.convention")]
    convention -> PyResult<String> = enum_to_py_string(&s.inner.convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexParams.num_constituents")]
    num_constituents -> Option<u32> = s.inner.num_constituents;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsIndexConstituent",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsIndexConstituent {
    /// Inner canonical Rust constituent.
    pub(crate) inner: CdsIndexConstituent,
}

#[pymethods]
impl PyCdsIndexConstituent {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.__init__")]
    #[new]
    #[pyo3(signature = (reference_entity, recovery_rate, credit_curve_id, weight, defaulted=false))]
    #[pyo3(
        text_signature = "(reference_entity, recovery_rate, credit_curve_id, weight, defaulted=False)"
    )]
    fn new(
        reference_entity: &str,
        recovery_rate: f64,
        credit_curve_id: &str,
        weight: f64,
        defaulted: bool,
    ) -> Self {
        Self {
            inner: CdsIndexConstituent {
                credit: CreditParams::new(
                    reference_entity,
                    recovery_rate,
                    CurveId::new(credit_curve_id.to_string()),
                ),
                weight,
                defaulted,
            },
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsIndexConstituent(reference_entity={:?}, recovery_rate={}, credit_curve_id={:?}, weight={}, defaulted={})",
            self.inner.credit.reference_entity,
            float_repr(self.inner.credit.recovery_rate),
            self.inner.credit.credit_curve_id.as_str(),
            float_repr(self.inner.weight),
            bool_repr(self.inner.defaulted),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsIndexConstituent |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.reference_entity")]
    reference_entity -> String = s.inner.credit.reference_entity.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.recovery_rate")]
    recovery_rate -> f64 = s.inner.credit.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.credit_curve_id")]
    credit_curve_id -> String = s.inner.credit.credit_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.weight")]
    weight -> f64 = s.inner.weight;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexConstituent.defaulted")]
    defaulted -> bool = s.inner.defaulted;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsIndex",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsIndex {
    /// Inner canonical Rust CDS index.
    pub(crate) inner:
        finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex,
}

instrument_envelope_methods!(
    PyCdsIndex("CdsIndex"),
    CdsIndex,
    builder = PyCdsIndexBuilder("CdsIndexBuilder", fields),
    seed =
        finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex::builder()
            .constituents(Vec::new()),
);
instrument_pricing_methods!(PyCdsIndex, "CdsIndex");

#[pymethods]
impl PyCdsIndex {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.from_preset")]
    #[staticmethod]
    #[pyo3(
        text_signature = "(preset, id, notional, side, start, end, recovery_rate, discount_curve_id, credit_curve_id)"
    )]
    // PyO3 binding: the argument list mirrors the Rust constructor one-for-one.
    #[allow(clippy::too_many_arguments)]
    fn from_preset(
        preset: PyRef<'_, PyCdsIndexParams>,
        id: &str,
        notional: &Bound<'_, PyAny>,
        side: &str,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        recovery_rate: f64,
        discount_curve_id: &str,
        credit_curve_id: &str,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex::from_preset(
            &preset.inner,
            InstrumentId::new(id.to_string()),
            money_from_py(notional, None, "notional")?,
            enum_from_str(side, "side")?,
            py_to_date(start)?,
            py_to_date(end)?,
            recovery_rate,
            CurveId::new(discount_curve_id.to_string()),
            CurveId::new(credit_curve_id.to_string()),
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.par_spread")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn par_spread(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner.par_spread(&market, as_of).map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.risky_pv01")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn risky_pv01(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner.risky_pv01(&market, as_of).map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.cs01")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn cs01(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let provider =
            finstack_quant_calibration::recalibration::CachedRecalibrationProvider::default();
        self.inner
            .cs01(&market, as_of, &provider)
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.constituents")]
    #[getter]
    fn constituents(&self) -> Vec<PyCdsIndexConstituent> {
        self.inner
            .constituents
            .iter()
            .map(|row| PyCdsIndexConstituent { inner: row.clone() })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.upfront")]
    #[getter]
    fn upfront<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .upfront
            .map(|(date, money)| Ok((date_to_py(py, date)?, money_to_py(money))))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .margin_spec
            .as_ref()
            .map(|spec| crate::bindings::pandas_utils::serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsIndex(id={:?}, index_name={:?}, series={}, side={:?}, notional={}, coupon_bp={}, end={}, pricing={:?})",
            self.inner.id.as_str(),
            self.inner.index_name,
            self.inner.series,
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            opt_repr(self.inner.premium_leg.coupon_bp.to_f64()),
            date_repr(self.inner.premium_leg.end),
            enum_to_py_string(&self.inner.pricing).unwrap_or_default(),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsIndex |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.index_name")]
    index_name -> String = s.inner.index_name.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.series")]
    series -> u16 = s.inner.series;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.version")]
    version -> u16 = s.inner.version;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.index_factor")]
    index_factor -> f64 = s.inner.index_factor;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.side")]
    side -> PyResult<String> = enum_to_py_string(&s.inner.side);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.convention")]
    convention -> PyResult<String> = enum_to_py_string(&s.inner.convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.premium_leg")]
    premium_leg -> PyPremiumLegSpec = PyPremiumLegSpec { inner: s.inner.premium_leg.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.protection_leg")]
    protection_leg -> PyProtectionLegSpec =
        PyProtectionLegSpec { inner: s.inner.protection_leg.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.pricing")]
    pricing -> PyResult<String> = enum_to_py_string(&s.inner.pricing);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndex.num_constituents")]
    num_constituents -> Option<u32> = s.inner.num_constituents;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsIndexBuilder",
    skip_from_py_object
)]
pub struct PyCdsIndexBuilder {
    inner: Option<CdsIndexBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyCdsIndexBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CdsIndexBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.index_name")]
    #[pyo3(text_signature = "($self, value)")]
    fn index_name<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            index_name,
            format!("{value:?}"),
            |b: CdsIndexBuilder| b.index_name(value.to_string())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.series")]
    #[pyo3(text_signature = "($self, value)")]
    fn series<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, series, value.to_string(), |b: CdsIndexBuilder| b
            .series(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.version")]
    #[pyo3(text_signature = "($self, value)")]
    fn version<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, version, value.to_string(), |b: CdsIndexBuilder| b
            .version(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(slf, notional, money_repr(money), |b: CdsIndexBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.index_factor")]
    #[pyo3(text_signature = "($self, value)")]
    fn index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            index_factor,
            float_repr(value),
            |b: CdsIndexBuilder| b.index_factor(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.side")]
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        builder_set!(slf, side, format!("{value:?}"), |b: CdsIndexBuilder| b
            .side(side))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn convention<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let convention = enum_from_str::<CdsConvention>(value, "convention")?;
        builder_set!(
            slf,
            convention,
            format!("{value:?}"),
            |b: CdsIndexBuilder| b.convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.premium_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn premium_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyPremiumLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "PremiumLegSpec(coupon_bp={}, start={}, end={})",
            leg.coupon_bp,
            date_repr(leg.start),
            date_repr(leg.end)
        );
        builder_set!(slf, premium_leg, shown, |b: CdsIndexBuilder| b
            .premium_leg(leg))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.protection_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn protection_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyProtectionLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "ProtectionLegSpec(credit_curve_id={:?}, recovery_rate={})",
            leg.credit_curve_id.as_str(),
            leg.recovery_rate
        );
        builder_set!(slf, protection_leg, shown, |b: CdsIndexBuilder| b
            .protection_leg(leg))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.pricing")]
    #[pyo3(text_signature = "($self, value)")]
    fn pricing<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let pricing: IndexPricing = enum_from_str(value, "pricing")?;
        builder_set!(slf, pricing, format!("{value:?}"), |b: CdsIndexBuilder| b
            .pricing(pricing))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.constituents")]
    #[pyo3(text_signature = "($self, value)")]
    fn constituents<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let constituents =
            typed_list_from_py(py, value, "constituents", |row: &PyCdsIndexConstituent| {
                row.inner.clone()
            })?;
        let shown = format!("<{} constituents>", constituents.len());
        builder_set!(slf, constituents, shown, |b: CdsIndexBuilder| b
            .constituents(constituents))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.num_constituents")]
    #[pyo3(text_signature = "($self, value)")]
    fn num_constituents<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            num_constituents,
            value.to_string(),
            |b: CdsIndexBuilder| b.num_constituents(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.upfront")]
    #[pyo3(text_signature = "($self, value)")]
    fn upfront<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (date, money) = dated_money_from_py(value, None, "upfront")?;
        let shown = format!("({}, {})", date_repr(date), money_repr(money));
        builder_set!(slf, upfront, shown, |b: CdsIndexBuilder| b
            .upfront((date, money)))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.margin_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::module_utils::py_to_serde(py, value, "margin_spec")?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, margin_spec, shown, |b: CdsIndexBuilder| b
            .margin_spec(spec))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsIndexBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: CdsIndexBuilder| b
            .attributes(attrs))
    }
}

/// Register the CDS-index helper classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCdsIndexParams>()?;
    m.add_class::<PyCdsIndexConstituent>()?;
    Ok(())
}

crate::bindings::macros::wire_methods!(
    PyCdsIndexConstituent,
    CdsIndexConstituent,
    "CdsIndexConstituent",
    module = "finstack_quant.valuations.instruments"
);
