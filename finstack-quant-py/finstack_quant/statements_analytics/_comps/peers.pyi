"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/comps/peers.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

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

class PeerFilter:
    """
    Screening criteria for building a peer set from a universe.

    All non-empty criteria are AND-ed; list criteria are OR-ed within.

    Every parameter is keyword-only: Rust reads the keywords as the
    ``PeerFilter`` serde struct, in which every criterion is optional: an omitted
    list is ``[]`` and an omitted bound ``None`` (no constraint).

    Raises
    ------
    ValueError
        If a keyword is unknown or a value does not have its wire form.
    TypeError
        If a positional argument is passed.

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
        *,
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
    subject : CompanyMetrics | dict
        The company being evaluated (typed or serde dict).
    peers : list[CompanyMetrics | dict]
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
    def from_universe(subject: Any, universe: list[Any], filter: PeerFilter, period_basis: str) -> tuple[PeerSet, int]:
        """
        Build a peer set from a universe by applying a ``PeerFilter``.

        The subject is never included in the peers even when it passes.

        Raises
        ------
        ValueError
            If ``subject`` or a universe entry is not valid company metrics.

        Parameters
        ----------
        subject : CompanyMetrics | dict
            The company being evaluated.
        universe : list[CompanyMetrics | dict]
            Candidate companies.
        filter : PeerFilter
            Screening criteria.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label (required).
        Returns
        -------
        tuple[PeerSet, int]
            The peer set and the number of universe members the filter dropped;
            a member with the subject's id is not counted.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerFilter, PeerSet
        >>> PeerSet.from_universe({}, [], PeerFilter(), "ltm")
        Traceback (most recent call last):
        ValueError: ...
        """
    @staticmethod
    def from_dataframe(
        df: pd.DataFrame | pl.DataFrame | Any,
        subject_id: str,
        period_basis: str,
        id_column: str | None = None,
    ) -> PeerSet:
        """
        Build a peer set from a pandas ``DataFrame``, Polars ``DataFrame``, or Arrow table (rows = companies).

        Parameters
        ----------
        df : pandas.DataFrame | polars.DataFrame | Any
            One row per company. The pandas index or ``id_column`` (for Polars/
            Arrow, ``id_column`` or the first column when omitted) supplies
            company ids; numeric columns become metrics (canonical names fill
            their fields, others go to ``custom``); string columns become
            attribute ``meta`` entries; ``NaN``/``None``/``null`` cells are
            treated as missing.
        subject_id : str
            Id of the subject row; every other row becomes a peer.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label (required).
        id_column : str | None
            Column holding company ids; ``None`` uses the pandas index (or the
            first column for Polars/Arrow).

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
