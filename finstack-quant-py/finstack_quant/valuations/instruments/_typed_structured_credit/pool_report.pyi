"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/pool_report.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.pool_stats import PoolStats

class PoolConcentrationEntry:
    """
    Balance held by one obligor, industry or rating: a row of a
    :class:`PoolReport` concentration table.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PoolConcentrationEntry
    >>> row = PoolConcentrationEntry.from_json(
    ...     '{"key": "BB+", "balance": {"amount": "250", "currency": "USD"}, "balance_pct": 25.0, "asset_count": 2}'
    ... )
    >>> row.key, row.balance.amount, row.balance_pct, row.asset_count
    ('BB+', 250.0, 25.0, 2)
    """

    @staticmethod
    def from_json(json: str) -> PoolConcentrationEntry:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PoolConcentrationEntry`` (the shape ``to_json`` writes).

        Returns
        -------
        PoolConcentrationEntry
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PoolConcentrationEntry
        >>> try:
        ...     PoolConcentrationEntry.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``PoolConcentrationEntry``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PoolConcentrationEntry.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def key(self) -> str | None:
        """
        Obligor id, industry or rating wire form (``"BB+"``) this row groups.

        Returns
        -------
        str | None
            The group, or ``None`` for the assets with no value on the
            dimension.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Current balance of the row's assets, defaulted assets included.

        Returns
        -------
        Money
            Amount in the pool's base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balance_pct(self) -> float:
        """
        Share of :attr:`PoolReport.total_balance`.

        Returns
        -------
        float
            Percent points (``20.0`` = 20%), as ``ConcentrationLimit.max_pct``
            and ``PoolStats.defaulted_balance_pct``. The rows of one table sum
            to ``100.0``; ``0.0`` when the pool balance is not positive.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_count(self) -> int:
        """
        Number of asset rows grouped, zero-balance rows included.

        Returns
        -------
        int
            The asset count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PoolReport:
    """
    Statistics, WARF and concentration of a collateral pool at one date (the
    return value of :func:`pool_report`).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PoolReport, StructuredCredit, pool_report
    >>> report = pool_report(StructuredCredit.example().pool, "2024-01-01")
    >>> PoolReport.from_json(report.to_json()).warf == report.warf
    True
    >>> list(report.to_pandas().columns)
    ['dimension', 'key', 'balance', 'currency', 'balance_pct', 'asset_count']
    """

    @staticmethod
    def from_json(json: str) -> PoolReport:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PoolReport`` (the shape ``to_json`` writes).

        Returns
        -------
        PoolReport
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PoolReport
        >>> try:
        ...     PoolReport.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``PoolReport``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PoolReport.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        The three concentration tables as one long :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``dimension`` (``"obligor"``, ``"industry"`` or ``"rating"``),
        ``key`` (``None`` for the assets with no value on the dimension),
        ``balance``, ``currency``, ``balance_pct`` (percent points) and
        ``asset_count``. Rows keep each table's order. :meth:`PoolStats.to_arrow`
        on :attr:`stats` is the statistics table.

        Returns
        -------
        ArrowTable
            One row per concentration entry.

        Raises
        ------
        ValueError
            If the table cannot be constructed.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        The three concentration tables as one long pandas ``DataFrame``.

        Columns: ``dimension`` (``"obligor"``, ``"industry"`` or ``"rating"``),
        ``key`` (``None`` for the assets with no value on the dimension),
        ``balance``, ``currency``, ``balance_pct`` (percent points) and
        ``asset_count``. Rows keep each table's order. :meth:`PoolStats.to_pandas`
        on :attr:`stats` is the statistics frame.

        Returns
        -------
        pd.DataFrame
            One row per concentration entry.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the table cannot be converted.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        The three concentration tables as one long Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per concentration entry with columns matching :meth:`to_arrow`.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the table cannot be converted.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        The three concentration tables as one long ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per concentration entry with columns matching :meth:`to_arrow`.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the table cannot be converted.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    @property
    def total_balance(self) -> Money:
        """
        Current balance of every asset row, defaulted assets included.

        Returns
        -------
        Money
            Amount in the pool's base currency; the denominator of each
            ``balance_pct``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stats(self) -> PoolStats:
        """
        :func:`calculate_pool_stats` of the same rows.

        Returns
        -------
        PoolStats
            Statistics whose fields mix units; see
            :func:`pool_report_field_units`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warf(self) -> float | None:
        """
        Moody's weighted-average rating factor of the performing collateral.

        Weighted by balance over the performing rows only: defaulted and
        non-positive-balance rows are out of numerator and denominator. An
        unrated asset takes the Moody's table factor for ``NR``.

        Returns
        -------
        float | None
            The rating factor, as the ``clo_warf`` metric reports it; ``None``
            when no performing balance remains, where that metric reports ``0.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_obligor(self) -> list[PoolConcentrationEntry]:
        """
        Balance per ``PoolAsset.obligor_id``.

        Returns
        -------
        list[PoolConcentrationEntry]
            Rows by balance descending, then key ascending with ``None`` first.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_industry(self) -> list[PoolConcentrationEntry]:
        """
        Balance per ``PoolAsset.industry``.

        Returns
        -------
        list[PoolConcentrationEntry]
            Rows by balance descending, then key ascending with ``None`` first.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_rating(self) -> list[PoolConcentrationEntry]:
        """
        Balance per ``PoolAsset.rating`` wire form.

        An asset with no rating is in the ``None`` row, not in an explicit
        ``"NR"`` row, although :attr:`warf` weights both with the ``NR`` factor.

        Returns
        -------
        list[PoolConcentrationEntry]
            Rows by balance descending, then key ascending with ``None`` first.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def pool_report(pool: AssetPool, as_of: datetime.date | str) -> PoolReport:
    """
    Statistics, WARF and concentration of a collateral pool (mirrors Rust ``pool_report``).

    Each concentration table covers every asset row, defaulted assets included
    in balances and denominator, so its shares sum to 100%. Assets with no value
    on a dimension form one row with key ``None``. Rows are ordered by balance
    descending, then key ascending with ``None`` first. Representative lines and
    instrument collateral are normalized to asset rows first and land in the
    ``None`` rows.

    Parameters
    ----------
    pool : AssetPool
        Collateral pool to report on, in any of its three representations.
    as_of : datetime.date | str
        Date the remaining maturities are measured from, either a date-like
        object or an ISO 8601 string. It also stands in for the closing date
        when a pool is normalized: it ages representative lines and counts the
        delayed draws already funded on instrument collateral.

    Returns
    -------
    PoolReport
        The statistics, WARF, pool balance and the tables by obligor, industry
        and rating. An empty pool reports zero balances and empty tables.

    Raises
    ------
    ValueError
        If ``as_of`` is not a date, the pool cannot be normalized, an asset is
        not in the pool's base currency, or a rating has no Moody's factor.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StructuredCredit, pool_report
    >>> report = pool_report(StructuredCredit.example().pool, "2024-01-01")
    >>> sum(row.balance_pct for row in report.by_rating)
    100.0
    """
    ...
