//! Python bindings for P&L attribution.
//!
//! Exposes the JSON-spec attribution pipeline and a `PnlAttribution` wrapper
//! for interactive exploration from Python.

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe, serde_rows_to_dataframe,
};
use crate::errors::{display_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::PyList;

// ---------------------------------------------------------------------------
// Ergonomic entry point
// ---------------------------------------------------------------------------

/// Run P&L attribution for a single instrument and return JSON.
///
/// This is the main entry point. It accepts the instrument, two market
/// snapshots, valuation dates, and a method descriptor — all as simple
/// Python objects — and returns the canonical JSON form of the attribution.
/// Use ``PnlAttribution.from_json(...)`` when you want the richer Python wrapper.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged instrument JSON (``{"type": "bond", "spec": {...}}``).
/// market_t0_json : str
///     JSON-serialized ``MarketContext`` at T₀.
/// market_t1_json : str
///     JSON-serialized ``MarketContext`` at T₁.
/// as_of_t0 : str
///     Valuation date T₀ as an ISO 8601 calendar date (``YYYY-MM-DD``).
///     Time-of-day and timezone offsets are not accepted; for time-of-day
///     -sensitive workflows pass the start-of-day calendar date in UTC.
/// as_of_t1 : str
///     Valuation date T₁ as an ISO 8601 calendar date (``YYYY-MM-DD``).
/// method : str | dict
///     Attribution method. One of:
///
///     * ``"Parallel"``
///     * ``{"Waterfall": ["Carry", "RatesCurves", ...]}``
///     * ``"MetricsBased"``
///     * ``{"Taylor": {"include_gamma": true, ...}}``
/// config : dict, optional
///     Optional attribution config overrides (tolerance, metrics, bump sizes).
///
/// Returns
/// -------
/// str
///     Compact JSON ``PnlAttribution`` payload.
///
/// Examples
/// --------
/// >>> attr_json = attribute_pnl(inst, mkt_t0, mkt_t1, "2025-01-15", "2025-01-16", "Parallel")
/// >>> attr = PnlAttribution.from_json(attr_json)
/// >>> print(attr.explain())
/// >>> attr.to_dataframe()
#[pyfunction]
#[pyo3(signature = (instrument_json, market_t0_json, market_t1_json, as_of_t0, as_of_t1, method, config=None, full_cross_attribution=None))]
#[allow(clippy::too_many_arguments)]
fn attribute_pnl(
    py: Python<'_>,
    instrument_json: &str,
    market_t0_json: &str,
    market_t1_json: &str,
    as_of_t0: &str,
    as_of_t1: &str,
    method: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    full_cross_attribution: Option<bool>,
) -> PyResult<String> {
    let method_json = py_to_json_string(py, method, "method")?;
    let config_json = config
        .map(|value| py_to_json_string(py, value, "config"))
        .transpose()?;
    let mut spec = finstack_attribution::AttributionSpec::from_json_inputs(
        instrument_json,
        market_t0_json,
        market_t1_json,
        as_of_t0,
        as_of_t1,
        &method_json,
        config_json.as_deref(),
    )
    .map_err(display_to_py)?;

    if let Some(val) = full_cross_attribution {
        spec.full_cross_attribution = val;
    }

    // GIL is released for the entire attribution computation. The closure body
    // accesses no Python objects (`spec` is a fully-deserialized Rust value
    // built from `&str` arguments above), so concurrent Python callers can run
    // attributions in parallel without serializing on the GIL. Rayon
    // parallelism inside `spec.execute()` is unaffected.
    let result = py.detach(|| spec.execute()).map_err(display_to_py)?;
    serde_json::to_string(&result.attribution).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Raw JSON envelope entry point (power-user / round-trip)
// ---------------------------------------------------------------------------

/// Run attribution from a full JSON ``AttributionEnvelope`` and return JSON.
///
/// This is the raw JSON round-trip variant. Most users should prefer
/// :func:`attribute_pnl` which accepts separate arguments and returns
/// a ``PnlAttribution`` directly.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``AttributionResultEnvelope``.
#[pyfunction]
fn attribute_pnl_from_spec(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    use finstack_attribution::AttributionEnvelope;

    let envelope: AttributionEnvelope = serde_json::from_str(spec_json).map_err(display_to_py)?;
    let result_envelope = py.detach(|| envelope.execute()).map_err(display_to_py)?;
    serde_json::to_string(&result_envelope).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate an attribution specification JSON.
///
/// Deserializes the input against the ``AttributionEnvelope`` schema and
/// returns the canonical (re-serialized) JSON.
///
/// Parameters
/// ----------
/// json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON.
#[pyfunction]
fn validate_attribution_json(json: &str) -> PyResult<String> {
    let envelope: finstack_attribution::AttributionEnvelope =
        serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid attribution JSON"))?;
    serde_json::to_string(&envelope).map_err(display_to_py)
}

/// Return the default waterfall factor ordering.
///
/// Returns
/// -------
/// list[str]
///     Factor names in the default waterfall order.
#[pyfunction]
fn default_waterfall_order() -> Vec<String> {
    finstack_attribution::default_waterfall_order()
        .into_iter()
        .map(|f| f.to_string())
        .collect()
}

/// Return the default metric IDs used by metrics-based attribution.
///
/// Returns
/// -------
/// list[str]
///     Metric identifier strings.
#[pyfunction]
fn default_attribution_metrics() -> Vec<String> {
    finstack_attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect()
}

/// Serialize a Python object to JSON via `json.dumps`.
fn py_to_json_string<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<String> {
    let json_mod = py.import("json")?;
    json_mod
        .call_method1("dumps", (obj,))
        .and_then(|value| value.extract())
        .map_err(|e| crate::errors::value_error(format!("invalid {label}: {e}")))
}

// ---------------------------------------------------------------------------
// PnlAttribution wrapper
// ---------------------------------------------------------------------------

/// P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent risk factors: carry, rates curves,
/// credit curves, inflation, correlations, FX, volatility, cross-factor
/// interactions, model parameters, market scalars, and residual.
///
/// Construct via :func:`attribute_pnl` or :meth:`from_json`.
#[pyclass(
    name = "PnlAttribution",
    module = "finstack.attribution",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPnlAttribution {
    pub(crate) inner: finstack_attribution::PnlAttribution,
}

#[pymethods]
impl PyPnlAttribution {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_attribution::PnlAttribution =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Export the canonical serde-shaped attribution payload as a Python dict.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let json = serde_json::to_string(&self.inner).map_err(display_to_py)?;
        let json_mod = py.import("json")?;
        json_mod.call_method1("loads", (json,))
    }

    // --- Aggregate P&L fields (amount as f64) ---

    /// Total P&L amount.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl.amount()
    }

    /// Raw mark-to-market P&L: ``val_t1 − val_t0`` with no intra-period
    /// cashflow adjustment.
    ///
    /// When the attribution method added coupon income to ``total_pnl``
    /// (the standard total-return convention used by parallel/waterfall/Taylor),
    /// this field still reports the raw mark-to-market change so a downstream
    /// consumer can reconcile against their own computation. Returns ``None``
    /// for attributions deserialized from a pre-audit JSON payload that did
    /// not carry the field.
    #[getter]
    fn mark_to_market_pnl(&self) -> Option<f64> {
        self.inner.mark_to_market_pnl.map(|m| m.amount())
    }

    /// Carry (theta + accruals) P&L amount.
    #[getter]
    fn carry(&self) -> f64 {
        self.inner.carry.amount()
    }

    /// Interest rate curves P&L amount.
    #[getter]
    fn rates_curves_pnl(&self) -> f64 {
        self.inner.rates_curves_pnl.amount()
    }

    /// Credit hazard curves P&L amount.
    #[getter]
    fn credit_curves_pnl(&self) -> f64 {
        self.inner.credit_curves_pnl.amount()
    }

    /// Inflation curves P&L amount.
    #[getter]
    fn inflation_curves_pnl(&self) -> f64 {
        self.inner.inflation_curves_pnl.amount()
    }

    /// Base correlation curves P&L amount.
    #[getter]
    fn correlations_pnl(&self) -> f64 {
        self.inner.correlations_pnl.amount()
    }

    /// FX rate changes P&L amount.
    ///
    /// Pricing-impact FX P&L for cross-currency instruments (FX matrix
    /// feeding into the instrument's own pricer). For pure single-currency
    /// instruments this is zero.
    #[getter]
    fn fx_pnl(&self) -> f64 {
        self.inner.fx_pnl.amount()
    }

    /// FX translation P&L amount.
    ///
    /// Reporting-currency FX P&L when the attribution was translated into a
    /// non-native ``target_ccy`` via ``AttributionConfig.target_ccy``. Equal
    /// to ``val_t0_native × (T1_fx − T0_fx)`` — the FX move applied to the
    /// opening position. Zero when the attribution stayed in its native
    /// currency (the default).
    #[getter]
    fn fx_translation_pnl(&self) -> f64 {
        self.inner.fx_translation_pnl.amount()
    }

    /// Implied volatility changes P&L amount.
    #[getter]
    fn vol_pnl(&self) -> f64 {
        self.inner.vol_pnl.amount()
    }

    /// Cross-factor interaction P&L amount.
    #[getter]
    fn cross_factor_pnl(&self) -> f64 {
        self.inner.cross_factor_pnl.amount()
    }

    /// Model parameters P&L amount.
    #[getter]
    fn model_params_pnl(&self) -> f64 {
        self.inner.model_params_pnl.amount()
    }

    /// Market scalars P&L amount.
    #[getter]
    fn market_scalars_pnl(&self) -> f64 {
        self.inner.market_scalars_pnl.amount()
    }

    /// Residual (unexplained) P&L amount.
    #[getter]
    fn residual(&self) -> f64 {
        self.inner.residual.amount()
    }

    /// Currency code for all P&L amounts.
    #[getter]
    fn currency(&self) -> String {
        self.inner.total_pnl.currency().to_string()
    }

    // --- Metadata ---

    /// Instrument identifier.
    #[getter]
    fn instrument_id(&self) -> &str {
        &self.inner.meta.instrument_id
    }

    /// Attribution method name.
    #[getter]
    fn method(&self) -> String {
        self.inner.meta.method.to_string()
    }

    /// Start date (T₀) as ISO string.
    #[getter]
    fn t0(&self) -> String {
        self.inner.meta.t0.to_string()
    }

    /// End date (T₁) as ISO string.
    #[getter]
    fn t1(&self) -> String {
        self.inner.meta.t1.to_string()
    }

    /// Number of repricings performed.
    #[getter]
    fn num_repricings(&self) -> usize {
        self.inner.meta.num_repricings
    }

    /// Residual as percentage of total P&L.
    #[getter]
    fn residual_pct(&self) -> f64 {
        self.inner.meta.residual_pct
    }

    /// Diagnostic notes.
    #[getter]
    fn notes(&self) -> Vec<String> {
        self.inner.meta.notes.clone()
    }

    /// True if the attribution was flagged invalid (e.g. a non-finite factor
    /// sensitivity, or a residual that could not be computed). When ``True``,
    /// ``residual`` / ``residual_pct`` are not meaningful and the tolerance
    /// checks return ``False``.
    #[getter]
    fn result_invalid(&self) -> bool {
        self.inner.result_invalid
    }

    /// Check whether the residual is within tolerance.
    ///
    /// With no arguments this uses the attribution's own stored,
    /// method-appropriate tolerances — identical to
    /// :meth:`residual_within_meta_tolerance` and consistent with the native
    /// (Rust) check. Pass explicit values to override either threshold.
    ///
    /// Parameters
    /// ----------
    /// pct_tolerance : float, optional
    ///     Percentage tolerance (e.g. 0.1 for 0.1%). Defaults to the
    ///     attribution's stored ``meta.tolerance_pct``.
    /// abs_tolerance : float, optional
    ///     Absolute tolerance. Defaults to the attribution's stored
    ///     ``meta.tolerance_abs``.
    ///
    /// Returns
    /// -------
    /// bool
    #[pyo3(signature = (pct_tolerance=None, abs_tolerance=None))]
    fn residual_within_tolerance(
        &self,
        pct_tolerance: Option<f64>,
        abs_tolerance: Option<f64>,
    ) -> bool {
        self.inner.residual_within_tolerance(
            pct_tolerance.unwrap_or(self.inner.meta.tolerance_pct),
            abs_tolerance.unwrap_or(self.inner.meta.tolerance_abs),
        )
    }

    /// Check whether the residual is within the attribution's stored,
    /// method-appropriate tolerances (``meta.tolerance_pct`` /
    /// ``meta.tolerance_abs``).
    ///
    /// This matches the native ``residual_within_meta_tolerance`` check and is
    /// the recommended pass/fail gate — the per-method tolerances differ
    /// (waterfall is far tighter than metrics-based or Taylor).
    ///
    /// Returns
    /// -------
    /// bool
    fn residual_within_meta_tolerance(&self) -> bool {
        self.inner.residual_within_meta_tolerance()
    }

    /// Validate that every factor's currency matches ``total_pnl.currency``.
    ///
    /// Useful before building a DataFrame or summing across instruments — a
    /// silent currency mismatch would otherwise be visible only in the raw
    /// ``to_dict()`` payload. Raises ``ValueError`` on mismatch.
    fn validate_currencies(&self) -> PyResult<()> {
        self.inner.validate_currencies().map_err(display_to_py)
    }

    /// Human-readable tree explanation (non-zero factors only).
    fn explain(&self) -> String {
        self.inner.explain()
    }

    /// Verbose tree explanation including zero-valued factors.
    fn explain_verbose(&self) -> String {
        self.inner.explain_verbose()
    }

    /// Export attribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``instrument_id``, ``method``, ``t0``, ``t1``, ``currency``,
    /// ``total_pnl``, ``mark_to_market_pnl`` (nullable), ``carry``,
    /// ``rates_curves_pnl``, ``credit_curves_pnl``, ``inflation_curves_pnl``,
    /// ``correlations_pnl``, ``fx_pnl``, ``vol_pnl``, ``cross_factor_pnl``,
    /// ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
    /// ``residual_pct``, ``num_repricings``, ``result_invalid``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `mark_to_market_pnl` is Option<Money>; serialize as null when missing
        // so pandas treats the column as a nullable float (consistent with the
        // additive serde extension on the Rust struct).
        let row = serde_json::json!({
            "instrument_id": self.inner.meta.instrument_id,
            "method": self.inner.meta.method.to_string(),
            "t0": self.inner.meta.t0.to_string(),
            "t1": self.inner.meta.t1.to_string(),
            "currency": self.inner.total_pnl.currency().to_string(),
            "total_pnl": self.inner.total_pnl.amount(),
            "mark_to_market_pnl": self.inner.mark_to_market_pnl.map(|m| m.amount()),
            "carry": self.inner.carry.amount(),
            "rates_curves_pnl": self.inner.rates_curves_pnl.amount(),
            "credit_curves_pnl": self.inner.credit_curves_pnl.amount(),
            "inflation_curves_pnl": self.inner.inflation_curves_pnl.amount(),
            "correlations_pnl": self.inner.correlations_pnl.amount(),
            "fx_pnl": self.inner.fx_pnl.amount(),
            "fx_translation_pnl": self.inner.fx_translation_pnl.amount(),
            "vol_pnl": self.inner.vol_pnl.amount(),
            "cross_factor_pnl": self.inner.cross_factor_pnl.amount(),
            "model_params_pnl": self.inner.model_params_pnl.amount(),
            "market_scalars_pnl": self.inner.market_scalars_pnl.amount(),
            "residual": self.inner.residual.amount(),
            "residual_pct": self.inner.meta.residual_pct,
            "num_repricings": self.inner.meta.num_repricings,
            // `result_invalid` lets downstream pipelines refuse to aggregate
            // attributions flagged invalid (non-finite sensitivities, residual
            // computation failures).
            "result_invalid": self.inner.result_invalid,
        });
        serde_object_to_single_row_dataframe(py, &row)
    }

    /// Export every populated detail breakdown as a single long-format DataFrame.
    ///
    /// Columns: ``kind``, ``factor``, ``key_a``, ``key_b``, ``amount``,
    /// ``currency``.
    ///
    /// ``kind`` is a dotted path identifying the row's origin
    /// (e.g. ``"rates.by_curve"``, ``"rates.by_tenor"``, ``"credit.by_curve"``,
    /// ``"fx.by_pair"``, ``"vol.by_surface"``, ``"cross_factor.by_pair"``,
    /// ``"scalars.dividends"``, ``"credit_factor.generic"``,
    /// ``"credit_factor.level"``, ``"credit_factor.adder"``,
    /// ``"credit_factor.curve_shape"``, ``"carry.theta"``,
    /// ``"carry.coupon_income"``, etc.). ``factor`` is the parent factor
    /// family (``"rates"``, ``"credit"``, ``"fx"``, ``"vol"``,
    /// ``"cross_factor"``, ``"scalars"``, ``"credit_factor"``, ``"carry"``,
    /// ``"inflation"``, ``"correlations"``, ``"model_params"``).
    ///
    /// ``key_a`` is the primary identifier (curve_id, pair label, surface_id,
    /// equity_id, level_name, sub-component name). ``key_b`` is the secondary
    /// key when present (tenor for per-tenor rows, ``to`` currency for FX
    /// pairs, bucket path for credit-factor per-bucket rows); ``None`` when
    /// only one dimension is meaningful.
    ///
    /// The DataFrame is empty (zero rows, schema columns present) when no
    /// detail breakdown was populated. Use ``df.query("kind.str.startswith('rates')")``
    /// or ``df.pivot_table(index="key_a", columns="key_b", values="amount")``
    /// to slice the desired view.
    fn to_long_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = build_long_detail_rows(&self.inner);
        serde_rows_to_dataframe(py, &rows)
    }

    /// Export the carry decomposition as a typed wide DataFrame.
    ///
    /// Columns: ``component`` (theta / coupon_income / pull_to_par / roll_down
    /// / funding_cost / total), ``amount``, ``currency``, ``rates_part``
    /// (nullable), ``credit_part`` (nullable). The ``rates_part`` / ``credit_part``
    /// columns are populated only when a ``CreditFactorModel`` was supplied to
    /// the attribution and the source line carries a typed split (PR-8b §7.1).
    ///
    /// Returns an empty DataFrame when ``carry_detail`` is not populated.
    fn to_carry_detail_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = build_carry_detail_rows(&self.inner);
        serde_rows_to_dataframe(py, &rows)
    }

    /// Export the credit-factor hierarchy decomposition as a typed long
    /// DataFrame.
    ///
    /// Columns: ``component`` (generic / level / adder / curve_shape /
    /// adder_by_issuer), ``level_name`` (nullable, populated for level rows),
    /// ``bucket`` (nullable, populated for per-bucket and per-issuer rows),
    /// ``amount``, ``currency``, ``model_id``.
    ///
    /// Returns an empty DataFrame when ``credit_factor_detail`` is not
    /// populated (no ``credit_factor_model`` was supplied, or the instrument
    /// has no resolvable issuer).
    fn to_credit_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = build_credit_factor_rows(&self.inner);
        serde_rows_to_dataframe(py, &rows)
    }

    fn __repr__(&self) -> String {
        format!(
            "PnlAttribution(id={:?}, method={}, total_pnl={:.2} {}, residual_pct={:.2}%)",
            self.inner.meta.instrument_id,
            self.inner.meta.method,
            self.inner.total_pnl.amount(),
            self.inner.total_pnl.currency(),
            self.inner.meta.residual_pct,
        )
    }
}

// ---------------------------------------------------------------------------
// Long-format detail DataFrame builders
// ---------------------------------------------------------------------------

/// Long-format row for the unified detail DataFrame (see
/// [`PyPnlAttribution::to_long_dataframe`]). Currency is owned because
/// `Currency::Display` allocates; the row is dropped immediately after
/// JSON serialization so the per-row String is cheap.
#[derive(serde::Serialize)]
struct LongDetailRow {
    kind: &'static str,
    factor: &'static str,
    key_a: String,
    key_b: Option<String>,
    amount: f64,
    currency: String,
}

fn build_long_detail_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();

    if let Some(detail) = &attribution.rates_detail {
        let ccy_str = attribution.rates_curves_pnl.currency().to_string();
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "rates.by_curve",
                factor: "rates",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow {
                kind: "rates.by_tenor",
                factor: "rates",
                key_a: curve_id.as_str().to_string(),
                key_b: Some(tenor.clone()),
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
        rows.push(LongDetailRow {
            kind: "rates.discount_total",
            factor: "rates",
            key_a: String::new(),
            key_b: None,
            amount: detail.discount_total.amount(),
            currency: ccy_str.clone(),
        });
        rows.push(LongDetailRow {
            kind: "rates.forward_total",
            factor: "rates",
            key_a: String::new(),
            key_b: None,
            amount: detail.forward_total.amount(),
            currency: ccy_str,
        });
    }

    if let Some(detail) = &attribution.credit_detail {
        let ccy_str = attribution.credit_curves_pnl.currency().to_string();
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "credit.by_curve",
                factor: "credit",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
        for ((curve_id, tenor), money) in &detail.by_tenor {
            rows.push(LongDetailRow {
                kind: "credit.by_tenor",
                factor: "credit",
                key_a: curve_id.as_str().to_string(),
                key_b: Some(tenor.clone()),
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    if let Some(detail) = &attribution.inflation_detail {
        let ccy_str = attribution.inflation_curves_pnl.currency().to_string();
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "inflation.by_curve",
                factor: "inflation",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
        if let Some(by_tenor) = &detail.by_tenor {
            for ((curve_id, tenor), money) in by_tenor {
                rows.push(LongDetailRow {
                    kind: "inflation.by_tenor",
                    factor: "inflation",
                    key_a: curve_id.as_str().to_string(),
                    key_b: Some(tenor.clone()),
                    amount: money.amount(),
                    currency: ccy_str.clone(),
                });
            }
        }
    }

    if let Some(detail) = &attribution.correlations_detail {
        let ccy_str = attribution.correlations_pnl.currency().to_string();
        for (curve_id, money) in &detail.by_curve {
            rows.push(LongDetailRow {
                kind: "correlations.by_curve",
                factor: "correlations",
                key_a: curve_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    if let Some(detail) = &attribution.fx_detail {
        let ccy_str = attribution.fx_pnl.currency().to_string();
        for ((from, to), money) in &detail.by_pair {
            rows.push(LongDetailRow {
                kind: "fx.by_pair",
                factor: "fx",
                key_a: from.to_string(),
                key_b: Some(to.to_string()),
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    if let Some(detail) = &attribution.vol_detail {
        let ccy_str = attribution.vol_pnl.currency().to_string();
        for (surface_id, money) in &detail.by_surface {
            rows.push(LongDetailRow {
                kind: "vol.by_surface",
                factor: "vol",
                key_a: surface_id.as_str().to_string(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    if let Some(detail) = &attribution.cross_factor_detail {
        let ccy_str = attribution.cross_factor_pnl.currency().to_string();
        for (pair_label, money) in &detail.by_pair {
            rows.push(LongDetailRow {
                kind: "cross_factor.by_pair",
                factor: "cross_factor",
                key_a: pair_label.clone(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    if let Some(detail) = &attribution.scalars_detail {
        let ccy_str = attribution.market_scalars_pnl.currency().to_string();
        let mut push_scalar_map = |kind: &'static str,
                                   map: &indexmap::IndexMap<
            finstack_core::types::CurveId,
            finstack_core::money::Money,
        >| {
            for (id, money) in map {
                rows.push(LongDetailRow {
                    kind,
                    factor: "scalars",
                    key_a: id.as_str().to_string(),
                    key_b: None,
                    amount: money.amount(),
                    currency: ccy_str.clone(),
                });
            }
        };
        push_scalar_map("scalars.dividends", &detail.dividends);
        push_scalar_map("scalars.inflation", &detail.inflation);
        push_scalar_map("scalars.equity_prices", &detail.equity_prices);
        push_scalar_map("scalars.commodity_prices", &detail.commodity_prices);
    }

    if let Some(detail) = &attribution.model_params_detail {
        let ccy_str = attribution.model_params_pnl.currency().to_string();
        let mut push_opt = |key: &'static str, money: &Option<finstack_core::money::Money>| {
            if let Some(m) = money {
                rows.push(LongDetailRow {
                    kind: "model_params.named",
                    factor: "model_params",
                    key_a: key.to_string(),
                    key_b: None,
                    amount: m.amount(),
                    currency: ccy_str.clone(),
                });
            }
        };
        push_opt("prepayment", &detail.prepayment);
        push_opt("default_rate", &detail.default_rate);
        push_opt("recovery_rate", &detail.recovery_rate);
        push_opt("conversion_ratio", &detail.conversion_ratio);
        for (k, money) in &detail.other {
            rows.push(LongDetailRow {
                kind: "model_params.other",
                factor: "model_params",
                key_a: k.clone(),
                key_b: None,
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    // Carry detail folded into the long view alongside the typed accessor.
    rows.extend(build_carry_detail_rows(attribution));

    // Credit-factor hierarchy folded into the long view alongside the typed
    // accessor. Per-bucket rows go through the same dotted-key convention as
    // the typed accessor for symmetry.
    rows.extend(build_credit_factor_rows(attribution));

    rows
}

fn build_carry_detail_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.carry_detail else {
        return rows;
    };
    let ccy_str = detail.total.currency().to_string();

    let mut push = |kind: &'static str, key_a: &str, money: &finstack_core::money::Money| {
        rows.push(LongDetailRow {
            kind,
            factor: "carry",
            key_a: key_a.to_string(),
            key_b: None,
            amount: money.amount(),
            currency: ccy_str.clone(),
        });
    };

    push("carry.total", "total", &detail.total);
    if let Some(theta) = &detail.theta {
        push("carry.theta", "theta", theta);
    }
    if let Some(ci) = &detail.coupon_income {
        push("carry.coupon_income", "total", &ci.total);
        if let Some(r) = &ci.rates_part {
            push("carry.coupon_income.rates", "rates_part", r);
        }
        if let Some(c) = &ci.credit_part {
            push("carry.coupon_income.credit", "credit_part", c);
        }
    }
    if let Some(ptp) = &detail.pull_to_par {
        push("carry.pull_to_par", "pull_to_par", ptp);
    }
    if let Some(rd) = &detail.roll_down {
        push("carry.roll_down", "total", &rd.total);
        if let Some(r) = &rd.rates_part {
            push("carry.roll_down.rates", "rates_part", r);
        }
        if let Some(c) = &rd.credit_part {
            push("carry.roll_down.credit", "credit_part", c);
        }
    }
    if let Some(fc) = &detail.funding_cost {
        push("carry.funding_cost", "funding_cost", fc);
    }

    rows
}

fn build_credit_factor_rows(
    attribution: &finstack_attribution::PnlAttribution,
) -> Vec<LongDetailRow> {
    let mut rows = Vec::new();
    let Some(detail) = &attribution.credit_factor_detail else {
        return rows;
    };
    let ccy_str = detail.generic_pnl.currency().to_string();

    rows.push(LongDetailRow {
        kind: "credit_factor.generic",
        factor: "credit_factor",
        key_a: "generic".to_string(),
        key_b: None,
        amount: detail.generic_pnl.amount(),
        currency: ccy_str.clone(),
    });
    for level in &detail.levels {
        rows.push(LongDetailRow {
            kind: "credit_factor.level",
            factor: "credit_factor",
            key_a: level.level_name.clone(),
            key_b: None,
            amount: level.total.amount(),
            currency: ccy_str.clone(),
        });
        for (bucket, money) in &level.by_bucket {
            rows.push(LongDetailRow {
                kind: "credit_factor.level.by_bucket",
                factor: "credit_factor",
                key_a: level.level_name.clone(),
                key_b: Some(bucket.clone()),
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }
    rows.push(LongDetailRow {
        kind: "credit_factor.adder",
        factor: "credit_factor",
        key_a: "adder".to_string(),
        key_b: None,
        amount: detail.adder_pnl_total.amount(),
        currency: ccy_str.clone(),
    });
    rows.push(LongDetailRow {
        kind: "credit_factor.curve_shape",
        factor: "credit_factor",
        key_a: "curve_shape".to_string(),
        key_b: None,
        amount: detail.curve_shape_pnl.amount(),
        currency: ccy_str.clone(),
    });
    if let Some(by_issuer) = &detail.adder_pnl_by_issuer {
        for (issuer_id, money) in by_issuer {
            rows.push(LongDetailRow {
                kind: "credit_factor.adder_by_issuer",
                factor: "credit_factor",
                key_a: "adder".to_string(),
                key_b: Some(issuer_id.as_str().to_string()),
                amount: money.amount(),
                currency: ccy_str.clone(),
            });
        }
    }

    rows
}

/// Register the attribution submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "attribution")?;
    m.setattr("__doc__", "P&L attribution across multiple methodologies.")?;
    m.add_class::<PyPnlAttribution>()?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl_from_spec, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_attribution_json, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_waterfall_order, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_attribution_metrics, &m)?)?;
    let all = PyList::new(
        py,
        [
            "PnlAttribution",
            "attribute_pnl",
            "attribute_pnl_from_spec",
            "validate_attribution_json",
            "default_waterfall_order",
            "default_attribution_metrics",
        ],
    )?;
    m.setattr("__all__", all)?;
    parent.add_submodule(&m)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("finstack.attribution", &m)?;
    Ok(())
}
