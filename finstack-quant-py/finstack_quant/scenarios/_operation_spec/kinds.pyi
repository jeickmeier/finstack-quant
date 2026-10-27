"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/operation_spec/kinds.rs``."""

from __future__ import annotations

# Typed operation builders
#
# These mirror the Rust ``OperationSpec`` enum and its supporting enums. They
# replace the raw-JSON authoring path so quants can write
# ``OperationSpec.curve_parallel_bp(...)`` and feed the result straight into
# ``ScenarioSpec(...)``.

class CurveKind:
    """
    Type of market curve targeted by a scenario operation.

    Examples
    --------
    >>> from finstack_quant.scenarios import CurveKind
    >>> CurveKind.discount().value
    'discount'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"discount"``, ``"forward"``, ``"par_cds"``, ``"inflation"``, ``"commodity"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> CurveKind("par_cds").value
        'par_cds'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``CurveKind`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def discount(cls) -> CurveKind:
        """
        Curve kind for a discount-factor (zero) curve.

        Returns
        -------
        CurveKind
            The ``discount`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.discount())
        'CurveKind.Discount'
        """
        ...

    @classmethod
    def forward(cls) -> CurveKind:
        """
        Curve kind for a forward/projection rate curve.

        Returns
        -------
        CurveKind
            The ``forward`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.forward())
        'CurveKind.Forward'
        """
        ...

    @classmethod
    def par_cds(cls) -> CurveKind:
        """
        Curve kind for a par CDS spread (hazard) curve.

        Returns
        -------
        CurveKind
            The ``par_cds`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.par_cds())
        'CurveKind.ParCDS'
        """
        ...

    @classmethod
    def inflation(cls) -> CurveKind:
        """
        Curve kind for an inflation/CPI index curve.

        Returns
        -------
        CurveKind
            The ``inflation`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.inflation())
        'CurveKind.Inflation'
        """
        ...

    @classmethod
    def commodity(cls) -> CurveKind:
        """
        Commodity forward curve.

        Returns
        -------
        CurveKind
            The ``commodity`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.commodity())
        'CurveKind.Commodity'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"Discount"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class TenorMatchMode:
    """
    Tenor-pillar alignment strategy for curve-node operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TenorMatchMode
    >>> TenorMatchMode.exact().value
    'exact'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"exact"``, ``"interpolate"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> TenorMatchMode("interpolate").value
        'interpolate'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``TenorMatchMode`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def exact(cls) -> TenorMatchMode:
        """
        Match curve nodes by exact tenor string.

        Returns
        -------
        TenorMatchMode
            The ``exact`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> str(TenorMatchMode.exact())
        'TenorMatchMode.Exact'
        """
        ...

    @classmethod
    def interpolate(cls) -> TenorMatchMode:
        """
        Interpolate between adjacent curve nodes when tenor is not exact.

        Returns
        -------
        TenorMatchMode
            The ``interpolate`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> str(TenorMatchMode.interpolate())
        'TenorMatchMode.Interpolate'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"Exact"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"exact"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class TimeRollMode:
    """
    Calendar-vs-business-day semantics for time-roll operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TimeRollMode
    >>> TimeRollMode.calendar_days().value
    'calendar_days'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"business_days"``, ``"calendar_days"``, ``"approximate"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> TimeRollMode("calendar_days").value
        'calendar_days'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``TimeRollMode`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def business_days(cls) -> TimeRollMode:
        """
        Roll by business days using the market calendar.

        Returns
        -------
        TimeRollMode
            The ``business_days`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.business_days())
        'TimeRollMode.BusinessDays'
        """
        ...

    @classmethod
    def calendar_days(cls) -> TimeRollMode:
        """
        Roll by calendar days (no holiday adjustment).

        Returns
        -------
        TimeRollMode
            The ``calendar_days`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.calendar_days())
        'TimeRollMode.CalendarDays'
        """
        ...

    @classmethod
    def approximate(cls) -> TimeRollMode:
        """
        Approximate roll (e.g. 30/360 day count).

        Returns
        -------
        TimeRollMode
            The ``approximate`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.approximate())
        'TimeRollMode.Approximate'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"CalendarDays"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"calendar_days"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class Compounding:
    """
    Compounding convention for rate-extraction operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import Compounding
    >>> Compounding.continuous().value
    'continuous'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"simple"``, ``"continuous"``, ``"semi_annual"``, ``"quarterly"``, ``"monthly"`` or ``"periodic(n)"`` for a positive integer n; annual compounding is ``"periodic(1)"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> Compounding("periodic(1)").value
        'periodic(1)'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``Compounding`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def simple(cls) -> Compounding:
        """
        Simple (zero-rate) compounding.

        Returns
        -------
        Compounding
            The ``simple`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.simple())
        'Compounding.simple'
        """
        ...

    @classmethod
    def continuous(cls) -> Compounding:
        """
        Continuously compounded rate.

        Returns
        -------
        Compounding
            The ``continuous`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.continuous())
        'Compounding.continuous'
        """
        ...

    @classmethod
    def semi_annual(cls) -> Compounding:
        """
        Semi-annual compounding.

        Returns
        -------
        Compounding
            The ``semi_annual`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.semi_annual())
        'Compounding.semi_annual'
        """
        ...

    @classmethod
    def quarterly(cls) -> Compounding:
        """
        Quarterly compounding convention for the bound rate.

        Returns
        -------
        Compounding
            The ``quarterly`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.quarterly())
        'Compounding.quarterly'
        """
        ...

    @classmethod
    def monthly(cls) -> Compounding:
        """
        Monthly compounding convention for the bound rate.

        Returns
        -------
        Compounding
            The ``monthly`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.monthly())
        'Compounding.monthly'
        """
        ...

    @property
    def name(self) -> str:
        """
        The Rust label, e.g. ``"continuous"`` or ``"semi_annual"``
        (same as ``value``).

        Returns
        -------
        str
            Canonical snake_case label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"continuous"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
