"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/comps.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class CompanyMetrics:
    """
    Metrics for one company in a peer set.

    Monetary values must already be in one currency; ratios are plain scalars
    (``6.5`` = 6.5x) and growth/margin inputs are decimals (``0.05`` = 5%).
    Canonical metric names populate dedicated fields; any other name is kept
    in ``custom``.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    id : str
        Company identifier.
    metrics : dict[str, float | None]
        Flat ``{metric_name: value}`` map. Known names (``enterprise_value``,
        ``market_cap``, ``share_price``, ``oas_bp``, ``yield_pct``, ``ebitda``,
        ``revenue``, ``ebit``, ``ufcf``, ``lfcf``, ``net_income``,
        ``book_value``, ``tangible_book_value``, ``dividends_per_share``,
        ``leverage``, ``interest_coverage``, ``revenue_growth``,
        ``ebitda_margin``) fill their fields; other names go to ``custom``.
        ``None`` values are treated as missing.
    tags : list[str] | None
        Attribute tags used by ``PeerFilter.required_tags`` / ``excluded_tags``.
    meta : dict[str, str] | None
        Attribute metadata (``gics_sector``, ``gics_industry``, ``country``,
        ``rating``) used by ``PeerFilter``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics
    >>> CompanyMetrics("ACME", {"leverage": 3.0, "oas_bp": 250.0}).get("leverage")
    3.0
    """
    def __init__(
        self, id: str, metrics: Any | None = None, tags: list[str] | None = None, meta: dict[str, str] | None = None
    ) -> None: ...
    @property
    def id(self) -> str:
        """
        Company identifier.

        This property does not raise.

        Returns
        -------
        str
            Company identifier.
        """
    @property
    def tags(self) -> list[str]:
        """
        Free-form attribute tags carried alongside the metrics.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags in insertion order; empty when the company carries none.
        """
    @property
    def meta(self) -> dict[str, str]:
        """
        Attribute metadata.

        This property does not raise.

        Returns
        -------
        dict[str, str]
            Attribute metadata.
        """
    @property
    def custom(self) -> list[tuple[str, float]]:
        """
        Custom (non-canonical) metrics.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Custom (non-canonical) metrics.
        """
    def get(self, name: str) -> float | None:
        """
        Read one metric by name (canonical field or ``custom`` key).

        This method reads already-computed state and does not raise.

        Parameters
        ----------
        name : str
            Canonical metric field name (for example ``"ebitda"``) or a key
            of the ``custom`` mapping. Absent metrics return ``None``.

        Returns
        -------
        float | None
            Read one metric by name (canonical field or ``custom`` key). Returns ``None`` when the metric is absent.
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
    def from_json(json: str) -> CompanyMetrics:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CompanyMetrics
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CompanyMetrics`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CompanyMetrics
        >>> CompanyMetrics.from_json("{")
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

class PeerFilter:
    """
    Screening criteria for building a peer set from a universe.

    All non-empty criteria are AND-ed; list criteria are OR-ed within.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    gics_sectors : list[str]
        GICS sector codes to include (``meta["gics_sector"]``).
    gics_industries : list[str]
        GICS industry codes to include (``meta["gics_industry"]``).
    countries : list[str]
        ISO country codes to include (``meta["country"]``).
    market_cap_min : float | None
        Inclusive market-cap floor.
    market_cap_max : float | None
        Inclusive market-cap ceiling.
    ratings : list[str]
        Rating bands to include (``meta["rating"]``).
    required_tags : list[str]
        Tags every peer must carry.
    excluded_tags : list[str]
        Tags no peer may carry.
    selectors : list[str]
        Attribute selector strings (``Attributes.matches_selector``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import PeerFilter
    >>> PeerFilter(ratings=["BB", "B"]).ratings
    ['BB', 'B']
    """
    def __init__(
        self,
        gics_sectors: list[str] = ...,
        gics_industries: list[str] = ...,
        countries: list[str] = ...,
        market_cap_min: float | None = None,
        market_cap_max: float | None = None,
        ratings: list[str] = ...,
        required_tags: list[str] = ...,
        excluded_tags: list[str] = ...,
        selectors: list[str] = ...,
    ) -> None: ...
    @property
    def gics_sectors(self) -> list[str]:
        """
        GICS sector codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            GICS sector codes to include.
        """
    @property
    def gics_industries(self) -> list[str]:
        """
        GICS industry codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            GICS industry codes to include.
        """
    @property
    def countries(self) -> list[str]:
        """
        ISO country codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            ISO country codes to include.
        """
    @property
    def market_cap_min(self) -> float | None:
        """
        Inclusive market-cap floor, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Inclusive market-cap floor, or ``None``.
        """
    @property
    def market_cap_max(self) -> float | None:
        """
        Inclusive market-cap ceiling, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Inclusive market-cap ceiling, or ``None``.
        """
    @property
    def ratings(self) -> list[str]:
        """
        Rating bands to include.

        This property does not raise.

        Returns
        -------
        list[str]
            Rating bands to include.
        """
    @property
    def required_tags(self) -> list[str]:
        """
        Tags every peer must carry.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags every peer must carry.
        """
    @property
    def excluded_tags(self) -> list[str]:
        """
        Tags no peer may carry.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags no peer may carry.
        """
    @property
    def selectors(self) -> list[str]:
        """
        Attribute selector strings.

        This property does not raise.

        Returns
        -------
        list[str]
            Attribute selector strings.
        """
    def accepts(self, company: CompanyMetrics) -> bool:
        """
        Whether ``company`` satisfies every criterion.

        This method reads already-computed state and does not raise.

        Parameters
        ----------
        company : CompanyMetrics
            Candidate peer tested against the filter's size, sector and
            metric criteria.

        Returns
        -------
        bool
            Whether ``company`` satisfies every criterion.
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
    def from_json(json: str) -> PeerFilter:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerFilter
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerFilter`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerFilter
        >>> PeerFilter.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PeerSet:
    """
    A subject company alongside its comparison peers.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    subject : CompanyMetrics | dict | str
        The company being evaluated (typed, serde dict, or JSON).
    peers : list[CompanyMetrics | dict | str]
        Peer companies.
    period_basis : str
        ``"ltm"``, ``"ntm"`` or a custom label such as ``"FY2025E"``.
        Required: Rust ``PeerSet`` has no default period basis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics, PeerSet
    >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.0})
    >>> peers = [CompanyMetrics("P1", {"leverage": 1.0}), CompanyMetrics("P2", {"leverage": 3.0})]
    >>> PeerSet(subject, peers, "ltm").peer_count
    2
    """
    def __init__(self, subject: Any, peers: list[Any], period_basis: str) -> None: ...
    @staticmethod
    def from_universe(subject: Any, universe: list[Any], filter: PeerFilter, period_basis: str) -> PeerSet:
        """
        Build a peer set from a universe by applying a ``PeerFilter``.

        The subject is never included in the peers even when it passes.

        Raises
        ------
        ValueError
            If ``subject`` or a universe entry is not valid company metrics.

        Parameters
        ----------
        subject : CompanyMetrics | dict | str
            The company being evaluated.
        universe : list[CompanyMetrics | dict | str]
            Candidate companies.
        filter : PeerFilter
            Screening criteria.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label (required).
        Returns
        -------
        PeerSet
            Build a peer set from a universe by applying a ``PeerFilter``. The subject is never included in the peers even when it passes.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerFilter, PeerSet
        >>> PeerSet.from_universe("{}", [], PeerFilter(), "ltm")
        Traceback (most recent call last):
        ValueError: ...
        """
    @staticmethod
    def from_dataframe(df: Any, subject_id: str, period_basis: str, id_column: str | None = None) -> PeerSet:
        """
        Build a peer set from a pandas ``DataFrame`` (rows = companies).

        Parameters
        ----------
        df : pandas.DataFrame
            One row per company. The index (or ``id_column``) supplies company
            ids; numeric columns become metrics (canonical names fill their
            fields, others go to ``custom``); string columns become attribute
            ``meta`` entries; ``NaN``/``None`` cells are treated as missing.
        subject_id : str
            Id of the subject row; every other row becomes a peer.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label (required).
        id_column : str | None
            Column holding company ids; ``None`` uses the index.

        Returns
        -------
        PeerSet
            The selected subject and every other company row as its peers.

        Raises
        ------
        KeyError
            If ``subject_id`` is not present.
        ValueError
            If a cell is neither numeric, string, nor missing.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.statements_analytics import PeerSet
        >>> frame = pd.DataFrame({"id": ["SUBJ", "PEER"], "leverage": [3.0, 2.0]})
        >>> PeerSet.from_dataframe(frame, "SUBJ", "ltm", id_column="id").peer_count
        1
        """
    @property
    def subject(self) -> CompanyMetrics:
        """
        The subject company.

        This property does not raise.

        Returns
        -------
        CompanyMetrics
            The subject company.
        """
    @property
    def peers(self) -> list[CompanyMetrics]:
        """
        Companies retained as comparables for the subject.

        This property does not raise.

        Returns
        -------
        list[CompanyMetrics]
            Peers in selection order, always excluding the subject.
        """
    @property
    def period_basis(self) -> str:
        """
        Period basis label (``"ltm"``, ``"ntm"`` or the custom label).

        This property does not raise.

        Returns
        -------
        str
            Period basis label (``"ltm"``, ``"ntm"`` or the custom label).
        """
    @property
    def peer_count(self) -> int:
        """
        Number of peers (excluding the subject).

        This property does not raise.

        Returns
        -------
        int
            Number of peers (excluding the subject).
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
    def from_json(json: str) -> PeerSet:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerSet
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerSet`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerSet
        >>> PeerSet.from_json("{")
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
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame`` with one column per statistic.

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame`` with one column per statistic.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
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
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame``.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
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
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-dimension scores as a pandas ``DataFrame``.

        Columns: ``label``, ``percentile`` (0-1), ``z_score``,
        ``regression_residual`` (Y units, ``NaN`` without X), ``r_squared``
        (``NaN`` without X), ``weight``. One row per dimension. The composite
        score, confidence and peer count are result metadata.

        Returns
        -------
        pd.DataFrame
            Export the per-dimension scores as a pandas ``DataFrame``. Columns: ``label``, ``percentile`` (0-1), ``z_score``, ``regression_residual`` (Y units, ``NaN`` without X), ``r_squared`` (``NaN`` without X), ``weight``. One row per dimension. The composite score, confidence and peer count are result metadata.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
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

def score_relative_value(peer_set: Any, dimensions: Any) -> RelativeValueResult:
    """Score a subject against its peers across weighted dimensions.

    The composite is the weighted average of the direction-adjusted dimension
    scores: positive = cheap, negative = rich.
    Unusable dimensions (missing metrics, insufficient or degenerate samples)
    are excluded; regression dimensions retain their regression semantics.

    Parameters
    ----------
    peer_set : PeerSet | dict | str
        Typed peer set, or the canonical serde ``PeerSet`` payload as a dict or
        JSON string (``{"subject": ..., "peers": [...], "period_basis": "ltm"}``).
    dimensions : list[ScoringDimension | dict] | str
        Typed dimensions, canonical ``ScoringDimension`` dicts, or a JSON list.

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
