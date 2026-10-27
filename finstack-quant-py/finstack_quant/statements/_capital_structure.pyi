"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/capital_structure.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.core.money import Money

class EcfSweepSpec:
    """
    Excess Cash Flow sweep specification.

    Configures how ECF is computed (EBITDA minus taxes/capex/WC/cash interest)
    and what fraction sweeps to debt paydown.

    Examples
    --------
    >>> from finstack_quant.statements import EcfSweepSpec
    >>> sweep = EcfSweepSpec("ebitda", 0.5)
    >>> (sweep.ebitda_node, sweep.sweep_percentage)
    ('ebitda', 0.5)

    """

    def __init__(
        self,
        ebitda_node: str,
        sweep_percentage: float,
        taxes_node: str | None = None,
        capex_node: str | None = None,
        working_capital_node: str | None = None,
        cash_interest_node: str | None = None,
        target_instrument_id: str | None = None,
    ) -> None:
        """
        Configure an excess-cash-flow debt sweep.

        Parameters
        ----------
        ebitda_node : str
            Model node identifier supplying EBITDA before ECF deductions.
        sweep_percentage : float
            Decimal fraction of computed ECF swept to debt, such as ``0.50``.
        taxes_node : str or None, default None
            Optional node identifier for cash taxes deducted from EBITDA.
        capex_node : str or None, default None
            Optional node identifier for capital expenditures deducted from EBITDA.
        working_capital_node : str or None, default None
            Optional node identifier for working-capital cash use or release.
        cash_interest_node : str or None, default None
            Optional node identifier for cash interest deducted before the sweep.
        target_instrument_id : str or None, default None
            Optional debt instrument receiving the ECF paydown; ``None`` uses
            the waterfall's eligible debt allocation.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
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
        >>> sweep = EcfSweepSpec("ebitda", 0.5)
        >>> EcfSweepSpec.from_json(sweep.to_json()).sweep_percentage
        0.5

        """
        ...
    def validate(self) -> None:
        """
        Validate the sweep on its own (the Rust ``EcfSweepSpec::validate``).

        The waterfall-level rule that a positive sweep needs a prepayment
        priority is checked by :meth:`WaterfallSpec.validate`. Twin of the WASM
        ``statements.validateEcfSweepSpecJson``.

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
    def ebitda_node(self) -> str:
        """
        Node reference or DSL formula supplying EBITDA, the ECF starting point.

        Returns
        -------
        str
            A node id (``"ebitda"``) or an expression over nodes
            (``"revenue - cogs - opex"``), evaluated per period as a monetary
            amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

    @property
    def target_instrument_id(self) -> str | None:
        """
        Debt instrument the sweep pays down, if the sweep is targeted.

        Returns
        -------
        str | None
            Instrument id, or ``None`` when the sweep applies to all term
            loans in the capital structure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    @property
    def taxes_node(self) -> str | None:
        """Return the optional taxes node reference.

        Returns
        -------
        str | None
            Cash-tax node deducted from EBITDA. None means omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def capex_node(self) -> str | None:
        """Return the optional capex node reference.

        Returns
        -------
        str | None
            Capital-expenditure node deducted from EBITDA. None means omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def working_capital_node(self) -> str | None:
        """Return the optional working capital node reference.

        Returns
        -------
        str | None
            Working-capital movement node deducted from EBITDA. None means omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def cash_interest_node(self) -> str | None:
        """Return the optional cash interest node reference.

        Returns
        -------
        str | None
            Cash-interest node deducted from EBITDA; None uses contractual period debt-service interest. None means omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

class PikToggleSpec:
    """
    PIK toggle specification.

    Controls when interest accrues as PIK versus cash based on a liquidity
    signal crossing ``threshold``, with optional hysteresis.

    Examples
    --------
    >>> from finstack_quant.statements import PikToggleSpec
    >>> toggle = PikToggleSpec("cash", 100.0, ["TL"])
    >>> (toggle.liquidity_metric, toggle.min_periods_in_pik)
    ('cash', 0)

    """

    def __init__(
        self,
        liquidity_metric: str,
        threshold: float,
        target_instrument_ids: list[str],
        min_periods_in_pik: int = 0,
    ) -> None:
        """
        Configure a liquidity-triggered payment-in-kind interest toggle.

        Parameters
        ----------
        liquidity_metric : str
            Model metric or node identifier compared with the trigger threshold.
        threshold : float
            Liquidity threshold in the metric's units that activates PIK logic.
        target_instrument_ids : list[str]
            Explicit nonempty borrowing-debt identifiers whose cash coupons may
            capitalize into principal. Instrument-level PIK capability is not
            modeled, so an empty list is rejected by
            :meth:`validate` and :meth:`WaterfallSpec.validate` (it does not
            mean "every instrument"); model validation also rejects hedge or
            option targets. Construction stores the specification unvalidated.
        min_periods_in_pik : int, default 0
            Hysteresis floor counted in **periods** on the model's own cadence
            (not months): once triggered, PIK stays on for at least this many
            periods. Default 0 lets PIK toggle every period.

        Raises
        ------
        TypeError
            If targets are omitted or are not a list of strings. Domain validation
            is performed by :meth:`validate`.
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
        >>> toggle = PikToggleSpec("cash", 100.0, ["TL"])
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
            If ``target_instrument_ids`` is empty.
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
            when it is a coverage ratio. PIK triggers when
            ``metric < threshold``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def min_periods_in_pik(self) -> int:
        """
        Hysteresis floor: minimum time PIK stays on once triggered.

        Returns
        -------
        int
            Count in **periods** on the model's own cadence (quarters for a
            quarterly model), not months. ``0`` disables hysteresis, letting
            PIK toggle every period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    @property
    def target_instrument_ids(self) -> list[str]:
        """Return explicit borrowing-debt targets for coupon capitalization.

        Returns
        -------
        list[str]
            Stored identifiers. Empty lists fail waterfall validation; swaps and options fail model
            validation because they cannot capitalize into borrowing principal.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
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

    Combines priority-of-payments with optional ECF sweep, PIK toggle,
    payment classes, and separate mandatory / voluntary prepay nodes.
    Call :meth:`validate` before passing to a builder to surface inconsistent
    configurations (for example ``Sweep`` ordered after ``Equity``).

    Examples
    --------
    >>> from finstack_quant.statements import WaterfallSpec
    >>> waterfall = WaterfallSpec()
    >>> waterfall.priority_of_payments[-1]
    'equity'

    """

    def __init__(
        self,
        priority_of_payments: list[str] | None = None,
        available_cash_node: str | None = None,
        ecf_sweep: EcfSweepSpec | None = None,
        pik_toggle: PikToggleSpec | None = None,
        payment_classes: list[PaymentClassSpec] | None = None,
        mandatory_prepay_node: str | None = None,
        voluntary_prepay_node: str | None = None,
    ) -> None:
        """
        Configure dynamic cash allocation for a financial-model waterfall.

        Parameters
        ----------
        priority_of_payments : list[str] or None, default None
            Ordered payment labels, from highest to lowest priority; ``None``
            applies the builder's default debt-before-equity sequence.
        available_cash_node : str or None, default None
            Pre-waterfall cash-pool node or formula; ``None`` uses ``"cash"``.
            Do not deduct ``cs`` debt-service tokens here.
        ecf_sweep : EcfSweepSpec or None, default None
            Optional excess-cash-flow sweep applied within the waterfall.
        pik_toggle : PikToggleSpec or None, default None
            Optional liquidity-driven PIK versus cash-interest configuration.
        payment_classes : list[PaymentClassSpec] or None, default None
            Intra-category seniority classes; ``None`` or empty is one implicit
            class (pro-rata across all instruments).
        mandatory_prepay_node : str or None, default None
            Node or formula sizing the ``mandatory_prepayment`` rung. Required
            when that priority is listed.
        voluntary_prepay_node : str or None, default None
            Node or formula sizing the ``voluntary_prepayment`` rung. Required
            when that priority is listed.

        Raises
        ------
        ValueError
            If priority_of_payments contains an unknown priority name.

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
        sweep percentage outside [0, 1], an empty PIK target list, a positive
        ECF sweep without the ``sweep`` priority, mismatched prepayment
        node/priority pairs in either direction, and — when
        ``available_cash_node`` is set — a stack missing ``fees``,
        ``interest`` or ``amortization``. It does not confirm that referenced
        nodes or instruments exist; that needs the enclosing model.

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
    def mandatory_prepay_node(self) -> str | None:
        """
        Node or formula sizing the ``mandatory_prepayment`` rung.

        Returns
        -------
        str | None
            Node id or formula, or ``None`` when that rung is unused.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def voluntary_prepay_node(self) -> str | None:
        """
        Node or formula sizing the ``voluntary_prepayment`` rung.

        Returns
        -------
        str | None
            Node id or formula, or ``None`` when that rung is unused.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
            Sweep configuration, or None when no ECF sweep was configured; a positive percentage requires the sweep priority.

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
            PIK signal, threshold, eligible instruments, and hysteresis, or None when omitted.

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
    def equity_distribution(self) -> dict[str, float]:
        """
        Residual cash to equity, keyed by period id.

        This property does not raise.

        Returns
        -------
        dict[str, float]
            Residual cash to equity, keyed by period id.
        """
        ...

    @property
    def available_cash(self) -> dict[str, Money]:
        """
        Cash available to the waterfall, keyed by period id.

        The value of the waterfall's ``available_cash_node`` floored at zero,
        in the waterfall's currency. It reconciles the period's uses:
        ``fees + cash interest + principal_payment + equity_distribution ==
        available_cash``.

        This property does not raise.

        Returns
        -------
        dict[str, Money]
            Period id to ``Money``; empty unless a waterfall ran.
        """
        ...

    def get_component(self, component: str, instrument_or_total: str, period_id: str) -> float:
        """Read a capital-structure component for an instrument or reporting total.

        Parameters
        ----------
        component : str
            One of ``interest_expense`` (cash plus PIK), ``interest_expense_cash``,
            ``interest_expense_pik``, ``interest_income``, ``principal_payment``,
            ``debt_balance``, ``fees``, or ``accrued_interest``. Flows are positive
            payment or receipt magnitudes; debt balance is closing principal.
            Rate hedges have zero debt balance and debt accrued interest.
        instrument_or_total : str
            Instrument identifier for a native-currency amount, or ``"total"``
            for the reporting-currency aggregate. No FX conversion is performed
            by this accessor.
        period_id : str
            Reporting-period label such as ``"2025Q1"`` present in the result.

        Returns
        -------
        float
            Requested component in major units of the selected currency.
            Absent optional interest income is zero.

        Raises
        ------
        ValueError
            If ``period_id`` is not a valid reporting-period label.
        RuntimeError
            If the component, instrument, or period is unknown, reporting totals
            are unavailable for multiple currencies, or cash and PIK currencies
            differ when requesting total interest expense.
        """
        ...

    @property
    def totals_by_currency(self) -> dict[str, dict[str, dict[str, Money]]]:
        """Cross-instrument totals bucketed by native instrument currency.

        Populated for every evaluation with capital-structure flows, including
        multi-currency structures without a reporting currency, where
        ``to_totals_dataframe`` and the ``get_component(..., "total", ...)`` lookups have nothing
        to report.

        Returns
        -------
        dict[str, dict[str, dict[str, Money]]]
            ISO currency code to period id to flow type (the ``flow_type``
            names of :meth:`to_dataframe`) to amount in that currency.

        This accessor does not raise; it is empty when no capital-structure
        flows were evaluated.

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Long frame of per-instrument flows.

        Returns
        -------
        pandas.DataFrame
            Columns ``instrument``, ``period``, ``flow_type``, ``amount``
            and ``currency``. ``flow_type`` is one of
            ``interest_expense_cash``, ``interest_income_cash`` (when
            present), ``interest_expense_pik``, ``principal_payment``,
            ``fees``, ``debt_balance``, ``accrued_interest`` and — on
            evaluator-produced cashflows — ``opening_balance`` (debt balance
            at period start) plus the four sources of ``principal_payment``:
            ``scheduled_principal``, ``mandatory_prepayment``,
            ``sweep_prepayment`` and ``voluntary_prepayment``. The four
            source rows sum to the ``principal_payment`` row, so do not add
            them together.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """
        ...

    def to_totals_dataframe(self) -> pd.DataFrame:
        """Long frame of per-period totals across all instruments.

        Returns
        -------
        pandas.DataFrame
            Columns ``period``, ``flow_type``, ``amount`` and ``currency``.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

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
