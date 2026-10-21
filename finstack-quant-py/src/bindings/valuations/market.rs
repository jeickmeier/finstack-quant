//! `finstack_quant.valuations.market` — read-only market-convention lookups.
//!
//! Mirrors `finstack_quant_valuations::market::conventions`: the embedded
//! `ConventionRegistry` plus the frozen convention records it returns. The
//! registry is process-global and loaded once from the crate's embedded JSON
//! tables; every lookup here borrows it, so the wrappers below are plain
//! serde-backed value objects (``to_json`` / ``from_json`` / pickle).
//! Listed-product coverage (`listed_product_catalog`) is registered from
//! `pricing.rs`.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{enum_to_py_string, opt_repr};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_valuations::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId, IrFutureContractId,
    SwaptionConventionId, XccyConventionId,
};
use finstack_quant_valuations::market::conventions::{
    CdsConventionSpec, ConventionRegistry, InflationSwapConventions, IrFutureConventions,
    RateIndexConventions, SwaptionConventions, XccyConventions,
};

/// Names this module contributes to `finstack_quant.valuations.market.__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "CdsConventionSpec",
    "ConventionRegistry",
    "InflationSwapConventions",
    "IrFutureConventions",
    "RateIndexConventions",
    "SwaptionConventions",
    "XccyConventions",
];

/// Shared `to_json` / `from_json` / `__reduce__` / `to_dict` plumbing for the
/// serde-backed convention records.
macro_rules! convention_record_methods {
    ($py:ty, $rust:ty) => {
        #[pymethods]
        impl $py {
            /// Deserialize from the JSON produced by ``to_json``.
            ///
            /// Parameters
            /// ----------
            /// json : str
            ///     Strict JSON object with exactly the fields ``to_json`` writes.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the JSON is malformed or has the wrong shape.
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                let inner: $rust = serde_json::from_str(json).map_err(display_to_py)?;
                Ok(Self { inner })
            }

            /// Serialize to the canonical JSON wire form.
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string(&self.inner).map_err(display_to_py)
            }

            /// Return every field as a plain ``dict`` (canonical serde shape).
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                serde_to_py(py, &self.inner)
            }

            /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }
        }
    };
}

/// Market conventions for one floating-rate index (SOFR, EURIBOR-3M, ...).
///
/// Returned by ``ConventionRegistry.require_rate_index``. Fields are read-only;
/// build swaps from them with ``InterestRateSwap.from_conventions``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> conv = ConventionRegistry().require_rate_index("USD-SOFR")
/// >>> conv.currency
/// 'USD'
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "RateIndexConventions",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyRateIndexConventions {
    pub(crate) inner: RateIndexConventions,
}

convention_record_methods!(PyRateIndexConventions, RateIndexConventions);

#[pymethods]
impl PyRateIndexConventions {
    /// OIS compounding style for overnight indices, or ``None``.
    #[getter]
    fn ois_compounding(&self) -> PyResult<Option<String>> {
        self.inner
            .ois_compounding
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RateIndexConventions(currency='{}', kind='{}', tenor={}, day_count='{}', reset_lag_days={}, payment_lag_days={})",
            self.inner.currency,
            enum_to_py_string(&self.inner.kind).unwrap_or_default(),
            opt_repr(
                self.inner
                    .tenor
                    .as_ref()
                    .and_then(|t| enum_to_py_string(t).ok())
                    .map(|t| format!("'{t}'"))
            ),
            enum_to_py_string(&self.inner.day_count).unwrap_or_default(),
            self.inner.default_reset_lag_days,
            self.inner.default_payment_lag_days,
        )
    }
}

crate::bindings::macros::py_getters!(PyRateIndexConventions |s| {
    /// ISO-4217 currency code of the index.
    currency -> String = s.inner.currency.to_string();

    /// Index family: ``"overnight_rfr"``, ``"term_ibor"`` or the serde name of the variant.
    kind -> PyResult<String> = enum_to_py_string(&s.inner.kind);

    /// Index tenor (``"3M"``) or ``None`` for overnight indices.
    tenor -> PyResult<Option<String>> = s.inner.tenor.as_ref().map(enum_to_py_string).transpose();

    /// Accrual day count of the floating leg (serde name, e.g. ``"act_360"``).
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    /// Default floating-leg payment frequency (``"3M"``, ``"1Y"`` ...).
    default_payment_frequency -> String = s.inner.default_payment_frequency.to_string();

    /// Default payment lag in business days after period end.
    default_payment_lag_days -> i32 = s.inner.default_payment_lag_days;

    /// Default fixing (reset) lag in business days before accrual start.
    default_reset_lag_days -> i32 = s.inner.default_reset_lag_days;

    /// Calendar identifier used for fixings and payments.
    market_calendar_id -> String = s.inner.market_calendar_id.clone();

    /// Spot settlement lag in business days (T+n).
    market_settlement_days -> i32 = s.inner.market_settlement_days;

    /// Business-day convention for date rolls (serde name).
    market_business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.market_business_day_convention);

    /// Default fixed-leg day count for a standard swap on this index.
    default_fixed_leg_day_count -> PyResult<String> =
        enum_to_py_string(&s.inner.default_fixed_leg_day_count);

    /// Default fixed-leg payment frequency for a standard swap on this index.
    default_fixed_leg_frequency -> String = s.inner.default_fixed_leg_frequency.to_string();
});

/// Schedule conventions for one CDS family (``isda_na``, ``isda_eu``, ...).
///
/// Returned by ``ConventionRegistry.resolve_cds``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> spec = ConventionRegistry().resolve_cds("USD", "isda_na")
/// >>> spec.family
/// 'isda_na'
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "CdsConventionSpec",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCdsConventionSpec {
    pub(crate) inner: CdsConventionSpec,
}

convention_record_methods!(PyCdsConventionSpec, CdsConventionSpec);

#[pymethods]
impl PyCdsConventionSpec {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CdsConventionSpec(family='{}', calendar_id='{}', day_count='{}', frequency='{}', settlement_days={})",
            enum_to_py_string(&self.inner.family).unwrap_or_default(),
            self.inner.calendar_id,
            enum_to_py_string(&self.inner.day_count).unwrap_or_default(),
            self.inner.frequency,
            self.inner.settlement_days,
        )
    }
}

crate::bindings::macros::py_getters!(PyCdsConventionSpec |s| {
    /// Convention family (``"isda_na"``, ``"isda_eu"``, ``"isda_as"``, ``"custom"``).
    family -> PyResult<String> = enum_to_py_string(&s.inner.family);

    /// Calendar identifier for premium-leg date rolls.
    calendar_id -> String = s.inner.calendar_id.clone();

    /// Premium-leg accrual day count (serde name).
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    /// Business-day convention for premium dates (serde name).
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    /// Stub rule for the first premium period (serde name).
    stub -> PyResult<String> = enum_to_py_string(&s.inner.stub);

    /// Settlement lag in business days (T+n; SNAC is T+1 for the step-in date).
    settlement_days -> u16 = s.inner.settlement_days;

    /// Premium payment frequency (``"3M"`` for standard IMM-roll CDS).
    frequency -> String = s.inner.frequency.to_string();
});

/// Market conventions for a swaption family (settlement, fixed-leg schedule, index).
///
/// Returned by ``ConventionRegistry.require_swaption``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> conv = ConventionRegistry().require_swaption("USD")
/// >>> isinstance(conv.float_leg_index, str)
/// True
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "SwaptionConventions",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PySwaptionConventions {
    pub(crate) inner: SwaptionConventions,
}

convention_record_methods!(PySwaptionConventions, SwaptionConventions);

#[pymethods]
impl PySwaptionConventions {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "SwaptionConventions(float_leg_index='{}', fixed_leg_frequency='{}', fixed_leg_day_count='{}', settlement_days={})",
            self.inner.float_leg_index,
            self.inner.fixed_leg_frequency,
            enum_to_py_string(&self.inner.fixed_leg_day_count).unwrap_or_default(),
            self.inner.settlement_days,
        )
    }
}

crate::bindings::macros::py_getters!(PySwaptionConventions |s| {
    /// Calendar identifier for expiry and settlement rolls.
    calendar_id -> String = s.inner.calendar_id.clone();

    /// Settlement lag in business days from expiry to swap start.
    settlement_days -> i32 = s.inner.settlement_days;

    /// Business-day convention for schedule rolls (serde name).
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    /// Fixed-leg payment frequency of the underlying swap.
    fixed_leg_frequency -> String = s.inner.fixed_leg_frequency.to_string();

    /// Fixed-leg day count of the underlying swap (serde name).
    fixed_leg_day_count -> PyResult<String> = enum_to_py_string(&s.inner.fixed_leg_day_count);

    /// Floating-leg index identifier of the underlying swap.
    float_leg_index -> String = s.inner.float_leg_index.clone();
});

/// Market conventions for a zero-coupon inflation swap family.
///
/// Returned by ``ConventionRegistry.require_inflation_swap``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> registry = ConventionRegistry()
/// >>> isinstance(registry, ConventionRegistry)
/// True
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "InflationSwapConventions",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyInflationSwapConventions {
    pub(crate) inner: InflationSwapConventions,
}

convention_record_methods!(PyInflationSwapConventions, InflationSwapConventions);

#[pymethods]
impl PyInflationSwapConventions {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "InflationSwapConventions(calendar_id='{}', day_count='{}', inflation_lag='{}', settlement_days={})",
            self.inner.calendar_id,
            enum_to_py_string(&self.inner.day_count).unwrap_or_default(),
            self.inner.inflation_lag,
            self.inner.settlement_days,
        )
    }
}

crate::bindings::macros::py_getters!(PyInflationSwapConventions |s| {
    /// Monthly reference-index rule: ``linear`` daily weights or ``step`` monthly values.
    interpolation -> String = s.inner.interpolation.to_string();

    /// Calendar identifier for schedule rolls.
    calendar_id -> String = s.inner.calendar_id.clone();

    /// Settlement lag in business days.
    settlement_days -> i32 = s.inner.settlement_days;

    /// Business-day convention for schedule rolls (serde name).
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    /// Accrual day count (serde name).
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    /// Index observation lag (``"3M"`` for the standard 3-month lag).
    inflation_lag -> String = s.inner.inflation_lag.to_string();
});

/// Market conventions for a cross-currency basis swap pair.
///
/// Returned by ``ConventionRegistry.require_xccy``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> registry = ConventionRegistry()
/// >>> isinstance(registry, ConventionRegistry)
/// True
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "XccyConventions",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyXccyConventions {
    pub(crate) inner: XccyConventions,
}

convention_record_methods!(PyXccyConventions, XccyConventions);

#[pymethods]
impl PyXccyConventions {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "XccyConventions(base_currency='{}', quote_currency='{}', base_index_id='{}', quote_index_id='{}', settlement_days={})",
            self.inner.base_currency,
            self.inner.quote_currency,
            self.inner.base_index_id,
            self.inner.quote_index_id,
            self.inner.settlement_days,
        )
    }
}

crate::bindings::macros::py_getters!(PyXccyConventions |s| {
    /// ISO-4217 code of the base (first) currency.
    base_currency -> String = s.inner.base_currency.to_string();

    /// ISO-4217 code of the quote (second) currency.
    quote_currency -> String = s.inner.quote_currency.to_string();

    /// Floating index identifier of the base-currency leg.
    base_index_id -> String = s.inner.base_index_id.to_string();

    /// Floating index identifier of the quote-currency leg.
    quote_index_id -> String = s.inner.quote_index_id.to_string();

    /// T+N spot settlement lag in business days.
    settlement_days -> u32 = s.inner.settlement_days;

    /// Payment frequency of both legs.
    payment_frequency -> String = s.inner.payment_frequency.to_string();

    /// Accrual day count (serde name).
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    /// Business-day convention for schedule rolls (serde name).
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    /// Calendar identifier of the base-currency leg.
    base_calendar_id -> String = s.inner.base_calendar_id.clone();

    /// Calendar identifier of the quote-currency leg.
    quote_calendar_id -> String = s.inner.quote_calendar_id.clone();

    /// Notional-exchange style (serde name of ``NotionalExchange``).
    notional_exchange -> PyResult<String> = enum_to_py_string(&s.inner.notional_exchange);
});

/// Contract conventions for a listed interest-rate future.
///
/// Returned by ``ConventionRegistry.require_ir_future``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> registry = ConventionRegistry()
/// >>> isinstance(registry, ConventionRegistry)
/// True
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "IrFutureConventions",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyIrFutureConventions {
    pub(crate) inner: IrFutureConventions,
}

convention_record_methods!(PyIrFutureConventions, IrFutureConventions);

#[pymethods]
impl PyIrFutureConventions {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "IrFutureConventions(index_id='{}', face_value={}, tick_size={}, tick_value={}, settlement_days={})",
            self.inner.index_id,
            self.inner.face_value,
            self.inner.tick_size,
            self.inner.tick_value,
            self.inner.settlement_days,
        )
    }
}

crate::bindings::macros::py_getters!(PyIrFutureConventions |s| {
    /// Underlying floating index identifier.
    index_id -> String = s.inner.index_id.to_string();

    /// Reference-rate compounding over the reference period: ``"simple"``,
    /// ``"simple_average"``, or the JSON text of a compounded variant such as
    /// ``{"compounded_in_arrears":{"lookback_days":0}}``.
    compounding -> PyResult<String> = enum_to_py_string(&s.inner.compounding);

    /// Reference-period placement relative to expiry (serde name).
    reference_period -> PyResult<String> = enum_to_py_string(&s.inner.reference_period);

    /// Exchange calendar identifier.
    calendar_id -> String = s.inner.calendar_id.clone();

    /// Settlement lag in business days.
    settlement_days -> i32 = s.inner.settlement_days;

    /// Number of listed delivery months.
    delivery_months -> u8 = s.inner.delivery_months;

    /// Contract face value in the contract currency.
    face_value -> f64 = s.inner.face_value;

    /// Minimum price increment in price points.
    tick_size -> f64 = s.inner.tick_size;

    /// Currency value of one tick.
    tick_value -> f64 = s.inner.tick_value;

    /// Fixed convexity adjustment in decimal rate, or ``None``.
    convexity_adjustment -> Option<f64> = s.inner.convexity_adjustment;
});

/// Process-global registry of embedded market conventions.
///
/// Mirrors Rust ``ConventionRegistry::try_global()``: the tables (rate
/// indices, CDS families, swaptions, inflation swaps, IR futures, cross-currency
/// pairs) are loaded once from the crate's embedded JSON and shared. Lookups
/// are read-only.
///
/// Raises
/// ------
/// RuntimeError
///     If the embedded convention tables fail to load (a packaging error).
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import ConventionRegistry
/// >>> registry = ConventionRegistry()
/// >>> registry.require_rate_index("USD-SOFR").currency
/// 'USD'
/// >>> registry.primary_cds_family("USD")
/// 'isda_na'
#[pyclass(
    module = "finstack_quant.valuations.market",
    name = "ConventionRegistry",
    frozen,
    skip_from_py_object
)]
pub struct PyConventionRegistry;

fn global_registry() -> PyResult<&'static ConventionRegistry> {
    ConventionRegistry::try_global().map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "ConventionRegistry failed to initialize: {err}"
        ))
    })
}

#[pymethods]
impl PyConventionRegistry {
    /// Open the process-global convention registry.
    ///
    /// Raises
    /// ------
    /// RuntimeError
    ///     If the embedded convention tables fail to load.
    #[new]
    #[pyo3(text_signature = "()")]
    fn new() -> PyResult<Self> {
        global_registry()?;
        Ok(Self)
    }

    /// Look up the conventions of a floating-rate index.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Index identifier as used on curves and legs, e.g. ``"USD-SOFR"``,
    ///     ``"EUR-EURIBOR-3M"``.
    ///
    /// Returns
    /// -------
    /// RateIndexConventions
    ///     Day count, frequencies, lags, calendar and settlement conventions.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``id`` is not in the embedded rate-index table.
    #[pyo3(text_signature = "($self, id)")]
    fn require_rate_index(&self, id: &str) -> PyResult<PyRateIndexConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_rate_index(&finstack_quant_core::types::IndexId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyRateIndexConventions { inner })
    }

    /// Resolve the CDS schedule conventions for a currency and doc clause.
    ///
    /// Parameters
    /// ----------
    /// currency : str
    ///     ISO-4217 code of the contract currency (``"USD"``, ``"EUR"``).
    /// doc_clause : str
    ///     ISDA doc clause or family name: ``"cr14"``, ``"mr14"``, ``"mm14"``,
    ///     ``"xr14"``, ``"isda_na"``, ``"isda_eu"``, ``"isda_as"``,
    ///     ``"isda_au"``, ``"isda_nz"``. The 2014 clauses map to the currency's
    ///     primary family (``"isda_na"`` is the SNAC / post-Big-Bang standard).
    ///
    /// Returns
    /// -------
    /// CdsConventionSpec
    ///     Calendar, day count, roll, stub, settlement and frequency conventions.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` or ``doc_clause`` is not recognised.
    /// KeyError
    ///     If no convention exists for the resolved family.
    #[pyo3(text_signature = "($self, currency, doc_clause)")]
    fn resolve_cds(&self, currency: &str, doc_clause: &str) -> PyResult<PyCdsConventionSpec> {
        let registry = global_registry()?;
        let currency = crate::bindings::module_utils::parse_currency(currency)?;
        let doc_clause: CdsDocClause = doc_clause.parse().map_err(|_| {
            crate::errors::value_error(format!(
                "unknown CDS doc clause '{doc_clause}'; expected one of cr14, mr14, mm14, xr14, isda_na, isda_eu, isda_as, isda_au, isda_nz"
            ))
        })?;
        let key = CdsConventionKey {
            currency,
            doc_clause,
        };
        let inner = registry.resolve_cds(&key).map_err(core_to_py)?.clone();
        Ok(PyCdsConventionSpec { inner })
    }

    /// Return the primary CDS convention family for a currency.
    ///
    /// Parameters
    /// ----------
    /// currency : str
    ///     ISO-4217 code (``"USD"`` → ``"isda_na"``, ``"EUR"`` → ``"isda_eu"``).
    ///
    /// Returns
    /// -------
    /// str | None
    ///     Family serde name, or ``None`` when the currency has no primary family.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``currency`` is not a valid ISO-4217 code.
    #[pyo3(text_signature = "($self, currency)")]
    fn primary_cds_family(&self, currency: &str) -> PyResult<Option<String>> {
        let registry = global_registry()?;
        let currency = crate::bindings::module_utils::parse_currency(currency)?;
        registry
            .primary_cds_family(currency)
            .map(|family| enum_to_py_string(&family))
            .transpose()
    }

    /// Look up swaption conventions.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Swaption convention identifier (typically the underlying index id,
    ///     e.g. ``"USD"`` or ``"EUR"``).
    ///
    /// Returns
    /// -------
    /// SwaptionConventions
    ///     Settlement, roll and fixed-leg conventions of the underlying swap.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``id`` is not in the embedded swaption table.
    #[pyo3(text_signature = "($self, id)")]
    fn require_swaption(&self, id: &str) -> PyResult<PySwaptionConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_swaption(&SwaptionConventionId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PySwaptionConventions { inner })
    }

    /// Look up zero-coupon inflation-swap conventions.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Inflation-swap convention identifier (e.g. ``"USD-CPI"``, ``"EUR-HICP"``, ``"UK-RPI"``).
    ///
    /// Returns
    /// -------
    /// InflationSwapConventions
    ///     Calendar, settlement, day count and observation-lag conventions.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``id`` is not in the embedded inflation-swap table.
    #[pyo3(text_signature = "($self, id)")]
    fn require_inflation_swap(&self, id: &str) -> PyResult<PyInflationSwapConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_inflation_swap(&InflationSwapConventionId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyInflationSwapConventions { inner })
    }

    /// Look up listed interest-rate-future contract conventions.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Contract identifier (e.g. ``"CME:SR3"`` for CME 3M SOFR futures).
    ///
    /// Returns
    /// -------
    /// IrFutureConventions
    ///     Index, averaging, reference period, tick and settlement conventions.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``id`` is not in the embedded IR-future table.
    #[pyo3(text_signature = "($self, id)")]
    fn require_ir_future(&self, id: &str) -> PyResult<PyIrFutureConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_ir_future(&IrFutureContractId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyIrFutureConventions { inner })
    }

    /// Look up cross-currency basis-swap conventions.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Pair identifier (e.g. ``"EUR/USD-XCCY"``).
    ///
    /// Returns
    /// -------
    /// XccyConventions
    ///     Leg indices, calendars, lags, frequency and notional-exchange style.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``id`` is not in the embedded cross-currency table.
    #[pyo3(text_signature = "($self, id)")]
    fn require_xccy(&self, id: &str) -> PyResult<PyXccyConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_xccy(&XccyConventionId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyXccyConventions { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        "ConventionRegistry(global)".to_string()
    }
}

/// Register convention lookups on the `market` submodule.
///
/// # Arguments
///
/// * `_py` - Interpreter token.
/// * `m` - The `finstack_quant.valuations.market` module under construction.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConventionRegistry>()?;
    m.add_class::<PyRateIndexConventions>()?;
    m.add_class::<PyCdsConventionSpec>()?;
    m.add_class::<PySwaptionConventions>()?;
    m.add_class::<PyInflationSwapConventions>()?;
    m.add_class::<PyXccyConventions>()?;
    m.add_class::<PyIrFutureConventions>()?;
    Ok(())
}
