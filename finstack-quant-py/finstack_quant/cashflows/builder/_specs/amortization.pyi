"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/amortization.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money

class AmortizationSpec:
    """
    Amortization rule for principal over the life of a cashflow leg.

    Immutable, hashable value type. ``NONE`` is a class-attribute singleton
    for the fieldless variant; the remaining factory functions each
    construct a data-carrying Rust ``AmortizationSpec`` variant.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import AmortizationSpec
    >>> AmortizationSpec.NONE is not None
    True
    """

    NONE: AmortizationSpec
    """No amortization; principal remains constant until final redemption."""

    @staticmethod
    def linear_to(final_notional: Money) -> AmortizationSpec:
        """
        Linear principal paydown towards a target final notional.

        Parameters
        ----------
        final_notional : Money
            Target remaining principal at the end of the schedule; must be
            in the same currency as, and not exceed, the leg's initial
            notional.

        Returns
        -------
        AmortizationSpec
            A linear-paydown amortization rule.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> from finstack_quant.core.money import Money
        >>> AmortizationSpec.linear_to(Money(0.0, "USD")) is not None
        True
        """
        ...

    @staticmethod
    def step_remaining(
        schedule: list[tuple[datetime.date, Money]],
    ) -> AmortizationSpec:
        """
        Explicit schedule of remaining principal after each listed date.

        Parameters
        ----------
        schedule : list[tuple[datetime.date, Money]]
            Ordered ``(date, remaining_principal_after_date)`` pairs; dates
            must be strictly increasing and targets finite, nonnegative, and
            in the notional currency. A target may exceed an earlier target
            or the initial notional when prior draws or PIK fund it. Building
            the schedule rejects targets above the live balance on that date.

        Returns
        -------
        AmortizationSpec
            A step-schedule amortization rule.

        Raises
        ------
        TypeError
            If a schedule date does not expose integer ``year``, ``month``,
            and ``day`` attributes.
        ValueError
            If a schedule entry contains an invalid calendar date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> from finstack_quant.core.money import Money
        >>> spec = AmortizationSpec.step_remaining([(datetime.date(2026, 1, 1), Money(500_000.0, "USD"))])
        >>> spec is not None
        True
        """
        ...

    @staticmethod
    def percent_of_original_per_period(pct: float) -> AmortizationSpec:
        """
        Fixed percentage of original notional paid each period.

        Parameters
        ----------
        pct : float
            Fraction of original notional paid per period (e.g. ``0.05``
            for 5%); does not compound across periods.

        Returns
        -------
        AmortizationSpec
            A sinking-fund style amortization rule.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> AmortizationSpec.percent_of_original_per_period(0.05) is not None
        True
        """
        ...

    @staticmethod
    def percent_of_remaining_per_period(pct: float) -> AmortizationSpec:
        """
        Fixed percentage of the remaining outstanding paid each period.

        Parameters
        ----------
        pct : float
            Fraction of the outstanding principal paid per period (e.g.
            ``0.025`` for 2.5%), in ``[0, 1]``; the payment declines
            geometrically (declining balance).

        Returns
        -------
        AmortizationSpec
            A declining-balance amortization rule.

        Notes
        -----
        This method does not raise; the range is validated when a schedule
        is built.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> AmortizationSpec.percent_of_remaining_per_period(0.025).kind
        'percent_of_remaining_per_period'
        """
        ...

    @staticmethod
    def linear_between(start: datetime.date | str, end: datetime.date | str) -> AmortizationSpec:
        """
        Equal principal installments on coupon accrual boundaries in ``(start, end]``.

        The installment uses outstanding after start-date PIK and principal
        movements. The final installment clears the live remaining balance.
        Cash settles on each boundary's adjusted, lagged payment date.

        Parameters
        ----------
        start : datetime.date | str
            Economic amortization start, on or after issue; installments apply
            strictly after it.
        end : datetime.date | str
            Final economic repayment date, which must be a coupon accrual
            boundary no later than the terminal accrual date; validated at build.

        Returns
        -------
        AmortizationSpec
            Dated amortization rule; cash payment can follow the economic end.

        Raises
        ------
        ValueError
            If either date cannot be parsed.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> AmortizationSpec.linear_between(datetime.date(2025, 1, 1), datetime.date(2029, 1, 1)).kind
        'linear_between'
        """
        ...

    @staticmethod
    def custom_principal(
        items: list[tuple[datetime.date, Money]],
    ) -> AmortizationSpec:
        """
        Custom principal exchanges on specific dates (absolute cash amounts).

        Parameters
        ----------
        items : list[tuple[datetime.date, Money]]
            ``(date, principal_amount)`` exchanges with finite, nonnegative
            amounts in the notional currency; positive amounts reduce
            outstanding principal. Cumulative repayments may exceed the
            initial notional when prior draws or PIK fund them. Building the
            schedule rejects repayments above the live balance on each date.

        Returns
        -------
        AmortizationSpec
            A custom-principal-exchange amortization rule.

        Raises
        ------
        TypeError
            If an item date does not expose integer ``year``, ``month``,
            and ``day`` attributes.
        ValueError
            If an item contains an invalid calendar date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> from finstack_quant.core.money import Money
        >>> spec = AmortizationSpec.custom_principal([(datetime.date(2026, 1, 1), Money(50_000.0, "USD"))])
        >>> spec is not None
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Variant label: ``"none"``, ``"linear_to"``, ``"step_remaining"``, ``"percent_of_original_per_period"``, ``"percent_of_remaining_per_period"``, ``"linear_between"`` or ``"custom_principal"``.

        Returns
        -------
        str
            Variant label: ``"none"``, ``"linear_to"``, ``"step_remaining"``, ``"percent_of_original_per_period"``, ``"percent_of_remaining_per_period"``, ``"linear_between"`` or ``"custom_principal"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def final_notional(self) -> Money | None:
        """
        Target notional for ``linear_to``, else ``None``.

        Returns
        -------
        Money | None
            Target notional for ``linear_to``, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def schedule(self) -> list[tuple[datetime.date, Money]] | None:
        """
        ``(date, remaining)`` pairs for ``step_remaining``, else ``None``.

        Returns
        -------
        list[tuple[datetime.date, Money]] | None
            ``(date, remaining)`` pairs for ``step_remaining``, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def pct(self) -> float | None:
        """
        Per-period decimal percentage for ``percent_of_original_per_period`` and ``percent_of_remaining_per_period``, else ``None``.

        Returns
        -------
        float | None
            Per-period decimal percentage for ``percent_of_original_per_period`` and ``percent_of_remaining_per_period``, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def window(self) -> tuple[datetime.date, datetime.date] | None:
        """
        ``(start, end)`` dates for ``linear_between``, else ``None``.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None
            ``(start, end)`` dates for ``linear_between``, else ``None``.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def items(self) -> list[tuple[datetime.date, Money]] | None:
        """
        ``(date, amount)`` pairs for ``custom_principal``, else ``None``.

        Returns
        -------
        list[tuple[datetime.date, Money]] | None
            ``(date, amount)`` pairs for ``custom_principal``, else ``None``.

        Notes
        -----
        This accessor does not raise.
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
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> isinstance(AmortizationSpec.NONE.to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> AmortizationSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        AmortizationSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import AmortizationSpec
        >>> value = AmortizationSpec.NONE
        >>> AmortizationSpec.from_json(value.to_json()).to_json() == value.to_json()
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

class Notional:
    """
    Notional amount with an optional amortization rule.

    Combines an initial principal amount with an :class:`AmortizationSpec`
    describing how it evolves over the life of the leg.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import Notional
    >>> n = Notional.par(1_000_000.0, "USD")
    >>> n.initial.amount
    1000000.0
    """

    def __init__(
        self,
        initial: Money,
        amort: AmortizationSpec | None = None,
    ) -> None:
        """
        Construct a notional with an optional amortization rule.

        Parameters
        ----------
        initial : Money
            Initial principal amount outstanding at leg inception.
        amort : AmortizationSpec, optional
            Amortization rule applied after each period (default: none).

        Raises
        ------
        ValueError
            If *initial* has a non-finite amount.
        """
        ...

    @staticmethod
    def par(amount: float, currency: Currency | str) -> Notional:
        """
        Plain (non-amortising) notional helper.

        Parameters
        ----------
        amount : float
            Initial principal amount.
        currency : Currency | str
            Currency of the notional, as a ``Currency`` instance or ISO
            4217 code.

        Returns
        -------
        Notional
            A bullet notional with no amortization.

        Raises
        ------
        ValueError
            If *currency* is not a valid ISO 4217 code or *amount* is not
            finite.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import Notional
        >>> Notional.par(1_000_000.0, "USD").initial.amount
        1000000.0
        """
        ...

    @property
    def initial(self) -> Money:
        """
        Initial principal amount.

        Returns
        -------
        Money
            The principal amount outstanding at leg inception.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def currency(self) -> Currency:
        """
        Currency of the notional.

        Returns
        -------
        Currency
            The currency of the initial notional amount.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def validate(self) -> None:
        """
        Validate the notional and its amortization rule.

        Step targets and custom repayments are checked for finite,
        nonnegative amounts in the notional currency. Step dates must be
        strictly increasing. The builder checks each event against the live
        balance after prior draws and PIK; these rules have no initial-notional
        cap or requirement for successive step targets to decrease.

        Raises
        ------
        ValueError
            If the notional or amortization parameters are invalid, including
            a currency mismatch, a negative amount, unordered step dates, or
            a linear-to target above the initial notional.
        """
        ...

    @property
    def amort(self) -> AmortizationSpec:
        """
        Amortization rule.

        Returns
        -------
        AmortizationSpec
            Amortization rule.

        Notes
        -----
        This accessor does not raise.
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
        >>> from finstack_quant.cashflows.builder import Notional
        >>> isinstance(Notional.par(1.0, "USD").to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Notional:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        Notional
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import Notional
        >>> value = Notional.par(1.0, "USD")
        >>> Notional.from_json(value.to_json()).to_json() == value.to_json()
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
