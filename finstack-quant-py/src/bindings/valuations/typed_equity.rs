//! Typed equity instruments: `EquityOption`.
//! Mirrors the `PyInterestRateSwap` pattern in `typed_rates.rs`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::equity::equity_option::{
    EquityOptionMarketData, EquityOptionParams,
};

use super::convert::enum_from_str;
use super::convert::{
    attributes_from_py, bool_repr, date_repr, day_count_from_py, enum_to_py_string, float_repr,
};
use super::pricing::typed_option_greeks;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

use finstack_quant_valuations::instruments::equity::equity_option::EquityOptionBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EquityOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityOption {
    /// Inner canonical Rust equity option.
    pub(crate) inner: finstack_quant_valuations::instruments::equity::equity_option::EquityOption,
}

instrument_envelope_methods!(
    PyEquityOption(
        "EquityOption",
        module = "finstack_quant.valuations.instruments._typed_equity"
    ),
    EquityOption,
    builder = PyEquityOptionBuilder("EquityOptionBuilder", fields),
    seed = finstack_quant_valuations::instruments::equity::equity_option::EquityOption::builder(),
);
instrument_pricing_methods!(
    PyEquityOption,
    "EquityOption",
    module = "finstack_quant.valuations.instruments._typed_equity"
);

#[pymethods]
impl PyEquityOption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::equity::equity_option::EquityOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.european")]
    #[staticmethod]
    #[pyo3(signature = (id, ticker, strike, expiry, quantity, currency, option_type, *, discount_curve_id="USD-OIS",
                        spot_id="EQUITY-SPOT", vol_surface_id="EQUITY-VOL",
                        div_yield_id="EQUITY-DIVYIELD"))]
    #[pyo3(
        text_signature = "(id, ticker, strike, expiry, quantity, currency, option_type, *, discount_curve_id='USD-OIS', \
spot_id='EQUITY-SPOT', vol_surface_id='EQUITY-VOL', div_yield_id='EQUITY-DIVYIELD')"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn european(
        id: &str,
        ticker: &str,
        strike: f64,
        expiry: &Bound<'_, PyAny>,
        quantity: f64,
        currency: &Bound<'_, PyAny>,
        option_type: &str,
        discount_curve_id: &str,
        spot_id: &str,
        vol_surface_id: &str,
        div_yield_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut market_data =
            EquityOptionMarketData::new(discount_curve_id, spot_id, vol_surface_id);
        if let Some(div) = div_yield_id {
            market_data = market_data.with_div_yield_id(div);
        }
        let option_params = EquityOptionParams::new(
            strike,
            py_to_date(expiry)?,
            enum_from_str(option_type, "option_type")?,
            quantity,
            crate::bindings::core::currency::extract_currency(currency)?,
        );
        let inner =
            finstack_quant_valuations::instruments::equity::equity_option::EquityOption::european_with_market_data(
                id,
                ticker,
                option_params,
                market_data,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.implied_vol")]
    #[pyo3(text_signature = "($self, market, as_of, target_price)")]
    fn implied_vol(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        target_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner
            .implied_vol(&market, as_of, target_price)
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.greeks")]
    #[pyo3(signature = (market, as_of, model="default"))]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        model: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        typed_option_greeks(py, Box::new(self.inner.clone()), market, as_of, model)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.exercise")]
    #[getter]
    fn exercise<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(exercise) = self.inner.exercise else {
            return Ok(None);
        };
        let out = PyDict::new(py);
        out.set_item("date", date_to_py(py, exercise.date)?)?;
        out.set_item("spot", exercise.spot)?;
        out.set_item("settlement_date", date_to_py(py, exercise.settlement_date)?)?;
        out.set_item("exercised", exercise.exercised)?;
        Ok(Some(out))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.discrete_dividends")]
    #[getter]
    fn discrete_dividends<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .discrete_dividends
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, *amount)))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.exercise_dates")]
    #[getter]
    fn exercise_dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        self.inner
            .exercise_dates
            .as_ref()
            .map(|dates| dates.iter().map(|d| date_to_py(py, *d)).collect())
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EquityOption(id={:?}, underlying_ticker={:?}, option_type={:?}, strike={}, expiry={}, exercise_style={:?}, quantity={}, currency={})",
            self.inner.id.as_str(),
            self.inner.underlying_ticker,
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            float_repr(self.inner.strike),
            date_repr(self.inner.expiry),
            enum_to_py_string(&self.inner.exercise_style).unwrap_or_default(),
            float_repr(self.inner.quantity),
            self.inner.currency,
        )
    }
}

crate::bindings::macros::py_getters!(PyEquityOption |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.underlying_ticker")]
    underlying_ticker -> String = s.inner.underlying_ticker.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.strike")]
    strike -> f64 = s.inner.strike;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.option_type")]
    option_type -> PyResult<String> = enum_to_py_string(&s.inner.option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.exercise_style")]
    exercise_style -> PyResult<String> = enum_to_py_string(&s.inner.exercise_style);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.quantity")]
    quantity -> f64 = s.inner.quantity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount =
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.spot_id")]
    spot_id -> String = s.inner.spot_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOption.div_yield_id")]
    div_yield_id -> Option<String> = s.inner.div_yield_id.as_ref().map(|id| id.to_string());
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EquityOptionBuilder",
    skip_from_py_object
)]
pub struct PyEquityOptionBuilder {
    inner: Option<EquityOptionBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyEquityOptionBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: EquityOptionBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.underlying_ticker")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_ticker,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.underlying_ticker(value.to_string())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.strike")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, strike, float_repr(value), |b: EquityOptionBuilder| b
            .strike(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.option_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let option_type = enum_from_str(value, "option_type")?;
        builder_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.option_type(option_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.exercise_style")]
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let exercise_style = enum_from_str(value, "exercise_style")?;
        builder_set!(
            slf,
            exercise_style,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.exercise_style(exercise_style)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.expiry")]
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = py_to_date(value)?;
        builder_set!(slf, expiry, date_repr(expiry), |b: EquityOptionBuilder| b
            .expiry(expiry))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.day_count")]
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
            |b: EquityOptionBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.settlement(settlement)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.exercise")]
    #[pyo3(text_signature = "($self, date, spot, settlement_date, exercised)")]
    fn exercise<'py>(
        mut slf: PyRefMut<'py, Self>,
        date: &Bound<'_, PyAny>,
        spot: f64,
        settlement_date: &Bound<'_, PyAny>,
        exercised: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let exercise = finstack_quant_valuations::instruments::equity::equity_option::EquityOptionExercise::new(
            py_to_date(date)?,
            spot,
            py_to_date(settlement_date)?,
            exercised,
        );
        let shown = format!(
            "({}, {}, {}, {})",
            date_repr(exercise.date),
            float_repr(exercise.spot),
            date_repr(exercise.settlement_date),
            bool_repr(exercise.exercised)
        );
        builder_set!(slf, exercise, shown, |b: EquityOptionBuilder| b
            .exercise(exercise))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.quantity")]
    #[pyo3(text_signature = "($self, value)")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            quantity,
            float_repr(value),
            |b: EquityOptionBuilder| b.quantity(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            currency,
            format!("{:?}", ccy.to_string()),
            |b: EquityOptionBuilder| b.currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.spot_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            spot_id,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.spot_id(PriceId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.div_yield_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            div_yield_id,
            format!("{value:?}"),
            |b: EquityOptionBuilder| b.div_yield_id(PriceId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.discrete_dividends")]
    #[pyo3(text_signature = "($self, value)")]
    fn discrete_dividends<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'py, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let dividends = value
            .into_iter()
            .map(|(date, amount)| Ok((py_to_date(&date)?, amount)))
            .collect::<PyResult<Vec<_>>>()?;
        let shown = format!(
            "[{}]",
            dividends
                .iter()
                .map(|(d, a)| format!("({}, {})", date_repr(*d), float_repr(*a)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        builder_set!(slf, discrete_dividends, shown, |b: EquityOptionBuilder| b
            .discrete_dividends(dividends))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.exercise_dates")]
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let dates = value.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        let shown = format!(
            "[{}]",
            dates
                .iter()
                .map(|d| date_repr(*d))
                .collect::<Vec<_>>()
                .join(", ")
        );
        builder_set!(slf, exercise_dates, shown, |b: EquityOptionBuilder| b
            .exercise_dates(dates))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_equity", "EquityOptionBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: EquityOptionBuilder| b
            .attributes(attrs))
    }
}

/// Register the typed equity instruments on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEquityOption>()?;
    m.add_class::<PyEquityOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &["EquityOption", "EquityOptionBuilder"];
