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
    Money(150, 'USD')

Examples
--------
>>> from finstack_quant.core.money import Money
>>> Money(25.0, "USD").format_with()
'USD 25.00'

"""

from __future__ import annotations

from decimal import Decimal
from typing import Optional, Union, overload

from finstack_quant.core.config import FinstackConfig, RoundingMode
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
    amount : decimal.Decimal | float | int | str
        Finite monetary amount. ``Decimal`` and decimal strings
        (``"1234.56"``) are parsed exactly. ``float`` and ``int`` inputs are
        converted through their finite Python ``float`` value before being
        stored as Rust ``Decimal``.
    currency : Currency | str
        ISO-4217 currency (object or alphabetic code string).
    config : FinstackConfig | None
        When given, ``float``/``int`` amounts are rounded on ingest using the
        config's rounding mode and per-currency ingest scale.

    Raises
    ------
    ValueError
        If *amount* is not finite / not parsable or *currency* is invalid.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> usd_100 = Money(100.0, "USD")
    >>> usd_100.format_with()
    'USD 100.00'
    >>> usd_100 * 1.5
    Money(150.0, 'USD')
    >>> Money("1234567.891", "USD").format_with(group=",")
    'USD 1,234,567.89'
    >>> Money(300.0, "USD") / Money(100.0, "USD")
    3.0
    """

    def __init__(
        self,
        amount: Union[float, int, Decimal, str],
        currency: Union[Currency, str],
        config: Optional[FinstackConfig] = None,
    ) -> None:
        """
        Construct from an amount and a currency.

        Parameters
        ----------
        amount : float | int | decimal.Decimal | str
            Finite monetary amount. ``Decimal`` and ``str`` inputs preserve
            full precision (no IEEE 754 round-trip); ``float``/``int`` follow
            standard IEEE 754 semantics.
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code string.
        config : FinstackConfig | None
            Optional config whose rounding mode and ingest scale are applied
            to ``float``/``int`` amounts.

        Raises
        ------
        ValueError
            If *amount* is not finite, cannot be parsed as a Decimal, or
            *currency* is invalid.
        TypeError
            If *amount* is not a number, ``Decimal`` or ``str``.
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
            Decimal-backed amount in ``currency`` without an intermediate binary float.

        Examples
        --------
        >>> from decimal import Decimal
        >>> from finstack_quant.core.money import Money
        >>> Money.from_decimal(Decimal("1.25"), "USD").format_with()
        'USD 1.25'

        """
        ...

    @classmethod
    def from_decimal_str(cls, amount: str, currency: Union[Currency, str]) -> Money:
        """
        Construct from exact decimal text, rejecting inexact amounts.

        Unlike general ``Money`` construction or JSON deserialization, this
        entry point never routes the amount through ``float`` or tolerates
        lossy re-rendering: the text must be exactly representable as a Rust
        ``Decimal`` (96-bit mantissa, at most 28 fractional digits). For
        scientific input the mantissa itself must be exact before the
        exponent is applied. The currency is a tag only; no FX conversion is
        performed.

        Parameters
        ----------
        amount : str
            Exact decimal amount text in major currency units, in fixed-point
            (``"1234.56"``) or scientific (``"1.2345e3"``) notation.
        currency : Currency | str
            Currency object or ISO-4217 code string.

        Returns
        -------
        Money
            Decimal-backed amount equal to the supplied text exactly.

        Raises
        ------
        ValueError
            If *amount* is malformed, non-finite, requires more precision
            than ``Decimal`` can hold, or underflows its supported scale;
            or if *currency* is invalid.
        TypeError
            If *amount* or *currency* has an unsupported host type.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> Money.from_decimal_str("1.245", "USD").amount_decimal
        Decimal('1.245')

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
        >>> Money.zero("EUR").amount
        0.0

        """
        ...

    @property
    def amount(self) -> float:
        """
        Numeric amount as ``float``.

        Returns
        -------
        float
            Numeric amount as ``float``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
            Lossless amount as ``decimal.Decimal``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def currency(self) -> Currency:
        """
        ISO-4217 currency of this money amount.

        Returns
        -------
        Currency
            Currency tag stored with the decimal amount.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def format_with(
        self,
        decimals: int | None = None,
        show_currency: bool = True,
        group: str | None = None,
        rounding: Union[RoundingMode, str, None] = None,
    ) -> str:
        """
        Format the amount with optional currency prefix, grouping and rounding.

        Delegates to the canonical Rust ``Money::format_with``. When
        *decimals* is omitted the currency's ISO minor-unit precision is used.

        Parameters
        ----------
        decimals : int | None
            Number of decimal places, inclusive range ``0`` to ``1_000_000``.
            Defaults to the currency's minor units.
        show_currency : bool
            Whether to prepend the currency code (default ``True``).
        group : str | None
            Single-character thousands separator (e.g. ``","``); ``None``
            disables grouping.
        rounding : RoundingMode | str | None
            Rounding applied to the displayed value; a ``RoundingMode`` or
            its exact lowercase name. Defaults to bankers rounding.

        Returns
        -------
        str
            Formatted string such as ``"USD 100.00"`` or ``"1,234.57"``.

        Raises
        ------
        ValueError
            If *decimals* exceeds the native precision bound of 1,000,000,
            *group* is not a single character, or *rounding* is an
            unrecognised name.
        OverflowError
            If *decimals* is a negative integer or too large to fit the
            native ``usize`` precision type.
        TypeError
            If *decimals*, *show_currency*, or *group* has an unsupported
            host type (for example a non-integer precision).
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
        >>> money = Money(25.0, "USD")
        >>> Money.from_json(money.to_json()).format_with()
        'USD 25.00'

        """
        ...

    def to_tuple(self) -> tuple[float, str]:
        """
        Return ``(amount, currency_code)`` tuple.

        Returns
        -------
        tuple[float, str]
            Binary-float amount, which may lose Decimal precision, and the
            ISO-4217 alphabetic currency code.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
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
            Decimal-backed amount multiplied by ``rate`` and denominated in
            ``target``; same-currency conversion returns this amount unchanged
            without validating ``rate``.

        Raises
        ------
        ValueError
            If *target* is not a recognized currency or, for a different
            currency, *rate* is non-finite or not strictly positive or the
            converted amount exceeds Decimal's representable range.

        """
        ...

    @classmethod
    def from_tuple(cls, tup: tuple[Union[float, int, Decimal, str], Union[Currency, str]]) -> Money:
        """
        Build from an ``(amount, currency)`` tuple.

        Parameters
        ----------
        tup : tuple
            A two-element tuple of ``(amount, currency)``; ``amount`` accepts
            the same types as the constructor (``float``, ``int``,
            ``Decimal``, ``str``) and ``currency`` a ``Currency`` or ISO code.

        Returns
        -------
        Money
            Money built through the same ingest path as the constructor
            (``Decimal``/``str`` exact, numbers via binary float).

        Raises
        ------
        ValueError
            If the tuple does not have two elements, the currency is invalid
            or the amount is non-finite / unparsable.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> Money.from_tuple((25.0, "EUR")).currency.code
        'EUR'

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Money) -> bool:
        """Order two amounts of the same currency.

        Raises
        ------
        ValueError
            ``Currency mismatch: expected X, got Y`` when currencies differ.

        Returns
        -------
        bool
        """
        ...
    def __le__(self, other: Money) -> bool: ...
    def __gt__(self, other: Money) -> bool: ...
    def __ge__(self, other: Money) -> bool: ...
    def __add__(self, other: Money) -> Money: ...
    def __sub__(self, other: Money) -> Money: ...
    def __mul__(self, other: float) -> Money: ...
    def __rmul__(self, other: float) -> Money: ...
    @overload
    def __truediv__(self, other: Money) -> float: ...
    @overload
    def __truediv__(self, other: float) -> Money: ...
    def __truediv__(self, other: Union[Money, float]) -> Union[Money, float]:
        """Divide by a scalar (``-> Money``) or by a same-currency ``Money`` (``-> float`` ratio).

        Raises
        ------
        ValueError
            ``division by zero`` for a zero divisor, or a currency mismatch
            when dividing by ``Money`` in another currency.

        Returns
        -------
        Money | float
        """
        ...
    def __neg__(self) -> Money: ...
    def __abs__(self) -> Money:
        """Absolute value in the same currency.

        Returns
        -------
        Money
        """
        ...
    def __float__(self) -> float:
        """``float(money)`` — the ``amount`` view.

        Returns
        -------
        float
        """
        ...
    def __round__(self, ndigits: int | None = None) -> Money:
        """Bankers-round the amount to *ndigits* places (default: currency minor units).

        Raises
        ------
        ValueError
            If *ndigits* is negative.

        Returns
        -------
        Money
        """
        ...
    def __radd__(self, other: Union[Money, float]) -> Money: ...
    def __rsub__(self, other: float) -> Money: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...
