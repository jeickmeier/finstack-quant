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
    /// ISO-4217 currency code of the index.
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    /// Index family: ``"overnight_rfr"``, ``"term_ibor"`` or the serde name of the variant.
    #[getter]
    fn kind(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.kind)
    }

    /// Index tenor (``"3M"``) or ``None`` for overnight indices.
    #[getter]
    fn tenor(&self) -> PyResult<Option<String>> {
        self.inner.tenor.as_ref().map(enum_to_py_string).transpose()
    }

    /// Accrual day count of the floating leg (serde name, e.g. ``"act_360"``).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Default floating-leg payment frequency (``"3M"``, ``"1Y"`` ...).
    #[getter]
    fn default_payment_frequency(&self) -> String {
        self.inner.default_payment_frequency.to_string()
    }

    /// Default payment lag in business days after period end.
    #[getter]
    fn default_payment_lag_days(&self) -> i32 {
        self.inner.default_payment_lag_days
    }

    /// Default fixing (reset) lag in business days before accrual start.
    #[getter]
    fn default_reset_lag_days(&self) -> i32 {
        self.inner.default_reset_lag_days
    }

    /// OIS compounding style for overnight indices, or ``None``.
    #[getter]
    fn ois_compounding(&self) -> PyResult<Option<String>> {
        self.inner
            .ois_compounding
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Calendar identifier used for fixings and payments.
    #[getter]
    fn market_calendar_id(&self) -> String {
        self.inner.market_calendar_id.clone()
    }

    /// Spot settlement lag in business days (T+n).
    #[getter]
    fn market_settlement_days(&self) -> i32 {
        self.inner.market_settlement_days
    }

    /// Business-day convention for date rolls (serde name).
    #[getter]
    fn market_business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.market_business_day_convention)
    }

    /// Default fixed-leg day count for a standard swap on this index.
    #[getter]
    fn default_fixed_leg_day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.default_fixed_leg_day_count)
    }

    /// Default fixed-leg payment frequency for a standard swap on this index.
    #[getter]
    fn default_fixed_leg_frequency(&self) -> String {
        self.inner.default_fixed_leg_frequency.to_string()
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
    /// Convention family (``"isda_na"``, ``"isda_eu"``, ``"isda_as"``, ``"custom"``).
    #[getter]
    fn family(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.family)
    }

    /// Calendar identifier for premium-leg date rolls.
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.clone()
    }

    /// Premium-leg accrual day count (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Business-day convention for premium dates (serde name).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Stub rule for the first premium period (serde name).
    #[getter]
    fn stub(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.stub)
    }

    /// Settlement lag in business days (T+n; SNAC is T+1 for the step-in date).
    #[getter]
    fn settlement_days(&self) -> u16 {
        self.inner.settlement_days
    }

    /// Premium payment frequency (``"3M"`` for standard IMM-roll CDS).
    #[getter]
    fn frequency(&self) -> String {
        self.inner.frequency.to_string()
    }

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
    /// Calendar identifier for expiry and settlement rolls.
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.clone()
    }

    /// Settlement lag in business days from expiry to swap start.
    #[getter]
    fn settlement_days(&self) -> i32 {
        self.inner.settlement_days
    }

    /// Business-day convention for schedule rolls (serde name).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Fixed-leg payment frequency of the underlying swap.
    #[getter]
    fn fixed_leg_frequency(&self) -> String {
        self.inner.fixed_leg_frequency.to_string()
    }

    /// Fixed-leg day count of the underlying swap (serde name).
    #[getter]
    fn fixed_leg_day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.fixed_leg_day_count)
    }

    /// Floating-leg index identifier of the underlying swap.
    #[getter]
    fn float_leg_index(&self) -> String {
        self.inner.float_leg_index.clone()
    }

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
    /// Monthly reference-index rule: ``linear`` daily weights or ``step`` monthly values.
    #[getter]
    fn interpolation(&self) -> String {
        self.inner.interpolation.to_string()
    }

    /// Calendar identifier for schedule rolls.
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.clone()
    }

    /// Settlement lag in business days.
    #[getter]
    fn settlement_days(&self) -> i32 {
        self.inner.settlement_days
    }

    /// Business-day convention for schedule rolls (serde name).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Accrual day count (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Index observation lag (``"3M"`` for the standard 3-month lag).
    #[getter]
    fn inflation_lag(&self) -> String {
        self.inner.inflation_lag.to_string()
    }

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
    /// ISO-4217 code of the base (first) currency.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// ISO-4217 code of the quote (second) currency.
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    /// Floating index identifier of the base-currency leg.
    #[getter]
    fn base_index_id(&self) -> String {
        self.inner.base_index_id.to_string()
    }

    /// Floating index identifier of the quote-currency leg.
    #[getter]
    fn quote_index_id(&self) -> String {
        self.inner.quote_index_id.to_string()
    }

    /// Spot lag in business days.
    #[getter]
    fn spot_lag_days(&self) -> i32 {
        self.inner.spot_lag_days
    }

    /// Payment frequency of both legs.
    #[getter]
    fn payment_frequency(&self) -> String {
        self.inner.payment_frequency.to_string()
    }

    /// Accrual day count (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Business-day convention for schedule rolls (serde name).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Calendar identifier of the base-currency leg.
    #[getter]
    fn base_calendar_id(&self) -> String {
        self.inner.base_calendar_id.clone()
    }

    /// Calendar identifier of the quote-currency leg.
    #[getter]
    fn quote_calendar_id(&self) -> String {
        self.inner.quote_calendar_id.clone()
    }

    /// Notional-exchange style (serde name of ``NotionalExchange``).
    #[getter]
    fn notional_exchange(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.notional_exchange)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "XccyConventions(base_currency='{}', quote_currency='{}', base_index_id='{}', quote_index_id='{}', spot_lag_days={})",
            self.inner.base_currency,
            self.inner.quote_currency,
            self.inner.base_index_id,
            self.inner.quote_index_id,
            self.inner.spot_lag_days,
        )
    }
}

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
    /// Underlying floating index identifier.
    #[getter]
    fn index_id(&self) -> String {
        self.inner.index_id.to_string()
    }

    /// Rate averaging method over the reference period (serde name).
    #[getter]
    fn rate_averaging(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.rate_averaging)
    }

    /// Reference-period placement relative to expiry (serde name).
    #[getter]
    fn reference_period(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.reference_period)
    }

    /// Exchange calendar identifier.
    #[getter]
    fn calendar_id(&self) -> String {
        self.inner.calendar_id.clone()
    }

    /// Settlement lag in business days.
    #[getter]
    fn settlement_days(&self) -> i32 {
        self.inner.settlement_days
    }

    /// Number of listed delivery months.
    #[getter]
    fn delivery_months(&self) -> u8 {
        self.inner.delivery_months
    }

    /// Contract face value in the contract currency.
    #[getter]
    fn face_value(&self) -> f64 {
        self.inner.face_value
    }

    /// Minimum price increment in price points.
    #[getter]
    fn tick_size(&self) -> f64 {
        self.inner.tick_size
    }

    /// Currency value of one tick.
    #[getter]
    fn tick_value(&self) -> f64 {
        self.inner.tick_value
    }

    /// Fixed convexity adjustment in decimal rate, or ``None``.
    #[getter]
    fn convexity_adjustment(&self) -> Option<f64> {
        self.inner.convexity_adjustment
    }

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
    for name in EXPORTS {
        m.getattr(*name)?
            .setattr("__module__", "finstack_quant.valuations.market")?;
    }
    Ok(())
}
