"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/orchestrator.rs``."""

from __future__ import annotations

import datetime
from decimal import Decimal
from finstack_quant.core.cashflow import CFKind
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.cashflows.builder._schedule import CashFlowSchedule
from finstack_quant.cashflows.builder._specs.amortization import AmortizationSpec
from finstack_quant.cashflows.builder._specs.coupon import (
    CouponType,
    FixedCouponSpec,
    FloatingCouponSpec,
    StepUpCouponSpec,
)
from finstack_quant.cashflows.builder._specs.fees import FeeSpec
from finstack_quant.cashflows.builder._specs.schedule import PrincipalExchange

class CashFlowBuilder:
    """
    Fluent builder assembling a principal, coupon legs, fees, and principal
    events into a :class:`CashFlowSchedule`.

    Created only via :meth:`CashFlowSchedule.builder`; there is no direct
    constructor. Every chaining method mutates the builder in place and
    returns that same instance, matching the Rust ``&mut self`` fluent API.
    Configuration errors on individual calls (e.g. an invalid principal
    event sign) are deferred and surfaced from :meth:`build`.

    Window boundaries: coupon windows (:meth:`add_fixed_window`,
    :meth:`add_floating_window`, :meth:`fixed_to_float`,
    :meth:`float_margin_steps`) and payment windows
    (:meth:`add_payment_window`, :meth:`payment_split_program`) all cut the
    principal horizon, and coupon dates are generated independently between
    adjacent cut points. With the default front stub, the coupon dates before
    an off-cycle boundary roll back from it rather than from issue; with
    ``StubKind.NONE`` such a boundary is rejected. :meth:`step_up_cf` keeps
    one full-horizon grid instead. Snap off-cycle dates onto the accrual grid
    first when coupon dates must not move.

    Examples
    --------
    >>> import datetime
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import CashFlowSchedule, FixedCouponSpec, ScheduleParams
    >>> from finstack_quant.core.money import Money
    >>> schedule = (
    ...     CashFlowSchedule
    ...     .builder()
    ...     .principal(Money(1_000_000.0, "USD"), datetime.date(2025, 1, 15), datetime.date(2026, 1, 15))
    ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360()))
    ...     .build()
    ... )
    >>> len(schedule.get_flows())
    6
    """

    def principal(
        self,
        initial: Money,
        issue_date: datetime.date,
        maturity: datetime.date,
    ) -> CashFlowBuilder:
        """
        Set the principal amount and instrument horizon.

        Parameters
        ----------
        initial : Money
            Initial notional amount outstanding at issue.
        issue_date : datetime.date
            Issue date; also the date of the initial funding cashflow.
        maturity : datetime.date
            Final maturity date of the instrument.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If a date cannot be converted from the supplied value (raised
            immediately), or *issue_date* is not strictly before *maturity*
            (deferred and raised from :meth:`build`).
        """
        ...

    def principal_exchange(self, exchange: PrincipalExchange) -> CashFlowBuilder:
        """
        Select whether issue funding and maturity redemption notionals are emitted.

        Outstanding still starts at the :meth:`principal` initial amount for
        coupon math. Scheduled amortization and explicit principal events
        still emit. The default is
        :attr:`PrincipalExchange.INITIAL_AND_FINAL`.

        Parameters
        ----------
        exchange : PrincipalExchange
            ``INITIAL_AND_FINAL`` emits the issue draw and the redemption
            balloon on the lagged payment date. ``NONE`` tracks outstanding
            only (vanilla IRS / basis-swap convention).

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> import datetime
        >>> from decimal import Decimal
        >>> from finstack_quant.cashflows.builder import (
        ...     CashFlowSchedule,
        ...     FixedCouponSpec,
        ...     PrincipalExchange,
        ...     ScheduleParams,
        ... )
        >>> from finstack_quant.core.cashflow import CFKind
        >>> from finstack_quant.core.money import Money
        >>> schedule = (
        ...     CashFlowSchedule
        ...     .builder()
        ...     .principal(Money(1_000_000.0, "USD"), datetime.date(2025, 1, 15), datetime.date(2026, 1, 15))
        ...     .principal_exchange(PrincipalExchange.NONE)
        ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360()))
        ...     .build()
        ... )
        >>> all(cf.kind != CFKind.NOTIONAL for cf in schedule.get_flows())
        True
        """
        ...

    def amortization(self, spec: AmortizationSpec) -> CashFlowBuilder:
        """
        Configure amortization for the instrument notional.

        Parameters
        ----------
        spec : AmortizationSpec
            Amortization rule applied to the principal over the schedule.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If the amortization schedule is inconsistent with the
            principal set via :meth:`principal`. The error is deferred and
            raised from :meth:`build`.
        """
        ...

    def add_principal_event(
        self,
        date: datetime.date,
        payment_date: datetime.date,
        delta: Money,
        kind: CFKind | str,
        cash: Money | None = None,
    ) -> CashFlowBuilder:
        """
        Add a single principal event (draw, repayment, or exchange).

        Parameters
        ----------
        date : datetime.date
            Economic date from which principal changes interest accrual.
        payment_date : datetime.date
            Cash settlement date, independently adjusted from the economic date.
        delta : Money
            Outstanding delta; sign convention depends on *kind* (e.g.
            ``CFKind.NOTIONAL`` draws require ``delta >= 0``).
        kind : CFKind | str
            Classification for the emitted cashflow. Required; matches the
            Rust API, which has no default.
        cash : Money, optional
            Cash amount before the cashflow kind's sign handling. When omitted,
            defaults to ``-delta`` for amortization and ``delta`` for draws;
            emitted settlement cash then follows the kind's sign convention.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If *delta* violates the sign convention for *kind*, or *cash*
            is supplied in a different currency than *delta*. The error is
            deferred and raised from :meth:`build`.
        """
        ...

    def fixed_cf(self, spec: FixedCouponSpec) -> CashFlowBuilder:
        """
        Add a full-horizon fixed-rate coupon leg.

        Parameters
        ----------
        spec : FixedCouponSpec
            Fixed coupon rate, settlement, and schedule conventions.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If the coupon schedule cannot be generated between the
            principal's issue and maturity dates. The error is deferred
            and raised from :meth:`build`.
        """
        ...

    def floating_cf(self, spec: FloatingCouponSpec) -> CashFlowBuilder:
        """
        Add a full-horizon floating-rate coupon leg.

        Parameters
        ----------
        spec : FloatingCouponSpec
            Floating rate index, settlement, and schedule conventions.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If the coupon schedule cannot be generated between the
            principal's issue and maturity dates. The error is deferred
            and raised from :meth:`build`.
        """
        ...

    def step_up_cf(self, spec: StepUpCouponSpec) -> CashFlowBuilder:
        """
        Add a full-horizon step-up/step-down coupon leg.

        Parameters
        ----------
        spec : StepUpCouponSpec
            Initial rate, step schedule, and schedule conventions.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If the coupon schedule cannot be generated between the
            principal's issue and maturity dates. The error is deferred
            and raised from :meth:`build`.
        """
        ...

    def fee(self, spec: FeeSpec) -> CashFlowBuilder:
        """
        Add a fee specification (one-time fixed or periodic basis-point).

        Parameters
        ----------
        spec : FeeSpec
            Fixed or periodic basis-point fee specification.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If a periodic fee's schedule cannot be generated. The error is
            deferred and raised from :meth:`build`.
        """
        ...

    def add_fixed_window(
        self,
        start: datetime.date,
        end: datetime.date,
        spec: FixedCouponSpec,
    ) -> CashFlowBuilder:
        """
        Add a fixed-rate coupon over the half-open window ``[start, end)``.

        ``start`` and ``end`` cut the coupon horizon; see the window-boundary
        rule on :class:`CashFlowBuilder`.

        Parameters
        ----------
        start : datetime.date
            Window start date (inclusive).
        end : datetime.date
            Window end date (exclusive).
        spec : FixedCouponSpec
            Fixed coupon rate, settlement, and schedule conventions applied
            within the window.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If *start* is not strictly before *end*, or the coupon
            schedule cannot be generated over the window. The error is
            deferred and raised from :meth:`build`.
        """
        ...

    def add_floating_window(
        self,
        start: datetime.date,
        end: datetime.date,
        spec: FloatingCouponSpec,
    ) -> CashFlowBuilder:
        """
        Add a floating-rate coupon over the half-open window ``[start, end)``.

        ``start`` and ``end`` cut the coupon horizon; see the window-boundary
        rule on :class:`CashFlowBuilder`.

        Parameters
        ----------
        start : datetime.date
            Window start date (inclusive).
        end : datetime.date
            Window end date (exclusive).
        spec : FloatingCouponSpec
            Floating rate index, settlement, and schedule conventions
            applied within the window.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If *start* is not strictly before *end*, or the coupon
            schedule cannot be generated over the window. The error is
            deferred and raised from :meth:`build`.
        """
        ...

    def add_payment_window(
        self,
        start: datetime.date,
        end: datetime.date,
        split: CouponType,
    ) -> CashFlowBuilder:
        """
        Set the payment split (cash/PIK/split) over ``[start, end)``.

        ``start`` and ``end`` also cut the coupon horizon, so an off-cycle
        date moves the coupon dates around it; see the window-boundary rule
        on :class:`CashFlowBuilder`.

        Parameters
        ----------
        start : datetime.date
            Window start date (inclusive).
        end : datetime.date
            Window end date (exclusive).
        split : CouponType
            Settlement split applied to coupons accruing within the window.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If *start* is not strictly before *end*. The error is deferred
            and raised from :meth:`build`.
        """
        ...

    def payment_split_program(
        self,
        steps: list[tuple[datetime.date, CouponType]],
    ) -> CashFlowBuilder:
        """
        Configure a payment split program from ordered boundary dates.

        Parameters
        ----------
        steps : list[tuple[datetime.date, CouponType]]
            Ordered ``(end_date, split)`` pairs: each split applies up to (not
            including) its date, and periods after the last step pay cash.
            Step dates cut the coupon horizon; see the window-boundary rule
            on :class:`CashFlowBuilder`.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If the step dates are not strictly increasing. The error is
            deferred and raised from :meth:`build`.
        """
        ...

    def fixed_to_float(
        self,
        switch: datetime.date,
        fixed: FixedCouponSpec,
        floating: FloatingCouponSpec,
    ) -> CashFlowBuilder:
        """
        Switch from a fixed coupon to a floating coupon at ``switch``.

        ``switch`` cuts the coupon horizon, so each leg gets its own schedule;
        see the window-boundary rule on :class:`CashFlowBuilder`.

        Parameters
        ----------
        switch : datetime.date
            Date on which the floating leg begins (exclusive end of the
            fixed window).
        fixed : FixedCouponSpec
            Fixed rate, settlement type, and schedule for the pre-switch window.
        floating : FloatingCouponSpec
            Floating coupon spec for the post-switch window.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If ``switch`` is outside the principal horizon or a coupon
            schedule cannot be generated. The error is deferred and
            raised from :meth:`build`.
        """
        ...

    def float_margin_steps(
        self,
        steps: list[tuple[datetime.date, Decimal]],
        base_spec: FloatingCouponSpec,
    ) -> CashFlowBuilder:
        """
        Floating coupons whose margin changes on effective-from ``steps``.

        Parameters
        ----------
        steps : list[tuple[datetime.date, decimal.Decimal]]
            ``(effective_from, spread_bp)`` pairs in strictly increasing date
            order. Each spread (bp over the index, ``250`` = 2.50%) replaces
            the base spread from its date until the next step; the last runs
            to maturity. An off-cycle date regenerates the coupon dates on
            both sides of it; see the window-boundary rule on
            :class:`CashFlowBuilder`.
        base_spec : FloatingCouponSpec
            Base floating spec; its ``spread_bp`` applies from issue until
            the first step.

        Returns
        -------
        CashFlowBuilder
            This same builder, for chaining.

        Raises
        ------
        ValueError
            If step dates are not strictly increasing, a step lies outside
            the principal horizon, or a coupon schedule cannot be generated.
            The error is deferred and raised from :meth:`build`.
        """
        ...

    def build(self, market: MarketContext | None = None) -> CashFlowSchedule:
        """
        Compile the configured legs into a canonical cashflow schedule.

        Parameters
        ----------
        market : MarketContext, optional
            Market context supplying observed fixings and forward curves for
            floating coupons. Every floating coupon needs its forward curve,
            even when the fixing series covers it, unless an explicit
            fallback applies. Without ``as_of`` each forward curve's base date
            is the observation cutoff. Fixed coupons and deterministic fees
            do not require a market context.

        Returns
        -------
        CashFlowSchedule
            The compiled, deterministically ordered cashflow schedule.

        Raises
        ------
        KeyError
            If required inputs are missing, such as an unset principal or a
            forward curve needed for an unobserved fixing with fallback ERROR.
        ValueError
            If a spec or date validation fails, including any deferred
            error recorded by an earlier fluent call or a negative reset lag.
            Invalid accrual grids include end-of-month rolling with either
            IMM rule, ACT/ACT ICMA with third-Wednesday IMM, and a plain
            ACT/ACT ICMA end-of-month grid whose regular anchor is not month-end.
            Supplied fixing history must contain exact-date observations
            before the forward curve's base date, under every fallback policy.
            An explicit fallback replaces the whole coupon when its projection
            fails (missing curve, absent fixing series, a reset before the
            curve base date, or a projection day-count error); errors outside
            coupon projection still propagate.
        RuntimeError
            If an operational or numerical computation failure occurs during
            construction; a fallback policy does not suppress it.
        """
        ...

    def build_as_of(self, market: MarketContext, as_of: datetime.date | str) -> CashFlowSchedule:
        """Build cashflows with an explicit historical-observation cutoff.

        Parameters
        ----------
        market : MarketContext
            Projection curves and exact historical index fixings.
        as_of : datetime.date
            Earlier resets require fixings independently of curve base dates;
            available same-day fixings take precedence over projection.

        Returns
        -------
        CashFlowSchedule
            Canonical cashflows containing realized and projected coupons.

        Raises
        ------
        KeyError
            If principal or required market data is missing.
        ValueError
            If dates, schedule inputs, or required historical fixings are invalid.
        RuntimeError
            If numerical computation fails during cashflow construction.
        """
        ...

    def __repr__(self) -> str: ...
