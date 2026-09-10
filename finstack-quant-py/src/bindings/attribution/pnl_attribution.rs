//! PnlAttribution Python wrapper.

use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_attribution::{
    pnl_attribution_carry_rows, pnl_attribution_credit_factor_rows, pnl_attribution_long_rows,
    pnl_attribution_wide_row,
};
use pyo3::prelude::*;

/// Column schema shared by the long-format detail exports.
const LONG_DETAIL_COLUMNS: [ColumnSchema<'static>; 7] = [
    ("kind", "str"),
    ("factor", "str"),
    ("sub", "str"),
    ("key_a", "str"),
    ("key_b", "str"),
    ("amount", "float64"),
    ("currency", "str"),
];

/// Column schema of the wide single-row export (`to_dataframe`) and of the
/// batch table returned by `attribute_pnl_many`.
pub(crate) const WIDE_COLUMNS: [ColumnSchema<'static>; 22] = [
    ("instrument_id", "str"),
    ("method", "str"),
    ("t0", "str"),
    ("t1", "str"),
    ("currency", "str"),
    ("total_pnl", "float64"),
    ("mark_to_market_pnl", "float64"),
    ("carry", "float64"),
    ("rates_curves_pnl", "float64"),
    ("credit_curves_pnl", "float64"),
    ("inflation_curves_pnl", "float64"),
    ("correlations_pnl", "float64"),
    ("fx_pnl", "float64"),
    ("fx_translation_pnl", "float64"),
    ("vol_pnl", "float64"),
    ("cross_factor_pnl", "float64"),
    ("model_params_pnl", "float64"),
    ("market_scalars_pnl", "float64"),
    ("residual", "float64"),
    ("residual_pct", "float64"),
    ("num_repricings", "int64"),
    ("result_invalid", "bool"),
];

/// P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent risk factors: carry, rates curves,
/// credit curves, inflation, correlations, FX, volatility, cross-factor
/// interactions, model parameters, market scalars, and residual.
///
/// Construct via ``attribute_pnl`` or ``from_json``.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import PnlAttribution
/// >>> try:
/// ...     PnlAttribution.from_json("{}")
/// ... except ValueError as exc:
/// ...     "total_pnl" in str(exc)
/// True
#[pyclass(
    name = "PnlAttribution",
    module = "finstack_quant.attribution",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPnlAttribution {
    pub(crate) inner: finstack_quant_attribution::PnlAttribution,
}

#[pymethods]
impl PyPnlAttribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the JSON does not match the wire schema.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_attribution::PnlAttribution = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid PnlAttribution JSON"))?;
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
    /// non-native ``target_currency`` via ``AttributionConfig.target_currency``. Equal
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

    /// Instrument identifier.
    #[getter]
    fn instrument_id(&self) -> &str {
        &self.inner.meta.instrument_id
    }

    /// Canonical snake-case attribution method name.
    #[getter]
    fn method(&self) -> String {
        self.inner.meta.method.as_str().to_owned()
    }

    /// Start date (T₀) as ``datetime.date``.
    #[getter]
    fn t0<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.meta.t0)
    }

    /// End date (T₁) as ``datetime.date``.
    #[getter]
    fn t1<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.meta.t1)
    }

    /// Risk metric ids the attribution method consumes.
    ///
    /// Non-empty only for ``metrics_based`` (``theta``, ``dv01``, ``cs01``,
    /// ``bucketed_cs01``, ``vega``, ... plus the second-order terms); the
    /// repricing methods return an empty list because they do not use
    /// pre-computed metrics.
    fn required_metrics(&self) -> Vec<String> {
        self.inner
            .meta
            .method
            .required_metrics()
            .iter()
            .map(ToString::to_string)
            .collect()
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

    /// Absolute tolerance used for residual validation (``meta.tolerance_abs``).
    #[getter]
    fn tolerance_abs(&self) -> f64 {
        self.inner.meta.tolerance_abs
    }

    /// Percentage tolerance used for residual validation (``meta.tolerance_pct``).
    #[getter]
    fn tolerance_pct(&self) -> f64 {
        self.inner.meta.tolerance_pct
    }

    /// Rounding context in force for the run, as a serde-shaped dict.
    #[getter]
    fn rounding<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta.rounding)
    }

    /// FX policy metadata as a serde-shaped dict, or ``None`` when no FX
    /// conversions were applied.
    #[getter]
    fn fx_policy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta.fx_policy)
    }

    /// Execution policy the attribution ran under (``"serial"`` /
    /// ``"parallel"``), or ``None`` for methods without a policy knob
    /// (metrics-based).
    #[getter]
    fn execution_policy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta.execution_policy)
    }

    /// Carry decomposition detail as a serde-shaped dict, or ``None`` when not
    /// populated. Gross carry equals net CarryTotal plus FundingCost on the metrics path. Keys mirror the Rust ``CarryDetail`` wire schema (``total``,
    /// ``coupon_income``, ``pull_to_par``, ``roll_down``, ``funding_cost``).
    #[getter]
    fn carry_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.carry_detail)
    }

    /// Rates-curves detail (``by_curve``, ``by_tenor``, ``discount_total``,
    /// ``forward_total``) as a serde-shaped dict, or ``None`` when not
    /// populated.
    #[getter]
    fn rates_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rates_detail)
    }

    /// Credit-curves detail (``by_curve``, ``by_tenor``) as a serde-shaped
    /// dict, or ``None`` when not populated.
    #[getter]
    fn credit_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_detail)
    }

    /// Inflation-curves detail (``by_curve``, optional ``by_tenor``) as a
    /// serde-shaped dict, or ``None`` when not populated.
    #[getter]
    fn inflation_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.inflation_detail)
    }

    /// Base-correlation detail (``by_curve``) as a serde-shaped dict, or
    /// ``None`` when not populated.
    #[getter]
    fn correlations_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.correlations_detail)
    }

    /// FX detail (``by_pair``, keyed ``"FROM/TO"``) as a serde-shaped dict, or
    /// ``None`` when not populated.
    #[getter]
    fn fx_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.fx_detail)
    }

    /// Volatility detail (``by_surface``) as a serde-shaped dict, or ``None``
    /// when not populated.
    #[getter]
    fn vol_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.vol_detail)
    }

    /// Cross-factor interaction detail (``total``, ``by_pair``) as a
    /// serde-shaped dict, or ``None`` when not populated.
    #[getter]
    fn cross_factor_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cross_factor_detail)
    }

    /// Model-parameter detail (``prepayment``, ``default_rate``,
    /// ``recovery_rate``, ``conversion_ratio``, ``other``) as a serde-shaped
    /// dict, or ``None`` when not populated.
    #[getter]
    fn model_params_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.model_params_detail)
    }

    /// Market-scalars detail (``dividends``, ``inflation``, ``equity_prices``,
    /// ``commodity_prices``) as a serde-shaped dict, or ``None`` when not
    /// populated.
    #[getter]
    fn scalars_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scalars_detail)
    }

    /// Credit-factor hierarchy decomposition (``model_id``, ``generic_pnl``,
    /// ``levels``, ``adder_pnl_total``, ``curve_shape_pnl``, ...) as a
    /// serde-shaped dict, or ``None`` when no ``credit_factor_model`` was
    /// supplied.
    #[getter]
    fn credit_factor_detail<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_factor_detail)
    }

    /// Factor-cut decomposition of carry under a credit factor model
    /// (``rates_carry_total``, ``credit_carry_total``, ``credit_by_level``)
    /// as a serde-shaped dict, or ``None`` when not populated.
    #[getter]
    fn credit_carry_decomposition<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_carry_decomposition)
    }

    /// Check whether the residual is within tolerance.
    ///
    /// With no arguments this uses the attribution's own stored,
    /// method-appropriate tolerances. Pass explicit values to override
    /// either threshold.
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
    /// Raises ``ValueError`` if any factor is denominated in a currency other
    /// than ``total_pnl``'s. The row carries ONE ``currency`` label beside every
    /// factor amount, so a mixed-currency attribution would make
    /// ``df[factor_cols].sum(axis=1)`` add unlike units — this is the same check
    /// :meth:`validate_currencies` performs, applied before the frame is built.
    /// The long format (:meth:`to_long_dataframe`) carries currency per row and
    /// has no such restriction.
    ///
    /// Columns: ``instrument_id``, ``method``, ``t0``, ``t1``, ``currency``,
    /// ``total_pnl``, ``mark_to_market_pnl`` (``None`` for payloads predating
    /// the field — note the column dtype is then ``object``, not ``float64``;
    /// coerce with ``pd.to_numeric`` before concatenating mixed vintages),
    /// ``carry``,
    /// ``rates_curves_pnl``, ``credit_curves_pnl``, ``inflation_curves_pnl``,
    /// ``correlations_pnl``, ``fx_pnl``, ``fx_translation_pnl``, ``vol_pnl``,
    /// ``cross_factor_pnl``,
    /// ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
    /// ``residual_pct``, ``num_repricings``, ``result_invalid``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // One `currency` column labels every factor amount on this row, so the
        // frame is only meaningful when the factors agree with `total_pnl`.
        // Without this, a EUR `fx_pnl` beside a USD total is presented as
        // comparable and `df[factors].sum(axis=1)` silently adds unlike units.
        let wide = pnl_attribution_wide_row(&self.inner).map_err(display_to_py)?;
        let names: Vec<&str> = WIDE_COLUMNS.iter().map(|(name, _)| *name).collect();
        serde_object_to_single_row_dataframe_with_schema(py, &wide, &names)
    }

    /// Export every populated detail breakdown as a single long-format DataFrame.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency``. ``sub`` is ``kind`` with the ``factor.``
    /// prefix removed (``"by_curve"``, ``"coupon_income.rates"``), so
    /// ``df.pivot_table(index="factor", columns="sub", values="amount")``
    /// works without string surgery.
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
    /// ``key_a`` is the primary identifier (curve_id, pair label, vol_surface_id,
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
        let rows = pnl_attribution_long_rows(&self.inner);
        serde_rows_to_dataframe_with_schema(py, &rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the carry decomposition as a long-format DataFrame.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency`` as in :meth:`to_long_dataframe`.
    /// ``kind`` is one of ``carry.total`` / ``carry.theta`` /
    /// ``carry.coupon_income`` / ``carry.coupon_income.rates`` /
    /// ``carry.coupon_income.credit`` / ``carry.pull_to_par`` /
    /// ``carry.roll_down`` / ``carry.roll_down.rates`` /
    /// ``carry.roll_down.credit`` / ``carry.funding_cost``; ``factor`` is
    /// always ``"carry"`` and ``key_b`` always null here. The rates/credit
    /// split rows are present only
    /// when a ``CreditFactorModel`` was supplied to the attribution and the
    /// source line carries a typed split (PR-8b §7.1).
    ///
    /// Returns an empty DataFrame (zero rows, schema columns present) when
    /// ``carry_detail`` is not populated.
    fn to_carry_detail_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = pnl_attribution_carry_rows(&self.inner);
        serde_rows_to_dataframe_with_schema(py, &rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the credit-factor hierarchy decomposition as a long-format
    /// DataFrame.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency`` as in :meth:`to_long_dataframe`. ``kind`` is
    /// one of ``credit_factor.generic`` / ``credit_factor.level`` /
    /// ``credit_factor.level.by_bucket`` / ``credit_factor.adder`` /
    /// ``credit_factor.curve_shape`` / ``credit_factor.adder_by_issuer``;
    /// ``factor`` is always ``"credit_factor"``, ``key_a`` the level name or
    /// component, ``key_b`` the bucket path / issuer id when applicable.
    ///
    /// Returns an empty DataFrame (zero rows, schema columns present) when
    /// ``credit_factor_detail`` is not populated (no ``credit_factor_model``
    /// was supplied, or the instrument has no resolvable issuer).
    fn to_credit_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = pnl_attribution_credit_factor_rows(&self.inner);
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

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
