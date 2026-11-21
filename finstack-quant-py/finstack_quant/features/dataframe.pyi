"""
Type stubs for ``finstack_quant.features.dataframe``.

DataFrame convenience wrappers around the compiled feature transforms
(pandas and Polars). Each helper selects columns (or pandas index levels) from
a :class:`pandas.DataFrame` or :class:`polars.DataFrame`, forwards them to the
corresponding :mod:`finstack_quant.features` entry point, and returns a
``Series`` aligned to the input rows (or a ``DataFrame`` for :func:`panel`)
using the same DataFrame library (pandas in -> pandas out; Polars in ->
Polars out). Cross-sectional ``time_key`` and time-series ``order`` may be
omitted when a pandas input uses a ``DatetimeIndex`` or a Polars input has a
unique ``Date``/``Datetime`` column.

Examples
--------
>>> from finstack_quant.features import dataframe
>>> sorted(dataframe.__all__)[0]
'cross_sectional'
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

import pandas as pd
import polars as pl

from . import (
    CrossSectionalOp as _CrossSectionalOp,
    PairwiseOp as _PairwiseOp,
    TimeSeriesOp as _TimeSeriesOp,
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

def cross_sectional(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
    *,
    op: str | _CrossSectionalOp,
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Transform a value column across entities within each timestamp partition.

    Forwards to :func:`finstack_quant.features.transform_cross_sectional`,
    partitioning rows by ``time_key``.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Numeric column to transform; ``NaN``/``None``/``null`` entries are missing.
    time_key : str or int, optional
        Column, pandas index level name, or integer pandas index level position
        that partitions the cross-section. Omit for a pandas ``DatetimeIndex``
        or a unique Polars ``Date``/``Datetime`` column.
    op : str or CrossSectionalOp
        Cross-sectional operation name (required keyword-only).
    params : dict, optional
        Operation parameters forwarded to the compiled transform.

    Returns
    -------
    pandas.Series | polars.Series
        Transformed values matching the input host library (aligned to
        ``df.index`` for pandas, or a ``polars.Series`` named ``value``).

    Raises
    ------
    ValueError
        If a key is ambiguous, the operation name is unknown, parameters are
        invalid, keys mix aware and naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``op`` is omitted or is an enum value from another operation family.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import cross_sectional
    >>> frame = pd.DataFrame({"date": ["2026-01-01"] * 2, "signal": [1.0, 3.0]})
    >>> cross_sectional(frame, "signal", "date", op="rank").tolist()
    [0.0, 1.0]
    """
    ...

def timeseries(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    entity: KeySelector,
    order: KeySelector | None = None,
    *,
    op: str | _TimeSeriesOp,
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Transform a value column within each entity over time.

    Forwards to :func:`finstack_quant.features.transform_timeseries`,
    grouping rows by ``entity`` and sorting each group by ``order``.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Numeric column to transform; ``NaN``/``None``/``null`` entries are missing.
    entity : str or int
        Column, pandas index level name, or integer pandas index level position
        naming the entity key.
    order : str or int, optional
        Sort key within each entity. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    op : str or TimeSeriesOp
        Time-series operation name (required keyword-only).
    params : dict, optional
        Operation parameters forwarded to the compiled transform.

    Returns
    -------
    pandas.Series | polars.Series
        Transformed values matching the input host library (aligned to
        ``df.index`` for pandas, or a ``polars.Series`` named ``value``).

    Raises
    ------
    ValueError
        If a key is ambiguous, the operation name is unknown, parameters are
        invalid, keys mix aware and naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``op`` is omitted or is an enum value from another operation family.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import timeseries
    >>> frame = pd.DataFrame({"date": ["1", "2", "3"], "asset": ["A"] * 3, "signal": [1.0, 3.0, 6.0]})
    >>> timeseries(frame, "signal", "asset", "date", op="diff").iloc[1:].tolist()
    [2.0, 3.0]
    """
    ...

def panel(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    operations: Sequence[Mapping[str, Any]],
    *,
    entity: str | None = None,
    order: str | None = None,
    time_key: str | None = None,
) -> pd.DataFrame | pl.DataFrame:
    """
    Apply a JSON panel transform pipeline to a DataFrame value column.

    Forwards to :func:`finstack_quant.features.transform_panel`. Operations
    run sequentially; set ``input`` to ``"values"`` to branch from the raw
    column.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Numeric column shared by every operation.
    operations : Sequence[Mapping[str, Any]]
        Operation mappings with ``name``, ``family`` (``"timeseries"`` or
        ``"cross_sectional"``), ``op``, optional ``params``, and optional
        ``input`` (default: previous column, or the raw ``value`` column
        for the first op).
    entity : str, optional
        Entity key; Rust requires it when any operation is
        ``family="timeseries"``.
    order : str, optional
        Sort key; Rust requires it for time-series operations. Defaults to
        ``df.index`` when that is a pandas ``DatetimeIndex`` or a unique Polars
        ``Date``/``Datetime`` column.
    time_key : str, optional
        Partition key; Rust requires it for cross-sectional operations.
        Defaults to ``df.index`` when that is a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.

    Returns
    -------
    pandas.DataFrame | polars.DataFrame
        One column per operation name, matching the input host library.

    Raises
    ------
    KeyError
        If ``value`` or a named key is not a column or index level.
    ValueError
        If operation names are duplicated, a key the operation family
        requires is missing (raised by Rust ``transform_panel``), or a key
        selector is ambiguous.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import panel
    >>> frame = pd.DataFrame({"date": ["1", "1", "2", "2"], "signal": [1.0, 3.0, 2.0, 4.0]})
    >>> operations = [{"name": "rank", "family": "cross_sectional", "op": "rank"}]
    >>> panel(frame, "signal", operations, time_key="date")["rank"].tolist()
    [0.0, 1.0, 0.0, 1.0]
    """
    ...

def grouped(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
    *,
    groups: KeySelector,
    op: str | _CrossSectionalOp,
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Transform a value column within each timestamp/group sub-partition.

    Forwards to
    :func:`finstack_quant.features.transform_cross_sectional_grouped`,
    partitioning rows by the ``(time_key, groups)`` pair.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Numeric column to transform; ``NaN``/``None``/``null`` entries are missing.
    time_key : str or int, optional
        Timestamp partition key. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    groups : str or int
        Group key forming the sub-partition (required keyword-only).
    op : str or CrossSectionalOp
        Cross-sectional operation name (required keyword-only).
    params : dict, optional
        Operation parameters forwarded to the compiled transform.

    Returns
    -------
    pandas.Series | polars.Series
        Transformed values matching the input host library (aligned to
        ``df.index`` for pandas, or a ``polars.Series`` named ``value``).

    Raises
    ------
    ValueError
        If a key is ambiguous, the operation name is unknown, parameters are
        invalid, keys mix aware and naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``op`` or ``groups`` is omitted.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import grouped
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "group": ["x", "x", "y", "y"],
    ...     "signal": [1.0, 3.0, 10.0, 14.0],
    ... })
    >>> [round(value, 3) for value in grouped(frame, "signal", "date", groups="group", op="zscore").tolist()]
    [-1.0, 1.0, -1.0, 1.0]
    """
    ...

def neutralize(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
    *,
    exposures: Sequence[str],
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Return cross-sectional OLS residuals for a DataFrame signal column.

    Forwards to :func:`finstack_quant.features.neutralize`, regressing
    ``value`` on the exposure columns within each ``time_key`` partition.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Signal column to neutralize.
    time_key : str or int, optional
        Cross-section partition key. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    exposures : Sequence[str]
        Exposure columns regressed against ``value`` (required keyword-only).
    params : dict, optional
        Parameters forwarded to the compiled transform.

    Returns
    -------
    pandas.Series | polars.Series
        Residualized signal matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, parameters are invalid, keys mix aware and
        naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``exposures`` is omitted.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import neutralize
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "factor": [0.0, 1.0, 0.0, 1.0],
    ... })
    >>> [round(value, 3) for value in neutralize(frame, "signal", "date", exposures=["factor"]).tolist()]
    [-0.5, -1.0, 0.5, 1.0]
    """
    ...

def pairwise(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    other: str,
    entity: KeySelector,
    order: KeySelector | None = None,
    *,
    op: str | _PairwiseOp,
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Transform two value columns per entity with a rolling pairwise operation.

    Forwards to
    :func:`finstack_quant.features.transform_timeseries_pairwise`, grouping
    rows by ``entity`` and sorting each group by ``order``.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        First numeric column.
    other : str
        Second numeric column paired with ``value``.
    entity : str or int
        Entity key.
    order : str or int, optional
        Sort key within each entity. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    op : str or PairwiseOp
        Pairwise operation name (required keyword-only).
    params : dict, optional
        Operation parameters forwarded to the compiled transform.

    Returns
    -------
    pandas.Series | polars.Series
        Pairwise-transformed values matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, the operation name is unknown, parameters are
        invalid, keys mix aware and naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``op`` is omitted or is an enum value from another operation family.

    Examples
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
    ...     op="rolling_beta",
    ...     params={"window": 3, "min_periods": 3},
    ... )
    >>> round(float(beta.iloc[-1]), 3)
    0.643
    """
    ...

def rolling_regression_residual(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    exposures: Sequence[str],
    entity: KeySelector,
    order: KeySelector | None = None,
    params: TransformParams | None = None,
) -> pd.Series | pl.Series:
    """
    Return rolling per-entity OLS residuals for a DataFrame signal column.

    Forwards to
    :func:`finstack_quant.features.rolling_regression_residual`, grouping
    rows by ``entity`` and sorting each group by ``order``.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Signal column.
    exposures : Sequence[str]
        Exposure columns regressed against ``value``.
    entity : str or int
        Entity key.
    order : str or int, optional
        Sort key within each entity. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    params : dict, optional
        Row-window parameters: positive ``window`` and ``min_periods`` complete
        rows, with ``min_periods <= window``; ``fit_intercept`` defaults to true.

    Returns
    -------
    pandas.Series | polars.Series
        Rolling residuals matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, parameters are invalid, keys mix aware and
        naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.

    Examples
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
    ...

def risk_scaled_weights(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
    *,
    volatility: str,
) -> pd.Series | pl.Series:
    """
    Convert a DataFrame signal column to inverse-risk-scaled weights.

    Forwards to :func:`finstack_quant.features.risk_scaled_weights`. Each
    ``time_key`` partition is scaled as ``signal / volatility``, demeaned,
    then gross-normalized so the weights sum to zero.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Signal column.
    time_key : str or int, optional
        Cross-section partition key. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    volatility : str
        Nonnegative volatility column in common units and horizon (required).
        Zero, missing, and non-finite estimates produce missing weights;
        negative estimates raise ``ValueError``.

    Returns
    -------
    pandas.Series | polars.Series
        Gross-normalized weights matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, parameters are invalid, keys mix aware and
        naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``volatility`` is omitted.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import risk_scaled_weights
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "vol": [1.0, 2.0, 1.0, 2.0],
    ... })
    >>> risk_scaled_weights(frame, "signal", "date", volatility="vol").tolist()
    [-0.25, -0.25, 0.25, 0.25]
    """
    ...

def rank_to_weights(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
) -> pd.Series | pl.Series:
    """
    Convert DataFrame signal ranks to gross-normalized long/short weights.

    Forwards to :func:`finstack_quant.features.rank_to_weights`, ranking,
    demeaning, and gross-normalizing each ``time_key`` partition.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Signal column.
    time_key : str or int, optional
        Cross-section partition key. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.

    Returns
    -------
    pandas.Series | polars.Series
        Long/short weights matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, parameters are invalid, keys mix aware and
        naive datetimes, or numeric validation fails.
    KeyError
        If a required column is missing.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import rank_to_weights
    >>> frame = pd.DataFrame({"date": ["2026-01-01"] * 3, "signal": [1.0, 2.0, 100.0]})
    >>> rank_to_weights(frame, "signal", "date").tolist()
    [-0.5, 0.0, 0.5]
    """
    ...

def neutralize_and_zscore(
    df: pd.DataFrame | pl.DataFrame,
    value: str,
    time_key: KeySelector | None = None,
    *,
    exposures: Sequence[str],
) -> pd.Series | pl.Series:
    """
    Neutralize a DataFrame signal column against exposures, then z-score.

    Forwards to :func:`finstack_quant.features.neutralize_and_zscore`,
    residualizing ``value`` on the exposure columns with an intercept within
    each ``time_key`` partition and z-scoring the residuals.

    Parameters
    ----------
    df : pandas.DataFrame | polars.DataFrame
        Source DataFrame.
    value : str
        Signal column.
    time_key : str or int, optional
        Cross-section partition key. Omit for a pandas ``DatetimeIndex`` or a
        unique Polars ``Date``/``Datetime`` column.
    exposures : Sequence[str]
        Exposure columns regressed against ``value`` (required keyword-only).

    Returns
    -------
    pandas.Series | polars.Series
        Z-scored residual signal matching the input host library.

    Raises
    ------
    ValueError
        If a key is ambiguous, keys mix aware and naive datetimes, an
        exposure partition cannot be fitted, or numeric validation fails.
    KeyError
        If a required column is missing.
    TypeError
        If ``exposures`` is omitted.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.features.dataframe import neutralize_and_zscore
    >>> frame = pd.DataFrame({
    ...     "date": ["2026-01-01"] * 4,
    ...     "signal": [1.0, 2.0, 2.0, 4.0],
    ...     "factor": [0.0, 1.0, 0.0, 1.0],
    ... })
    >>> scores = neutralize_and_zscore(frame, "signal", "date", exposures=["factor"])
    >>> [round(value, 3) for value in scores.tolist()]
    [-0.632, -1.265, 0.632, 1.265]
    """
    ...
