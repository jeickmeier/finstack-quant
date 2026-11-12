"""
Cashflow primitives: CashFlow, CFKind, settlement classification.

Typed bindings for ``finstack_quant_core::cashflow``, mirroring the
Rust-canonical ``CFKind`` classification enum and the ``CashFlow`` dated
payment record used throughout schedule construction and accrual.

Example::

    >>> import datetime
    >>> from finstack_quant.core.cashflow import CashFlow, CFKind
    >>> from finstack_quant.core.money import Money
    >>> cf = CashFlow(
    ...     date=datetime.date(2025, 6, 15),
    ...     amount=Money(50_000.0, "USD"),
    ...     kind=CFKind.FIXED,
    ...     accrual_factor=0.5,
    ...     rate=0.05,
    ... )
    >>> cf.kind.name
    'fixed'

Examples
--------
>>> from finstack_quant.core.cashflow import CFKind
>>> CFKind.parse("fixed") == CFKind.FIXED
True

"""

from __future__ import annotations

from typing import Any

import datetime

from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount, DayCountContext

__all__ = ["CFKind", "CashFlow", "CashFlowAccrual"]

class CFKind:
    """
    Cashflow classification (mirrors ``finstack_quant_core::cashflow::CFKind``).

    Each class attribute is a singleton instance for one Rust enum variant.
    Instances compare and hash by the underlying kind, and round-trip through
    their snake_case ``Display``/``FromStr`` label (e.g. ``"floating"``).

    Examples
    --------
    >>> from finstack_quant.core.cashflow import CFKind
    >>> CFKind.FIXED.name
    'fixed'
    >>> CFKind.parse("amortization") == CFKind.AMORTIZATION
    True
    """

    FIXED: CFKind
    FLOATING: CFKind
    INFLATION_COUPON: CFKind
    FEE: CFKind
    COMMITMENT_FEE: CFKind
    USAGE_FEE: CFKind
    FACILITY_FEE: CFKind
    LC_FEE: CFKind
    FRONTING_FEE: CFKind
    NOTIONAL: CFKind
    PIK: CFKind
    AMORTIZATION: CFKind
    PRE_PAYMENT: CFKind
    REVOLVING_DRAW: CFKind
    REVOLVING_REPAYMENT: CFKind
    DEFAULTED_NOTIONAL: CFKind
    RECOVERY: CFKind
    ACCRUED_ON_DEFAULT: CFKind
    PROTECTION_PAYMENT: CFKind
    STUB: CFKind
    INITIAL_MARGIN_POST: CFKind
    INITIAL_MARGIN_RETURN: CFKind
    VARIATION_MARGIN_RECEIVE: CFKind
    VARIATION_MARGIN_PAY: CFKind
    MARGIN_INTEREST: CFKind
    COLLATERAL_SUBSTITUTION_IN: CFKind
    COLLATERAL_SUBSTITUTION_OUT: CFKind

    @classmethod
    def parse(cls, name: str) -> CFKind:
        """
        Parse a canonical cashflow kind label, ignoring outer whitespace and ASCII case.

        Parameters
        ----------
        name : str
            Snake_case label such as ``"fixed"``, ``"floating"``, or
            ``"pre_payment"``; the same string as :attr:`name` and the
            ``kind`` field of serialized cashflows. Surrounding whitespace and
            ASCII case are normalized; aliases are rejected.

        Returns
        -------
        CFKind
            The kind whose label equals *name*.

        Raises
        ------
        ValueError
            If *name* is not a known cashflow kind label.

        Examples
        --------
        >>> from finstack_quant.core.cashflow import CFKind
        >>> CFKind.parse("fixed") == CFKind.FIXED
        True
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical snake_case label of this kind.

        Returns
        -------
        str
            The Rust ``Display`` label, such as ``"floating"`` or
            ``"defaulted_notional"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def is_coupon(self) -> bool:
        """
        Return whether this kind is one of the four ordinary coupon kinds.

        Covers ``FIXED``, ``FLOATING``, ``INFLATION_COUPON``, and ``STUB``;
        all other kinds (principal, fees, credit events, margin) return
        ``False``. A classification, not a measure of all interest: PIK,
        accrued-on-default and margin interest are selected by their owners.

        Returns
        -------
        bool
            ``True`` for interest-bearing coupon kinds, ``False`` otherwise.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def is_principal_like(self) -> bool:
        """
        Return whether this kind changes principal balance.

        Covers ``NOTIONAL``, ``PIK``, ``AMORTIZATION``, ``PRE_PAYMENT``,
        revolving draws/repayments, and ``DEFAULTED_NOTIONAL``. Interest,
        fees, recovery, and margin kinds return ``False``.

        Returns
        -------
        bool
            ``True`` for principal-balance kinds, ``False`` otherwise.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def is_cash_settlement(self) -> bool:
        """
        Return whether this kind represents a cash settlement.

        ``PIK`` is a capitalization event and ``DEFAULTED_NOTIONAL`` is a
        write-down; both return ``False``. All other (settlement) kinds return
        ``True``.

        Returns
        -------
        bool
            ``True`` for settlement kinds, ``False`` for ``PIK`` and
            ``DEFAULTED_NOTIONAL``.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

class CashFlowAccrual:
    """Contractual coupon boundaries and conventions, independent of payment timing.

    Examples
    --------
    >>> from finstack_quant.core.cashflow import CashFlowAccrual
    >>> from finstack_quant.core.dates import DayCount, DayCountContext
    >>> context = DayCountContext("weekends_only", end_is_termination_date=False)
    >>> a = CashFlowAccrual("2025-01-01", "2025-04-01", DayCount.BUS_252, context)
    >>> a.context.calendar_id
    'weekends_only'
    """

    def __init__(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        day_count: DayCount,
        context: DayCountContext,
        projected_index_rate: float | None = None,
    ) -> None:
        """Create coupon accrual metadata.

        Parameters
        ----------
        start : datetime.date or str
            Inclusive accrual start, using an ISO date when supplied as text.
        end : datetime.date or str
            Exclusive accrual end, independent of the cash payment date.
        day_count : DayCount
            Convention used to compute year fractions.
        context : DayCountContext
            The complete day-count context the accrual factor was calculated
            with: calendar (required for BUS/252; '+' joint identifiers),
            coupon frequency, business-day basis, regular ACT/ACT ICMA
            reference period or full ACT/365L coupon bounds, and whether end is
            the instrument termination date (the 30E/360 ISDA February
            exception requires it stated, not left as None).
        projected_index_rate : float, optional
            Decimal index rate before spread, gearing, floors and caps.

        Raises
        ------
        ValueError
            If a supplied date cannot be parsed. Call CashFlow.validate() after
            attachment to check ordering and finite projected rates.
        """
        ...

    @property
    def start(self) -> datetime.date:
        """Return the inclusive accrual start.

        Returns
        -------
        datetime.date
            First date earning interest. Access does not raise domain errors.
        """
        ...

    @property
    def end(self) -> datetime.date:
        """Return the exclusive accrual end.

        Returns
        -------
        datetime.date
            Boundary where earning stops; payment may occur later. Access does not raise domain errors.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Return the coupon day-count convention.

        Returns
        -------
        DayCount
            Year-fraction convention. Access does not raise domain errors.
        """
        ...

    @property
    def projected_index_rate(self) -> float | None:
        """Return the unconstrained decimal index rate.

        Returns
        -------
        float or None
            Rate before spread/gearing/constraints, or None if absent. Access does not raise domain errors.
        """
        ...

    @property
    def context(self) -> DayCountContext:
        """Return the day-count context the accrual factor was calculated with.

        Returns
        -------
        DayCountContext
            Calendar, frequency, business-day basis, reference coupon period
            and termination status. Access does not raise domain errors.
        """
        ...

class CashFlow:
    """
    A single dated cash-flow (payment or reset).

    Represents a monetary flow at a specific date together with the
    classification and accrual metadata needed for downstream schedule
    aggregation, accrual, and risk calculations.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.cashflow import CashFlow, CFKind
    >>> from finstack_quant.core.money import Money
    >>> cf = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED, 0.0)
    >>> cf.kind == CFKind.FIXED
    True
    """

    def __init__(
        self,
        date: datetime.date,
        amount: Money,
        kind: CFKind | str,
        accrual_factor: float,
        reset_date: datetime.date | None = None,
        rate: float | None = None,
    ) -> None:
        """
        Construct a dated cashflow.

        Parameters
        ----------
        date : datetime.date
            Payment date (or reset date for ``CFKind.FLOATING`` fixing
            events).
        amount : Money
            Monetary amount including its currency.
        kind : CFKind | str
            Cashflow classification, either a ``CFKind`` instance or its
            snake_case label string (e.g. ``"fixed"``).
        accrual_factor : float
            Accrual year fraction used for the coupon amount; required, as in
            Rust ``CashFlow::new`` and the JSON wire form (``0.0`` for
            non-coupon rows). Checked by ``validate`` and
            ``CashFlowSchedule.from_flows``.
        reset_date : datetime.date, optional
            Index reset date for floating coupons; must not be after *date*.
        rate : float, optional
            Effective annual rate used to compute this cashflow, when known.

        Raises
        ------
        ValueError
            If *kind* is a string that does not match a known label, or if
            *amount* has a non-finite value.
        """
        ...

    @property
    def date(self) -> datetime.date:
        """
        Payment date for this cashflow.

        Returns
        -------
        datetime.date
            The payment (or reset, for ``FLOATING``) date of this flow.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def reset_date(self) -> datetime.date | None:
        """
        Optional index reset date for floating coupons.

        Returns
        -------
        datetime.date or None
            The reset date, or ``None`` when this flow has no separate
            reset event.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def amount(self) -> Money:
        """
        Monetary amount including its currency.

        Returns
        -------
        Money
            The currency-tagged amount of this cashflow.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> CFKind:
        """
        Cashflow classification.

        Returns
        -------
        CFKind
            The classification of this flow (interest, principal, fee, etc.).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accrual_factor(self) -> float:
        """
        Accrual year fraction used for the coupon amount.

        Returns
        -------
        float
            The accrual year fraction, non-negative.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float | None:
        """
        Effective annual rate used to calculate this cashflow, when known.

        Returns
        -------
        float or None
            The rate used for interest/fee flows, or ``None`` when this flow
            is not rate-based or the rate is unknown.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accrual(self) -> CashFlowAccrual | None:
        """Return contractual coupon metadata.

        Returns
        -------
        CashFlowAccrual or None
            Boundaries, day count, calendar and index metadata, or None if absent.
            Access does not raise domain errors.
        """
        ...

    @property
    def principal_delta(self) -> Money | None:
        """Return an explicit principal change independent of settlement cash.

        Returns
        -------
        Money or None
            Positive increases outstanding; None means derive from kind and amount.
            Access does not raise domain errors.
        """
        ...

    @property
    def principal_date(self) -> datetime.date | None:
        """Return the explicit economic principal date.

        Returns
        -------
        datetime.date or None
            Date on which principal changes; None for a row that moves no principal.
            Access does not raise.
        """
        ...

    def get_balance_date(self) -> datetime.date | None:
        """Return the explicit balance-effective date of a principal row.

        Returns
        -------
        datetime.date or None
            The row's ``principal_date``; None for a row that moves no principal.
            There is no payment-date fallback. Access does not raise.
        """
        ...

    def with_principal_date(self, date: datetime.date | str) -> CashFlow:
        """Set the economic principal date on a copy of this row.

        Parameters
        ----------
        date : datetime.date | str
            Balance-effective date, independently of this row's cash settlement date.

        Returns
        -------
        CashFlow
            New row with the supplied principal date and original payment date.

        Raises
        ------
        ValueError
            If the supplied date cannot be parsed.
        """
        ...

    def with_accrual(self, accrual: CashFlowAccrual) -> CashFlow:
        """Attach contractual metadata to a copy of this row.

        Parameters
        ----------
        accrual : CashFlowAccrual
            Coupon boundaries, day count, calendar and optional projected index.

        Returns
        -------
        CashFlow
            New row carrying the metadata. This method does not validate it;
            call validate() to check dates and finite rates. Attachment does not raise.
        """
        ...

    def with_principal_delta(self, delta: Money) -> CashFlow:
        """Attach a principal change to a copy of this row.

        Parameters
        ----------
        delta : Money
            Signed principal movement in the cashflow currency; positive increases it.

        Returns
        -------
        CashFlow
            New row with explicit principal movement. Call validate() to check
            currency; attachment does not raise and performs no domain validation.
        """
        ...

    def validate(self) -> None:
        """
        Validate accrual inputs, reset-date ordering and principal currency.

        Monetary amounts are already validated by Money construction; zero
        amounts are allowed. Accrual factors and rates must be finite, the
        accrual factor must be nonnegative, reset dates cannot follow payment,
        and principal deltas must share the cashflow currency.

        Raises
        ------
        ValueError
            If an accrual factor or rate is non-finite, the accrual factor
            is negative, reset-date ordering or accrual metadata is invalid,
            or the principal delta has a different currency.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict-serde JSON document; round-trips through :meth:`from_json`.

        Raises
        ------
        ValueError
            If an optional coupon rate or projected index rate is non-finite,
            or another field cannot be serialized. Invalid rates are rejected
            rather than serialized as absent metadata.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.cashflow import CashFlow
        >>> from finstack_quant.core.money import Money
        >>> isinstance(CashFlow(datetime.date(2025, 6, 15), Money(1.0, "USD"), "fixed", 0.0).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> CashFlow:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        CashFlow
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.cashflow import CashFlow
        >>> from finstack_quant.core.money import Money
        >>> value = CashFlow(datetime.date(2025, 6, 15), Money(1.0, "USD"), "fixed", 0.0)
        >>> CashFlow.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form (``from_json``, ``(to_json(),)``).

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If an optional coupon rate or projected index rate is non-finite,
            or another field cannot be serialized.
        """
        ...
