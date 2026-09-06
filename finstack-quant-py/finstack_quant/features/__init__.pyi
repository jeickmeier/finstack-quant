"""
Feature engineering: panel-data transformations for signal research.

Bindings for the ``finstack-quant-features`` crate
(``finstack_quant.features``). Provides time-series, cross-sectional, and
pairwise transforms (z-score, rank, rolling mean, neutralization, risk-scaled
weights) plus a general panel dispatcher :func:`transform_panel` (typed) /
:func:`transform_panel_json` (JSON), and the operation selectors
:class:`TimeSeriesOp`, :class:`CrossSectionalOp`, :class:`PairwiseOp`.

Examples
--------
>>> from finstack_quant.features import transform_cross_sectional
>>> transform_cross_sectional([1.0, 3.0], ["2026-01-01"] * 2, "rank")
[0.0, 1.0]
"""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from types import ModuleType
from typing import Any

import pandas as pd

TransformParams = dict[str, Any]
KeyColumn = Sequence[str | int | datetime.date | Any]

__all__ = [
    "CrossSectionalOp",
    "PairwiseOp",
    "PanelTransformResult",
    "PanelTransformSpec",
    "TimeSeriesOp",
    "dataframe",
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
]

dataframe: ModuleType

class TimeSeriesOp:
    """
    Time-series (per-entity, backward-looking) operation selector.

    Accepts the snake_case name (``TimeSeriesOp("returns")``) or an
    ``UPPER_SNAKE`` member (``TimeSeriesOp["RETURNS"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import TimeSeriesOp
    >>> TimeSeriesOp("returns").param_keys
    ['periods']
    """

    __members__: dict[str, TimeSeriesOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import TimeSeriesOp
        >>> TimeSeriesOp("returns").name
        'returns'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"returns"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import TimeSeriesOp
        >>> "returns" in TimeSeriesOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> TimeSeriesOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"RETURNS"``.

        Returns
        -------
        TimeSeriesOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class CrossSectionalOp:
    """
    Cross-sectional (per-timestamp) operation selector.

    Accepts the snake_case name (``CrossSectionalOp("winsorize")``) or an
    ``UPPER_SNAKE`` member (``CrossSectionalOp["WINSORIZE"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import CrossSectionalOp
    >>> CrossSectionalOp("winsorize").param_keys
    ['lower', 'upper']
    """

    __members__: dict[str, CrossSectionalOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import CrossSectionalOp
        >>> CrossSectionalOp("winsorize").name
        'winsorize'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"winsorize"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import CrossSectionalOp
        >>> "winsorize" in CrossSectionalOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> CrossSectionalOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"WINSORIZE"``.

        Returns
        -------
        CrossSectionalOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class PairwiseOp:
    """
    Pairwise rolling operation selector (cov / corr / beta).

    Accepts the snake_case name (``PairwiseOp("rolling_beta")``) or an
    ``UPPER_SNAKE`` member (``PairwiseOp["ROLLING_BETA"]``). ``values()``
    lists every accepted name; ``param_keys`` lists the JSON ``params`` keys the
    operation reads (any other key is rejected).

    Examples
    --------
    >>> from finstack_quant.features import PairwiseOp
    >>> PairwiseOp("rolling_beta").param_keys
    ['window', 'min_periods']
    """

    __members__: dict[str, PairwiseOp]

    def __init__(self, name: str) -> None:
        """
        Parse an operation from its snake_case name.

        Parameters
        ----------
        name : str
            Canonical operation name; see :meth:`values`.

        Raises
        ------
        ValueError
            If ``name`` is not accepted; the message lists every accepted name.

        Examples
        --------
        >>> from finstack_quant.features import PairwiseOp
        >>> PairwiseOp("rolling_beta").name
        'rolling_beta'
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case operation name.

        Returns
        -------
        str
            Wire name such as ``"rolling_beta"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def param_keys(self) -> list[str]:
        """
        JSON ``params`` keys this operation reads.

        Returns
        -------
        list[str]
            Accepted parameter keys; any other key raises ``ValueError``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def values() -> list[str]:
        """
        Every accepted operation name, in declaration order.

        Returns
        -------
        list[str]
            Canonical names.

        Examples
        --------
        >>> from finstack_quant.features import PairwiseOp
        >>> "rolling_beta" in PairwiseOp.values()
        True

        Notes
        -----
        This method does not raise.
        """
        ...

    def __class_getitem__(cls, key: str) -> PairwiseOp:
        """
        Look up an operation by ``UPPER_SNAKE`` member name.

        Parameters
        ----------
        key : str
            Member name such as ``"ROLLING_BETA"``.

        Returns
        -------
        PairwiseOp
            The matching operation.

        Raises
        ------
        KeyError
            If ``key`` is not a member.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class PanelTransformSpec:
    """
    Specification for a sequential panel transform pipeline.

    Examples
    --------
    >>> from finstack_quant.features import PanelTransformSpec
    >>> spec = PanelTransformSpec(
    ...     [1.0, 3.0], [{"name": "r", "family": "cross_sectional", "op": "rank"}], time_key=["d", "d"]
    ... )
    >>> spec.operation_names
    ['r']
    """

    def __init__(
        self,
        values: Sequence[float | None],
        operations: Sequence[dict[str, Any]],
        entity: KeyColumn | None = None,
        order: KeyColumn | None = None,
        time_key: KeyColumn | None = None,
    ) -> None:
        """
        Construct a panel spec.

        Parameters
        ----------
        values : sequence of float or None
            Input value column; ``None`` / NaN is missing.
        operations : sequence of dict
            Ordered operations ``{"name", "family" ("timeseries" |
            "cross_sectional"), "op", "params"?, "input"?}``.
        entity : sequence, optional
            Row-aligned entity keys (required for time-series ops; str, int
            or date-like, coerced to str).
        order : sequence, optional
            Row-aligned sort keys (required for time-series ops).
        time_key : sequence, optional
            Row-aligned partition keys (required for cross-sectional ops).

        Raises
        ------
        ValueError
            If an operation mapping is malformed (unknown family, op, or key).

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformSpec
        >>> PanelTransformSpec([1.0], [], time_key=["d"]).operation_names
        []
        """
        ...

    @property
    def operation_names(self) -> list[str]:
        """
        Output column names in operation order.

        Returns
        -------
        list[str]
            Operation ``name`` fields.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def values(self) -> list[float | None]:
        """
        Input value column.

        Returns
        -------
        list[float | None]
            Values as supplied.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON accepted by :func:`transform_panel_json`.

        Returns
        -------
        str
            JSON document.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PanelTransformSpec:
        """
        Deserialize from JSON (strict field names).

        Parameters
        ----------
        json : str
            JSON document.

        Returns
        -------
        PanelTransformSpec
            Reconstructed spec.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformSpec
        >>> PanelTransformSpec.from_json('{"values": [1.0], "operations": []}').values
        [1.0]
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form.

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    def __repr__(self) -> str: ...

class PanelTransformResult:
    """
    Ordered output columns of a panel transform pipeline.

    Examples
    --------
    >>> from finstack_quant.features import transform_panel
    >>> res = transform_panel({
    ...     "values": [1.0, 3.0],
    ...     "time_key": ["d", "d"],
    ...     "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}],
    ... })
    >>> res.columns
    ['r']
    """

    @property
    def columns(self) -> list[str]:
        """
        Output column names in operation order.

        Returns
        -------
        list[str]
            Column names.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def get_column(self, name: str) -> list[float | None]:
        """
        Values of one output column, aligned to the input rows.

        Parameters
        ----------
        name : str
            Operation output name (case-sensitive).

        Returns
        -------
        list[float | None]
            Column values.

        Raises
        ------
        KeyError
            If no column has that name.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON produced by :func:`transform_panel_json`.

        Returns
        -------
        str
            ``{"columns": [{"name", "values"}, ...]}``.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PanelTransformResult:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON document.

        Returns
        -------
        PanelTransformResult
            Reconstructed result.

        Raises
        ------
        ValueError
            If the JSON is malformed.

        Examples
        --------
        >>> from finstack_quant.features import PanelTransformResult
        >>> PanelTransformResult.from_json('{"columns": []}').columns
        []
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form.

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If a value cannot be represented in JSON.
        """
        ...

    def to_dataframe(self, index: Any | None = None) -> pd.DataFrame:
        """
        Columns as a ``pandas.DataFrame`` (one float column per operation).

        Parameters
        ----------
        index : pandas.Index or sequence, optional
            Row index to attach (e.g. the source frame's index).

        Returns
        -------
        pandas.DataFrame
            ``None`` outputs become ``NaN``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str: ...

def transform_panel(spec: PanelTransformSpec | dict[str, Any] | str) -> PanelTransformResult:
    """
    Apply a named panel transform pipeline (typed twin of :func:`transform_panel_json`).

    Operations run sequentially; each reads the previous column unless
    ``input`` selects ``"values"`` or an earlier operation name.

    Parameters
    ----------
    spec : PanelTransformSpec, dict or str
        Typed spec, an equivalent dict (``values``, ``operations``, optional
        ``entity`` / ``order`` / ``time_key``), or its JSON.

    Returns
    -------
    PanelTransformResult
        Ordered output columns with ``get_column`` and ``to_dataframe``.

    Raises
    ------
    ValueError
        If the spec is malformed, an operation name is duplicated or reserved,
        ``input`` is unknown, or an operation fails.

    Examples
    --------
    >>> from finstack_quant.features import transform_panel
    >>> spec = {
    ...     "values": [1.0, 3.0],
    ...     "time_key": ["d", "d"],
    ...     "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}],
    ... }
    >>> transform_panel(spec).get_column("r")
    [0.0, 1.0]
    """
    ...

def transform_timeseries(
    values: list[float | None],
    entity: KeyColumn,
    order: KeyColumn,
    op: str | TimeSeriesOp | CrossSectionalOp | PairwiseOp,
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Transform a panel value column within each entity over time.

    The input is a flat panel column. Rows are grouped by ``entity``, sorted by
    ``order`` within each group, transformed, and returned in the original input
    order. ``order`` is lexicographic; use ISO-8601 for calendar chronology.
    ``window``, ``periods``, ``half_life``, and EWMA ``span`` count finite
    observations (pandas ``skipna``); missing rows do not decay. ``None`` and
    non-finite numeric values are treated as missing and produce ``None``
    where the requested transform cannot be evaluated.

    Parameters
    ----------
    values : list[float | None]
        Numeric input column. ``None`` represents missing data.
    entity : list[str]
        Entity key for each row; length must match ``values``.
    order : list[str]
        Sort key for each row within an entity; length must match
        ``values``. Ties preserve input order.
    op : str
        Operation name. Supported values are ``"returns"``,
        ``"log_returns"``, ``"diff"``, ``"lag"``,
        ``"rolling_mean"``, ``"rolling_sum"``, ``"rolling_std"``,
        ``"rolling_min"``, ``"rolling_max"``, ``"rolling_zscore"``,
        ``"rolling_rank"``, ``"rolling_quantile"``, ``"rolling_skew"``,
        ``"rolling_kurtosis"``, ``"rolling_slope"``,
        ``"rolling_sharpe"``, ``"rolling_winsorize"``, ``"drawdown"``,
        ``"hampel_filter"``, ``"exponential_decay_weights"``,
        ``"ewma_mean"``, ``"ewma_vol"``, and ``"ewma_zscore"``.
    params : TransformParams or None
        Optional operation parameters:
        ``periods`` for ``returns``, ``log_returns``, ``diff``, and
        ``lag`` (default ``1``); ``window`` and ``min_periods`` for
        rolling operations (defaults ``1`` and ``window``); optional
        ``risk_free`` for ``rolling_sharpe`` (default ``0.0``, same units
        as the return series, no annualization); required
        positive finite pandas ``span`` for EWMA operations (not a
        RiskMetrics ``lambda``); required positive finite ``half_life``
        for ``exponential_decay_weights``.

    Returns
    -------
    list[float | None]
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, ``op`` is unsupported, or params are
        malformed. Integer params must be positive. EWMA operations require
        a positive finite ``span``.

    Notes
    -----
    ``returns`` and ``log_returns`` return ``None`` when the prior value is
    missing or has magnitude at or below ``1e-12``. ``rolling_std`` and
    ``rolling_zscore`` use sample standard deviation and require at least
    two finite observations. ``ewma_mean``, ``ewma_vol``, and
    ``ewma_zscore`` expect a **return** series and share one pandas
    ``adjust=False`` centered-variance recursion. The first finite
    observation has vol ``None`` and z-score ``0.0``. Missing rows skip
    without decaying.     ``rolling_sharpe`` is a period feature
    ``(mean - risk_free) / sample_std`` on returns, not the annualized
    ``analytics`` / GIPS Sharpe. ``risk_free`` defaults to ``0.0`` in the
    same units as the return series. ``drawdown`` takes a **level**
    series; the ``analytics`` drawdown takes **returns**.

    Examples
    --------
    >>> from finstack_quant.features import transform_timeseries
    >>> transform_timeseries([1.0, 3.0, 6.0], ["A"] * 3, ["1", "2", "3"], "diff")
    [None, 2.0, 3.0]
    """
    ...

def transform_cross_sectional(
    values: list[float | None],
    time_key: KeyColumn,
    op: str | TimeSeriesOp | CrossSectionalOp | PairwiseOp,
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Transform a panel value column across entities at each timestamp.

    Rows are partitioned by ``time_key`` and the selected operation is applied
    independently within each partition. Results are returned in the original
    input order. ``None`` and non-finite numeric values are skipped.

    Parameters
    ----------
    values : list[float | None]
        Numeric input column. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    op : str
        Operation name. Supported values are ``"zscore"``, ``"rank"``,
        ``"percentile_rank"``, ``"quantile_bucket"``, ``"demean"``,
        ``"robust_zscore"``, ``"minmax_scale"``, ``"clip"``,
        ``"clip_by_sigma"``,
        ``"normal_score_transform"``, ``"long_short_weights"``,
        ``"cap_weights"``,
        ``"fill_missing"``, ``"is_finite"``, ``"nan_mask"``, and
        ``"winsorize"``.
    params : TransformParams or None
        Optional operation parameters. ``quantile_bucket`` accepts
        ``buckets``; ``clip`` accepts explicit ``lower`` and ``upper``;
        ``clip_by_sigma`` accepts ``sigma``; ``winsorize`` accepts ``lower``
        and ``upper`` quantile
        probabilities; ``cap_weights`` accepts ``max_abs``;
        ``fill_missing`` accepts ``value``.

    Returns
    -------
    list[float | None]
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, ``op`` is unsupported, params are
        malformed, explicit clip bounds are inverted, ``sigma`` is
        negative, or quantile bounds do not satisfy
        ``0 <= lower <= upper <= 1``.

    Notes
    -----
    ``zscore`` uses population standard deviation and returns ``0.0`` for
    finite rows when partition standard deviation is at or below ``1e-12``.
    ``rank`` returns percentile ranks in ``[0, 1]``; ties share the lowest
    tied rank and a single finite row maps to ``0.0``. ``percentile_rank``
    returns open-interval ranks using average tied positions.

    Examples
    --------
    >>> from finstack_quant.features import transform_cross_sectional
    >>> transform_cross_sectional([1.0, 2.0, 3.0], ["2026-01-01"] * 3, "rank")
    [0.0, 0.5, 1.0]
    """
    ...

def transform_cross_sectional_grouped(
    values: list[float | None],
    time_key: KeyColumn,
    groups: KeyColumn,
    op: str | TimeSeriesOp | CrossSectionalOp | PairwiseOp,
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Transform a panel value column within each timestamp/group sub-partition.

    Rows are partitioned by the ``(time_key, groups)`` pair and the selected
    cross-sectional operation is applied independently within each
    sub-partition. Results are returned in the original input order.

    Parameters
    ----------
    values : list[float | None]
        Numeric input column. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    groups : list[str]
        Secondary partition key combined with ``time_key``; length must
        match ``values``.
    op : str
        Cross-sectional operation name. Accepts the same operations as
        :func:`transform_cross_sectional`.
    params : TransformParams or None
        Optional operation parameters, forwarded to the chosen ``op``.

    Returns
    -------
    list[float | None]
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, ``op`` is unsupported, or params are
        malformed.

    Examples
    --------
    >>> from finstack_quant.features import transform_cross_sectional_grouped
    >>> transform_cross_sectional_grouped(
    ...     [1.0, 3.0, 10.0, 14.0],
    ...     ["2026-01-01"] * 4,
    ...     ["tech", "tech", "finance", "finance"],
    ...     "zscore",
    ... )
    [-1.0, 1.0, -1.0, 1.0]
    """
    ...

def neutralize(
    values: list[float | None],
    time_key: KeyColumn,
    exposures: list[list[float | None]],
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Return cross-sectional OLS residuals after regressing on exposures.

    Within each ``time_key`` partition, ``values`` is regressed on the exposure
    columns by ordinary least squares and the residuals are returned. Rows whose
    ``values`` or any exposure is missing are excluded from the fit and map to
    ``None`` in the output.

    Parameters
    ----------
    values : list[float | None]
        Signal column to neutralize. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    exposures : list[list[float | None]]
        Exposure columns, each aligned to ``values`` (same length and
        row order).
    params : TransformParams or None
        Optional parameters. ``fit_intercept`` (default ``True``) adds an
        intercept term to the regression.

    Returns
    -------
    list[float | None]
        Residual column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, an exposure column has the wrong length,
        params are malformed, or a ``time_key`` partition is singular
        or underdetermined (the error names that ``time_key``).

    Examples
    --------
    >>> from finstack_quant.features import neutralize
    >>> neutralize([1.0, 2.0, 2.0, 4.0], ["2026-01-01"] * 4, [[0.0, 1.0, 0.0, 1.0]])
    [-0.5, -1.0, 0.5, 1.0]
    """
    ...

def transform_timeseries_pairwise(
    values: list[float | None],
    other: list[float | None],
    entity: KeyColumn,
    order: KeyColumn,
    op: str | TimeSeriesOp | CrossSectionalOp | PairwiseOp,
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Transform two panel columns per entity with a rolling pairwise operation.

    Rows are grouped by ``entity`` and sorted by ``order`` within each group.
    ``order`` is lexicographic; use ISO-8601 for calendar chronology. Each
    output row is computed from the trailing ``window`` of paired finite
    ``(values, other)`` observations. ``window`` counts finite pairs, not
    calendar days (pandas ``skipna``).

    Parameters
    ----------
    values : list[float | None]
        First numeric column. ``None`` represents missing data.
    other : list[float | None]
        Second numeric column aligned to ``values``; length must match.
    entity : list[str]
        Entity key for each row; length must match ``values``.
    order : list[str]
        Sort key for each row within an entity; length must match
        ``values``. Ties preserve input order.
    op : str
        Operation name. Supported values are ``"rolling_cov"``,
        ``"rolling_corr"``, and ``"rolling_beta"``.
    params : TransformParams or None
        Optional parameters. ``window`` (default ``1``) and
        ``min_periods`` (default ``window``) bound the trailing window.

    Returns
    -------
    list[float | None]
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, ``op`` is unsupported, or params are
        malformed.

    Notes
    -----
    At least two paired finite observations are always required, regardless
    of ``min_periods``.

    Examples
    --------
    >>> from finstack_quant.features import transform_timeseries_pairwise
    >>> beta = transform_timeseries_pairwise(
    ...     [1.0, 2.0, 3.0],
    ...     [1.0, 2.0, 4.0],
    ...     ["A"] * 3,
    ...     ["1", "2", "3"],
    ...     "rolling_beta",
    ...     {"window": 3, "min_periods": 3},
    ... )
    >>> [None if value is None else round(value, 3) for value in beta]
    [None, None, 0.643]
    """
    ...

def rolling_regression_residual(
    values: list[float | None],
    exposures: list[list[float | None]],
    entity: KeyColumn,
    order: KeyColumn,
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Return rolling per-entity OLS residuals against exposure columns.

    Rows are grouped by ``entity`` and sorted by ``order``. For each row, an OLS
    fit of ``values`` on the exposure columns is computed over the trailing
    ``window`` of complete rows, and the row's residual from that fit is
    returned.

    Parameters
    ----------
    values : list[float | None]
        Signal column. ``None`` represents missing data.
    exposures : list[list[float | None]]
        Exposure columns, each aligned to ``values`` (same length and
        row order).
    entity : list[str]
        Entity key for each row; length must match ``values``.
    order : list[str]
        Sort key for each row within an entity; length must match
        ``values``. Ties preserve input order.
    params : TransformParams or None
        Optional parameters. ``window`` (default ``1``); ``min_periods``
        (default ``window``) is the minimum number of complete rows required
        to fit; ``fit_intercept`` (default ``True``).

    Returns
    -------
    list[float | None]
        Residual column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ, an exposure column has the wrong length,
        or params are malformed.

    Notes
    -----
    Rank-deficient windows emit ``None`` for that row. That is intentional
    and unlike :func:`neutralize`, which fails the call.

    Examples
    --------
    >>> from finstack_quant.features import rolling_regression_residual
    >>> residual = rolling_regression_residual(
    ...     [1.0, 2.0, 5.0],
    ...     [[0.0, 1.0, 2.0]],
    ...     ["A"] * 3,
    ...     ["1", "2", "3"],
    ...     {"window": 3, "min_periods": 3},
    ... )
    >>> [None if value is None else round(value, 3) for value in residual]
    [None, None, 0.333]
    """
    ...

def risk_scaled_weights(
    values: list[float | None],
    time_key: KeyColumn,
    volatility: list[float | None],
) -> list[float | None]:
    """
    Convert a signal to dollar-neutral inverse-risk-scaled weights.

    Within each ``time_key`` partition, finite rows with ``|vol| > 1e-12``
    become ``raw = signal / vol``, then ``centered = raw - mean(raw)``,
    then ``weight = centered / sum(|centered|)``.

    Parameters
    ----------
    values : list[float | None]
        Signal column. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    volatility : list[float | None]
        Risk estimate per row, aligned to ``values``. A magnitude at
        or below ``1e-12`` is treated as missing.
    Returns
    -------
    list[float | None]
        Weight column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ.

    Notes
    -----
    Rows with missing ``values`` or ``|volatility| <= 1e-12`` map to
    ``None``. A partition whose centered gross is at or below ``1e-12``
    emits ``0.0`` for those finite rows.

    Examples
    --------
    >>> from finstack_quant.features import risk_scaled_weights
    >>> risk_scaled_weights([1.0, 2.0, 2.0, 4.0], ["2026-01-01"] * 4, [1.0, 2.0, 1.0, 2.0])
    [-0.25, -0.25, 0.25, 0.25]
    """
    ...

def rank_to_weights(
    values: list[float | None],
    time_key: KeyColumn,
) -> list[float | None]:
    """
    Convert cross-sectional ranks into gross-normalized long/short weights.

    Within each ``time_key`` partition, values are ranked, demeaned, and scaled
    so the sum of absolute weights is ``1``, yielding a dollar-neutral long/short
    profile.

    Parameters
    ----------
    values : list[float | None]
        Signal column. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    Returns
    -------
    list[float | None]
        Weight column aligned to ``values``. The output length always matches
        the input length.

    Raises
    ------
    ValueError
        If lengths differ.

    Examples
    --------
    >>> from finstack_quant.features import rank_to_weights
    >>> rank_to_weights([1.0, 2.0, 100.0], ["2026-01-01"] * 3)
    [-0.5, 0.0, 0.5]
    """
    ...

def neutralize_and_zscore(
    values: list[float | None],
    time_key: KeyColumn,
    exposures: list[list[float | None]],
    params: TransformParams | None = None,
) -> list[float | None]:
    """
    Neutralize a signal against exposures, then cross-sectional z-score.

    Runs :func:`neutralize` to residualize ``values`` on the exposure columns
    within each ``time_key`` partition, then applies a ``"zscore"`` transform to
    the residuals.

    Parameters
    ----------
    values : list[float | None]
        Signal column. ``None`` represents missing data.
    time_key : list[str]
        Cross-sectional partition key for each row; length must match
        ``values``.
    exposures : list[list[float | None]]
        Exposure columns, each aligned to ``values`` (same length and
        row order).
    params : TransformParams or None
        Optional parameters forwarded to :func:`neutralize`;
        ``fit_intercept`` (default ``True``).

    Returns
    -------
    list[float | None]
        Z-scored residual column aligned to ``values``. The output length always
        matches the input length.

    Raises
    ------
    ValueError
        If lengths differ, an exposure column has the wrong length,
        params are malformed, or a ``time_key`` partition is singular
        or underdetermined (the error names that ``time_key``).

    Examples
    --------
    >>> from finstack_quant.features import neutralize_and_zscore
    >>> scores = neutralize_and_zscore([1.0, 2.0, 2.0, 4.0], ["2026-01-01"] * 4, [[0.0, 1.0, 0.0, 1.0]])
    >>> [round(value, 3) for value in scores]
    [-0.632, -1.265, 0.632, 1.265]
    """
    ...

def transform_panel_json(spec_json: str) -> str:
    """
    Apply a JSON panel transform pipeline and return JSON result columns.

    ``spec_json`` is a JSON object with:

    - ``values``: list of numbers or ``null``.
    - ``entity`` and ``order``: required when any operation has
      ``"family": "timeseries"``.
    - ``time_key``: required when any operation has
      ``"family": "cross_sectional"``.
    - ``operations``: list of named operations. Each operation has ``name``,
      ``family`` (``"timeseries"`` or ``"cross_sectional"``), ``op``,
      optional ``params``, and optional ``input``.

    Operations run sequentially. ``input`` selects the source column: omit it
    (default) to read the previous operation output, or the raw ``values``
    column for the first operation. Set ``input`` to ``"values"`` to branch
    from the raw column, or to an already evaluated operation name. Forward
    references are rejected.

    Operation names must be unique, non-empty, and must not be the reserved
    name ``values``. Unknown fields are rejected by the Rust serde model.

    Parameters
    ----------
    spec_json : str
        JSON-serialized panel transform specification.

    Returns
    -------
    str
        JSON string shaped as
        ``{"columns": [{"name": name, "values": values}]}``, preserving
        operation order with every output column aligned to the input values.

    Raises
    ------
    ValueError
        If the JSON is malformed, required keys are missing,
        operation names are duplicated, empty, or reserved (``values``),
        ``input`` names an unknown column, or an operation fails
        validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.features import transform_panel_json
    >>> spec = {
    ...     "values": [10.0, 12.0, 20.0, 21.0],
    ...     "time_key": ["1", "2", "1", "2"],
    ...     "operations": [{"name": "rank", "family": "cross_sectional", "op": "rank"}],
    ... }
    >>> json.loads(transform_panel_json(json.dumps(spec)))["columns"][0]["values"]
    [0.0, 0.0, 1.0, 1.0]
    """
    ...
