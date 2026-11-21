//! PnlAttribution Python wrapper.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_attribution::long_rows::{
    pnl_attribution_carry_rows, pnl_attribution_credit_factor_rows, pnl_attribution_long_rows,
    pnl_attribution_sensitivity_step_rows, pnl_attribution_waterfall_step_rows,
    pnl_attribution_wide_row,
};
use pyo3::prelude::*;

/// Column schema shared by the long-format detail exports.
const LONG_DETAIL_COLUMNS: [(&str, &str); 7] = [
    ("kind", "str"),
    ("factor", "str"),
    ("sub", "str"),
    ("key_a", "str"),
    ("key_b", "str"),
    ("amount", "float64"),
    ("currency", "str"),
];

/// Column schema of the waterfall step export.
const WATERFALL_STEP_COLUMNS: [(&str, &str); 6] = [
    ("step_index", "int64"),
    ("factor", "str"),
    ("pv_before", "float64"),
    ("pv_after", "float64"),
    ("step_pnl", "float64"),
    ("currency", "str"),
];

/// Column schema of the sensitivity step export.
const SENSITIVITY_STEP_COLUMNS: [(&str, &str); 10] = [
    ("step_index", "int64"),
    ("factor", "str"),
    ("tenor_years", "float64"),
    ("move_unit", "str"),
    ("sensitivity", "float64"),
    ("market_move", "float64"),
    ("explained_pnl", "float64"),
    ("gamma_pnl", "float64"),
    ("repriced_pv", "float64"),
    ("currency", "str"),
];

/// Column schema of the wide single-row export and of the batch table returned by `attribute_pnl_many`.
pub(crate) const WIDE_COLUMNS: [(&str, &str); 24] = [
    ("instrument_id", "str"),
    ("method", "str"),
    ("t0", "str"),
    ("t1", "str"),
    ("currency", "str"),
    ("total_pnl", "float64"),
    ("mark_to_market_pnl", "float64"),
    ("pv_t0", "float64"),
    ("pv_t1", "float64"),
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
        crate::bindings::json_bridge::serde_to_py(py, &self.inner)
    }

    /// Total P&L amount.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl.amount()
    }

    /// Raw mark-to-market P&L: ``val_t1 − val_t0`` with no intra-period
    /// cashflow adjustment.
    ///
    /// When the attribution method added economic cash, including principal
    /// receipts, to ``total_pnl`` (the total-return convention used by
    /// parallel/waterfall/Taylor),
    /// this field still reports the raw mark-to-market change so a downstream
    /// consumer can reconcile against their own computation. Returns ``None``
    /// when the attribution path or input payload does not supply the field.
    #[getter]
    fn mark_to_market_pnl(&self) -> Option<f64> {
        self.inner.mark_to_market_pnl.map(|m| m.amount())
    }

    /// Present value at T₀ on the T₀ market: the opening endpoint.
    ///
    /// ``mark_to_market_pnl == pv_t1 - pv_t0``. After a target-currency
    /// translation this is the opening value at T₀ FX. Returns ``None`` when
    /// the result was not produced from two endpoint valuations in
    /// ``currency`` (a composite built without endpoints, or a payload written
    /// before the field existed).
    #[getter]
    fn pv_t0(&self) -> Option<f64> {
        self.inner.pv_t0.map(|m| m.amount())
    }

    /// Present value at T₁ on the T₁ market: the closing endpoint.
    ///
    /// After a target-currency translation this is the closing value at T₁
    /// FX. Returns ``None`` under the same conditions as ``pv_t0``.
    #[getter]
    fn pv_t1(&self) -> Option<f64> {
        self.inner.pv_t1.map(|m| m.amount())
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

    /// Ordered waterfall repricing steps as serde-shaped dicts.
    ///
    /// One dict per applied factor with ``step_index``, ``factor``,
    /// ``pv_before``, ``pv_after`` and ``step_pnl`` (money values are
    /// ``{"amount", "currency"}`` dicts). ``step_pnl == pv_after - pv_before``
    /// and each ``pv_after`` is the next step's ``pv_before``. Empty unless
    /// the waterfall method produced the result. Use
    /// ``to_waterfall_steps_table`` for an ``ArrowTable`` with float columns.
    #[getter]
    fn waterfall_steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.waterfall_steps)
    }

    /// The working behind each Taylor or metrics-based factor, as
    /// serde-shaped dicts.
    ///
    /// One dict per factor with ``factor``, ``explained_pnl`` and, where they
    /// apply, ``move_unit``, ``sensitivity``, ``market_move``, ``buckets``
    /// (``tenor_years``, ``sensitivity``, ``market_move``), ``repriced_pv`` and
    /// ``gamma_pnl``. Empty for the parallel and waterfall methods. Use
    /// ``to_sensitivity_steps_table`` for an ``ArrowTable`` with float columns.
    #[getter]
    fn sensitivity_steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sensitivity_steps)
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

    /// Share of total P&L, in percent, that an amount represents.
    ///
    /// Computes ``amount / total_pnl * 100`` (Rust
    /// ``PnlAttribution::pct_of_total``); the zero test on ``total_pnl`` uses
    /// the run's rounding context, as ``residual_pct`` does.
    ///
    /// Parameters
    /// ----------
    /// amount : float
    ///     P&L amount in ``total_pnl`` currency units, normally one factor
    ///     field (``carry``, ``rates_curves_pnl``, ``residual``, ...) or
    ///     ``total_pnl`` itself.
    ///
    /// Returns
    /// -------
    /// float or None
    ///     Signed percentage points (``25.0`` = 25% of total P&L), or ``None``
    ///     when ``total_pnl`` is effectively zero and the share is undefined.
    #[pyo3(text_signature = "($self, amount)")]
    fn pct_of_total(&self, amount: f64) -> Option<f64> {
        self.inner.pct_of_total(amount)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.attribution._pnl_attribution", "PnlAttribution.pct_of_total_by_bucket")]
    #[pyo3(text_signature = "($self)")]
    fn pct_of_total_by_bucket<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.pct_of_total_by_bucket())
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
        self.inner
            .residual_within_tolerance(pct_tolerance, abs_tolerance)
    }

    /// Validate that every monetary value matches ``total_pnl.currency``.
    ///
    /// Useful before building a DataFrame or summing across instruments — a
    /// silent currency mismatch would otherwise be visible only in the raw
    /// ``to_dict()`` payload. Raises ``ValueError`` on mismatch.
    fn validate_currencies(&self) -> PyResult<()> {
        self.inner
            .validate_currencies()
            .map_err(crate::errors::core_to_py)
    }

    /// Human-readable tree explanation of the factor breakdown.
    #[pyo3(signature = (show_zeros = false))]
    fn explain(&self, show_zeros: bool) -> String {
        self.inner.explain(show_zeros)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.attribution._pnl_attribution", "PnlAttribution.compare")]
    #[staticmethod]
    fn compare(attributions: Vec<PyRef<'_, Self>>) -> PyResult<PyArrowTable> {
        let rows = attributions
            .iter()
            .map(|attribution| pnl_attribution_wide_row(&attribution.inner))
            .collect::<Result<Vec<_>, _>>()
            .map_err(core_to_py)?;
        serde_rows_to_arrow_table(&rows, &WIDE_COLUMNS)
    }

    /// Export every populated detail breakdown as a single long-format ``ArrowTable``.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency``. ``sub`` is ``kind`` with the ``factor.``
    /// prefix removed (``"by_curve"``, ``"coupon_income.rates"``).
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
    /// Returns an empty ``ArrowTable`` (zero rows, schema columns present) when no
    /// detail breakdown was populated.
    fn to_long_table(&self) -> PyResult<PyArrowTable> {
        let rows = pnl_attribution_long_rows(&self.inner);
        serde_rows_to_arrow_table(&rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the waterfall step chain as an ``ArrowTable``, one row per step in
    /// the order applied.
    ///
    /// Columns: ``step_index``, ``factor`` (snake-case factor name such as
    /// ``"rates_curves"``), ``pv_before``, ``pv_after``, ``step_pnl``
    /// (``pv_after - pv_before``), ``currency``. The first ``pv_before`` is
    /// ``pv_t0`` and each ``pv_after`` is the next row's ``pv_before``. The
    /// ``carry`` bucket is the carry row's ``step_pnl`` plus the period cash
    /// (``total_pnl - mark_to_market_pnl``); every other bucket equals its
    /// row's ``step_pnl``.
    ///
    /// Returns an empty ``ArrowTable`` (zero rows, schema columns present) unless
    /// the waterfall method produced the result.
    fn to_waterfall_steps_table(&self) -> PyResult<PyArrowTable> {
        let rows = pnl_attribution_waterfall_step_rows(&self.inner);
        serde_rows_to_arrow_table(&rows, &WATERFALL_STEP_COLUMNS)
    }

    /// Export the working behind each Taylor or metrics-based factor as an
    /// ``ArrowTable``.
    ///
    /// Columns: ``step_index``, ``factor``, ``tenor_years``, ``move_unit``
    /// (``"basis_point"``, ``"vol_point"``, ``"percent"``, ``"price_unit"``),
    /// ``sensitivity`` (currency per one ``move_unit``), ``market_move``,
    /// ``explained_pnl``, ``gamma_pnl``, ``repriced_pv``, ``currency``.
    ///
    /// Each factor has one factor-level row (``tenor_years`` null) carrying
    /// its P&L, followed by one row per key-rate bucket (``tenor_years`` set)
    /// carrying only that bucket's ``sensitivity`` and ``market_move``. P&L
    /// columns are null on bucket rows, so column sums do not double count.
    /// ``sensitivity * market_move`` reproduces ``explained_pnl`` on scalar
    /// rows, and summed over a factor's bucket rows on key-rate factors.
    ///
    /// Returns an empty ``ArrowTable`` (zero rows, schema columns present) for the
    /// parallel and waterfall methods.
    fn to_sensitivity_steps_table(&self) -> PyResult<PyArrowTable> {
        let rows = pnl_attribution_sensitivity_step_rows(&self.inner);
        serde_rows_to_arrow_table(&rows, &SENSITIVITY_STEP_COLUMNS)
    }

    /// Export the carry decomposition as a long-format ``ArrowTable``.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency`` as in :meth:`to_long_table`.
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
    /// Returns an empty ``ArrowTable`` (zero rows, schema columns present) when
    /// ``carry_detail`` is not populated.
    fn to_carry_detail_table(&self) -> PyResult<PyArrowTable> {
        let rows = pnl_attribution_carry_rows(&self.inner);
        serde_rows_to_arrow_table(&rows, &LONG_DETAIL_COLUMNS)
    }

    /// Export the credit-factor hierarchy decomposition as a long-format
    /// ``ArrowTable``.
    ///
    /// Columns: ``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
    /// ``amount``, ``currency`` as in :meth:`to_long_table`. ``kind`` is
    /// one of ``credit_factor.generic`` / ``credit_factor.level`` /
    /// ``credit_factor.level.by_bucket`` / ``credit_factor.adder`` /
    /// ``credit_factor.curve_shape`` / ``credit_factor.adder_by_issuer``;
    /// ``factor`` is always ``"credit_factor"``, ``key_a`` the level name or
    /// component, ``key_b`` the bucket path / issuer id when applicable.
    ///
    /// Returns an empty ``ArrowTable`` (zero rows, schema columns present) when
    /// ``credit_factor_detail`` is not populated (no ``credit_factor_model``
    /// was supplied, or the instrument has no resolvable issuer).
    fn to_credit_factor_table(&self) -> PyResult<PyArrowTable> {
        let rows = pnl_attribution_credit_factor_rows(&self.inner);
        serde_rows_to_arrow_table(&rows, &LONG_DETAIL_COLUMNS)
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

impl PyPnlAttribution {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let wide = pnl_attribution_wide_row(&self.inner).map_err(core_to_py)?;
        serde_object_to_single_row_arrow_table(&wide, &WIDE_COLUMNS)
    }
}

tabular_methods!(PyPnlAttribution);
