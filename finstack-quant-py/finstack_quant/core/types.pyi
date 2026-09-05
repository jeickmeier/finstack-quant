"""
Core finstack_quant types: rates, identifiers, credit ratings, and attributes.

Provides typed wrappers for financial primitives used throughout the
``finstack_quant`` library.

Example::

    >>> from finstack_quant.core.types import Rate, Bps, Percentage
    >>> r = Rate(0.05)
    >>> r.as_percent
    5.0
    >>> r.as_bp
    500
    >>> Bps(250).as_decimal
    0.025
    >>> Percentage(12.5).as_decimal
    0.125

Examples
--------
>>> from finstack_quant.core.types import Rate
>>> Rate("5%") == Rate.from_percent(5.0) == Rate("500bp")
True

"""

from __future__ import annotations

from typing import Optional, Union

__all__ = [
    "Rate",
    "Bps",
    "Percentage",
    "CreditRating",
    "CurveId",
    "InstrumentId",
    "Attributes",
]

class Rate:
    """
    A financial rate expressed as a decimal fraction (``0.05`` is 5% / 500 bp).

    Immutable, hashable, ordered value type. Supports checked arithmetic
    (``+``/``-`` with ``Rate`` or ``Bps``, ``*``/``/`` by ``float``) and
    conversion between decimal, percent and basis-point representations.
    Serializes as a bare JSON number and is picklable.

    Parameters
    ----------
    value : float | str
        Decimal fraction (``0.05``) or a quote string ``"5%"``, ``"25bp"``,
        ``"25bps"`` or ``"0.05"`` (units case-insensitive; fractional bp such
        as ``"62.5bp"`` accepted).

    Raises
    ------
    ValueError
        If *value* is not finite or the string cannot be parsed.

    Examples
    --------
    >>> from finstack_quant.core.types import Rate, Bps
    >>> Rate("5%") == Rate(0.05)
    True
    >>> (Rate(0.05) + Bps(25)).as_bp
    525
    """

    ZERO: Rate
    """Zero rate (0% as a decimal rate)."""

    def __init__(self, value: Union[float, str]) -> None:
        """
        Construct a rate from a decimal fraction or a quote string.

        Parameters
        ----------
        value : float | str
            Decimal fraction (``0.05`` for 5%) or ``"5%"`` / ``"25bp"`` /
            ``"25bps"`` / ``"0.05"``.

        Raises
        ------
        ValueError
            If *value* is not finite or the string cannot be parsed.
        TypeError
            If *value* is neither a number nor a string.
        """
        ...

    @classmethod
    def from_percent(cls, percent: float) -> Rate:
        """
        Build from a percent value.

        Parameters
        ----------
        percent : float
            Rate in percent (e.g. ``5.0`` for 5%).

        Returns
        -------
        Rate
            Rate stored as ``percent / 100`` in decimal-rate form.

        Raises
        ------
        ValueError
            If *percent* is not finite.

        Examples
        --------
        >>> from finstack_quant.core.types import Rate
        >>> Rate.from_percent(5.0).as_decimal
        0.05

        """
        ...

    @classmethod
    def from_bp(cls, bp: int) -> Rate:
        """
        Build from an integer basis-point amount.

        Parameters
        ----------
        bp : int
            Basis points (e.g. ``500`` for 5%).

        Returns
        -------
        Rate
            Rate stored as ``bp / 10_000`` in decimal-rate form.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.types import Rate
        >>> Rate.from_bp(25).as_decimal
        0.0025
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Rate:
        """
        Deserialize from JSON (a bare decimal number such as ``0.05``).

        Parameters
        ----------
        json : str
            JSON number text.

        Returns
        -------
        Rate
            Parsed rate.

        Raises
        ------
        ValueError
            If *json* is not a finite number.

        Examples
        --------
        >>> from finstack_quant.core.types import Rate
        >>> Rate.from_json(Rate(0.05).to_json()).as_bp
        500
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (the bare decimal number).

        Returns
        -------
        str
            JSON number text such as ``"0.05"``.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a finite rate).
        """
        ...

    @property
    def as_decimal(self) -> float:
        """
        Rate as a decimal fraction.

        Returns
        -------
        float
            Rate as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_percent(self) -> float:
        """
        Rate as a percent value.

        Returns
        -------
        float
            Rate as a percent value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_bp(self) -> int:
        """
        Rate rounded to the nearest basis point.

        Returns
        -------
        int
            Rate rounded to the nearest basis point.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_basis_points(self) -> Bps:
        """
        Rate as a ``Bps`` value, rounded to the nearest whole basis point.

        Returns
        -------
        Bps
            Whole-basis-point quote.

        Notes
        -----
        This accessor does not raise; sub-bp precision is rounded away.
        """
        ...

    @property
    def as_percentage(self) -> Percentage:
        """
        Rate as a ``Percentage`` value.

        Returns
        -------
        Percentage
            ``Percentage(rate * 100)``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def abs(self) -> Rate:
        """
        Magnitude of the rate, discarding its sign (decimal units preserved).

        Returns
        -------
        Rate
            Non-negative rate of the same magnitude.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_zero(self) -> bool:
        """
        Return whether the rate is exactly zero.

        Returns
        -------
        bool
            ``True`` for ``Rate(0.0)``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_positive(self) -> bool:
        """
        Return whether the rate is strictly positive.

        Returns
        -------
        bool
            ``True`` when ``as_decimal > 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_negative(self) -> bool:
        """
        Return whether the rate is strictly negative.

        Returns
        -------
        bool
            ``True`` when ``as_decimal < 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Rate) -> bool: ...
    def __le__(self, other: Rate) -> bool: ...
    def __gt__(self, other: Rate) -> bool: ...
    def __ge__(self, other: Rate) -> bool: ...
    def __add__(self, other: Union[Rate, Bps]) -> Rate: ...
    def __sub__(self, other: Union[Rate, Bps]) -> Rate: ...
    def __mul__(self, rhs: float) -> Rate: ...
    def __rmul__(self, lhs: float) -> Rate: ...
    def __truediv__(self, rhs: float) -> Rate: ...
    def __neg__(self) -> Rate: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class Bps:
    """
    A value measured in whole basis points (1 bp = 0.0001 = 0.01%).

    Immutable, hashable, ordered value type. Integer-valued internally;
    fractional input is rejected rather than rounded. Serializes as a bare
    JSON integer and is picklable.

    Parameters
    ----------
    bp : float
        Whole basis-point value.

    Raises
    ------
    ValueError
        If *bp* is not finite or not a whole number of basis points. Use a
        decimal ``Rate`` (``Rate("62.5bp")``) for sub-bp precision.

    Examples
    --------
    >>> from finstack_quant.core.types import Bps
    >>> Bps(250).as_decimal
    0.025
    >>> Bps(250).as_percent
    2.5
    """

    ZERO: Bps
    """Zero basis points."""

    def __init__(self, bp: float) -> None:
        """
        Construct from a whole basis-point value.

        Parameters
        ----------
        bp : float
            Whole basis-point value.

        Raises
        ------
        ValueError
            If *bp* is not finite or not a whole number of basis points.

        Examples
        --------
        >>> from finstack_quant.core.types import Bps
        >>> Bps(250).as_decimal
        0.025
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Bps:
        """
        Deserialize from JSON (a bare integer such as ``250``).

        Parameters
        ----------
        json : str
            JSON integer text.

        Returns
        -------
        Bps
            Parsed basis-point value.

        Raises
        ------
        ValueError
            If *json* is not an integer.

        Examples
        --------
        >>> from finstack_quant.core.types import Bps
        >>> Bps.from_json("250") == Bps(250)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (the bare integer basis-point quote).

        Returns
        -------
        str
            JSON integer text such as ``"250"``.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid value).
        """
        ...

    @property
    def as_decimal(self) -> float:
        """
        Value as a decimal fraction.

        Returns
        -------
        float
            Value as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.core.types import Bps
        >>> Bps(250).as_decimal
        0.025
        """
        ...

    @property
    def as_bp(self) -> int:
        """
        Value as whole basis points.

        Returns
        -------
        int
            Value as whole basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.core.types import Bps
        >>> Bps(250).as_bp
        250
        """
        ...

    @property
    def as_percent(self) -> float:
        """
        Value in percent units (``Bps(250).as_percent == 2.5``).

        Returns
        -------
        float
            The same spread expressed in percent, i.e. basis points divided
            by 100 (100 bp is ``1.0``).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def as_rate(self) -> Rate:
        """
        Value as a decimal ``Rate``.

        Returns
        -------
        Rate
            ``Rate(bp / 10_000)``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def as_percentage(self) -> Percentage:
        """
        Value as a ``Percentage``.

        Returns
        -------
        Percentage
            ``Percentage(bp / 100)``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def abs(self) -> Bps:
        """
        Magnitude of the spread, discarding its sign (basis points preserved).

        Returns
        -------
        Bps
            Non-negative basis points of the same magnitude.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_zero(self) -> bool:
        """
        Return whether exactly zero basis points.

        Returns
        -------
        bool
            ``True`` for ``Bps(0)``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_positive(self) -> bool:
        """
        Return whether strictly positive.

        Returns
        -------
        bool
            ``True`` when ``as_bp > 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_negative(self) -> bool:
        """
        Return whether strictly negative.

        Returns
        -------
        bool
            ``True`` when ``as_bp < 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Bps) -> bool: ...
    def __le__(self, other: Bps) -> bool: ...
    def __gt__(self, other: Bps) -> bool: ...
    def __ge__(self, other: Bps) -> bool: ...
    def __add__(self, other: Bps) -> Bps: ...
    def __sub__(self, other: Bps) -> Bps: ...
    def __mul__(self, rhs: int) -> Bps: ...
    def __rmul__(self, lhs: int) -> Bps: ...
    def __truediv__(self, rhs: int) -> Bps: ...
    def __neg__(self) -> Bps: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class Percentage:
    """
    A percentage value (``12.5`` means 12.5%).

    Immutable, hashable, ordered value type with checked arithmetic
    (``+``/``-`` with ``Percentage``, ``*``/``/`` by ``float``) and
    conversions to ``Rate`` and ``Bps``. Serializes as a bare JSON number and
    is picklable.

    Parameters
    ----------
    percent : float
        Percentage value (e.g. ``12.5`` for 12.5%).

    Raises
    ------
    ValueError
        If *percent* is not finite.

    Examples
    --------
    >>> from finstack_quant.core.types import Percentage
    >>> Percentage(12.5).as_decimal
    0.125
    >>> (Percentage(10.0) + Percentage(2.5)).as_percent
    12.5
    """

    ZERO: Percentage
    """Zero percent."""

    def __init__(self, percent: float) -> None:
        """
        Construct from a percent value.

        Parameters
        ----------
        percent : float
            Percentage value (e.g. ``12.5`` for 12.5%).

        Raises
        ------
        ValueError
            If *percent* is not finite.

        Examples
        --------
        >>> from finstack_quant.core.types import Percentage
        >>> Percentage(12.5).as_decimal
        0.125
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Percentage:
        """
        Deserialize from JSON (a bare percent number such as ``12.5``).

        Parameters
        ----------
        json : str
            JSON number text in percent units.

        Returns
        -------
        Percentage
            Parsed percentage.

        Raises
        ------
        ValueError
            If *json* is not a finite number.

        Examples
        --------
        >>> from finstack_quant.core.types import Percentage
        >>> Percentage.from_json("12.5").as_decimal
        0.125
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (the bare percent number).

        Returns
        -------
        str
            JSON number text such as ``"12.5"``.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a finite value).
        """
        ...

    @property
    def as_decimal(self) -> float:
        """
        Value as a decimal fraction.

        Returns
        -------
        float
            Value as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.core.types import Percentage
        >>> Percentage(12.5).as_decimal
        0.125
        """
        ...

    @property
    def as_percent(self) -> float:
        """
        Value expressed in percent units (``5.0`` means five percent).

        Returns
        -------
        float
            Value expressed in percent units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.core.types import Percentage
        >>> Percentage(12.5).as_percent
        12.5
        """
        ...

    @property
    def as_bp(self) -> int:
        """
        Value rounded to the nearest whole basis point.

        Returns
        -------
        int
            ``round(percent * 100)``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def as_rate(self) -> Rate:
        """
        Value as a decimal ``Rate``.

        Returns
        -------
        Rate
            ``Rate(percent / 100)``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def as_basis_points(self) -> Bps:
        """
        Value as a ``Bps`` value, rounded to the nearest whole basis point.

        Returns
        -------
        Bps
            Whole-basis-point quote.

        Notes
        -----
        This accessor does not raise; sub-bp precision is rounded away.
        """
        ...

    def abs(self) -> Percentage:
        """
        Magnitude of the percentage, discarding its sign (percent units kept).

        Returns
        -------
        Percentage
            Non-negative percentage of the same magnitude.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_zero(self) -> bool:
        """
        Return whether exactly zero percent.

        Returns
        -------
        bool
            ``True`` for ``Percentage(0.0)``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_positive(self) -> bool:
        """
        Return whether strictly positive.

        Returns
        -------
        bool
            ``True`` when ``as_percent > 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_negative(self) -> bool:
        """
        Return whether strictly negative.

        Returns
        -------
        bool
            ``True`` when ``as_percent < 0``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Percentage) -> bool: ...
    def __le__(self, other: Percentage) -> bool: ...
    def __gt__(self, other: Percentage) -> bool: ...
    def __ge__(self, other: Percentage) -> bool: ...
    def __add__(self, other: Percentage) -> Percentage: ...
    def __sub__(self, other: Percentage) -> Percentage: ...
    def __mul__(self, rhs: float) -> Percentage: ...
    def __rmul__(self, lhs: float) -> Percentage: ...
    def __truediv__(self, rhs: float) -> Percentage: ...
    def __neg__(self) -> Percentage: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class CreditRating:
    """
    Standardised credit rating category on the 23-step S&P/Fitch scale.

    Immutable, hashable, ordered enum-style type with class attributes for
    each rating level. Ordering follows credit quality, so a *smaller* rating
    is a *stronger* credit: ``AAA < AA+ < ... < CCC- < CC < C < NR < D``.
    ``NR`` (not rated) is placed between ``C`` and ``D``; it is neither
    investment nor speculative grade. Notched ratings (``"BBB+"``, ``"Baa1"``)
    preserve their notch-level precision. Compares equal to a rating string
    (``CreditRating.BBB == "BBB"``), but ``hash(CreditRating.BBB) !=
    hash("BBB")``. Serializes as the quoted S&P label and is picklable.

    Parameters
    ----------
    name : str
        Rating string in S&P/Fitch or Moody's notation, case-insensitive
        (``"BBB+"``, ``"Baa1"``, ``"nr"``).

    Raises
    ------
    ValueError
        If *name* is not a recognised rating.

    Examples
    --------
    >>> from finstack_quant.core.types import CreditRating
    >>> CreditRating("Baa1") == CreditRating.BBB_PLUS
    True
    >>> CreditRating.BBB.notches_to(CreditRating.BB)
    3
    >>> CreditRating.AAA < CreditRating.D
    True
    """

    AAA: CreditRating
    """Highest quality rating."""
    AA_PLUS: CreditRating
    """AA+ / Aa1."""
    AA: CreditRating
    """AA category."""
    AA_MINUS: CreditRating
    """AA- / Aa3."""
    A_PLUS: CreditRating
    """A+ / A1."""
    A: CreditRating
    """Single-A category."""
    A_MINUS: CreditRating
    """A- / A3."""
    BBB_PLUS: CreditRating
    """BBB+ / Baa1."""
    BBB: CreditRating
    """BBB category."""
    BBB_MINUS: CreditRating
    """BBB- / Baa3."""
    BB_PLUS: CreditRating
    """BB+ / Ba1."""
    BB: CreditRating
    """BB category."""
    BB_MINUS: CreditRating
    """BB- / Ba3."""
    B_PLUS: CreditRating
    """B+ / B1."""
    B: CreditRating
    """B category."""
    B_MINUS: CreditRating
    """B- / B3."""
    CCC_PLUS: CreditRating
    """CCC+ / Caa1."""
    CCC: CreditRating
    """CCC category."""
    CCC_MINUS: CreditRating
    """CCC- / Caa3."""
    CC: CreditRating
    """CC category."""
    C: CreditRating
    """C category."""
    D: CreditRating
    """Default rating."""
    NR: CreditRating
    """Not rated (ordered between C and D)."""

    def __init__(self, name: str) -> None:
        """
        Parse a rating string; equivalent to :meth:`from_name`.

        Parameters
        ----------
        name : str
            Rating string (e.g. ``"BBB"``, ``"bbb+"``, ``"Baa1"``).

        Raises
        ------
        ValueError
            If *name* cannot be parsed.
        """
        ...

    @classmethod
    def from_name(cls, name: str) -> CreditRating:
        """
        Parse a rating string case-insensitively while preserving notches.

        Parameters
        ----------
        name : str
            Rating string (e.g. ``"BBB"``, ``"bbb+"``, ``"Baa1"``).

        Returns
        -------
        CreditRating
            Canonical S&P/Fitch grade after case normalization, agency-alias
            mapping, and notch preservation.

        Raises
        ------
        ValueError
            If *name* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.types import CreditRating
        >>> CreditRating.from_name("bbb+").name
        'BBB+'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CreditRating:
        """
        Deserialize from JSON (a quoted S&P/Fitch label such as ``"BBB+"``).

        Parameters
        ----------
        json : str
            JSON string literal.

        Returns
        -------
        CreditRating
            Parsed rating.

        Raises
        ------
        ValueError
            If *json* is not a recognised rating label.

        Examples
        --------
        >>> from finstack_quant.core.types import CreditRating
        >>> CreditRating.from_json('"BBB+"') == CreditRating.BBB_PLUS
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (the quoted S&P/Fitch label).

        Returns
        -------
        str
            JSON string literal such as ``'"BBB+"'``.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid rating).
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical S&P/Fitch-style rating name (e.g. ``"BBB-"``).

        Returns
        -------
        str
            Canonical S&P/Fitch-style rating name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.core.types import CreditRating
        >>> CreditRating.AAA.name
        'AAA'
        """
        ...

    def is_investment_grade(self) -> bool:
        """
        Return whether the rating is BBB- or better.

        Returns
        -------
        bool
            ``True`` for AAA through BBB-.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_speculative_grade(self) -> bool:
        """
        Return whether the rating is below BBB- (``NR`` is neither grade).

        Returns
        -------
        bool
            ``True`` for BB+ through D, ``False`` for NR.

        Notes
        -----
        This method does not raise.
        """
        ...

    def is_default(self) -> bool:
        """
        Return whether the rating is ``D``.

        Returns
        -------
        bool
            ``True`` only for ``CreditRating.D``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_moodys_string(self) -> str:
        """
        Moody's-style label for this rating.

        Returns
        -------
        str
            e.g. ``"Baa1"`` for BBB+; ``"D"`` and ``"NR"`` are unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    def notches_to(self, other: Union[CreditRating, str]) -> int:
        """
        Signed notch distance from this rating to *other*.

        Parameters
        ----------
        other : CreditRating | str
            Rating to measure against (a rating string is parsed).

        Returns
        -------
        int
            Positive when *other* is weaker, negative when stronger, zero when
            equal; ``a.notches_to(b) == -b.notches_to(a)``. ``NR`` counts as
            one notch below ``C``.

        Raises
        ------
        ValueError
            If *other* is a string that is not a recognised rating.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: Union[CreditRating, str]) -> bool: ...
    def __le__(self, other: Union[CreditRating, str]) -> bool: ...
    def __gt__(self, other: Union[CreditRating, str]) -> bool: ...
    def __ge__(self, other: Union[CreditRating, str]) -> bool: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class CurveId:
    """
    A unique identifier for a market data curve.

    Immutable, hashable, lexicographically ordered string wrapper. Empty
    identifiers are accepted (see :meth:`is_empty`). Serializes as a quoted
    JSON string and is picklable.

    Parameters
    ----------
    value : str
        Curve identifier string, stored verbatim.

    Examples
    --------
    >>> from finstack_quant.core.types import CurveId
    >>> CurveId("USD-OIS").as_str()
    'USD-OIS'
    >>> len(CurveId("USD-OIS"))
    7
    """

    def __init__(self, value: str) -> None:
        """
        Create a curve identifier from its string value.

        Parameters
        ----------
        value : str
            Curve identifier; may be empty.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.core.types import CurveId
        >>> CurveId("USD-OIS").as_str()
        'USD-OIS'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CurveId:
        """
        Deserialize from JSON (a quoted string).

        Parameters
        ----------
        json : str
            JSON string literal.

        Returns
        -------
        CurveId
            Parsed identifier.

        Raises
        ------
        ValueError
            If *json* is not a JSON string.

        Examples
        --------
        >>> from finstack_quant.core.types import CurveId
        >>> CurveId.from_json('"USD-OIS"') == CurveId("USD-OIS")
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (a quoted string).

        Returns
        -------
        str
            JSON string literal.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid id).
        """
        ...

    def as_str(self) -> str:
        """
        Underlying string value.

        Returns
        -------
        str
            Exact curve-identifier text supplied at construction.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.core.types import CurveId
        >>> CurveId("USD-OIS").as_str()
        'USD-OIS'
        """
        ...

    def is_empty(self) -> bool:
        """
        Return whether the identifier is the empty string.

        Returns
        -------
        bool
            ``True`` for ``CurveId("")``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __len__(self) -> int:
        """Return the identifier length in UTF-8 bytes.

        Returns
        -------
        int
        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this curve id.

        Returns
        -------
        str
        """
        ...
    def __str__(self) -> str:
        """Return the string value of this curve id.

        Returns
        -------
        str
        """
        ...
    def __hash__(self) -> int:
        """Return a hash for this curve id.

        Returns
        -------
        int
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two curve ids are equal.

        Returns
        -------
        bool
        """
        ...
    def __ne__(self, other: object) -> bool:
        """Return whether two curve ids are not equal.

        Returns
        -------
        bool
        """
        ...
    def __lt__(self, other: CurveId) -> bool: ...
    def __le__(self, other: CurveId) -> bool: ...
    def __gt__(self, other: CurveId) -> bool: ...
    def __ge__(self, other: CurveId) -> bool: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class InstrumentId:
    """
    A unique identifier for a financial instrument.

    Immutable, hashable, lexicographically ordered string wrapper. Empty
    identifiers are accepted (see :meth:`is_empty`). Serializes as a quoted
    JSON string and is picklable.

    Parameters
    ----------
    value : str
        Instrument identifier string, stored verbatim.

    Examples
    --------
    >>> from finstack_quant.core.types import InstrumentId
    >>> InstrumentId("BOND_A").as_str()
    'BOND_A'
    >>> InstrumentId("A") < InstrumentId("B")
    True
    """

    def __init__(self, value: str) -> None:
        """
        Create an instrument identifier from its string value.

        Parameters
        ----------
        value : str
            Instrument identifier; may be empty.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.core.types import InstrumentId
        >>> InstrumentId("BOND_A").as_str()
        'BOND_A'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> InstrumentId:
        """
        Deserialize from JSON (a quoted string).

        Parameters
        ----------
        json : str
            JSON string literal.

        Returns
        -------
        InstrumentId
            Parsed identifier.

        Raises
        ------
        ValueError
            If *json* is not a JSON string.

        Examples
        --------
        >>> from finstack_quant.core.types import InstrumentId
        >>> InstrumentId.from_json('"BOND_A"') == InstrumentId("BOND_A")
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (a quoted string).

        Returns
        -------
        str
            JSON string literal.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid id).
        """
        ...

    def as_str(self) -> str:
        """
        Underlying string value.

        Returns
        -------
        str
            Exact instrument-identifier text supplied at construction.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.core.types import InstrumentId
        >>> InstrumentId("BOND_A").as_str()
        'BOND_A'
        """
        ...

    def is_empty(self) -> bool:
        """
        Return whether the identifier is the empty string.

        Returns
        -------
        bool
            ``True`` for ``InstrumentId("")``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __len__(self) -> int:
        """Return the identifier length in UTF-8 bytes.

        Returns
        -------
        int
        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this instrument id.

        Returns
        -------
        str
        """
        ...
    def __str__(self) -> str:
        """Return the string value of this instrument id.

        Returns
        -------
        str
        """
        ...
    def __hash__(self) -> int:
        """Return a hash for this instrument id.

        Returns
        -------
        int
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two instrument ids are equal.

        Returns
        -------
        bool
        """
        ...
    def __ne__(self, other: object) -> bool:
        """Return whether two instrument ids are not equal.

        Returns
        -------
        bool
        """
        ...
    def __lt__(self, other: InstrumentId) -> bool: ...
    def __le__(self, other: InstrumentId) -> bool: ...
    def __gt__(self, other: InstrumentId) -> bool: ...
    def __ge__(self, other: InstrumentId) -> bool: ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class Attributes:
    """
    User-defined tags and string metadata attached to instruments.

    Tags are a sorted set of labels; metadata is a sorted ``str -> str`` map.
    The mapping protocol (``attrs["key"]``, ``"key" in attrs``,
    ``len(attrs)``, :meth:`keys`, :meth:`items`) covers the metadata map.
    Selectors (:meth:`matches_selector`) accept ``"*"``, ``"tag:<name>"`` and
    ``"meta:<key>=<value>"``; unknown syntax matches nothing. Structural
    equality, JSON (``{"tags": [...], "meta": {...}}``) and pickle are
    supported.

    Examples
    --------
    >>> from finstack_quant.core.types import Attributes
    >>> attributes = Attributes()
    >>> attributes.add_tag("energy")
    >>> attributes.set_meta("desk", "credit")
    >>> (attributes["desk"], "desk" in attributes, attributes.tags, attributes.matches_selector("tag:energy"))
    ('credit', True, ['energy'], True)

    """

    def __init__(self) -> None:
        """
        Create an empty attribute set with no tags and no metadata.

        Notes
        -----
        Construction does not raise.
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Attributes:
        """
        Deserialize from JSON ``{"tags": [...], "meta": {...}}``.

        Parameters
        ----------
        json : str
            JSON object; both fields optional, unknown fields rejected.

        Returns
        -------
        Attributes
            Parsed attribute set.

        Raises
        ------
        ValueError
            If *json* is malformed or contains unknown fields.

        Examples
        --------
        >>> from finstack_quant.core.types import Attributes
        >>> Attributes.from_json('{"tags": ["energy"]}').has_tag("energy")
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON; empty ``tags``/``meta`` parts are omitted.

        Returns
        -------
        str
            JSON object text (``"{}"`` for an empty set).

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid set).
        """
        ...

    @property
    def tags(self) -> list[str]:
        """
        Tags in sorted order.

        Returns
        -------
        list[str]
            Sorted tag labels.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def add_tag(self, tag: str) -> None:
        """
        Add a tag (no-op if already present).

        Parameters
        ----------
        tag : str
            Free-form, case-sensitive tag label (for example ``"energy"``).
            Tags are stored in a set, so re-adding an existing label leaves
            the attribute set unchanged; the empty string is accepted.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def has_tag(self, tag: str) -> bool:
        """
        Return whether *tag* is present.

        Parameters
        ----------
        tag : str
            Free-form tag label to look up. Matching is exact and
            case-sensitive; no wildcard or prefix matching is applied.

        Returns
        -------
        bool
            ``True`` when the tag set contains *tag*.

        Notes
        -----
        This method does not raise.
        """
        ...

    def matches_selector(self, selector: str) -> bool:
        """
        Match against a selector string.

        Parameters
        ----------
        selector : str
            ``"*"`` (always true), ``"tag:<name>"`` (tag present) or
            ``"meta:<key>=<value>"`` (exact metadata match).

        Returns
        -------
        bool
            ``True`` on a match; unknown selector syntax returns ``False``.

        Notes
        -----
        This method does not raise.
        """
        ...

    def get_meta(self, key: str) -> Optional[str]:
        """
        Return a metadata value by key, or ``None`` when the key is absent.

        Parameters
        ----------
        key : str
            Metadata key.

        Returns
        -------
        str | None
            Value if present, otherwise ``None``.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def set_meta(self, key: str, value: Union[str, int, float]) -> None:
        """
        Insert or replace a metadata entry.

        Parameters
        ----------
        key : str
            Metadata key.
        value : str | int | float
            Metadata value; non-string values are stored as ``str(value)``.

        Notes
        -----
        This method does not raise; it updates stored state in place.
        """
        ...

    def contains_meta_key(self, key: str) -> bool:
        """
        Return whether *key* exists in metadata.

        Parameters
        ----------
        key : str
            Metadata key.

        Returns
        -------
        bool
            ``True`` when the metadata map contains ``key``; otherwise ``False``.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def keys(self) -> list[str]:
        """
        Metadata keys in sorted order.

        Returns
        -------
        list[str]
            Metadata keys sorted lexicographically for deterministic iteration.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def items(self) -> list[tuple[str, str]]:
        """
        Metadata ``(key, value)`` pairs in sorted key order.

        Returns
        -------
        list[tuple[str, str]]
            Sorted key/value pairs.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __getitem__(self, key: str) -> str:
        """Return the metadata value for *key*.

        Raises
        ------
        KeyError
            If *key* is absent.

        Returns
        -------
        str
        """
        ...
    def __contains__(self, key: str) -> bool:
        """Return whether *key* is a metadata key.

        Returns
        -------
        bool
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two attribute sets have the same tags and metadata.

        Returns
        -------
        bool
        """
        ...
    def __ne__(self, other: object) -> bool: ...
    def __repr__(self) -> str:
        """Return a debug representation of this attribute set.

        Returns
        -------
        str
        """
        ...
    def __len__(self) -> int:
        """Return the number of metadata entries.

        Returns
        -------
        int
        """
        ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...
