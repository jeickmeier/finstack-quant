"""Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics."""

from __future__ import annotations

import pandas as pd

from finstack.core.market_data import MarketContext

__all__ = [
    "FinstackFxError",
    "FinstackOptimizationError",
    "FinstackValuationError",
    "Portfolio",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioResult",
    "PortfolioValuation",
    "aggregate_full_cashflows",
    "aggregate_metrics",
    "almgren_chriss_impact",
    "amihud_illiquidity",
    "apply_scenario_and_revalue",
    "brinson_fachler",
    "build_portfolio_from_spec",
    "carino_link",
    "days_to_liquidate",
    "evaluate_risk_budget",
    "historical_var_decomposition",
    "kyle_lambda",
    "liquidity_tier",
    "lvar_bangia",
    "mwr_xirr",
    "optimize_portfolio",
    "parametric_es_decomposition",
    "parametric_var_decomposition",
    "parse_portfolio_spec",
    "portfolio_result_get_metric",
    "portfolio_result_total_value",
    "replay_portfolio",
    "roll_effective_spread",
    "twrr_linked",
    "twrr_modified_dietz",
    "value_portfolio",
    # factor_model typed result classes
    "FactorContribution",
    "PositionFactorContribution",
    "PositionResidualContribution",
    "RiskDecomposition",
    "FactorRiskDecomposition",
    "SensitivityMatrix",
    "FactorPnlProfile",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "decompose_factor_risk",
    "PositionVarContribution",
    "PositionEsContribution",
    "PositionRiskDecomposition",
    "PositionBudgetEntry",
    "RiskBudgetResult",
    "FactorContributionDelta",
    "WhatIfResult",
    "StressResult",
    "StressPositionEntry",
    "TailScenarioBreakdown",
    "StressAttribution",
    "PositionAssignment",
    "UnmatchedEntry",
    "FactorAssignmentReport",
    "LevelVolContribution",
    "PositionVolContribution",
    "CreditVolReport",
    "VolHorizon",
    "DecompositionConfig",
    "parametric_var_decomposition_typed",
    "historical_var_decomposition_typed",
    "evaluate_risk_budget_typed",
    "position_component_var",
    # optimization spec/result classes
    "WeightingScheme",
    "MissingMetricPolicy",
    "Inequality",
    "TradeDirection",
    "TradeType",
    "PerPositionMetric",
    "PositionFilter",
    "MetricExpr",
    "Objective",
    "Constraint",
    "CandidatePosition",
    "TradeUniverse",
    "OptimizationStatus",
    "TradeSpec",
    "PortfolioOptimizationSpec",
    "PortfolioOptimizationResult",
    "optimize_portfolio_typed",
]

class PortfolioError(ValueError):
    """Portfolio validation or calculation failure."""

class FinstackValuationError(PortfolioError):
    """Portfolio valuation failure."""

class FinstackFxError(PortfolioError):
    """Portfolio FX conversion or market-data failure."""

class FinstackOptimizationError(PortfolioError):
    """Portfolio optimization failure."""

class Portfolio:
    """Built runtime portfolio. Cheap to clone; pass directly to pipeline functions.

    Build once with :meth:`from_spec` and reuse across ``value_portfolio``,
    ``aggregate_full_cashflows``, ``aggregate_metrics``, and
    ``apply_scenario_and_revalue`` to skip the per-call spec parse + index
    rebuild.
    """

    @staticmethod
    def from_spec(spec_json: str) -> Portfolio:
        """Parse a ``PortfolioSpec`` JSON string into a runtime portfolio."""
        ...

    @property
    def id(self) -> str: ...
    @property
    def as_of(self) -> str: ...
    @property
    def base_ccy(self) -> str: ...
    def __len__(self) -> int: ...
    def to_spec_json(self) -> str: ...
    def __repr__(self) -> str: ...

class PortfolioValuation:
    """Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation: ...
    def to_json(self) -> str: ...
    @property
    def total_value(self) -> float: ...
    @property
    def base_ccy(self) -> str: ...
    @property
    def as_of(self) -> str: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class PortfolioCashflows:
    """Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows: ...
    def to_json(self) -> str: ...
    def events_json(self) -> str: ...
    def by_date_json(self) -> str: ...
    def issues_json(self) -> str: ...
    def num_positions(self) -> int: ...
    def num_issues(self) -> int: ...
    def collapse_to_base_by_date_kind(
        self,
        market: MarketContext | str,
        base_ccy: str,
        as_of: str,
    ) -> str:
        """Collapse the ladder to a base-currency ``(date, kind) → Money`` JSON.

        Uses **spot-equivalent** FX at each payment date. ``as_of`` is the
        valuation/run date used to flag far-future conversions.
        """
        ...

    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class PortfolioResult:
    """Typed wrapper around a ``PortfolioResult`` envelope.

    Use the scalar accessors (``total_value``, ``get_metric``) to read single
    values without re-parsing the JSON envelope.
    """

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult: ...
    def to_json(self) -> str: ...
    @property
    def total_value(self) -> float: ...
    def get_metric(self, metric_id: str) -> float | None: ...
    def require_metric(self, metric_id: str) -> float: ...
    def __repr__(self) -> str: ...

def parse_portfolio_spec(json_str: str) -> str:
    """Parse and canonicalize a ``PortfolioSpec`` from JSON."""
    ...

def build_portfolio_from_spec(spec_json: str) -> str:
    """Build a runtime portfolio from JSON and return the round-tripped spec.

    Prefer :meth:`Portfolio.from_spec` for real work — it returns the typed
    object that pipeline functions reuse without rebuilding.
    """
    ...

def portfolio_result_total_value(result: PortfolioResult | str) -> float:
    """Read total portfolio value from a ``PortfolioResult`` envelope.

    Accepts a typed :class:`PortfolioResult` (O(1)) or a JSON string
    (O(size-of-envelope)).
    """
    ...

def portfolio_result_get_metric(result: PortfolioResult | str, metric_id: str) -> float | None:
    """Read one metric from a ``PortfolioResult``.

    Accepts a typed :class:`PortfolioResult` or a JSON string.
    """
    ...

def aggregate_metrics(
    valuation: PortfolioValuation | str,
    base_ccy: str,
    market: MarketContext | str,
    as_of: str,
) -> str:
    """Aggregate portfolio metrics from a valuation.

    Accepts a typed :class:`PortfolioValuation` (fast path) or a JSON string.
    """
    ...

def value_portfolio(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool = False,
) -> str:
    """Value a portfolio.

    Accepts either a typed :class:`Portfolio` (no rebuild) or a JSON
    ``PortfolioSpec`` string, and either a typed ``MarketContext`` or a JSON
    string. Returns JSON for backwards compatibility — wrap with
    :meth:`PortfolioValuation.from_json` once to enable the fast downstream
    path into ``aggregate_metrics``.
    """
    ...

def aggregate_full_cashflows(portfolio: Portfolio | str, market: MarketContext | str) -> PortfolioCashflows:
    """Build the full classified cashflow ladder for the portfolio.

    Returns a typed :class:`PortfolioCashflows` wrapper; call ``to_json()``
    to get the raw ladder or use the typed accessors to drill in.
    """
    ...

def apply_scenario_and_revalue(
    portfolio: Portfolio | str,
    scenario_json: str,
    market: MarketContext | str,
) -> tuple[str, str]:
    """Apply a scenario and revalue the portfolio.

    Returns ``(valuation_json, report_json)``.
    """
    ...

def optimize_portfolio(spec_json: str, market: MarketContext | str) -> str:
    """Optimize portfolio weights using the LP-based optimizer.

    Accepts a ``PortfolioOptimizationSpec`` JSON that combines the portfolio
    specification with an objective function, constraints, and weighting
    scheme. Returns compact JSON — use :func:`json.dumps(json.loads(...), indent=2)`
    to pretty-print if desired.
    """
    ...

def replay_portfolio(
    portfolio: Portfolio | str,
    snapshots_json: str,
    config_json: str,
) -> str: ...
def parametric_var_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]: ...
def parametric_es_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]: ...
def historical_var_decomposition(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]: ...
def evaluate_risk_budget(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> dict[str, object]: ...
def roll_effective_spread(returns: list[float]) -> float | None: ...
def amihud_illiquidity(returns: list[float], volumes: list[float]) -> float | None: ...
def days_to_liquidate(
    position_value: float,
    avg_daily_volume: float,
    participation_rate: float,
) -> float: ...
def liquidity_tier(days_to_liquidate: float) -> str: ...
def lvar_bangia(
    var: float,
    spread_mean: float,
    spread_vol: float,
    confidence: float,
    position_value: float,
) -> dict[str, float]: ...
def almgren_chriss_impact(
    position_size: float,
    avg_daily_volume: float,
    volatility: float,
    execution_horizon_days: float,
    permanent_impact_coef: float,
    temporary_impact_coef: float,
    reference_price: float | None = None,
) -> dict[str, float]: ...
def kyle_lambda(volumes: list[float], returns: list[float]) -> float | None: ...
def brinson_fachler(sectors_json: str) -> str:
    """Compute single-period Brinson-Fachler attribution from sector JSON."""
    ...

def carino_link(periods_json: str) -> str:
    """Compute Carino-linked multi-period Brinson attribution from period JSON."""
    ...

def twrr_modified_dietz(period_json: str) -> float | None:
    """Compute a Modified-Dietz TWRR sub-period return from period JSON."""
    ...

def twrr_linked(returns_json: str, horizon_years: float) -> str | None:
    """Geometrically link TWRR sub-period returns from returns JSON."""
    ...

def mwr_xirr(cashflows_json: str) -> float:
    """Compute money-weighted return via XIRR from dated cashflow JSON."""
    ...

# ---------------------------------------------------------------------------
# factor_model typed result classes (Slice 8)
# ---------------------------------------------------------------------------

class FactorContribution:
    """Aggregate contribution of a single factor to portfolio risk."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorContribution: ...
    def to_json(self) -> str: ...
    @property
    def factor_id(self) -> str: ...
    @property
    def absolute_risk(self) -> float: ...
    @property
    def relative_risk(self) -> float: ...
    @property
    def marginal_risk(self) -> float: ...
    def __repr__(self) -> str: ...

class PositionFactorContribution:
    """Per-position contribution to a specific factor bucket."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionFactorContribution: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def factor_id(self) -> str: ...
    @property
    def risk_contribution(self) -> float: ...
    def __repr__(self) -> str: ...

class PositionResidualContribution:
    """Annualized residual variance contributed by a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionResidualContribution: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def residual_variance(self) -> float: ...
    @property
    def source_kind(self) -> str: ...
    @property
    def source_issuer_id(self) -> str | None: ...
    def __repr__(self) -> str: ...

class RiskDecomposition:
    """Portfolio-level risk decomposition across factors and residuals."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskDecomposition: ...
    def to_json(self) -> str: ...
    @property
    def total_risk(self) -> float: ...
    @property
    def measure_json(self) -> str: ...
    @property
    def residual_risk(self) -> float: ...
    @property
    def factor_contributions(self) -> list[FactorContribution]: ...
    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]: ...
    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]: ...
    def __repr__(self) -> str: ...

class PositionVarContribution:
    """Per-position component / marginal VaR."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionVarContribution: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def component_var(self) -> float: ...
    @property
    def relative_var(self) -> float: ...
    @property
    def marginal_var(self) -> float | None: ...
    @property
    def incremental_var(self) -> float | None: ...
    def __repr__(self) -> str: ...

class PositionEsContribution:
    """Per-position component / marginal ES."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionEsContribution: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def component_es(self) -> float: ...
    @property
    def relative_es(self) -> float: ...
    @property
    def marginal_es(self) -> float | None: ...
    def __repr__(self) -> str: ...

class PositionRiskDecomposition:
    """Complete position-level risk decomposition."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionRiskDecomposition: ...
    def to_json(self) -> str: ...
    @property
    def portfolio_var(self) -> float: ...
    @property
    def portfolio_es(self) -> float: ...
    @property
    def confidence(self) -> float: ...
    @property
    def n_positions(self) -> int: ...
    @property
    def method(self) -> str: ...
    @property
    def euler_residual(self) -> float | None: ...
    @property
    def var_contributions(self) -> list[PositionVarContribution]: ...
    @property
    def es_contributions(self) -> list[PositionEsContribution]: ...
    def __repr__(self) -> str: ...

class PositionBudgetEntry:
    """Per-position budget comparison entry."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionBudgetEntry: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def actual_component_var(self) -> float: ...
    @property
    def target_component_var(self) -> float: ...
    @property
    def utilization(self) -> float: ...
    @property
    def excess(self) -> float: ...
    def __repr__(self) -> str: ...

class RiskBudgetResult:
    """Budget evaluation result across positions."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskBudgetResult: ...
    def to_json(self) -> str: ...
    @property
    def total_overbudget(self) -> float: ...
    @property
    def has_breach(self) -> bool: ...
    @property
    def positions(self) -> list[PositionBudgetEntry]: ...
    def __repr__(self) -> str: ...

class FactorContributionDelta:
    """Per-factor contribution change between a baseline and a scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorContributionDelta: ...
    def to_json(self) -> str: ...
    @property
    def factor_id(self) -> str: ...
    @property
    def absolute_change(self) -> float: ...
    @property
    def relative_change(self) -> float: ...
    def __repr__(self) -> str: ...

class WhatIfResult:
    """Result of a position what-if scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> WhatIfResult: ...
    def to_json(self) -> str: ...
    @property
    def before(self) -> RiskDecomposition: ...
    @property
    def after(self) -> RiskDecomposition: ...
    @property
    def delta(self) -> list[FactorContributionDelta]: ...
    def __repr__(self) -> str: ...

class StressResult:
    """Result of a factor-stress scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> StressResult: ...
    def to_json(self) -> str: ...
    @property
    def total_pnl(self) -> float: ...
    @property
    def position_pnl(self) -> list[tuple[str, float]]: ...
    @property
    def stressed_decomposition(self) -> RiskDecomposition: ...
    def __repr__(self) -> str: ...

class StressPositionEntry:
    """Single position's contribution to tail stress."""

    @classmethod
    def from_json(cls, json_str: str) -> StressPositionEntry: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def avg_tail_pnl(self) -> float: ...
    @property
    def pct_of_tail_loss(self) -> float: ...
    @property
    def worst_scenario_pnl(self) -> float: ...
    def __repr__(self) -> str: ...

class TailScenarioBreakdown:
    """Breakdown of a single tail scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> TailScenarioBreakdown: ...
    def to_json(self) -> str: ...
    @property
    def scenario_index(self) -> int: ...
    @property
    def portfolio_pnl(self) -> float: ...
    @property
    def position_pnls(self) -> list[tuple[str, float]]: ...
    def __repr__(self) -> str: ...

class StressAttribution:
    """Per-position attribution of portfolio losses in tail scenarios."""

    @classmethod
    def from_json(cls, json_str: str) -> StressAttribution: ...
    def to_json(self) -> str: ...
    @property
    def var_threshold(self) -> float: ...
    @property
    def n_tail_scenarios(self) -> int: ...
    @property
    def position_contributions(self) -> list[StressPositionEntry]: ...
    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]: ...
    def __repr__(self) -> str: ...

class PositionAssignment:
    """Matched factor assignments for a single portfolio position.

    The full ``(dependency, factor_id)`` pairs are available as JSON via
    :meth:`mappings_json`; matched factor identifiers are accessible directly
    via the :attr:`factor_ids` property.
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionAssignment: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def n_mappings(self) -> int: ...
    def mappings_json(self) -> str: ...
    @property
    def factor_ids(self) -> list[str]: ...
    def __repr__(self) -> str: ...

class UnmatchedEntry:
    """Single unmatched dependency surfaced during assignment."""

    @classmethod
    def from_json(cls, json_str: str) -> UnmatchedEntry: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    def dependency_json(self) -> str: ...
    def __repr__(self) -> str: ...

class FactorAssignmentReport:
    """Assignment results for a portfolio-level factor mapping pass."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorAssignmentReport: ...
    def to_json(self) -> str: ...
    @property
    def assignments(self) -> list[PositionAssignment]: ...
    @property
    def unmatched(self) -> list[UnmatchedEntry]: ...
    def __repr__(self) -> str: ...

class LevelVolContribution:
    """Aggregated risk contribution for a single hierarchy level."""

    @property
    def level_name(self) -> str: ...
    @property
    def total(self) -> float: ...
    @property
    def by_bucket(self) -> dict[str, float]: ...
    def __repr__(self) -> str: ...

class PositionVolContribution:
    """Per-position vol breakdown under :class:`CreditVolReport`."""

    @property
    def position_id(self) -> str: ...
    @property
    def factor_total(self) -> float: ...
    @property
    def idiosyncratic(self) -> float: ...
    @property
    def total(self) -> float: ...
    def __repr__(self) -> str: ...

class CreditVolReport:
    """Aggregated vol report grouped by hierarchy level."""

    @property
    def total(self) -> float: ...
    @property
    def measure_json(self) -> str: ...
    @property
    def generic(self) -> float: ...
    @property
    def idiosyncratic_total(self) -> float: ...
    @property
    def by_level(self) -> list[LevelVolContribution]: ...
    @property
    def by_position(self) -> list[PositionVolContribution] | None: ...
    def __repr__(self) -> str: ...

class VolHorizon:
    """Forecast horizon used to scale a calibrated `Sample` vol estimate."""

    @classmethod
    def one_step(cls) -> VolHorizon: ...
    @classmethod
    def unconditional(cls) -> VolHorizon: ...
    @classmethod
    def n_steps(cls, n: int) -> VolHorizon: ...
    @classmethod
    def years(cls, years: float) -> VolHorizon: ...
    @classmethod
    def parse(cls, s: str) -> VolHorizon: ...
    @property
    def kind(self) -> str: ...
    @property
    def n(self) -> int | None: ...
    @property
    def years_value(self) -> float | None: ...
    def __repr__(self) -> str: ...

class DecompositionConfig:
    """Configuration for position-level VaR decomposition."""

    @classmethod
    def parametric_95(cls) -> DecompositionConfig: ...
    @classmethod
    def parametric_99(cls) -> DecompositionConfig: ...
    @classmethod
    def historical(cls, confidence: float) -> DecompositionConfig: ...
    def with_incremental(self) -> DecompositionConfig: ...
    def with_seed(self, seed: int) -> DecompositionConfig: ...
    @property
    def confidence(self) -> float: ...
    @property
    def method(self) -> str: ...
    @property
    def compute_incremental(self) -> bool: ...
    @property
    def seed(self) -> int | None: ...
    def __repr__(self) -> str: ...

def parametric_var_decomposition_typed(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
    compute_incremental: bool = False,
) -> PositionRiskDecomposition:
    """Typed sibling of :func:`parametric_var_decomposition`."""
    ...

def historical_var_decomposition_typed(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> PositionRiskDecomposition:
    """Typed sibling of :func:`historical_var_decomposition`."""
    ...

def evaluate_risk_budget_typed(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> RiskBudgetResult:
    """Typed sibling of :func:`evaluate_risk_budget`."""
    ...

def position_component_var(
    decomp: PositionRiskDecomposition,
    position_id: str,
) -> float:
    """Look up a position's component VaR inside a decomposition (raises KeyError)."""
    ...

# ---------------------------------------------------------------------------
# optimization spec/result classes (Slice 9)
# ---------------------------------------------------------------------------

class WeightingScheme:
    """How optimization weights are defined."""

    @classmethod
    def value_weight(cls) -> WeightingScheme: ...
    @classmethod
    def notional_weight(cls) -> WeightingScheme: ...
    @classmethod
    def unit_scaling(cls) -> WeightingScheme: ...
    @property
    def label(self) -> str: ...
    def __repr__(self) -> str: ...

class MissingMetricPolicy:
    """Policy for handling positions missing required metrics."""

    @classmethod
    def zero(cls) -> MissingMetricPolicy: ...
    @classmethod
    def exclude(cls) -> MissingMetricPolicy: ...
    @classmethod
    def strict(cls) -> MissingMetricPolicy: ...
    @property
    def label(self) -> str: ...
    def __repr__(self) -> str: ...

class Inequality:
    """Inequality / equality operator (`<=`, `>=`, `==`)."""

    @classmethod
    def le(cls) -> Inequality: ...
    @classmethod
    def ge(cls) -> Inequality: ...
    @classmethod
    def eq(cls) -> Inequality: ...
    @property
    def label(self) -> str: ...
    def __repr__(self) -> str: ...

class TradeDirection:
    """Trade direction (buy/sell/hold)."""

    @classmethod
    def buy(cls) -> TradeDirection: ...
    @classmethod
    def sell(cls) -> TradeDirection: ...
    @classmethod
    def hold(cls) -> TradeDirection: ...
    @property
    def label(self) -> str: ...
    def __repr__(self) -> str: ...

class TradeType:
    """Trade type (existing/new-position/close-out)."""

    @classmethod
    def existing(cls) -> TradeType: ...
    @classmethod
    def new_position(cls) -> TradeType: ...
    @classmethod
    def close_out(cls) -> TradeType: ...
    @property
    def label(self) -> str: ...
    def __repr__(self) -> str: ...

class PerPositionMetric:
    """Per-position metric source for optimization expressions."""

    @classmethod
    def metric(cls, metric_id: str) -> PerPositionMetric: ...
    @classmethod
    def custom_key(cls, key: str) -> PerPositionMetric: ...
    @classmethod
    def pv_base(cls) -> PerPositionMetric: ...
    @classmethod
    def pv_native(cls) -> PerPositionMetric: ...
    @classmethod
    def attribute(cls, key: str) -> PerPositionMetric: ...
    @classmethod
    def attribute_indicator(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PerPositionMetric: ...
    @classmethod
    def constant(cls, value: float) -> PerPositionMetric: ...
    @classmethod
    def from_json(cls, json_str: str) -> PerPositionMetric: ...
    def to_json(self) -> str: ...
    @property
    def kind(self) -> str: ...
    def __repr__(self) -> str: ...

class PositionFilter:
    """Declarative filter for selecting which positions a rule applies to."""

    @classmethod
    def all(cls) -> PositionFilter: ...
    @classmethod
    def by_entity_id(cls, entity_id: str) -> PositionFilter: ...
    @classmethod
    def by_attribute(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PositionFilter: ...
    @classmethod
    def by_position_ids(cls, position_ids: list[str]) -> PositionFilter: ...
    @classmethod
    def not_(cls, inner: PositionFilter) -> PositionFilter: ...
    @classmethod
    def and_(cls, filters: list[PositionFilter]) -> PositionFilter: ...
    @classmethod
    def or_(cls, filters: list[PositionFilter]) -> PositionFilter: ...
    @classmethod
    def from_json(cls, json_str: str) -> PositionFilter: ...
    def to_json(self) -> str: ...
    @property
    def kind(self) -> str: ...
    def __repr__(self) -> str: ...

class MetricExpr:
    """Portfolio-level metric expression."""

    @classmethod
    def weighted_sum(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr: ...
    @classmethod
    def value_weighted_average(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr: ...
    @classmethod
    def from_json(cls, json_str: str) -> MetricExpr: ...
    def to_json(self) -> str: ...
    @property
    def kind(self) -> str: ...
    def __repr__(self) -> str: ...

class Objective:
    """Optimization direction and target."""

    @classmethod
    def maximize(cls, expr: MetricExpr) -> Objective: ...
    @classmethod
    def minimize(cls, expr: MetricExpr) -> Objective: ...
    @classmethod
    def from_json(cls, json_str: str) -> Objective: ...
    def to_json(self) -> str: ...
    @property
    def direction(self) -> str: ...
    @property
    def expr(self) -> MetricExpr: ...
    def __repr__(self) -> str: ...

class Constraint:
    """Declarative constraint specification."""

    @classmethod
    def metric_bound(
        cls,
        metric: MetricExpr,
        op: Inequality,
        rhs: float,
        label: str | None = None,
    ) -> Constraint: ...
    @classmethod
    def weight_bounds(
        cls,
        filter: PositionFilter,
        min: float,
        max: float,
        label: str | None = None,
    ) -> Constraint: ...
    @classmethod
    def max_turnover(
        cls,
        max_turnover: float,
        label: str | None = None,
    ) -> Constraint: ...
    @classmethod
    def budget(cls, rhs: float) -> Constraint: ...
    @classmethod
    def exposure_limit(
        cls,
        key: str,
        value: str,
        max_share: float,
        label: str | None = None,
    ) -> Constraint: ...
    @classmethod
    def exposure_minimum(
        cls,
        key: str,
        value: str,
        min_share: float,
        label: str | None = None,
    ) -> Constraint: ...
    def with_label(self, label: str) -> Constraint: ...
    @classmethod
    def from_json(cls, json_str: str) -> Constraint: ...
    def to_json(self) -> str: ...
    @property
    def kind(self) -> str: ...
    @property
    def label(self) -> str | None: ...
    def __repr__(self) -> str: ...

class CandidatePosition:
    """Candidate instrument that could be added to the portfolio.

    Construction from Python is not yet supported (requires the instrument
    binding bridge). Returned by getters on :class:`TradeUniverse`.
    """

    @property
    def id(self) -> str: ...
    @property
    def entity_id(self) -> str: ...
    @property
    def max_weight(self) -> float: ...
    @property
    def min_weight(self) -> float: ...
    @property
    def instrument_id(self) -> str: ...
    def __repr__(self) -> str: ...

class TradeUniverse:
    """Universe of tradeable existing positions and candidate additions."""

    @classmethod
    def all_positions(cls) -> TradeUniverse: ...
    @property
    def tradeable_filter(self) -> PositionFilter: ...
    @property
    def held_filter(self) -> PositionFilter | None: ...
    @property
    def candidates(self) -> list[CandidatePosition]: ...
    @property
    def allow_short_candidates(self) -> bool: ...
    def __repr__(self) -> str: ...

class OptimizationStatus:
    """Status of an optimization run."""

    @classmethod
    def optimal(cls) -> OptimizationStatus: ...
    @classmethod
    def feasible_but_suboptimal(cls) -> OptimizationStatus: ...
    @classmethod
    def unbounded(cls) -> OptimizationStatus: ...
    @classmethod
    def infeasible(cls, conflicting_constraints: list[str]) -> OptimizationStatus: ...
    @classmethod
    def error(cls, message: str) -> OptimizationStatus: ...
    @classmethod
    def from_json(cls, json_str: str) -> OptimizationStatus: ...
    def to_json(self) -> str: ...
    @property
    def kind(self) -> str: ...
    @property
    def is_feasible(self) -> bool: ...
    @property
    def conflicting_constraints(self) -> list[str]: ...
    @property
    def message(self) -> str | None: ...
    def __repr__(self) -> str: ...

class TradeSpec:
    """Trade specification for a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> TradeSpec: ...
    def to_json(self) -> str: ...
    @property
    def position_id(self) -> str: ...
    @property
    def instrument_id(self) -> str: ...
    @property
    def trade_type(self) -> TradeType: ...
    @property
    def direction(self) -> TradeDirection: ...
    @property
    def current_quantity(self) -> float: ...
    @property
    def target_quantity(self) -> float: ...
    @property
    def delta_quantity(self) -> float: ...
    @property
    def current_weight(self) -> float: ...
    @property
    def target_weight(self) -> float: ...
    def __repr__(self) -> str: ...

class PortfolioOptimizationSpec:
    """JSON-serializable portfolio optimization specification."""

    @classmethod
    def new(
        cls,
        portfolio_spec_json: str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec: ...
    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec: ...
    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec: ...
    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec: ...
    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec: ...
    def with_label(self, label: str) -> PortfolioOptimizationSpec: ...
    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec: ...
    def to_json(self) -> str: ...
    @property
    def objective(self) -> Objective: ...
    @property
    def constraints(self) -> list[Constraint]: ...
    @property
    def weighting(self) -> WeightingScheme: ...
    @property
    def missing_metric_policy(self) -> MissingMetricPolicy: ...
    @property
    def label(self) -> str | None: ...
    def portfolio_spec_json(self) -> str: ...
    def __repr__(self) -> str: ...

class PortfolioOptimizationResult:
    """Result of an optimization run (Serialize-only; no ``from_json``)."""

    def to_json(self) -> str: ...
    @property
    def status(self) -> OptimizationStatus: ...
    @property
    def is_feasible(self) -> bool: ...
    @property
    def objective_value(self) -> float: ...
    @property
    def current_weights(self) -> dict[str, float]: ...
    @property
    def optimal_weights(self) -> dict[str, float]: ...
    @property
    def weight_deltas(self) -> dict[str, float]: ...
    @property
    def implied_quantities(self) -> dict[str, float]: ...
    @property
    def metric_values(self) -> dict[str, float]: ...
    @property
    def dual_values(self) -> dict[str, float]: ...
    @property
    def constraint_slacks(self) -> dict[str, float]: ...
    @property
    def turnover(self) -> float: ...
    def to_trade_list(self) -> list[TradeSpec]: ...
    def new_position_trades(self) -> list[TradeSpec]: ...
    def binding_constraints(self) -> list[tuple[str, float]]: ...
    def __repr__(self) -> str: ...

def optimize_portfolio_typed(
    spec: PortfolioOptimizationSpec,
    market: MarketContext | str,
) -> PortfolioOptimizationResult:
    """Typed sibling of :func:`optimize_portfolio`.

    Accepts a typed :class:`PortfolioOptimizationSpec` and returns a typed
    :class:`PortfolioOptimizationResult` rather than JSON strings.
    """
    ...

# ---------------------------------------------------------------------------
# Factor Sensitivity
# ---------------------------------------------------------------------------

class SensitivityMatrix:
    """Positions-by-factors sensitivity matrix.

    Each element ``(i, j)`` is the first-order sensitivity of position *i* to
    factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
    for a rates factor).

    Construct via :func:`compute_factor_sensitivities`.

    Example:
        >>> from finstack.portfolio import compute_factor_sensitivities
        >>> matrix = compute_factor_sensitivities(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    """

    @property
    def position_ids(self) -> list[str]:
        """Ordered position identifiers (row axis)."""
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Ordered factor identifiers (column axis)."""
        ...

    @property
    def n_positions(self) -> int:
        """Number of positions (rows)."""
        ...

    @property
    def n_factors(self) -> int:
        """Number of factors (columns)."""
        ...

    def delta(self, position_idx: int, factor_idx: int) -> float:
        """Read a single sensitivity element.

        Args:
            position_idx: Row index.
            factor_idx: Column index.

        Returns:
            Sensitivity value.
        """
        ...

    def position_deltas(self, position_idx: int) -> list[float]:
        """Sensitivity row for a single position across all factors.

        Args:
            position_idx: Row index.

        Returns:
            List of delta values, one per factor.
        """
        ...

    def factor_deltas(self, factor_idx: int) -> list[float]:
        """Sensitivity column for a single factor across all positions.

        Args:
            factor_idx: Column index.

        Returns:
            List of delta values, one per position.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Export as a pandas DataFrame with positions as rows and factors as columns.

        Returns:
            DataFrame indexed by position IDs with factor IDs as column names.
        """
        ...

    def __repr__(self) -> str: ...

class FactorPnlProfile:
    """P&L profile for one factor across a scenario grid.

    Each profile captures the hypothetical P&L for every position at each
    scenario shift, enabling non-linear (gamma, convexity) analysis.

    Construct via :func:`compute_pnl_profiles`.

    Example:
        >>> from finstack.portfolio import compute_pnl_profiles
        >>> profiles = compute_pnl_profiles(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
    """

    @property
    def factor_id(self) -> str:
        """Factor identifier."""
        ...

    @property
    def shifts(self) -> list[float]:
        """Scenario shift coordinates (bump-size multiples)."""
        ...

    @property
    def position_pnls(self) -> list[list[float]]:
        """Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``."""
        ...

    def to_dataframe(self, position_ids: list[str]) -> pd.DataFrame:
        """Export as a pandas DataFrame with shifts as rows and positions as columns.

        Args:
            position_ids: Position identifiers to use as column names.  Must
                match the number of positions in the profile.

        Returns:
            DataFrame indexed by shift values with position IDs as column names.

        Raises:
            ValueError: If ``len(position_ids)`` does not match the profile width.
        """
        ...

    def __repr__(self) -> str: ...

def compute_factor_sensitivities(
    positions_json: str,
    factors_json: str,
    market: MarketContext | str,
    as_of: str,
    bump_config_json: str | None = None,
) -> SensitivityMatrix:
    """Compute first-order factor sensitivities using central finite differences.

    Args:
        positions_json: JSON array of position objects, each with ``id`` (str),
            ``instrument`` (tagged instrument JSON), and ``weight`` (float).
        factors_json: JSON array of ``FactorDefinition`` objects.
        market: ``MarketContext`` instance or JSON string.
        as_of: Valuation date in ISO 8601 format.
        bump_config_json: Optional JSON-serialized ``BumpSizeConfig``.
            Defaults to 1 bp / 1 % per factor type.

    Returns:
        Positions-by-factors delta matrix.

    Example:
        >>> from finstack.portfolio import compute_factor_sensitivities
        >>> matrix = compute_factor_sensitivities(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
        >>> matrix.to_dataframe()  # doctest: +SKIP
    """
    ...

def compute_pnl_profiles(
    positions_json: str,
    factors_json: str,
    market: MarketContext | str,
    as_of: str,
    bump_config_json: str | None = None,
    n_scenario_points: int = 5,
) -> list[FactorPnlProfile]:
    """Compute scenario P&L profiles via full repricing across a factor grid.

    Args:
        positions_json: JSON array of position objects (same schema as
            :func:`compute_factor_sensitivities`).
        factors_json: JSON array of ``FactorDefinition`` objects.
        market: ``MarketContext`` instance or JSON string.
        as_of: Valuation date in ISO 8601 format.
        bump_config_json: Optional JSON-serialized ``BumpSizeConfig``.
        n_scenario_points: Number of scenario grid points
            (default 5 produces shifts ``[-2, -1, 0, 1, 2]``).

    Returns:
        One profile per factor, each containing scenario P&L for every position.

    Example:
        >>> from finstack.portfolio import compute_pnl_profiles
        >>> profiles = compute_pnl_profiles(pos_json, fac_json, mkt_json, "2025-01-15")  # doctest: +SKIP
        >>> profiles[0].to_dataframe(["bond_1", "equity_1"])  # doctest: +SKIP
    """
    ...

# ---------------------------------------------------------------------------
# Risk Decomposition
# ---------------------------------------------------------------------------

class FactorRiskDecomposition:
    """Portfolio-level decomposition of total risk across factors and positions.

    Obtain via :func:`decompose_factor_risk`.  The decomposition expresses
    forecasted portfolio risk (variance, volatility, VaR, or ES) as a sum of
    Euler-allocated factor-level contributions, each drillable to per-position
    detail.

    Example:
        >>> from finstack.portfolio import decompose_factor_risk  # doctest: +SKIP
        >>> result = decompose_factor_risk(sens, cov_json)  # doctest: +SKIP
        >>> result.total_risk  # doctest: +SKIP
        0.042
    """

    @property
    def total_risk(self) -> float:
        """Total portfolio risk under the selected measure."""
        ...

    @property
    def measure(self) -> str:
        """Risk measure used (e.g. ``"Variance"``, ``"Volatility"``)."""
        ...

    @property
    def residual_risk(self) -> float:
        """Residual (idiosyncratic) risk not attributed to any factor."""
        ...

    def factor_contributions(self) -> list[dict[str, object]]:
        """Factor-level contributions as a list of dicts.

        Each dict contains ``factor_id``, ``absolute_risk``, ``relative_risk``,
        and ``marginal_risk``.

        Returns:
            List of per-factor contribution dicts.
        """
        ...

    def position_factor_contributions(self) -> list[dict[str, object]]:
        """Position x factor contributions as a list of dicts.

        Each dict contains ``position_id``, ``factor_id``, and
        ``risk_contribution``.

        Returns:
            List of per-position, per-factor contribution dicts.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns:
            DataFrame with one row per factor.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """Export position x factor contributions as a pandas DataFrame.

        Columns: ``position_id``, ``factor_id``, ``risk_contribution``.

        Returns:
            DataFrame with one row per position-factor pair.
        """
        ...

    def __repr__(self) -> str: ...

def decompose_factor_risk(
    sensitivities: SensitivityMatrix,
    covariance_json: str,
    risk_measure: str | dict | None = None,
) -> FactorRiskDecomposition:
    """Decompose portfolio risk into factor and position contributions.

    Uses the parametric (covariance-based) Euler decomposition to attribute
    forecasted portfolio risk across factors and individual positions.

    Args:
        sensitivities: Weighted position x factor sensitivity matrix, as
            returned by :func:`compute_factor_sensitivities`.
        covariance_json: JSON-serialized ``FactorCovarianceMatrix``.  Must use
            the same factor IDs and ordering as the sensitivity matrix.
        risk_measure: Risk measure.  Defaults to ``"variance"``.
            Accepts Python strings (``"variance"``, ``"volatility"``) or dicts
            (``{"var": {"confidence": 0.99}}``,
            ``{"expected_shortfall": {"confidence": 0.975}}``).

    Returns:
        Portfolio-level risk decomposition with factor and position detail.

    Raises:
        ValueError: If factor axes do not match or the covariance matrix is
            invalid.

    Example:
        >>> from finstack.portfolio import compute_factor_sensitivities, decompose_factor_risk
        >>> sens = compute_factor_sensitivities(pos, fac, mkt, "2025-01-15")  # doctest: +SKIP
        >>> result = decompose_factor_risk(sens, cov_json, "volatility")  # doctest: +SKIP
        >>> result.to_factor_dataframe()  # doctest: +SKIP
    """
    ...
