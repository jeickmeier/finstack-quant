"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_asset_backed_facility/facility_terms.rs``."""

from __future__ import annotations

import datetime
from typing import Any

class TermOutSpec:
    """
    Term-out: months after the revolving period ends over which the drawn
    balance is repaid.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermOutSpec
    >>> TermOutSpec(24).months
    24
    """

    def __init__(self, months: int) -> None:
        """
        Construct a term-out.

        Parameters
        ----------
        months : int
            Months after the revolving period (or an earlier amortization
            event) by which the line is repaid.

        Notes
        -----
        This constructor does not raise; any non-negative month count is
        accepted.
        """
        ...

    @staticmethod
    def from_json(json: str) -> TermOutSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TermOutSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        TermOutSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermOutSpec
        >>> try:
        ...     TermOutSpec.from_json("{}")
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
            Canonical JSON.

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
            ``(TermOutSpec.from_json, (json,))``.

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
    def months(self) -> int:
        """
        Term-out length after the revolving period.

        Returns
        -------
        int
            Number of months over which the drawn balance is repaid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AmortizationEvent:
    """
    An event that ends a facility's revolving period early.

    Built through :meth:`date` (a fixed date), :meth:`cumulative_loss` (a
    cumulative-loss threshold as a decimal fraction of the original
    collateral) or
    :meth:`excess_spread` (a trailing three-month excess-spread floor).

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import AmortizationEvent
    >>> AmortizationEvent.cumulative_loss(0.04).max_cumulative_loss
    0.04
    >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).kind
    'date'
    """

    @staticmethod
    def date(date: datetime.date) -> AmortizationEvent:
        """
        The revolving period ends on a fixed date.

        Parameters
        ----------
        date : datetime.date
            Date the event fires (the first payment date at or after it).

        Returns
        -------
        AmortizationEvent
            The dated event.

        Raises
        ------
        ValueError
            If ``date`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).date_value
        datetime.date(2025, 1, 15)
        """
        ...

    @staticmethod
    def cumulative_loss(max_cumulative_loss: float) -> AmortizationEvent:
        """
        The revolving period ends once cumulative losses reach a threshold.

        Parameters
        ----------
        max_cumulative_loss : float
            Cumulative net loss as a decimal fraction of the original
            collateral balance (``0.04`` = 4%), in ``(0, 1]``.

        Returns
        -------
        AmortizationEvent
            The loss event.

        Raises
        ------
        ValueError
            If ``max_cumulative_loss`` is not finite or outside ``(0, 1]``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.cumulative_loss(0.04).kind
        'cumulative_loss'
        """
        ...

    @staticmethod
    def excess_spread(min_excess_spread_3m: float) -> AmortizationEvent:
        """
        The revolving period ends once trailing excess spread falls below a
        floor.

        Parameters
        ----------
        min_excess_spread_3m : float
            Minimum trailing three-month annualized excess spread as a decimal
            (``0.01`` = 1%).

        Returns
        -------
        AmortizationEvent
            The excess-spread event.

        Raises
        ------
        ValueError
            If ``min_excess_spread_3m`` is not finite.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.excess_spread(0.01).min_excess_spread_3m
        0.01
        """
        ...

    @staticmethod
    def from_json(json: str) -> AmortizationEvent:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``AmortizationEvent`` (the shape ``to_json`` writes).

        Returns
        -------
        AmortizationEvent
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> try:
        ...     AmortizationEvent.from_json("{}")
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
            Canonical JSON.

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
            ``(AmortizationEvent.from_json, (json,))``.

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
    def kind(self) -> str:
        """
        Which early-amortization event this is.

        Returns
        -------
        str
            ``"date"``, ``"cumulative_loss"`` or ``"excess_spread"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def date_value(self) -> datetime.date | None:
        """
        Event date of a dated event.

        Returns
        -------
        datetime.date | None
            ``None`` for the other kinds.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def max_cumulative_loss(self) -> float | None:
        """
        Cumulative-loss threshold.

        Returns
        -------
        float | None
            Decimal fraction of the original collateral, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def min_excess_spread_3m(self) -> float | None:
        """
        Excess-spread floor.

        Returns
        -------
        float | None
            Decimal, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
