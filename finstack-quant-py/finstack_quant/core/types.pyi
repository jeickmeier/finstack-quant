"""Core finstack_quant types: rates, identifiers, credit ratings, and attributes.

Provides typed wrappers for financial primitives used throughout the
``finstack_quant`` library.

Example::

    >>> from finstack_quant.core.types import Rate, Bps, Percentage
    >>> r = Rate(0.05)
    >>> r.as_percent
    5.0
    >>> r.as_bps
    500
    >>> Bps(250).as_decimal
    0.025
    >>> Percentage(12.5).as_decimal
    0.125
"""

from __future__ import annotations

from typing import Optional

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
    """A financial rate expressed as a decimal fraction.

    Immutable, hashable value type. Supports arithmetic and conversion between
    decimal, percent, and basis-point representations.

    Parameters
    ----------
    decimal : float
        Rate as a decimal fraction (e.g. ``0.05`` for 5%).

    Raises
    ------
    ValueError
        If *decimal* is not finite.

    Examples
    --------
    >>> from finstack_quant.core.types import Rate
    >>> r = Rate(0.05)
    >>> r.as_percent
    5.0
    >>> r.as_bps
    500
    >>> Rate.from_percent(5.0) == r
    True
    """

    ZERO: Rate
    """Zero rate (0% as a decimal rate)."""

    def __init__(self, decimal: float) -> None:
        """Construct a rate from a decimal fraction.

        Parameters
        ----------
        decimal : float
            Rate as a decimal (e.g. ``0.05`` for 5%).

        Raises
        ------
        ValueError
            If *decimal* is not finite.
        """
        ...

    @classmethod
    def from_percent(cls, percent: float) -> Rate:
        """Build from a percent value.

        Parameters
        ----------
        percent : float
            Rate in percent (e.g. ``5.0`` for 5%).

        Returns
        -------
        Rate

        Raises
        ------
        ValueError
            If *percent* is not finite.
        """
        ...

    @classmethod
    def from_bps(cls, bps: int) -> Rate:
        """Build from an integer basis-point amount.

        Parameters
        ----------
        bps : int
            Basis points (e.g. ``500`` for 5%).

        Returns
        -------
        Rate
        """
        ...

    @property
    def as_decimal(self) -> float:
        """Rate as a decimal fraction.

        Returns
        -------
        float
        """
        ...

    @property
    def as_percent(self) -> float:
        """Rate as a percent value.

        Returns
        -------
        float
        """
        ...

    @property
    def as_bps(self) -> int:
        """Rate rounded to the nearest basis point.

        Returns
        -------
        int
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
    def __add__(self, other: Rate) -> Rate: ...
    def __sub__(self, other: Rate) -> Rate: ...
    def __mul__(self, rhs: float) -> Rate: ...
    def __truediv__(self, rhs: float) -> Rate: ...
    def __neg__(self) -> Rate: ...

class Bps:
    """A value measured in basis points (1 bp = 0.0001).

    Immutable, hashable value type. Integer-valued internally after rounding.

    Parameters
    ----------
    bps : float
        Basis-point value (rounded to the nearest integer bp).

    Raises
    ------
    ValueError
        If *bps* is not finite.

    Examples
    --------
    >>> from finstack_quant.core.types import Bps
    >>> Bps(250).as_decimal
    0.025
    >>> Bps(100).as_bps
    100
    """

    ZERO: Bps
    """Zero basis points."""

    def __init__(self, bps: float) -> None:
        """Construct from a floating basis-point value (rounded to nearest integer bp).

        Parameters
        ----------
        bps : float
            Basis-point value.

        Raises
        ------
        ValueError
            If *bps* is not finite.

        Examples
        --------
        >>> from finstack_quant.core.types import Bps
        >>> Bps(250).as_decimal
        0.025
        """
        ...

    @property
    def as_decimal(self) -> float:
        """Value as a decimal fraction.

        Returns
        -------
        float

        Examples
        --------
        >>> Bps(250).as_decimal
        0.025
        """
        ...

    @property
    def as_bps(self) -> int:
        """Value as whole basis points.

        Returns
        -------
        int

        Examples
        --------
        >>> Bps(250).as_bps
        250
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
    def __truediv__(self, rhs: int) -> Bps: ...
    def __neg__(self) -> Bps: ...

class Percentage:
    """A percentage value (e.g. 12.5 means 12.5%).

    Immutable, hashable value type.

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
    >>> Percentage(50.0).as_percent
    50.0
    """

    ZERO: Percentage
    """Zero percent."""

    def __init__(self, percent: float) -> None:
        """Construct from a percent value.

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

    @property
    def as_decimal(self) -> float:
        """Value as a decimal fraction.

        Returns
        -------
        float

        Examples
        --------
        >>> Percentage(12.5).as_decimal
        0.125
        """
        ...

    @property
    def as_percent(self) -> float:
        """Value in percent terms.

        Returns
        -------
        float

        Examples
        --------
        >>> Percentage(12.5).as_percent
        12.5
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

class CreditRating:
    """Standardised credit rating category.

    Immutable, hashable enum-style type with class attributes for each
    rating level. Notched ratings (e.g. ``"BBB+"``) map to the base letter
    category.

    Parameters
    ----------
    None
        Use class attributes (e.g. ``CreditRating.AAA``) or
        :meth:`from_name` to construct.

    Examples
    --------
    >>> from finstack_quant.core.types import CreditRating
    >>> CreditRating.AAA.name
    'AAA'
    >>> CreditRating.from_name("bbb+") == CreditRating.BBB
    True
    """

    AAA: CreditRating
    """Highest quality rating."""
    AA: CreditRating
    """AA category."""
    A: CreditRating
    """Single-A category."""
    BBB: CreditRating
    """BBB category."""
    BB: CreditRating
    """BB category."""
    B: CreditRating
    """B category."""
    CCC: CreditRating
    """CCC category."""
    CC: CreditRating
    """CC category."""
    C: CreditRating
    """C category."""
    D: CreditRating
    """Default rating."""
    NR: CreditRating
    """Not rated."""

    @classmethod
    def from_name(cls, name: str) -> CreditRating:
        """Parse a rating string (case-insensitive; notches map to the base letter).

        Parameters
        ----------
        name : str
            Rating string (e.g. ``"BBB"``, ``"bbb+"``, ``"Baa1"``).

        Returns
        -------
        CreditRating

        Raises
        ------
        ValueError
            If *name* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.types import CreditRating
        >>> CreditRating.from_name("bbb+").name
        'BBB'
        """
        ...

    @property
    def name(self) -> str:
        """Canonical rating name (e.g. ``"BBB"``).

        Returns
        -------
        str

        Examples
        --------
        >>> CreditRating.AAA.name
        'AAA'
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class CurveId:
    """A unique identifier for a market data curve.

    Immutable, hashable string-wrapper type.

    Parameters
    ----------
    value : str
        Curve identifier string.

    Examples
    --------
    >>> from finstack_quant.core.types import CurveId
    >>> CurveId("USD-OIS").as_str()
    'USD-OIS'
    """

    def __init__(self, value: str) -> None:
        """Create a curve identifier from its string value.

        Parameters
        ----------
        value : str
            Curve identifier.

        Examples
        --------
        >>> from finstack_quant.core.types import CurveId
        >>> CurveId("USD-OIS").as_str()
        'USD-OIS'
        """
        ...

    def as_str(self) -> str:
        """Underlying string value.

        Returns
        -------
        str

        Examples
        --------
        >>> CurveId("USD-OIS").as_str()
        'USD-OIS'
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

class InstrumentId:
    """A unique identifier for a financial instrument.

    Immutable, hashable string-wrapper type.

    Parameters
    ----------
    value : str
        Instrument identifier string.

    Examples
    --------
    >>> from finstack_quant.core.types import InstrumentId
    >>> InstrumentId("BOND_A").as_str()
    'BOND_A'
    """

    def __init__(self, value: str) -> None:
        """Create an instrument identifier from its string value.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Examples
        --------
        >>> from finstack_quant.core.types import InstrumentId
        >>> InstrumentId("BOND_A").as_str()
        'BOND_A'
        """
        ...

    def as_str(self) -> str:
        """Underlying string value.

        Returns
        -------
        str

        Examples
        --------
        >>> InstrumentId("BOND_A").as_str()
        'BOND_A'
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

class Attributes:
    """A mutable key-value metadata bag.

    Stores string-typed metadata entries with set/get semantics.

    Examples
    --------
    >>> from finstack_quant.core.types import Attributes
    >>> attrs = Attributes()  # doctest: +SKIP
    >>> attrs.set_meta("sector", "tech")  # doctest: +SKIP
    >>> attrs.get_meta("sector")  # doctest: +SKIP
    'tech'
    """

    def __init__(self) -> None:
        """Create an empty attribute set.

        Examples
        --------
        >>> from finstack_quant.core.types import Attributes
        >>> attrs = Attributes()  # doctest: +SKIP
        """
        ...

    def get_meta(self, key: str) -> Optional[str]:
        """Fetch metadata by key.

        Parameters
        ----------
        key : str
            Metadata key.

        Returns
        -------
        str | None
            Value if present, otherwise ``None``.

        Examples
        --------
        >>> attrs = Attributes()  # doctest: +SKIP
        >>> attrs.get_meta("missing")  # doctest: +SKIP
        None
        """
        ...

    def set_meta(self, key: str, value: str) -> None:
        """Insert or replace a metadata entry.

        Parameters
        ----------
        key : str
            Metadata key.
        value : str
            Metadata value.

        Examples
        --------
        >>> attrs = Attributes()  # doctest: +SKIP
        >>> attrs.set_meta("sector", "tech")  # doctest: +SKIP
        """
        ...

    def contains_meta_key(self, key: str) -> bool:
        """Return whether *key* exists in metadata.

        Parameters
        ----------
        key : str
            Metadata key.

        Returns
        -------
        bool

        Examples
        --------
        >>> attrs = Attributes()  # doctest: +SKIP
        >>> attrs.contains_meta_key("sector")  # doctest: +SKIP
        False
        """
        ...

    def keys(self) -> list[str]:
        """Metadata keys in sorted order.

        Returns
        -------
        list[str]

        Examples
        --------
        >>> attrs = Attributes()  # doctest: +SKIP
        >>> attrs.keys()  # doctest: +SKIP
        []
        """
        ...

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
