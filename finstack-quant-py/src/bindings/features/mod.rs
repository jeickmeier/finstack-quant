//! Python bindings for vectorized panel feature transforms.
//!
//! Every transform takes row-aligned Python lists, releases the GIL for the
//! Rust kernel, and returns a list of ``float | None`` aligned to the input.
//! Key columns (``entity``, ``order``, ``time_key``, ``groups``) accept any
//! sequence of strings, ints, dates, timestamps or other objects: each entry
//! uses UTC and fixed nanosecond precision for aware datetimes; naive datetimes
//! retain wall time. Other date-like objects use ``isoformat()``, else ``str()``.
//! Mixed aware/naive datetime keys fail; opaque strings retain caller ordering.

use crate::bindings::module_utils::{py_to_json_value, register_submodule, ParentNameSource};
use crate::errors::core_to_py;
use finstack_quant_features::{
    CrossSectionalOp, PairwiseOp, PanelTransformResult, PanelTransformSpec, TimeSeriesOp,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyType};
use serde_json::Value;

/// Coerce one key-column sequence into strings.
///
/// Strings pass through. Aware datetimes normalize to UTC; datetimes and
/// pandas timestamps use fixed nanosecond precision. Dates use ``isoformat``;
/// other keys use ``str()``. Aware and naive datetimes must not be mixed.
fn extract_keys(obj: &Bound<'_, PyAny>, role: &str) -> PyResult<Vec<String>> {
    let mut keys = Vec::new();
    let datetime = obj.py().import("datetime")?;
    let datetime_type = datetime.getattr("datetime")?;
    let utc = datetime.getattr("timezone")?.getattr("utc")?;
    let mut aware_kind = None;
    for item in obj.try_iter().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{role} must be a sequence of str, int, or date-like values"
        ))
    })? {
        let item = item?;
        if let Ok(text) = item.extract::<String>() {
            keys.push(text);
        } else if item.is_instance(&datetime_type)? {
            let aware = !item.call_method0("utcoffset")?.is_none();
            if aware_kind.is_some_and(|previous| previous != aware) {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "{role} must not mix timezone-aware and naive datetimes"
                )));
            }
            aware_kind = Some(aware);
            let normalized = if aware {
                item.call_method1("astimezone", (&utc,))?
            } else {
                item.clone()
            };
            keys.push(datetime_key(&normalized, aware)?);
        } else if item.hasattr("isoformat")? {
            keys.push(item.call_method0("isoformat")?.extract()?);
        } else {
            keys.push(item.str()?.to_string());
        }
    }
    Ok(keys)
}

/// Fixed nanosecond precision preserves both datetime and pandas Timestamp order.
fn datetime_key(value: &Bound<'_, PyAny>, aware: bool) -> PyResult<String> {
    let year: i32 = value.getattr("year")?.extract()?;
    let month: u32 = value.getattr("month")?.extract()?;
    let day: u32 = value.getattr("day")?.extract()?;
    let hour: u32 = value.getattr("hour")?.extract()?;
    let minute: u32 = value.getattr("minute")?.extract()?;
    let second: u32 = value.getattr("second")?.extract()?;
    let microsecond: u32 = value.getattr("microsecond")?.extract()?;
    let nanosecond: u32 = if value.hasattr("nanosecond")? {
        value.getattr("nanosecond")?.extract()?
    } else {
        0
    };
    let fraction = microsecond * 1000 + nanosecond;
    let suffix = if aware { "+00:00" } else { "" };
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{fraction:09}{suffix}"
    ))
}

fn parse_params(
    py: Python<'_>,
    params: Option<&Bound<'_, PyAny>>,
    label: &str,
) -> PyResult<Option<Value>> {
    params
        .map(|value| py_to_json_value(py, value, label))
        .transpose()
}

/// Generate a string-accepting operation enum wrapper with ``__members__``
/// and ``values()``.
#[rustfmt::skip]
macro_rules! op_enum {
    ($py_type:ident, $rust_type:ident, $name:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(
                    name = $name,
                    module = "finstack_quant.features",
                    frozen,
                    eq,
                    skip_from_py_object
                )]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $py_type {
            /// Inner operation.
            pub(crate) inner: $rust_type,
        }

        #[pymethods]
        impl $py_type {
            /// Parse an operation from its snake_case name (e.g. ``"rolling_mean"``).
            ///
            /// Parameters
            /// ----------
            /// name : str
            ///     Canonical snake_case operation name; see ``values()``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If ``name`` is not an accepted operation; the message lists
            ///     every accepted name.
            #[new]
            #[pyo3(text_signature = "(name)")]
            fn new(name: &str) -> PyResult<Self> {
                name.parse::<$rust_type>()
                    .map(|inner| Self { inner })
                    .map_err(core_to_py)
            }

            /// Canonical snake_case operation name.
            #[getter]
            fn name(&self) -> String {
                self.inner.name()
            }

            /// JSON ``params`` keys this operation reads; any other key is rejected.
            #[getter]
            fn param_keys(&self) -> Vec<&'static str> {
                self.inner.param_keys().to_vec()
            }

            /// Every accepted operation name, in declaration order.
            #[staticmethod]
            #[pyo3(text_signature = "()")]
            fn values() -> Vec<String> {
                $rust_type::names()
            }

            /// Mapping ``{UPPER_SNAKE_NAME: op}`` of every operation (enum-style).
            #[classattr]
            #[allow(non_snake_case)]
            fn __members__(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
                let members = PyDict::new(py);
                for op in $rust_type::ALL {
                    members.set_item(op.name().to_ascii_uppercase(), Self { inner: *op })?;
                }
                Ok(members)
            }

            /// Look up an operation by ``UPPER_SNAKE`` member name (``Op["ROLLING_MEAN"]``).
            #[classmethod]
            fn __class_getitem__(cls: &Bound<'_, PyType>, key: &str) -> PyResult<Self> {
                let members = cls.getattr("__members__")?;
                Ok(*members.get_item(key)?.extract::<PyRef<'_, Self>>()?)
            }

            /// Snake_case name.
            fn __str__(&self) -> String {
                self.inner.name()
            }

            /// Python-style representation (``TimeSeriesOp.ROLLING_MEAN``).
            fn __repr__(&self) -> String {
                format!("{}.{}", $name, self.inner.name().to_ascii_uppercase())
            }
        }
    };
}

op_enum!(
    PyTimeSeriesOp,
    TimeSeriesOp,
    "TimeSeriesOp",
    "Time-series (per-entity, backward-looking) operation selector.\n\n\
     Accepts the snake_case name (``TimeSeriesOp(\"returns\")``) or an\n\
     ``UPPER_SNAKE`` member (``TimeSeriesOp[\"RETURNS\"]``). ``values()`` lists\n\
     every accepted name; ``param_keys`` lists the JSON ``params`` keys an\n\
     operation reads.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import TimeSeriesOp\n\
     >>> TimeSeriesOp(\"returns\").param_keys\n\
     ['periods']"
);
op_enum!(
    PyCrossSectionalOp,
    CrossSectionalOp,
    "CrossSectionalOp",
    "Cross-sectional (per-timestamp) operation selector.\n\n\
     Accepts the snake_case name (``CrossSectionalOp(\"zscore\")``) or an\n\
     ``UPPER_SNAKE`` member. ``values()`` lists every accepted name;\n\
     ``param_keys`` lists the JSON ``params`` keys an operation reads.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import CrossSectionalOp\n\
     >>> CrossSectionalOp(\"winsorize\").param_keys\n\
     ['lower', 'upper']"
);
op_enum!(
    PyPairwiseOp,
    PairwiseOp,
    "PairwiseOp",
    "Pairwise rolling operation selector (``rolling_cov``, ``rolling_corr``,\n\
     ``rolling_beta``). Accepts the snake_case name or an ``UPPER_SNAKE`` member.\n\n\
     Examples\n\
     --------\n\
     >>> from finstack_quant.features import PairwiseOp\n\
     >>> PairwiseOp.values()\n\
     ['rolling_cov', 'rolling_corr', 'rolling_beta']"
);

/// Extract an operation name from a ``str`` or op wrapper.
fn extract_op_name<T: pyo3::PyClass + Clone>(
    obj: &Bound<'_, PyAny>,
    name_of: impl Fn(&T) -> String,
) -> PyResult<String> {
    if let Ok(text) = obj.extract::<String>() {
        return Ok(text);
    }
    if let Ok(op) = obj.extract::<PyRef<'_, T>>() {
        return Ok(name_of(&op));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "op must be a str or an operation enum value",
    ))
}

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
///     Row-aligned entity keys (str, int or date-like; aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence
///     Row-aligned sort keys within each entity (ISO strings, dates, ints).
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
fn transform_timeseries(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let entity = extract_keys(entity, "entity")?;
    let order = extract_keys(order, "order")?;
    let op = extract_op_name::<PyTimeSeriesOp>(op, |o| o.inner.name())?;
    let params = parse_params(py, params, "time-series transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_timeseries(
            &values,
            &entity,
            &order,
            &op,
            params.as_ref(),
        )
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
///     Row-aligned partition keys (str, int or date-like; aware datetimes normalize to UTC; strings stay opaque).
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
fn transform_cross_sectional(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let time_key = extract_keys(time_key, "time_key")?;
    let op = extract_op_name::<PyCrossSectionalOp>(op, |o| o.inner.name())?;
    let params = parse_params(py, params, "cross-sectional transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_cross_sectional(&values, &time_key, &op, params.as_ref())
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
fn transform_cross_sectional_grouped(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
    groups: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let time_key = extract_keys(time_key, "time_key")?;
    let groups = extract_keys(groups, "groups")?;
    let op = extract_op_name::<PyCrossSectionalOp>(op, |o| o.inner.name())?;
    let params = parse_params(py, params, "grouped cross-sectional transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_cross_sectional_grouped(
            &values,
            &time_key,
            &groups,
            &op,
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
fn neutralize(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
    exposures: Vec<Vec<Option<f64>>>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
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
fn transform_timeseries_pairwise(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    other: Vec<Option<f64>>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    op: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let entity = extract_keys(entity, "entity")?;
    let order = extract_keys(order, "order")?;
    let op = extract_op_name::<PyPairwiseOp>(op, |o| o.inner.name())?;
    let params = parse_params(py, params, "pairwise time-series transform params")?;
    py.detach(move || {
        finstack_quant_features::transform_timeseries_pairwise(
            &values,
            &other,
            &entity,
            &order,
            &op,
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
fn rolling_regression_residual(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    exposures: Vec<Vec<Option<f64>>>,
    entity: &Bound<'_, PyAny>,
    order: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
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
fn risk_scaled_weights(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
    volatility: Vec<Option<f64>>,
) -> PyResult<Vec<Option<f64>>> {
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
fn rank_to_weights(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
) -> PyResult<Vec<Option<f64>>> {
    let time_key = extract_keys(time_key, "time_key")?;
    py.detach(move || finstack_quant_features::rank_to_weights(&values, &time_key))
        .map_err(core_to_py)
}

/// Neutralize a signal with an intercept and z-score residuals.
///
/// ``fit_intercept=False`` raises ``ValueError`` because subsequent demeaning
/// would reintroduce factor exposure. Exact-fit roundoff residuals become zero.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Row-aligned signal values.
/// time_key : sequence
///     Row-aligned partition keys (aware datetimes normalize to UTC; strings stay opaque).
/// exposures : list[list[float | None]]
///     One row-aligned exposure column each.
/// params : dict, optional
///     ``fit_intercept`` must be True (default); False or unknown keys raise
///     ``ValueError`` to preserve neutrality after standardization.
///
/// Returns
/// -------
/// list[float | None]
///     Z-scored residuals per row; exact fits produce zero signals.
///
/// Raises
/// ------
/// ValueError
///     If lengths differ, a partition cannot be fitted, ``fit_intercept`` is
///     False, datetime keys mix awareness, or arithmetic is non-finite.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import neutralize_and_zscore
/// >>> len(neutralize_and_zscore([1.0, 2.0, 4.0], ["d"] * 3, [[1.0, 1.0, 2.0]]))
/// 3
#[pyfunction]
#[pyo3(
    signature = (values, time_key, exposures, params=None),
    text_signature = "(values, time_key, exposures, params=None)"
)]
fn neutralize_and_zscore(
    py: Python<'_>,
    values: Vec<Option<f64>>,
    time_key: &Bound<'_, PyAny>,
    exposures: Vec<Vec<Option<f64>>>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Option<f64>>> {
    let time_key = extract_keys(time_key, "time_key")?;
    let params = parse_params(py, params, "neutralize and zscore params")?;
    py.detach(move || {
        finstack_quant_features::neutralize_and_zscore(
            &values,
            &time_key,
            &exposures,
            params.as_ref(),
        )
    })
    .map_err(core_to_py)
}

/// Specification for a sequential panel transform pipeline.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Input value column; ``None``, NaN and infinity are missing.
/// operations : list[dict]
///     Ordered operations, each ``{"name", "family" ("timeseries" |
///     "cross_sectional"), "op", "params"?, "input"?}``. ``input`` selects
///     ``"values"`` or an earlier operation name (default: previous column).
/// entity : sequence, optional
///     Row-aligned entity keys (required for time-series ops; aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence, optional
///     Row-aligned sort keys (required for time-series ops).
/// time_key : sequence, optional
///     Row-aligned partition keys (required for cross-sectional ops).
///
/// Raises
/// ------
/// ValueError
///     If an operation mapping is malformed (unknown family, op, or key).
///
/// Examples
/// --------
/// >>> from finstack_quant.features import PanelTransformSpec
/// >>> spec = PanelTransformSpec([1.0, 3.0], [{"name": "r", "family": "cross_sectional", "op": "rank"}], time_key=["d", "d"])
/// >>> spec.operation_names
/// ['r']
#[pyclass(
    name = "PanelTransformSpec",
    module = "finstack_quant.features",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPanelTransformSpec {
    /// Inner typed spec.
    pub(crate) inner: PanelTransformSpec,
}

#[pymethods]
impl PyPanelTransformSpec {
    /// Construct a panel spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (values, operations, entity=None, order=None, time_key=None),
        text_signature = "(values, operations, entity=None, order=None, time_key=None)"
    )]
    fn new(
        py: Python<'_>,
        values: Vec<Option<f64>>,
        operations: &Bound<'_, PyAny>,
        entity: Option<&Bound<'_, PyAny>>,
        order: Option<&Bound<'_, PyAny>>,
        time_key: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let ops_json = py_to_json_value(py, operations, "panel transform operations")?;
        let operations = serde_json::from_value(ops_json).map_err(|e| {
            crate::errors::serde_json_to_py(e, "invalid panel transform operations")
        })?;
        Ok(Self {
            inner: PanelTransformSpec {
                values: values
                    .into_iter()
                    .map(|value| value.filter(|v| v.is_finite()))
                    .collect(),
                entity: entity.map(|e| extract_keys(e, "entity")).transpose()?,
                order: order.map(|o| extract_keys(o, "order")).transpose()?,
                time_key: time_key.map(|t| extract_keys(t, "time_key")).transpose()?,
                operations,
            },
        })
    }

    /// Output column names in operation order.
    #[getter]
    fn operation_names(&self) -> Vec<String> {
        self.inner
            .operations
            .iter()
            .map(|op| op.name().to_string())
            .collect()
    }

    /// Input value column.
    #[getter]
    fn values(&self) -> Vec<Option<f64>> {
        self.inner.values.clone()
    }

    /// Serialize to the JSON accepted by ``transform_panel_json``.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            crate::errors::serde_json_to_py(e, "failed to serialize PanelTransformSpec")
        })
    }

    /// Deserialize from JSON (strict field names).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<PanelTransformSpec>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PanelTransformSpec JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "PanelTransformSpec(rows={}, operations={:?})",
            self.inner.values.len(),
            self.operation_names()
        )
    }
}

/// Ordered output columns of a panel transform pipeline.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_panel
/// >>> res = transform_panel({"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]})
/// >>> res.columns
/// ['r']
/// >>> res.get_column("r")
/// [0.0, 1.0]
#[pyclass(
    name = "PanelTransformResult",
    module = "finstack_quant.features",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPanelTransformResult {
    /// Inner typed result.
    pub(crate) inner: PanelTransformResult,
}

#[pymethods]
impl PyPanelTransformResult {
    /// Output column names in operation order.
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.inner.columns.iter().map(|c| c.name.clone()).collect()
    }

    /// Values of one output column, aligned to the input rows.
    ///
    /// Parameters
    /// ----------
    /// name : str
    ///     Operation output name (case-sensitive).
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no column has that name.
    #[pyo3(text_signature = "(self, name)")]
    fn get_column(&self, name: &str) -> PyResult<Vec<Option<f64>>> {
        self.inner
            .get_column(name)
            .map(<[Option<f64>]>::to_vec)
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "no panel transform column '{name}'; available: {:?}",
                    self.columns()
                ))
            })
    }

    /// Serialize to the JSON produced by ``transform_panel_json``.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            crate::errors::serde_json_to_py(e, "failed to serialize PanelTransformResult")
        })
    }

    /// Deserialize from JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<PanelTransformResult>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PanelTransformResult JSON"))
    }

    /// Support ``pickle`` through the JSON wire form.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Columns as a ``pandas.DataFrame`` (one float column per operation).
    ///
    /// Parameters
    /// ----------
    /// index : pandas.Index or sequence, optional
    ///     Row index to attach (e.g. the source frame's index); default
    ///     ``RangeIndex``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     ``None`` outputs become ``NaN``.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(signature = (index=None), text_signature = "(self, index=None)")]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        index: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let columns = PyDict::new(py);
        for column in &self.inner.columns {
            columns.set_item(&column.name, column.values.clone())?;
        }
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, index)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "PanelTransformResult(columns={:?}, rows={})",
            self.columns(),
            self.inner.columns.first().map_or(0, |c| c.values.len())
        )
    }
}

/// Apply a named panel transform pipeline (typed twin of ``transform_panel_json``).
///
/// Operations run sequentially; each reads the previous column unless
/// ``input`` selects ``"values"`` or an earlier operation name.
///
/// Parameters
/// ----------
/// spec : PanelTransformSpec, dict or str
///     Typed spec, an equivalent ``dict`` (``values``, ``operations``,
///     optional ``entity`` / ``order`` / ``time_key``), or its JSON.
///
/// Returns
/// -------
/// PanelTransformResult
///     Ordered output columns with ``get_column`` and ``to_dataframe``.
///
/// Raises
/// ------
/// ValueError
///     If the spec is malformed, an operation name is duplicated or
///     reserved, ``input`` is unknown, or an operation fails.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_panel
/// >>> spec = {"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]}
/// >>> transform_panel(spec).get_column("r")
/// [0.0, 1.0]
#[pyfunction]
#[pyo3(text_signature = "(spec)")]
fn transform_panel(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyPanelTransformResult> {
    let spec: PanelTransformSpec = if let Ok(typed) =
        spec.extract::<PyRef<'_, PyPanelTransformSpec>>()
    {
        typed.inner.clone()
    } else if let Ok(json) = spec.extract::<String>() {
        serde_json::from_str(&json).map_err(|error| {
            crate::errors::serde_json_to_py(error, "invalid panel transform spec")
        })?
    } else {
        let mapping = spec.cast::<PyDict>()?;
        for key in mapping.keys() {
            let key: String = key.extract()?;
            if !["values", "operations", "entity", "order", "time_key"].contains(&key.as_str()) {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown panel transform field '{key}'"
                )));
            }
        }
        let values = mapping.get_item("values")?.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("panel transform values is required")
        })?;
        let operations = mapping.get_item("operations")?.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("panel transform operations is required")
        })?;
        let entity = mapping.get_item("entity")?.filter(|value| !value.is_none());
        let order = mapping.get_item("order")?.filter(|value| !value.is_none());
        let time_key = mapping
            .get_item("time_key")?
            .filter(|value| !value.is_none());
        PyPanelTransformSpec::new(
            py,
            values.extract()?,
            &operations,
            entity.as_ref(),
            order.as_ref(),
            time_key.as_ref(),
        )?
        .inner
    };
    py.detach(move || finstack_quant_features::transform_panel(&spec))
        .map(|inner| PyPanelTransformResult { inner })
        .map_err(core_to_py)
}

/// Apply a JSON panel transform pipeline (JSON twin of ``transform_panel``).
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-encoded ``PanelTransformSpec``.
///
/// Returns
/// -------
/// str
///     JSON ``{"columns": [{"name", "values"}, ...]}`` in operation order.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed or an operation fails.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.features import transform_panel_json
/// >>> spec = {"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]}
/// >>> json.loads(transform_panel_json(json.dumps(spec)))["columns"][0]["name"]
/// 'r'
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn transform_panel_json(py: Python<'_>, spec_json: String) -> PyResult<String> {
    py.detach(move || finstack_quant_features::transform_panel_json(&spec_json))
        .map_err(core_to_py)
}

/// Register the features submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "features")?;
    m.setattr("__doc__", "Vectorized panel feature transforms.")?;
    m.add_class::<PyCrossSectionalOp>()?;
    m.add_class::<PyPairwiseOp>()?;
    m.add_class::<PyPanelTransformResult>()?;
    m.add_class::<PyPanelTransformSpec>()?;
    m.add_class::<PyTimeSeriesOp>()?;
    m.add_function(wrap_pyfunction!(transform_timeseries, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional_grouped, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_timeseries_pairwise, &m)?)?;
    m.add_function(wrap_pyfunction!(rolling_regression_residual, &m)?)?;
    m.add_function(wrap_pyfunction!(risk_scaled_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(rank_to_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize_and_zscore, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_panel, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_panel_json, &m)?)?;
    let all = PyList::new(
        py,
        [
            "CrossSectionalOp",
            "PairwiseOp",
            "PanelTransformResult",
            "PanelTransformSpec",
            "TimeSeriesOp",
            "neutralize",
            "neutralize_and_zscore",
            "rank_to_weights",
            "risk_scaled_weights",
            "rolling_regression_residual",
            "transform_cross_sectional",
            "transform_cross_sectional_grouped",
            "transform_panel",
            "transform_panel_json",
            "transform_timeseries",
            "transform_timeseries_pairwise",
        ],
    )?;
    m.setattr("__all__", all)?;
    register_submodule(
        py,
        parent,
        &m,
        "features",
        crate::bindings::module_utils::ROOT_PACKAGE,
        ParentNameSource::Name,
    )
}
