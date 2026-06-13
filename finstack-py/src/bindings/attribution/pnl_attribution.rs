//! PnlAttribution Python wrapper.

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe, serde_rows_to_dataframe,
    serde_rows_to_dataframe_with_schema,
};
use crate::errors::display_to_py;
use pyo3::prelude::*;

use super::dataframe::{build_carry_detail_rows, build_credit_factor_rows, build_long_detail_rows};

/// P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent risk factors: carry, rates curves,
/// credit curves, inflation, correlations, FX, volatility, cross-factor
/// interactions, model parameters, market scalars, and residual.
///
/// Construct via :func:`attribute_pnl` or :meth:`from_json`.
const LONG_DETAIL_COLUMNS: [&str; 6] = ["kind", "factor", "key_a", "key_b", "amount", "currency"];

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
    /// ``total_pnl``, ``mark_to_market_pnl`` (``None`` for payloads predating
    /// the field — note the column dtype is then ``object``, not ``float64``;
    /// coerce with ``pd.to_numeric`` before concatenating mixed vintages),
    /// ``carry``,
    /// ``rates_curves_pnl``, ``credit_curves_pnl``, ``inflation_curves_pnl``,
    /// ``correlations_pnl``, ``fx_pnl``, ``vol_pnl``, ``cross_factor_pnl``,
    /// ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
    /// ``residual_pct``, ``num_repricings``, ``result_invalid``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `mark_to_market_pnl` is Option<Money>; serialize as null when
        // missing. A null makes pandas infer dtype `object` for the column
        // (documented caveat above) — present values give `float64`.
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
        serde_rows_to_dataframe_with_schema(py, &rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the carry decomposition as a long-format DataFrame.
    ///
    /// Columns: ``kind`` (``carry.total`` / ``carry.theta`` /
    /// ``carry.coupon_income`` / ``carry.coupon_income.rates`` /
    /// ``carry.coupon_income.credit`` / ``carry.pull_to_par`` /
    /// ``carry.roll_down`` / ``carry.roll_down.rates`` /
    /// ``carry.roll_down.credit`` / ``carry.funding_cost``), ``factor``
    /// (always ``"carry"``), ``key_a``, ``key_b`` (always null here),
    /// ``amount``, ``currency``. The rates/credit split rows are present only
    /// when a ``CreditFactorModel`` was supplied to the attribution and the
    /// source line carries a typed split (PR-8b §7.1).
    ///
    /// Returns an empty DataFrame (zero rows, schema columns present) when
    /// ``carry_detail`` is not populated.
    fn to_carry_detail_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = build_carry_detail_rows(&self.inner);
        serde_rows_to_dataframe_with_schema(py, &rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the credit-factor hierarchy decomposition as a long-format
    /// DataFrame.
    ///
    /// Columns: ``kind`` (``credit_factor.generic`` / ``credit_factor.level``
    /// / ``credit_factor.level.by_bucket`` / ``credit_factor.adder`` /
    /// ``credit_factor.curve_shape`` / ``credit_factor.adder_by_issuer``),
    /// ``factor`` (always ``"credit_factor"``), ``key_a`` (level name or
    /// component), ``key_b`` (bucket path / issuer id when applicable),
    /// ``amount``, ``currency``.
    ///
    /// Returns an empty DataFrame (zero rows, schema columns present) when
    /// ``credit_factor_detail`` is not populated (no ``credit_factor_model``
    /// was supplied, or the instrument has no resolvable issuer).
    fn to_credit_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = build_credit_factor_rows(&self.inner);
        serde_rows_to_dataframe_with_schema(py, &rows, &LONG_DETAIL_COLUMNS)
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
