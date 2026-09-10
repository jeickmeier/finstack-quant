"""
Cashflow primitives: CashFlow, CFKind, settlement classification.

Typed bindings for ``finstack_quant_cashflows::primitives``, mirroring the
Rust-canonical ``CFKind`` classification enum and the ``CashFlow`` dated
payment record used throughout schedule construction and accrual.

Example::

    >>> import datetime
    >>> from finstack_quant.cashflows.primitives import CashFlow, CFKind
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
>>> from finstack_quant.cashflows.primitives import CFKind
>>> CFKind.parse("fixed") == CFKind.FIXED
True

"""

from __future__ import annotations

from typing import Any

import datetime

from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount

__all__ = ["CFKind", "CashFlow", "CashFlowAccrual", "is_cash_settlement_kind"]

class CFKind:
    """
    Cashflow classification (mirrors ``finstack_quant_core::cashflow::CFKind``).

    Each class attribute is a singleton instance for one Rust enum variant.
    Instances compare and hash by the underlying kind, and round-trip through
    their snake_case ``Display``/``FromStr`` label (e.g. ``"float_reset"``).

    Examples
    --------
    >>> from finstack_quant.cashflows.primitives import CFKind
    >>> CFKind.FIXED.name
    'fixed'
    >>> CFKind.parse("amortization") == CFKind.AMORTIZATION
    True
    """

    FIXED: CFKind
    FLOAT_RESET: CFKind
    INFLATION_COUPON: CFKind
    FEE: CFKind
    COMMITMENT_FEE: CFKind
    USAGE_FEE: CFKind
    FACILITY_FEE: CFKind
    NOTIONAL: CFKind
    PIK: CFKind
    AMORTIZATION: CFKind
    PRE_PAYMENT: CFKind
    REVOLVING_DRAW: CFKind
    REVOLVING_REPAYMENT: CFKind
    DEFAULTED_NOTIONAL: CFKind
    RECOVERY: CFKind
    ACCRUED_ON_DEFAULT: CFKind
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
        Parse a cashflow kind from its snake_case label or a documented alias.

        Parameters
        ----------
        name : str
            Snake_case label such as ``"fixed"`` or ``"float_reset"``, or a
            documented alias such as ``"amort"`` for ``AMORTIZATION``.
            Matching is case-insensitive and separator-normalized.

        Returns
        -------
        CFKind
            The kind whose canonical label or alias matches *name*.

        Raises
        ------
        ValueError
            If *name* does not match any known label or alias.

        Examples
        --------
        >>> from finstack_quant.cashflows.primitives import CFKind
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
            The Rust ``Display`` label, such as ``"float_reset"`` or
            ``"defaulted_notional"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def is_interest_like(self) -> bool:
        """
        Return whether this kind is interest-bearing.

        Covers ``FIXED``, ``FLOAT_RESET``, ``INFLATION_COUPON``, and ``STUB``;
        all other kinds (principal, fees, credit events, margin) return
        ``False``.

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

class CashFlowAccrual:
    """Contractual coupon boundaries and conventions, independent of payment timing.

    Examples
    --------
    >>> from finstack_quant.cashflows.primitives import CashFlowAccrual
    >>> from finstack_quant.core.dates import DayCount
    >>> a = CashFlowAccrual("2025-01-01", "2025-04-01", DayCount.BUS_252, calendar_id="weekends_only")
    >>> a.calendar_id
    'weekends_only'
    """

    def __init__(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        day_count: DayCount,
        projected_index_rate: float | None = None,
        calendar_id: str | None = None,
        coupon_period: tuple[datetime.date | str, datetime.date | str] | None = None,
        end_is_termination_date: bool = False,
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
        projected_index_rate : float, optional
            Decimal index rate before spread, gearing, floors and caps.
        calendar_id : str, optional
            Registered calendar identifier, including '+' joint identifiers.
            Required for BUS/252 accrued interest; otherwise optional.
        coupon_period : tuple[datetime.date or str, datetime.date or str], optional
            Regular ACT/ACT ICMA reference period for stub accrual. None leaves
            reference-period selection to the schedule accrual caller.
        end_is_termination_date : bool, optional
            Whether end is instrument maturity, for the 30E/360 ISDA February
            exception. False for intermediate coupon periods by default.

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
    def calendar_id(self) -> str | None:
        """Return the calendar used for calendar-dependent accrual.

        Returns
        -------
        str or None
            Registered identifier, or None if unspecified. Access does not raise; no lookup is performed.
        """
        ...

    @property
    def coupon_period(self) -> tuple[datetime.date, datetime.date] | None:
        """Return regular reference dates for ACT/ACT ICMA accrual.

        Returns
        -------
        tuple[datetime.date, datetime.date] or None
            Regular coupon anchors, or None when unspecified. Access does not raise domain errors.
        """
        ...

    @property
    def end_is_termination_date(self) -> bool:
        """Return whether accrual ends at instrument termination.

        Returns
        -------
        bool
            True applies the 30E/360 ISDA February maturity exception. Access does not raise domain errors.
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
    >>> from finstack_quant.cashflows.primitives import CashFlow, CFKind
    >>> from finstack_quant.core.money import Money
    >>> cf = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED)
    >>> cf.kind == CFKind.FIXED
    True
    """

    def __init__(
        self,
        date: datetime.date,
        amount: Money,
        kind: CFKind | str,
        reset_date: datetime.date | None = None,
        accrual_factor: float = 0.0,
        rate: float | None = None,
    ) -> None:
        """
        Construct a dated cashflow.

        Parameters
        ----------
        date : datetime.date
            Payment date (or reset date for ``CFKind.FLOAT_RESET`` fixing
            events).
        amount : Money
            Monetary amount including its currency.
        kind : CFKind | str
            Cashflow classification, either a ``CFKind`` instance or its
            snake_case label string (e.g. ``"fixed"``).
        reset_date : datetime.date, optional
            Index reset date for floating coupons; must not be after *date*.
        accrual_factor : float
            Accrual year fraction used for the coupon amount, default
            ``0.0``.
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
            The payment (or reset, for ``FLOAT_RESET``) date of this flow.

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
            Date on which principal changes; None uses the payment date. Access does not raise.
        """
        ...

    def get_balance_date(self) -> datetime.date:
        """Resolve the economic date of the principal movement.

        Returns
        -------
        datetime.date
            Explicit principal date, otherwise payment date. Resolution does not raise.
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
        Validate amount, accrual factor, rate, and reset-date ordering.

        Zero amounts are valid (e.g. floored coupons); only non-finite
        values, a negative accrual factor, or a reset date after the
        payment date are rejected.

        Raises
        ------
        ValueError
            If a value is non-finite, the accrual factor is negative, or
            the reset date is after the payment date.
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
            If a field cannot be represented in JSON (non-finite float).

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.primitives import CashFlow
        >>> from finstack_quant.core.money import Money
        >>> isinstance(CashFlow(datetime.date(2025, 6, 15), Money(1.0, "USD"), "fixed").to_json(), str)
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
        >>> from finstack_quant.cashflows.primitives import CashFlow
        >>> from finstack_quant.core.money import Money
        >>> value = CashFlow(datetime.date(2025, 6, 15), Money(1.0, "USD"), "fixed")
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
            If the value holds a non-finite float that JSON cannot carry.
        """
        ...

def is_cash_settlement_kind(kind: CFKind | str) -> bool:
    """
    Return whether a classified flow represents a cash settlement.

    ``PIK`` is a capitalization event and ``DEFAULTED_NOTIONAL`` is a
    write-down; both return ``False``. All other (settlement) kinds return
    ``True``.

    Parameters
    ----------
    kind : CFKind | str
        Classified cashflow kind to test, either a ``CFKind`` instance or
        its snake_case label string.

    Returns
    -------
    bool
        ``True`` for settlement kinds, ``False`` for ``PIK`` and
        ``DEFAULTED_NOTIONAL``.

    Raises
    ------
    ValueError
        If *kind* is a string that does not match a known label.

    Examples
    --------
    >>> from finstack_quant.cashflows.primitives import CFKind, is_cash_settlement_kind
    >>> is_cash_settlement_kind(CFKind.FIXED)
    True
    >>> is_cash_settlement_kind(CFKind.PIK)
    False
    """
    ...
