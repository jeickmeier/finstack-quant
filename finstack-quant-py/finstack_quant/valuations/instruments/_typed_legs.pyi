"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_legs.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.cashflows.builder import RollRule
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.types import Bps, Rate

class FixedLegSpec:
    """
    Fixed leg of an interest-rate swap.

    Thin typed wrapper for the canonical Rust ``FixedLegSpec``. Used to build
    :class:`InterestRateSwap` and :class:`Swaption` instruments. Immutable:
    every constructor argument is readable back through a property of the
    same name; ``to_json`` / ``from_json`` (and therefore ``pickle``) round-trip
    the wire form.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FixedLegSpec
    >>> leg = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2024-01-15",
    ...     "2029-01-15",
    ... )
    >>> leg.rate
    0.04
    """

    def __init__(
        self,
        discount_curve_id: str,
        rate: float | Rate,
        frequency: Tenor,
        day_count: DayCount,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        *,
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        payment_lag_days: int = 0,
        end_of_month: bool = False,
    ) -> None:
        """
        Create a fixed leg.

        Parameters
        ----------
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        rate : float | Rate
            Fixed rate as a decimal (``0.04`` = 4%) or a ``Rate``.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the fixed leg (ISO 8601 strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the fixed leg.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        payment_lag_days : int, default 0
            Payment lag in business days after period end.
        end_of_month : bool, default False
            End-of-month roll convention.

        Raises
        ------
        ValueError
            If an enum value is invalid, ``rate`` is not finite, or the
            accrual period is malformed (``start >= end``).
        TypeError
            If ``rate`` is neither a number nor a ``Rate`` or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FixedLegSpec
        >>> leg = FixedLegSpec(
        ...     "USD-OIS",
        ...     0.04,
        ...     Tenor.semi_annual(),
        ...     DayCount.THIRTY_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ... )
        >>> leg.end_of_month
        False
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FixedLegSpec:
        """
        Deserialize a fixed-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``FixedLegSpec`` (the value
            ``FixedLegSpec.to_json`` returns).

        Returns
        -------
        FixedLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FixedLegSpec
        >>> leg = FixedLegSpec(
        ...     "USD-OIS",
        ...     0.04,
        ...     Tenor.semi_annual(),
        ...     DayCount.THIRTY_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ... )
        >>> FixedLegSpec.from_json(leg.to_json()).rate
        0.04
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`FixedLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the fixed-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``FixedLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``FixedLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rate(self) -> float:
        """
        Fixed rate as a decimal.

        Returns
        -------
        float
            ``0.04`` for 4%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Payment lag in business days after period end.

        Returns
        -------
        int
            Number of business days between accrual end and payment (``0`` = pay on period end).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end_of_month(self) -> bool:
        """
        End-of-month roll convention flag.

        Returns
        -------
        bool
            ``True`` when roll dates stick to month ends, ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class FloatLegSpec:
    """
    Floating leg of an interest-rate swap.

    Thin typed wrapper for the canonical Rust ``FloatLegSpec``. Used to build
    :class:`InterestRateSwap` and :class:`Swaption` instruments. Immutable:
    every constructor argument is readable back through a property of the
    same name; ``to_json`` / ``from_json`` (and therefore ``pickle``) round-trip
    the wire form. ``reset_lag_days`` defaults to ``0`` (fixing on the accrual
    start), so a swap whose first period starts on or after the valuation date
    prices off the forward curve without historical fixings.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FloatLegSpec
    >>> leg = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2024-01-15",
    ...     "2029-01-15",
    ... )
    >>> (leg.reset_lag_days, leg.compounding)
    (0, 'simple')
    """

    def __init__(
        self,
        discount_curve_id: str,
        forward_curve_id: str,
        spread_bp: float | Bps,
        frequency: Tenor,
        day_count: DayCount,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        *,
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        reset_lag_days: int = 0,
        fixing_calendar_id: str | None = None,
        compounding: Literal["simple"] | dict[str, dict[str, int]] | None = None,
        payment_lag_days: int = 0,
        end_of_month: bool = False,
    ) -> None:
        """
        Create a floating leg.

        Parameters
        ----------
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        forward_curve_id : str
            Forward curve identifier for rate projections.
        spread_bp : float | Bps
            Spread over the index in basis points (``25.0`` = 25bp) or a
            ``Bps``.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the floating leg (ISO 8601 strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the floating leg.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        reset_lag_days : int, default 0
            Reset lag in business days before each accrual start. ``0``
            (the Rust default) fixes on the accrual start date; use ``2``
            for a T-2 term index. :meth:`InterestRateSwap.from_conventions`
            applies the registered market default for an index. A fixing
            date before the valuation date requires a
            ``ScalarTimeSeries("FIXING:<forward_curve_id>", ...)`` in the
            market context.
        fixing_calendar_id : str, optional
            Calendar used for rate fixing (reset lag).
        compounding : str | dict, optional
            Coupon compounding; ``None`` means ``"simple"`` (term indices).
            Overnight RFR legs pass a struct variant such as
            ``{"compounded_in_arrears": {"lookback_days": 0}}``,
            ``{"compounded_with_observation_shift": {"shift_days": 0}}`` or
            ``{"compounded_with_rate_cutoff": {"cutoff_days": 0}}``.
        payment_lag_days : int, default 0
            Payment lag in business days after period end.
        end_of_month : bool, default False
            End-of-month roll convention.

        Raises
        ------
        ValueError
            If an enum value is invalid, ``compounding`` does not name a
            variant, ``spread_bp`` is not finite, or the accrual period is
            malformed (``start >= end``).
        TypeError
            If ``spread_bp`` is neither a number nor a ``Bps`` or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FloatLegSpec
        >>> ois = FloatLegSpec(
        ...     "USD-OIS",
        ...     "USD-SOFR",
        ...     0.0,
        ...     Tenor.annual(),
        ...     DayCount.ACT_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ...     compounding={"compounded_in_arrears": {"lookback_days": 0}},
        ... )
        >>> ois.compounding
        {'compounded_in_arrears': {'lookback_days': 0}}
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FloatLegSpec:
        """
        Deserialize a floating-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``FloatLegSpec`` (the value
            ``FloatLegSpec.to_json`` returns).

        Returns
        -------
        FloatLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FloatLegSpec
        >>> leg = FloatLegSpec(
        ...     "USD-OIS",
        ...     "USD-SOFR-3M",
        ...     0.0,
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ... )
        >>> FloatLegSpec.from_json(leg.to_json()).forward_curve_id
        'USD-SOFR-3M'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`FloatLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the floating-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``FloatLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``FloatLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def forward_curve_id(self) -> str:
        """
        Forward (projection) curve identifier.

        Returns
        -------
        str
            Curve id used to project the index.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spread_bp(self) -> float:
        """
        Spread over the index in basis points.

        Returns
        -------
        float
            ``25.0`` for 25bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def reset_lag_days(self) -> int:
        """
        Reset lag in business days before each accrual start.

        Returns
        -------
        int
            ``0`` fixes on the accrual start.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixing_calendar_id(self) -> str | None:
        """
        Fixing calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def compounding(self) -> str | dict[str, dict[str, int]]:
        """
        Coupon compounding in serde form.

        Returns
        -------
        str | dict[str, dict[str, int]]
            ``"simple"`` or a one-key dict for the compounded variants.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Payment lag in business days after period end.

        Returns
        -------
        int
            Number of business days between accrual end and payment (``0`` = pay on period end).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end_of_month(self) -> bool:
        """
        End-of-month roll convention flag.

        Returns
        -------
        bool
            ``True`` when roll dates stick to month ends, ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class PremiumLegSpec:
    """
    Premium (fixed coupon) leg of a CDS or CDS index.

    Thin typed wrapper for the canonical Rust ``PremiumLegSpec``. Used by
    :class:`CreditDefaultSwap` and :class:`CdsIndex` builders. Immutable;
    fields are readable through properties and ``to_json`` / ``from_json``
    (and therefore ``pickle``) round-trip the wire form.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import RollRule
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import PremiumLegSpec
    >>> leg = PremiumLegSpec(
    ...     "2024-03-20",
    ...     "2029-06-20",
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     100.0,
    ...     "USD-OIS",
    ...     roll_rule=RollRule.CDS_IMM,
    ... )
    >>> leg.coupon_bp
    100.0
    """

    def __init__(
        self,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        coupon_bp: float | Bps,
        discount_curve_id: str,
        *,
        roll_rule: RollRule,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
    ) -> None:
        """
        Create a premium leg.

        Parameters
        ----------
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of protection / premium accrual (ISO strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of protection / premium accrual.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        coupon_bp : float | Bps
            Contractual running coupon in basis points (``100.0`` = 100bp = 1%).
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        roll_rule : RollRule
            ``RollRule.CDS_IMM`` for the prescribed quarterly CDS 20th dates
            (requires quarterly frequency and a short-front stub at pricing).
            ``RollRule.NONE`` generates a bespoke schedule from ``frequency``
            and ``stub``. ``RollRule.IMM`` is rejected at pricing.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.

        Raises
        ------
        ValueError
            If an enum value is invalid or ``coupon_bp`` is not finite.
        TypeError
            If ``coupon_bp`` is neither a number nor a ``Bps`` or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import RollRule
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import PremiumLegSpec
        >>> leg = PremiumLegSpec(
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     100.0,
        ...     "USD-OIS",
        ...     roll_rule=RollRule.CDS_IMM,
        ... )
        >>> leg.discount_curve_id
        'USD-OIS'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> PremiumLegSpec:
        """
        Deserialize a premium-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``PremiumLegSpec`` (the value
            ``PremiumLegSpec.to_json`` returns).

        Returns
        -------
        PremiumLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import RollRule
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import PremiumLegSpec
        >>> leg = PremiumLegSpec(
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     100.0,
        ...     "USD-OIS",
        ...     roll_rule=RollRule.CDS_IMM,
        ... )
        >>> PremiumLegSpec.from_json(leg.to_json()).coupon_bp
        100.0
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`PremiumLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the premium-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``PremiumLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``PremiumLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def roll_rule(self) -> RollRule:
        """Premium roll-date grid.

        Returns
        -------
        RollRule
            ``RollRule.CDS_IMM`` for the standard CDS roll grid;
            ``RollRule.NONE`` for bespoke frequency/stub dates.

        Notes
        -----
        This accessor does not raise; it returns the stored rule.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def coupon_bp(self) -> float:
        """
        Contractual running coupon in basis points.

        Returns
        -------
        float
            ``100.0`` for 100bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...

class ProtectionLegSpec:
    """
    Protection (default-contingent) leg of a CDS or CDS index.

    Thin typed wrapper for the canonical Rust ``ProtectionLegSpec``. Used by
    :class:`CreditDefaultSwap` and :class:`CdsIndex` builders. Immutable;
    fields are readable through properties and ``to_json`` / ``from_json``
    (and therefore ``pickle``) round-trip the wire form.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
    >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
    >>> leg.recovery_rate
    0.4
    """

    def __init__(self, credit_curve_id: str, recovery_rate: float, settlement_delay: int) -> None:
        """
        Create a protection leg.

        Parameters
        ----------
        credit_curve_id : str
            Hazard/credit curve identifier for default probabilities.
        recovery_rate : float
            Recovery rate in ``[0.0, 1.0)`` (e.g. 0.4 = 40%).
        settlement_delay : int
            Settlement delay in business days between the credit event and the
            protection payment. Required: the lag is a contract term (ISDA North
            America uses 3, ISDA Europe uses 1); read it from
            ``ConventionRegistry().resolve_cds(...).settlement_days`` when the
            contract follows a standard convention.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is outside ``[0.0, 1.0)``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
        >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
        >>> leg.settlement_delay
        3
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> ProtectionLegSpec:
        """
        Deserialize a protection-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``ProtectionLegSpec`` (the value
            ``ProtectionLegSpec.to_json`` returns).

        Returns
        -------
        ProtectionLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
        >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
        >>> ProtectionLegSpec.from_json(leg.to_json()).credit_curve_id
        'ACME-CDS'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`ProtectionLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the protection-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``ProtectionLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``ProtectionLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def credit_curve_id(self) -> str:
        """
        Hazard / credit curve identifier.

        Returns
        -------
        str
            Curve id used for survival probabilities.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate as a decimal.

        Returns
        -------
        float
            Value in ``[0.0, 1.0)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_delay(self) -> int:
        """
        Settlement delay in business days after a credit event.

        Returns
        -------
        int
            Number of business days between default and protection settlement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
