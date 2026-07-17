"""
Currency-tagged money bindings from ``finstack-quant-core``.

Provides the :class:`Money` type for representing monetary amounts with
currency tags. Supports arithmetic operations, serialization, and formatting.

Example::

    >>> from finstack_quant.core.money import Money
    >>> m = Money(100.0, "USD")
    >>> m.amount
    100.0
    >>> m.currency.code
    'USD'
    >>> m + Money(50.0, "USD")
    Money(150.0, 'USD')

Examples
--------
>>> import finstack_quant.core.money as money
>>> money.__name__
'finstack_quant.core.money'
"""

from __future__ import annotations

from decimal import Decimal
from typing import Union

from finstack_quant.core.currency import Currency

__all__ = ["Money"]

class Money:
    """
    A currency-tagged monetary amount.

    Immutable, Decimal-backed value type combining a precision-preserving
    monetary amount with an ISO-4217 currency. Arithmetic is checked: addition
    and subtraction require matching currencies, and invalid/non-finite inputs
    are rejected. ``amount_decimal`` exposes the stored amount losslessly;
    ``amount`` is its interoperable ``float`` view.

    Parameters
    ----------
    amount : decimal.Decimal | float | int
        Finite monetary amount. ``Decimal`` preserves its full decimal
        precision. ``float`` and ``int`` inputs are converted through their
        finite Python ``float`` value before being stored as Rust ``Decimal``.
    currency : Currency | str
        ISO-4217 currency (object or alphabetic code string).

    Raises
    ------
    ValueError
        If *amount* is not finite or *currency* is invalid.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> usd_100 = Money(100.0, "USD")
    >>> usd_100.format()
    'USD 100.00'
    >>> usd_100 * 1.5
    Money(150.0, 'USD')
    """

    def __init__(self, amount: Union[float, int, Decimal], currency: Union[Currency, str]) -> None:
        """
        Construct from an amount and a currency.

        Parameters
        ----------
        amount : float | int | decimal.Decimal
            Finite monetary amount. ``Decimal`` inputs preserve full precision
            (no IEEE 754 round-trip); ``float``/``int`` follow standard IEEE 754
            semantics. Use ``Decimal`` when exact decimal precision matters.
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code string.

        Raises
        ------
        ValueError
            If *amount* is not finite, cannot be parsed as a Decimal, or
            *currency* is invalid.
        """
        ...

    @classmethod
    def from_decimal(cls, amount: Decimal, currency: Union[Currency, str]) -> Money:
        """
        Construct from a ``decimal.Decimal``, preserving full precision.

        This is the recommended entry point when the caller already holds a
        high-precision value. Unlike the regular ``Money(amount, ccy)``
        constructor's float path, this never rounds through ``f64``.

        Parameters
        ----------
        amount : decimal.Decimal
            Decimal monetary amount.
        currency : Currency | str
            Currency object or ISO-4217 code string.

        Raises
        ------
        ValueError
            If *amount* cannot be parsed or *currency* is invalid.

        Returns
        -------
        Money
            Result of from decimal for this `Money` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> callable(Money.from_decimal)
        True
        """
        ...

    @classmethod
    def zero(cls, currency: Union[Currency, str]) -> Money:
        """
        Zero amount in the given currency.

        Parameters
        ----------
        currency : Currency | str
            Currency object or ISO-4217 code string.

        Returns
        -------
        Money
            A zero-value Money in the specified currency.

        Raises
        ------
        ValueError
            If *currency* is unrecognised.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> callable(Money.zero)
        True
        """
        ...

    @property
    def amount(self) -> float:
        """
        Numeric amount as ``float``.

        Returns
        -------
        float
            The amount exposed by this `Money`.
        """
        ...

    @property
    def amount_decimal(self) -> Decimal:
        """
        Lossless amount as ``decimal.Decimal``.

        The internal Rust ``Decimal`` is rendered to a string and parsed by
        ``decimal.Decimal``; no ``float`` round-trip occurs.

        Returns
        -------
        decimal.Decimal
            The amount decimal exposed by this `Money`.
        """
        ...

    @property
    def currency(self) -> Currency:
        """
        Return the currency for `Money`.
        Currency tag.

        Returns
        -------
        Currency
            The currency exposed by this `Money`.
        """
        ...

    def format(self, decimals: int | None = None, show_currency: bool = True) -> str:
        """
        Format with *decimals* places and optional currency prefix.

        When *decimals* is omitted the currency's ISO minor-unit precision
        is used.

        Parameters
        ----------
        decimals : int | None
            Number of decimal places. Defaults to the currency's minor units.
        show_currency : bool
            Whether to prepend the currency code (default ``True``).

        Returns
        -------
        str
            Formatted string such as ``"USD 100.00"``.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON representation.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Money:
        """
        Deserialize from a JSON string.

        Parameters
        ----------
        json : str
            JSON payload.

        Returns
        -------
        Money
            The deserialized money value.

        Raises
        ------
        ValueError
            If *json* is not valid.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> callable(Money.from_json)
        True
        """
        ...

    def to_tuple(self) -> tuple[float, str]:
        """
        Return ``(amount, currency_code)`` tuple.

        Returns
        -------
        tuple[float, str]
            Result of to tuple for this `Money` in the annotated representation.
        """
        ...

    def convert_at_rate(self, target: Union[Currency, str], rate: float) -> Money:
        """
        Convert with an already-resolved positive FX rate.

        The multiplication remains Decimal-backed; no destination minor-unit
        rounding is applied until formatting.

        Parameters
        ----------
        target : Currency or str
            Destination currency as a ``Currency`` object or ISO-4217 code.
        rate : float
            Positive conversion rate satisfying ``1 source_currency = rate
            target_currency``; it must already reflect the desired quote side.

        Returns
        -------
        Money
            Result of convert at rate for this `Money` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @classmethod
    def from_tuple(cls, tup: tuple[float, str]) -> Money:
        """
        Build from ``(amount, currency_code)`` tuple.

        Parameters
        ----------
        tup : tuple[float, str]
            A two-element tuple of ``(amount, code)``.

        Returns
        -------
        Money

            Result of from tuple for this `Money` in the annotated representation.
        Raises
        ------
        ValueError
            If the currency code is invalid or the amount is non-finite.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> callable(Money.from_tuple)
        True
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Money) -> bool: ...
    def __le__(self, other: Money) -> bool: ...
    def __gt__(self, other: Money) -> bool: ...
    def __ge__(self, other: Money) -> bool: ...
    def __add__(self, other: Money) -> Money: ...
    def __sub__(self, other: Money) -> Money: ...
    def __mul__(self, other: float) -> Money: ...
    def __rmul__(self, other: float) -> Money: ...
    def __truediv__(self, other: float) -> Money: ...
    def __neg__(self) -> Money: ...
    def __radd__(self, other: Union[Money, float]) -> Money: ...
    def __rsub__(self, other: float) -> Money: ...
