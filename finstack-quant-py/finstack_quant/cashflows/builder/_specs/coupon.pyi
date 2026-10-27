"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/specs/coupon.rs``."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.cashflows.builder._specs.schedule import ScheduleParams

class CouponType:
    """
    Coupon settlement type: cash, PIK, or an explicit cash/PIK split.

    Immutable, hashable enum-style type. Class attributes ``CASH`` and
    ``PIK`` are singletons for the fieldless variants; :meth:`split`
    constructs the data-carrying ``Split`` variant.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import CouponType
    >>> CouponType.CASH != CouponType.PIK
    True
    """

    CASH: CouponType
    """100% paid in cash."""
    PIK: CouponType
    """100% capitalized into principal."""

    @staticmethod
    def split(cash_fraction: Decimal | float | str, pik_fraction: Decimal | float | str) -> CouponType:
        """
        Split settlement with explicit cash and PIK fractions.

        Parameters
        ----------
        cash_fraction : decimal.Decimal | float
            Fraction of the coupon paid in cash, in ``[0, 1]``.
        pik_fraction : decimal.Decimal | float
            Fraction of the coupon capitalized as PIK, in ``[0, 1]``;
            ``cash_fraction + pik_fraction`` must sum to ``1``.

        Returns
        -------
        CouponType
            A ``Split`` coupon type with the given fractions.

        Raises
        ------
        TypeError
            If either percentage is not a ``Decimal``, ``float``, or
            ``int``.
        ValueError
            If either percentage cannot be represented as a finite
            ``Decimal``.

        Examples
        --------
        >>> from decimal import Decimal
        >>> from finstack_quant.cashflows.builder import CouponType
        >>> CouponType.split(Decimal("0.5"), Decimal("0.5")) is not None
        True
        """
        ...

    @property
    def cash_fraction(self) -> Decimal:
        """
        Cash fraction: ``1`` for CASH, ``0`` for PIK, the split value otherwise.

        Returns
        -------
        Decimal
            Cash fraction: ``1`` for CASH, ``0`` for PIK, the split value otherwise.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def pik_fraction(self) -> Decimal:
        """
        PIK fraction: ``0`` for CASH, ``1`` for PIK, the split value otherwise.

        Returns
        -------
        Decimal
            PIK fraction: ``0`` for CASH, ``1`` for PIK, the split value otherwise.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class FixedCouponSpec:
    """
    Fixed-rate coupon specification.

    Combines the coupon quote, settlement behavior, and schedule
    conventions required to emit a fixed-rate leg.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams
    >>> spec = FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360())
    >>> spec.rate
    Decimal('0.05')
    """

    def __init__(
        self,
        rate: Decimal | float,
        schedule: ScheduleParams,
        coupon_type: CouponType | None = None,
    ) -> None:
        """
        Construct a fixed-rate coupon specification.

        Parameters
        ----------
        rate : decimal.Decimal | float
            Annual coupon rate as a decimal (``0.05`` for 5%).
        schedule : ScheduleParams
            Accrual and payment schedule conventions.
        coupon_type : CouponType, optional
            Cash (default), PIK, or split settlement.

        Raises
        ------
        ValueError
            If *rate* is not representable as a finite decimal value.
        """
        ...

    @property
    def rate(self) -> Decimal:
        """
        Annual coupon rate in decimal terms (``0.05`` is 5%).

        Returns
        -------
        decimal.Decimal
            The coupon rate as an exact decimal (e.g. ``Decimal("0.05")``).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def schedule(self) -> ScheduleParams:
        """
        Accrual and payment schedule conventions.

        Returns
        -------
        ScheduleParams
            The schedule-generation parameters for this coupon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon_type(self) -> CouponType:
        """
        Cash / PIK / split settlement.

        Returns
        -------
        CouponType
            Cash / PIK / split settlement.

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
        >>> from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams
        >>> isinstance(FixedCouponSpec(0.05, ScheduleParams.quarterly_act360()).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> FixedCouponSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FixedCouponSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FixedCouponSpec, ScheduleParams
        >>> value = FixedCouponSpec(0.05, ScheduleParams.quarterly_act360())
        >>> FixedCouponSpec.from_json(value.to_json()).to_json() == value.to_json()
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

class FloatingCouponSpec:
    """
    Floating coupon specification composing a :class:`FloatingRateSpec`.

    Combines a floating-rate index specification with coupon settlement
    behavior and schedule conventions.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import (
    ...     FloatingCouponSpec,
    ...     FloatingRateSpec,
    ...     ScheduleParams,
    ... )
    >>> rate_spec = FloatingRateSpec(forward_curve_id="USD-SOFR-3M", spread_bp=Decimal("200"), reset_frequency="3M")
    >>> spec = FloatingCouponSpec(rate_spec, ScheduleParams.usd_sofr_swap())
    >>> spec.rate_spec.forward_curve_id
    'USD-SOFR-3M'
    """

    def __init__(
        self,
        rate_spec: FloatingRateSpec,
        schedule: ScheduleParams,
        coupon_type: CouponType | None = None,
    ) -> None:
        """
        Construct a floating-rate coupon specification.

        Parameters
        ----------
        rate_spec : FloatingRateSpec
            Floating rate specification (index, spread, floors/caps, etc.).
        schedule : ScheduleParams
            Accrual and payment schedule conventions.
        coupon_type : CouponType, optional
            Cash (default), PIK, or split settlement.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rate_spec(self) -> FloatingRateSpec:
        """
        Floating rate specification.

        Returns
        -------
        FloatingRateSpec
            The index, spread, floor/cap, and reset configuration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def schedule(self) -> ScheduleParams:
        """
        Schedule conventions.

        Returns
        -------
        ScheduleParams
            Schedule conventions.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def coupon_type(self) -> CouponType:
        """
        Cash / PIK / split settlement.

        Returns
        -------
        CouponType
            Cash / PIK / split settlement.

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
        >>> from finstack_quant.cashflows.builder import FloatingCouponSpec, FloatingRateSpec, ScheduleParams
        >>> isinstance(FloatingCouponSpec(FloatingRateSpec.sofr(50), ScheduleParams.usd_sofr_swap()).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> FloatingCouponSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FloatingCouponSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingCouponSpec, FloatingRateSpec, ScheduleParams
        >>> value = FloatingCouponSpec(FloatingRateSpec.sofr(50), ScheduleParams.usd_sofr_swap())
        >>> FloatingCouponSpec.from_json(value.to_json()).to_json() == value.to_json()
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

class FloatingRateFallback:
    """
    Policy for floating coupons that cannot be projected.

    ``ERROR`` and ``SPREAD_ONLY`` are class-attribute singletons;
    :meth:`fixed_rate` constructs the data-carrying ``FixedRate`` variant.
    The builder consults the policy whenever projecting a coupon fails: a
    missing forward curve (even when the fixing series covers the coupon), a
    pre-cutoff observation with no fixing series, a reset before the curve
    base date, or a projection day-count error. A non-error policy replaces
    the index for the whole coupon and logs a warning. It never fills a gap
    in recorded history: when a curve and a fixing series are both supplied,
    a missing pre-cutoff observation raises under every policy.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import FloatingRateFallback
    >>> FloatingRateFallback.ERROR != FloatingRateFallback.SPREAD_ONLY
    True
    """

    ERROR: FloatingRateFallback
    """Reject missing market data needed to observe or project the index (default)."""
    SPREAD_ONLY: FloatingRateFallback
    """Use a zero index when an absent fixing series or missing curve permits fallback."""

    @staticmethod
    def fixed_rate(rate: Decimal | float | str) -> FloatingRateFallback:
        """
        Supply a fixed index rate when missing market data permits fallback.

        Parameters
        ----------
        rate : decimal.Decimal | float
            Decimal annual index rate (e.g. ``0.045`` for 4.5%), not basis
            points. Used as the index for the whole coupon when projecting
            it fails (see :class:`FloatingRateFallback`). It cannot replace
            gaps in supplied history before an available curve's base date.

        Returns
        -------
        FloatingRateFallback
            A fallback policy using the fixed rate.

        Raises
        ------
        TypeError
            If *rate* is not a ``Decimal``, ``float``, or ``int``.
        ValueError
            If *rate* cannot be represented as a finite ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingRateFallback
        >>> FloatingRateFallback.fixed_rate(0.045) is not None
        True
        """
        ...

    @property
    def rate(self) -> Decimal | None:
        """
        Fixed decimal index rate for ``fixed_rate`` fallbacks, else ``None``.

        Returns
        -------
        Decimal | None
            Fixed decimal index rate for ``fixed_rate`` fallbacks, else ``None``.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class FloatingRateSpec:
    """
    Canonical floating rate specification for all instruments.

    Used by bonds, swaps, credit facilities, and structured products;
    mirrors ``finstack_quant_cashflows::builder::FloatingRateSpec``
    field-for-field. Construction validates reset lag and floor/cap ordering
    through the canonical Rust validator.

    Examples
    --------
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import FloatingRateSpec
    >>> spec = FloatingRateSpec(forward_curve_id="USD-SOFR-3M", spread_bp=Decimal("200"), reset_frequency="3M")
    >>> spec.forward_curve_id
    'USD-SOFR-3M'
    """

    def __init__(
        self,
        forward_curve_id: str,
        spread_bp: Decimal | float,
        reset_frequency: Tenor | str,
        gearing: Decimal | float | None = None,
        gearing_includes_spread: bool = True,
        index_floor_bp: Decimal | float | None = None,
        all_in_floor_bp: Decimal | float | None = None,
        all_in_cap_bp: Decimal | float | None = None,
        index_cap_bp: Decimal | float | None = None,
        overnight_index_constraints: OvernightIndexConstraintApplication | None = None,
        index_tenor: Tenor | str | None = None,
        reset_lag_days: int = 2,
        fixing_calendar_id: str | None = None,
        compounding: FloatingLegCompounding | None = None,
        overnight_basis: DayCount | None = None,
        fallback: FloatingRateFallback | None = None,
    ) -> None:
        """
        Construct a floating rate specification.

        Parameters
        ----------
        forward_curve_id : str
            Forward curve identifier (e.g. ``"USD-SOFR-3M"``).
        spread_bp : decimal.Decimal | float
            Spread/margin over the index in basis points.
        reset_frequency : Tenor | str
            Index reset cadence, for instruments that reset inside a payment
            period. The bare builder does not read it: it observes one term
            fixing per coupon accrual start, with ``reset_lag_days``, and
            overnight coupons use their daily observation schedule. The
            revolving credit facility builds its intra-period reset grid from
            it.
        gearing : decimal.Decimal | float, optional
            Leverage multiplier (default ``1``), applied before the all-in
            floor and cap; ``gearing_includes_spread`` sets whether it also
            scales the spread. Stored as an exact decimal.
        gearing_includes_spread : bool
            Whether gearing multiplies ``(index + spread)`` (``True``,
            default) or only the index component (``False``).
        index_floor_bp : decimal.Decimal | float, optional
            Floor applied to the index component, in basis points.
        all_in_floor_bp : decimal.Decimal | float, optional
            Floor applied to the final all-in rate, in basis points.
        all_in_cap_bp : decimal.Decimal | float, optional
            Cap applied to the final all-in rate, in basis points.
        index_cap_bp : decimal.Decimal | float, optional
            Cap applied to the index component, in basis points.
        overnight_index_constraints : OvernightIndexConstraintApplication, optional
            Where index floors/caps are applied for overnight-compounded
            coupons (default daily): daily bounds apply per observation;
            period index bounds and all-in bounds apply once to the completed
            coupon. With changing principal, any final annual-rate adjustment
            is allocated uniformly over contractual accrual time, preserving
            raw daily compound increments.
        index_tenor : Tenor | str, optional
            Expected term-index tenor. The builder only uses it to warn at
            build when it disagrees with the resolved forward curve's tenor
            by more than 10%; the curve's quoted tenor remains authoritative.
            Ignored for overnight coupons.
        reset_lag_days : int
            Nonnegative reset lag in business days (default ``2``, T-2
            convention). Negative values raise ``ValueError`` immediately.
        fixing_calendar_id : str, optional
            Calendar for the reset lag; defaults to the coupon schedule
            calendar when omitted.
        compounding : FloatingLegCompounding, optional
            How each period's fixings combine into the rate. Leave unset to
            let a pricing instrument resolve it from the rate-index registry
            (the bare cashflow builder then projects one term forward).
        overnight_basis : DayCount, optional
            Day-count basis for the overnight compounding denominator. When
            omitted for an overnight ``compounding`` method, the coupon
            schedule's day count is used if it is Act/360 or Act/365F; any
            other schedule day count makes :meth:`CashFlowBuilder.build` raise
            ``ValueError`` unless an explicit Act/360 or Act/365F basis is
            given. Ignored for non-overnight compounding.
        fallback : FloatingRateFallback, optional
            Policy when projecting a coupon fails (default ``ERROR``): a
            missing forward curve, an absent fixing series, a reset before
            the curve base date, or a projection day-count error. A non-error
            policy replaces the index for the whole coupon. Gaps in supplied
            history before an available curve's base date remain exact-date
            errors under every policy.

        Raises
        ------
        TypeError
            If a decimal quote is not a ``Decimal``, ``float``, or ``int``,
            or a tenor is neither a ``Tenor`` nor a string.
        ValueError
            If a decimal quote is not representable as a finite ``Decimal``,
            a tenor string is invalid, the reset lag is negative, an index
            floor exceeds its cap, or an all-in floor exceeds its cap.
        OverflowError
            If *reset_lag_days* is outside the signed 32-bit integer range.
        """
        ...

    @property
    def forward_curve_id(self) -> str:
        """
        Forward curve identifier.

        Returns
        -------
        str
            The forward curve id this spec projects from.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_bp(self) -> Decimal:
        """
        Spread over the index in basis points.

        Returns
        -------
        decimal.Decimal
            The spread quote, preserved at full precision.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def validate(self) -> None:
        """
        Validate reset lag and floor/cap ordering.

        Raises
        ------
        ValueError
            If the reset lag is negative or a floor exceeds its
            corresponding cap.
        """
        ...

    @property
    def gearing(self) -> Decimal:
        """
        Gearing multiplier applied to the index.

        Returns
        -------
        Decimal
            Gearing multiplier applied to the index.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def gearing_includes_spread(self) -> bool:
        """
        Whether gearing also scales the spread.

        Returns
        -------
        bool
            Whether gearing also scales the spread.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def index_floor_bp(self) -> Decimal | None:
        """
        Index floor in basis points, if any.

        Returns
        -------
        Decimal | None
            Index floor in basis points, if any.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def all_in_floor_bp(self) -> Decimal | None:
        """
        All-in floor in basis points, if any.

        Returns
        -------
        Decimal | None
            All-in floor in basis points, if any.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def all_in_cap_bp(self) -> Decimal | None:
        """
        All-in cap in basis points, if any.

        Returns
        -------
        Decimal | None
            All-in cap in basis points, if any.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def index_cap_bp(self) -> Decimal | None:
        """
        Index cap in basis points, if any.

        Returns
        -------
        Decimal | None
            Index cap in basis points, if any.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def overnight_index_constraints(self) -> OvernightIndexConstraintApplication:
        """
        Where index floors/caps apply on overnight legs.

        Returns
        -------
        OvernightIndexConstraintApplication
            Daily applies index bounds per observation; period applies them
            once to the completed coupon. All-in bounds also apply once at
            completion. With changing principal, final rate adjustments are
            allocated uniformly over contractual accrual time while preserving
            raw daily compound increments.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def reset_frequency(self) -> Tenor:
        """
        Return the index reset cadence.

        Returns
        -------
        Tenor
            Reset cadence for instruments that reset inside a payment period
            (the revolving credit facility's reset grid). The bare builder
            does not read it: term coupons observe one fixing at each accrual
            start, subject to reset lag, and overnight coupons use their
            daily fixing schedule.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def index_tenor(self) -> Tenor | None:
        """
        Return the expected term-index tenor, if set.

        Returns
        -------
        Tenor | None
            Tenor checked against the resolved forward curve: a disagreement
            exceeding 10% warns at build, and the curve's quoted tenor
            remains authoritative. ``None`` skips the check. Overnight
            coupons ignore this value.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def reset_lag_days(self) -> int:
        """
        Fixing lag in business days.

        Returns
        -------
        int
            Fixing lag in business days.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def fixing_calendar_id(self) -> str | None:
        """
        Fixing calendar identifier, if set.

        Returns
        -------
        str | None
            Fixing calendar identifier, if set.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def compounding(self) -> FloatingLegCompounding | None:
        """
        Period-rate compounding convention, if set.

        Returns
        -------
        FloatingLegCompounding | None
            The compounding convention, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def overnight_basis(self) -> DayCount | None:
        """
        Overnight compounding day count, if set.

        Returns
        -------
        DayCount | None
            Overnight compounding day count, or ``None`` to use the coupon
            schedule's day count (which must then be Act/360 or Act/365F).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def fallback(self) -> FloatingRateFallback:
        """
        Projection-failure fallback policy.

        Returns
        -------
        FloatingRateFallback
            Policy applied when a coupon cannot be projected; see
            :class:`FloatingRateFallback`.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @staticmethod
    def sofr(spread_bp: Decimal | float | str) -> FloatingRateSpec:
        """
        USD SOFR compounded in arrears (ARRC / ISDA 2021).

        Uses Act/360 daily compounding, no reset lag, and the dedicated
        ``sofr`` fixing/value-date calendar, which excludes Good Friday.
        The coupon schedule independently determines the payment calendar.

        Parameters
        ----------
        spread_bp : Decimal, float or str
            Spread over the index in basis points (``50`` means +50 bp per annum).

        Returns
        -------
        FloatingRateSpec
            Spec with ``forward_curve_id`` ``"USD-SOFR"``, unit gearing, no floors or caps
            and ``FloatingRateFallback.ERROR``.

        Raises
        ------
        ValueError
            If ``spread_bp`` cannot be represented as a ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingRateSpec
        >>> FloatingRateSpec.sofr(50).forward_curve_id
        'USD-SOFR'
        """
        ...

    @staticmethod
    def sonia(spread_bp: Decimal | float | str) -> FloatingRateSpec:
        """
        GBP SONIA compounded in arrears: Act/365F daily compounding, no reset lag, GBLO fixings.

        Parameters
        ----------
        spread_bp : Decimal, float or str
            Spread over the index in basis points (``50`` means +50 bp per annum).

        Returns
        -------
        FloatingRateSpec
            Spec with ``forward_curve_id`` ``"GBP-SONIA"``, unit gearing, no floors or caps
            and ``FloatingRateFallback.ERROR``.

        Raises
        ------
        ValueError
            If ``spread_bp`` cannot be represented as a ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingRateSpec
        >>> FloatingRateSpec.sonia(50).forward_curve_id
        'GBP-SONIA'
        """
        ...

    @staticmethod
    def euribor_3m(spread_bp: Decimal | float | str) -> FloatingRateSpec:
        """
        EUR 3M EURIBOR term rate: one fixing per coupon period at its lagged accrual start, with a 2-business-day TARGET2 lag and an explicit 3M index tenor.

        Parameters
        ----------
        spread_bp : Decimal, float or str
            Spread over the index in basis points (``50`` means +50 bp per annum).

        Returns
        -------
        FloatingRateSpec
            Spec with ``forward_curve_id`` ``"EUR-EURIBOR-3M"``, unit gearing, no floors or caps
            and ``FloatingRateFallback.ERROR``.

        Raises
        ------
        ValueError
            If ``spread_bp`` cannot be represented as a ``Decimal``.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingRateSpec
        >>> FloatingRateSpec.euribor_3m(50).forward_curve_id
        'EUR-EURIBOR-3M'
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
        >>> from finstack_quant.cashflows.builder import FloatingRateSpec
        >>> isinstance(FloatingRateSpec.sofr(50).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> FloatingRateSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        FloatingRateSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingRateSpec
        >>> value = FloatingRateSpec.sofr(50)
        >>> FloatingRateSpec.from_json(value.to_json()).to_json() == value.to_json()
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

class FloatingLegCompounding:
    """
    How the fixings of one accrual period combine into the period rate.

    ``SIMPLE`` (one term forward per period) and ``SIMPLE_AVERAGE``
    (accrual-weighted arithmetic average of daily overnight fixings) are
    class-attribute singletons; the compounded variants carry a business-day
    parameter and are built via their named factories. The JSON wire form is
    ``"simple"``, ``"simple_average"`` or e.g.
    ``{"compounded_in_arrears": {"lookback_days": 0}}``.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
    >>> FloatingLegCompounding.compounded_in_arrears(0).is_overnight()
    True
    >>> FloatingLegCompounding.SIMPLE.is_overnight()
    False
    """

    SIMPLE: FloatingLegCompounding
    """Simple term-rate projection (one forward per period; the default)."""
    SIMPLE_AVERAGE: FloatingLegCompounding
    """Arithmetic average of daily fixings weighted by accrual days."""

    @staticmethod
    def compounded_in_arrears(lookback_days: int) -> FloatingLegCompounding:
        """
        Compounded in arrears with an observation lookback.

        Parameters
        ----------
        lookback_days : int
            Business days by which observation dates move back while the day-count weights stay on the accrual dates; ``0`` is plain in-arrears (cleared OIS, also named by the ``sofr()`` / ``sonia()`` / ... presets).

        Returns
        -------
        FloatingLegCompounding
            A compounded-in-arrears convention with the given lookback.

        Raises
        ------
        OverflowError
            If *lookback_days* is outside the unsigned 32-bit integer range.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.compounded_in_arrears(2) is not None
        True
        """
        ...

    @staticmethod
    def compounded_with_observation_shift(shift_days: int) -> FloatingLegCompounding:
        """
        Compounded in arrears with both dates and weights shifted back.

        Parameters
        ----------
        shift_days : int
            Number of business days to shift observations and weights (ISDA 2021).

        Returns
        -------
        FloatingLegCompounding
            An observation-shift compounding convention with the given window.

        Raises
        ------
        OverflowError
            If *shift_days* is outside the unsigned 32-bit integer range.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.compounded_with_observation_shift(2) is not None
        True
        """
        ...

    @staticmethod
    def sofr_observation_shift() -> FloatingLegCompounding:
        """
        USD SOFR FRN convention: ISDA 2021 observation shift of 2 business days.

        Mirrors Rust ``FloatingLegCompounding::sofr_observation_shift``. The preset sets only the compounding method;
        the leg's day count is configured separately.

        Returns
        -------
        FloatingLegCompounding
            The market-standard convention, equal to ``compounded_with_observation_shift(2)``.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.sofr_observation_shift() == FloatingLegCompounding.compounded_with_observation_shift(
        ...     2
        ... )
        True
        """
        ...

    @staticmethod
    def sonia_observation_shift() -> FloatingLegCompounding:
        """
        GBP SONIA FRN convention: ISDA 2021 observation shift of 5 business days.

        Mirrors Rust ``FloatingLegCompounding::sonia_observation_shift``. The preset sets only the compounding method;
        the leg's day count is configured separately.

        Returns
        -------
        FloatingLegCompounding
            The market-standard convention, equal to ``compounded_with_observation_shift(5)``.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.sonia_observation_shift() == FloatingLegCompounding.compounded_with_observation_shift(
        ...     5
        ... )
        True
        """
        ...

    @staticmethod
    def rate_cutoff(cutoff_days: int) -> FloatingLegCompounding:
        """
        Compounded RFR with an end-of-period rate cut-off.

        Mirrors Rust ``FloatingLegCompounding::rate_cutoff``.

        Parameters
        ----------
        cutoff_days : int
            Business days before period end over which the overnight rate is frozen.

        Returns
        -------
        FloatingLegCompounding
            Compounded overnight convention that freezes the final ``cutoff_days``
            business days at the last observation before the cutoff.

        Raises
        ------
        OverflowError
            If *cutoff_days* is outside the unsigned 32-bit integer range.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.rate_cutoff(2).is_overnight()
        True
        """
        ...

    def is_overnight(self) -> bool:
        """
        Whether the period rate is built from daily overnight fixings.

        Mirrors Rust ``FloatingLegCompounding::is_overnight``: true for every
        convention except ``SIMPLE``.

        Returns
        -------
        bool
            ``False`` for ``SIMPLE``, ``True`` otherwise.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import FloatingLegCompounding
        >>> FloatingLegCompounding.compounded_in_arrears(0).is_overnight(), FloatingLegCompounding.SIMPLE.is_overnight()
        (True, False)
        """
        ...

class OvernightIndexConstraintApplication:
    """
    Where overnight index floors/caps are applied for compounded rates.

    Immutable, hashable enum-style type with one class attribute per
    Rust variant.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import OvernightIndexConstraintApplication
    >>> OvernightIndexConstraintApplication.DAILY != OvernightIndexConstraintApplication.PERIOD
    True
    """

    DAILY: OvernightIndexConstraintApplication
    """Apply index floors/caps to each daily fixing before compounding (default)."""
    PERIOD: OvernightIndexConstraintApplication
    """Apply index floors/caps once to the compounded period index rate."""

class StepUpCouponSpec:
    """
    Step-up/step-down coupon specification.

    A fixed-style coupon whose rate changes at scheduled effective dates,
    used for step-up bonds and covenant-linked pricing grids.

    Examples
    --------
    >>> import datetime
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import ScheduleParams, StepUpCouponSpec
    >>> spec = StepUpCouponSpec(
    ...     initial_rate=Decimal("0.04"),
    ...     step_schedule=[(datetime.date(2027, 1, 1), Decimal("0.045"))],
    ...     schedule=ScheduleParams.semiannual_30360(),
    ... )
    >>> spec is not None
    True
    """

    def __init__(
        self,
        initial_rate: Decimal | float,
        step_schedule: list[tuple[datetime.date, Decimal | float]],
        schedule: ScheduleParams,
        coupon_type: CouponType | None = None,
    ) -> None:
        """
        Construct a step-up/step-down coupon specification.

        Parameters
        ----------
        initial_rate : decimal.Decimal | float
            Rate in effect until the first step date.
        step_schedule : list[tuple[datetime.date, decimal.Decimal | float]]
            ``(effective_date, new_rate)`` pairs, strictly increasing by
            date; each rate is applied at its unadjusted accrual-period
            start.
        schedule : ScheduleParams
            Accrual and payment schedule conventions.
        coupon_type : CouponType, optional
            Cash (default), PIK, or split settlement.

        Raises
        ------
        TypeError
            If a rate is not a ``Decimal``, ``float``, or ``int``, or a
            step date is not date-like.
        ValueError
            If a rate is not representable as a finite ``Decimal`` or a
            step date contains invalid calendar fields.
        """
        ...

    @property
    def initial_rate(self) -> Decimal:
        """
        Rate until the first step date (decimal).

        Returns
        -------
        Decimal
            Rate until the first step date (decimal).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def step_schedule(self) -> list[tuple[datetime.date, Decimal]]:
        """
        ``(effective_date, new_rate)`` pairs.

        Returns
        -------
        list[tuple[datetime.date, Decimal]]
            ``(effective_date, new_rate)`` pairs.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def schedule(self) -> ScheduleParams:
        """
        Schedule conventions.

        Returns
        -------
        ScheduleParams
            Schedule conventions.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def coupon_type(self) -> CouponType:
        """
        Cash / PIK / split settlement.

        Returns
        -------
        CouponType
            Cash / PIK / split settlement.

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
        >>> from finstack_quant.cashflows.builder import ScheduleParams, StepUpCouponSpec
        >>> isinstance(StepUpCouponSpec(0.05, [], ScheduleParams.quarterly_act360()).to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> StepUpCouponSpec:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        StepUpCouponSpec
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import ScheduleParams, StepUpCouponSpec
        >>> value = StepUpCouponSpec(0.05, [], ScheduleParams.quarterly_act360())
        >>> StepUpCouponSpec.from_json(value.to_json()).to_json() == value.to_json()
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
