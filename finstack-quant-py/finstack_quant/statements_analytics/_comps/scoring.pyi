"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/comps/scoring.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements_analytics._comps.peers import CompanyMetrics, PeerSet

class ScoringDimension:
    """
    One weighted rich/cheap scoring dimension.

    Parameters
    ----------
    label : str
        Human-readable label (e.g. ``"Spread vs Leverage"``).
    y : str
        Dependent metric: a canonical name (``"oas_bp"``), a custom key, or
        ``"multiple:<name>"`` for a valuation multiple (``"multiple:ev_ebitda"``).
    x : str | None
        Optional explanatory metric in the same notation. ``None`` (default)
        scores the dependent metric against its peer distribution.
    weight : float
        Keyword-only, required. Finite non-negative relative weight in the
        composite score; zero disables the dimension. Positive weights are
        normalized over usable dimensions.
    direction : str | None
        ``"higher_is_cheap"`` (spread-like) or ``"higher_is_rich"``
        (multiple-like). ``None`` uses the Rust ``ScoreDirection`` default,
        ``"higher_is_cheap"``.

    Raises
    ------
    ValueError
        If ``direction`` or an extractor name is not recognized.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScoringDimension
    >>> ScoringDimension("Spread vs Leverage", "oas_bp", "leverage", weight=1.0).direction
    'higher_is_cheap'
    """
    def __init__(
        self, label: str, y: str, x: str | None = None, *, weight: float, direction: str | None = None
    ) -> None: ...
    @property
    def label(self) -> str:
        """
        Dimension label.

        This property does not raise.

        Returns
        -------
        str
            Dimension label.
        """
    @property
    def y(self) -> str:
        """
        Dependent metric in ``name`` / ``multiple:<name>`` notation.

        This property does not raise.

        Returns
        -------
        str
            Dependent metric in ``name`` / ``multiple:<name>`` notation.
        """
    @property
    def x(self) -> str | None:
        """
        Optional explanatory metric in ``name`` / ``multiple:<name>`` notation; None for distribution scoring.

        This property does not raise.

        Returns
        -------
        str | None
            Optional explanatory metric in ``name`` / ``multiple:<name>`` notation; None for distribution scoring.
        """
    @property
    def weight(self) -> float:
        """
        Weight in the composite score.

        This property does not raise.

        Returns
        -------
        float
            Weight in the composite score.
        """
    @property
    def direction(self) -> str:
        """
        ``"higher_is_cheap"`` or ``"higher_is_rich"``.

        This property does not raise.

        Returns
        -------
        str
            ``"higher_is_cheap"`` or ``"higher_is_rich"``.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ScoringDimension:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScoringDimension
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ScoringDimension`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScoringDimension
        >>> ScoringDimension.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PeerStats:
    """Descriptive statistics of a peer distribution.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import peer_stats
    >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).median
    3.0
    """
    @property
    def count(self) -> int:
        """
        Number of observations.

        This property does not raise.

        Returns
        -------
        int
            Number of observations.
        """
    @property
    def mean(self) -> float:
        """
        Arithmetic mean.

        This property does not raise.

        Returns
        -------
        float
            Arithmetic mean.
        """
    @property
    def median(self) -> float:
        """
        Median of the peer metric across the peer set.

        This property does not raise.

        Returns
        -------
        float
            Median value in the metric's own units.
        """
    @property
    def std_dev(self) -> float:
        """
        Sample standard deviation.

        This property does not raise.

        Returns
        -------
        float
            Sample standard deviation.
        """
    @property
    def min(self) -> float:
        """
        Smallest peer value of the metric.

        This property does not raise.

        Returns
        -------
        float
            Minimum value in the metric's own units.
        """
    @property
    def max(self) -> float:
        """
        Largest peer value of the metric.

        This property does not raise.

        Returns
        -------
        float
            Maximum value in the metric's own units.
        """
    @property
    def q1(self) -> float:
        """
        First quartile (25th percentile) of the peer metric.

        This property does not raise.

        Returns
        -------
        float
            25th-percentile value in the metric's own units.
        """
    @property
    def q3(self) -> float:
        """
        Third quartile (75th percentile) of the peer metric.

        This property does not raise.

        Returns
        -------
        float
            75th-percentile value in the metric's own units.
        """
    @property
    def iqr(self) -> float:
        """
        Interquartile range ``q3 - q1``.

        This property does not raise.

        Returns
        -------
        float
            Interquartile range ``q3 - q1``.
        """
    def to_arrow(self) -> ArrowTable:
        """
        Export as a single-row ``ArrowTable`` with one column per statistic.

        Returns
        -------
        ArrowTable
            Single-row table of peer statistics.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame`` with one column per statistic.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame of peer statistics.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export as a single-row Polars ``DataFrame`` with one column per statistic.

        Returns
        -------
        polars.DataFrame
            Single-row Polars frame of peer statistics.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export as a single-row ``pyarrow.Table`` with one column per statistic.

        Returns
        -------
        pyarrow.Table
            Single-row PyArrow table of peer statistics.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> PeerStats:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerStats
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerStats`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerStats
        >>> PeerStats.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class RegressionResult:
    """Single-factor OLS fit evaluated at the subject.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import regression_fair_value
    >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).fitted_value
    7.0
    """
    @property
    def intercept(self) -> float:
        """
        Intercept (alpha).

        This property does not raise.

        Returns
        -------
        float
            Intercept (alpha).
        """
    @property
    def slope(self) -> float:
        """
        Fitted regression slope on the explanatory variable.

        This property does not raise.

        Returns
        -------
        float
            Slope coefficient in units of y per unit of x.
        """
    @property
    def r_squared(self) -> float:
        """
        Coefficient of determination.

        This property does not raise.

        Returns
        -------
        float
            Coefficient of determination.
        """
    @property
    def fitted_value(self) -> float:
        """
        ``intercept + slope * subject_x``.

        This property does not raise.

        Returns
        -------
        float
            ``intercept + slope * subject_x``.
        """
    @property
    def residual(self) -> float:
        """
        ``subject_y - fitted_value``.

        This property does not raise.

        Returns
        -------
        float
            ``subject_y - fitted_value``.
        """
    @property
    def n(self) -> int:
        """
        Number of observations used.

        This property does not raise.

        Returns
        -------
        int
            Number of observations used.
        """
    def to_arrow(self) -> ArrowTable:
        """
        Export as a single-row ``ArrowTable``.

        Returns
        -------
        ArrowTable
            Single-row table of regression outputs.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame of regression outputs.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Single-row Polars frame of regression outputs.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row PyArrow table of regression outputs.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> RegressionResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        RegressionResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``RegressionResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RegressionResult
        >>> RegressionResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class DimensionScore:
    """
    Decomposed score of one dimension in a `RelativeValueResult`.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DimensionScore
    >>> [field for field in ("label", "percentile") if hasattr(DimensionScore, field)]
    ['label', 'percentile']
    """
    @property
    def label(self) -> str:
        """
        Dimension label.

        This property does not raise.

        Returns
        -------
        str
            Dimension label.
        """
    @property
    def percentile(self) -> float:
        """
        Percentile rank of the subject within peers (0-1).

        This property does not raise.

        Returns
        -------
        float
            Percentile rank of the subject within peers (0-1).
        """
    @property
    def z_score(self) -> float:
        """
        Raw z-score of the subject versus the peer distribution.

        This property does not raise.

        Returns
        -------
        float
            Raw z-score of the subject versus the peer distribution.
        """
    @property
    def regression_residual(self) -> float | None:
        """
        Raw regression residual in Y units, or ``None`` without explanatory X.

        This property does not raise.

        Returns
        -------
        float | None
            Raw regression residual in Y units, or ``None`` without explanatory X.
        """
    @property
    def r_squared(self) -> float | None:
        """
        Regression R-squared, or ``None`` without explanatory X.

        This property does not raise.

        Returns
        -------
        float | None
            Regression R-squared, or ``None`` without explanatory X.
        """
    @property
    def weight(self) -> float:
        """
        Dimension weight in the composite.

        This property does not raise.

        Returns
        -------
        float
            Dimension weight in the composite.
        """

class RelativeValueResult:
    """Composite rich/cheap score of a subject against its peers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import (
    ...     CompanyMetrics,
    ...     PeerSet,
    ...     ScoringDimension,
    ...     score_relative_value,
    ... )
    >>> peers = [
    ...     CompanyMetrics(f"P{i}", {"leverage": float(i), "oas_bp": spread})
    ...     for i, spread in ((1, 100.0), (2, 210.0), (3, 300.0))
    ... ]
    >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"leverage": 2.0, "oas_bp": 250.0}), peers, "ltm")
    >>> result = score_relative_value(
    ...     peer_set, [ScoringDimension("Spread vs Leverage", "oas_bp", "leverage", weight=1.0)]
    ... )
    >>> result.company_id, result.peer_count
    ('SUBJ', 3)
    """
    @property
    def company_id(self) -> str:
        """
        Subject company id.

        This property does not raise.

        Returns
        -------
        str
            Subject company id.
        """
    @property
    def composite_score(self) -> float:
        """
        Weighted composite score: positive = cheap, negative = rich.

        This property does not raise.

        Returns
        -------
        float
            Weighted composite score: positive = cheap, negative = rich.
        """
    @property
    def dimensions(self) -> list[DimensionScore]:
        """
        Per-dimension decomposition.

        This property does not raise.

        Returns
        -------
        list[DimensionScore]
            Per-dimension decomposition.
        """
    @property
    def confidence(self) -> float:
        """
        Confidence in ``[0, 1]`` from peer count and regression fit.

        This property does not raise.

        Returns
        -------
        float
            Confidence in ``[0, 1]`` from peer count and regression fit.
        """
    @property
    def peer_count(self) -> int:
        """
        Number of peers scored against.

        This property does not raise.

        Returns
        -------
        int
            Number of peers scored against.
        """
    def to_arrow(self) -> ArrowTable:
        """
        Export the per-dimension scores as an ``ArrowTable``.

        Columns: ``label``, ``percentile`` (0-1), ``z_score``,
        ``regression_residual`` (Y units, null without X), ``r_squared``
        (null without X), ``weight``. One row per dimension. The composite
        score, confidence and peer count are result metadata.

        Returns
        -------
        ArrowTable
            Per-dimension scores table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the per-dimension scores as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`. One row per dimension.

        Returns
        -------
        pd.DataFrame
            Per-dimension scores DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the per-dimension scores as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Per-dimension scores Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the per-dimension scores as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Per-dimension scores PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> RelativeValueResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        RelativeValueResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``RelativeValueResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueResult
        >>> RelativeValueResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def percentile_rank(values: list[float], value: float) -> float | None:
    """Percentile rank of ``value`` within ``values`` (0-1 scale).

    Uses the "fraction of values less than or equal" convention (Rust
    ``percentile_rank(values, value)``).

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    values : list[float]
        Peer distribution (need not be sorted); non-finite entries are ignored.
    value : float
        The subject value to rank.

    Returns
    -------
    float | None
        Percentile rank in ``[0, 1]``, or ``None`` when ``values`` is empty.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import percentile_rank
    >>> percentile_rank([100.0, 200.0, 300.0, 400.0, 500.0], 250.0)
    0.4
    """

def z_score(values: list[float], value: float) -> float | None:
    """Standard (z-) score of ``value`` in the peer distribution (Rust ``z_score(values, value)``).

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    values : list[float]
        Peer distribution; non-finite entries are ignored.
    value : float
        The subject value.

    Returns
    -------
    float | None
        ``(value - mean(peers)) / stddev(peers)``, or ``None`` when fewer than
        two peers are provided or the peer variance is zero.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import z_score
    >>> z_score([1.0, 2.0, 3.0, 4.0, 5.0], 3.0)
    0.0
    """

def peer_stats(values: list[float]) -> PeerStats | None:
    """Descriptive statistics for a peer distribution (Rust ``peer_stats(values)``).

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    values : list[float]
        Peer distribution (need not be sorted); non-finite entries are ignored.

    Returns
    -------
    PeerStats | None
        Typed statistics, or ``None`` when no statistics can be computed
        (matching the WASM twin's ``undefined``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import peer_stats
    >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).count
    5
    """

def regression_fair_value(
    x_values: list[float], y_values: list[float], subject_x: float, subject_y: float
) -> RegressionResult | None:
    """Single-factor OLS fit and evaluation at the subject's X.

    Conventions: ``fitted_value = intercept + slope * subject_x`` and
    ``residual = subject_y - fitted_value``.

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    x_values : list[float]
        Peer X observations (independent variable).
    y_values : list[float]
        Peer Y observations; same length as ``x_values``.
    subject_x : float
        Subject's X value at which to evaluate the fit.
    subject_y : float
        Subject's observed Y value for the residual.

    Returns
    -------
    RegressionResult | None
        Typed fit, or ``None`` if fewer than three observations are available
        or X has zero variance, lengths differ, or any input/output is non-finite.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import regression_fair_value
    >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).residual
    3.0
    """

def compute_multiple(company_metrics: Any, multiple: str) -> float | None:
    """Compute a canonical valuation multiple for one company.

    Parameters
    ----------
    company_metrics : CompanyMetrics | dict[str, float | None]
        Typed metrics or a flat ``{metric_name: value}`` dict; only the fields
        the multiple needs must be populated. A ``None`` value is a missing
        metric (the Rust ``CompanyMetrics::from_flat_metrics`` rule), as a
        ``null`` is in the WASM ``computeMultiple``.
    multiple : str
        Serde name of the multiple: ``ev_ebitda``, ``ev_revenue``, ``ev_ebit``,
        ``ev_fcf``, ``pe``, ``pb``, ``ptbv``, ``p_fcf``, ``dividend_yield``,
        ``spread_per_turn`` or ``yield_per_coverage``.

    Returns
    -------
    float | None
        Multiple value, or ``None`` when a required input is missing or the
        denominator is not positive.

    Raises
    ------
    ValueError
        If ``multiple`` is not a known name or a metric value is not numeric.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import compute_multiple
    >>> compute_multiple({"enterprise_value": 8_500.0, "ebitda": 1_000.0}, "ev_ebitda")
    8.5
    """

def score_relative_value(peer_set: Any, dimensions: Any) -> RelativeValueResult:
    """Score a subject against its peers across weighted dimensions.

    The composite is the weighted average of the direction-adjusted dimension
    scores: positive = cheap, negative = rich.
    Unusable dimensions (missing metrics, insufficient or degenerate samples)
    are excluded; regression dimensions retain their regression semantics.

    Parameters
    ----------
    peer_set : PeerSet | dict
        Typed peer set, or the canonical serde ``PeerSet`` payload as a dict
        (``{"subject": ..., "peers": [...], "period_basis": "ltm"}``).
    dimensions : list[ScoringDimension | dict]
        Typed dimensions or canonical ``ScoringDimension`` dicts.

    Returns
    -------
    RelativeValueResult
        Composite score, per-dimension breakdown, confidence and peer count.

    Raises
    ------
    ValueError
        If a payload is malformed, a direction or extractor is unknown, or the
        weights are negative/non-finite, no positive-weight dimension is
        usable, or the composite cannot be represented as a finite number.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import (
    ...     CompanyMetrics,
    ...     PeerSet,
    ...     ScoringDimension,
    ...     score_relative_value,
    ... )
    >>> peers = [CompanyMetrics(f"P{i}", {"pe": float(10 * i)}) for i in (1, 2, 3)]
    >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"pe": 30.0}), peers, "ltm")
    >>> score_relative_value(
    ...     peer_set, [ScoringDimension("pe", "pe", weight=1.0, direction="higher_is_rich")]
    ... ).composite_score < 0
    True
    """
