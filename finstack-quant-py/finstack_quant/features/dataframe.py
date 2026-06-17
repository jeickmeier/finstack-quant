"""Pandas DataFrame convenience wrappers for feature transforms.

Each helper accepts a :class:`pandas.DataFrame` plus key selectors, forwards the
selected columns to the compiled feature transforms, and returns a
``pandas.Series`` aligned to the input index (or a ``pandas.DataFrame`` for
:func:`panel`). Numeric value columns may contain ``NaN``/``None`` for missing
data; key selectors may refer to columns, named index levels, or integer index
level positions, and key values are coerced to strings.

Cross-sectional ``time_key`` and time-series ``order`` may be omitted when the
DataFrame has a ``DatetimeIndex``. ``MultiIndex`` levels should be selected
explicitly by level name or integer position; ambiguous column/index key names
raise :class:`ValueError` instead of guessing.

``pandas`` is an optional dependency; importing or calling these helpers without
it installed raises :class:`ImportError`.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from importlib import import_module
import json
from typing import Any

from . import (
    clean_signal as _clean_signal,
    neutralize as _neutralize,
    neutralize_and_zscore as _neutralize_and_zscore,
    normalize_signal as _normalize_signal,
    rank_to_weights as _rank_to_weights,
    risk_scaled_weights as _risk_scaled_weights,
    rolling_regression_residual as _rolling_regression_residual,
    transform_cross_sectional as _transform_cross_sectional,
    transform_cross_sectional_grouped as _transform_cross_sectional_grouped,
    transform_panel as _transform_panel,
    transform_timeseries as _transform_timeseries,
    transform_timeseries_pairwise as _transform_timeseries_pairwise,
)

TransformParams = dict[str, Any]
KeySelector = str | int

__all__ = [
    "clean_signal",
    "cross_sectional",
    "grouped",
    "neutralize",
    "neutralize_and_zscore",
    "normalize_signal",
    "pairwise",
    "panel",
    "rank_to_weights",
    "risk_scaled_weights",
    "rolling_regression_residual",
    "timeseries",
]


def _require_pandas() -> Any:
    try:
        return import_module("pandas")
    except ModuleNotFoundError as exc:
        raise ImportError(
            "finstack_quant.features.dataframe requires pandas; install pandas to use DataFrame helpers"
        ) from exc


def _require_columns(df: Any, columns: Sequence[str]) -> None:
    missing = [column for column in columns if column not in df.columns]
    if missing:
        raise KeyError(f"DataFrame is missing required column(s): {missing}")


def _numeric_column(df: Any, column: str) -> list[float | None]:
    _require_columns(df, [column])
    pd = _require_pandas()
    return [None if pd.isna(value) else value for value in df[column].tolist()]


def _index_level_name_count(df: Any, key: str) -> int:
    return sum(name == key for name in df.index.names)


def _index_level_values(df: Any, key: KeySelector) -> Any:
    if isinstance(key, int):
        try:
            return df.index.get_level_values(key)
        except IndexError as exc:
            raise KeyError(f"DataFrame index has no level {key}") from exc

    count = _index_level_name_count(df, key)
    if count > 1:
        raise ValueError(f"ambiguous key {key!r}: multiple index levels share that name")
    if count == 1:
        return df.index.get_level_values(key)
    raise KeyError(f"DataFrame key {key!r} is not a column or index level")


def _default_datetime_index_values(df: Any, role: str) -> Any:
    pd = _require_pandas()
    if isinstance(df.index, pd.DatetimeIndex):
        return df.index
    raise KeyError(f"{role} is required when df.index is not a DatetimeIndex")


def _key_values(
    df: Any,
    key: KeySelector | None,
    *,
    role: str,
    default_datetime_index: bool = False,
) -> Any:
    if key is None:
        if default_datetime_index:
            return _default_datetime_index_values(df, role)
        raise TypeError(f"{role} is required")

    if isinstance(key, str) and key in df.columns:
        if _index_level_name_count(df, key) > 0:
            raise ValueError(f"ambiguous key {key!r}: found both a DataFrame column and an index level")
        return df[key]

    return _index_level_values(df, key)


def _key_column(
    df: Any,
    key: KeySelector | None,
    *,
    role: str,
    default_datetime_index: bool = False,
) -> list[str]:
    values = _key_values(
        df,
        key,
        role=role,
        default_datetime_index=default_datetime_index,
    )
    return values.astype(str).tolist()


def _require_operation(op: str | None) -> str:
    if op is None:
        raise TypeError("op is required")
    return op


def _require_sequence(value: Sequence[str] | None, role: str) -> Sequence[str]:
    if value is None:
        raise TypeError(f"{role} is required")
    return value


def _require_column_name(value: str | None, role: str) -> str:
    if value is None:
        raise TypeError(f"{role} is required")
    return value


def _operations_require(operations: Sequence[Mapping[str, Any]], family: str) -> bool:
    return any(operation.get("family") == family for operation in operations)


def _exposure_columns(df: Any, columns: Sequence[str]) -> list[list[float | None]]:
    _require_columns(df, columns)
    return [_numeric_column(df, column) for column in columns]


def _series(df: Any, values: list[float | None], name: str | None) -> Any:
    pd = _require_pandas()
    return pd.Series(values, index=df.index, name=name)


def cross_sectional(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    op: str | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Transform a value column across entities within each timestamp partition.

    Forwards to :func:`finstack_quant.features.transform_cross_sectional`,
    partitioning rows by the ``time_key`` column.

    Args:
        df: Source DataFrame.
        value: Name of the numeric column to transform. ``NaN``/``None`` entries
            are treated as missing.
        time_key: Name of the column whose values partition the cross-section;
            entries are coerced to strings.
        op: Cross-sectional operation name (e.g. ``"zscore"``, ``"rank"``,
            ``"winsorize"``). See ``transform_cross_sectional`` for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or ``time_key`` is not a column of ``df``.
        ValueError: If ``op`` is unsupported or ``params`` are malformed.
    """
    op = _require_operation(op)
    out = _transform_cross_sectional(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        op,
        params,
    )
    return _series(df, out, f"{value}_{op}")


def timeseries(
    df: Any,
    value: str,
    entity: KeySelector,
    order: KeySelector | None = None,
    op: str | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Transform a value column within each entity over time.

    Forwards to :func:`finstack_quant.features.transform_timeseries`, grouping
    rows by ``entity`` and sorting each group by ``order``.

    Args:
        df: Source DataFrame.
        value: Name of the numeric column to transform. ``NaN``/``None`` entries
            are treated as missing.
        entity: Name of the entity-key column; entries are coerced to strings.
        order: Name of the sort-key column within each entity; entries are
            coerced to strings.
        op: Time-series operation name (e.g. ``"returns"``, ``"rolling_mean"``,
            ``"ewma_mean"``). See ``transform_timeseries`` for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``entity``, or ``order`` is not a column of
            ``df``.
        ValueError: If ``op`` is unsupported or ``params`` are malformed.
    """
    op = _require_operation(op)
    out = _transform_timeseries(
        _numeric_column(df, value),
        _key_column(df, entity, role="entity"),
        _key_column(df, order, role="order", default_datetime_index=True),
        op,
        params,
    )
    return _series(df, out, f"{value}_{op}")


def panel(
    df: Any,
    value: str,
    operations: Sequence[Mapping[str, Any]],
    *,
    entity: str | None = None,
    order: str | None = None,
    time_key: str | None = None,
) -> Any:
    """Apply a JSON panel transform pipeline to a DataFrame value column.

    Forwards to :func:`finstack_quant.features.transform_panel`, running each
    named operation against the shared ``value`` column and assembling the
    results into a DataFrame.

    Args:
        df: Source DataFrame.
        value: Name of the numeric column shared by every operation.
            ``NaN``/``None`` entries are treated as missing.
        operations: Sequence of operation mappings, each with ``name``,
            ``family`` (``"timeseries"`` or ``"cross_sectional"``), ``op``, and
            optional ``params``. Names must be unique and non-empty.
        entity: Name of the entity-key column; required when any operation has
            ``family="timeseries"``. Entries are coerced to strings.
        order: Name of the sort-key column; required when any operation has
            ``family="timeseries"``. Entries are coerced to strings.
        time_key: Name of the partition-key column; required when any operation
            has ``family="cross_sectional"``. Entries are coerced to strings.

    Returns:
        pandas.DataFrame: One column per operation ``name``, in the order given,
        aligned to ``df.index``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or a referenced key column is not in ``df``.
        ValueError: If the pipeline spec is invalid (missing required keys,
            duplicate or empty names, or a failing operation).
    """
    pd = _require_pandas()
    spec: dict[str, Any] = {
        "values": _numeric_column(df, value),
        "operations": [dict(operation) for operation in operations],
    }
    if entity is not None:
        spec["entity"] = _key_column(df, entity, role="entity")
    elif _operations_require(operations, "timeseries"):
        raise TypeError("entity is required for timeseries panel operations")
    if order is not None or _operations_require(operations, "timeseries"):
        spec["order"] = _key_column(
            df,
            order,
            role="order",
            default_datetime_index=True,
        )
    if time_key is not None or _operations_require(operations, "cross_sectional"):
        spec["time_key"] = _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        )
    result = json.loads(_transform_panel(json.dumps(spec)))
    columns = result["columns"]
    ordered = {operation["name"]: columns[operation["name"]] for operation in operations}
    return pd.DataFrame(ordered, index=df.index)


def grouped(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    groups: KeySelector | None = None,
    op: str | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Transform a value column within each timestamp/group sub-partition.

    Forwards to
    :func:`finstack_quant.features.transform_cross_sectional_grouped`,
    partitioning rows by the ``(time_key, groups)`` pair.

    Args:
        df: Source DataFrame.
        value: Name of the numeric column to transform. ``NaN``/``None`` entries
            are treated as missing.
        time_key: Name of the primary partition-key column; entries are coerced
            to strings.
        groups: Name of the secondary partition-key column combined with
            ``time_key``; entries are coerced to strings.
        op: Cross-sectional operation name. See ``transform_cross_sectional``
            for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{groups}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``time_key``, or ``groups`` is not a column of
            ``df``.
        ValueError: If ``op`` is unsupported or ``params`` are malformed.
    """
    op = _require_operation(op)
    out = _transform_cross_sectional_grouped(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        _key_column(df, groups, role="groups"),
        op,
        params,
    )
    return _series(df, out, f"{value}_{groups}_{op}")


def neutralize(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    exposures: Sequence[str] | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Return cross-sectional OLS residuals for a DataFrame signal column.

    Forwards to :func:`finstack_quant.features.neutralize`, regressing ``value``
    on the exposure columns within each ``time_key`` partition.

    Args:
        df: Source DataFrame.
        value: Name of the signal column to neutralize. ``NaN``/``None`` entries
            are treated as missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        exposures: Names of the exposure columns regressed against ``value``.
        params: Optional parameters. ``fit_intercept`` (default ``True``) adds an
            intercept term.

    Returns:
        pandas.Series: Residuals aligned to ``df.index`` and named
        ``f"{value}_neutralized"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``time_key``, or any exposure column is missing.
        ValueError: If ``params`` are malformed.
    """
    out = _neutralize(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        _exposure_columns(df, _require_sequence(exposures, "exposures")),
        params,
    )
    return _series(df, out, f"{value}_neutralized")


def pairwise(
    df: Any,
    value: str,
    other: str,
    entity: KeySelector,
    order: KeySelector | None = None,
    op: str | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Transform two value columns per entity with a rolling pairwise operation.

    Forwards to
    :func:`finstack_quant.features.transform_timeseries_pairwise`, grouping rows
    by ``entity`` and sorting each group by ``order``.

    Args:
        df: Source DataFrame.
        value: Name of the first numeric column. ``NaN``/``None`` entries are
            treated as missing.
        other: Name of the second numeric column paired with ``value``.
        entity: Name of the entity-key column; entries are coerced to strings.
        order: Name of the sort-key column within each entity; entries are
            coerced to strings.
        op: Pairwise operation name: ``"rolling_cov"``, ``"rolling_corr"``, or
            ``"rolling_beta"``.
        params: Optional parameters ``window`` and ``min_periods``.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{other}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``other``, ``entity``, or ``order`` is missing.
        ValueError: If ``op`` is unsupported or ``params`` are malformed.
    """
    op = _require_operation(op)
    out = _transform_timeseries_pairwise(
        _numeric_column(df, value),
        _numeric_column(df, other),
        _key_column(df, entity, role="entity"),
        _key_column(df, order, role="order", default_datetime_index=True),
        op,
        params,
    )
    return _series(df, out, f"{value}_{other}_{op}")


def rolling_regression_residual(
    df: Any,
    value: str,
    exposures: Sequence[str],
    entity: KeySelector,
    order: KeySelector | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Return rolling per-entity OLS residuals for a DataFrame signal column.

    Forwards to
    :func:`finstack_quant.features.rolling_regression_residual`, grouping rows by
    ``entity`` and sorting each group by ``order``.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        exposures: Names of the exposure columns regressed against ``value``.
        entity: Name of the entity-key column; entries are coerced to strings.
        order: Name of the sort-key column within each entity; entries are
            coerced to strings.
        params: Optional parameters ``window``, ``min_periods``, and
            ``fit_intercept``.

    Returns:
        pandas.Series: Residuals aligned to ``df.index`` and named
        ``f"{value}_rolling_residual"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``entity``, ``order``, or any exposure column is
            missing.
        ValueError: If ``params`` are malformed.
    """
    out = _rolling_regression_residual(
        _numeric_column(df, value),
        _exposure_columns(df, exposures),
        _key_column(df, entity, role="entity"),
        _key_column(df, order, role="order", default_datetime_index=True),
        params,
    )
    return _series(df, out, f"{value}_rolling_residual")


def risk_scaled_weights(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    volatility: str | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Convert a DataFrame signal column to inverse-risk-scaled weights.

    Forwards to :func:`finstack_quant.features.risk_scaled_weights`, scaling each
    ``time_key`` partition by ``signal / volatility`` and normalizing gross
    weight to ``1``.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        volatility: Name of the risk-estimate column aligned to ``value``.
        params: Unused; accepted for signature symmetry with other helpers.

    Returns:
        pandas.Series: Weights aligned to ``df.index`` and named
        ``f"{value}_risk_scaled_weight"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``time_key``, or ``volatility`` is missing.
    """
    out = _risk_scaled_weights(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        _numeric_column(df, _require_column_name(volatility, "volatility")),
        params,
    )
    return _series(df, out, f"{value}_risk_scaled_weight")


def clean_signal(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Apply the default cross-sectional signal-cleaning pass.

    Forwards to :func:`finstack_quant.features.clean_signal`, clamping each
    ``time_key`` partition to its ``lower``/``upper`` sample quantiles.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        params: Optional quantile bounds ``lower`` (default ``0.01``) and
            ``upper`` (default ``0.99``).

    Returns:
        pandas.Series: Cleaned values aligned to ``df.index`` and named
        ``f"{value}_clean"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or ``time_key`` is missing.
        ValueError: If quantile bounds violate ``0 <= lower <= upper <= 1``.
    """
    out = _clean_signal(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        params,
    )
    return _series(df, out, f"{value}_clean")


def normalize_signal(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Normalize a DataFrame signal column cross-sectionally.

    Forwards to :func:`finstack_quant.features.normalize_signal`, applying the
    method named by ``params["method"]`` within each ``time_key`` partition.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        params: Optional parameters. ``method`` selects any single-column
            cross-sectional operation and defaults to ``"zscore"``; remaining
            params are forwarded to that operation.

    Returns:
        pandas.Series: Normalized values aligned to ``df.index`` and named
        ``f"{value}_normalized"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or ``time_key`` is missing.
        ValueError: If ``method`` is unsupported or ``params`` are malformed.
    """
    out = _normalize_signal(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        params,
    )
    return _series(df, out, f"{value}_normalized")


def rank_to_weights(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Convert DataFrame signal ranks to gross-normalized long/short weights.

    Forwards to :func:`finstack_quant.features.rank_to_weights`, ranking,
    demeaning, and gross-normalizing each ``time_key`` partition.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        params: Unused; accepted for signature symmetry with other helpers.

    Returns:
        pandas.Series: Weights aligned to ``df.index`` and named
        ``f"{value}_rank_weight"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or ``time_key`` is missing.
    """
    out = _rank_to_weights(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        params,
    )
    return _series(df, out, f"{value}_rank_weight")


def neutralize_and_zscore(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
    exposures: Sequence[str] | None = None,
    params: TransformParams | None = None,
) -> Any:
    """Neutralize a DataFrame signal column against exposures, then z-score.

    Forwards to :func:`finstack_quant.features.neutralize_and_zscore`,
    residualizing ``value`` on the exposure columns within each ``time_key``
    partition and z-scoring the residuals.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Name of the partition-key column; entries are coerced to
            strings.
        exposures: Names of the exposure columns regressed against ``value``.
        params: Optional parameters forwarded to ``neutralize``;
            ``fit_intercept`` (default ``True``).

    Returns:
        pandas.Series: Z-scored residuals aligned to ``df.index`` and named
        ``f"{value}_neutralized_zscore"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value``, ``time_key``, or any exposure column is missing.
        ValueError: If ``params`` are malformed.
    """
    out = _neutralize_and_zscore(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
        _exposure_columns(df, _require_sequence(exposures, "exposures")),
        params,
    )
    return _series(df, out, f"{value}_neutralized_zscore")
