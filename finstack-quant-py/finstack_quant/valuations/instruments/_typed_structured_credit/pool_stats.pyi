"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/pool_stats.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool

class PoolStats:
    """
    Collateral-pool statistics at one date (the return value of
    :func:`calculate_pool_stats`): weighted-average coupon, spread and
    maturity, Moody's diversity score, obligor and industry counts, the
    defaulted share and the undrawn commitment.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PoolStats
    >>> stats = PoolStats.from_json(
    ...     '{"wac": 0.07, "weighted_avg_spread_bp": 0.0, "weighted_avg_maturity": 10.0,'
    ...     ' "diversity_score": 1.0, "num_obligors": 0, "num_industries": 0,'
    ...     ' "defaulted_balance_pct": 0.0, "undrawn_commitment": 0.0}'
    ... )
    >>> stats.wac, list(stats.to_dataframe().columns)[:2]
    (0.07, ['wac', 'weighted_avg_spread_bp'])
    """

    @staticmethod
    def from_json(json: str) -> PoolStats:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PoolStats`` (the shape ``to_json`` writes).

        Returns
        -------
        PoolStats
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PoolStats
        >>> try:
        ...     PoolStats.from_json("{}")
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
            JSON-encoded ``PoolStats``.

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
            ``(PoolStats.from_json, (json,))``.

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

    def to_dataframe(self) -> pd.DataFrame:
        """
        The statistics as a one-row pandas ``DataFrame``.

        Columns: ``wac``, ``weighted_avg_spread_bp``,
        ``weighted_avg_maturity``, ``diversity_score``, ``num_obligors``,
        ``num_industries``, ``defaulted_balance_pct``, ``undrawn_commitment``.

        Returns
        -------
        pd.DataFrame
            One row holding every statistic.

        Raises
        ------
        ValueError
            If the statistics cannot be serialized.
        """
        ...

    @property
    def wac(self) -> float:
        """
        Weighted-average coupon of the performing fixed-rate collateral.

        Returns
        -------
        float
            Annual decimal (``0.07`` = 7%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weighted_avg_spread_bp(self) -> float:
        """
        Weighted-average spread of the performing collateral that carries a spread.

        Returns
        -------
        float
            Spread in basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weighted_avg_maturity(self) -> float:
        """
        Balance-weighted average remaining maturity.

        Returns
        -------
        float
            Maturity in years from the statistics date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def diversity_score(self) -> float:
        """
        Moody's diversity score of the pool.

        Returns
        -------
        float
            The diversity score.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_obligors(self) -> int:
        """
        Number of distinct obligors.

        Returns
        -------
        int
            The obligor count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_industries(self) -> int:
        """
        Number of distinct industries.

        Returns
        -------
        int
            The industry count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def defaulted_balance_pct(self) -> float:
        """
        Defaulted balance as a share of the current total pool balance.

        Returns
        -------
        float
            Percent points (``10.0`` = 10%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def undrawn_commitment(self) -> float:
        """
        Undrawn commitment across performing revolving and delayed-draw collateral.

        Returns
        -------
        float
            Amount in pool-currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def calculate_pool_stats(pool: AssetPool, as_of: datetime.date | str) -> PoolStats:
    """
    Collateral-pool statistics at a date (mirrors Rust ``calculate_pool_stats``).

    Parameters
    ----------
    pool : AssetPool
        Collateral pool to summarise.
    as_of : datetime.date | str
        Date the remaining maturities are measured from, either a date-like
        object or an ISO 8601 string.

    Returns
    -------
    PoolStats
        Weighted-average coupon, spread and maturity, diversity score, obligor
        and industry counts, defaulted share and undrawn commitment.

    Raises
    ------
    ValueError
        If ``as_of`` is not a date, or the pool balances are inconsistent
        (mixed currencies, or both asset rows and representative lines).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit, calculate_pool_stats
    >>> stats = calculate_pool_stats(StructuredCredit.example().pool, "2024-01-01")
    >>> stats.wac, round(stats.weighted_avg_maturity, 1)
    (0.07, 10.0)
    """
    ...
