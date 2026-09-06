"""
Typed covenant definitions, engine evaluation, templates and forecasting.

Bindings for ``finstack-quant-covenants``. Define covenants with
:class:`CovenantType` / :class:`Covenant` / :class:`CovenantSpec` (or take a
standard package from :func:`lbo_standard`, :func:`cov_lite`,
:func:`real_estate`, :func:`project_finance`), collect them in a
:class:`CovenantEngine`, and evaluate against a ``dict`` of metric values
into typed :class:`CovenantReport` results. Step-down schedules
(:class:`ThresholdSchedule`), waivers (:class:`CovenantWaiver`) and springing
conditions (:class:`SpringingCondition`) are typed too. Forecast future
compliance from a date-indexed ``pandas.DataFrame`` of projected metrics with
:func:`forecast_covenant` / :func:`forecast_breaches`.

Conventions
-----------
Ratio metrics and thresholds are in turns (``4.5`` means 4.5x); rate-style
custom metrics such as debt yield or LTV are decimal fractions (``0.08`` is
8%); amount metrics (capex, liquidity, baskets) are bare numbers in the
caller's reporting currency. The engine tests whenever you call ``evaluate``
— ``test_frequency`` is descriptive metadata. Required metric observations must be finite (``ValueError`` otherwise);
a missing required metric raises ``KeyError``. Net-debt tests also require
positive earnings, selected by ``denominator_metric_id`` (default ``ebitda``).

The JSON surface shared with WASM is kept: the ``validate_covenant_*_json``
validators, ``evaluate_engine`` on an engine document, and the ``*_json``
template twins that return a JSON array of specs.

Examples
--------
>>> from finstack_quant.covenants import Covenant, CovenantEngine, CovenantSpec, CovenantType
>>> covenant = Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_total_leverage")
>>> engine = CovenantEngine().add_spec(CovenantSpec(covenant, "debt_to_ebitda"))
>>> report = engine.evaluate({"debt_to_ebitda": 3.2}, "2025-03-31")["max_total_leverage"]
>>> report.passed, report.threshold
(True, 4.5)
"""

from __future__ import annotations

import datetime

from typing import Any

import pandas as pd

from finstack_quant.core.dates import Tenor

__all__ = [
    "Covenant",
    "CovenantBreach",
    "CovenantConsequence",
    "CovenantEngine",
    "CovenantForecast",
    "CovenantForecastConfig",
    "CovenantReport",
    "CovenantSpec",
    "CovenantType",
    "CovenantWaiver",
    "FutureBreach",
    "SpringingCondition",
    "ThresholdSchedule",
    "breaches_to_dataframe",
    "cov_lite",
    "cov_lite_json",
    "evaluate_engine",
    "forecast_breaches",
    "forecast_covenant",
    "lbo_standard",
    "lbo_standard_json",
    "project_finance",
    "project_finance_json",
    "real_estate",
    "real_estate_json",
    "reports_to_dataframe",
    "validate_covenant_engine_json",
    "validate_covenant_report_json",
    "validate_covenant_spec_json",
]

DateLike = datetime.date | str

class CovenantType:
    """
    Type of financial or operational covenant with its static threshold.

    Build with the classmethod matching the Rust variant. Ratio covenants take
    a threshold in turns; ``max_capex`` / ``min_liquidity`` / ``basket`` take
    reporting-currency amounts; ``custom`` takes a caller-defined metric with
    a ``"maximum"`` or ``"minimum"`` bound; ``negative`` / ``affirmative``
    are never tested numerically. Instances compare equal by value and
    pickle via their JSON form.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantType
    >>> ct = CovenantType.max_debt_to_ebitda(4.5)
    >>> ct.covenant_id, ct.threshold, ct.bound_kind
    ('max_debt_ebitda', 4.5, 'at_most')
    >>> str(CovenantType.custom("ltv", "maximum", 0.75))
    'ltv <= 0.75'
    """

    @staticmethod
    def max_debt_to_ebitda(threshold: float) -> CovenantType:
        """
        Maximum gross Debt/EBITDA covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Maximum allowed ratio in turns (``4.5`` means 4.5x).

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"max_debt_ebitda"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.max_debt_to_ebitda(4.5).threshold
        4.5
        """

    @staticmethod
    def min_interest_coverage(threshold: float) -> CovenantType:
        """
        Minimum interest coverage (EBIT / interest) covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Minimum required coverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"min_interest_coverage"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.min_interest_coverage(2.0).bound_kind
        'at_least'
        """

    @staticmethod
    def min_fixed_charge_coverage(threshold: float) -> CovenantType:
        """
        Minimum fixed-charge coverage covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Minimum required coverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"min_fcc"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.min_fixed_charge_coverage(1.1).covenant_id
        'min_fcc'
        """

    @staticmethod
    def max_total_leverage(threshold: float) -> CovenantType:
        """
        Maximum total leverage covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Maximum allowed leverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"max_total_leverage"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> str(CovenantType.max_total_leverage(7.0))
        'Total Leverage <= 7.00x'
        """

    @staticmethod
    def max_senior_leverage(threshold: float) -> CovenantType:
        """
        Maximum senior leverage covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Maximum allowed senior leverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"max_senior_leverage"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.max_senior_leverage(4.5).threshold
        4.5
        """

    @staticmethod
    def min_asset_coverage(threshold: float) -> CovenantType:
        """
        Minimum asset coverage covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Minimum required coverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"min_asset_coverage"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.min_asset_coverage(1.5).covenant_id
        'min_asset_coverage'
        """

    @staticmethod
    def negative(restriction: str) -> CovenantType:
        """
        Negative covenant (a prohibition); never tested numerically.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        restriction : str
            Description of the restriction.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"negative"`` and no threshold.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.negative("No additional liens").threshold is None
        True
        """

    @staticmethod
    def affirmative(requirement: str) -> CovenantType:
        """
        Affirmative covenant (a requirement); never tested numerically.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        requirement : str
            Description of the requirement.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"affirmative"`` and no threshold.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.affirmative("Deliver audited accounts").bound_kind is None
        True
        """

    @staticmethod
    def custom(metric: str, test: str, value: float) -> CovenantType:
        """
        Custom covenant testing a caller-defined metric against a bound.

        Parameters
        ----------
        metric : str
            Metric id looked up at evaluation when the spec has no
            ``metric_id`` of its own.
        test : str
            ``"maximum"`` (pass when metric <= ``value``) or ``"minimum"``
            (pass when metric >= ``value``).
        value : float
            Bound in the metric's own units (decimal fraction for rate-style
            metrics such as LTV).

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"custom"``.

        Raises
        ------
        ValueError
            If ``test`` is neither ``"maximum"`` nor ``"minimum"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.custom("ltv", "maximum", 0.75).bound_kind
        'at_most'
        """

    @staticmethod
    def basket(name: str, limit: float) -> CovenantType:
        """
        Basket covenant: utilization must stay at or below the limit.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        name : str
            Basket identifier, also the default metric id.
        limit : float
            Maximum utilization as a reporting-currency amount.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"basket"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.basket("general_debt_basket", 25_000_000.0).threshold
        25000000.0
        """

    @staticmethod
    def min_dscr(threshold: float) -> CovenantType:
        """
        Minimum debt service coverage ratio (EBITDA / debt service) covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Minimum required coverage in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"min_dscr"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> str(CovenantType.min_dscr(1.25))
        'DSCR >= 1.25x'
        """

    @staticmethod
    def max_net_debt_to_ebitda(threshold: float) -> CovenantType:
        """
        Maximum net Debt/EBITDA (net of cash) covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Maximum allowed ratio in turns.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"max_net_debt_ebitda"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.max_net_debt_to_ebitda(4.0).covenant_id
        'max_net_debt_ebitda'
        """

    @staticmethod
    def max_capex(threshold: float) -> CovenantType:
        """
        Maximum capital expenditure covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Maximum capex as a reporting-currency amount.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"max_capex"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.max_capex(50_000_000.0).bound_kind
        'at_most'
        """

    @staticmethod
    def min_liquidity(threshold: float) -> CovenantType:
        """
        Minimum liquidity (cash plus available revolver) covenant.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        threshold : float
            Minimum liquidity as a reporting-currency amount.

        Returns
        -------
        CovenantType
            Covenant type with ``covenant_id`` ``"min_liquidity"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.min_liquidity(10_000_000.0).bound_kind
        'at_least'
        """

    @staticmethod
    def from_json(json: str) -> CovenantType:
        """
        Deserialize from the externally-tagged JSON form.

        Parameters
        ----------
        json : str
            JSON such as ``{"max_debt_to_ebitda": {"threshold": 4.5}}``.

        Returns
        -------
        CovenantType
            Parsed covenant type.

        Raises
        ------
        ValueError
            If ``json`` is malformed or names an unknown variant.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantType
        >>> CovenantType.from_json('{"min_dscr": {"threshold": 1.2}}').threshold
        1.2
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Externally-tagged JSON, e.g. ``{"max_debt_to_ebitda":{"threshold":4.5}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_id(self) -> str:
        """
        Stable variant identifier.

        This property does not raise.

        Returns
        -------
        str
            ``"max_debt_ebitda"``, ``"min_interest_coverage"``, ``"min_fcc"``,
            ``"max_total_leverage"``, ``"max_senior_leverage"``,
            ``"min_asset_coverage"``, ``"min_dscr"``, ``"max_net_debt_ebitda"``,
            ``"max_capex"``, ``"min_liquidity"``, ``"negative"``,
            ``"affirmative"``, ``"custom"`` or ``"basket"``. Thresholds are not
            part of it.
        """

    @property
    def threshold(self) -> float | None:
        """
        Static threshold or limit.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` for negative / affirmative covenants.
        """

    @property
    def bound_kind(self) -> str | None:
        """
        Inequality direction of the numeric test.

        This property does not raise.

        Returns
        -------
        str | None
            ``"at_most"``, ``"at_least"``, or ``None`` for non-numeric covenants.
        """

    @property
    def description(self) -> str:
        """
        Human-readable description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 4.50x"``.
        """

    def __eq__(self, other: object) -> bool: ...
    def __str__(self) -> str: ...

class CovenantConsequence:
    """
    Consequence applied when a covenant breach is not cured in time.

    Build with the classmethod matching the Rust variant. Instances compare
    equal by value and pickle via JSON.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantConsequence
    >>> CovenantConsequence.rate_increase(200.0).kind
    'rate_increase'
    >>> CovenantConsequence.default() == CovenantConsequence.from_json('"default"')
    True
    """

    @staticmethod
    def default() -> CovenantConsequence:
        """
        Event of default.

        Inputs are stored verbatim, so this constructor does not raise.

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"default"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.default().kind
        'default'
        """

    @staticmethod
    def rate_increase(bp_increase: float) -> CovenantConsequence:
        """
        Interest margin step-up.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        bp_increase : float
            Margin increase in basis points (``200.0`` is 2.00%).

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"rate_increase"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.rate_increase(200.0).to_json()
        '{"rate_increase":{"bp_increase":200.0}}'
        """

    @staticmethod
    def cash_sweep(sweep_percentage: float) -> CovenantConsequence:
        """
        Mandatory sweep of excess cash flow.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        sweep_percentage : float
            Share of excess cash swept as a decimal fraction (``1.0`` is 100%).

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"cash_sweep"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.cash_sweep(0.5).kind
        'cash_sweep'
        """

    @staticmethod
    def block_distributions() -> CovenantConsequence:
        """
        Block distributions to equity holders.

        Inputs are stored verbatim, so this constructor does not raise.

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"block_distributions"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.block_distributions().kind
        'block_distributions'
        """

    @staticmethod
    def require_collateral(description: str) -> CovenantConsequence:
        """
        Require additional collateral.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        description : str
            Description of the collateral requirement.

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"require_collateral"``.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.require_collateral("Pledge receivables").kind
        'require_collateral'
        """

    @staticmethod
    def accelerate_maturity(new_maturity: DateLike) -> CovenantConsequence:
        """
        Accelerate the loan maturity.

        Parameters
        ----------
        new_maturity : datetime.date | str
            New maturity date (``datetime.date``, ``pandas.Timestamp`` or ISO
            ``YYYY-MM-DD`` string).

        Returns
        -------
        CovenantConsequence
            Consequence with ``kind`` ``"accelerate_maturity"``.

        Raises
        ------
        ValueError
            If the date string is not valid ISO 8601.
        TypeError
            If ``new_maturity`` is neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.accelerate_maturity("2027-12-31").kind
        'accelerate_maturity'
        """

    @staticmethod
    def from_json(json: str) -> CovenantConsequence:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            ``"default"``, ``"block_distributions"`` or an externally-tagged
            object such as ``{"rate_increase": {"bp_increase": 200.0}}``.

        Returns
        -------
        CovenantConsequence
            Parsed consequence.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantConsequence
        >>> CovenantConsequence.from_json('{"cash_sweep": {"sweep_percentage": 1.0}}').kind
        'cash_sweep'
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string in the externally-tagged serde form.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def kind(self) -> str:
        """
        Consequence variant name in snake_case.

        This property does not raise.

        Returns
        -------
        str
            ``"default"``, ``"rate_increase"``, ``"cash_sweep"``,
            ``"block_distributions"``, ``"require_collateral"`` or
            ``"accelerate_maturity"``.
        """

    def __eq__(self, other: object) -> bool: ...

class SpringingCondition:
    """
    Activation condition for a springing covenant.

    The covenant is tested only while ``metric_id`` satisfies the condition
    on the test date; otherwise it reports a pass with an explanatory
    ``details``. The activation metric must be present in the metrics passed
    to ``evaluate``.

    Examples
    --------
    >>> from finstack_quant.covenants import SpringingCondition
    >>> cond = SpringingCondition("revolver_utilization", "minimum", 0.30)
    >>> cond.metric_id, cond.test, cond.value
    ('revolver_utilization', 'minimum', 0.3)
    """

    def __init__(self, metric_id: str, test: str, value: float) -> None:
        """
        Create a springing condition.

        Parameters
        ----------
        metric_id : str
            Metric that controls activation (for example revolver utilization).
        test : str
            ``"minimum"`` (active when metric >= ``value``) or ``"maximum"``
            (active when metric <= ``value``).
        value : float
            Activation bound in the metric's own units.

        Raises
        ------
        ValueError
            If ``test`` is neither ``"maximum"`` nor ``"minimum"``.
        """

    @staticmethod
    def from_json(json: str) -> SpringingCondition:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON such as ``{"metric_id": "revolver_utilization", "test": {"minimum": 0.3}}``.

        Returns
        -------
        SpringingCondition
            Parsed condition.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import SpringingCondition
        >>> SpringingCondition.from_json('{"metric_id": "u", "test": {"maximum": 0.5}}').test
        'maximum'
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def metric_id(self) -> str:
        """
        Activation metric id.

        This property does not raise.

        Returns
        -------
        str
            Metric key looked up in the evaluation metrics.
        """

    @property
    def test(self) -> str:
        """
        Direction the activation metric is tested in.

        This property does not raise.

        Returns
        -------
        str
            ``"maximum"`` or ``"minimum"``.
        """

    @property
    def value(self) -> float:
        """
        Activation bound.

        This property does not raise.

        Returns
        -------
        float
            Bound compared against the activation metric.
        """

    def __eq__(self, other: object) -> bool: ...

class Covenant:
    """
    Financial covenant with test frequency, cure period, consequences, scope
    and optional springing condition.

    ``label`` is the covenant's identity: reports, breaches and waivers key
    off it, so two covenants of the same type must carry distinct labels.
    Defaults: 30-day cure period, no consequences, active, maintenance scope,
    no springing condition. The ``with_*`` methods return modified copies.

    Examples
    --------
    >>> from finstack_quant.covenants import Covenant, CovenantConsequence, CovenantType
    >>> cov = (
    ...     Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_total_leverage")
    ...     .with_cure_period(60)
    ...     .with_consequence(CovenantConsequence.rate_increase(200.0))
    ...     .with_scope("maintenance")
    ... )
    >>> cov.label, cov.cure_period_days, str(cov.test_frequency), cov.scope
    ('max_total_leverage', 60, '3M', 'maintenance')
    >>> [c.kind for c in cov.consequences]
    ['rate_increase']
    """

    def __init__(self, covenant_type: CovenantType, test_frequency: Tenor | str, label: str) -> None:
        """
        Create a covenant.

        Parameters
        ----------
        covenant_type : CovenantType
            Covenant type carrying the static threshold.
        test_frequency : Tenor | str
            Descriptive test frequency, a ``Tenor`` or tenor string such as
            ``"3M"`` / ``"1Y"``; the engine does not enforce it.
        label : str
            Instance label used as the report / breach / waiver key.

        Raises
        ------
        ValueError
            If the tenor string cannot be parsed.
        TypeError
            If ``test_frequency`` is neither a ``Tenor`` nor a string.
        """

    def with_cure_period(self, days: int | None) -> Covenant:
        """
        Return a copy with a different cure period.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        days : int | None
            Cure period in days; ``None`` removes it so a breach is
            immediate. Negative values fail engine validation.

        Returns
        -------
        Covenant
            Modified copy.
        """

    def with_consequence(self, consequence: CovenantConsequence) -> Covenant:
        """
        Return a copy with ``consequence`` appended.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        consequence : CovenantConsequence
            Consequence applied after an uncured breach.

        Returns
        -------
        Covenant
            Modified copy.
        """

    def with_scope(self, scope: str) -> Covenant:
        """
        Return a copy with a different scope.

        Parameters
        ----------
        scope : str
            ``"maintenance"`` (tested on a schedule) or ``"incurrence"``
            (tested on specific actions).

        Returns
        -------
        Covenant
            Modified copy.

        Raises
        ------
        ValueError
            If ``scope`` is any other string.
        """

    def with_springing_condition(self, condition: SpringingCondition) -> Covenant:
        """
        Return a copy that is active only while ``condition`` is met.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        condition : SpringingCondition
            Activation condition.

        Returns
        -------
        Covenant
            Modified copy.
        """

    @staticmethod
    def from_json(json: str) -> Covenant:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``Covenant`` wire schema (``test_frequency`` is
            ``{"count": 3, "unit": "months"}``; unknown fields are rejected).

        Returns
        -------
        Covenant
            Parsed covenant.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.covenants import Covenant, CovenantType
        >>> cov = Covenant(CovenantType.min_dscr(1.2), "3M", "min_dscr")
        >>> Covenant.from_json(cov.to_json()) == cov
        True
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_type(self) -> CovenantType:
        """
        Covenant type and static threshold.

        This property does not raise.

        Returns
        -------
        CovenantType
            The covenant type.
        """

    @property
    def test_frequency(self) -> Tenor:
        """
        Descriptive test frequency.

        This property does not raise.

        Returns
        -------
        Tenor
            Tenor such as ``3M``; metadata only.
        """

    @property
    def cure_period_days(self) -> int | None:
        """
        Cure period in days.

        This property does not raise.

        Returns
        -------
        int | None
            ``None`` when a breach is immediate.
        """

    @property
    def consequences(self) -> list[CovenantConsequence]:
        """
        Consequences applied after an uncured breach.

        This property does not raise.

        Returns
        -------
        list[CovenantConsequence]
            In the order they were added.
        """

    @property
    def is_active(self) -> bool:
        """
        Whether the covenant is active.

        This property does not raise.

        Returns
        -------
        bool
            Inactive covenants report a pass with ``details`` ``"Covenant inactive"``.
        """

    @property
    def scope(self) -> str:
        """
        When the covenant is tested during the facility's life.

        This property does not raise.

        Returns
        -------
        str
            ``"maintenance"`` or ``"incurrence"``.
        """

    @property
    def springing_condition(self) -> SpringingCondition | None:
        """
        Activation condition.

        This property does not raise.

        Returns
        -------
        SpringingCondition | None
            ``None`` for an always-on covenant.
        """

    @property
    def label(self) -> str:
        """
        Label this covenant is keyed by in reports and breaches.

        This property does not raise.

        Returns
        -------
        str
            Key under which reports and breaches are returned.
        """

    @property
    def description(self) -> str:
        """
        Human-readable description of the covenant type.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 4.50x"``.
        """

    def __eq__(self, other: object) -> bool: ...

class ThresholdSchedule:
    """
    Piecewise-constant threshold step-down schedule.

    Attached to a :class:`CovenantSpec` it overrides the static threshold:
    the threshold in force on a test date is the last entry whose effective
    date is on or before it, and before the first effective date the static
    threshold applies.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.covenants import ThresholdSchedule
    >>> schedule = ThresholdSchedule([("2026-01-01", 6.5), (datetime.date(2027, 1, 1), 6.0)])
    >>> schedule.threshold_for("2026-06-30"), schedule.threshold_for("2025-12-31")
    (6.5, None)
    >>> len(schedule)
    2
    """

    def __init__(self, entries: list[tuple[DateLike, float]]) -> None:
        """
        Create a schedule.

        Parameters
        ----------
        entries : list[tuple[datetime.date | str, float]]
            ``(effective_date, threshold)`` pairs in any order; thresholds in
            the covenant's units (turns for ratios).

        Raises
        ------
        ValueError
            If a threshold is NaN / infinite, two entries share a date, or a
            date string is not ISO 8601.
        TypeError
            If a date is neither a string nor date-like.
        """

    @staticmethod
    def from_json(json: str) -> ThresholdSchedule:
        """
        Deserialize from the JSON array form.

        Parameters
        ----------
        json : str
            JSON such as ``[["2026-01-01", 6.5], ["2027-01-01", 6.0]]``.

        Returns
        -------
        ThresholdSchedule
            Parsed schedule.

        Raises
        ------
        ValueError
            If ``json`` is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.covenants import ThresholdSchedule
        >>> ThresholdSchedule.from_json('[["2026-01-01", 6.5]]').entries[0][1]
        6.5
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON array of ``[date, threshold]`` pairs.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    def threshold_for(self, test_date: DateLike) -> float | None:
        """
        Threshold in force on a date.

        Parameters
        ----------
        test_date : datetime.date | str
            Covenant test date.

        Returns
        -------
        float | None
            Last threshold effective on or before ``test_date``, or ``None``
            before the first effective date.

        Raises
        ------
        ValueError
            If a date string is not ISO 8601.
        """

    @property
    def entries(self) -> list[tuple[datetime.date, float]]:
        """
        Schedule entries.

        This property does not raise.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(effective_date, threshold)`` pairs in ascending date order.
        """

    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

class CovenantWaiver:
    """
    Lender waiver or amendment for one covenant instance.

    A waiver without ``amended_threshold`` suppresses the test (the report
    passes with ``details`` ``"Waived by lender agreement"``); with a
    threshold it is an amendment and the covenant is tested against the
    amended value instead. ``expiry_date=None`` makes it permanent.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, CovenantWaiver, lbo_standard
    >>> engine = CovenantEngine.from_specs(lbo_standard(4.5, 2.0, 1.1, 50.0))
    >>> waiver = CovenantWaiver("max_debt_ebitda", "2026-01-01", "2026-12-31", amended_threshold=6.0)
    >>> _ = engine.add_waiver(waiver)
    >>> metrics = {"debt_to_ebitda": 5.0, "interest_coverage": 3.0, "fixed_charge_coverage": 1.5, "capex": 10.0}
    >>> engine.evaluate(metrics, "2026-03-31")["max_debt_ebitda"].threshold
    6.0
    >>> engine.evaluate(metrics, "2027-03-31")["max_debt_ebitda"].passed
    False
    """

    def __init__(
        self,
        covenant_id: str,
        effective_date: DateLike,
        expiry_date: DateLike | None = None,
        amended_threshold: float | None = None,
        description: str = "",
    ) -> None:
        """
        Create a waiver.

        Parameters
        ----------
        covenant_id : str
            ``Covenant.label`` of the waived covenant.
        effective_date : datetime.date | str
            First date the waiver applies.
        expiry_date : datetime.date | str | None
            Last date the waiver applies; ``None`` for a permanent amendment.
        amended_threshold : float | None
            Amended threshold in the covenant's units; ``None`` for a full
            waiver.
        description : str
            Free-text description of the waiver terms.

        Raises
        ------
        ValueError
            If a date string is not ISO 8601.
        TypeError
            If a date is neither a string nor date-like.
        """

    @staticmethod
    def from_json(json: str) -> CovenantWaiver:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantWaiver`` wire schema.

        Returns
        -------
        CovenantWaiver
            Parsed waiver.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantWaiver
        >>> w = CovenantWaiver("max_debt_ebitda", "2026-01-01")
        >>> CovenantWaiver.from_json(w.to_json()).expiry_date is None
        True
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the waived covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def effective_date(self) -> datetime.date:
        """
        First date the waiver applies.

        This property does not raise.

        Returns
        -------
        datetime.date
            Effective date.
        """

    @property
    def expiry_date(self) -> datetime.date | None:
        """
        Last date the waiver applies.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` for a permanent amendment.
        """

    @property
    def amended_threshold(self) -> float | None:
        """
        Amended threshold.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` for a full waiver.
        """

    @property
    def description(self) -> str:
        """
        Free-text description of the negotiated waiver terms.

        This property does not raise.

        Returns
        -------
        str
            Free text, possibly empty.
        """

    def __eq__(self, other: object) -> bool: ...

class CovenantSpec:
    """
    A covenant paired with the metric it is tested against and an optional
    step-down schedule.

    Examples
    --------
    >>> from finstack_quant.covenants import Covenant, CovenantEngine, CovenantSpec, CovenantType, ThresholdSchedule
    >>> cov = Covenant(CovenantType.max_debt_to_ebitda(7.0), "3M", "max_leverage")
    >>> spec = CovenantSpec(cov, "debt_to_ebitda").with_threshold_schedule(
    ...     ThresholdSchedule([("2026-01-01", 6.5), ("2027-01-01", 6.0)])
    ... )
    >>> engine = CovenantEngine.from_specs([spec])
    >>> engine.evaluate({"debt_to_ebitda": 6.2}, "2025-12-31")["max_leverage"].threshold
    7.0
    >>> engine.evaluate({"debt_to_ebitda": 6.2}, "2026-03-31")["max_leverage"].threshold
    6.5
    >>> engine.evaluate({"debt_to_ebitda": 6.2}, "2027-03-31")["max_leverage"].passed
    False
    """

    def __init__(self, covenant: Covenant, metric_id: str | None = None) -> None:
        """
        Pair a covenant with its metric; net debt/EBITDA also selects ``ebitda``
        as its required earnings denominator.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        covenant : Covenant
            Covenant to evaluate.
        metric_id : str | None
            Key looked up in the evaluation metrics. ``None`` falls back to the
            covenant type's conventional metric (``debt_to_ebitda``,
            ``interest_coverage``, ``fixed_charge_coverage``,
            ``total_leverage``, ``senior_leverage``, ``asset_coverage``,
            ``dscr``, ``net_debt_to_ebitda``, ``capex``, ``liquidity``) or to
            the ``metric`` / ``name`` of a custom or basket covenant.
        """

    def with_denominator_metric(self, metric_id: str) -> CovenantSpec:
        """Return a copy using the named earnings denominator for a leverage test.

        This method stores the identifier and does not raise.

        Parameters
        ----------
        metric_id : str
            Finite earnings metric for the same reporting period as the ratio.
            Net-debt constructors default to ``ebitda``. Non-positive earnings
            produce a breach with no meaningful ratio or headroom.

        Returns
        -------
        CovenantSpec
            Copy with the selected denominator metric.
        """

    @property
    def denominator_metric_id(self) -> str | None:
        """Earnings metric used to distinguish net cash from non-positive earnings.

        This property does not raise.

        Returns
        -------
        str | None
            Selected denominator, or ``None`` when no separate denominator is used.
        """

    def with_threshold_schedule(self, schedule: ThresholdSchedule) -> CovenantSpec:
        """
        Return a copy whose threshold follows a step-down schedule.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        schedule : ThresholdSchedule
            Effective-dated thresholds overriding the static one.

        Returns
        -------
        CovenantSpec
            Modified copy.
        """

    @staticmethod
    def from_json(json: str) -> CovenantSpec:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantSpec`` wire schema, for example one
            element of ``lbo_standard_json(...)``.

        Returns
        -------
        CovenantSpec
            Parsed spec.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.covenants import CovenantSpec, lbo_standard_json
        >>> raw = json.loads(lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))[0]
        >>> CovenantSpec.from_json(json.dumps(raw)).metric_id
        'debt_to_ebitda'
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string accepted by ``validate_covenant_spec_json``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant(self) -> Covenant:
        """
        The covenant being evaluated.

        This property does not raise.

        Returns
        -------
        Covenant
            Covenant definition.
        """

    @property
    def metric_id(self) -> str | None:
        """
        Metric key looked up at evaluation.

        This property does not raise.

        Returns
        -------
        str | None
            ``None`` when the covenant type's conventional metric is used.
        """

    @property
    def threshold_schedule(self) -> ThresholdSchedule | None:
        """
        Step-down schedule.

        This property does not raise.

        Returns
        -------
        ThresholdSchedule | None
            ``None`` when the static threshold applies.
        """

    def __eq__(self, other: object) -> bool: ...

class CovenantBreach:
    """
    A breach recorded by :meth:`CovenantEngine.evaluate_and_track`.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, cov_lite
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> _ = engine.evaluate_and_track({"total_leverage": 7.5, "senior_leverage": 3.0}, "2026-03-31", "incurrence")
    >>> breach = engine.breach_history[0]
    >>> breach.covenant_id, breach.breach_date.isoformat(), breach.is_cured
    ('max_total_leverage', '2026-03-31', False)
    """

    @staticmethod
    def from_json(json: str) -> CovenantBreach:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantBreach`` wire schema.

        Returns
        -------
        CovenantBreach
            Parsed breach.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantBreach
        >>> raw = '{"covenant_id": "x", "covenant_type": "DSCR >= 1.20x", "breach_date": "2026-03-31", "actual_value": 1.1, "threshold": 1.2, "cure_deadline": null, "is_cured": false, "consequences": [], "applied_consequences": []}'
        >>> CovenantBreach.from_json(raw).threshold
        1.2
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the breached covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def covenant_type(self) -> str:
        """
        Human-readable covenant description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Total Leverage <= 7.00x"``.
        """

    @property
    def breach_date(self) -> datetime.date:
        """
        Test date on which the breach was recorded.

        This property does not raise.

        Returns
        -------
        datetime.date
            Breach date.
        """

    @property
    def actual_value(self) -> float | None:
        """
        Metric value that caused the breach.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when the covenant was not numeric.
        """

    @property
    def threshold(self) -> float | None:
        """
        Threshold in force at the breach.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when the covenant was not numeric.
        """

    @property
    def cure_deadline(self) -> datetime.date | None:
        """
        End of the cure period.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` when the covenant has no cure period.
        """

    @property
    def is_cured(self) -> bool:
        """
        Whether a later pass inside the cure period cured the breach.

        This property does not raise.

        Returns
        -------
        bool
            ``True`` once cured.
        """

    @property
    def consequences(self) -> list[CovenantConsequence]:
        """Original ordered consequences captured when the breach began.

        This property does not raise.

        Returns
        -------
        list[CovenantConsequence]
            Captured actions; ``applied_consequences`` is the completed prefix.
        """

    @property
    def applied_consequences(self) -> list[CovenantConsequence]:
        """
        Consequences already applied for this breach.

        This property does not raise.

        Returns
        -------
        list[CovenantConsequence]
            Empty until consequences are applied (Rust-only today).
        """

    def __eq__(self, other: object) -> bool: ...

class CovenantEngine:
    """
    Covenant package: specifications, waivers and accumulated breach history.

    Build it empty and chain ``add_spec`` / ``add_waiver``, or start from a
    template with :meth:`from_specs`. ``evaluate`` returns a ``dict`` keyed by
    covenant label in spec order. Only ``specs`` is required in the JSON
    document; ``breach_history``, ``windows`` and ``waivers`` default to
    empty. Windows (``CovenantWindow``) are reachable only through JSON.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, lbo_standard
    >>> engine = CovenantEngine.from_specs(lbo_standard(6.0, 2.0, 1.1, 50.0))
    >>> list(
    ...     engine.evaluate(
    ...         {"debt_to_ebitda": 4.0, "interest_coverage": 3.0, "fixed_charge_coverage": 1.5, "capex": 40.0},
    ...         "2026-03-31",
    ...     )
    ... )
    ['max_debt_ebitda', 'min_interest_coverage', 'min_fcc', 'max_capex']
    >>> len(engine)
    4
    """

    def __init__(self) -> None:
        """
        Create an empty engine.

        Inputs are stored verbatim, so this constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantEngine
        >>> len(CovenantEngine())
        0
        """

    @staticmethod
    def from_specs(specs: list[CovenantSpec]) -> CovenantEngine:
        """
        Create an engine holding ``specs``.

        Inputs are stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        specs : list[CovenantSpec]
            Specifications in evaluation order, for example a template package.

        Returns
        -------
        CovenantEngine
            Engine with the given specs and no waivers or breaches.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantEngine, real_estate
        >>> [s.covenant.label for s in CovenantEngine.from_specs(real_estate(1.25, 0.08, 0.75)).specs]
        ['min_dscr', 'min_debt_yield', 'max_ltv']
        """

    def add_spec(self, spec: CovenantSpec) -> CovenantEngine:
        """
        Append a specification.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        spec : CovenantSpec
            Specification to append; its label must be unique among the specs
            applicable on a test date.

        Returns
        -------
        CovenantEngine
            This engine, for chaining.
        """

    def add_waiver(self, waiver: CovenantWaiver) -> CovenantEngine:
        """
        Record a waiver or amendment.

        This method builds a value in memory and does not raise.

        Parameters
        ----------
        waiver : CovenantWaiver
            Waiver keyed by covenant label.

        Returns
        -------
        CovenantEngine
            This engine, for chaining.
        """

    def validate(self) -> None:
        """
        Validate specs, waivers and windows without evaluating.

        Raises
        ------
        ValueError
            If a threshold is non-finite, a cure period is negative, a waiver
            expires before it takes effect, or windows overlap.
        """

    def evaluate(self, metrics: dict[str, float] | str, as_of: DateLike) -> dict[str, CovenantReport]:
        """
        Evaluate every applicable covenant on a date.

        Parameters
        ----------
        metrics : dict[str, float] | str
            Metric values keyed by metric id (or a JSON object string).
            Ratios in turns, amounts in the reporting currency; ``bool``
            values are rejected.
        as_of : datetime.date | str
            Test date (``datetime.date``, ``pandas.Timestamp`` or ISO string).
            An amended waiver or step-down schedule in force on this date
            changes the threshold used.

        Returns
        -------
        dict[str, CovenantReport]
            One report per covenant keyed by label, in spec order. Inactive,
            waived and unsprung covenants report ``passed=True`` with an
            explanatory ``details``.

        Raises
        ------
        KeyError
            If a required metric is missing from ``metrics``.
        ValueError
            If the engine is invalid, two specs share a label, a metric value
            is not a number, or the date string is not ISO 8601.
        TypeError
            If ``metrics`` is neither a dict nor a string, or ``as_of`` is
            neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantEngine, cov_lite
        >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
        >>> report = engine.evaluate({"total_leverage": 5.0, "senior_leverage": 3.0}, "2026-03-31")[
        ...     "max_senior_leverage"
        ... ]
        >>> report.passed, round(report.headroom, 4)
        (True, 0.3333)
        """

    def evaluate_and_track(
        self, metrics: dict[str, float] | str, as_of: DateLike, scope: str
    ) -> dict[str, CovenantReport]:
        """
        Evaluate like :meth:`evaluate` and update :attr:`breach_history`.

        A failing covenant without an active breach gains a
        :class:`CovenantBreach` (with its cure deadline); a later pass inside
        the cure period marks the breach cured. Inactive or waived reports do not
        establish recovery. Repeated failures of a
        still-active breach add no duplicate record.

        Parameters
        ----------
        metrics : dict[str, float] | str
            Metric values keyed by metric id (or a JSON object string).
        as_of : datetime.date | str
            Test date.
        scope : str
            ``maintenance`` for scheduled tests or ``incurrence`` for a completed
            action evaluated using pro forma metrics. Only that scope is tracked.

        Returns
        -------
        dict[str, CovenantReport]
            Same shape as :meth:`evaluate`.

        Raises
        ------
        KeyError
            If a required metric is missing; the history is left untouched.
        ValueError
            If scope, terms, or finite metric inputs are invalid, or the cure date overflows.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantEngine, cov_lite
        >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
        >>> _ = engine.evaluate_and_track({"total_leverage": 7.5, "senior_leverage": 3.0}, "2026-03-31", "incurrence")
        >>> _ = engine.evaluate_and_track({"total_leverage": 6.5, "senior_leverage": 3.0}, "2026-04-15", "incurrence")
        >>> engine.breach_history[0].is_cured
        True
        """

    def evaluate_series(self, metrics: pd.DataFrame) -> pd.DataFrame:
        """
        Evaluate the engine on every row of a date-indexed metrics frame.

        Parameters
        ----------
        metrics : pd.DataFrame
            Index holds the test dates (``datetime.date``, ``Timestamp`` or
            ISO strings); columns are metric ids; required cells must be finite. Duplicate metric columns are rejected.

        Returns
        -------
        pd.DataFrame
            Long frame with one row per (date, covenant) and columns
            ``as_of`` (ISO string), ``covenant`` (label), ``covenant_type``,
            ``passed``, ``actual_value``, ``threshold``, ``headroom``,
            ``details``.

        Raises
        ------
        KeyError
            If a required metric is missing on any date.
        ValueError
            If the engine is invalid or the frame is not numeric.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.covenants import CovenantEngine, cov_lite
        >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
        >>> frame = pd.DataFrame(
        ...     {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 3.5]},
        ...     index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
        ... )
        >>> out = engine.evaluate_series(frame)
        >>> out.loc[out["covenant"] == "max_total_leverage", "passed"].tolist()
        [True, False]
        """

    @staticmethod
    def from_json(json: str) -> CovenantEngine:
        """
        Deserialize an engine document.

        Parameters
        ----------
        json : str
            JSON with a ``specs`` array; ``breach_history``, ``windows`` and
            ``waivers`` are optional. Unknown fields are rejected.

        Returns
        -------
        CovenantEngine
            Parsed engine.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantEngine, cov_lite_json
        >>> engine = CovenantEngine.from_json('{"specs": ' + cov_lite_json(7.0, 4.5) + "}")
        >>> len(engine)
        3
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Engine document accepted by ``evaluate_engine`` and
            ``validate_covenant_engine_json``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def specs(self) -> list[CovenantSpec]:
        """
        Top-level specifications.

        This property does not raise.

        Returns
        -------
        list[CovenantSpec]
            In insertion order.
        """

    @property
    def waivers(self) -> list[CovenantWaiver]:
        """
        Recorded waivers and amendments.

        This property does not raise.

        Returns
        -------
        list[CovenantWaiver]
            In insertion order.
        """

    @property
    def breach_history(self) -> list[CovenantBreach]:
        """
        Breaches recorded by :meth:`evaluate_and_track` or loaded from JSON.

        This property does not raise.

        Returns
        -------
        list[CovenantBreach]
            In recording order.
        """

    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

class CovenantReport:
    """
    Result of a single covenant evaluation.

    Carries pass/fail status, the tested value against its threshold, the
    headroom (positive is cushion, negative is deficit), an optional
    human-readable explanation, and the audit stamp in force when the covenant
    was evaluated. Reports compare equal by value.

    Construct via :meth:`CovenantEngine.evaluate`, :func:`evaluate_engine` or
    :meth:`from_json`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import CovenantReport
    >>> report = CovenantReport.from_json(
    ...     json.dumps({
    ...         "covenant_type": "Debt/EBITDA <= 5.00x",
    ...         "passed": False,
    ...         "actual_value": 5.5,
    ...         "threshold": 5.0,
    ...         "details": "Exceeded",
    ...         "headroom": -0.5,
    ...     })
    ... )
    >>> report.passed
    False
    >>> report == CovenantReport.from_json(report.to_json())
    True
    """

    @staticmethod
    def from_json(json: str) -> CovenantReport:
        """
        Deserialize a ``CovenantReport`` from JSON.

        Parameters
        ----------
        json : str
            JSON string matching the ``CovenantReport`` wire schema.

        Returns
        -------
        CovenantReport
            Parsed report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or omits required fields.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.covenants import CovenantReport
        >>> report = CovenantReport.from_json(
        ...     json.dumps({
        ...         "covenant_type": "Debt/EBITDA <= 5.00x",
        ...         "passed": False,
        ...         "actual_value": 5.5,
        ...         "threshold": 5.0,
        ...         "details": "Exceeded",
        ...         "headroom": -0.5,
        ...     })
        ... )
        >>> CovenantReport.from_json(report.to_json()).headroom
        -0.5
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def covenant_type(self) -> str:
        """
        Human-readable description of the covenant being tested.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 5.00x"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def covenant_id(self) -> str | None:
        """
        Stable machine-readable covenant instance identifier.

        Returns
        -------
        str | None
            ``None`` when the report was produced without an identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def passed(self) -> bool:
        """
        Whether the covenant passed.

        Inactive covenants, unmet springing conditions, and full waivers also
        report ``True``; :attr:`details` carries the reason.

        Returns
        -------
        bool
            ``True`` when the tested metric satisfied its threshold. Because an
            untested covenant also reports ``True``, read it together with
            :attr:`actual_value`, which is ``None`` exactly when no numeric
            test ran.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def actual_value(self) -> float | None:
        """
        Observed metric value that was tested against the covenant.

        Returns
        -------
        float | None
            ``None`` when the covenant was not evaluated numerically (inactive,
            waived, or springing condition unmet).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def threshold(self) -> float | None:
        """
        Threshold the metric was tested against.

        Returns
        -------
        float | None
            ``None`` when no numeric test was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def details(self) -> str | None:
        """
        Explanation of the outcome.

        Returns
        -------
        str | None
            Free text such as ``"Covenant inactive"``, ``"Waived by lender
            agreement"``, or ``"In cure period"``. ``None`` for a plain numeric
            result whose evaluator supplied no commentary, so absence carries
            no meaning of its own.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def headroom(self) -> float | None:
        """
        Cushion relative to the threshold.

        Returns
        -------
        float | None
            Signed distance from the threshold divided by ``|threshold|``:
            positive is a passing buffer, negative a deficit. ``None`` when no
            numeric test was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Audit stamp: numeric mode, rounding context, and FX policy in force.

        Returns
        -------
        dict[str, Any]
            Keys ``numeric_mode``, ``rounding``, ``fx_policy_applied`` and
            ``version``; ``fx_policy_applied`` is ``None`` when the evaluation
            stayed in one currency. Reproducing a report requires re-running
            under the same ``rounding`` context.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report as a single-row pandas DataFrame.

        Columns: ``covenant_type``, ``covenant_id``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``details``.

        Returns
        -------
        pd.DataFrame
            One row describing this covenant evaluation.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __eq__(self, other: object) -> bool: ...

class CovenantForecastConfig:
    """
    Forecast policy for :func:`forecast_covenant` / :func:`forecast_breaches`.

    Deterministic by default (breach probability ``0`` or ``1`` per date).
    With ``stochastic=True`` a lognormal overlay with ``volatility`` scales
    shocks by ``sqrt(T)`` from ``reference_date`` (default: the day before
    the first forecast date): ``num_paths=0`` is the closed-form analytic
    mode, ``num_paths>0`` is Monte Carlo (deterministic for a given
    ``random_seed``).

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantForecastConfig
    >>> cfg = CovenantForecastConfig(stochastic=True, volatility=0.25, reference_date="2025-12-31")
    >>> cfg.stochastic, cfg.num_paths, cfg.breach_probability_threshold
    (True, 0, 0.05)
    """

    def __init__(
        self,
        stochastic: bool = False,
        num_paths: int = 0,
        volatility: float | None = None,
        random_seed: int | None = None,
        antithetic: bool = False,
        reference_date: DateLike | None = None,
        breach_probability_threshold: float = 0.05,
    ) -> None:
        """
        Create a forecast configuration.

        Parameters
        ----------
        stochastic : bool
            Use the lognormal stochastic overlay instead of deterministic
            pass/fail probabilities.
        num_paths : int
            Monte Carlo path count; ``0`` selects analytic mode. MC requires at
            least two independent samples, or two antithetic pairs.
        volatility : float | None
            Annualized lognormal volatility of the metric; required when
            ``stochastic`` is true.
        random_seed : int | None
            Seed for Monte Carlo mode; ``None`` uses the crate default ``0``.
        antithetic : bool
            Simulate Monte Carlo paths in ``(Z, -Z)`` pairs; requires
            ``num_paths > 0``.
        reference_date : datetime.date | str | None
            Anchor for ``sqrt(T)`` horizon scaling; ``None`` uses the day
            before the first forecast date.
        breach_probability_threshold : float
            Minimum breach probability (decimal) for :func:`forecast_breaches`
            to report a date; must lie in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``reference_date`` is a non-ISO string. Other invalid
            combinations (missing volatility, antithetic without paths,
            threshold outside ``[0, 1]``) raise ``ValueError`` at forecast time.
        """

    def with_scope(self, scope: str) -> CovenantForecastConfig:
        """Return a copy selecting the covenant scope for batch forecasts.

        Parameters
        ----------
        scope : str
            ``maintenance`` for scheduled compliance or ``incurrence`` for
            hypothetical action capacity. The default is ``maintenance``.

        Returns
        -------
        CovenantForecastConfig
            Configuration with the selected batch scope.

        Raises
        ------
        ValueError
            If scope is neither ``maintenance`` nor ``incurrence``.
        """

    @property
    def scope(self) -> str:
        """Scope selected by batch forecasts; single-spec forecasts ignore it.

        This property does not raise.

        Returns
        -------
        str
            ``maintenance`` or ``incurrence``.
        """

    @staticmethod
    def from_json(json: str) -> CovenantForecastConfig:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantForecastConfig`` wire schema.

        Returns
        -------
        CovenantForecastConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import CovenantForecastConfig
        >>> CovenantForecastConfig.from_json(
        ...     '{"scope": "maintenance", "stochastic": false, "num_paths": 0, "volatility": null, "random_seed": null}'
        ... ).antithetic
        False
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def stochastic(self) -> bool:
        """
        Whether the stochastic overlay is used.

        This property does not raise.

        Returns
        -------
        bool
            ``False`` for deterministic pass/fail.
        """

    @property
    def num_paths(self) -> int:
        """
        Monte Carlo path count.

        This property does not raise.

        Returns
        -------
        int
            ``0`` selects the closed-form analytic mode.
        """

    @property
    def volatility(self) -> float | None:
        """
        Annualized lognormal volatility.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` in deterministic mode.
        """

    @property
    def random_seed(self) -> int | None:
        """
        Monte Carlo seed.

        This property does not raise.

        Returns
        -------
        int | None
            ``None`` uses the crate default ``0``.
        """

    @property
    def antithetic(self) -> bool:
        """
        Whether antithetic pairing is enabled.

        This property does not raise.

        Returns
        -------
        bool
            Only meaningful when ``num_paths > 0``.
        """

    @property
    def reference_date(self) -> datetime.date | None:
        """
        Horizon anchor used to scale stochastic forecast variance.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` for the default (day before the first forecast date).
        """

    @property
    def breach_probability_threshold(self) -> float:
        """
        Reporting threshold for :func:`forecast_breaches`.

        This property does not raise.

        Returns
        -------
        float
            Decimal probability in ``[0, 1]``.
        """

    def __eq__(self, other: object) -> bool: ...

class CovenantForecast:
    """
    Forward compliance projection for one covenant across the forecast dates.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import Covenant, CovenantSpec, CovenantType, forecast_covenant
    >>> spec = CovenantSpec(Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"), "debt_to_ebitda")
    >>> frame = pd.DataFrame(
    ...     {"debt_to_ebitda": [4.0, 4.4, 4.8]}, index=pd.to_datetime(["2026-03-31", "2026-06-30", "2026-09-30"])
    ... )
    >>> forecast = forecast_covenant(spec, frame)
    >>> forecast.first_breach_date.isoformat(), forecast.breach_probability
    ('2026-09-30', [0.0, 0.0, 1.0])
    >>> list(forecast.to_dataframe().columns)
    ['test_date', 'projected_value', 'threshold', 'headroom', 'breach_probability', 'breach_probability_stderr']
    """

    @staticmethod
    def from_json(json: str) -> CovenantForecast:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``CovenantForecast`` wire schema.

        Returns
        -------
        CovenantForecast
            Parsed forecast.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.covenants import (
        ...     Covenant,
        ...     CovenantForecast,
        ...     CovenantSpec,
        ...     CovenantType,
        ...     forecast_covenant,
        ... )
        >>> spec = CovenantSpec(Covenant(CovenantType.min_dscr(1.2), "3M", "min_dscr"), "dscr")
        >>> frame = pd.DataFrame({"dscr": [1.3, 1.1]}, index=pd.to_datetime(["2026-03-31", "2026-06-30"]))
        >>> forecast = forecast_covenant(spec, frame)
        >>> CovenantForecast.from_json(forecast.to_json()) == forecast
        True
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per test date.

        Returns
        -------
        pd.DataFrame
            Columns ``test_date`` (ISO string), ``projected_value``,
            ``threshold``, ``headroom``, ``breach_probability``,
            ``breach_probability_stderr``; ``None`` where a value is not
            meaningful.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the forecast covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def covenant_description(self) -> str:
        """
        Human-readable covenant description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 4.50x"``.
        """

    @property
    def comparator(self) -> str:
        """
        Direction the activation metric is tested in.

        This property does not raise.

        Returns
        -------
        str
            ``"at_most"`` or ``"at_least"``.
        """

    @property
    def test_dates(self) -> list[datetime.date]:
        """
        Forecast test dates.

        This property does not raise.

        Returns
        -------
        list[datetime.date]
            One per frame row, ascending.
        """

    @property
    def projected_values(self) -> list[float | None]:
        """
        Projected metric per test date.

        This property does not raise.

        Returns
        -------
        list[float | None]
            ``None`` where the projection is not finite.
        """

    @property
    def thresholds(self) -> list[float]:
        """
        Threshold in force per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            Static threshold or step-down schedule value.
        """

    @property
    def headroom(self) -> list[float | None]:
        """
        Relative headroom per test date.

        This property does not raise.

        Returns
        -------
        list[float | None]
            ``None`` while a springing covenant is inactive or the ratio is
            not meaningful (negative EBITDA).
        """

    @property
    def breach_probability(self) -> list[float]:
        """
        Breach probability per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            ``0.0`` / ``1.0`` in deterministic mode.
        """

    @property
    def breach_probability_stderr(self) -> list[float]:
        """
        Monte Carlo standard error per test date.

        This property does not raise.

        Returns
        -------
        list[float]
            Zeros in deterministic and analytic modes.
        """

    @property
    def first_breach_date(self) -> datetime.date | None:
        """
        First projected breach.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` when no date breaches.
        """

    @property
    def min_headroom_date(self) -> datetime.date | None:
        """
        Date of minimum finite headroom.

        This property does not raise.

        Returns
        -------
        datetime.date | None
            ``None`` when no headroom is finite.
        """

    @property
    def min_headroom_value(self) -> float | None:
        """
        Minimum finite headroom across active dates.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when no headroom is finite.
        """

    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

class FutureBreach:
    """
    A projected covenant breach on one forecast date.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, cov_lite, forecast_breaches
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = pd.DataFrame(
    ...     {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 3.5]},
    ...     index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    ... )
    >>> breaches = forecast_breaches(engine, frame, CovenantForecastConfig().with_scope("incurrence"))
    >>> [(b.covenant_id, b.breach_date.isoformat()) for b in breaches]
    [('max_total_leverage', '2026-06-30')]
    """

    @staticmethod
    def from_json(json: str) -> FutureBreach:
        """
        Deserialize from JSON.

        Parameters
        ----------
        json : str
            JSON matching the ``FutureBreach`` wire schema.

        Returns
        -------
        FutureBreach
            Parsed breach.

        Raises
        ------
        ValueError
            If ``json`` is malformed.

        Examples
        --------
        >>> from finstack_quant.covenants import FutureBreach
        >>> raw = '{"covenant_id": "x", "covenant_description": "DSCR >= 1.20x", "breach_date": "2026-06-30", "projected_value": 1.1, "threshold": 1.2, "headroom": -0.0833, "breach_probability": 1.0}'
        >>> FutureBreach.from_json(raw).breach_probability
        1.0
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """

    @property
    def covenant_id(self) -> str:
        """
        Label of the covenant.

        This property does not raise.

        Returns
        -------
        str
            Matches ``Covenant.label``.
        """

    @property
    def covenant_description(self) -> str:
        """
        Human-readable covenant description.

        This property does not raise.

        Returns
        -------
        str
            For example ``"Total Leverage <= 7.00x"``.
        """

    @property
    def breach_date(self) -> datetime.date:
        """
        Forecast date of the breach.

        This property does not raise.

        Returns
        -------
        datetime.date
            Breach date.
        """

    @property
    def projected_value(self) -> float | None:
        """
        Projected metric value.

        This property does not raise.

        Returns
        -------
        float | None
            ``None`` when not finite.
        """

    @property
    def threshold(self) -> float:
        """
        Threshold in force on the breach date.

        This property does not raise.

        Returns
        -------
        float
            Static or scheduled threshold.
        """

    @property
    def headroom(self) -> float | None:
        """
        Relative headroom.

        This property does not raise.

        Returns
        -------
        float | None
            Negative means breach; ``None`` when not meaningful.
        """

    @property
    def breach_probability(self) -> float:
        """
        Breach probability.

        This property does not raise.

        Returns
        -------
        float
            ``1.0`` in deterministic mode.
        """

    def __eq__(self, other: object) -> bool: ...

def lbo_standard(
    initial_leverage: float,
    interest_coverage: float,
    fixed_charge_coverage: float,
    max_capex: float,
) -> list[CovenantSpec]:
    """
    Standard leveraged-buyout covenant package.

    Quarterly maintenance tests for maximum gross Debt/EBITDA, minimum
    interest coverage and minimum fixed-charge coverage, plus an annual
    maximum-capex test. Leverage and interest coverage carry 30-day cure
    periods; a leverage breach steps the rate up 200bp and a coverage breach
    blocks distributions. Labels: ``max_debt_ebitda``,
    ``min_interest_coverage``, ``min_fcc``, ``max_capex``; metrics:
    ``debt_to_ebitda``, ``interest_coverage``, ``fixed_charge_coverage``,
    ``capex``.

    Parameters
    ----------
    initial_leverage : float
        Maximum gross Debt/EBITDA in turns (``6.0`` for 6.0x).
    interest_coverage : float
        Minimum interest coverage ratio in turns.
    fixed_charge_coverage : float
        Minimum fixed-charge coverage ratio in turns.
    max_capex : float
        Maximum annual capex as a reporting-currency amount.

    Returns
    -------
    list[CovenantSpec]
        Four specs in the order above.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> from finstack_quant.covenants import lbo_standard
    >>> [s.covenant.label for s in lbo_standard(6.0, 2.0, 1.1, 50_000_000.0)]
    ['max_debt_ebitda', 'min_interest_coverage', 'min_fcc', 'max_capex']
    """

def cov_lite(max_leverage: float, max_senior_leverage: float) -> list[CovenantSpec]:
    """
    Covenant-lite leveraged-loan package (incurrence tests only).

    Maximum total leverage, maximum senior leverage, and an annual negative
    covenant restricting additional secured debt. Labels:
    ``max_total_leverage``, ``max_senior_leverage``, ``negative``; metrics:
    ``total_leverage``, ``senior_leverage``.

    Parameters
    ----------
    max_leverage : float
        Maximum total Debt/EBITDA in turns.
    max_senior_leverage : float
        Maximum senior Debt/EBITDA in turns.

    Returns
    -------
    list[CovenantSpec]
        Three specs in the order above.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> from finstack_quant.covenants import cov_lite
    >>> [s.covenant.scope for s in cov_lite(7.0, 4.5)]
    ['incurrence', 'incurrence', 'incurrence']
    """

def real_estate(min_dscr: float, min_debt_yield: float, max_ltv: float) -> list[CovenantSpec]:
    """
    Commercial real-estate covenant package.

    Quarterly maintenance tests for minimum DSCR (30-day cure, 100% cash
    sweep), minimum debt yield and maximum LTV (50% cash sweep). Labels:
    ``min_dscr``, ``min_debt_yield``, ``max_ltv``; metrics: ``dscr``,
    ``debt_yield``, ``ltv``.

    Parameters
    ----------
    min_dscr : float
        Minimum debt-service coverage ratio in turns (``1.25`` for 1.25x).
    min_debt_yield : float
        Minimum NOI / loan balance as a decimal fraction (``0.08`` for 8%).
    max_ltv : float
        Maximum loan-to-value as a decimal fraction (``0.75`` for 75%).

    Returns
    -------
    list[CovenantSpec]
        Three specs in the order above.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> from finstack_quant.covenants import real_estate
    >>> [s.metric_id for s in real_estate(1.25, 0.08, 0.75)]
    ['dscr', 'debt_yield', 'ltv']
    """

def project_finance(
    min_dscr: float,
    distribution_lockup_dscr: float,
    min_liquidity: float,
    max_net_leverage: float,
) -> list[CovenantSpec]:
    """
    Infrastructure / project-finance covenant package.

    Quarterly maintenance tests for a default DSCR (60-day cure, event of
    default), a higher distribution lock-up DSCR (blocks distributions),
    minimum debt-service-reserve liquidity and maximum net Debt/EBITDA.
    Labels: ``min_dscr_default``, ``min_dscr_lockup``, ``min_liquidity``,
    ``max_net_debt_ebitda``; metrics: ``dscr``, ``dscr``, ``liquidity``,
    ``net_debt_to_ebitda``.

    Parameters
    ----------
    min_dscr : float
        Minimum DSCR in turns whose breach leads to default.
    distribution_lockup_dscr : float
        Higher DSCR in turns below which distributions are blocked.
    min_liquidity : float
        Minimum debt-service reserve as a reporting-currency amount.
    max_net_leverage : float
        Maximum net Debt/EBITDA in turns.

    Returns
    -------
    list[CovenantSpec]
        Four specs in the order above.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> from finstack_quant.covenants import project_finance
    >>> [s.covenant.label for s in project_finance(1.2, 1.1, 10.0, 7.0)]
    ['min_dscr_default', 'min_dscr_lockup', 'min_liquidity', 'max_net_debt_ebitda']
    """

def forecast_covenant(
    spec: CovenantSpec,
    metrics: pd.DataFrame,
    config: CovenantForecastConfig | None = None,
) -> CovenantForecast:
    """
    Forecast one numeric covenant across a date-indexed projection frame.

    Each frame row is a forecast test date. The metric is resolved from the
    spec's explicit ``metric_id`` exclusively when present; otherwise it uses
    the covenant type's conventional metric name,
    then a custom covenant's ``metric``. Threshold schedules and springing
    conditions are honoured (the activation metric must be a frame column).

    Parameters
    ----------
    spec : CovenantSpec
        Numeric covenant to forecast.
    metrics : pd.DataFrame
        Index holds forecast dates; columns are metric ids; ``NaN`` cells are
        treated as absent.
    config : CovenantForecastConfig | None
        Forecast policy; ``None`` is deterministic.

    Returns
    -------
    CovenantForecast
        Per-date projections, thresholds, headroom and breach probabilities.

    Raises
    ------
    KeyError
        If the covenant's metric is absent on any date.
    ValueError
        If the frame is empty, the covenant is non-numeric or has a
        non-finite threshold, or the config is invalid (stochastic without
        ``volatility``, antithetic without paths, threshold outside
        ``[0, 1]``).

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import (
    ...     Covenant,
    ...     CovenantForecastConfig,
    ...     CovenantSpec,
    ...     CovenantType,
    ...     forecast_covenant,
    ... )
    >>> spec = CovenantSpec(Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"), "debt_to_ebitda")
    >>> frame = pd.DataFrame({"debt_to_ebitda": [4.0, 4.4]}, index=pd.to_datetime(["2026-03-31", "2026-06-30"]))
    >>> cfg = CovenantForecastConfig(stochastic=True, volatility=0.25, reference_date="2025-12-31")
    >>> forecast = forecast_covenant(spec, frame, cfg)
    >>> [0.0 < p < 1.0 for p in forecast.breach_probability]
    [True, True]
    """

def forecast_breaches(
    engine: CovenantEngine,
    metrics: pd.DataFrame,
    config: CovenantForecastConfig | None = None,
) -> list[FutureBreach]:
    """
    Forecast every active numeric covenant in an engine and collect the
    dates whose breach probability reaches the config threshold.

    Effective windows, waivers, schedules, and scope are honored. Missing or
    non-finite required observations fail the entire batch. Inactive and
    non-numeric covenants are skipped. Deterministic breaches are always retained.

    Parameters
    ----------
    engine : CovenantEngine
        Engine whose effective specifications and amendments are forecast.
    metrics : pd.DataFrame
        Index holds forecast dates; columns are metric ids.
    config : CovenantForecastConfig | None
        Forecast policy; ``None`` is deterministic with a 5% reporting
        threshold.

    Returns
    -------
    list[FutureBreach]
        Breaches sorted by date then covenant id; empty when no applicable test breaches.

    Raises
    ------
    KeyError
        If any active numeric test lacks a required metric at a requested date.
    ValueError
        If the frame is empty, has duplicate metric columns or non-finite required
        observations, or the engine, date ordering, or configuration is invalid.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.covenants import (
    ...     CovenantEngine,
    ...     CovenantForecastConfig,
    ...     breaches_to_dataframe,
    ...     cov_lite,
    ...     forecast_breaches,
    ... )
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = pd.DataFrame(
    ...     {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 5.0]},
    ...     index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    ... )
    >>> breaches_to_dataframe(forecast_breaches(engine, frame, CovenantForecastConfig().with_scope("incurrence")))[
    ...     "covenant_id"
    ... ].tolist()
    ['max_senior_leverage', 'max_total_leverage']
    """

def breaches_to_dataframe(breaches: list[FutureBreach]) -> pd.DataFrame:
    """
    Flatten forecast breaches into one frame row per breach.

    Parameters
    ----------
    breaches : list[FutureBreach]
        Output of :func:`forecast_breaches`.

    Returns
    -------
    pd.DataFrame
        Columns ``covenant_id``, ``covenant_description``, ``breach_date``
        (ISO string), ``projected_value``, ``threshold``, ``headroom``,
        ``breach_probability``; typed and empty when there are no breaches.

    Raises
    ------
    TypeError
        If an element is not a ``FutureBreach``.

    Examples
    --------
    >>> from finstack_quant.covenants import breaches_to_dataframe
    >>> list(breaches_to_dataframe([]).columns)
    ['covenant_id', 'covenant_description', 'breach_date', 'projected_value', 'threshold', 'headroom', 'breach_probability']
    """

def reports_to_dataframe(reports: dict[str, CovenantReport]) -> pd.DataFrame:
    """
    Flatten an ``evaluate`` result into one frame row per covenant.

    Parameters
    ----------
    reports : dict[str, CovenantReport]
        Output of :meth:`CovenantEngine.evaluate` or :func:`evaluate_engine`.

    Returns
    -------
    pd.DataFrame
        Columns ``covenant`` (dict key), ``covenant_type``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``details``, in dict
        order.

    Raises
    ------
    TypeError
        If a value is not a ``CovenantReport``.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, cov_lite, reports_to_dataframe
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = reports_to_dataframe(engine.evaluate({"total_leverage": 5.0, "senior_leverage": 3.0}, "2026-03-31"))
    >>> frame["covenant"].tolist()
    ['max_total_leverage', 'max_senior_leverage', 'negative']
    """

def validate_covenant_spec_json(spec_json: str) -> str:
    """
    Validate and canonicalize a covenant specification JSON string.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CovenantSpec`` (for example ``CovenantSpec.to_json()``
        or one element of a ``*_json`` template).

    Returns
    -------
    str
        Canonical JSON after validation: object keys sorted recursively,
        arrays in semantic order, no insignificant whitespace.

    Raises
    ------
    ValueError
        If the spec fails schema or semantic validation (unknown fields,
        non-finite threshold, negative cure period, invalid schedule).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import lbo_standard_json, validate_covenant_spec_json
    >>> spec = json.loads(lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))[0]
    >>> canonical = json.loads(validate_covenant_spec_json(json.dumps(spec)))
    >>> list(canonical)
    ['covenant', 'metric_id']
    >>> list(canonical["covenant"])[:3]
    ['consequences', 'covenant_type', 'cure_period_days']
    """

def validate_covenant_report_json(report_json: str) -> str:
    """
    Validate and canonicalize a covenant evaluation report JSON string.

    Parameters
    ----------
    report_json : str
        JSON-encoded ``CovenantReport`` with pass/fail and headroom per covenant.

    Returns
    -------
    str
        Canonical JSON after validation, with object keys sorted recursively
        and the audit ``meta`` stamp filled in.

    Raises
    ------
    ValueError
        If the report JSON is malformed or fails validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import validate_covenant_report_json
    >>> report = {
    ...     "covenant_type": "Debt/EBITDA <= 5.00x",
    ...     "covenant_id": "max_debt_ebitda",
    ...     "passed": False,
    ...     "actual_value": 5.5,
    ...     "threshold": 5.0,
    ...     "details": "Exceeded",
    ...     "headroom": -0.1,
    ... }
    >>> canonical = json.loads(validate_covenant_report_json(json.dumps(report)))
    >>> canonical["passed"], list(canonical)[:2]
    (False, ['actual_value', 'covenant_id'])
    """

def validate_covenant_engine_json(engine_json: str) -> str:
    """
    Validate and canonicalize a covenant engine JSON string.

    Parameters
    ----------
    engine_json : str
        JSON-encoded engine document. Only ``specs`` is required;
        ``breach_history``, ``windows`` and ``waivers`` default to empty.
        Unknown fields are rejected.

    Returns
    -------
    str
        Canonical JSON after validation, with object keys sorted recursively
        and the defaulted arrays written out.

    Raises
    ------
    ValueError
        If the engine JSON is malformed or fails validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import cov_lite_json, validate_covenant_engine_json
    >>> engine = {"specs": json.loads(cov_lite_json(7.0, 4.5))}
    >>> list(json.loads(validate_covenant_engine_json(json.dumps(engine))))
    ['breach_history', 'specs', 'waivers', 'windows']
    """

def evaluate_engine(engine_json: str, metrics: dict[str, float] | str, as_of: DateLike) -> dict[str, CovenantReport]:
    """
    Evaluate a covenant engine JSON document against a metric mapping.

    JSON twin of :meth:`CovenantEngine.evaluate`; prefer the typed engine
    when you build the package in Python.

    Parameters
    ----------
    engine_json : str
        Serialized engine (``CovenantEngine.to_json()`` or a hand-written
        document; only ``specs`` is required).
    metrics : dict[str, float] | str
        Metric values keyed by metric id, or a JSON object string. Ratios in
        turns, amounts in the reporting currency.
    as_of : datetime.date | str
        Evaluation date, a date-like object or an ISO 8601 string.

    Returns
    -------
    dict[str, CovenantReport]
        Typed report per covenant keyed by covenant label, in spec order.

    Raises
    ------
    KeyError
        If a required metric is missing from ``metrics``.
    ValueError
        If the engine document or a metric value is invalid, or the date
        string is not ISO 8601.
    TypeError
        If ``metrics`` is neither a dict nor a string, or ``as_of`` is
        neither a string nor date-like.

    Examples
    --------
    >>> from finstack_quant.covenants import evaluate_engine, lbo_standard_json
    >>> engine = '{"specs": ' + lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0) + "}"
    >>> metrics = {"debt_to_ebitda": 4.0, "interest_coverage": 3.0, "fixed_charge_coverage": 1.5, "capex": 5_000_000.0}
    >>> reports = evaluate_engine(engine, metrics, "2026-03-31")
    >>> reports["max_debt_ebitda"].passed, list(reports)[0]
    (True, 'max_debt_ebitda')
    """

def lbo_standard_json(
    initial_leverage: float,
    interest_coverage: float,
    fixed_charge_coverage: float,
    max_capex: float,
) -> str:
    """
    Standard leveraged-buyout covenant package as JSON (twin of :func:`lbo_standard`).

    Parameters
    ----------
    initial_leverage : float
        Maximum gross Debt/EBITDA in turns (``6.0`` for 6.0x).
    interest_coverage : float
        Minimum interest coverage ratio in turns.
    fixed_charge_coverage : float
        Minimum fixed-charge coverage ratio in turns.
    max_capex : float
        Maximum annual capex as a reporting-currency amount.

    Returns
    -------
    str
        JSON array of four ``CovenantSpec`` objects; wrap as
        ``{"specs": [...]}`` for an engine document.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import lbo_standard_json
    >>> len(json.loads(lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0)))
    4
    """

def cov_lite_json(max_leverage: float, max_senior_leverage: float) -> str:
    """
    Covenant-lite package as JSON (twin of :func:`cov_lite`).

    Parameters
    ----------
    max_leverage : float
        Maximum total Debt/EBITDA in turns.
    max_senior_leverage : float
        Maximum senior Debt/EBITDA in turns.

    Returns
    -------
    str
        JSON array of three incurrence ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import cov_lite_json
    >>> len(json.loads(cov_lite_json(7.0, 4.5)))
    3
    """

def real_estate_json(min_dscr: float, min_debt_yield: float, max_ltv: float) -> str:
    """
    Real-estate covenant package as JSON (twin of :func:`real_estate`).

    Parameters
    ----------
    min_dscr : float
        Minimum debt-service coverage ratio in turns.
    min_debt_yield : float
        Minimum debt yield as a decimal fraction (``0.08`` for 8%); encoded
        as a custom minimum covenant on metric ``debt_yield``.
    max_ltv : float
        Maximum loan-to-value as a decimal fraction (``0.75`` for 75%);
        encoded as a custom maximum covenant on metric ``ltv``.

    Returns
    -------
    str
        JSON array of three ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import real_estate_json
    >>> len(json.loads(real_estate_json(1.25, 0.08, 0.75)))
    3
    """

def project_finance_json(
    min_dscr: float,
    distribution_lockup_dscr: float,
    min_liquidity: float,
    max_net_leverage: float,
) -> str:
    """
    Project-finance covenant package as JSON (twin of :func:`project_finance`).

    Parameters
    ----------
    min_dscr : float
        Minimum DSCR in turns whose breach leads to default.
    distribution_lockup_dscr : float
        Higher DSCR in turns below which distributions are blocked.
    min_liquidity : float
        Minimum debt-service reserve as a reporting-currency amount.
    max_net_leverage : float
        Maximum net Debt/EBITDA in turns.

    Returns
    -------
    str
        JSON array of four ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import project_finance_json
    >>> len(json.loads(project_finance_json(1.30, 1.10, 5_000_000.0, 5.0)))
    4
    """
