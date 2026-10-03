//! CDS option (option on a CDS spread or clean index price) Python wrapper
//! and fluent builder.

use pyo3::prelude::*;

use crate::bindings::core::money::{decimal_from_py, decimal_to_py, is_python_decimal, PyMoney};
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionStrike;
use finstack_quant_valuations::instruments::InstrumentJson;
use rust_decimal::Decimal;

use super::super::convert::{
    attributes_from_py, bool_repr, builder_repr, date_repr, enum_to_py_string, float_repr,
    money_repr, money_to_py,
};
use super::super::instruments::{enum_from_str, serialize_typed_instrument_json};
use super::super::typed_fx::{
    instrument_envelope_methods, instrument_pricing_methods, take_builder,
};
use super::cds::cds_convention_from_str;

type CdsOptionBuilderInner =
    finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionBuilder;

/// European option on a single-name or index CDS (typed wrapper for Rust ``CdsOption``).
///
/// The strike is either a forward spread (``{"spread": "0.0325"}``, decimal
/// rate) or a clean index price in percentage points
/// (``{"clean_price_pct": "107.0"}``). A call is the right to buy protection
/// (payer), a put the right to sell it (receiver). Pricing uses the
/// Bloomberg CDSO numerical-quadrature model; ``delta``, ``gamma``, ``vega``,
/// ``theta`` and ``implied_vol`` are metric ids of ``price``/``metric``.
///
/// Build with ``CdsOption.builder()`` or start from ``CdsOption.example()``;
/// instances are accepted directly by ``price_instrument``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CdsOption
/// >>> option = CdsOption.example()
/// >>> (option.id, option.option_type, option.strike, option.vol_surface_id)
/// ('CDSOPT-CALL-CORP-5Y', 'call', {'spread': '0.01'}, 'CDSOPT-VOL')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsOption {
    /// Inner canonical Rust CDS option.
    pub(crate) inner: finstack_quant_valuations::instruments::CdsOption,
}

impl PyCdsOption {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::CdsOption(self.inner.clone()), "CdsOption")
    }
}

instrument_envelope_methods!(
    PyCdsOption,
    CdsOption,
    "cds_option",
    PyCdsOptionBuilder,
    finstack_quant_valuations::instruments::CdsOption::builder()
);
instrument_pricing_methods!(PyCdsOption);

/// Optional date as ``datetime.date`` or ``None``.
fn opt_date_to_py<'py>(
    py: Python<'py>,
    date: Option<time::Date>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    date.map(|d| date_to_py(py, d)).transpose()
}

#[pymethods]
impl PyCdsOption {
    /// Canonical example: 100bp-strike call on a 5-year USD 10,000,000 corporate CDS.
    ///
    /// Mirrors Rust ``CdsOption::example()``: expiry 2025-06-20, CDS maturity
    /// 2030-06-20, cash settlement, 40% recovery, curves ``USD-OIS`` /
    /// ``CORP-HAZARD`` and vol surface ``CDSOPT-VOL``.
    ///
    /// Returns
    /// -------
    /// CdsOption
    ///     The validated example option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the example option fails validation.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::CdsOption::example()
                .map_err(core_to_py)?,
        })
    }

    /// Strike as a dict: ``{"spread": "<decimal rate>"}`` or
    /// ``{"clean_price_pct": "<price points>"}`` (decimal strings).
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.strike)
    }

    /// ``"call"`` (right to buy protection) or ``"put"`` (right to sell protection).
    #[getter]
    fn option_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.option_type)
    }

    /// Exercise style (only ``"european"`` prices).
    #[getter]
    fn exercise_style(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.exercise_style)
    }

    /// Option expiry date.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    /// Maturity date of the underlying CDS.
    #[getter]
    fn underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.underlying_maturity)
    }

    /// Option notional.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Settlement type: ``"cash"`` or ``"physical"``.
    #[getter]
    fn settlement(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.settlement)
    }

    /// Option premium payment date, or ``None`` for the convention default.
    #[getter]
    fn premium_settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.premium_settlement_date)
    }

    /// Exercise proceeds payment date, or ``None`` for legal expiry.
    #[getter]
    fn exercise_settlement_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.exercise_settlement_date)
    }

    /// Explicit accrual-effective date of the underlying CDS, or ``None``.
    #[getter]
    fn underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.underlying_start_date)
    }

    /// Underlying accrual-start convention: ``"spot"`` or ``"forward"``.
    #[getter]
    fn protection_start_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.protection_start_convention)
    }

    /// Whether the option knocks out if the reference entity defaults before expiry.
    #[getter]
    fn knockout(&self) -> bool {
        self.inner.knockout
    }

    /// Recovery rate assumption as a decimal (``0.4`` = 40%).
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Hazard (credit) curve identifier.
    #[getter]
    fn credit_curve_id(&self) -> String {
        self.inner.credit_curve_id.to_string()
    }

    /// Volatility surface identifier.
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    /// ISDA convention of the underlying CDS (``"isda_na"``, ``"isda_eu"``, ...).
    #[getter]
    fn underlying_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.underlying_convention)
    }

    /// ``True`` when the underlying is a CDS index rather than a single name.
    #[getter]
    fn underlying_is_index(&self) -> bool {
        self.inner.underlying_is_index
    }

    /// Current index factor ``f`` in ``(0, 1]``.
    #[getter]
    fn index_factor(&self) -> f64 {
        self.inner.index_factor
    }

    /// Original index factor ``f0`` of a clean-price strike, or ``None``.
    #[getter]
    fn strike_index_factor(&self) -> Option<f64> {
        self.inner.strike_index_factor
    }

    /// Settled cumulative index loss since inception, as a decimal of original notional.
    #[getter]
    fn realized_loss(&self) -> f64 {
        self.inner.realized_loss
    }

    /// Running coupon of the underlying CDS in basis points as ``decimal.Decimal``,
    /// or ``None`` (the strike spread is then the coupon).
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

/// Fluent builder for ``CdsOption``; wraps the Rust ``FinancialBuilder``
/// builder (consuming setters).
///
/// Required: ``id``, ``strike``, ``option_type``, ``exercise_style``,
/// ``expiry``, ``underlying_maturity``, ``notional``, ``settlement``,
/// ``recovery_rate``, ``discount_curve_id``, ``credit_curve_id``,
/// ``vol_surface_id`` and ``underlying_is_index``. Builders are consumed by
/// ``build()``; create a new builder per instrument.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import CdsOption
/// >>> option = (CdsOption.builder().id("CDSO-1").strike({"spread": "0.0125"})
/// ...     .option_type("put").exercise_style("european")
/// ...     .expiry(datetime.date(2025, 6, 20)).underlying_maturity(datetime.date(2030, 6, 20))
/// ...     .notional(Money(5_000_000, Currency("USD"))).settlement("physical")
/// ...     .recovery_rate(0.4).discount_curve_id("USD-OIS").credit_curve_id("CORP-HAZARD")
/// ...     .vol_surface_id("CDSOPT-VOL").underlying_is_index(False).build())
/// >>> (option.option_type, option.settlement, option.strike)
/// ('put', 'physical', {'spread': '0.0125'})
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CdsOptionBuilder",
    skip_from_py_object
)]
pub struct PyCdsOptionBuilder {
    inner: Option<CdsOptionBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! cdso_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

/// Parse a ``decimal.Decimal | str | int | float`` exactly, via its string form.
fn decimal_arg(value: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if is_python_decimal(value)? {
        return decimal_from_py(value);
    }
    let text: String = value.str()?.extract()?;
    finstack_quant_core::decimal::parse_decimal(&text).map_err(core_to_py)
}

#[pymethods]
impl PyCdsOptionBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the option.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(slf, id, format!("{value:?}"), |b: CdsOptionBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the option strike as a forward spread or a clean index price.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``{"spread": "0.0325"}`` (forward spread as a decimal rate) or
    ///     ``{"clean_price_pct": "107.0"}`` (clean price in percentage
    ///     points), as a dict or JSON text.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the two strike forms.
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let strike: CdsOptionStrike = if let Ok(text) = value.extract::<String>() {
            serde_json::from_str(&text)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid strike JSON"))?
        } else {
            py_to_serde(py, value, "strike")?
        };
        let shown = value.repr()?.to_string();
        cdso_set!(slf, strike, shown, |b: CdsOptionBuilderInner| b
            .strike(strike))
    }

    /// Set the option type.
    ///
    /// Parameters
    /// ----------
    /// value : {"call", "put"}
    ///     ``"call"`` buys protection at expiry, ``"put"`` sells it.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized option type.
    #[pyo3(text_signature = "($self, value)")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "option_type")?;
        cdso_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.option_type(parsed)
        )
    }

    /// Set the exercise style.
    ///
    /// Parameters
    /// ----------
    /// value : {"european", "american", "bermudan"}
    ///     Exercise style; pricing supports ``"european"`` only.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized exercise style.
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "exercise_style")?;
        cdso_set!(
            slf,
            exercise_style,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.exercise_style(parsed)
        )
    }

    /// Set the option expiry date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Legal expiry; must precede ``underlying_maturity``.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cdso_set!(slf, expiry, date_repr(date), |b: CdsOptionBuilderInner| b
            .expiry(date))
    }

    /// Set the underlying CDS maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Maturity of the CDS delivered or cash-settled at exercise.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cdso_set!(
            slf,
            underlying_maturity,
            date_repr(date),
            |b: CdsOptionBuilderInner| b.underlying_maturity(date)
        )
    }

    /// Set the option notional.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Positive notional of the underlying CDS.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        cdso_set!(
            slf,
            notional,
            money_repr(money),
            |b: CdsOptionBuilderInner| b.notional(money)
        )
    }

    /// Set the settlement type.
    ///
    /// Parameters
    /// ----------
    /// value : {"cash", "physical"}
    ///     Settlement of the exercise proceeds.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized settlement type.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "settlement")?;
        cdso_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.settlement(parsed)
        )
    }

    /// Set the option premium payment date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Premium payment date; when never set, the underlying CDS
    ///     convention's settlement lag after the valuation date applies.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn premium_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cdso_set!(
            slf,
            premium_settlement_date,
            date_repr(date),
            |b: CdsOptionBuilderInner| b.premium_settlement_date(date)
        )
    }

    /// Set the exercise proceeds payment date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     On or after expiry and before CDS maturity; when never set, legal
    ///     expiry is used.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cdso_set!(
            slf,
            exercise_settlement_date,
            date_repr(date),
            |b: CdsOptionBuilderInner| b.exercise_settlement_date(date)
        )
    }

    /// Set the underlying CDS accrual-effective date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Accrual start used for the forward spread and risky annuity; when
    ///     never set, ``protection_start_convention`` selects it.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cdso_set!(
            slf,
            underlying_start_date,
            date_repr(date),
            |b: CdsOptionBuilderInner| b.underlying_start_date(date)
        )
    }

    /// Set the underlying accrual-start convention.
    ///
    /// Parameters
    /// ----------
    /// value : {"spot", "forward"}
    ///     ``"spot"`` (default) accrues from the prior CDS roll date;
    ///     ``"forward"`` accrues from option expiry.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized convention.
    #[pyo3(text_signature = "($self, value)")]
    fn protection_start_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = enum_from_str(value, "protection_start_convention")?;
        cdso_set!(
            slf,
            protection_start_convention,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.protection_start_convention(parsed)
        )
    }

    /// Set whether the option knocks out on default before expiry.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` for knock-out single-name options; default ``False``.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn knockout<'py>(mut slf: PyRefMut<'py, Self>, value: bool) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            knockout,
            bool_repr(value).to_string(),
            |b: CdsOptionBuilderInner| b.knockout(value)
        )
    }

    /// Set the recovery rate assumption.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Recovery as a decimal in ``[0, 1]`` (``0.4`` = 40%).
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            recovery_rate,
            float_repr(value),
            |b: CdsOptionBuilderInner| b.recovery_rate(value)
        )
    }

    /// Set the discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve id in the market context.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.discount_curve_id(CurveId::new(value))
        )
    }

    /// Set the hazard (credit) curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Hazard curve id in the market context.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.credit_curve_id(CurveId::new(value))
        )
    }

    /// Set the volatility surface identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Spread (or price) volatility surface id in the market context.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.vol_surface_id(CurveId::new(value))
        )
    }

    /// Set the ISDA convention of the underlying CDS.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     ``"isda_na"`` (default), ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized convention.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = cds_convention_from_str(value)?;
        cdso_set!(
            slf,
            underlying_convention,
            format!("{value:?}"),
            |b: CdsOptionBuilderInner| b.underlying_convention(convention)
        )
    }

    /// Set whether the underlying is a CDS index.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` for an index option (no knock-out, index factor applies),
    ///     ``False`` for a single-name option.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_is_index<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            underlying_is_index,
            bool_repr(value).to_string(),
            |b: CdsOptionBuilderInner| b.underlying_is_index(value)
        )
    }

    /// Set the current index factor.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Surviving fraction of the original index notional, in ``(0, 1]``;
    ///     default ``1.0``.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            index_factor,
            float_repr(value),
            |b: CdsOptionBuilderInner| b.index_factor(value)
        )
    }

    /// Set the original index factor of a clean-price strike.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     ``f0`` the clean-price strike is quoted on; required for
    ///     clean-price strikes and rejected for spread strikes.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn strike_index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            strike_index_factor,
            float_repr(value),
            |b: CdsOptionBuilderInner| b.strike_index_factor(value)
        )
    }

    /// Set the settled cumulative index loss since option inception.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Decimal fraction of original index notional in ``[0, 1]``; default
    ///     ``0.0`` (single-name options must keep ``0.0``).
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn realized_loss<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cdso_set!(
            slf,
            realized_loss,
            float_repr(value),
            |b: CdsOptionBuilderInner| b.realized_loss(value)
        )
    }

    /// Set the running coupon of the underlying CDS.
    ///
    /// Parameters
    /// ----------
    /// value : decimal.Decimal | str | int | float
    ///     Coupon in basis points (``100`` for CDX.NA.IG, ``500`` for
    ///     CDX.NA.HY), parsed exactly from its string form. When never set
    ///     the strike spread is the coupon (single-name SNAC).
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a decimal number.
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon = decimal_arg(value)?;
        cdso_set!(
            slf,
            coupon_bp,
            coupon.to_string(),
            |b: CdsOptionBuilderInner| b.coupon_bp(coupon)
        )
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// CdsOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        cdso_set!(slf, attributes, shown, |b: CdsOptionBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated CDS option.
    ///
    /// Validation is the Rust ``CdsOption::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// CdsOption
    ///     The validated option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed option fails validation (expiry not before CDS
    ///     maturity, non-positive notional, inconsistent strike state).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyCdsOption> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyCdsOption { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("CdsOptionBuilder", &self.fields)
    }
}
