"""Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF, corporate, Monte Carlo, reports, introspection."""

from __future__ import annotations

from typing import Any

from finstack_quant.statements import FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext

__all__ = [
    "SensitivityConfig",
    "VarianceConfig",
    "ScenarioSet",
    "MonteCarloConfig",
    "SensitivityResult",
    "VarianceRow",
    "VarianceReport",
    "ScenarioResultSet",
    "MonteCarloResults",
    "run_sensitivity",
    "generate_tornado_entries",
    "run_variance",
    "evaluate_scenario_set",
    "run_monte_carlo",
    "backtest_forecast",
    "goal_seek",
    "evaluate_dcf",
    "run_corporate_analysis",
    "pl_summary_report",
    "credit_assessment_report",
    "credit_assessment",
    "DependencyTracer",
    "direct_dependencies",
    "all_dependencies",
    "dependents",
    "explain_formula",
    "explain_formula_text",
    "run_checks",
    "run_three_statement_checks",
    "run_credit_underwriting_checks",
    "render_check_report_text",
    "render_check_report_html",
    "Exposure",
    "classify_stage",
    "compute_ecl",
    "compute_ecl_weighted",
    "percentile_rank",
    "z_score",
    "peer_stats",
    "regression_fair_value",
    "compute_multiple",
    "score_relative_value",
    # Credit scorecard extension
    "ScorecardMetric",
    "ScorecardConfig",
    "ScorecardReport",
    "CreditScorecardExtension",
    "validate_scorecard_config",
    # Corkscrew extension
    "AccountType",
    "CorkscrewAccount",
    "CorkscrewConfig",
    "CorkscrewReport",
    "CorkscrewExtension",
    # Vintage template
    "add_vintage_buildup",
    # Roll-forward template
    "add_roll_forward",
    "add_roll_forward_with_opening",
    # Real-estate template
    "SimpleLeaseSpec",
    "RentStepSpec",
    "FreeRentWindowSpec",
    "RenewalSpec",
    "LeaseGrowthConvention",
    "LeaseSpec",
    "RentRollOutputNodes",
    "ManagementFeeBase",
    "ManagementFeeSpec",
    "PropertyTemplateNodes",
    "add_noi_buildup",
    "add_ncf_buildup",
    "add_rent_roll",
    "add_rent_roll_rental_revenue",
    "add_property_operating_statement",
]

class SensitivityConfig:
    """Configure deterministic sensitivity scenarios for a statement model.

    Parameters
    ----------
    mode : str
        Scenario-construction mode accepted by the Rust sensitivity engine.
    parameters : list[tuple[str, str, float, list[float]]]
        Node-and-period shock specifications, including the base value and
        ordered values to evaluate; defaults to no parameter shocks.
    target_metrics : list[str]
        Output node IDs to collect for every generated scenario; defaults to
        an empty result selection.
    """
    def __init__(
        self,
        mode: str,
        parameters: list[tuple[str, str, float, list[float]]] = ...,
        target_metrics: list[str] = ...,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> SensitivityConfig:
        """Deserialize a sensitivity configuration from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload produced by ``to_json`` or following the
            ``SensitivityConfig`` schema.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def mode(self) -> str: ...
    @property
    def target_metrics(self) -> list[str]: ...
    @property
    def parameter_count(self) -> int: ...

class VarianceConfig:
    """Define the labels, metrics, and periods for a variance comparison.

    Parameters
    ----------
    baseline_label : str
        Reader-facing label for the baseline statement result.
    comparison_label : str
        Reader-facing label for the statement result compared with baseline.
    metrics : list[str]
        Statement node IDs whose absolute and percentage variances are shown.
    periods : list[str]
        Model period labels to include in the variance report, in report order.
    """
    def __init__(
        self,
        baseline_label: str,
        comparison_label: str,
        metrics: list[str],
        periods: list[str],
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> VarianceConfig:
        """Deserialize a variance configuration from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload describing the baseline, comparison, metrics, and
            periods to report.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def baseline_label(self) -> str: ...
    @property
    def comparison_label(self) -> str: ...
    @property
    def metrics(self) -> list[str]: ...
    @property
    def periods(self) -> list[str]: ...

class ScenarioSet:
    """Name statement-model scenarios and optional parent/model relationships.

    Parameters
    ----------
    scenarios : dict[str, dict[str, float]]
        Mapping from scenario name to node-ID overrides expressed as numeric
        model values.
    parents : dict[str, str] or None
        Optional mapping from scenario to inherited parent scenario; omitted
        scenarios have no parent.
    model_ids : dict[str, str] or None
        Optional mapping from scenario name to the model ID it targets.
    """
    def __init__(
        self,
        scenarios: dict[str, dict[str, float]],
        parents: dict[str, str] | None = ...,
        model_ids: dict[str, str] | None = ...,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> ScenarioSet:
        """Deserialize a named scenario set from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing scenario overrides and optional hierarchy.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def names(self) -> list[str]: ...

class MonteCarloConfig:
    """Set reproducible Monte Carlo sampling and retained-output options.

    Parameters
    ----------
    n_paths : int
        Number of stochastic paths to simulate; larger values improve sampling
        precision at greater runtime and memory cost.
    seed : int
        Deterministic random-number seed used to reproduce the simulation.
    percentiles : list[float] or None
        Requested percentile levels as decimal probabilities, such as ``0.95``;
        ``None`` uses the engine defaults.
    include_path_data : bool
        Whether to retain individual path data in addition to summary outputs.
    """
    def __init__(
        self,
        n_paths: int,
        seed: int,
        percentiles: list[float] | None = ...,
        include_path_data: bool = ...,
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> MonteCarloConfig:
        """Deserialize Monte Carlo sampling settings from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload describing paths, seed, percentile levels, and output
            retention.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def n_paths(self) -> int: ...
    @property
    def seed(self) -> int: ...
    @property
    def percentiles(self) -> list[float]: ...
    @property
    def include_path_data(self) -> bool: ...

class SensitivityResult:
    @staticmethod
    def from_json(json: str) -> SensitivityResult:
        """Deserialize a sensitivity-analysis result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by ``run_sensitivity`` or an equivalent
            serialized Rust result.
        """
        ...
    def to_json(self) -> str: ...
    def __len__(self) -> int: ...
    @property
    def target_metrics(self) -> list[str]: ...
    def get_parameter_value(self, scenario_index: int, parameter: str) -> float | None:
        """Return one shocked parameter value for a generated scenario.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the generated scenario in result order.
        parameter : str
            Parameter identifier configured in the sensitivity specification.
        """
        ...
    def get_value(self, scenario_index: int, node_id: str, period: str) -> float | None:
        """Return one scenario output value when it is available.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the generated scenario in result order.
        node_id : str
            Statement node ID whose simulated value is requested.
        period : str
            Model period label for the requested node value.
        """
        ...

class VarianceRow:
    @property
    def period(self) -> str: ...
    @property
    def metric(self) -> str: ...
    @property
    def baseline(self) -> float: ...
    @property
    def comparison(self) -> float: ...
    @property
    def abs_var(self) -> float: ...
    @property
    def pct_var(self) -> float | None: ...

class VarianceReport:
    @staticmethod
    def from_json(json: str) -> VarianceReport:
        """Deserialize a variance report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by ``run_variance`` or a serialized report.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def baseline_label(self) -> str: ...
    @property
    def comparison_label(self) -> str: ...
    @property
    def rows(self) -> list[VarianceRow]: ...

class ScenarioResultSet:
    @staticmethod
    def from_json(json: str) -> ScenarioResultSet:
        """Deserialize evaluated scenario results from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload mapping scenario names to their statement results.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def names(self) -> list[str]: ...
    def get(self, name: str) -> StatementResult | None:
        """Return the statement result for one named scenario.

        Parameters
        ----------
        name : str
            Scenario name as defined in the input ``ScenarioSet``.
        """
        ...

class MonteCarloResults:
    @staticmethod
    def from_json(json: str) -> MonteCarloResults:
        """Deserialize Monte Carlo output from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by ``run_monte_carlo`` or a serialized
            simulation result.
        """
        ...
    def to_json(self) -> str: ...
    @property
    def n_paths(self) -> int: ...
    @property
    def percentiles(self) -> list[float]: ...
    @property
    def forecast_periods(self) -> list[str]: ...
    def get_percentile_series(self, metric: str, percentile: float) -> dict[str, float] | None:
        """Return the requested metric's values at one percentile level.

        Parameters
        ----------
        metric : str
            Statement metric or node ID stored in the simulation result.
        percentile : float
            Percentile as a decimal probability, such as ``0.95`` for P95.
        """
        ...

def run_sensitivity(
    model: FinancialModelSpec | str,
    config: SensitivityConfig | str,
) -> SensitivityResult:
    """Run sensitivity analysis on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    config : SensitivityConfig or str
        Typed configuration or JSON string.

    Returns
    -------
    SensitivityResult
        Typed sensitivity result.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_sensitivity
    >>> out = run_sensitivity(model, config)  # doctest: +SKIP
    """
    ...

def generate_tornado_entries(
    result: SensitivityResult | str,
    metric_node: str,
    period: str | None = None,
) -> str:
    """Build tornado chart entries from a sensitivity result (JSON in/out).

    Parameters
    ----------
    result : SensitivityResult or str
        Typed sensitivity result or JSON string.
    metric_node : str
        Node ID to extract tornado entries for.
    period : str or None
        Optional period string to pin the tornado to.

    Returns
    -------
    str
        JSON-serialized list of ``TornadoEntry``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import generate_tornado_entries
    >>> entries_json = generate_tornado_entries(res_json, "ebitda", "2025Q4")  # doctest: +SKIP
    """
    ...

def run_variance(
    base: StatementResult | str,
    comparison: StatementResult | str,
    config: VarianceConfig | str,
) -> VarianceReport:
    """Run variance analysis comparing two statement results.

    Parameters
    ----------
    base : StatementResult or str
        Baseline ``StatementResult`` object or JSON string.
    comparison : StatementResult or str
        Comparison ``StatementResult`` object or JSON string.
    config : VarianceConfig or str
        Typed configuration or JSON string.

    Returns
    -------
    VarianceReport
        Typed variance report.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_variance
    >>> report = run_variance(base, comparison, config)  # doctest: +SKIP
    """
    ...

def evaluate_scenario_set(
    model: FinancialModelSpec | str,
    scenario_set: ScenarioSet | str,
) -> ScenarioResultSet:
    """Evaluate every scenario in a scenario set against a model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    scenario_set : ScenarioSet or str
        Typed scenario set or JSON string.

    Returns
    -------
    ScenarioResultSet
        Typed mapping from scenario names to statement results.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_scenario_set
    >>> results = evaluate_scenario_set(model, scenario_set)  # doctest: +SKIP
    """
    ...

def run_monte_carlo(
    model: FinancialModelSpec | str,
    config: MonteCarloConfig | str,
) -> MonteCarloResults:
    """Run Monte Carlo simulation on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    config : MonteCarloConfig or str
        Typed configuration or JSON string.

    Returns
    -------
    MonteCarloResults
        Typed Monte Carlo results.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_monte_carlo
    >>> results = run_monte_carlo(model, config)  # doctest: +SKIP
    """
    ...

def backtest_forecast(actual: list[float], forecast: list[float]) -> dict[str, float | int]:
    """Compute forecast accuracy metrics (MAE, MAPE, RMSE).

    Parameters
    ----------
    actual : list[float]
        Observed values.
    forecast : list[float]
        Predicted values (same length as ``actual``).

    Returns
    -------
    dict[str, float | int]
        Dict with keys ``mae``, ``mape``, ``rmse``, and ``n``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import backtest_forecast
    >>> backtest_forecast([1.0, 2.0], [1.1, 1.9])["mae"]
    0.1
    """
    ...

def goal_seek(
    model: FinancialModelSpec | str,
    target_node: str,
    target_period: str,
    target_value: float,
    driver_node: str,
    driver_period: str,
    update_model: bool = True,
    bounds: tuple[float, float] | None = None,
) -> tuple[float, str | None]:
    """Find the driver value that makes a target node hit a target value.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    target_node : str
        Node optimized toward ``target_value``.
    target_period : str
        Period string for the target (e.g. ``"2025Q4"``).
    target_value : float
        Desired value for the target node.
    driver_node : str
        Node adjusted to reach the target.
    driver_period : str
        Period string for the driver.
    update_model : bool
        If ``True``, write the solved value back into the returned model JSON.
    bounds : tuple[float, float] or None
        Optional ``(lo, hi)`` search bounds for bisection.

    Returns
    -------
    tuple[float, str | None]
        ``(solved_driver_value, updated_model_json)``. The updated model JSON
        is ``None`` when ``update_model`` is ``False``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import goal_seek
    >>> solved, new_model = goal_seek(mj, "ni", "2025", 10.0, "rev", "2025")  # doctest: +SKIP
    """
    ...

def evaluate_dcf(
    model: FinancialModelSpec | str,
    wacc: float,
    terminal_value_json: str,
    ufcf_node: str = "ufcf",
    net_debt_override: float | None = None,
    mid_year_convention: bool = False,
    shares_outstanding: float | None = None,
    equity_bridge_json: str | None = None,
    valuation_discounts_json: str | None = None,
    market: MarketContext | str | None = None,
) -> dict[str, float | str]:
    """Evaluate DCF valuation on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string (metadata must include ``currency``).
    wacc : float
        Weighted average cost of capital as a decimal (``0.10`` = 10%).
    terminal_value_json : str
        JSON ``TerminalValueSpec`` (tagged enum).
    ufcf_node : str
        Node ID for unlevered free cash flow.
    net_debt_override : float or None
        Optional flat net debt.
    mid_year_convention : bool
        Use mid-year discounting when ``True``.
    shares_outstanding : float or None
        Optional basic shares for per-share equity value.
    equity_bridge_json : str or None
        Optional JSON ``EquityBridge``.
    valuation_discounts_json : str or None
        Optional JSON ``ValuationDiscounts`` (DLOM, DLOC).
    market : MarketContext or str or None
        Optional ``MarketContext`` object or JSON string for curve-based discounting.

    Returns
    -------
    dict[str, float | str]
        Dict with ``equity_value``, ``equity_currency``, ``enterprise_value``, ``net_debt``,
        ``terminal_value_pv``, ``equity_value_per_share``, ``diluted_shares``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_dcf
    >>> dcf = evaluate_dcf(mj, 0.09, tv_json)  # doctest: +SKIP
    """
    ...

def run_corporate_analysis(
    model: FinancialModelSpec | str,
    wacc: float | None = None,
    terminal_value_json: str | None = None,
    net_debt_override: float | None = None,
    coverage_node: str = "ebitda",
    market: MarketContext | str | None = None,
    as_of: str | None = None,
) -> dict[str, Any]:
    """Run statements plus optional DCF equity and credit context.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    wacc : float or None
        If set, enables DCF at this discount rate (decimal).
    terminal_value_json : str or None
        Required JSON ``TerminalValueSpec`` when ``wacc`` is set.
    net_debt_override : float or None
        Optional flat net debt for the equity bridge.
    coverage_node : str
        Node for DSCR / interest coverage (default ``ebitda``).
    market : MarketContext or str or None
        Optional ``MarketContext`` object or JSON string.
    as_of : str or None
        Optional ISO 8601 valuation date string.

    Returns
    -------
    dict[str, Any]
        Dict with ``statement_json``, optional ``equity`` scalars, and ``credit`` (instrument_id → metrics JSON).
        The credit metrics include ``skipped_periods`` for periods dropped from min/max stats.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_corporate_analysis
    >>> out = run_corporate_analysis(model_json, wacc=0.1, terminal_value_json=tv_json)  # doctest: +SKIP
    """
    ...

def pl_summary_report(
    results: StatementResult | str,
    line_items: list[str],
    periods: list[str],
) -> str:
    """Render a P&L summary report as formatted text.

    Parameters
    ----------
    results : StatementResult or str
        ``StatementResult`` object or JSON string.
    line_items : list[str]
        Node IDs to include as rows.
    periods : list[str]
        Period strings for columns (e.g. ``["2025Q1", "2025Q2"]``).

    Returns
    -------
    str
        Formatted report text.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import pl_summary_report
    >>> text = pl_summary_report(res_json, ["rev", "cogs"], ["2025Q1"])  # doctest: +SKIP
    """
    ...

def credit_assessment_report(results: StatementResult | str, as_of: str) -> str:
    """Render a credit assessment report as formatted text.

    Parameters
    ----------
    results : StatementResult or str
        ``StatementResult`` object or JSON string.
    as_of : str
        Period string for the as-of date (e.g. ``"2025Q1"``).

    Returns
    -------
    str
        Formatted credit report text.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import credit_assessment_report
    >>> report = credit_assessment_report(res_json, "2025Q1")  # doctest: +SKIP
    """
    ...

def credit_assessment(results: StatementResult | str, as_of: str) -> dict[str, Any]:
    """Compute a structured credit assessment (leverage, coverage, FCF).

    Parameters
    ----------
    results : StatementResult or str
        ``StatementResult`` object or JSON string.
    as_of : str
        Period string for the as-of date (e.g. ``"2025Q4"``).

    Returns
    -------
    dict[str, Any]
        Dict with ``as_of`` (str), ``leverage_ratio``, ``interest_coverage``,
        ``free_cash_flow`` (float | None), and ``series`` (list of per-period
        dicts with the same metric keys plus ``period``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import credit_assessment
    >>> out = credit_assessment(res_json, "2025Q4")  # doctest: +SKIP
    """
    ...

class DependencyTracer:
    """Reusable dependency tracer for a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DependencyTracer
    >>> tree = DependencyTracer(model_json).dependency_tree("ebitda")  # doctest: +SKIP
    """

    def __init__(self, model: FinancialModelSpec | str) -> None:
        """Create a dependency tracer for the given model.

        Parameters
        ----------
        model : FinancialModelSpec or str
            ``FinancialModelSpec`` object or JSON string.
        """
        ...
    def dependency_tree(self, node_id: str) -> str:
        """Return an ASCII dependency tree for ``node_id``.

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        str
        """
        ...

    def dependency_tree_detailed(self, results: StatementResult | str, node_id: str, period: str) -> str:
        """Return an ASCII dependency tree annotated with values for one period.

        Parameters
        ----------
        results : StatementResult or str
            Statement results to annotate with.
        node_id : str
            Root node to trace.
        period : str
            Period to annotate.

        Returns
        -------
        str
        """
        ...

    def direct_dependencies(self, node_id: str) -> list[str]:
        """List immediate dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose directly referenced inputs are requested.

        Returns
        -------
        list[str]
        """
        ...
    def all_dependencies(self, node_id: str) -> list[str]:
        """List all transitive dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose complete upstream dependency set is requested.

        Returns
        -------
        list[str]
        """
        ...
    def dependents(self, node_id: str) -> list[str]:
        """List nodes that depend on ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose downstream dependents are requested.

        Returns
        -------
        list[str]
        """
        ...

def direct_dependencies(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List immediate dependencies of a node.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    node_id : str
        Node whose direct dependencies are listed.

    Returns
    -------
    list[str]
        Direct dependency node IDs.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import direct_dependencies
    >>> deps = direct_dependencies(model_json, "ebitda")  # doctest: +SKIP
    """
    ...

def all_dependencies(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List all transitive dependencies of a node in dependency order.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    node_id : str
        Root node for the dependency walk.

    Returns
    -------
    list[str]
        Transitive dependency node IDs.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import all_dependencies
    >>> chain = all_dependencies(model_json, "ni")  # doctest: +SKIP
    """
    ...

def dependents(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List nodes that depend on the given node (reverse dependencies).

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    node_id : str
        Node whose dependents are listed.

    Returns
    -------
    list[str]
        Dependent node IDs.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import dependents
    >>> rev_deps = dependents(model_json, "rev")  # doctest: +SKIP
    """
    ...

def explain_formula(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    node_id: str,
    period: str,
) -> dict[str, Any]:
    """Structured formula explanation for a node and period.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    results : StatementResult or str
        ``StatementResult`` object or JSON string.
    node_id : str
        Node to explain.
    period : str
        Period string.

    Returns
    -------
    dict[str, Any]
        Dict with ``node_id``, ``period_id``, ``final_value``, ``node_type``, ``formula_text``,
        and ``breakdown`` (list of component dicts: ``component``, ``value``, ``operation``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import explain_formula
    >>> detail = explain_formula(mj, rj, "rev", "2025Q1")  # doctest: +SKIP
    """
    ...

def explain_formula_text(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    node_id: str,
    period: str,
) -> str:
    """Human-readable multi-line formula explanation.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    results : StatementResult or str
        ``StatementResult`` object or JSON string.
    node_id : str
        Node to explain.
    period : str
        Period string.

    Returns
    -------
    str
        Detailed text explanation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import explain_formula_text
    >>> text = explain_formula_text(mj, rj, "rev", "2025Q1")  # doctest: +SKIP
    """
    ...

def run_checks(
    model: FinancialModelSpec | str,
    suite_spec_json: str,
    results: StatementResult | str | None = None,
) -> str:
    """Run checks from a suite spec against a model (JSON in/out).

    Resolves both built-in and formula checks, evaluates the model,
    and returns a full check report.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    suite_spec_json : str
        JSON-serialized ``CheckSuiteSpec``.
    results : StatementResult or str or None
        Optional pre-computed ``StatementResult`` (object or JSON);
        skips re-evaluation when provided.

    Returns
    -------
    str
        JSON-serialized ``CheckReport``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_checks
    >>> report_json = run_checks(model_json, suite_spec_json)  # doctest: +SKIP
    """
    ...

def run_three_statement_checks(
    model: FinancialModelSpec | str,
    mapping_json: str,
    results: StatementResult | str | None = None,
) -> str:
    """Run three-statement checks using a node mapping (JSON in/out).

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    mapping_json : str
        JSON-serialized ``ThreeStatementMapping``.
    results : StatementResult or str or None
        Optional pre-computed ``StatementResult`` (object or JSON);
        skips re-evaluation when provided.

    Returns
    -------
    str
        JSON-serialized ``CheckReport``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_three_statement_checks
    >>> report_json = run_three_statement_checks(model_json, mapping_json)  # doctest: +SKIP
    """
    ...

def run_credit_underwriting_checks(
    model: FinancialModelSpec | str,
    mapping_json: str,
    results: StatementResult | str | None = None,
) -> str:
    """Run credit underwriting checks using a node mapping (JSON in/out).

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.
    mapping_json : str
        JSON-serialized ``CreditMapping``.
    results : StatementResult or str or None
        Optional pre-computed ``StatementResult`` (object or JSON);
        skips re-evaluation when provided.

    Returns
    -------
    str
        JSON-serialized ``CheckReport``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_credit_underwriting_checks
    >>> report_json = run_credit_underwriting_checks(model_json, mapping_json)  # doctest: +SKIP
    """
    ...

def render_check_report_text(report_json: str) -> str:
    """Render a check report as plain text.

    Parameters
    ----------
    report_json : str
        JSON-serialized ``CheckReport``.

    Returns
    -------
    str
        Human-readable plain-text report.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_text
    >>> text = render_check_report_text(report_json)  # doctest: +SKIP
    """
    ...

def render_check_report_html(report_json: str) -> str:
    """Render a check report as HTML with inline styles.

    Parameters
    ----------
    report_json : str
        JSON-serialized ``CheckReport``.

    Returns
    -------
    str
        HTML-formatted report suitable for Jupyter notebooks.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_html
    >>> html = render_check_report_html(report_json)  # doctest: +SKIP
    """
    ...

class Exposure:
    """A single credit exposure for ECL / IFRS 9 / CECL computation.

    All monetary fields are in the exposure's base currency; all rates and
    probabilities are expressed as decimals (``0.05`` = 5%).

    Parameters
    ----------
    id : str
        Exposure identifier.
    ead : float
        Exposure at default.
    lgd : float
        Loss given default (decimal).
    eir : float
        Effective interest rate (decimal).
    remaining_maturity : float
        Remaining maturity in years.
    current_pd : float
        Current probability of default (decimal).
    origination_pd : float
        Probability of default at origination (decimal).
    dpd : int or None
        Days past due (optional).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure
    >>> exp = Exposure("loan_1", 1_000_000.0, 0.4, 0.05, 3.0, 0.02, 0.01)  # doctest: +SKIP
    """

    id: str
    ead: float
    lgd: float
    eir: float
    remaining_maturity: float
    current_pd: float
    origination_pd: float
    dpd: int

    def __init__(
        self,
        id: str,
        ead: float,
        lgd: float,
        eir: float,
        remaining_maturity: float,
        current_pd: float,
        origination_pd: float,
        dpd: int | None = None,
    ) -> None: ...

def classify_stage(
    exposure: Exposure,
    pd_delta_stage2: float | None = None,
    dpd_30_trigger: bool | None = None,
    dpd_90_trigger: bool | None = None,
) -> tuple[str, str]:
    """Classify an exposure into an IFRS 9 stage.

    Parameters
    ----------
    exposure : Exposure
        Credit exposure.
    pd_delta_stage2 : float or None
        Absolute PD increase threshold (decimal) for SICR.
    dpd_30_trigger : bool or None
        Apply the 30-DPD Stage 2 rebuttable backstop.
    dpd_90_trigger : bool or None
        Apply the 90-DPD Stage 3 non-rebuttable backstop.

    Returns
    -------
    tuple[str, str]
        ``(stage, trigger_reason)`` where stage is ``"Stage 1"``, ``"Stage 2"``,
        or ``"Stage 3"``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, classify_stage
    >>> exp = Exposure("loan_1", 1e6, 0.4, 0.05, 3.0, 0.02, 0.01)  # doctest: +SKIP
    >>> classify_stage(exp)  # doctest: +SKIP
    ('Stage 1', 'no_trigger')
    """
    ...

def compute_ecl(
    ead: float,
    pd_schedule: list[tuple[float, float]],
    lgd: float,
    eir: float,
    max_horizon_years: float,
    bucket_width_years: float | None = None,
    stage: str = "stage1",
    ead_schedule: list[tuple[float, float]] | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> float:
    """Compute single-scenario ECL for one exposure.

    Parameters
    ----------
    ead : float
        Exposure at default.
    pd_schedule : list[tuple[float, float]]
        ``[(time_years, cumulative_pd), ...]`` knots. A
        ``(0.0, 0.0)`` knot is inserted automatically if not present.
    lgd : float
        Loss given default (decimal).
    eir : float
        Effective interest rate (decimal).
    max_horizon_years : float
        Remaining maturity cap.
    bucket_width_years : float or None
        Time-bucket width (default ``0.25`` for quarterly).
    stage : str
        ``"stage1"``, ``"stage2"``, or ``"stage3"``.
    ead_schedule : list[tuple[float, float]] or None
        Optional EAD amortization profile as
        ``[(time_years, ead), ...]`` knots.
    stage3_time_to_recovery_years : float or None
        Stage 3 discounting horizon to expected recovery, in years.

    Returns
    -------
    float
        ECL amount in the exposure's base currency.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import compute_ecl
    >>> ecl = compute_ecl(1e6, [(1.0, 0.02), (3.0, 0.05)], 0.4, 0.05, 3.0)  # doctest: +SKIP
    """
    ...

def compute_ecl_weighted(
    ead: float,
    scenarios: list[tuple[float, list[tuple[float, float]]]],
    lgd: float,
    eir: float,
    max_horizon: float,
    stage: str = "stage1",
    ead_schedule: list[tuple[float, float]] | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> float:
    """Compute probability-weighted ECL across macro scenarios.

    Parameters
    ----------
    ead : float
        Exposure at default.
    scenarios : list[tuple[float, list[tuple[float, float]]]]
        List of ``(weight, pd_schedule)``. Weights must sum to 1.0.
        A ``(0.0, 0.0)`` knot is inserted automatically into each schedule
        if not present (same convention as ``compute_ecl``).
    lgd : float
        Loss given default (decimal).
    eir : float
        Effective interest rate (decimal).
    max_horizon : float
        Remaining maturity cap (years).
    stage : str
        ``"stage1"``, ``"stage2"``, or ``"stage3"``.
    ead_schedule : list[tuple[float, float]] or None
        Optional EAD amortization profile as
        ``[(time_years, ead), ...]`` knots.
    stage3_time_to_recovery_years : float or None
        Stage 3 discounting horizon to expected recovery, in years.

    Returns
    -------
    float
        Probability-weighted ECL amount.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import compute_ecl_weighted
    >>> ecl = compute_ecl_weighted(1e6, [(0.5, [(1.0, 0.01)]), (0.5, [(1.0, 0.03)])], 0.4, 0.05, 1.0)  # doctest: +SKIP
    """
    ...

# ---------------------------------------------------------------------------
# Comparable-company analysis
# ---------------------------------------------------------------------------

def percentile_rank(value: float, peer_values: list[float]) -> float | None:
    """Percentile rank of ``value`` within ``peer_values`` on a 0-1 scale.

    Parameters
    ----------
    value : float
        Value to rank.
    peer_values : list[float]
        Peer distribution.

    Returns
    -------
    float or None
        Percentile rank in ``[0, 1]``, or ``None`` for empty peers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import percentile_rank
    >>> percentile_rank(3.0, [1.0, 2.0, 3.0, 4.0, 5.0])
    0.5
    """
    ...

def z_score(value: float, peer_values: list[float]) -> float | None:
    """Standard score of ``value`` within the peer distribution.

    Parameters
    ----------
    value : float
        Value to score.
    peer_values : list[float]
        Peer distribution.

    Returns
    -------
    float or None
        Z-score, or ``None`` for empty peers or zero variance.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import z_score
    >>> round(z_score(3.0, [1.0, 2.0, 3.0, 4.0, 5.0]), 10)
    0.0
    """
    ...

def peer_stats(peer_values: list[float]) -> dict[str, float]:
    """Descriptive statistics for a peer distribution.

    Returns a dict with keys ``mean``, ``median``, ``q1``, ``q3``, ``iqr``,
    ``std_dev``, ``min``, ``max``, ``count`` (mirroring the Rust ``PeerStats``
    field names), or an empty dict when ``peer_values`` is empty.

    Parameters
    ----------
    peer_values : list[float]
        Peer distribution.

    Returns
    -------
    dict[str, float]
        Descriptive statistics.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import peer_stats
    >>> stats = peer_stats([1.0, 2.0, 3.0, 4.0, 5.0])
    >>> stats["mean"]
    3.0
    """
    ...

def regression_fair_value(
    x_values: list[float],
    y_values: list[float],
    subject_x: float,
    subject_y: float,
) -> dict[str, float]:
    """Single-factor OLS regression fair value with canonical residual semantics.

    Parameters
    ----------
    x_values : list[float]
        Independent variable values for the peer set.
    y_values : list[float]
        Dependent variable values for the peer set.
    subject_x : float
        Independent variable value for the subject company.
    subject_y : float
        Observed dependent variable value for the subject company.

    Returns
    -------
    dict[str, float]
        Regression fair value metrics.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import regression_fair_value
    >>> result = regression_fair_value([1.0, 2.0, 3.0], [2.0, 4.0, 6.0], 2.5, 5.0)  # doctest: +SKIP
    """
    ...

def compute_multiple(
    company_metrics: dict[str, float],
    multiple: str,
) -> float | None:
    """Canonical multiple computation for one company.

    Parameters
    ----------
    company_metrics : dict[str, float]
        Metric values for the company.
    multiple : str
        Multiple identifier (e.g. ``"ev_ebitda"``).

    Returns
    -------
    float or None
        Computed multiple, or ``None`` when inputs are missing.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import compute_multiple
    >>> compute_multiple({"ev": 100.0, "ebitda": 20.0}, "ev_ebitda")  # doctest: +SKIP
    5.0
    """
    ...

def score_relative_value(
    subject_metrics: dict[str, float | None],
    peer_metrics: list[dict[str, float | None]],
    dimensions: list[tuple[str, float] | dict[str, Any]],
) -> dict[str, Any]:
    """Composite relative-value score across weighted univariate or regression dimensions.

    Dimensions are ``(metric_name, weight)`` tuples or dicts with keys
    ``label``, ``y``, optional ``x`` (one selector or a list), optional
    ``direction`` (``"higher_is_cheap"`` (default) or ``"higher_is_rich"``),
    and ``weight``. Metric selectors are metric names or
    ``"multiple:<id>"`` (e.g. ``"multiple:ev_ebitda"``) for canonical
    valuation multiples. Positive composite = cheap, negative = rich. The
    returned dict uses the canonical Rust/WASM shape:
    ``company_id``, ``composite_score``, ``dimensions``, ``confidence``, and
    ``peer_count``.

    Parameters
    ----------
    subject_metrics : dict[str, float | None]
        Metric values for the subject company.
    peer_metrics : list[dict[str, float | None]]
        Metric values for each peer company.
    dimensions : list[tuple[str, float] | dict[str, Any]]
        Dimension specifications (metric name + weight or dict spec).

    Returns
    -------
    dict[str, Any]
        Composite relative-value score.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import score_relative_value
    >>> result = score_relative_value(
    ...     {"ev_ebitda": 8.0}, [{"ev_ebitda": 6.0}, {"ev_ebitda": 10.0}], [("ev_ebitda", 1.0)]
    ... )  # doctest: +SKIP
    """
    ...

# ---------------------------------------------------------------------------
# Credit scorecard extension
# ---------------------------------------------------------------------------

class ScorecardMetric:
    """Define one weighted metric in a credit-rating scorecard.

    Parameters
    ----------
    name : str
        Stable metric label used in scorecard reports and validation errors.
    formula : str
        Statement-model formula or node expression used to calculate the metric.
    weight : float
        Non-negative contribution weight for the composite rating; defaults to
        ``1.0`` before normalization across usable metrics.
    thresholds_json : str
        JSON mapping that defines rating thresholds for the calculated metric;
        defaults to an empty mapping.
    description : str or None
        Optional reader-facing explanation of the metric and its credit meaning.
    """

    def __init__(
        self,
        name: str,
        formula: str,
        weight: float = 1.0,
        thresholds_json: str = "{}",
        description: str | None = None,
    ) -> None: ...
    @property
    def name(self) -> str:
        """Value of ``name``.

        Returns
        -------
        str
        """
        ...
    @property
    def formula(self) -> str:
        """Value of ``formula``.

        Returns
        -------
        str
        """
        ...
    @property
    def weight(self) -> float:
        """Value of ``weight``.

        Returns
        -------
        float
        """
        ...
    @property
    def description(self) -> str | None:
        """Value of ``description``.

        Returns
        -------
        str or None
        """
        ...
    def thresholds_json(self) -> str:
        """Value of ``thresholds_json``.

        Returns
        -------
        str
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardMetric:
        """Deserialize one scorecard metric from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing metric formula, weight, and thresholds.
        """
        ...

class ScorecardConfig:
    """Configuration for credit scorecard analysis.

    ``period`` optionally pins the rated period (e.g. ``"2025Q4"``); when
    ``None`` the scorecard rates the last actual period in the model if any
    exists, otherwise the last model period.

    Parameters
    ----------
    rating_scale : str
        Rating-scale identifier used to interpret metric thresholds; defaults
        to the ``"S&P"`` scale.
    metrics : list[ScorecardMetric]
        Weighted metric definitions used to calculate the composite rating.
    min_rating : str or None
        Optional minimum acceptable rating used by downstream validation.
    period : str or None
        Optional model period to rate; ``None`` chooses the latest available
        actual or model period.
    """

    def __init__(
        self,
        rating_scale: str = "S&P",
        metrics: list[ScorecardMetric] = ...,
        min_rating: str | None = None,
        period: str | None = None,
    ) -> None: ...
    @property
    def rating_scale(self) -> str:
        """Value of ``rating_scale``.

        Returns
        -------
        str
        """
        ...
    @property
    def min_rating(self) -> str | None:
        """Value of ``min_rating``.

        Returns
        -------
        str or None
        """
        ...
    @property
    def period(self) -> str | None:
        """Value of ``period``.

        Returns
        -------
        str or None
        """
        ...
    @property
    def metrics(self) -> list[ScorecardMetric]:
        """Value of ``metrics``.

        Returns
        -------
        list[ScorecardMetric]
        """
        ...
    def validate(self) -> None:
        """Value of ``validate``.

        Returns
        -------
        None
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardConfig:
        """Deserialize a credit-scorecard configuration from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing scale, metrics, period selection, and
            optional minimum-rating policy.
        """
        ...

class ScorecardReport:
    """Report produced by ``CreditScorecardExtension.execute``.

    ``data_json()`` includes the rated ``period``, the ``partial`` flag, and
    ``weight_coverage`` alongside the per-metric scores and rating.
    """

    @property
    def status(self) -> str:
        """Value of ``status``.

        Returns
        -------
        str
        """
        ...
    @property
    def message(self) -> str:
        """Value of ``message``.

        Returns
        -------
        str
        """
        ...
    @property
    def warnings(self) -> list[str]:
        """Value of ``warnings``.

        Returns
        -------
        list[str]
        """
        ...
    @property
    def errors(self) -> list[str]:
        """Value of ``errors``.

        Returns
        -------
        list[str]
        """
        ...
    def data_json(self) -> str:
        """Value of ``data_json``.

        Returns
        -------
        str
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardReport:
        """Deserialize a credit-scorecard report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by a scorecard extension execution.
        """
        ...

class CreditScorecardExtension:
    """Credit scorecard extension for rating assignment and stress testing."""

    def __init__(self) -> None:
        """Value of ``__init__``.

        Returns
        -------
        None
        """
        ...
    @staticmethod
    def with_config(config: ScorecardConfig) -> CreditScorecardExtension:
        """Create a scorecard extension with a validated configuration.

        Parameters
        ----------
        config : ScorecardConfig
            Rating scale, weighted metrics, and period-selection policy to use.
        """
        ...
    def set_config(self, config: ScorecardConfig) -> None:
        """Replace the extension's scorecard configuration.

        Parameters
        ----------
        config : ScorecardConfig
            New rating scale, metric set, and period-selection policy to apply.
        """
        ...
    def config(self) -> ScorecardConfig | None:
        """Value of ``config``.

        Returns
        -------
        ScorecardConfig or None
        """
        ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> ScorecardReport:
        """Calculate a credit scorecard against evaluated statement results.

        Parameters
        ----------
        model : FinancialModelSpec or str
            Model specification object or equivalent JSON used to resolve nodes.
        results : StatementResult or str
            Evaluated statement result object or equivalent JSON to rate.
        """
        ...

def validate_scorecard_config(config: ScorecardConfig) -> None:
    """Validate a scorecard configuration without executing it.

    Parameters
    ----------
    config : ScorecardConfig
        Rating scale, metrics, thresholds, and period policy to validate.
    """
    ...

# ---------------------------------------------------------------------------
# Corkscrew (balance-sheet roll-forward) extension
# ---------------------------------------------------------------------------

class AccountType:
    """Balance-sheet account classifier: asset / liability / equity."""

    Asset: AccountType
    Liability: AccountType
    Equity: AccountType

    @staticmethod
    def from_str(value: str) -> AccountType:
        """Parse a balance-sheet account classification.

        Parameters
        ----------
        value : str
            Case-insensitive ``"asset"``, ``"liability"``, or ``"equity"`` value.
        """
        ...
    def value(self) -> str:
        """Value of ``value``.

        Returns
        -------
        str
        """
        ...

class CorkscrewAccount:
    """Map one balance-sheet account to its corkscrew input nodes.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the period-end account balance.
    account_type : AccountType
        Asset, liability, or equity classification controlling change signs.
    changes : list[str]
        Statement node IDs whose values are added or subtracted each period.
    beginning_balance_node : str or None
        Optional node ID supplying the opening balance instead of an inferred
        first-period balance.
    """

    def __init__(
        self,
        node_id: str,
        account_type: AccountType,
        changes: list[str] = ...,
        beginning_balance_node: str | None = None,
    ) -> None: ...
    @property
    def node_id(self) -> str:
        """Value of ``node_id``.

        Returns
        -------
        str
        """
        ...
    @property
    def account_type(self) -> AccountType:
        """Value of ``account_type``.

        Returns
        -------
        AccountType
        """
        ...
    @property
    def changes(self) -> list[str]:
        """Value of ``changes``.

        Returns
        -------
        list[str]
        """
        ...
    @property
    def beginning_balance_node(self) -> str | None:
        """Value of ``beginning_balance_node``.

        Returns
        -------
        str or None
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewAccount:
        """Deserialize an account mapping from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying the balance node, type, and change nodes.
        """
        ...

class CorkscrewConfig:
    """Configure corkscrew roll-forward validation across balance accounts.

    Parameters
    ----------
    accounts : list[CorkscrewAccount]
        Account mappings to reconcile; defaults to an empty validation set.
    tolerance : float
        Absolute currency-unit tolerance allowed for reconciliation differences;
        defaults to ``0.01``.
    fail_on_error : bool
        Whether reconciliation errors abort execution instead of being reported.
    """

    def __init__(
        self,
        accounts: list[CorkscrewAccount] = ...,
        tolerance: float = 0.01,
        fail_on_error: bool = False,
    ) -> None: ...
    @property
    def accounts(self) -> list[CorkscrewAccount]:
        """Value of ``accounts``.

        Returns
        -------
        list[CorkscrewAccount]
        """
        ...
    @property
    def tolerance(self) -> float:
        """Value of ``tolerance``.

        Returns
        -------
        float
        """
        ...
    @property
    def fail_on_error(self) -> bool:
        """Value of ``fail_on_error``.

        Returns
        -------
        bool
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewConfig:
        """Deserialize corkscrew validation settings from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing account mappings and reconciliation policy.
        """
        ...

class CorkscrewReport:
    """Report produced by ``CorkscrewExtension.execute``."""

    @property
    def status(self) -> str:
        """Value of ``status``.

        Returns
        -------
        str
        """
        ...
    @property
    def message(self) -> str:
        """Value of ``message``.

        Returns
        -------
        str
        """
        ...
    @property
    def warnings(self) -> list[str]:
        """Value of ``warnings``.

        Returns
        -------
        list[str]
        """
        ...
    @property
    def errors(self) -> list[str]:
        """Value of ``errors``.

        Returns
        -------
        list[str]
        """
        ...
    def data_json(self) -> str:
        """Value of ``data_json``.

        Returns
        -------
        str
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewReport:
        """Deserialize a corkscrew validation report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by a corkscrew extension execution.
        """
        ...

class CorkscrewExtension:
    """Corkscrew extension for balance-sheet roll-forward validation."""

    def __init__(self) -> None:
        """Value of ``__init__``.

        Returns
        -------
        None
        """
        ...
    @staticmethod
    def with_config(config: CorkscrewConfig) -> CorkscrewExtension:
        """Create a corkscrew extension with reconciliation settings.

        Parameters
        ----------
        config : CorkscrewConfig
            Accounts, tolerance, and error policy used during reconciliation.
        """
        ...
    def set_config(self, config: CorkscrewConfig) -> None:
        """Replace the extension's reconciliation configuration.

        Parameters
        ----------
        config : CorkscrewConfig
            Accounts, tolerance, and error policy to apply on the next run.
        """
        ...
    def config(self) -> CorkscrewConfig | None:
        """Value of ``config``.

        Returns
        -------
        CorkscrewConfig or None
        """
        ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> CorkscrewReport:
        """Validate account roll-forwards against evaluated statement results.

        Parameters
        ----------
        model : FinancialModelSpec or str
            Model specification object or JSON used to resolve configured nodes.
        results : StatementResult or str
            Evaluated statement results object or JSON to reconcile.
        """
        ...

# ---------------------------------------------------------------------------
# Vintage template
# ---------------------------------------------------------------------------

def add_vintage_buildup(
    model: FinancialModelSpec | str,
    name: str,
    new_volume_node: str,
    decay_curve: list[float],
) -> str:
    """Apply the vintage (cohort) buildup template to a model spec.

    Returns a JSON-serialized ``FinancialModelSpec`` with the convolution
    node added.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with the cohort schedule.
    name : str
        Prefix used to name the generated vintage buildup nodes.
    new_volume_node : str
        Existing node ID that supplies new volume for each cohort period.
    decay_curve : list[float]
        Ordered cohort-retention factors by elapsed period, expressed as decimal
        multipliers of original volume.
    """
    ...

# ---------------------------------------------------------------------------
# Roll-forward template
# ---------------------------------------------------------------------------

def add_roll_forward(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
) -> str:
    """Apply the roll-forward template (Beginning + Increases - Decreases = Ending) to a model spec.

    Returns a JSON-serialized ``FinancialModelSpec`` with ``{name}_beg`` and
    ``{name}_end`` nodes added. The first period opens at zero; use
    ``add_roll_forward_with_opening`` for an explicit opening balance.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.
    """
    ...

def add_roll_forward_with_opening(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
    opening: float,
) -> str:
    """Apply the roll-forward template with an explicit first-period opening balance.

    Same as ``add_roll_forward`` except the first period's beginning balance
    is ``opening`` instead of zero. Returns a JSON-serialized
    ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.
    opening : float
        Beginning balance assigned to the first modeled period in model units.
    """
    ...

# ---------------------------------------------------------------------------
# Real-estate template
# ---------------------------------------------------------------------------

class SimpleLeaseSpec:
    """Describe a simple per-lease rent schedule for a property model.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the lease's rental-revenue series.
    start : str
        First included model period label for the lease term.
    base_rent : float
        Contractual rent per modeled period before growth, concessions, and
        occupancy scaling, in the model's currency units.
    end : str or None
        Optional final included model period; ``None`` extends through the
        model horizon.
    growth_rate : float
        Periodic decimal rent-growth rate, such as ``0.03`` for 3%; defaults
        to zero growth.
    free_rent_periods : int
        Number of initial included periods with rent set to zero; defaults to
        no concession.
    occupancy : float
        Decimal occupancy multiplier applied to scheduled rent; defaults to
        fully occupied ``1.0``.
    """

    def __init__(
        self,
        node_id: str,
        start: str,
        base_rent: float,
        end: str | None = None,
        growth_rate: float = 0.0,
        free_rent_periods: int = 0,
        occupancy: float = 1.0,
    ) -> None: ...
    @property
    def node_id(self) -> str:
        """Value of ``node_id``.

        Returns
        -------
        str
        """
        ...
    @property
    def start(self) -> str:
        """Value of ``start``.

        Returns
        -------
        str
        """
        ...
    @property
    def end(self) -> str | None:
        """Value of ``end``.

        Returns
        -------
        str or None
        """
        ...
    @property
    def base_rent(self) -> float:
        """Value of ``base_rent``.

        Returns
        -------
        float
        """
        ...
    @property
    def growth_rate(self) -> float:
        """Value of ``growth_rate``.

        Returns
        -------
        float
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """Value of ``free_rent_periods``.

        Returns
        -------
        int
        """
        ...
    @property
    def occupancy(self) -> float:
        """Value of ``occupancy``.

        Returns
        -------
        float
        """
        ...
    def validate(self) -> None:
        """Value of ``validate``.

        Returns
        -------
        None
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> SimpleLeaseSpec:
        """Deserialize a simple lease schedule from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing lease term, rent, growth, and occupancy.
        """
        ...

class RentStepSpec:
    """Reset a lease's base rent from one model period onward.

    Parameters
    ----------
    start : str
        First model period label at which the stepped rent applies.
    rent : float
        Replacement periodic rent in the model's currency units.
    """

    def __init__(self, start: str, rent: float) -> None: ...
    @property
    def start(self) -> str:
        """Value of ``start``.

        Returns
        -------
        str
        """
        ...
    @property
    def rent(self) -> float:
        """Value of ``rent``.

        Returns
        -------
        float
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentStepSpec:
        """Deserialize a rent-step specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the step start period and replacement rent.
        """
        ...

class FreeRentWindowSpec:
    """Define a finite concession window that sets lease rent to zero.

    Parameters
    ----------
    start : str
        First model period label affected by the free-rent concession.
    periods : int
        Number of consecutive modeled periods with rent set to zero.
    """

    def __init__(self, start: str, periods: int) -> None: ...
    @property
    def start(self) -> str:
        """Value of ``start``.

        Returns
        -------
        str
        """
        ...
    @property
    def periods(self) -> int:
        """Value of ``periods``.

        Returns
        -------
        int
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> FreeRentWindowSpec:
        """Deserialize a free-rent concession window from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the concession start period and duration.
        """
        ...

class RenewalSpec:
    """Model a lease renewal in expected-value terms after the base term.

    Parameters
    ----------
    term_periods : int
        Number of modeled periods in the renewal term when the tenant renews.
    probability : float
        Decimal probability of renewal from zero through one.
    downtime_periods : int
        Vacancy periods between the original lease and renewal; defaults to zero.
    rent_factor : float
        Multiplier applied to the ending scheduled rent for the renewal term;
        defaults to ``1.0``.
    free_rent_periods : int
        Initial renewal periods with rent set to zero; defaults to no concession.
    """

    def __init__(
        self,
        term_periods: int,
        probability: float,
        downtime_periods: int = 0,
        rent_factor: float = 1.0,
        free_rent_periods: int = 0,
    ) -> None: ...
    @property
    def term_periods(self) -> int:
        """Value of ``term_periods``.

        Returns
        -------
        int
        """
        ...
    @property
    def probability(self) -> float:
        """Value of ``probability``.

        Returns
        -------
        float
        """
        ...
    @property
    def downtime_periods(self) -> int:
        """Value of ``downtime_periods``.

        Returns
        -------
        int
        """
        ...
    @property
    def rent_factor(self) -> float:
        """Value of ``rent_factor``.

        Returns
        -------
        float
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """Value of ``free_rent_periods``.

        Returns
        -------
        int
        """
        ...
    def validate(self) -> None:
        """Value of ``validate``.

        Returns
        -------
        None
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> RenewalSpec:
        """Deserialize renewal assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing renewal term, probability, downtime, and
            rent assumptions.
        """
        ...

class LeaseGrowthConvention:
    """Compounding convention for lease rent growth."""

    PerPeriod: LeaseGrowthConvention
    AnnualEscalator: LeaseGrowthConvention

    @staticmethod
    def from_str(value: str) -> LeaseGrowthConvention:
        """Parse a lease rent-growth compounding convention.

        Parameters
        ----------
        value : str
            Case-insensitive ``"per_period"`` or ``"annual_escalator"`` value.
        """
        ...
    def value(self) -> str:
        """Value of ``value``.

        Returns
        -------
        str
        """
        ...

class LeaseSpec:
    """Describe a rich lease schedule for rent-roll generation.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the lease's rental-revenue series.
    start : str
        First included model period label for the lease term.
    base_rent : float
        Contractual periodic rent before escalators, concessions, and occupancy
        scaling, in the model's currency units.
    end : str or None
        Optional final included model period; ``None`` extends through horizon.
    growth_rate : float
        Decimal rent-growth rate interpreted by ``growth_convention``.
    growth_convention : LeaseGrowthConvention
        Whether rent growth compounds every model period or as an annual step.
    rent_steps : list[RentStepSpec]
        Explicit rent resets applied from each step's start period onward.
    free_rent_periods : int
        Number of initial included periods with rent set to zero.
    free_rent_windows : list[FreeRentWindowSpec]
        Additional dated rent-free concession windows within the lease term.
    occupancy : float
        Decimal occupancy multiplier applied to scheduled rent.
    renewal : RenewalSpec or None
        Optional expected-value renewal assumptions applied after the base term.
    """

    def __init__(
        self,
        node_id: str,
        start: str,
        base_rent: float,
        end: str | None = None,
        growth_rate: float = 0.0,
        growth_convention: LeaseGrowthConvention = ...,
        rent_steps: list[RentStepSpec] = ...,
        free_rent_periods: int = 0,
        free_rent_windows: list[FreeRentWindowSpec] = ...,
        occupancy: float = 1.0,
        renewal: RenewalSpec | None = None,
    ) -> None: ...
    @property
    def node_id(self) -> str:
        """Value of ``node_id``.

        Returns
        -------
        str
        """
        ...
    @property
    def start(self) -> str:
        """Value of ``start``.

        Returns
        -------
        str
        """
        ...
    @property
    def end(self) -> str | None:
        """Value of ``end``.

        Returns
        -------
        str or None
        """
        ...
    @property
    def base_rent(self) -> float:
        """Value of ``base_rent``.

        Returns
        -------
        float
        """
        ...
    @property
    def growth_rate(self) -> float:
        """Value of ``growth_rate``.

        Returns
        -------
        float
        """
        ...
    @property
    def growth_convention(self) -> LeaseGrowthConvention:
        """Value of ``growth_convention``.

        Returns
        -------
        LeaseGrowthConvention
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """Value of ``free_rent_periods``.

        Returns
        -------
        int
        """
        ...
    @property
    def occupancy(self) -> float:
        """Value of ``occupancy``.

        Returns
        -------
        float
        """
        ...
    @property
    def renewal(self) -> RenewalSpec | None:
        """Value of ``renewal``.

        Returns
        -------
        RenewalSpec or None
        """
        ...
    def validate(self) -> None:
        """Value of ``validate``.

        Returns
        -------
        None
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> LeaseSpec:
        """Deserialize a rich lease schedule from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing term, rent, escalation, concession, and
            renewal assumptions.
        """
        ...

class RentRollOutputNodes:
    """Name the aggregate model nodes produced by a rent-roll template.

    Parameters
    ----------
    rent_pgi_node : str
        Node ID for potential gross rent before concessions and vacancy.
    free_rent_node : str
        Node ID for rent waived through free-rent concessions.
    vacancy_loss_node : str
        Node ID for the revenue reduction caused by vacancy or occupancy.
    rent_effective_node : str
        Node ID for effective rent after concessions and vacancy adjustments.
    """

    def __init__(
        self,
        rent_pgi_node: str = "rent_pgi",
        free_rent_node: str = "free_rent",
        vacancy_loss_node: str = "vacancy_loss",
        rent_effective_node: str = "rent_effective",
    ) -> None: ...
    @property
    def rent_pgi_node(self) -> str:
        """Value of ``rent_pgi_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def free_rent_node(self) -> str:
        """Value of ``free_rent_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def vacancy_loss_node(self) -> str:
        """Value of ``vacancy_loss_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def rent_effective_node(self) -> str:
        """Value of ``rent_effective_node``.

        Returns
        -------
        str
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentRollOutputNodes:
        """Deserialize rent-roll output-node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying potential, concession, vacancy, and
            effective-rent output nodes.
        """
        ...

class ManagementFeeBase:
    """Basis for management fee calculation."""

    Egi: ManagementFeeBase
    EffectiveRent: ManagementFeeBase

    @staticmethod
    def from_str(value: str) -> ManagementFeeBase:
        """Parse a management-fee calculation basis.

        Parameters
        ----------
        value : str
            Case-insensitive ``"egi"`` or ``"effective_rent"`` basis value.
        """
        ...
    def value(self) -> str:
        """Value of ``value``.

        Returns
        -------
        str
        """
        ...

class ManagementFeeSpec:
    """Set a percentage management fee and the revenue base it applies to.

    Parameters
    ----------
    rate : float
        Decimal fee rate, such as ``0.03`` for a 3% management fee.
    base : ManagementFeeBase
        Effective-rent or EGI base used to calculate the fee; defaults to the
        binding's standard basis.
    """

    def __init__(self, rate: float, base: ManagementFeeBase = ...) -> None: ...
    @property
    def rate(self) -> float:
        """Value of ``rate``.

        Returns
        -------
        float
        """
        ...
    @property
    def base(self) -> ManagementFeeBase:
        """Value of ``base``.

        Returns
        -------
        ManagementFeeBase
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> ManagementFeeSpec:
        """Deserialize management-fee assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the decimal rate and revenue basis.
        """
        ...

class PropertyTemplateNodes:
    """Name generated node IDs for a property operating-statement template.

    Parameters
    ----------
    rent_roll : RentRollOutputNodes or None
        Optional rent-roll output names; ``None`` uses the template defaults.
    other_income_total_node : str
        Node ID aggregating other-income components.
    egi_node : str
        Node ID for effective gross income after rent and other income.
    management_fee_node : str
        Node ID for the management-fee expense series.
    opex_total_node : str
        Node ID aggregating operating-expense components.
    noi_node : str
        Node ID for net operating income before capital expenditures.
    capex_total_node : str
        Node ID aggregating capital-expenditure components.
    ncf_node : str
        Node ID for net cash flow after operating items and capital expenditure.
    """

    def __init__(
        self,
        rent_roll: RentRollOutputNodes | None = None,
        other_income_total_node: str = "other_income_total",
        egi_node: str = "egi",
        management_fee_node: str = "management_fee",
        opex_total_node: str = "opex_total",
        noi_node: str = "noi",
        capex_total_node: str = "capex_total",
        ncf_node: str = "ncf",
    ) -> None: ...
    @property
    def rent_roll(self) -> RentRollOutputNodes:
        """Value of ``rent_roll``.

        Returns
        -------
        RentRollOutputNodes
        """
        ...
    @property
    def other_income_total_node(self) -> str:
        """Value of ``other_income_total_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def egi_node(self) -> str:
        """Value of ``egi_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def management_fee_node(self) -> str:
        """Value of ``management_fee_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def opex_total_node(self) -> str:
        """Value of ``opex_total_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def noi_node(self) -> str:
        """Value of ``noi_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def capex_total_node(self) -> str:
        """Value of ``capex_total_node``.

        Returns
        -------
        str
        """
        ...
    @property
    def ncf_node(self) -> str:
        """Value of ``ncf_node``.

        Returns
        -------
        str
        """
        ...
    def to_json(self) -> str:
        """Value of ``to_json``.

        Returns
        -------
        str
        """
        ...
    @staticmethod
    def from_json(json: str) -> PropertyTemplateNodes:
        """Deserialize property-template node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying rent, income, expense, NOI, capex, and NCF
            nodes.
        """
        ...

def add_noi_buildup(
    model: FinancialModelSpec | str,
    total_revenue_node: str,
    revenue_nodes: list[str],
    total_expenses_node: str,
    expense_nodes: list[str],
    noi_node: str,
) -> str:
    """Apply the NOI buildup template and return JSON ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NOI calculations.
    total_revenue_node : str
        Output node ID that sums the selected revenue nodes.
    revenue_nodes : list[str]
        Existing node IDs included as revenue in the NOI calculation.
    total_expenses_node : str
        Output node ID that sums the selected operating-expense nodes.
    expense_nodes : list[str]
        Existing node IDs included as operating expenses in the NOI calculation.
    noi_node : str
        Output node ID for revenue less operating expenses.
    """
    ...

def add_ncf_buildup(
    model: FinancialModelSpec | str,
    noi_node: str,
    capex_nodes: list[str],
    ncf_node: str,
) -> str:
    """Apply the NCF buildup template and return JSON ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NCF calculations.
    noi_node : str
        Existing node ID supplying net operating income before capital spending.
    capex_nodes : list[str]
        Existing node IDs whose values are deducted as capital expenditures.
    ncf_node : str
        Output node ID for net operating income less capital expenditures.
    """
    ...

def add_rent_roll(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    nodes: RentRollOutputNodes | None = None,
) -> str:
    """Apply the rich rent-roll template and return JSON ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with rental-revenue nodes.
    leases : list[LeaseSpec]
        Rich lease schedules to calculate and aggregate into the rent roll.
    nodes : RentRollOutputNodes or None
        Optional aggregate output-node names; ``None`` uses template defaults.
    """
    ...

def add_rent_roll_rental_revenue(
    model: FinancialModelSpec | str,
    leases: list[SimpleLeaseSpec],
    total_rent_node: str,
) -> str:
    """Apply the simple rent-roll template and return JSON ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with rental-revenue nodes.
    leases : list[SimpleLeaseSpec]
        Simple lease schedules to calculate and aggregate into rental revenue.
    total_rent_node : str
        Output node ID that sums all calculated simple-lease rent series.
    """
    ...

def add_property_operating_statement(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    other_income_nodes: list[str] = ...,
    opex_nodes: list[str] = ...,
    capex_nodes: list[str] = ...,
    management_fee: ManagementFeeSpec | None = None,
    nodes: PropertyTemplateNodes | None = None,
) -> str:
    """Apply the full property operating-statement template and return JSON.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with property statements.
    leases : list[LeaseSpec]
        Rich lease schedules used to build rental-revenue and rent-roll outputs.
    other_income_nodes : list[str]
        Existing node IDs aggregated as other income; defaults to an empty list.
    opex_nodes : list[str]
        Existing node IDs aggregated as operating expenses; defaults to empty.
    capex_nodes : list[str]
        Existing node IDs aggregated as capital expenditures; defaults to empty.
    management_fee : ManagementFeeSpec or None
        Optional fee assumptions; ``None`` omits management-fee calculation.
    nodes : PropertyTemplateNodes or None
        Optional generated-node names; ``None`` uses the template defaults.
    """
    ...
