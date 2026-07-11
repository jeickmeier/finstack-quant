"""Currency-tagged money bindings from ``finstack-quant-core``.

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
"""

from __future__ import annotations

from decimal import Decimal
from typing import Union

from finstack_quant.core.currency import Currency

__all__ = ["Money"]

class Money:
    """A currency-tagged monetary amount.

    Immutable value type combining a floating-point amount with an ISO-4217
    currency. Arithmetic is checked: addition and subtraction require matching
    currencies, and operations that would produce non-finite values are
    rejected.

    Parameters
    ----------
    amount : float
        Finite monetary amount.
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
        """Construct from an amount and a currency.

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
        """Construct from a ``decimal.Decimal``, preserving full precision.

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
        """
        ...

    @classmethod
    def zero(cls, currency: Union[Currency, str]) -> Money:
        """Zero amount in the given currency.

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
        """
        ...

    @property
    def amount(self) -> float:
        """Numeric amount as ``float``.

        Returns
        -------
        float
        """
        ...

    @property
    def amount_decimal(self) -> Decimal:
        """Lossless amount as ``decimal.Decimal``.

        The internal Rust ``Decimal`` is rendered to a string and parsed by
        ``decimal.Decimal``; no ``float`` round-trip occurs.

        Returns
        -------
        decimal.Decimal
        """
        ...

    @property
    def currency(self) -> Currency:
        """Currency tag.

        Returns
        -------
        Currency
        """
        ...

    def format(self, decimals: int | None = None, show_currency: bool = True) -> str:
        """Format with *decimals* places and optional currency prefix.

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
        """
        ...

    def to_json(self) -> str:
        """Serialize to a JSON string.

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
        """Deserialize from a JSON string.

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
        """
        ...

    def to_tuple(self) -> tuple[float, str]:
        """Return ``(amount, currency_code)`` tuple.

        Returns
        -------
        tuple[float, str]
        """
        ...

    def convert_at_rate(self, target: Union[Currency, str], rate: float) -> Money:
        """Convert with an already-resolved positive FX rate.

        The multiplication remains Decimal-backed; no destination minor-unit
        rounding is applied until formatting.
        """
        ...

    @classmethod
    def from_tuple(cls, tup: tuple[float, str]) -> Money:
        """Build from ``(amount, currency_code)`` tuple.

        Parameters
        ----------
        tup : tuple[float, str]
            A two-element tuple of ``(amount, code)``.

        Returns
        -------
        Money

        Raises
        ------
        ValueError
            If the currency code is invalid or the amount is non-finite.
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
