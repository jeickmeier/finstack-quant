"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/checks.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.statements import CheckReport

class CreditMapping:
    """
    Node-id mapping for the credit-underwriting check suite.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    debt_node : str
        Total-debt node.
    ebitda_node : str
        EBITDA node.
    interest_expense_node : str
        Interest-expense node.
    fcf_node : str | None
        Free-cash-flow node (enables the FCF sign check).
    cash_node : str | None
        Cash balance node (enables the liquidity check).
    cash_burn_node : str | None
        Cash-burn node (liquidity runway).
    leverage_warn : tuple[float, float] | None
        ``(min, max)`` debt/EBITDA warning band in turns; leverage outside it
        warns. The error band is fixed at ``(0, 10)``. ``None`` uses the
        Rust default ``(0.0, 6.0)``.
    coverage_min_warn : float | None
        Minimum EBITDA/interest coverage in turns before a warning.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping
    >>> CreditMapping("total_debt", "ebitda", "interest_expense", leverage_warn=(4.0, 6.0)).leverage_warn
    (4.0, 6.0)
    """
    def __init__(
        self,
        debt_node: str,
        ebitda_node: str,
        interest_expense_node: str,
        fcf_node: str | None = None,
        cash_node: str | None = None,
        cash_burn_node: str | None = None,
        leverage_warn: tuple[float, float] | None = None,
        coverage_min_warn: float | None = None,
    ) -> None: ...
    @property
    def debt_node(self) -> str:
        """
        Total-debt node.

        This property does not raise.

        Returns
        -------
        str
            Total-debt node.
        """
    @property
    def ebitda_node(self) -> str:
        """
        Statement node id read as EBITDA by the credit metrics.

        This property does not raise.

        Returns
        -------
        str
            Node id resolved against the evaluated statement model.
        """
    @property
    def interest_expense_node(self) -> str:
        """
        Interest-expense node.

        This property does not raise.

        Returns
        -------
        str
            Interest-expense node.
        """
    @property
    def fcf_node(self) -> str | None:
        """
        Free-cash-flow node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Free-cash-flow node, or ``None``.
        """
    @property
    def cash_node(self) -> str | None:
        """
        Cash balance node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash balance node, or ``None``.
        """
    @property
    def cash_burn_node(self) -> str | None:
        """
        Cash-burn node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash-burn node, or ``None``.
        """
    @property
    def leverage_warn(self) -> tuple[float, float] | None:
        """
        ``(min, max)`` leverage warning band in turns, or ``None``.

        This property does not raise.

        Returns
        -------
        tuple[float, float] | None
            ``(min, max)`` leverage warning band in turns, or ``None``.
        """
    @property
    def coverage_min_warn(self) -> float | None:
        """
        Minimum coverage in turns before a warning, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Minimum coverage in turns before a warning, or ``None``.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CreditMapping:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CreditMapping
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CreditMapping`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditMapping
        >>> CreditMapping.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ThreeStatementMapping:
    """
    Node-id mapping for the three-statement check suite.

    Required ids name the balance-sheet articulation nodes; optional ids switch
    on the reconciliation checks that need them (depreciation, interest, tax,
    cash-flow subtotals, capex, dividends, working capital, debt roll-forwards).

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    cash_node : str
        Cash balance node.
    retained_earnings_node : str
        Retained-earnings balance node.
    net_income_node : str
        Net-income node.
    assets_nodes : list[str]
        Nodes summed to total assets. Required, as in the Rust/JSON form.
    liabilities_nodes : list[str]
        Nodes summed to total liabilities. Required, as in the Rust/JSON form.
    equity_nodes : list[str]
        Nodes summed to total equity. Required, as in the Rust/JSON form.
    ppe_node : str | None
        Net PP&E balance node.
    depreciation_node : str | None
        Depreciation-expense node.
    interest_expense_node : str | None
        Interest-expense node.
    tax_expense_node : str | None
        Tax-expense node.
    pretax_income_node : str | None
        Pre-tax income node.
    cfo_node : str | None
        Cash from operations node.
    cfi_node : str | None
        Cash from investing node.
    cff_node : str | None
        Cash from financing node.
    total_cf_node : str | None
        Net change in cash node.
    capex_node : str | None
        Capital-expenditure node.
    dividends_node : str | None
        Dividends-paid node.
    ppe_additions_node : str | None
        PP&E additions node (capex reconciliation).
    intangible_additions_node : str | None
        Intangible additions node (capex reconciliation).
    dividends_equity_node : str | None
        Dividends recorded in the equity roll-forward.
    debt_balance_nodes : list[tuple[str, str | None]]
        ``(balance_node, rate_node)`` pairs for interest reconciliation.
    cs_interest_node : str | None
        Capital-structure interest node.
    wc_change_cf_node : str | None
        Working-capital change node on the cash-flow statement.
    current_assets_nodes : list[str]
        Current-asset nodes for the working-capital check.
    current_liabilities_nodes : list[str]
        Current-liability nodes for the working-capital check.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ThreeStatementMapping
    >>> m = ThreeStatementMapping("cash", "retained_earnings", "net_income", ["cash"], [], ["retained_earnings"])
    >>> m.cash_node
    'cash'
    """
    def __init__(
        self,
        cash_node: str,
        retained_earnings_node: str,
        net_income_node: str,
        assets_nodes: list[str],
        liabilities_nodes: list[str],
        equity_nodes: list[str],
        ppe_node: str | None = None,
        depreciation_node: str | None = None,
        interest_expense_node: str | None = None,
        tax_expense_node: str | None = None,
        pretax_income_node: str | None = None,
        cfo_node: str | None = None,
        cfi_node: str | None = None,
        cff_node: str | None = None,
        total_cf_node: str | None = None,
        capex_node: str | None = None,
        dividends_node: str | None = None,
        ppe_additions_node: str | None = None,
        intangible_additions_node: str | None = None,
        dividends_equity_node: str | None = None,
        debt_balance_nodes: list[Any] = ...,
        cs_interest_node: str | None = None,
        wc_change_cf_node: str | None = None,
        current_assets_nodes: list[str] = ...,
        current_liabilities_nodes: list[str] = ...,
    ) -> None: ...
    @property
    def cash_node(self) -> str:
        """
        Cash balance node.

        This property does not raise.

        Returns
        -------
        str
            Cash balance node.
        """
    @property
    def retained_earnings_node(self) -> str:
        """
        Retained-earnings balance node.

        This property does not raise.

        Returns
        -------
        str
            Retained-earnings balance node.
        """
    @property
    def net_income_node(self) -> str:
        """
        Net-income node.

        This property does not raise.

        Returns
        -------
        str
            Net-income node.
        """
    @property
    def assets_nodes(self) -> list[str]:
        """
        Nodes summed to total assets.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total assets.
        """
    @property
    def liabilities_nodes(self) -> list[str]:
        """
        Nodes summed to total liabilities.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total liabilities.
        """
    @property
    def equity_nodes(self) -> list[str]:
        """
        Nodes summed to total equity.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total equity.
        """
    @property
    def ppe_node(self) -> str | None:
        """
        Net PP&E node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Net PP&E node, or ``None``.
        """
    @property
    def depreciation_node(self) -> str | None:
        """
        Depreciation-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Depreciation-expense node, or ``None``.
        """
    @property
    def interest_expense_node(self) -> str | None:
        """
        Interest-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Interest-expense node, or ``None``.
        """
    @property
    def tax_expense_node(self) -> str | None:
        """
        Tax-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Tax-expense node, or ``None``.
        """
    @property
    def pretax_income_node(self) -> str | None:
        """
        Pre-tax income node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Pre-tax income node, or ``None``.
        """
    @property
    def cfo_node(self) -> str | None:
        """
        Cash from operations node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from operations node, or ``None``.
        """
    @property
    def cfi_node(self) -> str | None:
        """
        Cash from investing node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from investing node, or ``None``.
        """
    @property
    def cff_node(self) -> str | None:
        """
        Cash from financing node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from financing node, or ``None``.
        """
    @property
    def total_cf_node(self) -> str | None:
        """
        Net change in cash node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Net change in cash node, or ``None``.
        """
    @property
    def capex_node(self) -> str | None:
        """
        Capital-expenditure node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Capital-expenditure node, or ``None``.
        """
    @property
    def dividends_node(self) -> str | None:
        """
        Dividends-paid node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Dividends-paid node, or ``None``.
        """
    @property
    def ppe_additions_node(self) -> str | None:
        """
        PP&E additions node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            PP&E additions node, or ``None``.
        """
    @property
    def intangible_additions_node(self) -> str | None:
        """
        Intangible additions node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Intangible additions node, or ``None``.
        """
    @property
    def dividends_equity_node(self) -> str | None:
        """
        Dividends node in the equity roll-forward, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Dividends node in the equity roll-forward, or ``None``.
        """
    @property
    def debt_balance_nodes(self) -> list[Any]:
        """
        ``(balance_node, rate_node)`` pairs for interest reconciliation.

        This property does not raise.

        Returns
        -------
        list[Any]
            ``(balance_node, rate_node)`` pairs for interest reconciliation.
        """
    @property
    def cs_interest_node(self) -> str | None:
        """
        Capital-structure interest node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Capital-structure interest node, or ``None``.
        """
    @property
    def wc_change_cf_node(self) -> str | None:
        """
        Working-capital change node on the cash-flow statement, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Working-capital change node on the cash-flow statement, or ``None``.
        """
    @property
    def current_assets_nodes(self) -> list[str]:
        """
        Current-asset nodes.

        This property does not raise.

        Returns
        -------
        list[str]
            Current-asset nodes.
        """
    @property
    def current_liabilities_nodes(self) -> list[str]:
        """
        Current-liability nodes.

        This property does not raise.

        Returns
        -------
        list[str]
            Current-liability nodes.
        """
    @property
    def all_nodes(self) -> list[str]:
        """
        Every node id referenced by the mapping.

        This property does not raise.

        Returns
        -------
        list[str]
            Every node id referenced by the mapping.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ThreeStatementMapping:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ThreeStatementMapping
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ThreeStatementMapping`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ThreeStatementMapping
        >>> ThreeStatementMapping.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def render_check_report_html(report: Any) -> str:
    """Render a check report as HTML with inline styles.

    Parameters
    ----------
    report : CheckReport | dict | str
        Typed report, its serde dict, or JSON string.

    Returns
    -------
    str
        HTML-formatted report suitable for Jupyter notebooks.

    Raises
    ------
    ValueError
        If ``report`` is not a valid ``CheckReport`` payload.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_html
    >>> render_check_report_html("{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def render_check_report_text(report: Any) -> str:
    """Render a check report as plain text.

    Parameters
    ----------
    report : CheckReport | dict | str
        Typed report, its serde dict, or JSON string.

    Returns
    -------
    str
        Human-readable plain-text report.

    Raises
    ------
    ValueError
        If ``report`` is not a valid ``CheckReport`` payload.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_text
    >>> render_check_report_text("{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def run_checks(model: Any, spec: Any, results: Any | None = None) -> CheckReport:
    """Run checks from a suite spec against a model.

    Resolves both built-in and formula checks from the spec, evaluates the
    model (unless ``results`` is supplied), and returns a full check report.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    spec : CheckSuiteSpec | dict | str
        Typed ``finstack_quant.statements.CheckSuiteSpec``, its serde dict, or
        JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; when provided the model is not
        re-evaluated. Accounting identity checks require supplied node-unit
        metadata to agree with the model's declarations or explicit monetary values.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If a model, spec, or results payload is malformed, a check configuration
        is invalid, formula parsing or evaluation fails, accounting operands
        have incompatible currencies/units, or supplied result units conflict
        with the model's declarations or explicit monetary values.
    KeyError
        If a check references a node missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import run_checks
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2024Q1..Q2", None)
    >>> _ = b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
    >>> spec = {
    ...     "name": "s",
    ...     "builtin_checks": [],
    ...     "formula_checks": [
    ...         {
    ...             "id": "pos",
    ...             "name": "positive",
    ...             "category": "internal_consistency",
    ...             "severity": "error",
    ...             "formula": "revenue > 0",
    ...             "message_template": "bad {period}",
    ...         }
    ...     ],
    ... }
    >>> run_checks(b.build(), spec).passed
    True
    """

def run_credit_underwriting_checks(model: Any, mapping: Any, results: Any | None = None) -> CheckReport:
    """Run the built-in credit-underwriting check suite.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    mapping : CreditMapping | dict | str
        Typed node mapping, its serde dict, or JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; skips re-evaluation when provided.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If the mapping is malformed or the evaluation fails.
    KeyError
        If ``results`` is omitted and a model formula references a node that
        does not exist. A mapped node that is missing from the model does not
        raise: its check is skipped or reported as a finding.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping, run_credit_underwriting_checks
    >>> mapping = CreditMapping("total_debt", "ebitda", "interest_expense")
    >>> callable(run_credit_underwriting_checks)
    True
    """

def run_three_statement_checks(model: Any, mapping: Any, results: Any | None = None) -> CheckReport:
    """Run the built-in three-statement check suite.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    mapping : ThreeStatementMapping | dict | str
        Typed node mapping, its serde dict, or JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; skips re-evaluation when provided.
        Supplied accounting-node units must agree with the model's declarations
        or explicit monetary values.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If a model, mapping, or results payload is malformed, evaluation fails,
        accounting operands have incompatible currencies/units, or supplied
        result units conflict with model declarations or explicit monetary values.
    KeyError
        If ``results`` is omitted and a model formula references a node that
        does not exist. A mapped node that is missing from the model does not
        raise: its check is skipped or reported as a finding.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ThreeStatementMapping, run_three_statement_checks
    >>> mapping = ThreeStatementMapping(
    ...     "cash", "retained_earnings", "net_income", ["assets"], ["liabilities"], ["equity"]
    ... )
    >>> callable(run_three_statement_checks)
    True
    """
