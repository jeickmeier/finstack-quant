"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/fees.rs``."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any
from finstack_quant.core.dates import BusinessDayConvention, DayCount, StubKind, Tenor
from finstack_quant.core.money import Money

class FeeAccrualBasis:
    """
    Controls how the outstanding balance is sampled during fee accrual.

    Immutable, hashable enum-style type with one class attribute per
    Rust variant.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import FeeAccrualBasis
    >>> FeeAccrualBasis.POINT_IN_TIME != FeeAccrualBasis.TIME_WEIGHTED_AVERAGE
    True
    """

    POINT_IN_TIME: FeeAccrualBasis
    """Sample the outstanding balance at accrual-period start (default)."""
    TIME_WEIGHTED_AVERAGE: FeeAccrualBasis
    """Time-weighted average outstanding over the accrual period."""

class FeeBase:
    """
    Economic balance used as the base for a periodic basis-point fee.

    ``DRAWN`` is a class-attribute singleton for the fieldless variant;
    :meth:`undrawn` constructs the data-carrying ``Undrawn`` variant.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import FeeBase
    >>> FeeBase.DRAWN is not None
    True
    """

    DRAWN: FeeBase
    """Fee accrues on the drawn outstanding balance."""

    @staticmethod
    def undrawn(commitment: Money) -> FeeBase:
        """
        Fee accrues on undrawn = max(commitment - outstanding, 0).

        Parameters
        ----------
        commitment : Money
            Total facility commitment used to compute the undrawn amount.

        Returns
        -------
        FeeBase
            A fee base tied to the undrawn balance.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FeeBase
        >>> from finstack_quant.core.money import Money
        >>> FeeBase.undrawn(Money(10_000_000.0, "USD")) is not None
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Variant label: ``"drawn"`` or ``"undrawn"``.

        Returns
        -------
        str
            Variant label: ``"drawn"`` or ``"undrawn"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def commitment(self) -> Money | None:
        """
        Facility limit for ``undrawn`` bases, else ``None``.

        Returns
        -------
        Money | None
            Facility limit for ``undrawn`` bases, else ``None``.

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
        >>> from finstack_quant.cashflows.builder import FeeBase
        >>> isinstance(FeeBase.DRAWN.to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> FeeBase:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FeeBase
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FeeBase
        >>> value = FeeBase.DRAWN
        >>> FeeBase.from_json(value.to_json()).to_json() == value.to_json()
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

class FeeSpec:
    """
    Fee specification: a one-time fixed fee or a periodic basis-point fee.

    Constructed via :meth:`fixed` or :meth:`periodic_bp`, mirroring the
    two Rust ``FeeSpec`` variants. Negative amounts and bp quotes are
    treated as rebates and flow through unchanged.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows.builder import FeeSpec
    >>> from finstack_quant.core.money import Money
    >>> FeeSpec.fixed(datetime.date(2025, 1, 15), Money(-5_000.0, "USD")) is not None
    True
    """

    @staticmethod
    def fixed(date: datetime.date | str, amount: Money) -> FeeSpec:
        """
        Fixed fee paid once on a specified date.

        Parameters
        ----------
        date : datetime.date
            Payment date of the fixed fee.
        amount : Money
            Fee amount; negative amounts are rebates.

        Returns
        -------
        FeeSpec
            A one-time fixed fee specification.

        Raises
        ------
        TypeError
            If *date* is not a date-like object exposing integer ``year``,
            ``month``, and ``day`` attributes.
        ValueError
            If *date* contains invalid calendar fields.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.builder import FeeSpec
        >>> from finstack_quant.core.money import Money
        >>> FeeSpec.fixed(datetime.date(2025, 1, 15), Money(-5_000.0, "USD")) is not None
        True
        """
        ...

    @staticmethod
    def periodic_bp(
        *,
        base: FeeBase | str | dict[str, Any],
        bp: Decimal | str,
        frequency: Tenor,
        day_count: DayCount | str,
        calendar_id: str,
        business_day_convention: BusinessDayConvention | str,
        stub: StubKind | str = "short_front",
        accrual_basis: FeeAccrualBasis | str = "point_in_time",
    ) -> FeeSpec:
        """
        Periodic fee quoted in basis points per annum, accrued over generated periods.

        The keyword-only fields are the Rust ``FeeSpec::PeriodicBp`` variant:
        Rust supplies the defaults below and rejects an unknown keyword.

        Parameters
        ----------
        base : FeeBase | str | dict
            Economic balance the fee accrues against (drawn or undrawn).
        bp : decimal.Decimal | str
            Fee quote in basis points per annum, as an exact decimal
            (``"25"``); a ``float`` is rejected.
        frequency : Tenor
            Accrual and payment frequency for the fee schedule.
        day_count : DayCount | str
            Day-count convention used to annualize the fee accrual.
        calendar_id : str
            Holiday calendar id used with *business_day_convention*.
        business_day_convention : BusinessDayConvention | str
            Business-day convention applied to generated fee dates (default
            ``"modified_following"``).
        stub : StubKind | str
            Stub-handling rule for irregular periods (default ``"short_front"``).
        accrual_basis : FeeAccrualBasis | str
            How the outstanding balance is sampled (default ``"point_in_time"``).

        Returns
        -------
        FeeSpec
            A periodic basis-point fee specification.

        Raises
        ------
        TypeError
            If a positional argument is passed, or a value is not
            JSON-encodable (neither a wire value nor a typed wrapper).
        ValueError
            If a required field is missing, a keyword is unknown, *bp* is not
            an exact-decimal string, or a label is invalid.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FeeBase, FeeSpec
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.core.money import Money
        >>> spec = FeeSpec.periodic_bp(
        ...     base=FeeBase.undrawn(commitment=Money(10_000_000.0, "USD")),
        ...     bp="50",
        ...     frequency=Tenor.quarterly(),
        ...     day_count=DayCount.ACT_360,
        ...     business_day_convention="modified_following",
        ...     calendar_id="weekends_only",
        ... )
        >>> spec is not None
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Variant label: ``"fixed"`` or ``"periodic_bp"``.

        Returns
        -------
        str
            Variant label: ``"fixed"`` or ``"periodic_bp"``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def date(self) -> datetime.date | None:
        """
        Payment date of a fixed fee, else ``None``.

        Returns
        -------
        datetime.date | None
            Payment date of a fixed fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def amount(self) -> Money | None:
        """
        Amount of a fixed fee, else ``None``.

        Returns
        -------
        Money | None
            Amount of a fixed fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def base(self) -> FeeBase | None:
        """
        Fee base of a periodic fee, else ``None``.

        Returns
        -------
        FeeBase | None
            Fee base of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def bp(self) -> Decimal | None:
        """
        Basis-point quote of a periodic fee, else ``None``.

        Returns
        -------
        Decimal | None
            Basis-point quote of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def frequency(self) -> Tenor | None:
        """
        Accrual frequency of a periodic fee, else ``None``.

        Returns
        -------
        Tenor | None
            Accrual frequency of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def day_count(self) -> DayCount | None:
        """
        Day count of a periodic fee, else ``None``.

        Returns
        -------
        DayCount | None
            Day count of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def business_day_convention(self) -> BusinessDayConvention | None:
        """
        Business-day convention of a periodic fee, else ``None``.

        Returns
        -------
        BusinessDayConvention | None
            Business-day convention of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def calendar_id(self) -> str | None:
        """
        Calendar id of a periodic fee, else ``None``.

        Returns
        -------
        str | None
            Calendar id of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def stub(self) -> StubKind | None:
        """
        Stub rule of a periodic fee, else ``None``.

        Returns
        -------
        StubKind | None
            Stub rule of a periodic fee, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def accrual_basis(self) -> FeeAccrualBasis | None:
        """
        Balance-sampling basis of a periodic fee, else ``None``.

        Returns
        -------
        FeeAccrualBasis | None
            Balance-sampling basis of a periodic fee, else ``None``.

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
        >>> from finstack_quant.cashflows.builder import FeeBase, FeeSpec
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.dates import Tenor
        >>> isinstance(
        ...     FeeSpec.periodic_bp(
        ...         base=FeeBase.DRAWN,
        ...         bp="25",
        ...         frequency=Tenor.parse("3M"),
        ...         day_count=DayCount.ACT_360,
        ...         calendar_id="weekends_only",
        ...         business_day_convention="modified_following",
        ...     ).to_json(),
        ...     str,
        ... )
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> FeeSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FeeSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FeeBase, FeeSpec
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.dates import Tenor
        >>> value = FeeSpec.periodic_bp(
        ...     base=FeeBase.DRAWN,
        ...     bp="25",
        ...     frequency=Tenor.parse("3M"),
        ...     day_count=DayCount.ACT_360,
        ...     calendar_id="weekends_only",
        ...     business_day_convention="modified_following",
        ... )
        >>> FeeSpec.from_json(value.to_json()).to_json() == value.to_json()
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
