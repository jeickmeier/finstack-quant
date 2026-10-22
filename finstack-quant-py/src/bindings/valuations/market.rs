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
use crate::bindings::pydoc::pydoc;
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
/// serde-backed convention records; `$name` is the Python class name keying
/// the per-class stub docstrings.
macro_rules! convention_record_methods {
    ($py:ty, $rust:ty, $name:literal) => {
        #[pymethods]
        impl $py {
            #[doc = pydoc!("finstack_quant.valuations.market", concat!($name, ".from_json"))]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                let inner: $rust = serde_json::from_str(json).map_err(display_to_py)?;
                Ok(Self { inner })
            }

            #[doc = pydoc!("finstack_quant.valuations.market", concat!($name, ".to_json"))]
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string(&self.inner).map_err(display_to_py)
            }

            #[doc = pydoc!("finstack_quant.valuations.market", concat!($name, ".to_dict"))]
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

#[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions")]
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

convention_record_methods!(
    PyRateIndexConventions,
    RateIndexConventions,
    "RateIndexConventions"
);

#[pymethods]
impl PyRateIndexConventions {
    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.ois_compounding")]
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
    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.kind")]
    kind -> PyResult<String> = enum_to_py_string(&s.inner.kind);

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.tenor")]
    tenor -> PyResult<Option<String>> = s.inner.tenor.as_ref().map(enum_to_py_string).transpose();

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.day_count")]
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.default_payment_frequency")]
    default_payment_frequency -> String = s.inner.default_payment_frequency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.default_payment_lag_days")]
    default_payment_lag_days -> i32 = s.inner.default_payment_lag_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.default_reset_lag_days")]
    default_reset_lag_days -> i32 = s.inner.default_reset_lag_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.market_calendar_id")]
    market_calendar_id -> String = s.inner.market_calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.market_settlement_days")]
    market_settlement_days -> i32 = s.inner.market_settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.market_business_day_convention")]
    market_business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.market_business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.default_fixed_leg_day_count")]
    default_fixed_leg_day_count -> PyResult<String> =
        enum_to_py_string(&s.inner.default_fixed_leg_day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "RateIndexConventions.default_fixed_leg_frequency")]
    default_fixed_leg_frequency -> String = s.inner.default_fixed_leg_frequency.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec")]
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

convention_record_methods!(PyCdsConventionSpec, CdsConventionSpec, "CdsConventionSpec");

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
    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.family")]
    family -> PyResult<String> = enum_to_py_string(&s.inner.family);

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.calendar_id")]
    calendar_id -> String = s.inner.calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.day_count")]
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.stub")]
    stub -> PyResult<String> = enum_to_py_string(&s.inner.stub);

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.settlement_days")]
    settlement_days -> u16 = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "CdsConventionSpec.frequency")]
    frequency -> String = s.inner.frequency.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions")]
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

convention_record_methods!(
    PySwaptionConventions,
    SwaptionConventions,
    "SwaptionConventions"
);

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
    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.calendar_id")]
    calendar_id -> String = s.inner.calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.settlement_days")]
    settlement_days -> i32 = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.fixed_leg_frequency")]
    fixed_leg_frequency -> String = s.inner.fixed_leg_frequency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.fixed_leg_day_count")]
    fixed_leg_day_count -> PyResult<String> = enum_to_py_string(&s.inner.fixed_leg_day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "SwaptionConventions.float_leg_index")]
    float_leg_index -> String = s.inner.float_leg_index.clone();
});

#[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions")]
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

convention_record_methods!(
    PyInflationSwapConventions,
    InflationSwapConventions,
    "InflationSwapConventions"
);

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
    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.interpolation")]
    interpolation -> String = s.inner.interpolation.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.calendar_id")]
    calendar_id -> String = s.inner.calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.settlement_days")]
    settlement_days -> i32 = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.day_count")]
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "InflationSwapConventions.inflation_lag")]
    inflation_lag -> String = s.inner.inflation_lag.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions")]
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

convention_record_methods!(PyXccyConventions, XccyConventions, "XccyConventions");

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
    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.base_currency")]
    base_currency -> String = s.inner.base_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.quote_currency")]
    quote_currency -> String = s.inner.quote_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.base_index_id")]
    base_index_id -> String = s.inner.base_index_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.quote_index_id")]
    quote_index_id -> String = s.inner.quote_index_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.settlement_days")]
    settlement_days -> u32 = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.payment_frequency")]
    payment_frequency -> String = s.inner.payment_frequency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.day_count")]
    day_count -> PyResult<String> = enum_to_py_string(&s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.base_calendar_id")]
    base_calendar_id -> String = s.inner.base_calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.quote_calendar_id")]
    quote_calendar_id -> String = s.inner.quote_calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "XccyConventions.notional_exchange")]
    notional_exchange -> PyResult<String> = enum_to_py_string(&s.inner.notional_exchange);
});

#[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions")]
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

convention_record_methods!(
    PyIrFutureConventions,
    IrFutureConventions,
    "IrFutureConventions"
);

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
    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.index_id")]
    index_id -> String = s.inner.index_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.compounding")]
    compounding -> PyResult<String> = enum_to_py_string(&s.inner.compounding);

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.reference_period")]
    reference_period -> PyResult<String> = enum_to_py_string(&s.inner.reference_period);

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.calendar_id")]
    calendar_id -> String = s.inner.calendar_id.clone();

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.settlement_days")]
    settlement_days -> i32 = s.inner.settlement_days;

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.delivery_months")]
    delivery_months -> u8 = s.inner.delivery_months;

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.face_value")]
    face_value -> f64 = s.inner.face_value;

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.tick_size")]
    tick_size -> f64 = s.inner.tick_size;

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.tick_value")]
    tick_value -> f64 = s.inner.tick_value;

    #[doc = pydoc!("finstack_quant.valuations.market", "IrFutureConventions.convexity_adjustment")]
    convexity_adjustment -> Option<f64> = s.inner.convexity_adjustment;
});

#[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry")]
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
    #[new]
    #[pyo3(text_signature = "()")]
    fn new() -> PyResult<Self> {
        global_registry()?;
        Ok(Self)
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.require_rate_index")]
    #[pyo3(text_signature = "($self, id)")]
    fn require_rate_index(&self, id: &str) -> PyResult<PyRateIndexConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_rate_index(&finstack_quant_core::types::IndexId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyRateIndexConventions { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.resolve_cds")]
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

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.primary_cds_family")]
    #[pyo3(text_signature = "($self, currency)")]
    fn primary_cds_family(&self, currency: &str) -> PyResult<Option<String>> {
        let registry = global_registry()?;
        let currency = crate::bindings::module_utils::parse_currency(currency)?;
        registry
            .primary_cds_family(currency)
            .map(|family| enum_to_py_string(&family))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.require_swaption")]
    #[pyo3(text_signature = "($self, id)")]
    fn require_swaption(&self, id: &str) -> PyResult<PySwaptionConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_swaption(&SwaptionConventionId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PySwaptionConventions { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.require_inflation_swap")]
    #[pyo3(text_signature = "($self, id)")]
    fn require_inflation_swap(&self, id: &str) -> PyResult<PyInflationSwapConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_inflation_swap(&InflationSwapConventionId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyInflationSwapConventions { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.require_ir_future")]
    #[pyo3(text_signature = "($self, id)")]
    fn require_ir_future(&self, id: &str) -> PyResult<PyIrFutureConventions> {
        let registry = global_registry()?;
        let inner = registry
            .require_ir_future(&IrFutureContractId::new(id))
            .map_err(core_to_py)?
            .clone();
        Ok(PyIrFutureConventions { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry.require_xccy")]
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
