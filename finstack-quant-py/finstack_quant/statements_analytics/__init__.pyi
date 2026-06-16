"""Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF, corporate, Monte Carlo, reports, introspection."""

from __future__ import annotations

from typing import Any

from finstack_quant.statements import FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext

__all__ = [
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

def run_sensitivity(model: FinancialModelSpec | str, config_json: str) -> str:
    """Run sensitivity analysis on a financial model.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        config_json: JSON-serialized ``SensitivityConfig``.

    Returns:
        JSON-serialized ``SensitivityResult``.

    Example:
        >>> from finstack_quant.statements_analytics import run_sensitivity
        >>> out = run_sensitivity(model_json, config_json)
    """
    ...

def generate_tornado_entries(
    result_json: str,
    metric_node: str,
    period: str | None = None,
) -> str:
    """Build tornado chart entries from a sensitivity result (JSON in/out).

    Args:
        result_json: JSON-serialized ``SensitivityResult``.
        metric_node: Node ID to extract tornado entries for.
        period: Optional period string to pin the tornado to.

    Returns:
        JSON-serialized list of ``TornadoEntry``.

    Example:
        >>> from finstack_quant.statements_analytics import generate_tornado_entries
        >>> entries_json = generate_tornado_entries(res_json, "ebitda", "2025Q4")
    """
    ...

def run_variance(
    base: StatementResult | str,
    comparison: StatementResult | str,
    config_json: str,
) -> str:
    """Run variance analysis comparing two statement results.

    Args:
        base: Baseline ``StatementResult`` object or JSON string.
        comparison: Comparison ``StatementResult`` object or JSON string.
        config_json: JSON-serialized ``VarianceConfig``.

    Returns:
        JSON-serialized variance report.

    Example:
        >>> from finstack_quant.statements_analytics import run_variance
        >>> report_json = run_variance(base_json, cmp_json, cfg_json)
    """
    ...

def evaluate_scenario_set(model: FinancialModelSpec | str, scenario_set_json: str) -> str:
    """Evaluate every scenario in a scenario set against a model.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        scenario_set_json: JSON-serialized ``ScenarioSet``.

    Returns:
        JSON object mapping scenario name to ``StatementResult`` JSON.

    Example:
        >>> from finstack_quant.statements_analytics import evaluate_scenario_set
        >>> results_map_json = evaluate_scenario_set(model_json, set_json)
    """
    ...

def run_monte_carlo(model: FinancialModelSpec | str, config_json: str) -> str:
    """Run Monte Carlo simulation on a financial model.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        config_json: JSON-serialized ``MonteCarloConfig`` (``n_paths``, ``seed``, optional ``percentiles`` and ``include_path_data``).

    Returns:
        JSON-serialized ``MonteCarloResults``.

    Example:
        >>> from finstack_quant.statements_analytics import run_monte_carlo
        >>> mc_json = run_monte_carlo(model_json, mc_cfg_json)
    """
    ...

def backtest_forecast(actual: list[float], forecast: list[float]) -> dict[str, float | int]:
    """Compute forecast accuracy metrics (MAE, MAPE, RMSE).

    Args:
        actual: Observed values.
        forecast: Predicted values (same length as ``actual``).

    Returns:
        Dict with keys ``mae``, ``mape``, ``rmse``, and ``n``.

    Example:
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

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        target_node: Node optimized toward ``target_value``.
        target_period: Period string for the target (e.g. ``"2025Q4"``).
        target_value: Desired value for the target node.
        driver_node: Node adjusted to reach the target.
        driver_period: Period string for the driver.
        update_model: If ``True``, write the solved value back into the returned model JSON.
        bounds: Optional ``(lo, hi)`` search bounds for bisection.

    Returns:
        ``(solved_driver_value, updated_model_json)``. The updated model JSON
        is ``None`` when ``update_model`` is ``False``.

    Example:
        >>> from finstack_quant.statements_analytics import goal_seek
        >>> solved, new_model = goal_seek(mj, "ni", "2025", 10.0, "rev", "2025")
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

    Args:
        model: ``FinancialModelSpec`` object or JSON string (metadata must include ``currency``).
        wacc: Weighted average cost of capital as a decimal (``0.10`` = 10%).
        terminal_value_json: JSON ``TerminalValueSpec`` (tagged enum).
        ufcf_node: Node ID for unlevered free cash flow.
        net_debt_override: Optional flat net debt.
        mid_year_convention: Use mid-year discounting when ``True``.
        shares_outstanding: Optional basic shares for per-share equity value.
        equity_bridge_json: Optional JSON ``EquityBridge``.
        valuation_discounts_json: Optional JSON ``ValuationDiscounts`` (DLOM, DLOC).
        market: Optional ``MarketContext`` object or JSON string for curve-based discounting.

    Returns:
        Dict with ``equity_value``, ``equity_currency``, ``enterprise_value``, ``net_debt``,
        ``terminal_value_pv``, ``equity_value_per_share``, ``diluted_shares``.

    Example:
        >>> from finstack_quant.statements_analytics import evaluate_dcf
        >>> dcf = evaluate_dcf(mj, 0.09, tv_json)
        >>> float(dcf["equity_value"])
        0.0
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

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        wacc: If set, enables DCF at this discount rate (decimal).
        terminal_value_json: Required JSON ``TerminalValueSpec`` when ``wacc`` is set.
        net_debt_override: Optional flat net debt for the equity bridge.
        coverage_node: Node for DSCR / interest coverage (default ``ebitda``).
        market: Optional ``MarketContext`` object or JSON string.
        as_of: Optional ISO 8601 valuation date string.

    Returns:
        Dict with ``statement_json``, optional ``equity`` scalars, and ``credit`` (instrument_id → metrics JSON).
        The credit metrics include ``skipped_periods`` for periods dropped from min/max stats.

    Example:
        >>> from finstack_quant.statements_analytics import run_corporate_analysis
        >>> out = run_corporate_analysis(model_json, wacc=0.1, terminal_value_json=tv_json)
    """
    ...

def pl_summary_report(
    results: StatementResult | str,
    line_items: list[str],
    periods: list[str],
) -> str:
    """Render a P&L summary report as formatted text.

    Args:
        results: ``StatementResult`` object or JSON string.
        line_items: Node IDs to include as rows.
        periods: Period strings for columns (e.g. ``["2025Q1", "2025Q2"]``).

    Returns:
        Formatted report text.

    Example:
        >>> from finstack_quant.statements_analytics import pl_summary_report
        >>> text = pl_summary_report(res_json, ["rev", "cogs"], ["2025Q1"])
    """
    ...

def credit_assessment_report(results: StatementResult | str, as_of: str) -> str:
    """Render a credit assessment report as formatted text.

    Args:
        results: ``StatementResult`` object or JSON string.
        as_of: Period string for the as-of date (e.g. ``"2025Q1"``).

    Returns:
        Formatted credit report text.

    Example:
        >>> from finstack_quant.statements_analytics import credit_assessment_report
        >>> report = credit_assessment_report(res_json, "2025Q1")
    """
    ...

class DependencyTracer:
    """Reusable dependency tracer for a financial model.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.

    Example:
        >>> from finstack_quant.statements_analytics import DependencyTracer
        >>> tree = DependencyTracer(model_json).dependency_tree("ebitda")
    """

    def __init__(self, model: FinancialModelSpec | str) -> None: ...
    def dependency_tree(self, node_id: str) -> str:
        """Return an ASCII dependency tree for ``node_id``."""
        ...

    def dependency_tree_detailed(self, results: StatementResult | str, node_id: str, period: str) -> str:
        """Return an ASCII dependency tree annotated with values for one period."""
        ...

    def direct_dependencies(self, node_id: str) -> list[str]: ...
    def all_dependencies(self, node_id: str) -> list[str]: ...
    def dependents(self, node_id: str) -> list[str]: ...

def direct_dependencies(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List immediate dependencies of a node.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        node_id: Node whose direct dependencies are listed.

    Returns:
        Direct dependency node IDs.

    Example:
        >>> from finstack_quant.statements_analytics import direct_dependencies
        >>> deps = direct_dependencies(model_json, "ebitda")
    """
    ...

def all_dependencies(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List all transitive dependencies of a node in dependency order.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        node_id: Root node for the dependency walk.

    Returns:
        Transitive dependency node IDs.

    Example:
        >>> from finstack_quant.statements_analytics import all_dependencies
        >>> chain = all_dependencies(model_json, "ni")
    """
    ...

def dependents(model: FinancialModelSpec | str, node_id: str) -> list[str]:
    """List nodes that depend on the given node (reverse dependencies).

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        node_id: Node whose dependents are listed.

    Returns:
        Dependent node IDs.

    Example:
        >>> from finstack_quant.statements_analytics import dependents
        >>> rev_deps = dependents(model_json, "rev")
    """
    ...

def explain_formula(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    node_id: str,
    period: str,
) -> dict[str, Any]:
    """Structured formula explanation for a node and period.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        results: ``StatementResult`` object or JSON string.
        node_id: Node to explain.
        period: Period string.

    Returns:
        Dict with ``node_id``, ``period_id``, ``final_value``, ``node_type``, ``formula_text``,
        and ``breakdown`` (list of component dicts: ``component``, ``value``, ``operation``).

    Example:
        >>> from finstack_quant.statements_analytics import explain_formula
        >>> detail = explain_formula(mj, rj, "rev", "2025Q1")
    """
    ...

def explain_formula_text(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    node_id: str,
    period: str,
) -> str:
    """Human-readable multi-line formula explanation.

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        results: ``StatementResult`` object or JSON string.
        node_id: Node to explain.
        period: Period string.

    Returns:
        Detailed text explanation.

    Example:
        >>> from finstack_quant.statements_analytics import explain_formula_text
        >>> text = explain_formula_text(mj, rj, "rev", "2025Q1")
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

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        suite_spec_json: JSON-serialized ``CheckSuiteSpec``.
        results: Optional pre-computed ``StatementResult`` (object or JSON);
            skips re-evaluation when provided.

    Returns:
        JSON-serialized ``CheckReport``.

    Example:
        >>> from finstack_quant.statements_analytics import run_checks
        >>> report_json = run_checks(model_json, suite_spec_json)
    """
    ...

def run_three_statement_checks(
    model: FinancialModelSpec | str,
    mapping_json: str,
    results: StatementResult | str | None = None,
) -> str:
    """Run three-statement checks using a node mapping (JSON in/out).

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        mapping_json: JSON-serialized ``ThreeStatementMapping``.
        results: Optional pre-computed ``StatementResult`` (object or JSON);
            skips re-evaluation when provided.

    Returns:
        JSON-serialized ``CheckReport``.

    Example:
        >>> from finstack_quant.statements_analytics import run_three_statement_checks
        >>> report_json = run_three_statement_checks(model_json, mapping_json)
    """
    ...

def run_credit_underwriting_checks(
    model: FinancialModelSpec | str,
    mapping_json: str,
    results: StatementResult | str | None = None,
) -> str:
    """Run credit underwriting checks using a node mapping (JSON in/out).

    Args:
        model: ``FinancialModelSpec`` object or JSON string.
        mapping_json: JSON-serialized ``CreditMapping``.
        results: Optional pre-computed ``StatementResult`` (object or JSON);
            skips re-evaluation when provided.

    Returns:
        JSON-serialized ``CheckReport``.

    Example:
        >>> from finstack_quant.statements_analytics import run_credit_underwriting_checks
        >>> report_json = run_credit_underwriting_checks(model_json, mapping_json)
    """
    ...

def render_check_report_text(report_json: str) -> str:
    """Render a check report as plain text.

    Args:
        report_json: JSON-serialized ``CheckReport``.

    Returns:
        Human-readable plain-text report.

    Example:
        >>> from finstack_quant.statements_analytics import render_check_report_text
        >>> text = render_check_report_text(report_json)
    """
    ...

def render_check_report_html(report_json: str) -> str:
    """Render a check report as HTML with inline styles.

    Args:
        report_json: JSON-serialized ``CheckReport``.

    Returns:
        HTML-formatted report suitable for Jupyter notebooks.

    Example:
        >>> from finstack_quant.statements_analytics import render_check_report_html
        >>> html = render_check_report_html(report_json)
    """
    ...

class Exposure:
    """A single credit exposure for ECL / IFRS 9 / CECL computation.

    All monetary fields are in the exposure's base currency; all rates and
    probabilities are expressed as decimals (``0.05`` = 5%).
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

    Args:
        exposure: Credit exposure.
        pd_delta_stage2: Absolute PD increase threshold (decimal) for SICR.
        dpd_30_trigger: Apply the 30-DPD Stage 2 rebuttable backstop.
        dpd_90_trigger: Apply the 90-DPD Stage 3 non-rebuttable backstop.

    Returns:
        ``(stage, trigger_reason)`` where stage is ``"Stage 1"``, ``"Stage 2"``,
        or ``"Stage 3"``.
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

    Args:
        ead: Exposure at default.
        pd_schedule: ``[(time_years, cumulative_pd), ...]`` knots. A
            ``(0.0, 0.0)`` knot is inserted automatically if not present.
        lgd: Loss given default (decimal).
        eir: Effective interest rate (decimal).
        max_horizon_years: Remaining maturity cap.
        bucket_width_years: Time-bucket width (default ``0.25`` for quarterly).
        stage: ``"stage1"``, ``"stage2"``, or ``"stage3"``.
        ead_schedule: Optional EAD amortization profile as
            ``[(time_years, ead), ...]`` knots.
        stage3_time_to_recovery_years: Stage 3 discounting horizon to expected
            recovery, in years.

    Returns:
        ECL amount in the exposure's base currency.
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

    Args:
        ead: Exposure at default.
        scenarios: List of ``(weight, pd_schedule)``. Weights must sum to 1.0.
            A ``(0.0, 0.0)`` knot is inserted automatically into each schedule
            if not present (same convention as ``compute_ecl``).
        lgd: Loss given default (decimal).
        eir: Effective interest rate (decimal).
        max_horizon: Remaining maturity cap (years).
        stage: ``"stage1"``, ``"stage2"``, or ``"stage3"``.
        ead_schedule: Optional EAD amortization profile as
            ``[(time_years, ead), ...]`` knots.
        stage3_time_to_recovery_years: Stage 3 discounting horizon to expected
            recovery, in years.

    Returns:
        Probability-weighted ECL amount.
    """
    ...

# ---------------------------------------------------------------------------
# Comparable-company analysis
# ---------------------------------------------------------------------------

def percentile_rank(value: float, peer_values: list[float]) -> float | None:
    """Percentile rank of ``value`` within ``peer_values`` on a 0-1 scale."""
    ...

def z_score(value: float, peer_values: list[float]) -> float | None:
    """Standard score of ``value`` within the peer distribution."""
    ...

def peer_stats(peer_values: list[float]) -> dict[str, float]:
    """Descriptive statistics for a peer distribution.

    Returns a dict with keys ``mean``, ``median``, ``q1``, ``q3``, ``iqr``,
    ``std_dev``, ``min``, ``max``, ``count`` (mirroring the Rust ``PeerStats``
    field names), or an empty dict when ``peer_values`` is empty.
    """
    ...

def regression_fair_value(
    x_values: list[float],
    y_values: list[float],
    subject_x: float,
    subject_y: float,
) -> dict[str, float]:
    """Single-factor OLS regression fair value with canonical residual semantics."""
    ...

def compute_multiple(
    company_metrics: dict[str, float],
    multiple: str,
) -> float | None:
    """Canonical multiple computation for one company."""
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
    """
    ...

# ---------------------------------------------------------------------------
# Credit scorecard extension
# ---------------------------------------------------------------------------

class ScorecardMetric:
    """A single scorecard metric (name, formula, weight, rating thresholds)."""

    def __init__(
        self,
        name: str,
        formula: str,
        weight: float = 1.0,
        thresholds_json: str = "{}",
        description: str | None = None,
    ) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def formula(self) -> str: ...
    @property
    def weight(self) -> float: ...
    @property
    def description(self) -> str | None: ...
    def thresholds_json(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> ScorecardMetric: ...

class ScorecardConfig:
    """Configuration for credit scorecard analysis.

    ``period`` optionally pins the rated period (e.g. ``"2025Q4"``); when
    ``None`` the scorecard rates the last actual period in the model if any
    exists, otherwise the last model period.
    """

    def __init__(
        self,
        rating_scale: str = "S&P",
        metrics: list[ScorecardMetric] = ...,
        min_rating: str | None = None,
        period: str | None = None,
    ) -> None: ...
    @property
    def rating_scale(self) -> str: ...
    @property
    def min_rating(self) -> str | None: ...
    @property
    def period(self) -> str | None: ...
    @property
    def metrics(self) -> list[ScorecardMetric]: ...
    def validate(self) -> None: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> ScorecardConfig: ...

class ScorecardReport:
    """Report produced by ``CreditScorecardExtension.execute``.

    ``data_json()`` includes the rated ``period``, the ``partial`` flag, and
    ``weight_coverage`` alongside the per-metric scores and rating.
    """

    @property
    def status(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def warnings(self) -> list[str]: ...
    @property
    def errors(self) -> list[str]: ...
    def data_json(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> ScorecardReport: ...

class CreditScorecardExtension:
    """Credit scorecard extension for rating assignment and stress testing."""

    def __init__(self) -> None: ...
    @staticmethod
    def with_config(config: ScorecardConfig) -> CreditScorecardExtension: ...
    def set_config(self, config: ScorecardConfig) -> None: ...
    def config(self) -> ScorecardConfig | None: ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> ScorecardReport: ...

def validate_scorecard_config(config: ScorecardConfig) -> None:
    """Validate a scorecard configuration without executing."""
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
    def from_str(value: str) -> AccountType: ...
    def value(self) -> str: ...

class CorkscrewAccount:
    """Single corkscrew account: balance node + change nodes + optional beginning override."""

    def __init__(
        self,
        node_id: str,
        account_type: AccountType,
        changes: list[str] = ...,
        beginning_balance_node: str | None = None,
    ) -> None: ...
    @property
    def node_id(self) -> str: ...
    @property
    def account_type(self) -> AccountType: ...
    @property
    def changes(self) -> list[str]: ...
    @property
    def beginning_balance_node(self) -> str | None: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> CorkscrewAccount: ...

class CorkscrewConfig:
    """Configuration for corkscrew (roll-forward) validation."""

    def __init__(
        self,
        accounts: list[CorkscrewAccount] = ...,
        tolerance: float = 0.01,
        fail_on_error: bool = False,
    ) -> None: ...
    @property
    def accounts(self) -> list[CorkscrewAccount]: ...
    @property
    def tolerance(self) -> float: ...
    @property
    def fail_on_error(self) -> bool: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> CorkscrewConfig: ...

class CorkscrewReport:
    """Report produced by ``CorkscrewExtension.execute``."""

    @property
    def status(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def warnings(self) -> list[str]: ...
    @property
    def errors(self) -> list[str]: ...
    def data_json(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> CorkscrewReport: ...

class CorkscrewExtension:
    """Corkscrew extension for balance-sheet roll-forward validation."""

    def __init__(self) -> None: ...
    @staticmethod
    def with_config(config: CorkscrewConfig) -> CorkscrewExtension: ...
    def set_config(self, config: CorkscrewConfig) -> None: ...
    def config(self) -> CorkscrewConfig | None: ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> CorkscrewReport: ...

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
    """
    ...

# ---------------------------------------------------------------------------
# Real-estate template
# ---------------------------------------------------------------------------

class SimpleLeaseSpec:
    """Lightweight per-lease rent schedule (period strings + base rent + growth + occupancy)."""

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
    def node_id(self) -> str: ...
    @property
    def start(self) -> str: ...
    @property
    def end(self) -> str | None: ...
    @property
    def base_rent(self) -> float: ...
    @property
    def growth_rate(self) -> float: ...
    @property
    def free_rent_periods(self) -> int: ...
    @property
    def occupancy(self) -> float: ...
    def validate(self) -> None: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> SimpleLeaseSpec: ...

class RentStepSpec:
    """Rent step that resets the base rent starting at ``start``."""

    def __init__(self, start: str, rent: float) -> None: ...
    @property
    def start(self) -> str: ...
    @property
    def rent(self) -> float: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> RentStepSpec: ...

class FreeRentWindowSpec:
    """Free rent (concession) window that zeros out rent for ``periods`` starting at ``start``."""

    def __init__(self, start: str, periods: int) -> None: ...
    @property
    def start(self) -> str: ...
    @property
    def periods(self) -> int: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> FreeRentWindowSpec: ...

class RenewalSpec:
    """Renewal specification modeled in expected-value terms."""

    def __init__(
        self,
        term_periods: int,
        probability: float,
        downtime_periods: int = 0,
        rent_factor: float = 1.0,
        free_rent_periods: int = 0,
    ) -> None: ...
    @property
    def term_periods(self) -> int: ...
    @property
    def probability(self) -> float: ...
    @property
    def downtime_periods(self) -> int: ...
    @property
    def rent_factor(self) -> float: ...
    @property
    def free_rent_periods(self) -> int: ...
    def validate(self) -> None: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> RenewalSpec: ...

class LeaseGrowthConvention:
    """Compounding convention for lease rent growth."""

    PerPeriod: LeaseGrowthConvention
    AnnualEscalator: LeaseGrowthConvention

    @staticmethod
    def from_str(value: str) -> LeaseGrowthConvention: ...
    def value(self) -> str: ...

class LeaseSpec:
    """Rich lease specification for rent-roll generation."""

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
    def node_id(self) -> str: ...
    @property
    def start(self) -> str: ...
    @property
    def end(self) -> str | None: ...
    @property
    def base_rent(self) -> float: ...
    @property
    def growth_rate(self) -> float: ...
    @property
    def growth_convention(self) -> LeaseGrowthConvention: ...
    @property
    def free_rent_periods(self) -> int: ...
    @property
    def occupancy(self) -> float: ...
    @property
    def renewal(self) -> RenewalSpec | None: ...
    def validate(self) -> None: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> LeaseSpec: ...

class RentRollOutputNodes:
    """Aggregated output node ids for a rent roll."""

    def __init__(
        self,
        rent_pgi_node: str = "rent_pgi",
        free_rent_node: str = "free_rent",
        vacancy_loss_node: str = "vacancy_loss",
        rent_effective_node: str = "rent_effective",
    ) -> None: ...
    @property
    def rent_pgi_node(self) -> str: ...
    @property
    def free_rent_node(self) -> str: ...
    @property
    def vacancy_loss_node(self) -> str: ...
    @property
    def rent_effective_node(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> RentRollOutputNodes: ...

class ManagementFeeBase:
    """Basis for management fee calculation."""

    Egi: ManagementFeeBase
    EffectiveRent: ManagementFeeBase

    @staticmethod
    def from_str(value: str) -> ManagementFeeBase: ...
    def value(self) -> str: ...

class ManagementFeeSpec:
    """Management fee specification (rate + base)."""

    def __init__(self, rate: float, base: ManagementFeeBase = ...) -> None: ...
    @property
    def rate(self) -> float: ...
    @property
    def base(self) -> ManagementFeeBase: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> ManagementFeeSpec: ...

class PropertyTemplateNodes:
    """Standard node ids for the full property operating-statement template."""

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
    def rent_roll(self) -> RentRollOutputNodes: ...
    @property
    def other_income_total_node(self) -> str: ...
    @property
    def egi_node(self) -> str: ...
    @property
    def management_fee_node(self) -> str: ...
    @property
    def opex_total_node(self) -> str: ...
    @property
    def noi_node(self) -> str: ...
    @property
    def capex_total_node(self) -> str: ...
    @property
    def ncf_node(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(json: str) -> PropertyTemplateNodes: ...

def add_noi_buildup(
    model: FinancialModelSpec | str,
    total_revenue_node: str,
    revenue_nodes: list[str],
    total_expenses_node: str,
    expense_nodes: list[str],
    noi_node: str,
) -> str:
    """Apply the NOI buildup template to a model spec. Returns JSON ``FinancialModelSpec``."""
    ...

def add_ncf_buildup(
    model: FinancialModelSpec | str,
    noi_node: str,
    capex_nodes: list[str],
    ncf_node: str,
) -> str:
    """Apply the NCF buildup template to a model spec. Returns JSON ``FinancialModelSpec``."""
    ...

def add_rent_roll(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    nodes: RentRollOutputNodes | None = None,
) -> str:
    """Apply the rich rent-roll template to a model spec. Returns JSON ``FinancialModelSpec``."""
    ...

def add_rent_roll_rental_revenue(
    model: FinancialModelSpec | str,
    leases: list[SimpleLeaseSpec],
    total_rent_node: str,
) -> str:
    """Apply the simple rent-roll rental-revenue template. Returns JSON ``FinancialModelSpec``."""
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
    """Apply the full property operating-statement template. Returns JSON ``FinancialModelSpec``."""
    ...
