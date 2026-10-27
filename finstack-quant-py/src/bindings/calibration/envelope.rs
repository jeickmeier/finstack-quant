//! Typed calibration-envelope authoring: steps, plans and envelopes.
//!
//! Every wrapper holds the canonical Rust value; constructors assemble the
//! serde wire shape and let the Rust deserializer validate it, so the binding
//! never re-implements field rules.

use super::config::extract_config;
use super::quotes::extract_market_datum;
use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::extract_date_iso;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep,
};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{Map, Value};

/// Deserialize a JSON object into `T` with a labelled error.
pub(crate) fn from_value<T: serde::de::DeserializeOwned>(value: Value, label: &str) -> PyResult<T> {
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

/// Merge ``**params`` keyword overrides into a wire object.
fn merge_kwargs(
    py: Python<'_>,
    target: &mut Map<String, Value>,
    params: Option<&Bound<'_, PyDict>>,
    label: &str,
) -> PyResult<()> {
    if let Some(params) = params {
        for (key, value) in params.iter() {
            let key: String = key.extract()?;
            let value = if value.is_none() {
                Value::Null
            } else {
                py_to_json_value(py, &value, &format!("{label} field '{key}'"))?
            };
            target.insert(key, value);
        }
    }
    Ok(())
}

fn extract_market_data(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<MarketDatum>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| extract_market_datum(py, &item?))
        .collect()
}

fn extract_prior_market(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<PriorMarketObject>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| {
            let value = py_to_json_value(py, &item?, "prior market object")?;
            from_value(value, "prior market object (expected {\"kind\": ..., ...})")
        })
        .collect()
}

/// One calibration step: the object to build, its quote set, and kind-specific parameters.
///
/// Every constructor takes the step ``id`` and the kind's required fields;
/// remaining optional fields are keyword overrides named exactly as in the
/// Rust ``StepParams`` wire schema (see ``schema.get("calibration.schema.json")``).
/// Quote payloads live in ``CalibrationEnvelope.market_data``. The plan
/// names their IDs in ``quote_sets``; a step references the set by name.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationStep, RateQuote
/// >>> quotes = [RateQuote.deposit("D3M", "USD-SOFR-OIS", "3M", 0.052)]
/// >>> step = CalibrationStep.discount("USD-OIS", "USD", "2026-05-08")
/// >>> step.kind, step.quote_set
/// ('discount', 'USD-OIS')
#[pyclass(
    name = "CalibrationStep",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationStep {
    pub(crate) inner: CalibrationStep,
}

impl PyCalibrationStep {
    pub(crate) fn from_inner(inner: CalibrationStep) -> Self {
        Self { inner }
    }

    /// Shared constructor: `kind` + `id` + explicit fields + `**params`.
    ///
    /// Rust `CalibrationStep::from_wire_fields` owns the omitted-argument
    /// defaults (quote set and produced-object identifiers fall back to `id`).
    fn build(
        py: Python<'_>,
        kind: &str,
        id: &str,
        quote_set: Option<String>,
        mut fields: Map<String, Value>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        merge_kwargs(py, &mut fields, params, kind)?;
        let step = CalibrationStep::from_wire_fields(kind, id, quote_set.as_deref(), fields)
            .map_err(core_to_py)?;
        Ok(Self::from_inner(step))
    }
}

/// Insert `key: value` pairs into a wire map.
fn fields(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

#[pymethods]
impl PyCalibrationStep {
    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.discount")]
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, quote_set = None, curve_id = None, **params))]
    #[pyo3(text_signature = "(id, currency, base_date, quote_set=None, curve_id=None, **params)")]
    #[allow(clippy::too_many_arguments)]
    fn discount(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "discount", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.forward")]
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, tenor_years, discount_curve_id, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, tenor_years, discount_curve_id, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn forward(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        tenor_years: f64,
        discount_curve_id: &str,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("tenor_years", Value::from(tenor_years)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
        ]);
        Self::build(py, "forward", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.hazard")]
    #[staticmethod]
    #[pyo3(signature = (id, entity, currency, base_date, discount_curve_id, recovery_rate, seniority = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, entity, currency, base_date, discount_curve_id, recovery_rate, seniority=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn hazard(
        py: Python<'_>,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        recovery_rate: f64,
        seniority: Option<&str>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            ("entity", Value::String(entity.into())),
            ("seniority", seniority.map_or(Value::Null, Value::from)),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("recovery_rate", Value::from(recovery_rate)),
        ]);
        Self::build(py, "hazard", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.inflation")]
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, discount_curve_id, index, observation_lag, base_cpi, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, discount_curve_id, index, observation_lag, base_cpi, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn inflation(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        index: &str,
        observation_lag: &str,
        base_cpi: f64,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("index", Value::String(index.into())),
            ("observation_lag", Value::String(observation_lag.into())),
            ("base_cpi", Value::from(base_cpi)),
        ]);
        Self::build(py, "inflation", id, quote_set, f, params)
    }

    /// Equity/FX volatility-surface (SABR) step from option vol quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and surface id).
    /// base_date : datetime.date | str
    ///     Surface base date.
    /// underlying_ticker : str
    ///     Underlying identifier the quotes reference.
    /// quote_set
    ///     As in ``discount``.
    /// vol_surface_id : str | None, default None
    ///     Identifier of the produced surface; defaults to ``id``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``beta``,
    ///     ``target_expiries``, ``target_strikes``, ``spot_override``,
    ///     ``dividend_yield_override``, ``expiry_extrapolation``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, underlying_ticker, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, underlying_ticker,  quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn vol_surface(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        underlying_ticker: &str,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                vol_surface_id.map_or(Value::Null, Value::String),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("underlying_ticker", Value::String(underlying_ticker.into())),
        ]);
        Self::build(py, "vol_surface", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.swaption_vol")]
    #[staticmethod]
    #[pyo3(signature = (id, base_date, discount_curve_id, currency, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, discount_curve_id, currency, quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn swaption_vol(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: &Bound<'_, PyAny>,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                vol_surface_id.map_or(Value::Null, Value::String),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
        ]);
        Self::build(py, "swaption_vol", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.base_correlation")]
    #[staticmethod]
    #[pyo3(signature = (id, index_id, series, maturity_years, base_date, discount_curve_id, currency, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, index_id, series, maturity_years, base_date, discount_curve_id, currency, quote_set=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn base_correlation(
        py: Python<'_>,
        id: &str,
        index_id: &str,
        series: u16,
        maturity_years: f64,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: &Bound<'_, PyAny>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("index_id", Value::String(index_id.into())),
            ("series", Value::from(series)),
            ("maturity_years", Value::from(maturity_years)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
        ]);
        Self::build(py, "base_correlation", id, quote_set, f, params)
    }

    /// Student-t copula degrees-of-freedom step for one tranche.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name).
    /// tranche_instrument_id : str
    ///     Tranche instrument whose ``"{id}_STUDENT_T_DF"`` scalar is written.
    /// base_correlation_curve_id : str
    ///     Base-correlation curve the tranche is priced on.
    /// quote_set
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``initial_df``,
    ///     ``df_bounds``, ``correlation``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, tranche_instrument_id, base_correlation_curve_id, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, tranche_instrument_id, base_correlation_curve_id, quote_set=None, **params)"
    )]
    fn student_t(
        py: Python<'_>,
        id: &str,
        tranche_instrument_id: &str,
        base_correlation_curve_id: &str,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "tranche_instrument_id",
                Value::String(tranche_instrument_id.into()),
            ),
            (
                "base_correlation_curve_id",
                Value::String(base_correlation_curve_id.into()),
            ),
        ]);
        Self::build(py, "student_t", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.hull_white")]
    #[staticmethod]
    #[pyo3(signature = (id, curve_id, currency, base_date, quote_set = None, **params))]
    #[pyo3(text_signature = "(id, curve_id, currency, base_date, quote_set=None, **params)")]
    #[allow(clippy::too_many_arguments)]
    fn hull_white(
        py: Python<'_>,
        id: &str,
        curve_id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", Value::String(curve_id.into())),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "hull_white", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.cap_floor_hull_white")]
    #[staticmethod]
    #[pyo3(signature = (id, discount_curve_id, forward_curve_id, index_id, currency, base_date, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, discount_curve_id, forward_curve_id, index_id, currency, base_date, quote_set=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn cap_floor_hull_white(
        py: Python<'_>,
        id: &str,
        discount_curve_id: &str,
        forward_curve_id: &str,
        index_id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("forward_curve_id", Value::String(forward_curve_id.into())),
            ("index_id", Value::String(index_id.into())),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "cap_floor_hull_white", id, quote_set, f, params)
    }

    /// SVI volatility-surface step from option vol quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and surface id).
    /// base_date : datetime.date | str
    ///     Surface base date.
    /// underlying_ticker : str
    ///     Underlying identifier the quotes reference.
    /// quote_set, vol_surface_id
    ///     As in ``vol_surface``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``target_expiries``,
    ///     ``target_strikes``, ``spot_override``, ``dividend_yield_override``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, underlying_ticker, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, underlying_ticker, quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn svi_surface(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        underlying_ticker: &str,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                vol_surface_id.map_or(Value::Null, Value::String),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("underlying_ticker", Value::String(underlying_ticker.into())),
        ]);
        Self::build(py, "svi_surface", id, quote_set, f, params)
    }

    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationStep.xccy_basis")]
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, fx_spot, domestic_discount_id, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, fx_spot, domestic_discount_id, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn xccy_basis(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        fx_spot: f64,
        domestic_discount_id: &str,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            (
                "currency",
                Value::String(extract_currency(currency)?.to_string()),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("fx_spot", Value::from(fx_spot)),
            (
                "domestic_discount_id",
                Value::String(domestic_discount_id.into()),
            ),
        ]);
        Self::build(py, "xccy_basis", id, quote_set, f, params)
    }

    /// Parametric (Nelson-Siegel / Svensson) curve fit step.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// model : str, optional
    ///     Parametric family (``"ns"`` Nelson-Siegel or ``"nss"`` Svensson).
    ///     ``None`` uses the Rust authoring default
    ///     ``CalibrationStep::DEFAULT_PARAMETRIC_MODEL`` (``"ns"``).
    /// quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire field: ``initial_params``. Only single-curve discount
    ///     fitting is supported. Acceptance uses the configured
    ///     discount-curve validation tolerance without a least-squares floor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, model = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(text_signature = "(id, base_date, model=None, quote_set=None, curve_id=None, **params)")]
    #[allow(clippy::too_many_arguments)]
    fn parametric(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        model: Option<&str>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", curve_id.map_or(Value::Null, Value::String)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("model", model.map_or(Value::Null, Value::from)),
        ]);
        Self::build(py, "parametric", id, quote_set, f, params)
    }

    /// Step identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Name of the quote set this step reads from ``plan.quote_sets``.
    #[getter]
    fn quote_set(&self) -> String {
        self.inner.quote_set.clone()
    }

    /// Step kind (``"discount"``, ``"forward"``, ``"hazard"``, ...).
    #[getter]
    fn kind(&self) -> String {
        self.inner.params.kind_str().to_string()
    }

    /// Kind-specific parameters as a dict (including ``kind``).
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.params)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationStep(id={:?}, kind={:?}, quote_set={:?})",
            self.inner.id,
            self.inner.params.kind_str(),
            self.inner.quote_set,
        )
    }
}

wire_methods!(PyCalibrationStep, CalibrationStep, "CalibrationStep");

/// Ordered calibration plan: steps, named quote sets, and solver settings.
///
/// Quote IDs are supplied explicitly in ``quote_sets`` and resolve against
/// the enclosing ``CalibrationEnvelope.market_data``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, CalibrationStep, RateQuote, calibrate
/// >>> quotes = [
/// ...     RateQuote.deposit("USD-DEP-3M", "USD-SOFR-OIS", "3M", 0.052),
/// ...     RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.049),
/// ... ]
/// >>> plan = CalibrationPlan([CalibrationStep.discount("USD-OIS", "USD", "2026-05-08")], quote_sets={"USD-OIS": [q.id for q in quotes]})
/// >>> plan.quote_sets
/// {'USD-OIS': ['USD-DEP-3M', 'USD-SWAP-2Y']}
/// >>> calibrate(CalibrationEnvelope(plan, market_data=quotes)).success
/// True
#[pyclass(
    name = "CalibrationPlan",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationPlan {
    pub(crate) inner: CalibrationPlan,
}

impl PyCalibrationPlan {
    pub(crate) fn from_inner(inner: CalibrationPlan) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationPlan {
    /// Build a plan from typed steps.
    ///
    /// Parameters
    /// ----------
    /// steps : list[CalibrationStep]
    ///     Plain steps in execution order.
    /// id : str, default "plan"
    ///     Plan identifier (Rust ``CalibrationPlan::DEFAULT_ID`` when omitted).
    /// description : str | None, default None
    ///     Free-text description.
    /// settings : CalibrationConfig | dict | None, default None
    ///     Solver settings; a dict is overlaid onto the Rust defaults.
    /// quote_sets : dict[str, list[str]] | None, default None
    ///     Explicit quote sets (ids must exist in the envelope
    ///     ``market_data``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``settings`` is invalid.
    #[new]
    #[pyo3(signature = (steps, id = CalibrationPlan::DEFAULT_ID, description = None, settings = None, quote_sets = None))]
    #[pyo3(text_signature = "(steps, id='plan', description=None, settings=None, quote_sets=None)")]
    fn new(
        py: Python<'_>,
        steps: Vec<PyRef<'_, PyCalibrationStep>>,
        id: &str,
        description: Option<String>,
        settings: Option<&Bound<'_, PyAny>>,
        quote_sets: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let settings = extract_config(py, settings)?;
        let mut sets: IndexMap<String, Vec<finstack_quant_calibration::quotes::QuoteId>> =
            IndexMap::new();
        if let Some(quote_sets) = quote_sets {
            for (name, ids) in quote_sets.iter() {
                let name: String = name.extract()?;
                let ids: Vec<String> = ids.extract()?;
                sets.insert(
                    name,
                    ids.into_iter()
                        .map(finstack_quant_calibration::quotes::QuoteId::new)
                        .collect(),
                );
            }
        }
        let steps = steps.iter().map(|step| step.inner.clone()).collect();
        Ok(Self::from_inner(CalibrationPlan {
            id: id.to_string(),
            description,
            settings,
            quote_sets: sets,
            steps,
        }))
    }

    /// Plan identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Plan description, when set.
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Step identifiers in execution order.
    #[getter]
    fn step_ids(&self) -> Vec<String> {
        self.inner.steps.iter().map(|s| s.id.clone()).collect()
    }

    /// Plain typed steps in execution order.
    #[getter]
    fn steps(&self) -> Vec<PyCalibrationStep> {
        self.inner
            .steps
            .iter()
            .cloned()
            .map(PyCalibrationStep::from_inner)
            .collect()
    }

    /// Named quote sets: ``{name: [quote_id, ...]}``.
    #[getter]
    fn quote_sets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, ids) in &self.inner.quote_sets {
            let list: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
            dict.set_item(name, PyList::new(py, list)?)?;
        }
        Ok(dict)
    }

    /// Solver settings.
    #[getter]
    fn settings(&self) -> super::config::PyCalibrationConfig {
        super::config::PyCalibrationConfig::from_inner(self.inner.settings.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationPlan(id={:?}, steps={:?}, quote_sets={})",
            self.inner.id,
            self.step_ids(),
            self.inner.quote_sets.len(),
        )
    }
}

wire_methods!(PyCalibrationPlan, CalibrationPlan, "CalibrationPlan");

/// Complete calibration request: plan, flat market data, and prior calibrated objects.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan
/// >>> envelope = CalibrationEnvelope(CalibrationPlan([], id="smoke"))
/// >>> envelope.schema
/// 'finstack_quant.calibration/1'
/// >>> len(envelope.content_hash()) > 0
/// True
#[pyclass(
    name = "CalibrationEnvelope",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationEnvelope {
    pub(crate) inner: CalibrationEnvelope,
}

impl PyCalibrationEnvelope {
    pub(crate) fn from_inner(inner: CalibrationEnvelope) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationEnvelope {
    #[doc = pydoc!("finstack_quant.calibration._envelope", "CalibrationEnvelope.__init__")]
    #[new]
    #[pyo3(signature = (plan, market_data = None, prior_market = None))]
    #[pyo3(text_signature = "(plan, market_data=None, prior_market=None)")]
    fn new(
        py: Python<'_>,
        plan: PyRef<'_, PyCalibrationPlan>,
        market_data: Option<&Bound<'_, PyAny>>,
        prior_market: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let market_data = extract_market_data(py, market_data)?;
        let prior_market = extract_prior_market(py, prior_market)?;
        Ok(Self::from_inner(CalibrationEnvelope::new(
            plan.inner.clone(),
            market_data,
            prior_market,
        )))
    }

    /// Schema marker (``"finstack_quant.calibration/1"``).
    #[getter]
    fn schema(&self) -> String {
        finstack_quant_calibration::api::schema::CALIBRATION_SCHEMA.to_string()
    }

    /// The plain calibration plan.
    #[getter]
    fn plan(&self) -> PyCalibrationPlan {
        PyCalibrationPlan::from_inner(self.inner.plan.clone())
    }

    /// Flat market data as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn market_data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.market_data)
    }

    /// Prior calibrated objects as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn prior_market<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.prior_market)
    }

    /// Versioned SHA-256 hash of the canonical request JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the request contains a non-finite number.
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationEnvelope"))
    }

    /// Strictly load an envelope from JSON.
    ///
    /// Raises
    /// ------
    /// CalibrationEnvelopeError
    ///     If the JSON is malformed, the schema marker is missing or
    ///     unsupported, or the structure is invalid; ``diagnostics`` lists the
    ///     pointer-level findings.
    #[staticmethod]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        super::parse_envelope(py, json).map(Self::from_inner)
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationEnvelope(plan={:?}, steps={}, market_data={}, prior_market={})",
            self.inner.plan.id,
            self.inner.plan.steps.len(),
            self.inner.market_data.len(),
            self.inner.prior_market.len()
        )
    }
}
