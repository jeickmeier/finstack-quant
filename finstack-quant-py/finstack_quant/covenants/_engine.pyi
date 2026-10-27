"""Stub declarations mirroring ``finstack-quant-py/src/bindings/covenants/engine.rs``."""

from __future__ import annotations

import datetime
import pandas as pd
from finstack_quant.covenants import DateLike
from finstack_quant.covenants._report import CovenantReport
from finstack_quant.covenants._spec import CovenantConsequence, CovenantSpec, CovenantWaiver

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
