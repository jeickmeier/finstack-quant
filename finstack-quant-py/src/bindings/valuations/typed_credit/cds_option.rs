//! CDS option (option on a CDS spread or clean index price) Python wrapper
//! and fluent builder.

use pyo3::prelude::*;

use crate::bindings::core::money::{decimal_from_py, decimal_to_py, is_python_decimal, PyMoney};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionStrike;
use finstack_quant_valuations::market::conventions::CdsConvention;
use rust_decimal::Decimal;

use super::super::convert::enum_from_str;
use super::super::convert::{
    attributes_from_py, bool_repr, date_repr, enum_to_py_string, float_repr, money_repr,
    money_to_py,
};
use super::super::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::bindings::module_utils::spec_from_py;

use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsOption {
    /// Inner canonical Rust CDS option.
    pub(crate) inner:
        finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption,
}

instrument_envelope_methods!(
    PyCdsOption("CdsOption"),
    CdsOption,
    builder = PyCdsOptionBuilder("CdsOptionBuilder", fields),
    seed =
        finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption::builder(
        ),
);
instrument_pricing_methods!(PyCdsOption, "CdsOption");

/// Optional date as ``datetime.date`` or ``None``.
fn opt_date_to_py<'py>(
    py: Python<'py>,
    date: Option<finstack_quant_core::dates::Date>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    date.map(|d| date_to_py(py, d)).transpose()
}

#[pymethods]
impl PyCdsOption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.strike")]
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.strike)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.underlying_maturity")]
    #[getter]
    fn underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.underlying_maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.exercise_settlement_date")]
    #[getter]
    fn exercise_settlement_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.exercise_settlement_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.underlying_start_date")]
    #[getter]
    fn underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.underlying_start_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.coupon_bp")]
    #[getter]
    fn coupon_bp<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .coupon_bp
            .map(|value| decimal_to_py(py, value))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsOption(id={:?}, option_type={:?}, expiry={}, underlying_maturity={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            date_repr(self.inner.expiry),
            date_repr(self.inner.underlying_maturity),
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsOption |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.option_type")]
    option_type -> PyResult<String> = enum_to_py_string(&s.inner.option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.protection_start_convention")]
    protection_start_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.protection_start_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.knockout")]
    knockout -> bool = s.inner.knockout;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.recovery_rate")]
    recovery_rate -> f64 = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.credit_curve_id")]
    credit_curve_id -> String = s.inner.credit_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.underlying_convention")]
    underlying_convention -> PyResult<String> = enum_to_py_string(&s.inner.underlying_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.underlying_is_index")]
    underlying_is_index -> bool = s.inner.underlying_is_index;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.index_factor")]
    index_factor -> f64 = s.inner.index_factor;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.strike_index_factor")]
    strike_index_factor -> Option<f64> = s.inner.strike_index_factor;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOption.realized_loss")]
    realized_loss -> f64 = s.inner.realized_loss;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsOptionBuilder",
    skip_from_py_object
)]
pub struct PyCdsOptionBuilder {
    inner: Option<CdsOptionBuilder>,
    fields: Vec<(&'static str, String)>,
}

/// Parse a ``decimal.Decimal | str | int | float | Bps | Rate`` in basis points.
fn decimal_arg(value: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if is_python_decimal(value)? {
        return decimal_from_py(value);
    }
    if value.is_instance_of::<pyo3::types::PyString>() {
        let text: String = value.extract()?;
        return finstack_quant_core::decimal::parse_decimal(&text).map_err(core_to_py);
    }
    let bp = crate::bindings::extract::extract_basis_points(value, "coupon_bp")?;
    super::super::convert::decimal_from_f64(bp, "coupon_bp")
}

#[pymethods]
impl PyCdsOptionBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CdsOptionBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.strike")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let strike: CdsOptionStrike = spec_from_py(py, value, "strike")?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, strike, shown, |b: CdsOptionBuilder| b.strike(strike))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.option_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "option_type")?;
        builder_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.option_type(parsed)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.expiry")]
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(slf, expiry, date_repr(date), |b: CdsOptionBuilder| b
            .expiry(date))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.underlying_maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            underlying_maturity,
            date_repr(date),
            |b: CdsOptionBuilder| b.underlying_maturity(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(slf, notional, money_repr(money), |b: CdsOptionBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.settlement(parsed)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.exercise_settlement_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            exercise_settlement_date,
            date_repr(date),
            |b: CdsOptionBuilder| b.exercise_settlement_date(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.underlying_start_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = py_to_date(value)?;
        builder_set!(
            slf,
            underlying_start_date,
            date_repr(date),
            |b: CdsOptionBuilder| b.underlying_start_date(date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.protection_start_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn protection_start_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "protection_start_convention")?;
        builder_set!(
            slf,
            protection_start_convention,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.protection_start_convention(parsed)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.knockout")]
    #[pyo3(text_signature = "($self, value)")]
    fn knockout<'py>(mut slf: PyRefMut<'py, Self>, value: bool) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            knockout,
            bool_repr(value).to_string(),
            |b: CdsOptionBuilder| b.knockout(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.recovery_rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            recovery_rate,
            float_repr(value),
            |b: CdsOptionBuilder| b.recovery_rate(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.discount_curve_id(CurveId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.credit_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.credit_curve_id(CurveId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.vol_surface_id(CurveId::new(value))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.underlying_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = enum_from_str::<CdsConvention>(value, "convention")?;
        builder_set!(
            slf,
            underlying_convention,
            format!("{value:?}"),
            |b: CdsOptionBuilder| b.underlying_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.underlying_is_index")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_is_index<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_is_index,
            bool_repr(value).to_string(),
            |b: CdsOptionBuilder| b.underlying_is_index(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.index_factor")]
    #[pyo3(text_signature = "($self, value)")]
    fn index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            index_factor,
            float_repr(value),
            |b: CdsOptionBuilder| b.index_factor(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.strike_index_factor")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike_index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            strike_index_factor,
            float_repr(value),
            |b: CdsOptionBuilder| b.strike_index_factor(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.realized_loss")]
    #[pyo3(text_signature = "($self, value)")]
    fn realized_loss<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            realized_loss,
            float_repr(value),
            |b: CdsOptionBuilder| b.realized_loss(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.coupon_bp")]
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon = decimal_arg(value)?;
        builder_set!(slf, coupon_bp, coupon.to_string(), |b: CdsOptionBuilder| b
            .coupon_bp(coupon))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CdsOptionBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: CdsOptionBuilder| b
            .attributes(attrs))
    }
}
