"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/specs.rs``."""

from __future__ import annotations

import datetime
from typing import Any

class BalloonSpec:
    """
    Balloon terms of a commercial mortgage at maturity.

    At the loan's maturity a share ``extension_prob`` of the performing
    balance is extended ``extension_months`` at ``extension_rate`` (the
    original coupon when ``None``), a share ``loss_prob`` defaults into a
    workout that recovers ``1 − severity_pct / 100`` after ``workout_months``,
    and the rest pays as the balloon.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BalloonSpec
    >>> spec = BalloonSpec(0.3, 24, extension_rate=0.07, loss_prob=0.1, severity_pct=40.0, workout_months=18)
    >>> spec.extension_prob, spec.workout_months
    (0.3, 18)
    """

    def __init__(
        self,
        extension_prob: float,
        extension_months: int,
        *,
        extension_rate: float | None = None,
        loss_prob: float = 0.0,
        severity_pct: float = 0.0,
        workout_months: int = 0,
    ) -> None:
        """
        Construct balloon terms.

        Parameters
        ----------
        extension_prob : float
            Share of the performing balance extended at maturity, decimal in
            ``[0, 1]``.
        extension_months : int
            Length of the extension in months.
        extension_rate : float, optional
            Annual coupon (decimal) on the extended balance; ``None`` keeps the
            loan's coupon (spread and index for a floater).
        loss_prob : float, optional
            Share of the performing balance that defaults into a workout at
            maturity, decimal in ``[0, 1]``; ``0.0`` by default.
        severity_pct : float, optional
            Loss severity on the workout share in percent (``40.0`` = 40%);
            ``0.0`` by default.
        workout_months : int, optional
            Months after maturity until the workout recovery is received;
            ``0`` by default (the deal's recovery lag).

        Raises
        ------
        ValueError
            If a share is outside ``[0, 1]``, the severity is outside
            ``[0, 100]`` or the rate is not finite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BalloonSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BalloonSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        BalloonSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BalloonSpec
        >>> try:
        ...     BalloonSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(BalloonSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def extension_prob(self) -> float:
        """
        Share of the performing balance extended at maturity.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extension_months(self) -> int:
        """
        Length of the balloon extension.

        Returns
        -------
        int
            Number of months the extended share is carried past maturity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extension_rate(self) -> float | None:
        """
        Coupon on the extended balance.

        Returns
        -------
        float | None
            Annual decimal, or ``None`` to keep the loan's coupon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def loss_prob(self) -> float:
        """
        Share of the performing balance that defaults into a workout.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def severity_pct(self) -> float:
        """
        Loss severity on the workout share.

        Returns
        -------
        float
            Percent of the workout balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def workout_months(self) -> int:
        """
        Months after maturity until the workout recovery is received.

        Returns
        -------
        int
            Months; ``0`` uses the deal's recovery lag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PrepaymentPenalty:
    """
    Prepayment protection on a commercial mortgage.

    Built through one of the four constructors: :meth:`lockout` blocks
    voluntary prepayment, :meth:`fixed` charges a percent of the prepaid
    balance, :meth:`step_down` charges the step in force from a declining
    schedule and :meth:`yield_maintenance` charges the coupon lost over the
    remaining term. Premiums are collected by the trust as interest.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    >>> PrepaymentPenalty.fixed(3.0, through=datetime.date(2026, 1, 1)).kind
    'fixed'
    >>> PrepaymentPenalty.lockout(datetime.date(2025, 12, 31)).through
    datetime.date(2025, 12, 31)
    """

    @staticmethod
    def lockout(through: datetime.date | None = None) -> PrepaymentPenalty:
        """
        A lockout: no voluntary prepayment on or before ``through``.

        Parameters
        ----------
        through : datetime.date, optional
            Last date of the lockout; ``None`` locks the loan out to maturity.

        Returns
        -------
        PrepaymentPenalty
            The lockout.

        Raises
        ------
        ValueError
            If ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> PrepaymentPenalty.lockout().kind
        'lockout'
        """
        ...

    @staticmethod
    def fixed(pct: float, through: datetime.date | None = None) -> PrepaymentPenalty:
        """
        A fixed percent of the prepaid balance.

        Parameters
        ----------
        pct : float
            Premium in percent of the prepaid balance (``3.0`` = 3%).
        through : datetime.date, optional
            Last date the penalty applies; ``None`` applies it to maturity.

        Returns
        -------
        PrepaymentPenalty
            The fixed penalty.

        Raises
        ------
        ValueError
            If ``pct`` is negative or ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> PrepaymentPenalty.fixed(3.0).pct
        3.0
        """
        ...

    @staticmethod
    def step_down(schedule: list[tuple[datetime.date, float]]) -> PrepaymentPenalty:
        """
        A declining schedule of percents (5-4-3-2-1).

        Parameters
        ----------
        schedule : list[tuple[datetime.date, float]]
            ``(through, pct)`` steps in ascending date order; the first step
            whose date is on or after the prepayment applies, nothing after the
            last.

        Returns
        -------
        PrepaymentPenalty
            The step-down penalty.

        Raises
        ------
        ValueError
            If the schedule is empty, not ascending, or a percent is negative.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> steps = [(datetime.date(2025, 12, 31), 5.0), (datetime.date(2026, 12, 31), 3.0)]
        >>> len(PrepaymentPenalty.step_down(steps).schedule)
        2
        """
        ...

    @staticmethod
    def yield_maintenance(
        *,
        reinvestment_rate: float | None = None,
        discount_curve_id: str | None = None,
        floor_pct: float | None = None,
        through: datetime.date | None = None,
    ) -> PrepaymentPenalty:
        """
        Yield maintenance: the coupon lost over the remaining term.

        Parameters
        ----------
        reinvestment_rate : float, optional
            Annual reinvestment rate as a decimal; ``None`` uses the curve's
            zero rate to maturity (then ``discount_curve_id`` is required).
        discount_curve_id : str, optional
            Market-context discount curve the annual lost coupons are
            discounted on; ``None`` leaves the premium undiscounted.
        floor_pct : float, optional
            Minimum premium in percent of the prepaid balance (the common
            ``1.0``); ``None`` for no floor.
        through : datetime.date, optional
            Last date the penalty applies; ``None`` applies it to maturity.

        Returns
        -------
        PrepaymentPenalty
            The yield-maintenance penalty.

        Raises
        ------
        ValueError
            If neither a rate nor a curve is given, a rate or floor is
            negative, or ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> ym = PrepaymentPenalty.yield_maintenance(discount_curve_id="USD-OIS", floor_pct=1.0)
        >>> ym.kind, ym.floor_pct
        ('yield_maintenance', 1.0)
        """
        ...

    @staticmethod
    def from_json(json: str) -> PrepaymentPenalty:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PrepaymentPenalty`` (the shape ``to_json`` writes).

        Returns
        -------
        PrepaymentPenalty
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> try:
        ...     PrepaymentPenalty.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PrepaymentPenalty.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Kind of protection.

        Returns
        -------
        str
            ``"lockout"``, ``"fixed"``, ``"step_down"`` or ``"yield_maintenance"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def through(self) -> datetime.date | None:
        """
        Last date the penalty or lockout applies.

        Returns
        -------
        datetime.date | None
            ``None`` to maturity, or for a step-down schedule.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def pct(self) -> float | None:
        """
        Fixed premium percent.

        Returns
        -------
        float | None
            ``None`` for the other kinds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def schedule(self) -> list[tuple[datetime.date, float]] | None:
        """
        Step-down schedule.

        Returns
        -------
        list[tuple[datetime.date, float]] | None
            ``(through, pct)`` pairs, or ``None`` for the other kinds.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def reinvestment_rate(self) -> float | None:
        """
        Yield-maintenance reinvestment rate.

        Returns
        -------
        float | None
            Annual decimal, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_curve_id(self) -> str | None:
        """
        Yield-maintenance discount curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def floor_pct(self) -> float | None:
        """
        Yield-maintenance floor.

        Returns
        -------
        float | None
            Percent of the prepaid balance, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class SpecialServicingSpec:
    """
    Special-servicing state of a commercial mortgage at closing.

    The appraisal reduction (ASER) cuts the interest advanced on the loan to
    ``1 − appraisal_reduction_pct / 100`` of its balance; the loan is
    specially serviced from closing, so the deal's special servicer fee,
    workout fee and liquidation fee apply to it.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import SpecialServicingSpec
    >>> SpecialServicingSpec(40.0).appraisal_reduction_pct
    40.0
    """

    def __init__(self, appraisal_reduction_pct: float) -> None:
        """
        Construct the special-servicing state.

        Parameters
        ----------
        appraisal_reduction_pct : float
            Appraisal reduction in percent of the loan balance (``40.0`` =
            40%); required as in Rust (``0.0`` = specially serviced with full
            advancing).

        Raises
        ------
        ValueError
            If the percent is outside ``[0, 100]``.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SpecialServicingSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``SpecialServicingSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        SpecialServicingSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import SpecialServicingSpec
        >>> try:
        ...     SpecialServicingSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(SpecialServicingSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def appraisal_reduction_pct(self) -> float:
        """
        Appraisal reduction.

        Returns
        -------
        float
            Percent of the loan balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LiquidationSpec:
    """
    Resolution timeline of a non-performing or re-performing loan.

    The loan pays nothing until ``months_to_resolution`` months from its
    origination (acquisition date, else closing); then the share
    ``1 − reperformance_prob`` liquidates at ``proceeds_pct − carry_cost_pct``
    percent of its balance and the rest re-performs at ``modified_rate``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import LiquidationSpec
    >>> spec = LiquidationSpec(18, 65.0, carry_cost_pct=5.0, reperformance_prob=0.2, modified_rate=0.04)
    >>> spec.months_to_resolution, spec.net_proceeds_fraction
    (18, 0.6)
    """

    def __init__(
        self,
        months_to_resolution: int,
        proceeds_pct: float,
        *,
        carry_cost_pct: float = 0.0,
        reperformance_prob: float = 0.0,
        modified_rate: float | None = None,
    ) -> None:
        """
        Construct the resolution terms.

        Parameters
        ----------
        months_to_resolution : int
            Months from origination until the loan resolves.
        proceeds_pct : float
            Gross liquidation proceeds in percent of the balance.
        carry_cost_pct : float, optional
            Carry and disposition costs in percent of the balance, netted from
            the proceeds; ``0.0`` by default.
        reperformance_prob : float, optional
            Share of the balance that re-performs instead of liquidating,
            decimal in ``[0, 1]``; ``0.0`` by default.
        modified_rate : float, optional
            Annual coupon (decimal) of the re-performing share; ``None`` keeps
            the loan's coupon.

        Raises
        ------
        ValueError
            If a percent is outside ``[0, 100]``, the share is outside
            ``[0, 1]`` or the rate is not finite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LiquidationSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``LiquidationSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        LiquidationSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails the same
            canonical Rust validation as construction (invalid ranges, dates or rules).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import LiquidationSpec
        >>> try:
        ...     LiquidationSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(LiquidationSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def months_to_resolution(self) -> int:
        """
        Months from origination until the loan resolves.

        Returns
        -------
        int
            Number of months after origination on which the liquidation and
            re-performing shares are booked.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def proceeds_pct(self) -> float:
        """
        Gross liquidation proceeds.

        Returns
        -------
        float
            Percent of the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def carry_cost_pct(self) -> float:
        """
        Carry and disposition costs.

        Returns
        -------
        float
            Percent of the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reperformance_prob(self) -> float:
        """
        Share of the balance that re-performs.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def modified_rate(self) -> float | None:
        """
        Coupon of the re-performing share.

        Returns
        -------
        float | None
            Annual decimal, or ``None`` to keep the loan's coupon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def net_proceeds_fraction(self) -> float:
        """
        Net liquidation proceeds as a fraction of the balance.

        Returns
        -------
        float
            ``(proceeds_pct − carry_cost_pct) / 100``, floored at zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
