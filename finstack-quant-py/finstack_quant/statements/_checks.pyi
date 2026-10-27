"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/checks.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class CheckSuiteSpec:
    """
    A serializable suite specification describing which checks to run.

    Load from JSON (e.g. a team-wide check policy file) and inspect its
    composition (``builtin_check_count`` / ``formula_check_count``). Note:
    running a suite is not yet exposed through the Python bindings; this type is
    currently for loading and inspecting a policy definition only.

    Examples
    --------
    >>> from finstack_quant.statements import CheckSuiteSpec
    >>> suite = CheckSuiteSpec.from_json('{"name":"basic","builtin_checks":[],"formula_checks":[]}')
    >>> (suite.name, suite.builtin_check_count, suite.formula_check_count)
    ('basic', 0, 0)

    """

    def __init__(
        self,
        name: str,
        builtin_checks: list[dict[str, Any]] | None = None,
        formula_checks: list[FormulaCheckSpec] | None = None,
        config: CheckConfig | None = None,
        description: str | None = None,
    ) -> None:
        """Compose a check policy with validated default configuration thresholds.

        Parameters
        ----------
        name : str
            Suite label used in display and logging.
        builtin_checks : list[dict] | None
            Ordered built-in specifications, each the Rust ``BuiltinCheckSpec``
            serde form: a dict with the check's canonical ``type`` tag plus its
            node fields, e.g. ``{"type": "non_finite"}`` or ``{"type":
            "balance_sheet_articulation", "assets_nodes": ["assets"],
            "liabilities_nodes": ["liabilities"], "equity_nodes":
            ["equity"]}``. None means no built-in checks.
        formula_checks : list[FormulaCheckSpec] | None
            Ordered DSL predicate or tolerance-based residual checks. None means no
            formula checks; syntax, node references, and per-check tolerances are
            validated when the suite executes against a model.
        config : CheckConfig | None
            Tolerance and reporting policy with finite, non-negative numeric
            thresholds. None uses the Rust ``CheckConfig`` defaults.
        description : str | None
            Optional human-readable explanation of the policy.

        Raises
        ------
        ValueError
            If a built-in check ``type`` is unknown, a dict lacks a required
            field, an entry is not a ``dict``, or a configuration threshold is
            negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.statements import CheckSuiteSpec
        >>> CheckSuiteSpec("quality", builtin_checks=[{"type": "non_finite"}]).builtin_check_count
        1
        """
        ...

    @staticmethod
    def from_json(json: str) -> CheckSuiteSpec:
        """
        Deserialize a suite specification from JSON text.

        Parameters
        ----------
        json:
            JSON document matching the ``CheckSuiteSpec`` schema.

        Returns
        -------
        CheckSuiteSpec
            Parsed specification.

        Raises
        ------
        ValueError
            If ``json`` is malformed, fails schema validation, or a default
            configuration tolerance/materiality threshold is negative or non-finite.

        Notes
        -----
        Formula syntax, node references, and per-check tolerances are validated
        when the suite executes against a model.

        Examples
        --------
        >>> from finstack_quant.statements import CheckSuiteSpec
        >>> suite = CheckSuiteSpec.from_json('{"name":"basic","builtin_checks":[],"formula_checks":[]}')
        >>> suite.name
        'basic'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this specification to compact canonical JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `CheckSuiteSpec`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def name(self) -> str:
        """
        Suite name, used for display and logging.

        Returns
        -------
        str
            The suite's name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def builtin_check_count(self) -> int:
        """
        Number of built-in checks the spec will materialize.

        Built-ins are the crate-provided accounting-identity, reconciliation,
        and data-quality checks, selected by their ``type`` tag in the spec.

        Returns
        -------
        int
            Count of built-in check entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def formula_check_count(self) -> int:
        """
        Number of user-defined formula checks the spec will materialize.

        Formula checks are DSL expressions evaluated per period; their syntax
        is validated when the suite runs, not when the spec is loaded.

        Returns
        -------
        int
            Count of formula check entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the suite spec.
        Returns
        -------
        str
        """
        ...

    @property
    def description(self) -> str | None:
        """Return the optional human-readable check-policy description.

        Returns
        -------
        str | None
            Stored description, or None when omitted.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def builtin_checks(self) -> list[dict[str, Any]]:
        """Return built-in check specifications as canonical Python containers.

        Returns
        -------
        list[dict[str, Any]]
            Tagged built-in policies in suite order; their amounts and tolerances use the referenced nodes' units.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def formula_checks(self) -> list[FormulaCheckSpec]:
        """Return configured formula checks in execution order.

        Returns
        -------
        list[FormulaCheckSpec]
            Copies of predicate or tolerance-based residual specifications.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def config(self) -> CheckConfig:
        """Return the tolerances and severity policy used by this suite.

        Returns
        -------
        CheckConfig
            A copy of the suite's check configuration.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @staticmethod
    def builtin_check_names() -> list[str]:
        """List the canonical built-in check discriminants.

        Returns
        -------
        list[str]
            Names accepted in built-in policy JSON, including "balance_sheet_articulation" and "non_finite".

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.

        Examples
        --------
        >>> from finstack_quant.statements import CheckSuiteSpec
        >>> "balance_sheet_articulation" in CheckSuiteSpec.builtin_check_names()
        True
        """
        ...

class CheckReport:
    """
    Validation check report aggregating results and summary statistics.

    Loaded from JSON (``from_json``) produced by the Rust checks framework,
    then inspected via properties or rendered to text/HTML.

    Examples
    --------
    >>> from finstack_quant.statements import CheckReport
    >>> report = CheckReport.from_json(
    ...     '{"results":[],"summary":{"total_checks":0,"passed":0,"failed":0,"errors":0,"warnings":0,"infos":0}}'
    ... )
    >>> (report.passed, report.total_findings)
    (True, 0)

    """

    @staticmethod
    def from_json(json: str) -> CheckReport:
        """
        Deserialize a check report from JSON text.

        Parameters
        ----------
        json:
            JSON document matching the ``CheckReport`` schema.

        Returns
        -------
        CheckReport
            Parsed report.

        Raises
        ------
        ValueError
            If ``json`` is not valid or fails schema validation.

        Examples
        --------
        >>> from finstack_quant.statements import CheckReport
        >>> payload = (
        ...     '{"results":[],"summary":{"total_checks":0,"passed":0,"failed":0,"errors":0,"warnings":0,"infos":0}}'
        ... )
        >>> CheckReport.from_json(payload).total_checks
        0

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this report to pretty-printed JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `CheckReport`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def passed(self) -> bool:
        """
        Whether the whole report passed: no error-severity finding was retained
        by any check.

        Warnings and info findings do not fail a report. A suite's materiality
        threshold never suppresses error findings, so it cannot flip this to
        ``True``.

        Returns
        -------
        bool
            ``True`` when zero error-severity findings were retained.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_checks(self) -> int:
        """
        Number of checks that ran, one per row of :meth:`to_dataframe`.

        Reads the canonical Rust ``CheckSummary.total_checks`` counter.

        Returns
        -------
        int
            Count of check results in the report.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def has_errors(self) -> bool:
        """
        Whether the report contains at least one error-severity finding.

        Delegates to the canonical Rust ``CheckReport::has_errors``.

        Returns
        -------
        bool
            ``True`` when any retained finding has error severity.

        Notes
        -----
        This method does not raise; it reads the stored summary.

        Examples
        --------
        >>> from finstack_quant.statements import CheckReport
        >>> report = CheckReport.from_json(
        ...     '{"results": [], "summary": {"total_checks": 0, "passed": 0,'
        ...     ' "failed": 0, "errors": 0, "warnings": 0, "infos": 0}}'
        ... )
        >>> report.has_errors()
        False
        """
        ...

    def has_warnings(self) -> bool:
        """
        Whether the report contains at least one warning-severity finding.

        Delegates to the canonical Rust ``CheckReport::has_warnings``.

        Returns
        -------
        bool
            ``True`` when any retained finding has warning severity.

        Notes
        -----
        This method does not raise; it reads the stored summary.

        Examples
        --------
        >>> from finstack_quant.statements import CheckReport
        >>> report = CheckReport.from_json(
        ...     '{"results": [], "summary": {"total_checks": 0, "passed": 0,'
        ...     ' "failed": 0, "errors": 0, "warnings": 0, "infos": 0}}'
        ... )
        >>> report.has_warnings()
        False
        """
        ...

    def findings_by_severity(self, severity: str) -> list[CheckFinding]:
        """
        Return all retained findings of one severity as CheckFinding objects.

        Delegates to the canonical Rust ``CheckReport::findings_by_severity``.

        Parameters
        ----------
        severity:
            One of ``"info"``, ``"warning"``, ``"error"`` (the canonical
            snake_case severity discriminants).

        Returns
        -------
        list[CheckFinding]
            One ``CheckFinding`` object per matching retained finding.

        Raises
        ------
        ValueError
            If ``severity`` is not a canonical severity discriminant.

        Examples
        --------
        >>> from finstack_quant.statements import CheckReport
        >>> report = CheckReport.from_json(
        ...     '{"results": [], "summary": {"total_checks": 0, "passed": 0,'
        ...     ' "failed": 0, "errors": 0, "warnings": 0, "infos": 0}}'
        ... )
        >>> report.findings_by_severity("error")
        []
        """
        ...

    @property
    def total_findings(self) -> int:
        """
        Number of retained findings across all checks.

        Counts what survived the suite's ``min_severity`` and
        ``materiality_threshold`` reporting filters, not every raw diagnostic
        the checks produced — it matches the row count of
        :meth:`to_findings_dataframe`. Delegates to the Rust
        ``CheckReport::total_findings``.

        Returns
        -------
        int
            Retained error, warning and info findings combined.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_errors(self) -> int:
        """
        Number of retained error-severity findings across all checks.

        Returns
        -------
        int
            Retained error-severity finding count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_warnings(self) -> int:
        """
        Number of retained warning-severity findings across all checks.

        Returns
        -------
        int
            Retained warning-severity finding count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export one row per executed check as a pandas ``DataFrame``.

        Columns: ``check_id`` (stable identifier), ``check_name``
        (human-readable name), ``category`` (snake_case group, one of
        ``accounting_identity``, ``cross_statement_reconciliation``,
        ``internal_consistency``, ``credit_reasonableness``, ``data_quality``),
        ``passed`` (``True`` when the check retained no error-severity
        finding), ``finding_count`` (retained findings for this check, after
        the suite's reporting filters).

        One row per check result, so the row count equals
        :attr:`total_checks`. A report with no checks still yields the
        documented columns with zero rows. Per-finding detail — severity,
        message, and the observed-vs-reference materiality — lives in
        :meth:`to_findings_dataframe`.

        Returns
        -------
        pd.DataFrame
            One row per check result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_findings_dataframe(self) -> pd.DataFrame:
        """
        Export one row per retained finding as a pandas ``DataFrame``.

        Columns: ``check_id``, ``check_name``, ``category`` (as in
        :meth:`to_dataframe`), ``severity`` (``info``, ``warning`` or
        ``error``), ``message`` (human-readable description), ``period``
        (period identifier string such as ``"2025Q1"``, ``None`` when the
        finding is not period-specific), ``nodes`` (comma-joined node
        identifiers involved, empty string when none), ``materiality_absolute``
        (size of the discrepancy in the compared nodes' own units — currency
        amounts for monetary nodes), ``materiality_relative_pct`` (that
        discrepancy as a **percentage** of the reference value, i.e. already
        multiplied by 100, unlike the decimal-fraction rates used elsewhere in
        this module), ``materiality_reference_value`` (the denominator used),
        ``materiality_reference_label`` (what that denominator is, e.g.
        ``total_assets``), and ``comparison_actual``, ``comparison_expected``,
        ``comparison_tolerance`` (the two values the check compared and the
        absolute tolerance it allowed on their difference, all in the compared
        nodes' own units).

        The four materiality columns are ``None`` for findings that carry no
        materiality context, and the three comparison columns are ``None`` for
        findings that do not compare two numbers. The row count equals
        :attr:`total_findings`; a report with no findings still yields the
        documented columns with zero rows.

        Returns
        -------
        pd.DataFrame
            One row per retained finding.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_comparisons_dataframe(self) -> pd.DataFrame:
        """
        Export one row per numeric comparison as a pandas ``DataFrame``.

        Lists every comparison the checks evaluated — passing and failing
        alike — so a passing check still shows the numbers it was judged on.
        Unlike :meth:`to_findings_dataframe`, rows are not reduced by the
        suite's ``min_severity`` / ``materiality_threshold`` filters.

        Columns: ``check_id``, ``check_name``, ``category`` (as in
        :meth:`to_dataframe`), ``identity`` (snake_case name of the tested
        identity, e.g. ``balance_sheet_articulation``, ``cash_roll_forward``,
        ``cash_flow_components``), ``period`` (period identifier string,
        ``None`` when not period-specific), ``actual`` (observed value),
        ``expected`` (value the identity requires), ``difference``
        (``actual - expected``), ``tolerance`` (absolute tolerance applied to
        ``|difference|``) and ``within_tolerance`` (``True`` when
        ``|difference| <= tolerance``). ``actual``, ``expected``,
        ``difference`` and ``tolerance`` are in the compared nodes' own units
        (currency amounts for monetary nodes).

        Checks that do not compare two numbers, and periods a check skipped
        for missing inputs, contribute no rows. A report with no comparisons
        still yields the documented columns with zero rows.

        Returns
        -------
        pd.DataFrame
            One row per comparison, in check then evaluation order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary of the check report.
        Returns
        -------
        str
        """
        ...

    @property
    def findings(self) -> list[CheckFinding]:
        """Return retained findings flattened across executed checks.

        Returns
        -------
        list[CheckFinding]
            Findings in check execution order after severity and materiality filtering.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

class CheckConfig:
    """Tolerances and severity floor applied by a check suite.

    Examples
    --------
    >>> from finstack_quant.statements import CheckConfig
    >>> CheckConfig(default_tolerance=0.05).default_tolerance
    0.05

    """

    def __init__(
        self,
        default_tolerance: float | None = None,
        default_relative_tolerance: float | None = None,
        materiality_threshold: float | None = None,
        min_severity: str | None = None,
    ) -> None:
        """Configure check tolerances.

        Every parameter defaults to the Rust ``CheckConfig::default()`` value
        when omitted or ``None``.

        Parameters
        ----------
        default_tolerance : float | None
            Finite, non-negative absolute tolerance in the checked node's units
            (Rust default ``0.01``).
        default_relative_tolerance : float | None
            Finite, non-negative relative tolerance as a decimal fraction of
            the reference magnitude; zero disables relative tolerance (Rust
            default ``1e-9``).
        materiality_threshold : float | None
            Finite, non-negative absolute magnitude below which advisory
            findings are suppressed; error findings are retained (Rust default
            ``0.0``).
        min_severity : str | None
            Lowest severity retained: ``"info"`` (Rust default), ``"warning"``
            or ``"error"``.

        Raises
        ------
        ValueError
            If ``min_severity`` is unknown or a tolerance/materiality threshold is negative or non-finite.

        """
        ...

    @property
    def default_tolerance(self) -> float:
        """
        Absolute tolerance in the checked node's units.

        This property does not raise.

        Returns
        -------
        float
            Absolute tolerance in the checked node's units.
        """
        ...

    @property
    def default_relative_tolerance(self) -> float:
        """
        Relative tolerance as a decimal fraction.

        This property does not raise.

        Returns
        -------
        float
            Relative tolerance as a decimal fraction.
        """
        ...

    @property
    def materiality_threshold(self) -> float:
        """
        Absolute magnitude below which advisory findings are suppressed.

        This property does not raise.

        Returns
        -------
        float
            Absolute advisory-reporting threshold in the checked nodes' units;
            error findings are always retained.
        """
        ...

    @property
    def min_severity(self) -> str:
        """
        Lowest severity retained in the report.

        Returns
        -------
        str
            Lowest severity retained in the report.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
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
    def from_json(json: str, /) -> CheckConfig:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        CheckConfig
            Reconstructed configuration.

        Raises
        ------
        ValueError
            If the payload is malformed or a tolerance/materiality threshold is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.statements import CheckConfig
        >>> CheckConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...
    def validate(self) -> None:
        """Validate finite, non-negative check tolerances and materiality thresholds.

        Raises
        ------
        ValueError
            If a tolerance or the materiality threshold is negative or non-finite.
        """
        ...

class FormulaCheckSpec:
    """A user-defined check expressed as a statements DSL predicate.

    Each period passes, fails (a finding with the configured severity), or
    has insufficient data: a same-period input is missing or non-finite, or
    the formula consumes a non-finite value such as a ``lag`` before the first
    period. Insufficient data is reported as a ``"warning"`` finding naming
    the inputs and never fails the check. A division by zero over finite
    inputs still fails.

    Examples
    --------
    >>> from finstack_quant.statements import FormulaCheckSpec
    >>> FormulaCheckSpec(
    ...     "c1", "Positive revenue", "revenue > 0", "revenue <= 0", "internal_consistency", "error"
    ... ).severity
    'error'

    """

    def __init__(
        self,
        id: str,
        name: str,
        formula: str,
        message_template: str,
        category: str,
        severity: str,
        tolerance: float | None = None,
    ) -> None:
        """Define a formula check.

        Parameters
        ----------
        id : str
            Check identifier, unique within a suite.
        name : str
            Human-readable label.
        formula : str
            DSL predicate that must hold; a false result raises a finding.
        message_template : str
            Message emitted when the predicate fails.
        category : str
            Classification bucket: ``"accounting_identity"``,
            ``"cross_statement_reconciliation"``, ``"internal_consistency"``,
            ``"credit_reasonableness"`` or ``"data_quality"``. Required, as in
            the Rust/JSON form.
        severity : str
            ``"info"``, ``"warning"`` or ``"error"``. Required, as in the
            Rust/JSON form.
        tolerance : float, optional
            Finite, non-negative absolute residual bound in the formula's result
            units. When supplied, a period passes if the formula result's absolute
            value is at most this bound. None treats the formula as a predicate
            whose nonzero result passes. Validated when the suite executes.

        Raises
        ------
        ValueError
            If ``category`` or ``severity`` is not recognized. Formula compilation,
            node references, and tolerance validation occur when the suite executes.

        """
        ...

    @property
    def id(self) -> str:
        """
        Check identifier.

        This property does not raise.

        Returns
        -------
        str
            Check identifier.
        """
        ...

    @property
    def name(self) -> str:
        """
        Human-readable label.

        This property does not raise.

        Returns
        -------
        str
            Human-readable label.
        """
        ...

    @property
    def formula(self) -> str:
        """
        DSL predicate that must hold.

        This property does not raise.

        Returns
        -------
        str
            DSL predicate that must hold.
        """
        ...

    @property
    def message_template(self) -> str:
        """
        Message emitted when the predicate fails.

        This property does not raise.

        Returns
        -------
        str
            Message emitted when the predicate fails.
        """
        ...

    @property
    def category(self) -> str:
        """
        Classification bucket.

        Returns
        -------
        str
            Classification bucket.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    @property
    def severity(self) -> str:
        """
        Severity assigned to findings from this check.

        Returns
        -------
        str
            Severity assigned to findings from this check.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    @property
    def tolerance(self) -> float | None:
        """
        Absolute residual bound, or ``None`` to interpret the formula as a predicate.

        This property does not raise.

        Returns
        -------
        float | None
            Bound in the formula result's units; None makes any finite nonzero
            result pass. A configured bound makes absolute results at or below it pass.
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
    def from_json(json: str, /) -> FormulaCheckSpec:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        FormulaCheckSpec
            Reconstructed check specification.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import FormulaCheckSpec
        >>> FormulaCheckSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...

class CheckFinding:
    """One breach reported by a check, with its materiality context.

    Examples
    --------
    >>> from finstack_quant.statements import CheckFinding
    >>> payload = '{"check_id":"c1","severity":"error","message":"broken","nodes":[]}'
    >>> CheckFinding.from_json(payload).severity
    'error'

    """

    @property
    def check_id(self) -> str:
        """
        Identifier of the check that produced this finding.

        This property does not raise.

        Returns
        -------
        str
            Identifier of the check that produced this finding.
        """
        ...

    @property
    def severity(self) -> str:
        """
        ``"info"``, ``"warning"`` or ``"error"``.

        Returns
        -------
        str
            ``"info"``, ``"warning"`` or ``"error"``.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    @property
    def message(self) -> str:
        """
        Rendered description of the breach.

        This property does not raise.

        Returns
        -------
        str
            Rendered description of the breach.
        """
        ...

    @property
    def period(self) -> str | None:
        """
        Period id the breach was found in, or ``None`` if model-wide.

        This property does not raise.

        Returns
        -------
        str | None
            Period id the breach was found in, or ``None`` if model-wide.
        """
        ...

    @property
    def nodes(self) -> list[str]:
        """
        Node ids implicated in the breach.

        This property does not raise.

        Returns
        -------
        list[str]
            Node ids implicated in the breach.
        """
        ...

    @property
    def materiality_absolute(self) -> float | None:
        """
        Absolute breach magnitude in the node's units, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Absolute breach magnitude in the node's units, or ``None``.
        """
        ...

    @property
    def materiality_relative_pct(self) -> float | None:
        """
        Breach magnitude as a percentage of the reference value, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Breach magnitude as a percentage of the reference value, or ``None``.
        """
        ...

    @property
    def materiality_reference_value(self) -> float | None:
        """
        Value the relative materiality is measured against, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Value the relative materiality is measured against, or ``None``.
        """
        ...

    @property
    def materiality_reference_label(self) -> str | None:
        """
        Label describing the reference value, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Label describing the reference value, or ``None``.
        """
        ...

    @property
    def comparison(self) -> dict[str, Any] | None:
        """
        The failed numeric comparison behind this finding, or ``None``.

        Returns
        -------
        dict[str, Any] | None
            ``{"identity", "period", "actual", "expected", "tolerance"}``:
            ``identity`` names the tested identity (e.g.
            ``"balance_sheet_articulation"``), ``period`` is the period id
            (omitted when not period-specific), ``actual`` and ``expected``
            are the two compared values and ``tolerance`` is the absolute
            tolerance applied to ``|actual - expected|`` — all three in the
            compared nodes' own units (currency amounts for monetary nodes).
            ``None`` for findings that do not compare two numbers (skipped
            periods, missing or non-finite inputs, sign-convention warnings,
            formula checks without a tolerance) and for findings serialized
            before the comparison was recorded.

        Raises
        ------
        ValueError
            If the comparison cannot be converted to Python objects.
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
    def from_json(json: str, /) -> CheckFinding:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        CheckFinding
            Reconstructed finding.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import CheckFinding
        >>> CheckFinding.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...
