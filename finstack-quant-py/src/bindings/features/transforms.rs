//! Row-aligned panel feature transform Python functions.

use super::ops::{PyCrossSectionalOp, PyPairwiseOp, PyTimeSeriesOp};
use super::parse::{
    extract_exposures, extract_keys, extract_op, extract_opt_f64_vec, parse_params,
};
use crate::errors::core_to_py;
use finstack_quant_features::{CrossSectionalOp, PairwiseOp, TimeSeriesOp};
use pyo3::prelude::*;

/// Transform a time-series panel column per entity.
///
/// Rows are grouped by ``entity`` and ordered lexicographically by ``order``
/// (use date objects or uniform-timezone, fixed-precision ISO strings).
/// ``periods`` counts finite observations; rolling ``window``s span rows and
/// require ``min_periods <= window`` finite rows. Rolling aggregates may emit
/// at missing current rows; current-value transforms require finite inputs.
/// EWMA requires ``span >= 1`` and uses centered biased variance; volatility
/// is missing initially and zero for constant series after two finite rows.
/// Rolling slope uses row positions, retaining gaps from missing values.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned observations (levels for ``returns``/``drawdown``,
///     returns for EWMA and Sharpe ops).
/// entity : sequence
///     Row-aligned entity keys (str or date-like; aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence
///     Row-aligned sort keys within each entity (ISO strings or dates; ints are rejected).
/// op : str or TimeSeriesOp
///     Operation name, e.g. ``"returns"``, ``"rolling_mean"``, ``"ewma_vol"``.
/// params : dict, optional
///     Operation parameters (``periods``, ``window``, ``min_periods``,
///     ``span``, ``half_life``, ``quantile``, ``risk_free``, ``lower``,
///     ``upper``, ``threshold``). Unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     One output per input row, in input order.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, ``op`` is unknown, a parameter is malformed or not
///     read by ``op``, keys mix aware/naive datetimes, or arithmetic is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_timeseries
/// >>> transform_timeseries([100.0, 102.0], ["A", "A"], ["2026-01-01", "2026-01-02"], "returns")
/// [None, 0.020000000000000018]
#[pyfunction]
#[pyo3(
    signature = (values, entity, order, op, params=None),
    text_signature = "(values, entity, order, op, params=None)"
)]
pub(super) fn transform_timeseries(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let entity = extract_keys(entity, "entity")?;
    let order = extract_keys(order, "order")?;
    let op = extract_op::<PyTimeSeriesOp, TimeSeriesOp>(op, |o| o.inner)?;
    let params = parse_params(py, params, "time-series transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_timeseries(&values, &entity, &order, op, params.as_ref())
    })
    .map_err(core_to_py)
}

/// Transform a cross-section per timestamp.
///
/// ``cap_weights`` enforces final absolute caps with zero net and unit gross
/// exposure. ``0 < max_abs <= 1``; each demeaned-signal side must support
/// gross 0.5 at that cap or the call fails. Constant signals produce zeros.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned observations; ``None`` / NaN are skipped.
/// time_key : sequence
///     Row-aligned partition keys (str or date-like; aware datetimes normalize to UTC; strings stay opaque).
/// op : str or CrossSectionalOp
///     Operation name, e.g. ``"zscore"``, ``"rank"``, ``"winsorize"``.
/// params : dict, optional
///     Operation parameters (``buckets``, ``lower``, ``upper``, ``sigma``,
///     ``max_abs``, ``value``). Unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     One output per input row, in input order.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, ``op`` is unknown, or a parameter is malformed or
///     not read by ``op``.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_cross_sectional
/// >>> transform_cross_sectional([1.0, 3.0], ["2026-01-01"] * 2, "rank")
/// [0.0, 1.0]
#[pyfunction]
#[pyo3(
    signature = (values, time_key, op, params=None),
    text_signature = "(values, time_key, op, params=None)"
)]
pub(super) fn transform_cross_sectional(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let time_key = extract_keys(time_key, "time_key")?;
    let op = extract_op::<PyCrossSectionalOp, CrossSectionalOp>(op, |o| o.inner)?;
    let params = parse_params(py, params, "cross-sectional transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_cross_sectional(&values, &time_key, op, params.as_ref())
    })
    .map_err(core_to_py)
}

/// Transform a cross-section within each ``(time_key, group)`` sub-partition.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned observations.
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
/// groups : sequence
///     Row-aligned group labels (sector, country, ...; aware datetimes normalize to UTC; strings stay opaque).
/// op : str or CrossSectionalOp
///     Cross-sectional operation name.
/// params : dict, optional
///     Operation parameters; unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     One output per input row.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, ``op`` is unknown, or parameters are malformed.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_cross_sectional_grouped
/// >>> transform_cross_sectional_grouped([1.0, 2.0, 3.0, 4.0], ["d"] * 4, ["x", "x", "y", "y"], "demean")
/// [-0.5, 0.5, -0.5, 0.5]
#[pyfunction]
#[pyo3(
    signature = (values, time_key, groups, op, params=None),
    text_signature = "(values, time_key, groups, op, params=None)"
)]
pub(super) fn transform_cross_sectional_grouped(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
    groups: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let time_key = extract_keys(time_key, "time_key")?;
    let groups = extract_keys(groups, "groups")?;
    let op = extract_op::<PyCrossSectionalOp, CrossSectionalOp>(op, |o| o.inner)?;
    let params = parse_params(py, params, "grouped cross-sectional transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_cross_sectional_grouped(
            &values,
            &time_key,
            &groups,
            op,
            params.as_ref(),
        )
    })
    .map_err(core_to_py)
}

/// Remove cross-sectional exposure effects by OLS residualization.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned signal values.
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
/// exposures : list[list[float | None]]
///     One row-aligned column per exposure (beta, size, sector dummies, ...).
/// params : dict, optional
///     ``fit_intercept`` (bool, default True). Unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     OLS residuals per row; ``None`` where inputs are missing.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, or a partition has fewer complete rows than
///     columns or a singular design matrix (the message names the key).
///
/// Examples
/// --------
/// >>> from finstack_quant.features import neutralize
/// >>> out = neutralize([1.0, 2.0, 3.0], ["d"] * 3, [[1.0, 2.0, 3.0]])
/// >>> [round(v, 12) for v in out]
/// [0.0, 0.0, 0.0]
#[pyfunction]
#[pyo3(
    signature = (values, time_key, exposures, params=None),
    text_signature = "(values, time_key, exposures, params=None)"
)]
pub(super) fn neutralize(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
    exposures: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let exposures = extract_exposures(exposures)?;
    let time_key = extract_keys(time_key, "time_key")?;
    let params = parse_params(py, params, "neutralize params")?;
    py.detach(move || {
        finstack_quant_features::neutralize(&values, &time_key, &exposures, params.as_ref())
    })
    .map_err(core_to_py)
}

/// Transform two time-series panel columns per entity (rolling cov/corr/beta).
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned left series (e.g. asset returns).
/// other : list[float | None]
///     Row-aligned right series (e.g. benchmark returns).
/// entity : sequence
///     Row-aligned entity keys (aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence
///     Row-aligned sort keys within each entity.
/// op : str or PairwiseOp
///     ``"rolling_cov"``, ``"rolling_corr"`` or ``"rolling_beta"``.
/// params : dict, optional
///     ``window`` (default 1) and ``min_periods`` (default ``window``, at
///     least 2 paired finite rows). Unknown keys raise ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     One output per input row.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, ``op`` is unknown, or parameters are malformed.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_timeseries_pairwise
/// >>> out = transform_timeseries_pairwise([1.0, 2.0, 3.0], [2.0, 4.0, 6.0], ["A"] * 3, ["1", "2", "3"], "rolling_beta", {"window": 3})
/// >>> round(out[2], 12)
/// 0.5
#[pyfunction]
#[pyo3(
    signature = (values, other, entity, order, op, params=None),
    text_signature = "(values, other, entity, order, op, params=None)"
)]
pub(super) fn transform_timeseries_pairwise(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    other: &Bound<'_, PyAny>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let other = extract_opt_f64_vec(other, "other")?;
    let entity = extract_keys(entity, "entity")?;
    let order = extract_keys(order, "order")?;
    let op = extract_op::<PyPairwiseOp, PairwiseOp>(op, |o| o.inner)?;
    let params = parse_params(py, params, "pairwise time-series transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_timeseries_pairwise(
            &values,
            &other,
            &entity,
            &order,
            op,
            params.as_ref(),
        )
    })
    .map_err(core_to_py)
}

/// Return rolling OLS residuals per entity.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned dependent series.
/// exposures : list[list[float | None]]
///     One row-aligned regressor column each.
/// entity : sequence
///     Row-aligned entity keys (aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence
///     Row-aligned sort keys within each entity.
/// params : dict, optional
///     ``window``, ``min_periods``, ``fit_intercept``. Unknown keys raise
///     ``ValueError``.
///
/// Returns
/// -------
/// list[float | None]
///     Residual of the latest row in each trailing window.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ or parameters are malformed.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import rolling_regression_residual
/// >>> out = rolling_regression_residual([1.0, 2.0, 3.0], [[1.0, 2.0, 3.0]], ["A"] * 3, ["1", "2", "3"], {"window": 3})
/// >>> round(out[2], 12)
/// 0.0
#[pyfunction]
#[pyo3(
    signature = (values, exposures, entity, order, params=None),
    text_signature = "(values, exposures, entity, order, params=None)"
)]
pub(super) fn rolling_regression_residual(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    exposures: &Bound<'_, PyAny>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let exposures = extract_exposures(exposures)?;
    let entity = extract_keys(entity, "entity")?;
    let order = extract_keys(order, "order")?;
    let params = parse_params(py, params, "rolling regression residual params")?;
    py.detach(move || {
        finstack_quant_features::rolling_regression_residual(
            &values,
            &exposures,
            &entity,
            &order,
            params.as_ref(),
        )
    })
    .map_err(core_to_py)
}

/// Convert a signal to inverse-risk-scaled long/short weights per timestamp.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned signal values.
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
/// volatility : list[float | None]
///     Row-aligned nonnegative volatility estimates in common units and horizon.
///     Zero, missing, and non-finite volatility produce ``None``; negative values fail.
///
/// Returns
/// -------
/// list[float | None]
///     Dollar-neutral weights per timestamp (long and short legs each have gross 0.5).
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, volatility is negative, keys mix aware/naive
///     datetimes, or arithmetic produces a non-finite result.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import risk_scaled_weights
/// >>> risk_scaled_weights([1.0, -1.0], ["d", "d"], [0.1, 0.1])
/// [0.5, -0.5]
#[pyfunction]
#[pyo3(
    signature = (values, time_key, volatility),
    text_signature = "(values, time_key, volatility)"
)]
pub(super) fn risk_scaled_weights(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
    volatility: &Bound<'_, PyAny>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let volatility = extract_opt_f64_vec(volatility, "volatility")?;
    let time_key = extract_keys(time_key, "time_key")?;
    py.detach(move || finstack_quant_features::risk_scaled_weights(&values, &time_key, &volatility))
        .map_err(core_to_py)
}

/// Convert ranks into dollar-neutral long/short weights per timestamp.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned signal values (ranked internally).
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
///
/// Returns
/// -------
/// list[float | None]
///     Weights per row; long and short legs each have gross 0.5 within a timestamp.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import rank_to_weights
/// >>> rank_to_weights([1.0, 2.0, 3.0], ["d"] * 3)
/// [-0.5, 0.0, 0.5]
#[pyfunction]
#[pyo3(signature = (values, time_key), text_signature = "(values, time_key)")]
pub(super) fn rank_to_weights(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let time_key = extract_keys(time_key, "time_key")?;
    py.detach(move || finstack_quant_features::rank_to_weights(&values, &time_key))
        .map_err(core_to_py)
}

/// Neutralize a signal with an intercept and z-score residuals.
///
/// The intercept is always fitted to preserve factor neutrality after
/// standardization. Exact-fit roundoff residuals become zero.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned signal values.
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
/// exposures : list[list[float | None]]
///     One row-aligned exposure column each.
///
/// Returns
/// -------
/// list[float | None]
///     Z-scored residuals per row; exact fits produce zero signals.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, a partition cannot be fitted, datetime keys mix
///     awareness, or arithmetic is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import neutralize_and_zscore
/// >>> len(neutralize_and_zscore([1.0, 2.0, 4.0], ["d"] * 3, [[1.0, 1.0, 2.0]]))
/// 3
#[pyfunction]
#[pyo3(
    signature = (values, time_key, exposures),
    text_signature = "(values, time_key, exposures)"
)]
pub(super) fn neutralize_and_zscore(
    py: Python<'_>,
    values: &Bound<'_, PyAny>,
    time_key: &Bound<'_, PyAny>,
    exposures: &Bound<'_, PyAny>,
) -> PyResult<Vec<Option<f64>>> {
    let values = extract_opt_f64_vec(values, "values")?;
    let exposures = extract_exposures(exposures)?;
    let time_key = extract_keys(time_key, "time_key")?;
    py.detach(move || {
        finstack_quant_features::neutralize_and_zscore(&values, &time_key, &exposures)
    })
    .map_err(core_to_py)
}
