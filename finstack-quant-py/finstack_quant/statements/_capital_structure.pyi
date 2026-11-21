"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/capital_structure.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

import pandas as pd
import polars as pl
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable

class EcfSweepSpec:
    """
    Excess Cash Flow sweep specification.

    The agreement's formula is elected in full. Nothing is deducted
    automatically::

        ECF     = cash_measure - sum(deductions) + sum(credits)
        request = sweep_percentage * ECF   when ECF is positive, otherwise zero

    The request is paid at par from the cash the earlier priorities left, up to
    the eligible principal net of reserves. It is sized, paid and applied to
    principal in the same reporting period.

    Examples
    --------
    >>> from finstack_quant.statements import EcfSweepSpec
    >>> sweep = EcfSweepSpec(
    ...     cash_measure="ebitda",
    ...     deductions=[{"type": "input", "expression": "capex"}, {"type": "paid_cash_interest"}],
    ...     sweep_percentage=0.5,
    ...     facilities="all_eligible",
    ... )
    >>> (sweep.cash_measure, sweep.sweep_percentage, sweep.facilities)
    ('ebitda', 0.5, 'all_eligible')

    """

    def __init__(
        self,
        *,
        cash_measure: str,
        sweep_percentage: float,
        facilities: str | Mapping[str, list[str]],
        deductions: list[Mapping[str, str]] = ...,
        credits: list[Mapping[str, str]] = ...,
        payment_lag_days: int | None = None,
    ) -> None:
        """
        Configure an excess-cash-flow debt sweep from its canonical fields.

        The keywords are read as one mapping into the Rust ``EcfSweepSpec``,
        which rejects an unknown keyword.

        Parameters
        ----------
        cash_measure : str
            Node or formula of the agreement's cash measure before deductions,
            signed, in the waterfall currency.
        sweep_percentage : float
            Fraction of positive ECF requested, a decimal in ``[0, 1]`` (0.5
            is 50%).
        facilities : str or Mapping[str, list[str]]
            ``"all_eligible"`` (every term loan and revolver) or
            ``{"named": [instrument_id, ...]}``.
        deductions : list[Mapping[str, str]], default []
            Terms subtracted from the cash measure, each
            ``{"type": "input", "expression": node_or_formula}``,
            ``{"type": "paid_cash_interest"}``, ``{"type": "paid_fees"}`` or
            ``{"type": "paid_scheduled_principal"}``. A paid term is the cash
            the priorities ahead of the sweep actually allocated in the
            period. Nothing is deducted unless it is listed.
        credits : list[Mapping[str, str]], default []
            Terms added to the cash measure, in the same form. No expression
            may depend on a same-period ``cs`` component.
        payment_lag_days : int or None, default None
            Days from the close of the measured period to the date the sweep
            is due. When set, the measured amount is a dated obligation: it
            is paid in the period that contains its due date, and what cannot
            be paid is carried as arrears and paid first at the next sweep.
            ``0`` is due at the close of the measured period. ``None`` pays
            the sweep in the period it is measured and reports, without
            carrying, an unpaid amount.

        Raises
        ------
        ValueError
            If a required keyword is missing, a keyword is unknown or a term
            has an unknown type. The percentage, the
            facility list and the expressions are checked by :meth:`validate`
            and when the model is built.
        """
        ...
    @staticmethod
    def from_json(json: str) -> EcfSweepSpec:
        """
        Parse an ECF sweep specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the sweep node identifiers and percentage.

        Returns
        -------
        EcfSweepSpec
            Validated `EcfSweepSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or cannot be deserialized as an excess-cash-flow sweep specification.

        Examples
        --------
        >>> from finstack_quant.statements import EcfSweepSpec
        >>> sweep = EcfSweepSpec(cash_measure="ebitda", sweep_percentage=0.5, facilities="all_eligible")
        >>> EcfSweepSpec.from_json(sweep.to_json()).sweep_percentage
        0.5

        """
        ...
    def validate(self) -> None:
        """
        Validate the sweep on its own (the Rust ``EcfSweepSpec::validate``).

        :meth:`WaterfallSpec.validate` checks the pairing between this
        configuration and the ``sweep`` priority.

        Raises
        ------
        ValueError
            If ``sweep_percentage`` is outside ``[0.0, 1.0]`` or not finite.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `EcfSweepSpec` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `EcfSweepSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def cash_measure(self) -> str:
        """
        Node or formula of the agreement's cash measure before deductions.

        Returns
        -------
        str
            A node id (``"ebitda"``) or an expression over nodes, evaluated
            per period as a signed amount in the waterfall currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deductions(self) -> list[dict[str, Any]]:
        """
        The terms subtracted from the cash measure, in order.

        Returns
        -------
        list[dict[str, Any]]
            Each ``{"type": "input", "expression": ...}``,
            ``{"type": "paid_cash_interest"}``, ``{"type": "paid_fees"}`` or
            ``{"type": "paid_scheduled_principal"}``.

        Raises
        ------
        ValueError
            If the stored terms cannot be serialized.
        """
        ...

    @property
    def credits(self) -> list[dict[str, Any]]:
        """
        The terms added to the cash measure, in order.

        Returns
        -------
        list[dict[str, Any]]
            Terms in the same form as :attr:`deductions`.

        Raises
        ------
        ValueError
            If the stored terms cannot be serialized.
        """
        ...

    @property
    def payment_lag_days(self) -> int | None:
        """
        Days from the close of the measured period to the sweep's due date.

        Returns
        -------
        int | None
            The lag in calendar days, or ``None`` when the sweep is paid in
            the period it is measured and an unpaid amount is not carried.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def facilities(self) -> str | dict[str, list[str]]:
        """
        The facilities the sweep may repay.

        Returns
        -------
        str | dict[str, list[str]]
            ``"all_eligible"`` or ``{"named": [instrument_id, ...]}``.

        Raises
        ------
        ValueError
            If the stored selection cannot be serialized.
        """
        ...

    @property
    def sweep_percentage(self) -> float:
        """
        Fraction of excess cash flow swept to debt paydown.

        Returns
        -------
        float
            A **decimal fraction in [0, 1]**, not a percentage — 0.5 means a
            50% sweep. Values outside the unit interval are rejected by
            :meth:`WaterfallSpec.validate`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PikToggleSpec:
    """
    PIK toggle: the modeled exercise of a borrower's PIK election.

    In each forecast period the liquidity metric is compared with the
    threshold: a metric below it requests PIK, a metric at or above it
    requests the loan's base coupon terms. The request takes effect at the
    start of each listed loan's next coupon period after the period close,
    never on a coupon already running, through the loan's own coupon terms:
    the elected share of each covered coupon is zero cash and capitalizes
    once, on the coupon date. Overdue cash interest and fees stay cash claims.
    Actual periods are not exercised; record a historical election on the
    loan itself.

    Examples
    --------
    >>> from finstack_quant.statements import PikToggleSpec
    >>> toggle = PikToggleSpec(
    ...     liquidity_metric="cash",
    ...     threshold=100.0,
    ...     elections=[{"instrument_id": "TL", "pik_fraction": "0.4"}],
    ... )
    >>> (toggle.liquidity_metric, toggle.elections[0]["instrument_id"])
    ('cash', 'TL')

    """

    def __init__(
        self,
        *,
        liquidity_metric: str,
        threshold: float,
        elections: list[Mapping[str, Any]],
    ) -> None:
        """
        Configure a liquidity-triggered PIK election.

        The keywords are read as one mapping into the Rust ``PikToggleSpec``,
        which rejects an unknown keyword.

        Parameters
        ----------
        liquidity_metric : str
            Node or formula compared with the threshold. It must evaluate to
            a finite number in every forecast period.
        threshold : float
            Finite threshold in the metric's units: a metric below it requests
            PIK, a metric at or above it requests the base coupon terms.
        elections : list[Mapping[str, Any]]
            One entry per term loan authorized to elect PIK:
            ``{"instrument_id": str, "pik_fraction": str, "notice_days": int,
            "min_coupon_periods": int}``.
            ``pik_fraction`` is the share of each elected coupon that
            capitalizes, an exact decimal string in ``(0, 1]`` (``"0.4"`` is
            40% PIK and 60% cash, ``"1"`` is full PIK). ``notice_days``
            (default 0) is the notice an election needs: it takes effect at
            the first coupon period starting at least that many days after
            the close of the period that observed the metric.
            ``min_coupon_periods``
            (default 0) is the minimum number of consecutive coupon periods an
            election lasts, counting the first, measured in the loan's coupon
            periods and not in reporting periods.

        Raises
        ------
        ValueError
            If a required keyword is missing, a keyword or an election field
            is unknown, or a value has the wrong type. The threshold, the
            fractions and the list are checked by :meth:`validate`; model
            validation rejects an election that does not name a term loan of
            the capital structure.
        """
        ...
    @staticmethod
    def from_json(json: str) -> PikToggleSpec:
        """
        Parse a PIK-toggle specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the liquidity trigger and target instruments.

        Returns
        -------
        PikToggleSpec
            Parsed specification; waterfall/model validation subsequently checks
            that targets are an explicit nonempty list of borrowing debt.

        Raises
        ------
        ValueError
            If ``json`` is malformed or cannot be deserialized as a PIK-toggle specification.

        Examples
        --------
        >>> from finstack_quant.statements import PikToggleSpec
        >>> toggle = PikToggleSpec(
        ...     liquidity_metric="cash", threshold=100.0, elections=[{"instrument_id": "TL", "pik_fraction": "0.4"}]
        ... )
        >>> PikToggleSpec.from_json(toggle.to_json()).threshold
        100.0

        """
        ...
    def validate(self) -> None:
        """
        Validate the toggle on its own (the Rust ``PikToggleSpec::validate``).

        Twin of the WASM ``statements.validatePikToggleSpecJson``.

        Raises
        ------
        ValueError
            If the threshold is not finite, the metric is empty, no election
            is listed, a loan is listed twice, or a ``pik_fraction`` is
            outside ``(0, 1]``.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize `PikToggleSpec` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PikToggleSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def liquidity_metric(self) -> str:
        """
        Node reference or DSL formula producing the liquidity signal.

        Returns
        -------
        str
            A node id (``"cash_balance"``) or an expression
            (``"ebitda / interest_expense"``). Whether the value is monetary
            or a unitless ratio depends on the expression — the threshold must
            use the same units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def threshold(self) -> float:
        """
        Level below which interest accrues as PIK instead of cash.

        Returns
        -------
        float
            Threshold in the **same units as the liquidity metric** — a
            currency amount when the metric is a balance, a unitless ratio
            when it is a coverage ratio. A period requests PIK when
            ``metric < threshold``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    @property
    def elections(self) -> list[dict[str, Any]]:
        """
        The loans authorized to elect PIK, one mapping each.

        Returns
        -------
        list[dict[str, Any]]
            ``instrument_id``, ``pik_fraction`` (an exact decimal string in
            ``(0, 1]``), ``notice_days`` and ``min_coupon_periods`` (a count
            of the loan's coupon periods) for each authorized term loan.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PaymentClassSpec:
    """
    Seniority class for intra-category waterfall allocation.

    When attached to :class:`WaterfallSpec`, each payment category walks class
    rank and allocates pro-rata inside a class before the next class sees
    remaining cash.

    Examples
    --------
    >>> from finstack_quant.statements import PaymentClassSpec
    >>> cls = PaymentClassSpec("1L", 0, ["TL-A"])
    >>> (cls.id, cls.rank, cls.instrument_ids)
    ('1L', 0, ['TL-A'])

    """

    def __init__(self, id: str, rank: int, instrument_ids: list[str]) -> None:
        """
        Construct a payment class.

        Parameters
        ----------
        id : str
            Class identifier (for example ``"1L"``). Must be unique within a
            waterfall.
        rank : int
            Seniority rank; ``0`` is most senior. Ranks must be unique.
        instrument_ids : list[str]
            Debt instrument ids in this class. Each instrument may appear in
            at most one class.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    @staticmethod
    def from_json(json: str) -> PaymentClassSpec:
        """
        Parse a payment class from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing ``id``, ``rank``, and ``instrument_ids``.

        Returns
        -------
        PaymentClassSpec
            Instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or cannot be deserialized as a payment class.

        Examples
        --------
        >>> from finstack_quant.statements import PaymentClassSpec
        >>> cls = PaymentClassSpec("1L", 0, ["TL-A"])
        >>> PaymentClassSpec.from_json(cls.to_json()).id
        '1L'

        """
        ...
    def to_json(self) -> str:
        """
        Serialize `PaymentClassSpec` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PaymentClassSpec`, suitable
            for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def id(self) -> str:
        """
        Class identifier (for example ``"1L"``).

        Returns
        -------
        str
            Unique class id within the enclosing waterfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rank(self) -> int:
        """
        Seniority rank; ``0`` is most senior.

        Returns
        -------
        int
            Unique rank used to order classes when allocating a category.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_ids(self) -> list[str]:
        """
        Debt instrument ids that belong to this class.

        Returns
        -------
        list[str]
            Instrument ids allocated together inside this class.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class WaterfallSpec:
    """
    Waterfall specification for dynamic cash flow allocation.

    The default order pays fees, interest, scheduled principal, and equity.
    Prepayments require an explicit priority and sizing configuration.
    Call :meth:`validate` before passing to a builder to surface inconsistent
    configurations (for example ``sweep`` ordered after ``equity``).

    Examples
    --------
    >>> from finstack_quant.statements import WaterfallSpec
    >>> waterfall = WaterfallSpec()
    >>> waterfall.priority_of_payments
    ['fees', 'interest', 'amortization', 'equity']

    """

    def __init__(
        self,
        priority_of_payments: list[str] | None = None,
        available_cash_node: str | None = None,
        ecf_sweep: EcfSweepSpec | None = None,
        pik_toggle: PikToggleSpec | None = None,
        payment_classes: list[PaymentClassSpec] | None = None,
        mandatory_prepayment: Mapping[str, Any] | None = None,
        voluntary_prepayment: Mapping[str, Any] | None = None,
    ) -> None:
        """
        Configure dynamic cash allocation for a financial-model waterfall.

        Parameters
        ----------
        priority_of_payments : list[str] or None, default None
            Ordered payment labels, from highest to lowest priority. ``None``
            selects ``fees``, ``interest``, ``amortization``, and ``equity``.
        available_cash_node : str or None, default None
            Node or formula of the cash pool before debt service; ``None``
            uses ``"cash"``. It may not depend on a same-period ``cs``
            component, directly or through other nodes.
        ecf_sweep : EcfSweepSpec or None, default None
            Optional excess-cash-flow sweep applied within the waterfall.
            List ``sweep`` explicitly before any ``equity`` priority to
            allocate excess cash to principal.
        pik_toggle : PikToggleSpec or None, default None
            Optional liquidity-driven PIK versus cash-interest configuration.
        payment_classes : list[PaymentClassSpec] or None, default None
            Intra-category seniority classes; ``None`` or empty is one implicit
            class (pro-rata across all instruments).
        mandatory_prepayment : Mapping[str, Any] or None, default None
            The at-par mandatory prepayment, ``{"amount": node_or_formula,
            "facilities": "all_eligible" | {"named": [...]}}``. The amount is
            the non-negative request per period in the waterfall currency; a
            negative request fails the period. Required when
            ``mandatory_prepayment`` is a listed priority, and invalid without
            it.
        voluntary_prepayment : Mapping[str, Any] or None, default None
            The at-par voluntary prepayment, in the same form. Required when
            ``voluntary_prepayment`` is a listed priority, and invalid without
            it.

        Raises
        ------
        ValueError
            If priority_of_payments contains an unknown priority name, or a
            prepayment mapping has a missing or unknown key.

        """
        ...
    @staticmethod
    def from_json(json: str) -> WaterfallSpec:
        """
        Parse a waterfall specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing priority, cash source, and optional features.

        Returns
        -------
        WaterfallSpec
            Validated `WaterfallSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or cannot be deserialized as a waterfall specification.

        Examples
        --------
        >>> from finstack_quant.statements import WaterfallSpec
        >>> waterfall = WaterfallSpec()
        >>> WaterfallSpec.from_json(waterfall.to_json()).has_ecf_sweep
        False

        """
        ...
    def to_json(self) -> str:
        """
        Serialize `WaterfallSpec` to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `WaterfallSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def validate(self) -> None:
        """
        Validate the spec against internal consistency rules.

        Rejects duplicate priorities, a non-terminal ``equity`` entry, a
        sweep percentage outside [0, 1], an empty PIK target list, mismatched
        prepayment priority and sizing configuration pairs, and a stack missing
        ``fees``, ``interest``, or ``amortization``. Model evaluation resolves
        node and instrument references.

        Raises
        ------
        ValueError
            If the configuration is economically inconsistent.
        """
        ...

    @property
    def priority_of_payments(self) -> list[str]:
        """
        Payment priority order, highest priority first.

        Returns
        -------
        list[str]
            Snake-case priority names in allocation order — cash is applied to
            the first entry before any of the next. Allocation *within* a
            category is pro-rata inside each payment class, walking class rank.
            Empty ``payment_classes`` is one implicit class.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def available_cash_node(self) -> str:
        """
        Node reference or DSL formula for the cash pool the waterfall may spend.

        Returns
        -------
        str
            A node id or expression evaluating to a monetary amount per
            period. This is the **pre-waterfall** cash pool; do not deduct
            ``cs`` debt-service tokens here. Defaults to ``"cash"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_classes(self) -> list[PaymentClassSpec]:
        """
        Intra-category seniority classes.

        Returns
        -------
        list[PaymentClassSpec]
            Configured classes. Empty means one implicit class: pro-rata
            across all instruments in each category.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mandatory_prepayment(self) -> dict[str, Any] | None:
        """
        The mandatory prepayment's amount and eligible facilities.

        Returns
        -------
        dict[str, Any] | None
            ``{"amount": ..., "facilities": ...}``, or ``None`` when that
            priority is unused.

        Raises
        ------
        ValueError
            If the stored specification cannot be serialized.
        """
        ...

    @property
    def voluntary_prepayment(self) -> dict[str, Any] | None:
        """
        The voluntary prepayment's amount and eligible facilities.

        Returns
        -------
        dict[str, Any] | None
            ``{"amount": ..., "facilities": ...}``, or ``None`` when that
            priority is unused.

        Raises
        ------
        ValueError
            If the stored specification cannot be serialized.
        """
        ...

    @property
    def has_ecf_sweep(self) -> bool:
        """
        Whether an excess-cash-flow sweep is configured.

        Returns
        -------
        bool
            ``True`` when an :class:`EcfSweepSpec` is attached.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def has_pik_toggle(self) -> bool:
        """
        Whether a PIK toggle is configured.

        Returns
        -------
        bool
            ``True`` when a :class:`PikToggleSpec` is attached.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    @property
    def ecf_sweep(self) -> EcfSweepSpec | None:
        """Return the configured excess-cash-flow sweep.

        Returns
        -------
        EcfSweepSpec | None
            Sweep configuration paired with the ``sweep`` priority, or ``None``
            when no ECF sweep was configured.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def pik_toggle(self) -> PikToggleSpec | None:
        """Return the configured liquidity-based PIK toggle.

        Returns
        -------
        PikToggleSpec | None
            PIK signal, threshold and per-loan elections, or None when omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

class CapitalStructureCashflows:
    """Per-instrument, per-period debt cashflows produced by an evaluation.

    Amounts are floats stated in the model's reporting currency, reported by
    :attr:`reporting_currency`.

    Examples
    --------
    >>> from finstack_quant.statements import CapitalStructureCashflows
    >>> payload = (
    ...     '{"by_instrument":{},"totals":{},"totals_by_currency":{},'
    ...     '"reporting_currency":"USD","equity_distribution":{}}'
    ... )
    >>> CapitalStructureCashflows.from_json(payload).instrument_ids
    []

    """

    @property
    def instrument_ids(self) -> list[str]:
        """
        Debt instrument identifiers present in the cashflows.

        This property does not raise.

        Returns
        -------
        list[str]
            Debt instrument identifiers present in the cashflows.
        """
        ...

    @property
    def periods(self) -> list[str]:
        """
        Period ids covered, in model order.

        This property does not raise.

        Returns
        -------
        list[str]
            Period ids covered, in model order.
        """
        ...

    @property
    def reporting_currency(self) -> str | None:
        """
        ISO-4217 code the amounts are stated in, or ``None`` when scalar.

        This property does not raise.

        Returns
        -------
        str | None
            ISO-4217 code the amounts are stated in, or ``None`` when scalar.
        """
        ...

    @property
    def equity_distribution(self) -> dict[str, Money]:
        """
        Residual cash distributed to equity, keyed by period id.

        The cash left after fees, interest and principal when ``equity`` is
        the waterfall's terminal priority, and zero when it is not.

        This property does not raise.

        Returns
        -------
        dict[str, Money]
            Period id to ``Money``; empty unless a waterfall ran.
        """
        ...

    @property
    def fx_conversions(self) -> dict[str, list[dict[str, Any]]]:
        """
        FX conversions applied to consolidate each period's totals.

        Returns
        -------
        dict[str, list[dict[str, Any]]]
            Period id to its conversions, each ``{"from": currency code,
            "to": currency code, "rate": float, "date": ISO date, "source":
            "direct" | "triangulated"}``. ``rate`` is units of ``to`` per unit
            of ``from`` on the inclusive period-close ``date``. Empty when
            every instrument is in the reporting currency.

        Raises
        ------
        ValueError
            If the stored records cannot be serialized.
        """
        ...

    @property
    def consolidation_unavailable(self) -> dict[str, str]:
        """
        Why a period has no consolidated total, by period id.

        Returns
        -------
        dict[str, str]
            Period id to the reason consolidation could not be completed, for
            example a missing period-close FX quote or no reporting currency
            for instruments in more than one currency. Per-instrument and
            native-currency values of those periods are unaffected. Empty when
            every period consolidated.

        Raises
        ------
        ValueError
            If the stored reasons cannot be serialized.
        """
        ...

    @property
    def prepayments(self) -> dict[str, list[dict[str, Any]]]:
        """
        What each prepayment requested, paid and left unpaid, by period id.

        Returns
        -------
        dict[str, list[dict[str, Any]]]
            Period id to the period's prepayments in priority order, each
            ``{"kind": "mandatory_prepayment" | "voluntary_prepayment" |
            "sweep", "requested": money, "paid": money, "unpaid": money,
            "carried": money}`` with money as ``{"amount": ..., "currency":
            ...}``. ``paid`` is the request limited by the cash left and the
            eligible principal net of reserves; ``carried`` is the part of
            ``unpaid`` carried as arrears: all of it for a sweep with
            ``payment_lag_days``, zero otherwise.
            Empty unless a waterfall with a prepayment priority ran.

        Raises
        ------
        ValueError
            If the stored outcomes cannot be serialized.
        """
        ...

    @property
    def retained_cash(self) -> dict[str, Money]:
        """
        Residual cash the company retains, keyed by period id.

        The cash left after the payment stack when ``equity`` is not one of
        the waterfall's priorities, and zero when it is.

        This property does not raise.

        Returns
        -------
        dict[str, Money]
            Period id to ``Money``; empty unless a waterfall ran.
        """
        ...

    @property
    def available_cash(self) -> dict[str, Money]:
        """
        Cash available to the waterfall, keyed by period id.

        The value of the waterfall's ``available_cash_node``, in the
        waterfall's currency; a negative pool is a funding shortfall and
        allocates as zero. It reconciles the period's uses exactly:
        ``fees + cash interest + principal_payment + equity_distribution +
        retained_cash == available_cash``.

        This property does not raise.

        Returns
        -------
        dict[str, Money]
            Period id to ``Money``; empty unless a waterfall ran.
        """
        ...

    @property
    def distress_cash(self) -> dict[str, list[dict[str, Any]]]:
        """
        Distress cash by period id: recoveries, accrual paid on default and protection payments.

        Distress cash is outside scheduled debt service. It is in no
        breakdown, total or waterfall claim and changes no principal; an
        explicit write-down moves ``debt_balance`` on its own date.

        This property does not raise.

        Returns
        -------
        dict[str, list[dict[str, Any]]]
            Period id to its rows in schedule order. Each row has
            ``instrument_id``, ``payment_date`` (ISO date of the actual
            payment), ``kind`` (``"recovery"``, ``"accrued_on_default"`` or
            ``"protection_payment"``) and ``amount`` (``{"amount", "currency"}``
            in the instrument's native currency, signed for the company: a
            receipt is positive and a payment negative). Only periods with a
            distress row have an entry.
        """
        ...

    def get_component(self, component: str, instrument_or_total: str, period_id: str) -> float:
        """Read one reported capital-structure amount for a scope and period.

        Parameters
        ----------
        component : str
            One of ``interest_expense`` (cash plus PIK), ``interest_expense_cash``,
            ``interest_expense_pik``, ``interest_income``,
            ``net_interest_expense_cash`` (cash expense less cash income),
            ``principal_payment`` and its parts ``scheduled_principal``,
            ``mandatory_prepayment``, ``voluntary_prepayment`` and
            ``sweep_prepayment``, ``scheduled_cash_interest_due`` and
            ``scheduled_principal_due`` (scheduled service recognized once on
            its contractual due date, paid or not), ``fees``,
            ``opening_balance``,
            ``debt_balance``, ``accrued_interest`` (cash interest earned and
            not yet due), ``interest_arrears`` (cash interest due and unpaid),
            ``principal_pending`` (principal cash owed and not yet due),
            ``principal_arrears`` (principal cash due and unpaid),
            ``principal_advance_reserve`` (face paid for and not yet reduced),
            the receipts ``fee_rebates`` and ``repayment_refunds``, or the
            funding amounts ``funding_proceeds`` (signed company cash of
            funding settled in the period) and ``funding_principal`` (the
            principal that funding established).
            Flows are positive payment or receipt magnitudes; balances are
            period-close economic principal on the contract's effective dates.
        instrument_or_total : str
            The scope: an instrument identifier for its native-currency
            amount, ``"total"`` for the consolidated total in the reporting
            currency, or ``"total.<CCY>"`` (for example ``"total.EUR"``) for
            the total of the instruments in one native currency. No FX
            conversion is performed by this accessor.
        period_id : str
            Reporting-period label such as ``"2025Q1"`` present in the result.

        Returns
        -------
        float
            The amount in major units of the scope's currency. Absent optional
            interest income is zero.

        Raises
        ------
        ValueError
            If ``period_id`` is not a valid reporting-period label.
        RuntimeError
            If the component or scope is unknown or malformed, the scope has
            no data for the period (including a consolidated total that could
            not be formed, with the reason), or the breakdown does not record
            the component: an unknown amount is never read as zero.
        """
        ...

    def get_money(self, component: str, scope: str, period_id: str) -> Money:
        """Read one reported capital-structure amount as exact money.

        The typed twin of :meth:`get_component`: the same component and scope
        names, returning the exact amount with its currency (Rust
        ``CapitalStructureCashflows::get_money``).

        Parameters
        ----------
        component : str
            A component name accepted by :meth:`get_component`, for example
            ``"interest_expense_cash"`` or ``"debt_balance"``.
        scope : str
            An instrument identifier for its native-currency amount,
            ``"total"`` for the consolidated total in the reporting currency,
            or ``"total.<CCY>"`` for the total of the instruments in one
            native currency. No FX conversion is performed.
        period_id : str
            Reporting-period label such as ``"2025Q1"`` present in the result.

        Returns
        -------
        Money
            The exact amount in the scope's currency.

        Raises
        ------
        ValueError
            If ``period_id`` is not a valid reporting-period label.
        RuntimeError
            If the component or scope is unknown or malformed, the scope has
            no data for the period (including a consolidated total that could
            not be formed, with the reason), or the breakdown does not record
            the component.

        Examples
        --------
        >>> from finstack_quant.statements import CapitalStructureCashflows
        >>> flows = CapitalStructureCashflows.from_json(
        ...     '{"by_instrument": {}, "totals": {}, "reporting_currency": null}'
        ... )
        >>> flows.get_money("fees", "total", "2025Q1")
        Traceback (most recent call last):
            ...
        RuntimeError: Capital structure error: No total cashflow data for period 2025Q1
        """
        ...

    @property
    def totals_by_currency(self) -> dict[str, dict[str, dict[str, Money]]]:
        """Cross-instrument totals bucketed by native instrument currency.

        Populated for every evaluation with capital-structure flows, including
        multi-currency structures without a reporting currency, where
        ``to_totals_table`` and the ``get_component(..., "total", ...)`` lookups have nothing
        to report.

        Returns
        -------
        dict[str, dict[str, dict[str, Money]]]
            ISO currency code to period id to flow type (the ``flow_type``
            names of :meth:`to_arrow`) to amount in that currency.

        This accessor does not raise; it is empty when no capital-structure
        flows were evaluated.

        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Long ``ArrowTable`` of per-instrument flows.

        Returns
        -------
        ArrowTable
            Columns ``instrument``, ``period``, ``flow_type``, ``value`` (float
            view), ``amount`` (the exact amount as decimal text) and
            ``currency``. ``flow_type`` is one of
            ``interest_expense_cash``, ``interest_income_cash`` (when
            present), ``interest_expense_pik``, ``principal_payment``,
            ``fees``, ``debt_balance``, ``accrued_interest`` and — on
            evaluator-produced cashflows — ``opening_balance`` (debt balance
            at period start) plus the four sources of ``principal_payment``:
            ``scheduled_principal``, ``mandatory_prepayment``,
            ``sweep_prepayment`` and ``voluntary_prepayment``. The four
            source rows sum to the ``principal_payment`` row, so do not add
            them together. The last evaluator-produced rows are
            ``interest_arrears`` (cash interest due and unpaid at the close),
            ``principal_pending``, ``principal_arrears`` and
            ``principal_advance_reserve``.

        Raises
        ------
        ValueError
            If the table cannot be constructed.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """Long pandas ``DataFrame`` of per-instrument flows.

        Returns
        -------
        pandas.DataFrame
            Columns matching :meth:`to_arrow`.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the frame cannot be constructed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Long Polars ``DataFrame`` of per-instrument flows.

        Returns
        -------
        polars.DataFrame
            Columns matching :meth:`to_arrow`.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the frame cannot be constructed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Long ``pyarrow.Table`` of per-instrument flows.

        Returns
        -------
        pyarrow.Table
            Columns matching :meth:`to_arrow`.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the table cannot be constructed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_totals_table(self) -> ArrowTable:
        """Long ``ArrowTable`` of per-period totals across all instruments.

        Returns
        -------
        ArrowTable
            Columns ``period``, ``flow_type``, ``value`` (float view),
            ``amount`` (the exact amount as decimal text) and ``currency``.

        Raises
        ------
        ValueError
            If the table cannot be constructed.
        """
        ...

    def to_json(self) -> str:
        """Serialize to canonical JSON.

        Returns
        -------
        str
            Payload accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.

        """
        ...

    @staticmethod
    def from_json(json: str, /) -> CapitalStructureCashflows:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        CapitalStructureCashflows
            Reconstructed cashflows.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import CapitalStructureCashflows
        >>> CapitalStructureCashflows.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...
