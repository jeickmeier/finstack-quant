"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/schedule.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.core.dates import BusinessDayConvention, DayCount, StubKind, Tenor

class PrincipalExchange:
    """
    Whether the builder emits issue-funding and maturity-redemption notionals.

    Immutable, hashable enum-style type. Outstanding still starts at the
    configured initial principal for coupon math. This is not the
    cross-currency ``NotionalExchange`` policy.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import PrincipalExchange
    >>> PrincipalExchange.NONE != PrincipalExchange.INITIAL_AND_FINAL
    True
    """

    NONE: PrincipalExchange
    """Do not emit issue or redemption ``CFKind.NOTIONAL`` flows."""
    INITIAL_AND_FINAL: PrincipalExchange
    """Emit issue funding and maturity redemption (default)."""

class RollRule:
    """
    Roll-date rule applied when generating schedule anchors.

    Immutable, hashable enum-style type with one class attribute per
    Rust variant (``NONE``, ``IMM``, ``CDS_IMM``). ``IMM`` is a roll
    convention on the schedule's own frequency and stub rule; ``CDS_IMM`` is
    the ISDA standard CDS grid and overrides them with quarterly and
    short-back conventions for date generation and accrual. Neither supports
    end-of-month rolling. Schedule construction checks these convention
    combinations.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import RollRule
    >>> RollRule.NONE != RollRule.CDS_IMM
    True
    """

    NONE: RollRule
    """Plain tenor stepping from the schedule boundaries (default)."""
    IMM: RollRule
    """IMM roll convention: third Wednesdays of Mar/Jun/Sep/Dec at the frequency.

    Regular periods run between IMM dates one frequency apart; the frequency
    must be a whole number of quarters, and the stub rule picks forward or
    backward generation. The contractual start and maturity are kept, so an
    off-grid date produces a stub. A horizon with no interior IMM date
    produces one stub over the full horizon. ACT/ACT ICMA is rejected because
    this third-Wednesday grid has no supported nominal reference grid.
    """
    CDS_IMM: RollRule
    """Twentieths of Mar/Jun/Sep/Dec, supporting ACT/ACT ICMA accrual.

    The first coupon at the initial instrument horizon accrues from the
    preceding CDS roll date. Interior coupon-program windows start at their
    declared boundary and do not extend accrual backward before that window.
    """

class ScheduleParams:
    """
    Canonical schedule-generation parameters for coupons and periodic fees.

    Controls how accrual boundaries and payment dates are generated. Ten
    market-convention presets (:meth:`quarterly_act360`,
    :meth:`usd_sofr_swap`, etc.) are available as staticmethods alongside
    the general constructor.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import ScheduleParams
    >>> ScheduleParams.usd_sofr_swap().calendar_id
    'usny'
    """

    def __init__(
        self,
        frequency: Tenor | str,
        day_count: DayCount,
        calendar_id: str,
        business_day_convention: BusinessDayConvention | str | None = None,
        stub: StubKind | None = None,
        end_of_month: bool = False,
        payment_lag_days: int = 0,
        adjust_accrual_dates: bool = False,
        roll_rule: RollRule | None = None,
    ) -> None:
        """
        Construct schedule-generation parameters.

        Parameters
        ----------
        frequency : Tenor | str
            Accrual and payment frequency (e.g. ``"3M"``). IMM and CDS_IMM
            replace it with quarterly frequency for date generation and accrual.
        day_count : DayCount
            Day-count convention for accrual year fractions. ACT_ACT_ISMA
            (ACT/ACT ICMA) supports the CDS_IMM twentieth grid but rejects
            the IMM third-Wednesday grid at schedule construction.
        calendar_id : str
            Holiday calendar id (``"weekends_only"`` for weekend-only
            rolling).
        business_day_convention : BusinessDayConvention | str, optional
            Payment-date rolling convention (default Modified Following).
        stub : StubKind, optional
            Stub rule (default short-front). CDS_IMM uses short-back
            regardless of this input; IMM honours it. For a plain ACT/ACT ICMA grid, short-front
            and long-front anchor regular periods on maturity; other rules
            anchor them on the start date.
        end_of_month : bool
            Preserve end-of-month rolling (default ``False``). Both IMM modes
            reject True at schedule construction. A plain ACT/ACT ICMA grid
            requires its regular anchor, selected by *stub*, to be month-end;
            the opposite endpoint may be irregular.
        payment_lag_days : int
            Nonnegative payment lag in business days (default ``0``).
        adjust_accrual_dates : bool
            Roll accrual boundaries with *business_day_convention*, i.e. swap/ISDA convention
            (default ``False``, bond convention).
        roll_rule : RollRule, optional
            IMM/CDS-IMM anchor grid (default none). These modes use quarterly,
            short-back conventions; see :class:`RollRule` for horizon and
            front-accrual behavior. Grid compatibility is checked when building
            the schedule, after its start and maturity dates are known.

        Raises
        ------
        TypeError
            If *frequency* or *business_day_convention* is neither the
            documented wrapper type nor a string.
        ValueError
            If a tenor or business-day convention string is invalid,
            *calendar_id* is unknown, or *payment_lag_days* is negative.
        OverflowError
            If *payment_lag_days* is outside the signed 32-bit integer range.
        """
        ...

    @staticmethod
    def quarterly_act360() -> ScheduleParams:
        """
        Quarterly, Act/360, Modified Following, weekends-only calendar.

        Returns
        -------
        ScheduleParams
            Generic quarterly preset with a weekends-only calendar.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.quarterly_act360().calendar_id
        'weekends_only'
        """
        ...

    @staticmethod
    def semiannual_30360() -> ScheduleParams:
        """
        Semi-annual, 30/360, Modified Following, weekends-only calendar.

        Returns
        -------
        ScheduleParams
            Generic semi-annual preset with a weekends-only calendar.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.semiannual_30360().calendar_id
        'weekends_only'
        """
        ...

    @staticmethod
    def annual_actact() -> ScheduleParams:
        """
        Annual, Act/Act, Following, weekends-only calendar.

        Returns
        -------
        ScheduleParams
            Generic annual preset with a weekends-only calendar.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.annual_actact().calendar_id
        'weekends_only'
        """
        ...

    @staticmethod
    def usd_sofr_swap() -> ScheduleParams:
        """
        USD SOFR swap: quarterly, Act/360, MF, USNY, T+2 lag, adjusted accruals.

        Returns
        -------
        ScheduleParams
            Market-convention preset for USD SOFR swap legs.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.usd_sofr_swap().payment_lag_days
        2
        """
        ...

    @staticmethod
    def usd_corporate_bond() -> ScheduleParams:
        """
        USD corporate bond: semi-annual, 30/360, Following, USNY.

        Returns
        -------
        ScheduleParams
            Market-convention preset for USD corporate bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.usd_corporate_bond().calendar_id
        'usny'
        """
        ...

    @staticmethod
    def usd_treasury() -> ScheduleParams:
        """
        USD Treasury: semi-annual, Act/Act ISMA, Following, USNY.

        Returns
        -------
        ScheduleParams
            Market-convention preset for US Treasury securities.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.usd_treasury().calendar_id
        'usny'
        """
        ...

    @staticmethod
    def eur_estr_swap() -> ScheduleParams:
        """
        EUR ESTR swap: annual, Act/360, MF, TARGET2, T+2 lag, adjusted accruals.

        Returns
        -------
        ScheduleParams
            Market-convention preset for EUR ESTR swap legs.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.eur_estr_swap().payment_lag_days
        2
        """
        ...

    @staticmethod
    def eur_gov_bond() -> ScheduleParams:
        """
        EUR government bond: annual, Act/Act ISMA, Following, TARGET2.

        Returns
        -------
        ScheduleParams
            Market-convention preset for EUR government bonds.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.eur_gov_bond() is not None
        True
        """
        ...

    @staticmethod
    def gbp_sonia_swap() -> ScheduleParams:
        """
        GBP SONIA swap: annual, Act/365F, MF, GBLO, no lag, adjusted accruals.

        Returns
        -------
        ScheduleParams
            Market-convention preset for GBP SONIA swap legs.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.gbp_sonia_swap().payment_lag_days
        0
        """
        ...

    @staticmethod
    def jpy_tona_swap() -> ScheduleParams:
        """
        JPY TONA swap: annual, Act/365F, MF, JPTO, T+2 lag, adjusted accruals.

        Returns
        -------
        ScheduleParams
            Market-convention preset for JPY TONA swap legs.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.jpy_tona_swap().payment_lag_days
        2
        """
        ...

    @property
    def frequency(self) -> Tenor:
        """
        Accrual and payment frequency.

        Returns
        -------
        Tenor
            The frequency used to generate schedule boundaries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """
        Day-count convention used to accrue this coupon schedule.

        Returns
        -------
        DayCount
            The convention used to convert accrual periods to year
            fractions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Holiday calendar identifier.

        Returns
        -------
        str
            The calendar id used together with the business-day convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_lag_days(self) -> int:
        """
        Payment lag in business days.

        Returns
        -------
        int
            Business days added after the adjusted accrual end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def end_of_month(self) -> bool:
        """
        Whether end-of-month rolling is preserved.

        Returns
        -------
        bool
            ``True`` when schedule generation preserves end-of-month dates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def adjust_accrual_dates(self) -> bool:
        """
        Whether accrual boundaries are business-day adjusted.

        Returns
        -------
        bool
            ``True`` for the swap/ISDA convention (both boundaries
            adjusted), ``False`` for the bond convention (only payment
            dates adjusted).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def business_day_convention(self) -> BusinessDayConvention:
        """
        Payment-date rolling convention.

        Returns
        -------
        BusinessDayConvention
            Payment-date rolling convention.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def stub(self) -> StubKind:
        """
        Stub-handling rule.

        Returns
        -------
        StubKind
            Stub-handling rule.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def roll_rule(self) -> RollRule:
        """
        Roll-date rule (IMM / CDS-IMM grid or none).

        Returns
        -------
        RollRule
            Roll-date rule (IMM / CDS-IMM grid or none).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def validate(self) -> None:
        """
        Fail-fast validation: known calendar id and non-negative payment lag.

        This check does not validate the accrual grid. Schedule construction
        checks IMM, end-of-month and ACT/ACT ICMA compatibility using the
        actual start and maturity dates.

        Raises
        ------
        ValueError
            If ``calendar_id`` is unknown or ``payment_lag_days`` is negative.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> ScheduleParams.usd_sofr_swap().validate()
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
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> isinstance(ScheduleParams.quarterly_act360().to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> ScheduleParams:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        ScheduleParams
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams
        >>> value = ScheduleParams.quarterly_act360()
        >>> ScheduleParams.from_json(value.to_json()).to_json() == value.to_json()
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
