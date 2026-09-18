//! Python bindings for the `finstack-quant-valuations` crate.
//!
//! Exposes the [`PyValuationResult`] envelope for pricing output,
//! JSON-based instrument loading and the standard pricer pipeline.

pub(crate) mod composite;
pub(crate) mod convert;
mod credit_derivatives;
mod exotic_rates;
pub(crate) mod instruments;
pub(crate) mod market;
mod merton_mc;
mod pricing;
mod schema;
mod structured_credit;
pub(crate) mod typed_credit;
pub(crate) mod typed_equity;
pub(crate) mod typed_fx;
mod typed_legs;
pub(crate) mod typed_rates;
pub(crate) mod typed_revolving_credit;
#[macro_use]
pub(crate) mod typed_structured_credit;
// Declared after `typed_structured_credit` so its `sc_wire_methods!` macro is in scope.
pub(crate) mod typed_asset_backed_facility;

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py,
};
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// Valuation envelope: present value, currency, risk metrics, covenant reports and policy stamps.
///
/// Returned by ``price_instrument`` and the typed ``price``/``price_with_metrics``
/// helpers; ``from_json`` rebuilds one from a previously serialized payload.
///
/// Reading metrics
/// ---------------
/// ``result["dv01"]`` / ``result.get_metric("dv01")`` return a scalar measure;
/// ``result.metrics`` is the whole ``{key: value}`` dict in computation order.
/// Composite keys are fully qualified and literal: ``pv01::USD-OIS``,
/// ``bucketed_dv01::USD-OIS::10y``, ``cs01::ACME-HZD``. Legacy escaped keys
/// persisted by earlier releases (``pv01::USD_x2dOIS``) still resolve through
/// ``get_metric``/``__getitem__``. Units differ by metric (``ytm`` is a decimal
/// rate, ``par_spread`` is basis points, ``dv01`` is currency per bp) —
/// ``metric_units()`` labels every key.
///
/// Tabular exits
/// -------------
/// ``to_dataframe()`` is one wide row (one column per metric);
/// ``to_long_dataframe()`` is tidy ``metric / curve / bucket / value`` rows,
/// the shape a risk desk pivots bucketed risk in;
/// ``metric_series_dataframe("bucketed_dv01")`` restricts the tidy view to one
/// base metric.
///
/// ``details`` is the optional tagged model-specific pricing payload; ``meta``
/// is the Rust ``ResultsMeta`` policy stamp (numeric mode, rounding, FX,
/// timing). Two results compare equal when their JSON documents are
/// identical, which includes ``meta`` timestamps.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import StubKind
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.core.types import Rate
/// >>> from finstack_quant.valuations.instruments import Bond, price_instrument
/// >>> as_of = datetime.date(2024, 1, 15)
/// >>> bond = Bond.fixed(
/// ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 15), StubKind.NONE, "USD-OIS"
/// ... )
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
/// >>> result = price_instrument(bond, market, as_of, metrics=["ytm", "dv01"])
/// >>> (result.instrument_id, round(result.price, 2), result.currency, sorted(result.metrics))
/// ('B', 1018.16, 'USD', ['dv01', 'ytm'])
/// >>> result.metric_units()["ytm"]
/// 'decimal'
#[pyclass(
    name = "ValuationResult",
    module = "finstack_quant.valuations",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyValuationResult {
    pub(crate) inner: finstack_quant_valuations::results::ValuationResult,
}

impl PyValuationResult {
    fn long_rows_dataframe<'py>(
        py: Python<'py>,
        rows: &[finstack_quant_valuations::results::ValuationLongRow],
    ) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            rows,
            &[
                ("metric", "str"),
                ("curve", "str"),
                ("bucket", "str"),
                ("value", "float64"),
            ],
        )
    }
}

#[pymethods]
impl PyValuationResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a ``ValuationResult`` from JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Wire-format result document (``schema_version`` 1).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or is not a valuation result envelope.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::results::ValuationResult = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid ValuationResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact JSON (the document ``from_json`` accepts).
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Instrument identifier assigned by the pricer.
    #[getter]
    fn instrument_id(&self) -> &str {
        &self.inner.instrument_id
    }

    /// Valuation date (T+0) for the calculation, as ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    /// Wire-format schema version of the result envelope (currently ``1``).
    #[getter]
    fn schema_version(&self) -> u32 {
        self.inner.schema_version.into()
    }

    /// Present value amount as a ``float`` in ``currency``.
    #[getter]
    fn get_price(&self) -> f64 {
        self.inner.value.amount()
    }

    /// Present value as a currency-tagged ``Money`` (exact Decimal amount).
    #[getter]
    fn value(&self) -> crate::bindings::core::money::PyMoney {
        convert::money_to_py(self.inner.value)
    }

    /// Return the exact Decimal price as a string, without a float round-trip.
    ///
    /// Unlike the ``price`` property (a lossy ``float``), this preserves the
    /// internal Decimal representation exactly. Pass the result to
    /// ``decimal.Decimal`` for lossless arithmetic in Python.
    ///
    /// Returns
    /// -------
    /// str
    ///     Exact decimal string of the valuation amount, e.g. ``"1000000.00"``.
    #[pyo3(text_signature = "($self)")]
    fn price_decimal(&self) -> String {
        self.inner.value.amount_decimal().to_string()
    }

    /// ISO-4217 code of the present value currency.
    #[getter]
    fn currency(&self) -> String {
        self.inner.value.currency().to_string()
    }

    /// All computed measures as ``{metric_key: value}`` in computation order.
    ///
    /// Keys are literal composite keys (``bucketed_dv01::USD-OIS::10y``).
    /// The present value is not a measure; read ``price`` / ``value``.
    #[getter]
    fn metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.measures {
            dict.set_item(key.as_str(), *value)?;
        }
        Ok(dict)
    }

    /// Return a scalar measure by key, or ``None`` when absent.
    ///
    /// Parameters
    /// ----------
    /// key : str
    ///     Canonical metric key (``"ytm"``, ``"dv01"``, ``"pv01::USD-OIS"``).
    ///     Lookup matches the stored wire key exactly.
    ///
    /// Returns
    /// -------
    /// float or None
    ///     Metric value, or ``None`` if the key is not present.
    #[pyo3(text_signature = "($self, key)")]
    fn get_metric(&self, key: &str) -> Option<f64> {
        self.inner.metric_str(key)
    }

    /// ``result[key]``: scalar measure by key.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``key`` is not a measure of this result; the message lists the
    ///     five closest keys present.
    fn __getitem__(&self, key: &str) -> PyResult<f64> {
        self.inner.metric_str(key).ok_or_else(|| {
            let closest = self.inner.closest_metric_keys(key, 5);
            let hint = if closest.is_empty() {
                String::new()
            } else {
                format!("; closest keys: {}", closest.join(", "))
            };
            pyo3::exceptions::PyKeyError::new_err(format!(
                "metric '{key}' is not on this result ({} measures){hint}",
                self.inner.measures.len()
            ))
        })
    }

    /// ``key in result``: whether a measure with this key (canonical wire form) is present.
    fn __contains__(&self, key: &str) -> bool {
        self.inner.metric_str(key).is_some()
    }

    /// Decoded component vectors and values for a composite base metric.
    ///
    /// Despite the ``_series`` suffix (which mirrors the Rust name) this is a
    /// plain ``list`` of ``(components, value)`` tuples, not a
    /// ``pandas.Series``; use ``metric_series_dataframe`` for the tabular
    /// form.
    ///
    /// Results preserve the underlying ``measures`` insertion order. Canonical
    /// wire keys decode to unique coordinate vectors.
    ///
    /// Parameters
    /// ----------
    /// base : str
    ///     Unqualified base metric such as ``"bucketed_dv01"`` or ``"pv01"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the base metric key is not canonically encoded.
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series(&self, base: &str) -> PyResult<Vec<(Vec<String>, f64)>> {
        let base = base.parse().map_err(crate::errors::core_to_py)?;
        Ok(self.inner.metric_series(&base))
    }

    /// Tidy ``DataFrame`` of one composite base metric.
    ///
    /// Columns: ``metric`` (the base), ``curve`` (first component),
    /// ``bucket`` (remaining components joined with ``::``, or ``None``) and
    /// ``value``. The scalar aggregate stored directly under ``base`` is
    /// excluded, matching ``metric_series``.
    ///
    /// Parameters
    /// ----------
    /// base : str
    ///     Unqualified base metric such as ``"bucketed_dv01"``.
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series_dataframe<'py>(
        &self,
        py: Python<'py>,
        base: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<finstack_quant_valuations::results::ValuationLongRow> = self
            .inner
            .to_long_rows()
            .into_iter()
            .filter(|row| row.metric == base && row.curve.is_some())
            .collect();
        Self::long_rows_dataframe(py, &rows)
    }

    /// Every measure as one tidy row.
    ///
    /// Columns: ``metric`` (base name), ``curve`` (first composite component
    /// or ``None``), ``bucket`` (second and later components joined with
    /// ``::``, or ``None``) and ``value``. Rows follow computation order.
    /// This is the shape to ``pivot(index="bucket", columns="curve")`` bucketed
    /// risk from.
    #[pyo3(text_signature = "($self)")]
    fn to_long_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::long_rows_dataframe(py, &self.inner.to_long_rows())
    }

    /// Measure keys in computation order (literal composite form).
    #[pyo3(text_signature = "($self)")]
    fn metric_keys(&self) -> Vec<String> {
        self.inner.measures.keys().map(|k| k.to_string()).collect()
    }

    /// Number of measures on this result.
    #[pyo3(text_signature = "($self)")]
    fn metric_count(&self) -> usize {
        self.inner.measures.len()
    }

    /// Unit family of every measure, keyed by metric key.
    ///
    /// Values are ``"currency"`` (PV components and currency-per-bump
    /// sensitivities such as ``dv01``/``cs01``/``vega``), ``"decimal"``
    /// (``ytm``, ``z_spread``, ``par_rate``, probabilities), ``"basis_points"``
    /// (``par_spread``), ``"years"`` (durations, WAL), ``"percent"``,
    /// ``"dimensionless"`` (ratios, counts, discount factors) or ``"unknown"``
    /// (custom metrics). Composite keys inherit their base metric's unit.
    #[pyo3(text_signature = "($self)")]
    fn metric_units<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, unit) in self.inner.metric_units() {
            dict.set_item(key, unit.as_str())?;
        }
        Ok(dict)
    }

    /// Whether every covenant passed (``True`` when none were evaluated).
    #[pyo3(text_signature = "($self)")]
    fn all_covenants_passed(&self) -> bool {
        self.inner.all_covenants_passed()
    }

    /// Identifiers of the covenants that failed, in report order.
    #[pyo3(text_signature = "($self)")]
    fn failed_covenants(&self) -> Vec<String> {
        self.inner
            .failed_covenants()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Per-covenant compliance reports keyed by covenant id, or ``None``.
    ///
    /// Each report is the Rust ``CovenantReport`` document
    /// (``covenant_type``, ``passed``, ``actual_value``, ``threshold``,
    /// ``headroom``, ``details``, ``meta``). Present only for instruments
    /// with covenants (loans, structured credit).
    #[getter]
    fn covenants<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .covenants
            .as_ref()
            .map(|covenants| serde_to_py(py, covenants))
            .transpose()
    }

    /// Computation explanation trace as a dict, or ``None`` when not enabled.
    #[getter]
    fn explanation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .explanation
            .as_ref()
            .map(|trace| serde_to_py(py, trace))
            .transpose()
    }

    /// Export the headline result as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``instrument_id``, ``as_of_date`` (ISO 8601 string), ``pv``,
    /// ``currency``, then one column per metric key in ``measures`` insertion
    /// order.
    ///
    /// This is the default export. It is built from the Rust crate's own
    /// ``ValuationResult::to_row`` flattener, so the Python frame and the
    /// Rust-side DataFrame rows cannot drift apart. Stack a book with
    /// ``pd.concat([r.to_dataframe() for r in results])``; instruments with
    /// different metric sets align on column name and leave ``NaN`` elsewhere.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = self.inner.to_row();
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &["instrument_id", "as_of_date", "pv", "currency"],
        )
    }

    /// Jupyter rich display: the ``to_dataframe()`` table.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Policy stamps: numeric mode, rounding context, FX policy, and timing.
    ///
    /// Same serde shape as the Rust ``ResultsMeta`` object already present on
    /// the WASM ``ValuationResult``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Model-specific structured pricing detail, or ``None``.
    ///
    /// Same tagged ``{type, data}`` shape as the Rust ``ValuationDetails``
    /// enum. A stochastic ``"rates_credit"`` bond uses
    /// ``type="monte_carlo"``; its
    /// data contains ``model_key``, sampling-only ``standard_error``, configured
    /// independent exercise-policy paths (``training_paths``) and the simulated
    /// count including antithetic partners (``training_simulated_paths``),
    /// configured independent make-whole-reference paths
    /// (``make_whole_training_paths``) and their simulated count
    /// (``make_whole_training_simulated_paths``), independent estimator paths
    /// (``estimator_paths``) and their total simulated count (``simulated_paths``),
    /// ``seed``, ``time_grid``, and variance-reduction flags. ``standard_error`` measures pricing-path
    /// sampling uncertainty under the frozen fitted exercise policy and excludes
    /// regression approximation, time-grid discretization, and model error. A
    /// training stage that did not run reports zero paths.
    /// Absent when the pricer emitted only the scalar envelope.
    #[getter]
    fn details<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .details
            .as_ref()
            .map(|details| serde_to_py(py, details))
            .transpose()
    }

    /// Structural equality: two results are equal when their JSON documents match.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        let lhs = serde_json::to_value(&self.inner).map_err(display_to_py)?;
        let rhs = serde_json::to_value(&other.borrow().inner).map_err(display_to_py)?;
        Ok(lhs == rhs)
    }

    fn __repr__(&self) -> String {
        format!(
            "ValuationResult(id={:?}, price={:.4}, currency={}, metrics={})",
            self.inner.instrument_id,
            self.inner.value.amount(),
            self.inner.value.currency(),
            self.inner.measures.len()
        )
    }
}

/// Validate a tagged instrument JSON payload and return the canonical envelope.
///
/// Parameters
/// ----------
/// json : str
///     A ``finstack_quant.instrument/1`` envelope. Bare instrument payloads
///     are rejected.
///
/// Returns
/// -------
/// str
///     Canonical instrument envelope for the validated instrument.
///
/// Raises
/// ------
/// KeyError
///     If the envelope references an identifier that cannot be resolved.
/// ValueError
///     If ``json`` is malformed, is not a canonical v1 envelope, or fails
///     instrument validation.
#[pyfunction]
fn validate_instrument_json(json: &str) -> PyResult<String> {
    finstack_quant_valuations::pricer::validate_instrument_json(json)
        .map_err(crate::errors::core_to_py)
}

/// Validate a payload as one exact instrument type and return the canonical envelope.
///
/// Pure delegation to the Rust
/// ``finstack_quant_valuations::pricer::validate_typed_instrument_json`` used
/// by the WASM typed FX classes' ``fromJson`` constructors.
///
/// Parameters
/// ----------
/// type_tag : str
///     Canonical instrument discriminator (e.g. ``"fx_forward"``).
/// json : str
///     A ``finstack_quant.instrument/1`` envelope for exactly that type.
///
/// Returns
/// -------
/// str
///     The canonical instrument envelope for the validated instrument.
///
/// Raises
/// ------
/// KeyError
///     If the envelope references an identifier that cannot be resolved.
/// ValueError
///     If ``json`` is malformed, carries a different instrument type, or
///     fails instrument validation.
#[pyfunction]
#[pyo3(text_signature = "(type_tag, json)")]
fn validate_typed_instrument_json(type_tag: &str, json: &str) -> PyResult<String> {
    finstack_quant_valuations::pricer::validate_typed_instrument_json(type_tag, json)
        .map_err(crate::errors::core_to_py)
}

/// Re-render a canonical instrument envelope as pretty-printed JSON.
///
/// Pure delegation to the Rust
/// ``finstack_quant_valuations::pricer::pretty_instrument_json`` used by the
/// WASM typed FX classes' ``toJson`` methods.
///
/// Parameters
/// ----------
/// json : str
///     A canonical ``finstack_quant.instrument/1`` envelope.
///
/// Returns
/// -------
/// str
///     The same envelope, pretty-printed.
///
/// Raises
/// ------
/// ValueError
///     If ``json`` is malformed or cannot be rendered.
#[pyfunction]
#[pyo3(text_signature = "(json)")]
fn pretty_instrument_json(json: &str) -> PyResult<String> {
    finstack_quant_valuations::pricer::pretty_instrument_json(json)
        .map_err(crate::errors::core_to_py)
}

/// Construct tagged bond instrument JSON from a cashflow schedule.
#[pyfunction]
#[pyo3(
    signature = (instrument_id, schedule_json, discount_curve_id, quoted_clean = None),
    text_signature = "(instrument_id, schedule_json, discount_curve_id, quoted_clean=None)"
)]
fn bond_from_cashflows_json(
    py: Python<'_>,
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> PyResult<String> {
    py.detach(|| {
        finstack_quant_valuations::instruments::fixed_income::bond::bond_from_cashflows_json(
            instrument_id,
            schedule_json,
            discount_curve_id,
            quoted_clean,
        )
        .map_err(crate::errors::core_to_py)
    })
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "valuations")?;
    let qual = crate::bindings::module_utils::set_submodule_package(
        parent,
        &m,
        "valuations",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;
    m.setattr(
        "__doc__",
        concat!(
            "Instrument pricing and risk metrics (bonds, swaps, options, credit, structured products).\n\n",
            "Where things live:\n",
            "- market data (DiscountCurve, ForwardCurve, HazardCurve, MarketContext, FxMatrix):\n",
            "  ``finstack_quant.core.market_data``; curve bootstrapping: ``finstack_quant.calibration``\n",
            "- instruments, builders and ``price_instrument``: ``finstack_quant.valuations.instruments``\n",
            "- results: ``ValuationResult`` here, plus ``instrument_cashflows`` for per-flow tables\n",
            "- composites, credit-derivative examples, listed-market catalog, JSON schemas:\n",
            "  ``.composite``, ``.credit_derivatives``, ``.market``, ``.schema``\n\n",
            "The module-level ``*_coupon_profile`` / ``cms_spread_option_intrinsic`` / \n",
            "``callable_range_accrual_accrued`` functions are deterministic exotic-rates helpers.",
        ),
    )?;

    m.add_class::<PyValuationResult>()?;
    composite::register(py, &m)?;
    exotic_rates::register(py, &m)?;
    credit_derivatives::register(py, &m)?;
    schema::register(py, &m)?;
    register_instruments(py, &m)?;
    register_market(py, &m)?;

    let all = PyList::new(
        py,
        [
            "ValuationResult",
            "tarn_coupon_profile",
            "snowball_coupon_profile",
            "inverse_floater_coupon_profile",
            "cms_spread_option_intrinsic",
            "callable_range_accrual_accrued",
            "composite",
            "credit_derivatives",
            "instruments",
            "market",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;

    Ok(())
}

fn register_instruments(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "instruments")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "instruments",
        "finstack_quant.valuations",
    )?;
    m.setattr(
        "__doc__",
        "JSON validation, pricing, metric, and cashflow helpers for valuation workflows.",
    )?;

    m.add_function(wrap_pyfunction!(validate_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_typed_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(pretty_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(bond_from_cashflows_json, &m)?)?;
    for name in [
        "bond_from_cashflows_json",
        "pretty_instrument_json",
        "validate_typed_instrument_json",
    ] {
        m.getattr(name)?
            .setattr("__module__", "finstack_quant.valuations.instruments")?;
    }
    instruments::register(py, &m)?;
    merton_mc::register(py, &m)?;
    typed_legs::register(py, &m)?;
    typed_rates::register(py, &m)?;
    typed_credit::register(py, &m)?;
    typed_equity::register(py, &m)?;
    typed_fx::register(py, &m)?;
    typed_structured_credit::register(py, &m)?;
    typed_revolving_credit::register(py, &m)?;
    typed_asset_backed_facility::register(py, &m)?;
    pricing::register(py, &m)?;
    structured_credit::register(&m)?;
    let mut exports = vec![
        "AssetPool",
        "BarrierCrossing",
        "Bond",
        "CDSIndex",
        "CDSIndexBuilder",
        "CDSTranche",
        "CDSTrancheBuilder",
        "CapFloor",
        "CapFloorBuilder",
        "ConvertibleBond",
        "ConvertibleBondBuilder",
        "CreditDefaultSwap",
        "CreditDefaultSwapBuilder",
        "EquityOption",
        "EquityOptionBuilder",
        "FixedLegSpec",
        "FloatLegSpec",
        "FxForward",
        "FxForwardBuilder",
        "FxOption",
        "FxOptionBuilder",
        "InterestRateSwap",
        "InterestRateSwapBuilder",
        "PremiumLegSpec",
        "ProtectionLegSpec",
        "RepLine",
        "StructuredCredit",
        "StructuredCreditBuilder",
        "Swaption",
        "SwaptionBuilder",
        "TermLoan",
        "Tranche",
        "TrancheBuilder",
        "TrancheStructure",
        "bond_from_cashflows_json",
        "instrument_cashflows_json",
        "list_models",
        "list_models_grouped",
        "list_standard_metrics",
        "list_standard_metrics_grouped",
        "pretty_instrument_json",
        "price_instrument",
        "validate_instrument_json",
        "validate_typed_instrument_json",
    ];
    exports.extend_from_slice(merton_mc::EXPORTS);
    exports.extend_from_slice(structured_credit::EXPORTS);
    exports.extend_from_slice(instruments::EXPORTS);
    exports.extend_from_slice(typed_legs::EXPORTS);
    exports.extend_from_slice(typed_rates::EXPORTS);
    exports.extend_from_slice(typed_credit::EXPORTS);
    exports.extend_from_slice(typed_equity::EXPORTS);
    exports.extend_from_slice(typed_fx::EXPORTS);
    exports.extend_from_slice(typed_structured_credit::EXPORTS);
    exports.extend_from_slice(typed_revolving_credit::EXPORTS);
    exports.extend_from_slice(typed_asset_backed_facility::EXPORTS);
    exports.extend_from_slice(pricing::EXPORTS);
    exports.sort_unstable();
    exports.dedup();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;
    Ok(())
}

fn register_market(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "market")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "market",
        "finstack_quant.valuations",
    )?;
    m.setattr(
        "__doc__",
        "Listed-market product coverage and exchange routing metadata.",
    )?;

    pricing::register_market(py, &m)?;
    market::register(py, &m)?;
    let mut exports = vec!["listed_product_catalog"];
    exports.extend_from_slice(market::EXPORTS);
    exports.sort_unstable();
    exports.dedup();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;
    Ok(())
}
