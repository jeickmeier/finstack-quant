"""
Currency-tagged money bindings from ``finstack-quant-core``.

Provides the :class:`Money` type for representing monetary amounts with
currency tags. Supports arithmetic operations, serialization, and formatting.

Example::

    >>> from finstack_quant.core.money import Money
    >>> m = Money(100.0, "USD")
    >>> m.amount
    100.0
    >>> str(m.currency)
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
    ``amount`` is its interoperable ``float`` view. Use :meth:`from_decimal`
    for an exact ``decimal.Decimal`` amount.

    Parameters
    ----------
    amount : float | int
        Finite monetary amount, converted through its Python ``float`` value
        before being stored as Rust ``Decimal``.
    currency : Currency | str
        ISO-4217 currency (object or alphabetic code string).
    config : FinstackConfig | None
        Optional ingest configuration. When it has an ingest scale override for the currency, the
        amount is rounded once to that scale with the config's selected
        rounding mode; without an override it is kept exactly.

    Raises
    ------
    ValueError
        If *amount* is non-finite or *currency* is invalid, or *config* has
        an ingest scale override for the currency but no rounding mode.
    TypeError
        If *amount* is not a ``float`` or ``int``.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> usd_100 = Money(100.0, "USD")
    >>> usd_100.format_with()
    'USD 100.00'
    >>> usd_100 * 1.5
    Money(150.0, 'USD')
    >>> Money(1234567.891, "USD").format_with(group=",")
    'USD 1,234,567.89'
    >>> Money(300.0, "USD") / Money(100.0, "USD")
    3.0
    """

    def __init__(
        self,
        amount: Union[float, int],
        currency: Union[Currency, str],
        config: Optional[FinstackConfig] = None,
    ) -> None:
        """
        Construct from a ``float``/``int`` amount and a currency.

        Parameters
        ----------
        amount : float | int
            Finite monetary amount (IEEE 754 ``float`` semantics). Use
            :meth:`from_decimal` for an exact ``decimal.Decimal`` amount.
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code string
            (case-insensitive, not trimmed).
        config : FinstackConfig | None
            Optional ingest configuration, applied after the amount is
            admitted exactly. When it has an ingest scale override for the currency, the
            amount is rounded once to that scale with the config's selected
            rounding mode; without an override it is kept exactly.

        Raises
        ------
        ValueError
            If *amount* is non-finite or *currency* is invalid, or *config*
            has an ingest scale override for the currency but no rounding
            mode.
        TypeError
            If *amount* is not a ``float`` or ``int``.
        """
        ...

    @classmethod
    def from_decimal(
        cls,
        amount: Decimal,
        currency: Union[Currency, str],
        config: Optional[FinstackConfig] = None,
    ) -> Money:
        """
        Construct from a ``decimal.Decimal`` without going through ``float``.

        This is the recommended entry point when the caller already holds a
        high-precision value. Unlike the regular ``Money(amount, ccy)``
        constructor's float path, this never rounds through ``f64``; a value
        not exactly representable as a Rust ``Decimal`` (more than 28
        fractional digits) raises ``ValueError``.

        Parameters
        ----------
        amount : decimal.Decimal
            Decimal monetary amount (subclasses accepted).
        currency : Currency | str
            Currency object or ISO-4217 code string.
        config : FinstackConfig | None
            Optional ingest configuration (Rust
            ``Money::from_decimal_with_config``); the rounding never goes
            through ``float``. When it has an ingest scale override for the currency, the
            amount is rounded once to that scale with the config's selected
            rounding mode; without an override it is kept exactly.

        Raises
        ------
        ValueError
            If *amount* is non-finite or not exactly representable as a Rust
            ``Decimal``, *currency* is invalid, or *config* has an ingest
            scale override for the currency but no rounding mode.
        TypeError
            If *amount* is not a ``decimal.Decimal``.

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
        show_currency: bool | None = None,
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
            Number of decimal places, inclusive range ``0`` to ``28``.
            Defaults to the currency's minor units.
        show_currency : bool | None
            Whether to prepend the currency code; ``None`` (the default)
            selects the Rust default, ``True``.
        group : str | None
            Single-character thousands separator (e.g. ``","``); ``None``
            disables grouping.
        rounding : RoundingMode | str | None
            Rounding applied to the displayed value; a ``RoundingMode`` or
            its exact lowercase name. Defaults to the declared presentation
            default, bankers (nearest-even) display rounding.

        Returns
        -------
        str
            Formatted string such as ``"USD 100.00"`` or ``"1,234.57"``.

        Raises
        ------
        ValueError
            If *decimals* exceeds Decimal's scale ceiling of 28,
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

    def convert_at_rate(self, target: Union[Currency, str], rate: Union[Decimal, int, str]) -> Money:
        """
        Convert with an already-resolved exact FX rate.

        The rate is exact (``decimal.Decimal``, ``int`` or decimal ``str``),
        so a contractual rate never passes through ``float``; use
        ``convert_at_rate_f64`` for a ``float`` rate. The multiplication is
        Decimal-backed and no destination minor-unit rounding is applied.

        Parameters
        ----------
        target : Currency or str
            Destination currency as a ``Currency`` object or ISO-4217 code.
        rate : decimal.Decimal, int or str
            Strictly positive rate satisfying ``1 source_currency = rate
            target_currency``; it must already reflect the desired quote side.
            For the same currency it must be exactly ``1``.

        Returns
        -------
        Money
            Amount multiplied by ``rate`` and denominated in ``target``.

        Raises
        ------
        TypeError
            If *rate* is a ``float`` or ``bool`` or another non-exact type.
        ValueError
            If *target* is not a recognized currency, *rate* is not strictly
            positive, a same-currency *rate* is not exactly ``1``, or the
            converted amount overflows Decimal or a nonzero result rounds to
            zero at 28 decimal places.

        """
        ...

    def convert_at_rate_f64(self, target: Union[Currency, str], rate: float) -> Money:
        """
        Convert with an already-resolved ``float`` FX rate.

        The rate is admitted by its shortest round-trip decimal text and then
        converted exactly as ``convert_at_rate``.

        Parameters
        ----------
        target : Currency or str
            Destination currency as a ``Currency`` object or ISO-4217 code.
        rate : float
            Finite, strictly positive rate satisfying ``1 source_currency =
            rate target_currency``; exactly ``1.0`` for the same currency.

        Returns
        -------
        Money
            Amount multiplied by ``rate`` and denominated in ``target``.

        Raises
        ------
        TypeError
            If *rate* is not a ``float`` (``bool`` included).
        ValueError
            If *target* is not a recognized currency, *rate* is non-finite or
            not strictly positive, a same-currency *rate* is not exactly
            ``1.0``, or the converted amount overflows or underflows Decimal.

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
    def __mul__(self, other: Union[float, int, Decimal]) -> Money:
        """Scale by a scalar: exact for ``int``/``Decimal``, shortest round-trip text for ``float``.

        The product is exact when representable, otherwise rounded once,
        half-even, at the greatest scale (at most 28 places) that fits.

        Raises
        ------
        TypeError
            If *other* is a ``bool`` or not a ``float``, ``int`` or ``Decimal``.
        ValueError
            If the product overflows Decimal, a nonzero product rounds to
            zero at 28 decimal places, or a ``float`` scalar is non-finite.

        Returns
        -------
        Money
        """
        ...
    def __rmul__(self, other: Union[float, int, Decimal]) -> Money:
        """Right-multiply by a scalar; identical to ``__mul__``.

        Raises
        ------
        TypeError
            If *other* is a ``bool`` or not a ``float``, ``int`` or ``Decimal``.
        ValueError
            If the product overflows or underflows Decimal, or a ``float``
            scalar is non-finite.

        Returns
        -------
        Money
        """
        ...
    @overload
    def __truediv__(self, other: Money) -> float: ...
    @overload
    def __truediv__(self, other: Union[float, int, Decimal]) -> Money: ...
    def __truediv__(self, other: Union[Money, float, int, Decimal]) -> Union[Money, float]:
        """Divide by a scalar (``-> Money``) or by a same-currency ``Money`` (``-> float`` ratio).

        A scalar divisor is exact for ``int``/``Decimal`` and admitted by its
        shortest round-trip text for ``float``. ``Money / Money`` rounds each
        exact amount once to ``float`` and divides, without a Decimal quotient.

        Raises
        ------
        TypeError
            If *other* is a ``bool`` or not ``Money``, ``float``, ``int`` or
            ``Decimal``.
        ValueError
            ``Validation error: division by zero`` for a zero scalar or zero
            ``Money`` divisor (the Rust error, identical in WASM
            ``checkedDivF64(0)``), a non-finite scalar, a quotient that
            overflows Decimal or rounds a nonzero value to zero, or a currency
            mismatch when dividing by ``Money`` in another currency.

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
    def round(self, scale: int, mode: Union[RoundingMode, str]) -> Money:
        """
        Financially round the exact amount to an explicit scale and mode.

        Delegates to Rust ``Money::round``. There is no default scale or mode,
        and Python's ``round(money)`` is not supported. An explicit round may
        produce zero (``1e-28`` at scale 2 is ``0``); a rounded zero carries
        no sign.

        Parameters
        ----------
        scale : int
            Decimal places in major currency units, in ``0..=28``.
        mode : RoundingMode | str
            Rounding rule at that decimal quantum, as a ``RoundingMode`` or
            its exact lowercase name (``"bankers"``,
            ``"midpoint_away_from_zero"``, ``"toward_zero"``, ``"floor"``,
            ``"ceil"``).

        Returns
        -------
        Money
            The rounded amount in the same currency.

        Raises
        ------
        ValueError
            If *scale* is outside ``0..=28`` or *mode* is an unrecognised
            name.
        TypeError
            If *scale* is not an ``int`` (``bool`` included) or *mode* is
            neither a ``RoundingMode`` nor a ``str``.

        Examples
        --------
        >>> from decimal import Decimal
        >>> from finstack_quant.core.money import Money
        >>> m = Money.from_decimal(Decimal("2.5"), "USD")
        >>> (str(m.round(0, "bankers").amount_decimal), str(m.round(0, "midpoint_away_from_zero").amount_decimal))
        ('2', '3')
        """
        ...
    def __radd__(self, other: Union[Money, float]) -> Money: ...
    def __rsub__(self, other: float) -> Money: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...
