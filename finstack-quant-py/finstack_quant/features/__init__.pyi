from __future__ import annotations

from typing import Any

TransformParams = dict[str, Any]

__all__: list[str] = [
    "transform_cross_sectional",
    "transform_panel",
    "transform_timeseries",
]

def transform_timeseries(
    values: list[float | None],
    entity: list[str],
    order: list[str],
    op: str,
    params: TransformParams | None = None,
) -> list[float | None]:
    """Transform a panel value column within each entity over time.

    The input is a flat panel column. Rows are grouped by ``entity``, sorted by
    ``order`` within each group, transformed, and returned in the original input
    order. ``None`` and non-finite numeric values are treated as missing and
    produce ``None`` where the requested transform cannot be evaluated.

    Args:
        values: Numeric input column. ``None`` represents missing data.
        entity: Entity key for each row; length must match ``values``.
        order: Sort key for each row within an entity; length must match
            ``values``. Ties preserve input order.
        op: Operation name. Supported values are ``"returns"``,
            ``"log_returns"``, ``"lag"``, ``"rolling_mean"``,
            ``"rolling_sum"``, ``"rolling_std"``, ``"rolling_min"``,
            ``"rolling_max"``, and ``"ewma"``.
        params: Optional operation parameters:
            ``periods`` for ``returns``, ``log_returns``, and ``lag`` (default
            ``1``); ``window`` and ``min_periods`` for rolling operations
            (defaults ``1`` and ``window``); required positive finite ``span``
            for ``ewma``.

    Returns:
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises:
        ValueError: If lengths differ, ``op`` is unsupported, or params are
            malformed. Integer params must be positive. ``ewma`` requires a
            positive finite ``span``.

    Notes:
        ``returns`` and ``log_returns`` return ``None`` when the prior value is
        missing or has magnitude at or below ``1e-12``. ``rolling_std`` is
        sample standard deviation and requires at least two finite observations.
    """
    ...

def transform_cross_sectional(
    values: list[float | None],
    time_key: list[str],
    op: str,
    params: TransformParams | None = None,
) -> list[float | None]:
    """Transform a panel value column across entities at each timestamp.

    Rows are partitioned by ``time_key`` and the selected operation is applied
    independently within each partition. Results are returned in the original
    input order. ``None`` and non-finite numeric values are skipped.

    Args:
        values: Numeric input column. ``None`` represents missing data.
        time_key: Cross-sectional partition key for each row; length must match
            ``values``.
        op: Operation name. Supported values are ``"zscore"``, ``"rank"``,
            ``"demean"``, and ``"winsorize"``.
        params: Optional operation parameters. ``winsorize`` accepts ``lower``
            and ``upper`` quantile probabilities, defaulting to ``0.01`` and
            ``0.99``.

    Returns:
        Output column aligned to ``values``. The output length always matches
        the input length.

    Raises:
        ValueError: If lengths differ, ``op`` is unsupported, params are
            malformed, or ``winsorize`` does not satisfy
            ``0 <= lower <= upper <= 1``.

    Notes:
        ``zscore`` uses population standard deviation and returns ``0.0`` for
        finite rows when partition standard deviation is at or below ``1e-12``.
        ``rank`` returns percentile ranks in ``[0, 1]``; ties share the lowest
        tied rank and a single finite row maps to ``0.0``.
    """
    ...

def transform_panel(spec_json: str) -> str:
    """Apply a JSON panel transform pipeline and return JSON result columns.

    ``spec_json`` is a JSON object with:

    - ``values``: list of numbers or ``null``.
    - ``entity`` and ``order``: required when any operation has
      ``"family": "timeseries"``.
    - ``time_key``: required when any operation has
      ``"family": "cross_sectional"``.
    - ``operations``: list of named operations. Each operation has ``name``,
      ``family`` (``"timeseries"`` or ``"cross_sectional"``), ``op``, and
      optional ``params``.

    Operation names must be unique and non-empty. Unknown fields are rejected by
    the Rust serde model.

    Args:
        spec_json: JSON-serialized panel transform specification.

    Returns:
        JSON string shaped as ``{"columns": {name: values}}``, where each
        output column is aligned to the input ``values``.

    Raises:
        ValueError: If the JSON is malformed, required keys are missing,
            operation names are duplicated or empty, or an operation fails
            validation.
    """
    ...
