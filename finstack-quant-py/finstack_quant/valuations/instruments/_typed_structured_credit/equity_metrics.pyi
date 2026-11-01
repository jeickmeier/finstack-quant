"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/equity_metrics.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd

class EquityMetrics:
    """
    Residual-class return analytics of one deterministic projection
    (:meth:`StructuredCredit.equity_metrics`'s return value): the XIRR of the
    invested amount against every projected equity distribution, the
    multiple on invested capital, the NAV and the cash-on-cash series. With
    nothing invested (a zero price or balance) ``irr``, ``moic`` and each
    cash-on-cash share are ``None``: each divides by ``invested``.
    ``equity_metrics_field_units`` states the units.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityMetrics
    >>> metrics = EquityMetrics.from_json(
    ...     '{"tranche_id": "E", "currency": "USD", "invested": 100.0, "irr": 0.12,'
    ...     ' "moic": 1.5, "nav_pct": 90.0, "cash_on_cash": [["2025-01-15", 0.1]]}'
    ... )
    >>> metrics.moic, list(metrics.to_dataframe().columns)
    (1.5, ['date', 'cash_on_cash'])
    """

    @staticmethod
    def from_json(json: str) -> EquityMetrics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EquityMetrics`` (the shape ``to_json`` writes).

        Returns
        -------
        EquityMetrics
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityMetrics
        >>> try:
        ...     EquityMetrics.from_json("{}")
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
            JSON-encoded ``EquityMetrics``.

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
            ``(EquityMetrics.from_json, (json,))``.

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
        Cash-on-cash series as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string) and ``cash_on_cash`` (decimal
        share of the invested amount distributed on that date).

        Returns
        -------
        pd.DataFrame
            One row per distribution date.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the residual class.

        Returns
        -------
        str
            The class identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 code of the currency ``invested`` is denominated in.

        Returns
        -------
        str
            The currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def invested(self) -> float:
        """
        Amount invested on the valuation date (balance × purchase price).

        Returns
        -------
        float
            The amount in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def irr(self) -> float | None:
        """
        Annualized XIRR (ACT/365F) of the equity flows as a decimal.

        Returns
        -------
        float | None
            ``None`` when no rate solves: nothing invested or no positive
            distribution. A class that receives little still has a rate, near ``-1.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def moic(self) -> float | None:
        """
        Multiple on invested capital: total distributions over ``invested``, interest and principal.

        Returns
        -------
        float | None
            The multiple; ``None`` when nothing is invested. A class that receives
            nothing on a positive investment has a true ``0.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def nav_pct(self) -> float:
        """
        Distributions discounted on the deal's discount curve to the valuation date, as a percent of the equity's current balance.

        Returns
        -------
        float
            The NAV percent (``62.5`` is 62.5%); it does not depend on the purchase price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cash_on_cash(self) -> list[tuple[datetime.date, float | None]]:
        """
        Each distribution as a decimal share of ``invested``, not annualized.

        Returns
        -------
        list[tuple[datetime.date, float | None]]
            ``(date, share)`` pairs; each share is ``None`` when nothing is invested.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
