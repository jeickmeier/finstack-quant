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

Examples:
--------
>>> import pandas as pd
>>> from finstack_quant.features.dataframe import cross_sectional
>>> frame = pd.DataFrame({"date": ["2026-01-01"] * 2, "signal": [1.0, 3.0]})
>>> cross_sectional(frame, "signal", "date", "rank").tolist()
[0.0, 1.0]
"""

from collections.abc import Mapping, Sequence
from importlib import import_module as _import_module
from typing import Any

from . import (
    neutralize as _neutralize,
    neutralize_and_zscore as _neutralize_and_zscore,
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
    "cross_sectional",
    "grouped",
    "neutralize",
    "neutralize_and_zscore",
    "pairwise",
    "panel",
    "rank_to_weights",
    "risk_scaled_weights",
    "rolling_regression_residual",
    "timeseries",
]


def _require_pandas() -> Any:
    try:
        return _import_module("pandas")
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
) -> list[Any]:
    values = _key_values(
        df,
        key,
        role=role,
        default_datetime_index=default_datetime_index,
    )
    # The binding owns key coercion, including UTC normalization and timestamp
    # precision. Stringifying here would lose those semantics before dispatch.
    return values.tolist()


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
        time_key: Column name, index level name, or integer index level
            position that partitions the cross-section. Aware datetimes normalize to UTC; strings remain opaque. Omit when ``df.index`` is a ``DatetimeIndex``.
        op: Cross-sectional operation name (e.g. ``"zscore"``, ``"rank"``,
            ``"winsorize"``). See ``transform_cross_sectional`` for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` is missing, or ``time_key`` is not a column or
            index level (and no ``DatetimeIndex`` default applies).
        ValueError: If ``time_key`` is ambiguous (both a column and an index
            level), ``op`` is unsupported, or ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import cross_sectional
    >>> frame = pd.DataFrame({"date": ["2026-01-01"] * 2, "signal": [1.0, 3.0]})
    >>> cross_sectional(frame, "signal", "date", "rank").tolist()
    [0.0, 1.0]
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
        entity: Column name, index level name, or integer index level position
            identifying the entity. Aware datetimes normalize to UTC.
        order: Column name, index level name, or integer index level position
            used to sort within each entity. Aware datetimes normalize to UTC.
            Omit when ``df.index`` is a ``DatetimeIndex``.
        op: Time-series operation name (e.g. ``"returns"``, ``"rolling_mean"``,
            ``"ewma_mean"``). See ``transform_timeseries`` for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` is missing, or ``entity``/``order`` is not a
            column or index level (and no ``DatetimeIndex`` default applies for
            ``order``).
        ValueError: If a key selector is ambiguous (both a column and an index
            level), ``op`` is unsupported, or ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import timeseries
    >>> frame = pd.DataFrame({"date": ["1", "2", "3"], "asset": ["A"] * 3, "signal": [1.0, 3.0, 6.0]})
    >>> timeseries(frame, "signal", "asset", "date", "diff").iloc[1:].tolist()
    [2.0, 3.0]
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

    Forwards to :func:`finstack_quant.features.transform_panel`. Operations run
    sequentially: each reads the previous column by default. Set ``input`` to
    ``"values"`` to branch from the raw column, or to an earlier operation
    name.

    Args:
        df: Source DataFrame.
        value: Name of the numeric column shared by every operation.
            NaN, infinity and ``None`` entries are treated as missing.
        operations: Sequence of operation mappings, each with ``name``,
            ``family`` (``"timeseries"`` or ``"cross_sectional"``), ``op``,
            optional ``params``, and optional ``input`` (default: previous
            column, or the raw ``value`` column for the first op). Names must
            be unique, non-empty, and must not be the reserved name
            ``values``.
        entity: Column or index level name for the entity key; required when any
            operation has ``family="timeseries"``. Aware datetimes normalize to UTC; strings remain opaque.
        order: Column or index level name for the sort key; required when any
            operation has ``family="timeseries"`` unless ``df.index`` is a
            ``DatetimeIndex``. Aware datetimes normalize to UTC; strings remain opaque.
        time_key: Column or index level name for the partition key; required when
            any operation has ``family="cross_sectional"`` unless ``df.index`` is
            a ``DatetimeIndex``. Aware datetimes normalize to UTC; strings remain opaque.

    Returns:
        pandas.DataFrame: One column per operation ``name``, in the order given,
        aligned to ``df.index``.

    Raises:
        ImportError: If pandas is not installed.
        TypeError: If ``entity`` is omitted for a time-series operation.
        KeyError: If ``value`` or a referenced key is not a column or index
            level (and no ``DatetimeIndex`` default applies).
        ValueError: If a key selector is ambiguous, or the pipeline spec is
            invalid (duplicate or empty names, or a failing operation).

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import panel
    >>> frame = pd.DataFrame({"date": ["1", "1", "2", "2"], "signal": [1.0, 3.0, 2.0, 4.0]})
    >>> operations = [{"name": "rank", "family": "cross_sectional", "op": "rank"}]
    >>> panel(frame, "signal", operations, time_key="date")["rank"].tolist()
    [0.0, 1.0, 0.0, 1.0]
    """
    _require_pandas()
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
    result = _transform_panel(spec)
    frame = result.to_dataframe(index=df.index)
    return frame[[operation["name"] for operation in operations]]


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
        time_key: Column name, index level name, or integer index level
            position for the primary partition. Aware datetimes normalize to UTC; strings remain opaque.
            Omit when ``df.index`` is a ``DatetimeIndex``.
        groups: Column name, index level name, or integer index level position
            for the secondary partition combined with ``time_key``. Entries are
            coerced to strings.
        op: Cross-sectional operation name. See ``transform_cross_sectional``
            for the full set.
        params: Optional operation parameters.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{groups}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        TypeError: If ``groups`` is omitted.
        KeyError: If ``value`` is missing, or a key selector is not a column or
            index level (and no ``DatetimeIndex`` default applies for
            ``time_key``).
        ValueError: If a key selector is ambiguous, ``op`` is unsupported, or
            ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import grouped
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "group": ["x", "x", "y", "y"],
    ...     "signal": [1.0, 3.0, 10.0, 14.0],
    ... })
    >>> [round(value, 3) for value in grouped(frame, "signal", "date", "group", "zscore").tolist()]
    [-1.0, 1.0, -1.0, 1.0]
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
        time_key: Column name, index level name, or integer index level
            position that partitions the cross-section. Aware datetimes normalize to UTC; strings remain opaque. Omit when ``df.index`` is a ``DatetimeIndex``.
        exposures: Names of the exposure columns regressed against ``value``.
        params: Optional parameters. ``fit_intercept`` (default ``True``) adds an
            intercept term.

    Returns:
        pandas.Series: Residuals aligned to ``df.index`` and named
        ``f"{value}_neutralized"``.

    Raises:
        ImportError: If pandas is not installed.
        TypeError: If ``exposures`` is omitted.
        KeyError: If ``value`` or any exposure column is missing, or ``time_key``
            is not a column or index level (and no ``DatetimeIndex`` default
            applies).
        ValueError: If ``time_key`` is ambiguous or ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import neutralize
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "factor": [0.0, 1.0, 0.0, 1.0],
    ... })
    >>> [round(value, 3) for value in neutralize(frame, "signal", "date", ["factor"]).tolist()]
    [-0.5, -1.0, 0.5, 1.0]
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
        entity: Column name, index level name, or integer index level position
            identifying the entity. Aware datetimes normalize to UTC.
        order: Column name, index level name, or integer index level position
            used to sort within each entity. Aware datetimes normalize to UTC.
            Omit when ``df.index`` is a ``DatetimeIndex``.
        op: Pairwise operation name: ``"rolling_cov"``, ``"rolling_corr"``, or
            ``"rolling_beta"``.
        params: ``window`` spans rows, including gaps; ``min_periods <= window``
            counts complete pairs within that window.

    Returns:
        pandas.Series: Result aligned to ``df.index`` and named
        ``f"{value}_{other}_{op}"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or ``other`` is missing, or ``entity``/``order``
            is not a column or index level (and no ``DatetimeIndex`` default
            applies for ``order``).
        ValueError: If a key selector is ambiguous, ``op`` is unsupported, or
            ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import pairwise
    >>> frame = pd.DataFrame({
    ...     "date": ["1", "2", "3"],
    ...     "asset": ["A"] * 3,
    ...     "signal": [1.0, 2.0, 3.0],
    ...     "other": [1.0, 2.0, 4.0],
    ... })
    >>> beta = pairwise(
    ...     frame,
    ...     "signal",
    ...     "other",
    ...     "asset",
    ...     "date",
    ...     "rolling_beta",
    ...     {"window": 3, "min_periods": 3},
    ... )
    >>> round(float(beta.iloc[-1]), 3)
    0.643
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
        entity: Column name, index level name, or integer index level position
            identifying the entity. Aware datetimes normalize to UTC.
        order: Column name, index level name, or integer index level position
            used to sort within each entity. Aware datetimes normalize to UTC.
            Omit when ``df.index`` is a ``DatetimeIndex``.
        params: Optional parameters ``window``, ``min_periods``, and
            ``fit_intercept``.

    Returns:
        pandas.Series: Residuals aligned to ``df.index`` and named
        ``f"{value}_rolling_residual"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` or any exposure column is missing, or
            ``entity``/``order`` is not a column or index level (and no
            ``DatetimeIndex`` default applies for ``order``).
        ValueError: If a key selector is ambiguous or ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import rolling_regression_residual
    >>> frame = pd.DataFrame({
    ...     "date": ["1", "2", "3"],
    ...     "asset": ["A"] * 3,
    ...     "signal": [1.0, 2.0, 5.0],
    ...     "factor": [0.0, 1.0, 2.0],
    ... })
    >>> residual = rolling_regression_residual(
    ...     frame, "signal", ["factor"], "asset", "date", {"window": 3, "min_periods": 3}
    ... )
    >>> round(float(residual.iloc[-1]), 3)
    0.333
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
) -> Any:
    """Convert a DataFrame signal column to dollar-neutral inverse-vol weights.

    Forwards to :func:`finstack_quant.features.risk_scaled_weights`. Each
    ``time_key`` partition is scaled as ``signal / volatility``, demeaned,
    then gross-normalized so the weights sum to zero.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Column name, index level name, or integer index level
            position that partitions the cross-section. Aware datetimes normalize to UTC; strings remain opaque. Omit when ``df.index`` is a ``DatetimeIndex``.
        volatility: Name of the risk-estimate column aligned to ``value``.
            Values are used as ``signal / volatility``; non-positive magnitudes
            map to missing weights.

    Returns:
        pandas.Series: Weights aligned to ``df.index`` and named
        ``f"{value}_risk_scaled_weight"``.

    Raises:
        ImportError: If pandas is not installed.
        TypeError: If ``volatility`` is omitted.
        KeyError: If ``value`` or ``volatility`` is missing, or ``time_key`` is
            not a column or index level (and no ``DatetimeIndex`` default
            applies).
        ValueError: If volatility is negative, arithmetic is non-finite, or
            ``time_key`` is ambiguous.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import risk_scaled_weights
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "vol": [1.0, 2.0, 1.0, 2.0],
    ... })
    >>> risk_scaled_weights(frame, "signal", "date", "vol").tolist()
    [-0.25, -0.25, 0.25, 0.25]
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
    )
    return _series(df, out, f"{value}_risk_scaled_weight")


def rank_to_weights(
    df: Any,
    value: str,
    time_key: KeySelector | None = None,
) -> Any:
    """Convert DataFrame signal ranks to gross-normalized long/short weights.

    Forwards to :func:`finstack_quant.features.rank_to_weights`, ranking,
    demeaning, and gross-normalizing each ``time_key`` partition.

    Args:
        df: Source DataFrame.
        value: Name of the signal column. ``NaN``/``None`` entries are treated as
            missing.
        time_key: Column name, index level name, or integer index level
            position that partitions the cross-section. Aware datetimes normalize to UTC; strings remain opaque. Omit when ``df.index`` is a ``DatetimeIndex``.

    Returns:
        pandas.Series: Weights aligned to ``df.index`` and named
        ``f"{value}_rank_weight"``.

    Raises:
        ImportError: If pandas is not installed.
        KeyError: If ``value`` is missing, or ``time_key`` is not a column or
            index level (and no ``DatetimeIndex`` default applies).
        ValueError: If ``time_key`` is ambiguous.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import rank_to_weights
    >>> frame = pd.DataFrame({"date": ["2026-01-01"] * 3, "signal": [1.0, 2.0, 100.0]})
    >>> rank_to_weights(frame, "signal", "date").tolist()
    [-0.5, 0.0, 0.5]
    """
    out = _rank_to_weights(
        _numeric_column(df, value),
        _key_column(
            df,
            time_key,
            role="time_key",
            default_datetime_index=True,
        ),
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
        time_key: Column name, index level name, or integer index level
            position that partitions the cross-section. Aware datetimes normalize to UTC; strings remain opaque. Omit when ``df.index`` is a ``DatetimeIndex``.
        exposures: Names of the exposure columns regressed against ``value``.
        params: Optional parameters forwarded to ``neutralize``;
            ``fit_intercept`` (default ``True``).

    Returns:
        pandas.Series: Z-scored residuals aligned to ``df.index`` and named
        ``f"{value}_neutralized_zscore"``.

    Raises:
        ImportError: If pandas is not installed.
        TypeError: If ``exposures`` is omitted.
        KeyError: If ``value`` or any exposure column is missing, or ``time_key``
            is not a column or index level (and no ``DatetimeIndex`` default
            applies).
        ValueError: If ``fit_intercept=False``, arithmetic is non-finite,
            ``time_key`` is ambiguous, or ``params`` are malformed.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import neutralize_and_zscore
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "factor": [0.0, 1.0, 0.0, 1.0],
    ... })
    >>> scores = neutralize_and_zscore(frame, "signal", "date", ["factor"])
    >>> [round(value, 3) for value in scores.tolist()]
    [-0.632, -1.265, 0.632, 1.265]
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
