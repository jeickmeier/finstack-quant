//! Convertible bond Python wrappers: `ConvertibleBond`, its fluent builder
//! and the typed `ConversionSpec` / `CallPutSchedule` term classes.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DilutionEvent, DividendAdjustment,
    SoftCallTrigger,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

use super::super::convert::{
    attributes_from_py, builder_repr, date_repr, enum_to_py_string, float_repr, money_repr,
    money_to_py, opt_repr,
};
use super::super::instruments::serialize_typed_instrument_json;
use super::super::typed_fx::{
    instrument_envelope_methods, instrument_pricing_methods, take_builder,
};

type ConvertibleBondBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBondBuilder;

/// Deserialize a serde value from `str | dict | list` Python input.
fn serde_from_py<T: serde::de::DeserializeOwned>(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<T> {
    let json = py_to_json_value(py, value, what)?;
    serde_json::from_value(json).map_err(|e| serde_json_to_py(e, &format!("invalid {what}")))
}

/// Conversion terms of a convertible bond (typed wrapper for Rust ``ConversionSpec``).
///
/// At least one of ``ratio`` (shares per bond) and ``price`` (conversion
/// price per share) must be given; when both are, they must agree with
/// ``notional / price``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import ConversionSpec
/// >>> spec = ConversionSpec(ratio=25.0)
/// >>> (spec.ratio, spec.policy, spec.anti_dilution)
/// (25.0, 'voluntary', 'none')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConversionSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConversionSpec {
    /// Inner canonical Rust conversion spec.
    pub(crate) inner: ConversionSpec,
}

#[pymethods]
impl PyConversionSpec {
    /// Describe the conversion terms.
    ///
    /// Parameters
    /// ----------
    /// ratio : float | None
    ///     Conversion ratio (shares per bond); derived from ``price`` when ``None``.
    /// price : float | None
    ///     Conversion price per share; derived from ``ratio`` when ``None``.
    /// policy : str | dict | None
    ///     Conversion policy: ``"voluntary"`` (the default when ``None``), or a tagged dict such
    ///     as ``{"mandatory_on": "2027-03-15"}``,
    ///     ``{"window": {"start": ..., "end": ...}}``,
    ///     ``{"upon_event": "qualified_ipo"}`` or
    ///     ``{"mandatory_variable": {"conversion_date": ..., "upper_conversion_price": ..., "lower_conversion_price": ...}}``.
    /// anti_dilution : {"none", "full_ratchet", "weighted_average"}
    ///     Anti-dilution protection; default ``"none"``.
    /// dividend_adjustment : {"none", "adjust_price", "adjust_ratio"}
    ///     Dividend protection; default ``"none"``.
    /// dilution_events : list[dict] | None
    ///     Dilution events (``date``, ``new_issue_price``, ``new_shares_issued``,
    ///     ``shares_outstanding_before``) in chronological order; pricing raises
    ///     ``ValueError`` for out-of-order events. Default empty.
    ///
    /// Returns
    /// -------
    /// ConversionSpec
    ///     The conversion terms (validated when the bond is built).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``policy`` / ``anti_dilution`` / ``dividend_adjustment`` are not
    ///     recognized, or a dilution event does not match the schema.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import ConversionSpec
    /// >>> ConversionSpec(price=50.0, dividend_adjustment="adjust_ratio").dividend_adjustment
    /// 'adjust_ratio'
    #[new]
    #[pyo3(signature = (ratio=None, price=None, policy=None, anti_dilution="none", dividend_adjustment="none", dilution_events=None))]
    #[pyo3(
        text_signature = "(ratio=None, price=None, policy=None, anti_dilution='none', dividend_adjustment='none', dilution_events=None)"
    )]
    fn new(
        py: Python<'_>,
        ratio: Option<f64>,
        price: Option<f64>,
        policy: Option<&Bound<'_, PyAny>>,
        anti_dilution: &str,
        dividend_adjustment: &str,
        dilution_events: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let policy: ConversionPolicy = match policy {
            Some(p) if !p.is_none() => serde_from_py(py, p, "conversion policy")?,
            _ => ConversionPolicy::Voluntary,
        };
        let anti_dilution: AntiDilutionPolicy =
            super::super::instruments::enum_from_str(anti_dilution, "anti_dilution")?;
        let dividend_adjustment: DividendAdjustment =
            super::super::instruments::enum_from_str(dividend_adjustment, "dividend_adjustment")?;
        let dilution_events: Vec<DilutionEvent> = match dilution_events {
            Some(events) if !events.is_none() => serde_from_py(py, events, "dilution_events")?,
            _ => Vec::new(),
        };
        Ok(Self {
            inner: ConversionSpec {
                ratio,
                price,
                policy,
                anti_dilution,
                dividend_adjustment,
                dilution_events,
            },
        })
    }

    /// Deserialize from the canonical JSON shape.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with ``ratio``, ``price``, ``policy``, ``anti_dilution``,
    ///     ``dividend_adjustment`` and optional ``dilution_events``.
    ///
    /// Returns
    /// -------
    /// ConversionSpec
    ///     The parsed terms.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import ConversionSpec
    /// >>> ConversionSpec.from_json(ConversionSpec(ratio=20.0).to_json()).ratio
    /// 20.0
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid ConversionSpec JSON"))
    }

    /// Serialize to the canonical JSON shape.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON accepted by ``from_json`` and ``ConvertibleBondBuilder.conversion``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize ConversionSpec"))
    }

    /// Support `pickle` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Conversion ratio (shares per bond), if given explicitly.
    #[getter]
    fn ratio(&self) -> Option<f64> {
        self.inner.ratio
    }

    /// Conversion price per share, if given explicitly.
    #[getter]
    fn price(&self) -> Option<f64> {
        self.inner.price
    }

    /// Conversion policy: ``"voluntary"`` or the tagged dict form.
    #[getter]
    fn policy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.policy)
    }

    /// Anti-dilution policy (serde name).
    #[getter]
    fn anti_dilution(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.anti_dilution)
    }

    /// Dividend adjustment policy (serde name).
    #[getter]
    fn dividend_adjustment(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.dividend_adjustment)
    }

    /// Dilution events as dicts.
    #[getter]
    fn dilution_events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.dilution_events)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConversionSpec(ratio={}, price={}, policy={:?}, anti_dilution={:?}, dividend_adjustment={:?}, dilution_events=<{}>)",
            opt_repr(self.inner.ratio.map(float_repr)),
            opt_repr(self.inner.price.map(float_repr)),
            enum_to_py_string(&self.inner.policy).unwrap_or_default(),
            enum_to_py_string(&self.inner.anti_dilution).unwrap_or_default(),
            enum_to_py_string(&self.inner.dividend_adjustment).unwrap_or_default(),
            self.inner.dilution_events.len(),
        )
    }
}

/// Issuer call and holder put windows (typed wrapper for Rust ``CallPutSchedule``).
///
/// Each window is a dict ``{"start_date", "end_date", "price_pct_of_par",
/// "make_whole"?}`` with prices in percent of par (``101.0`` = 101%).
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CallPutSchedule
/// >>> sched = CallPutSchedule(
/// ...     calls=[{"start_date": "2026-03-15", "end_date": "2027-03-15", "price_pct_of_par": 101.0}]
/// ... )
/// >>> (len(sched.calls), len(sched.puts))
/// (1, 0)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CallPutSchedule",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCallPutSchedule {
    /// Inner canonical Rust schedule.
    pub(crate) inner: CallPutSchedule,
}

/// Coerce `list[dict] | str | None` to call/put windows.
fn windows_from_py(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    what: &str,
) -> PyResult<Vec<CallPut>> {
    match value {
        Some(v) if !v.is_none() => serde_from_py(py, v, what),
        _ => Ok(Vec::new()),
    }
}

#[pymethods]
impl PyCallPutSchedule {
    /// Describe the call and put windows.
    ///
    /// Parameters
    /// ----------
    /// calls : list[dict] | str | None
    ///     Issuer call windows (``start_date``, ``end_date``,
    ///     ``price_pct_of_par``, optional ``make_whole``); default none.
    /// puts : list[dict] | str | None
    ///     Holder put windows of the same shape; default none.
    ///
    /// Returns
    /// -------
    /// CallPutSchedule
    ///     The schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a window does not match the schema.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CallPutSchedule
    /// >>> CallPutSchedule().calls
    /// []
    #[new]
    #[pyo3(signature = (calls=None, puts=None))]
    #[pyo3(text_signature = "(calls=None, puts=None)")]
    fn new(
        py: Python<'_>,
        calls: Option<&Bound<'_, PyAny>>,
        puts: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CallPutSchedule {
                calls: windows_from_py(py, calls, "calls")?,
                puts: windows_from_py(py, puts, "puts")?,
            },
        })
    }

    /// Deserialize from the canonical JSON shape (``{"calls": [...], "puts": [...]}``).
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with ``calls`` and ``puts`` arrays.
    ///
    /// Returns
    /// -------
    /// CallPutSchedule
    ///     The parsed schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CallPutSchedule
    /// >>> CallPutSchedule.from_json('{"calls": [], "puts": []}').puts
    /// []
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid CallPutSchedule JSON"))
    }

    /// Serialize to the canonical JSON shape.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON accepted by ``from_json`` and ``ConvertibleBondBuilder.call_put``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CallPutSchedule"))
    }

    /// Support `pickle` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Issuer call windows as dicts.
    #[getter]
    fn calls<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.calls)
    }

    /// Holder put windows as dicts.
    #[getter]
    fn puts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.puts)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CallPutSchedule(calls=<{}>, puts=<{}>)",
            self.inner.calls.len(),
            self.inner.puts.len()
        )
    }
}

/// Coerce `ConversionSpec | dict | str` to the Rust spec.
fn conversion_from_py(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<ConversionSpec> {
    if let Ok(spec) = value.cast::<PyConversionSpec>() {
        return Ok(spec.borrow().inner.clone());
    }
    serde_from_py(py, value, "conversion")
}

/// Coerce `CallPutSchedule | dict | str` to the Rust schedule.
fn call_put_from_py(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<CallPutSchedule> {
    if let Ok(sched) = value.cast::<PyCallPutSchedule>() {
        return Ok(sched.borrow().inner.clone());
    }
    serde_from_py(py, value, "call_put")
}

/// Convertible bond (typed wrapper for Rust ``ConvertibleBond``).
///
/// Debt with an embedded equity conversion option, priced on a
/// Tsiveriotis–Fernandes style tree: the bond floor discounts on
/// ``credit_curve_id`` (falling back to ``discount_curve_id``), the equity
/// component on the risk-free curve. Conversion, call/put, soft-call and
/// coupon terms are typed (``ConversionSpec``, ``CallPutSchedule``) or
/// dict/JSON inputs.
///
/// Build with ``ConvertibleBond.builder()`` or start from
/// ``ConvertibleBond.example()`` / ``example_mandatory()``; instances are
/// accepted directly by ``price_instrument`` and expose ``price`` /
/// ``metric`` / ``parity`` / ``conversion_premium`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import ConvertibleBond
/// >>> cb = ConvertibleBond.example()
/// >>> (cb.id, cb.conversion_ratio, cb.underlying_equity_id)
/// ('CB-TECH-5Y', 25.0, 'TECH')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleBond",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConvertibleBond {
    /// Inner canonical Rust convertible bond.
    pub(crate) inner: finstack_quant_valuations::instruments::ConvertibleBond,
}

impl PyConvertibleBond {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::ConvertibleBond(self.inner.clone()),
            "ConvertibleBond",
        )
    }
}

instrument_envelope_methods!(
    PyConvertibleBond,
    ConvertibleBond,
    "convertible_bond",
    PyConvertibleBondBuilder,
    finstack_quant_valuations::instruments::ConvertibleBond::builder()
);
instrument_pricing_methods!(PyConvertibleBond);

#[pymethods]
impl PyConvertibleBond {
    /// Canonical example: 5-year USD 1,000,000 2% semi-annual convertible.
    ///
    /// Mirrors Rust ``ConvertibleBond::example()``: ratio 25 shares per
    /// bond, voluntary conversion, underlying ``"TECH"``, curves ``USD-IG`` /
    /// ``USD-CREDIT-BBB``, issue 2024-01-15, maturity 2029-01-15.
    ///
    /// Returns
    /// -------
    /// ConvertibleBond
    ///     The validated example bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical example fails validation (never for a released build).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::ConvertibleBond::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Mandatory (PERCS/DECS-style) convertible example.
    ///
    /// Mirrors Rust ``ConvertibleBond::example_mandatory()``: 3-year 5%
    /// semi-annual, mandatory-variable conversion at maturity (upper
    /// conversion price 60, lower 40), 130% soft call, call at 101% after
    /// year 2 and put at 100% after year 1.
    ///
    /// Returns
    /// -------
    /// ConvertibleBond
    ///     The validated example bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical example fails validation (never for a released build).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_mandatory() -> PyResult<Self> {
        finstack_quant_valuations::instruments::ConvertibleBond::example_mandatory()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Policy-specific conversion value divided by notional.
    ///
    /// Mirrors Rust ``ConvertibleBond::parity``. Ordinary conversion uses the
    /// effective ratio times spot; mandatory-variable contracts use their
    /// lower-price, variable-share and upper-price delivery regimes.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the underlying equity price.
    ///
    /// Returns
    /// -------
    /// float
    ///     Dimensionless conversion-value-to-notional ratio; 1.0 means par.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the underlying price is missing from ``market``.
    /// ValueError
    ///     If conversion terms or the nonnegative finite equity price are invalid.
    /// RuntimeError
    ///     If the bond has no ``underlying_equity_id``.
    #[pyo3(text_signature = "($self, market)")]
    fn parity(&self, py: Python<'_>, market: &Bound<'_, PyAny>) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        self.inner.parity(&market).map_err(core_to_py)
    }

    /// Conversion premium over parity.
    ///
    /// Mirrors Rust ``ConvertibleBond::conversion_premium``:
    /// ``bond_price / conversion_value - 1``, using policy-specific share delivery.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the underlying equity price.
    /// bond_price : float
    ///     Observed bond price in notional currency units per bond.
    ///
    /// Returns
    /// -------
    /// float
    ///     Conversion premium as a decimal fraction (``0.15`` = 15%).
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the underlying price is missing from ``market``.
    /// ValueError
    ///     If conversion value is nonpositive/non-finite or bond price is negative/non-finite.
    /// RuntimeError
    ///     If the bond has no ``underlying_equity_id``.
    #[pyo3(text_signature = "($self, market, bond_price)")]
    fn conversion_premium(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        bond_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        self.inner
            .conversion_premium(&market, bond_price)
            .map_err(core_to_py)
    }

    /// Tree Greeks of the convertible.
    ///
    /// Mirrors Rust ``ConvertibleBond::greeks`` with the default tree.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Curves, equity price and volatility. Active volatility overrides take
    ///     precedence; surfaces are sampled at conversion strike. Floating
    ///     coupons require the forward curve and realized historical fixings.
    /// as_of : datetime.date | str
    ///     Valuation date.
    /// bump_size : float | None
    ///     Finite-difference bump for delta/gamma as a fraction of spot;
    ///     ``None`` uses the pricer default.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     ``price``, ``delta``, ``gamma``, ``vega``, ``theta``, ``rho``.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If required market data is missing.
    /// RuntimeError
    ///     If the tree pricer fails.
    /// ValueError
    ///     If inputs violate the selected lattice's admissible numerical domain.
    #[pyo3(signature = (market, as_of, bump_size=None))]
    #[pyo3(text_signature = "($self, market, as_of, bump_size=None)")]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        bump_size: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let greeks = self
            .inner
            .greeks(&market, None, bump_size, as_of)
            .map_err(core_to_py)?;
        serde_to_py(py, &greeks)
    }

    /// Principal amount.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Issue date.
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    /// Maturity date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Discount curve identifier for the debt component.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Credit curve identifier for risky discounting, or ``None``.
    #[getter]
    fn credit_curve_id(&self) -> Option<String> {
        self.inner.credit_curve_id.as_ref().map(|id| id.to_string())
    }

    /// Conversion terms.
    #[getter]
    fn conversion(&self) -> PyConversionSpec {
        PyConversionSpec {
            inner: self.inner.conversion.clone(),
        }
    }

    /// Base conversion ratio (shares per bond), derived from ratio or price.
    #[getter]
    fn conversion_ratio(&self) -> Option<f64> {
        self.inner.conversion_ratio()
    }

    /// Conversion ratio after anti-dilution adjustments.
    #[getter]
    fn effective_conversion_ratio(&self) -> Option<f64> {
        self.inner.effective_conversion_ratio()
    }

    /// Underlying equity identifier, or ``None``.
    #[getter]
    fn underlying_equity_id(&self) -> Option<String> {
        self.inner.underlying_equity_id.clone()
    }

    /// Call/put schedule, or ``None``.
    #[getter]
    fn call_put(&self) -> Option<PyCallPutSchedule> {
        self.inner.call_put.as_ref().map(|inner| PyCallPutSchedule {
            inner: inner.clone(),
        })
    }

    /// Soft-call trigger as ``{"threshold_pct", "observation_days", "required_days_above"}``, or ``None``.
    #[getter]
    fn soft_call_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .soft_call_trigger
            .as_ref()
            .map(|t| serde_to_py(py, t))
            .transpose()
    }

    /// Settlement lag in business days, or ``None`` for same-day.
    #[getter]
    fn settlement_days(&self) -> Option<u32> {
        self.inner.settlement_days
    }

    /// Assumed recovery rate on default as a fraction, or ``None``.
    #[getter]
    fn recovery_rate(&self) -> Option<f64> {
        self.inner.recovery_rate
    }

    /// Fixed coupon specification as a dict, or ``None``.
    #[getter]
    fn fixed_coupon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fixed_coupon
            .as_ref()
            .map(|c| serde_to_py(py, c))
            .transpose()
    }

    /// Floating coupon specification as a dict, or ``None``.
    #[getter]
    fn floating_coupon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .floating_coupon
            .as_ref()
            .map(|c| serde_to_py(py, c))
            .transpose()
    }

    /// Maturity as seen by the pricer, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConvertibleBond(id={:?}, notional={}, issue_date={}, maturity={}, conversion_ratio={}, underlying_equity_id={})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional),
            date_repr(self.inner.issue_date),
            date_repr(self.inner.maturity),
            opt_repr(self.inner.conversion_ratio().map(float_repr)),
            opt_repr(self.inner.underlying_equity_id.as_ref().map(|s| format!("{s:?}"))),
        )
    }
}

/// Fluent builder for ``ConvertibleBond``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleBondBuilder",
    skip_from_py_object
)]
pub struct PyConvertibleBondBuilder {
    inner: Option<ConvertibleBondBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! cb_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyConvertibleBondBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the convertible bond.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilderInner| b.id(InstrumentId::new(value.to_string()))
        )
    }

    /// Set the principal amount.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Principal amount.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        cb_set!(
            slf,
            notional,
            money_repr(money),
            |b: ConvertibleBondBuilderInner| b.notional(money)
        )
    }

    /// Set the issue date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Issue date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let issue_date = extract_date(value)?;
        cb_set!(
            slf,
            issue_date,
            date_repr(issue_date),
            |b: ConvertibleBondBuilderInner| b.issue_date(issue_date)
        )
    }

    /// Set the maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Maturity date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = extract_date(value)?;
        cb_set!(
            slf,
            maturity,
            date_repr(maturity),
            |b: ConvertibleBondBuilderInner| b.maturity(maturity)
        )
    }

    /// Set the discount curve identifier for the debt component.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier for the debt component (risk-free or
    ///     funding).
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilderInner| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the credit curve identifier for risky discounting (bond floor).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Credit curve identifier. If not provided, falls back to
    ///     ``discount_curve_id`` (implies no credit spread). Must represent
    ///     zero-recovery (pure hazard) risky discounting.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            credit_curve_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilderInner| b.credit_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the conversion terms.
    ///
    /// Parameters
    /// ----------
    /// value : ConversionSpec | dict | str
    ///     Conversion terms as a typed ``ConversionSpec``, a dict, or a JSON
    ///     object string with fields ``ratio``, ``price``, ``policy``,
    ///     ``anti_dilution``, ``dividend_adjustment`` and ``dilution_events``.
    ///     At least one of ``ratio`` / ``price`` must be set.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``ConversionSpec`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn conversion<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let conversion = conversion_from_py(py, value)?;
        let shown = PyConversionSpec {
            inner: conversion.clone(),
        }
        .__repr__();
        cb_set!(slf, conversion, shown, |b: ConvertibleBondBuilderInner| b
            .conversion(conversion))
    }

    /// Set the underlying equity identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Underlying equity identifier (ticker or instrument id).
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_equity_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            underlying_equity_id,
            format!("{value:?}"),
            |b: ConvertibleBondBuilderInner| b.underlying_equity_id(value.to_string())
        )
    }

    /// Set the call/put schedule.
    ///
    /// Parameters
    /// ----------
    /// value : CallPutSchedule | dict | str
    ///     Schedule as a typed ``CallPutSchedule``, a dict, or a JSON object
    ///     string with ``calls`` and ``puts`` arrays of windows.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``CallPutSchedule`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn call_put<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let call_put = call_put_from_py(py, value)?;
        let shown = format!(
            "CallPutSchedule(calls=<{}>, puts=<{}>)",
            call_put.calls.len(),
            call_put.puts.len()
        );
        cb_set!(slf, call_put, shown, |b: ConvertibleBondBuilderInner| b
            .call_put(call_put))
    }

    /// Set the soft-call trigger condition.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``SoftCallTrigger`` as a dict or JSON object string with fields
    ///     ``threshold_pct`` (percent of conversion price, e.g. ``130.0``),
    ///     ``observation_days`` and ``required_days_above``.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``SoftCallTrigger`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn soft_call_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: SoftCallTrigger = serde_from_py(py, value, "soft_call_trigger")?;
        let shown = format!(
            "{{'threshold_pct': {}, 'observation_days': {}, 'required_days_above': {}}}",
            float_repr(trigger.threshold_pct),
            trigger.observation_days,
            trigger.required_days_above
        );
        cb_set!(
            slf,
            soft_call_trigger,
            shown,
            |b: ConvertibleBondBuilderInner| b.soft_call_trigger(trigger)
        )
    }

    /// Set the settlement lag.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Number of business days from trade date to settlement date
    ///     (e.g. ``2`` for US corporate convertibles). If never set,
    ///     settlement is assumed same-day.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            settlement_days,
            value.to_string(),
            |b: ConvertibleBondBuilderInner| b.settlement_days(value)
        )
    }

    /// Set the assumed recovery rate on default.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Recovery rate as a fraction (e.g. ``0.40`` = 40%). Used in the
    ///     Tsiveriotis-Zhang credit model; only relevant when
    ///     ``credit_curve_id`` is set.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cb_set!(
            slf,
            recovery_rate,
            float_repr(value),
            |b: ConvertibleBondBuilderInner| b.recovery_rate(value)
        )
    }

    /// Set the fixed coupon specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``FixedCouponSpec`` as a dict or JSON object string
    ///     (``coupon_type``, decimal ``rate`` and a ``schedule`` block).
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``FixedCouponSpec`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn fixed_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_cashflows::builder::FixedCouponSpec =
            serde_from_py(py, value, "fixed_coupon")?;
        let shown = format!("<fixed coupon rate={}>", spec.rate);
        cb_set!(
            slf,
            fixed_coupon,
            shown,
            |b: ConvertibleBondBuilderInner| b.fixed_coupon(spec)
        )
    }

    /// Set the floating coupon specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``FloatingCouponSpec`` as a dict or JSON object string.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``FloatingCouponSpec`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn floating_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_cashflows::builder::FloatingCouponSpec =
            serde_from_py(py, value, "floating_coupon")?;
        cb_set!(
            slf,
            floating_coupon,
            "<floating coupon>".to_string(),
            |b: ConvertibleBondBuilderInner| b.floating_coupon(spec)
        )
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute bag; a dict populates metadata, with an optional
    ///     ``"tags"`` list entry populating tags.
    ///
    /// Returns
    /// -------
    /// ConvertibleBondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a dict, nor ``None``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        cb_set!(slf, attributes, shown, |b: ConvertibleBondBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated convertible bond.
    ///
    /// Validation is the Rust ``ConvertibleBond::builder().build()``
    /// invariants only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// ConvertibleBond
    ///     The validated convertible bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed bond fails validation (for example, conversion
    ///     terms set neither ``ratio`` nor ``price``).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyConvertibleBond> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyConvertibleBond { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("ConvertibleBondBuilder", &self.fields)
    }
}

/// Register the convertible helper classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConversionSpec>()?;
    m.add_class::<PyCallPutSchedule>()?;
    Ok(())
}
