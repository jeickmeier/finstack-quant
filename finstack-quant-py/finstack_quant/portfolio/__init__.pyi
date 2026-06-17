"""Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics."""

from __future__ import annotations

from typing import Any

import pandas as pd

from finstack_quant.core.market_data import MarketContext

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
    "allocate_weights",
    "brinson_fachler",
    "build_credit_vol_report",
    "build_portfolio_from_spec",
    "build_stress_attribution",
    "carino_link",
    "days_to_liquidate",
    "evaluate_risk_budget",
    "factor_stress",
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
    "position_what_if",
    "roll_effective_spread",
    "twrr_linked",
    "twrr_modified_dietz",
    "validate_allocation_json",
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
    def id(self) -> str:
        """Portfolio identifier."""
        ...

    @property
    def as_of(self) -> str:
        """Portfolio as-of date as an ISO 8601 string."""
        ...

    @property
    def base_ccy(self) -> str:
        """Base currency code used for valuation and aggregation."""
        ...

    def __len__(self) -> int:
        """Number of positions in the built portfolio."""
        ...

    def to_spec_json(self) -> str:
        """Serialize the portfolio back to its canonical ``PortfolioSpec`` JSON."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PortfolioValuation:
    """Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation:
        """Deserialize a ``PortfolioValuation`` from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this valuation to canonical JSON."""
        ...

    @property
    def total_value(self) -> float:
        """Total portfolio value in ``base_ccy``."""
        ...

    @property
    def base_ccy(self) -> str:
        """Base currency code for this valuation."""
        ...

    @property
    def as_of(self) -> str:
        """Valuation date as an ISO 8601 string."""
        ...

    def __len__(self) -> int:
        """Number of valued positions."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PortfolioCashflows:
    """Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows:
        """Deserialize a cashflow ladder from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize the full cashflow ladder to canonical JSON."""
        ...

    def events_json(self) -> str:
        """Return all dated cashflow events as JSON."""
        ...

    def by_date_json(self) -> str:
        """Return cashflows grouped by date as JSON."""
        ...

    def issues_json(self) -> str:
        """Return cashflow aggregation or FX-conversion issues as JSON."""
        ...

    def num_positions(self) -> int:
        """Number of positions represented in the ladder."""
        ...

    def num_issues(self) -> int:
        """Number of diagnostic issues recorded on the ladder."""
        ...

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

    def __len__(self) -> int:
        """Number of cashflow events."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PortfolioResult:
    """Typed wrapper around a ``PortfolioResult`` envelope.

    Use the scalar accessors (``total_value``, ``get_metric``) to read single
    values without re-parsing the JSON envelope.
    """

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult:
        """Deserialize a portfolio result envelope from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this result envelope to canonical JSON."""
        ...

    @property
    def total_value(self) -> float:
        """Total value stored in the result envelope."""
        ...

    def get_metric(self, metric_id: str) -> float | None:
        """Return a metric value, or ``None`` when it is absent."""
        ...

    def require_metric(self, metric_id: str) -> float:
        """Return a metric value, raising ``KeyError`` when it is absent."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

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

def allocate_weights(spec_json: str) -> str:
    """Allocate strategy weights from a JSON specification.

    The specification contains a ``scheme`` (for example
    ``"inverse_volatility"``), ``total_capital``, and a list of strategy
    objects with ``id`` and return/history fields required by the selected
    scheme. The Rust allocator computes normalized weights and money amounts;
    Python only passes the JSON through.

    Args:
        spec_json: JSON-serialized allocation specification.

    Returns:
        JSON allocation result with the selected scheme and per-strategy
        ``id``, ``weight``, and allocated capital fields.

    Raises:
        ValueError: If the JSON is malformed, required fields are missing, the
            scheme is unsupported, or the selected scheme cannot be evaluated.
    """
    ...

def validate_allocation_json(spec_json: str) -> None:
    """Validate a strategy allocation JSON specification.

    Performs the same Rust-side parse and semantic validation used by
    :func:`allocate_weights` without computing allocations.

    Args:
        spec_json: JSON-serialized allocation specification.

    Raises:
        ValueError: If the specification is malformed or invalid.
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
) -> str:
    """Replay a portfolio through dated market snapshots.

    Args:
        portfolio: Typed :class:`Portfolio` or JSON ``PortfolioSpec``.
        snapshots_json: JSON array or envelope of market snapshots.
        config_json: JSON replay configuration controlling dates, valuation
            options, and output detail.

    Returns:
        JSON replay result containing dated valuations and diagnostics.

    Raises:
        PortfolioError: If the portfolio, snapshots, or replay config are
            invalid, or if a snapshot valuation fails.
    """
    ...

def parametric_var_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose portfolio parametric VaR across positions.

    Args:
        position_ids: Position identifiers aligned with ``weights``.
        weights: Portfolio weights or exposures.
        covariance: Square covariance matrix aligned with ``position_ids``.
        confidence: VaR confidence level in ``(0, 1)``.

    Returns:
        Dict containing portfolio VaR and per-position component, marginal, and
        relative VaR contributions.

    Raises:
        ValueError: If dimensions do not match, covariance is malformed, or the
            confidence level is invalid.
    """
    ...

def parametric_es_decomposition(
    position_ids: list[str],
    weights: list[float],
    covariance: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose portfolio parametric expected shortfall across positions.

    Args:
        position_ids: Position identifiers aligned with ``weights``.
        weights: Portfolio weights or exposures.
        covariance: Square covariance matrix aligned with ``position_ids``.
        confidence: ES confidence level in ``(0, 1)``.

    Returns:
        Dict containing portfolio ES and per-position component, marginal, and
        relative ES contributions.

    Raises:
        ValueError: If dimensions do not match, covariance is malformed, or the
            confidence level is invalid.
    """
    ...

def historical_var_decomposition(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> dict[str, object]:
    """Decompose historical VaR from scenario or realized position P&Ls.

    Args:
        position_ids: Position identifiers.
        position_pnls: Matrix of position P&Ls, one scenario row per list and
            one column per ``position_ids`` entry.
        confidence: Historical VaR confidence level in ``(0, 1)``.

    Returns:
        Dict containing portfolio historical VaR and per-position contribution
        estimates.

    Raises:
        ValueError: If the P&L matrix is empty, ragged, dimensionally
            inconsistent, or the confidence level is invalid.
    """
    ...

def evaluate_risk_budget(
    position_ids: list[str],
    actual_var: list[float],
    target_var_pct: list[float],
    portfolio_var: float,
    utilization_threshold: float = 1.20,
) -> dict[str, object]:
    """Compare actual position VaR against target risk-budget shares.

    Args:
        position_ids: Position identifiers aligned with ``actual_var`` and
            ``target_var_pct``.
        actual_var: Position component VaR amounts.
        target_var_pct: Target share of total portfolio VaR per position.
        portfolio_var: Total portfolio VaR used to convert target percentages
            into target VaR amounts.
        utilization_threshold: Breach threshold for actual / target
            utilization.

    Returns:
        Dict with per-position utilization, excess VaR, total over-budget risk,
        and breach flag.

    Raises:
        ValueError: If input lengths differ or risk-budget inputs are invalid.
    """
    ...

def roll_effective_spread(returns: list[float]) -> float | None:
    """Estimate the Roll effective bid-ask spread from returns.

    Returns ``None`` when there are too few observations or the first-order
    autocovariance does not imply a positive spread.
    """
    ...

def amihud_illiquidity(returns: list[float], volumes: list[float]) -> float | None:
    """Compute Amihud illiquidity from absolute returns and traded volumes.

    Args:
        returns: Period returns.
        volumes: Traded volumes aligned with ``returns``.

    Returns:
        Average ``abs(return) / volume`` over positive-volume observations, or
        ``None`` when no valid observations are available.
    """
    ...

def days_to_liquidate(
    position_value: float,
    avg_daily_volume: float,
    participation_rate: float,
) -> float:
    """Estimate liquidation horizon in trading days.

    Args:
        position_value: Position market value to liquidate.
        avg_daily_volume: Average daily market volume in the same notional
            units.
        participation_rate: Maximum fraction of daily volume the liquidation
            may consume.

    Returns:
        ``position_value / (avg_daily_volume * participation_rate)``.

    Raises:
        ValueError: If volume or participation inputs are non-positive.
    """
    ...

def liquidity_tier(days_to_liquidate: float) -> str:
    """Classify liquidation horizon into the Rust liquidity-tier labels."""
    ...

def lvar_bangia(
    var: float,
    spread_mean: float,
    spread_vol: float,
    confidence: float,
    position_value: float,
) -> dict[str, float]:
    """Compute Bangia-style liquidity-adjusted VaR.

    Args:
        var: Base market VaR.
        spread_mean: Mean bid-ask spread.
        spread_vol: Spread volatility.
        confidence: Confidence level for the liquidity adjustment.
        position_value: Position value used to scale spread cost.

    Returns:
        Dict containing the base VaR, liquidity add-on, and adjusted LVaR.
    """
    ...

def almgren_chriss_impact(
    position_size: float,
    avg_daily_volume: float,
    volatility: float,
    execution_horizon_days: float,
    permanent_impact_coef: float,
    temporary_impact_coef: float,
    reference_price: float | None = None,
) -> dict[str, float]:
    """Estimate Almgren-Chriss execution impact components.

    Args:
        position_size: Trade size in shares or notional units.
        avg_daily_volume: Average daily volume in matching units.
        volatility: Asset volatility used for risk scaling.
        execution_horizon_days: Execution horizon in trading days.
        permanent_impact_coef: Permanent impact coefficient.
        temporary_impact_coef: Temporary impact coefficient.
        reference_price: Optional price used to convert share impact to
            notional impact.

    Returns:
        Dict of permanent, temporary, and total impact estimates.
    """
    ...

def kyle_lambda(volumes: list[float], returns: list[float]) -> float | None:
    """Estimate Kyle's lambda from volume and return observations.

    Returns ``None`` when the aligned sample has too few valid observations or
    cannot support the regression-style estimate.
    """
    ...

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
    def from_json(cls, json_str: str) -> FactorContribution:
        """Deserialize a factor contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor contribution to JSON."""
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier."""
        ...

    @property
    def absolute_risk(self) -> float:
        """Absolute risk contribution."""
        ...

    @property
    def relative_risk(self) -> float:
        """Share of total portfolio risk."""
        ...

    @property
    def marginal_risk(self) -> float:
        """Marginal risk contribution."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionFactorContribution:
    """Per-position contribution to a specific factor bucket."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionFactorContribution:
        """Deserialize a position-factor contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position-factor contribution to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier."""
        ...

    @property
    def risk_contribution(self) -> float:
        """Risk contribution for this position-factor pair."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionResidualContribution:
    """Annualized residual variance contributed by a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionResidualContribution:
        """Deserialize a residual contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this residual contribution to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def residual_variance(self) -> float:
        """Residual variance assigned to this position."""
        ...

    @property
    def source_kind(self) -> str:
        """Source category used to derive residual risk."""
        ...

    @property
    def source_issuer_id(self) -> str | None:
        """Issuer identifier for issuer-sourced residual risk, if present."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class RiskDecomposition:
    """Portfolio-level risk decomposition across factors and residuals."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskDecomposition:
        """Deserialize a risk decomposition from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk decomposition to JSON."""
        ...

    @property
    def total_risk(self) -> float:
        """Total portfolio risk under the decomposition measure."""
        ...

    @property
    def measure_json(self) -> str:
        """Risk measure specification as JSON."""
        ...

    @property
    def residual_risk(self) -> float:
        """Residual risk not explained by factor contributions."""
        ...

    @property
    def factor_contributions(self) -> list[FactorContribution]:
        """Factor-level risk contributions."""
        ...

    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]:
        """Position-by-factor risk contributions."""
        ...

    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]:
        """Per-position residual risk contributions."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionVarContribution:
    """Per-position component / marginal VaR."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionVarContribution:
        """Deserialize a position VaR contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position VaR contribution to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def component_var(self) -> float:
        """Component VaR assigned to this position."""
        ...

    @property
    def relative_var(self) -> float:
        """Share of total portfolio VaR."""
        ...

    @property
    def marginal_var(self) -> float | None:
        """Marginal VaR, if computed."""
        ...

    @property
    def incremental_var(self) -> float | None:
        """Incremental VaR, if requested in the decomposition config."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionEsContribution:
    """Per-position component / marginal ES."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionEsContribution:
        """Deserialize a position ES contribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position ES contribution to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def component_es(self) -> float:
        """Component expected shortfall assigned to this position."""
        ...

    @property
    def relative_es(self) -> float:
        """Share of total portfolio expected shortfall."""
        ...

    @property
    def marginal_es(self) -> float | None:
        """Marginal expected shortfall, if computed."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionRiskDecomposition:
    """Complete position-level risk decomposition."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionRiskDecomposition:
        """Deserialize a position risk decomposition from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position risk decomposition to JSON."""
        ...

    @property
    def portfolio_var(self) -> float:
        """Portfolio VaR."""
        ...

    @property
    def portfolio_es(self) -> float:
        """Portfolio expected shortfall."""
        ...

    @property
    def confidence(self) -> float:
        """Confidence level used for VaR/ES."""
        ...

    @property
    def n_positions(self) -> int:
        """Number of positions included in the decomposition."""
        ...

    @property
    def method(self) -> str:
        """Decomposition method label."""
        ...

    @property
    def euler_residual(self) -> float | None:
        """Euler allocation residual, if reported."""
        ...

    @property
    def var_contributions(self) -> list[PositionVarContribution]:
        """Per-position VaR contributions."""
        ...

    @property
    def es_contributions(self) -> list[PositionEsContribution]:
        """Per-position expected shortfall contributions."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionBudgetEntry:
    """Per-position budget comparison entry."""

    @classmethod
    def from_json(cls, json_str: str) -> PositionBudgetEntry:
        """Deserialize a risk-budget entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk-budget entry to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def actual_component_var(self) -> float:
        """Actual component VaR for this position."""
        ...

    @property
    def target_component_var(self) -> float:
        """Target component VaR for this position."""
        ...

    @property
    def utilization(self) -> float:
        """Actual-to-target utilization ratio."""
        ...

    @property
    def excess(self) -> float:
        """Actual component VaR less target component VaR."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class RiskBudgetResult:
    """Budget evaluation result across positions."""

    @classmethod
    def from_json(cls, json_str: str) -> RiskBudgetResult:
        """Deserialize a risk-budget result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this risk-budget result to JSON."""
        ...

    @property
    def total_overbudget(self) -> float:
        """Total amount above target risk budgets."""
        ...

    @property
    def has_breach(self) -> bool:
        """Whether any position exceeds the utilization threshold."""
        ...

    @property
    def positions(self) -> list[PositionBudgetEntry]:
        """Per-position risk-budget entries."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class FactorContributionDelta:
    """Per-factor contribution change between a baseline and a scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorContributionDelta:
        """Deserialize a factor contribution delta from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor contribution delta to JSON."""
        ...

    @property
    def factor_id(self) -> str:
        """Factor identifier."""
        ...

    @property
    def absolute_change(self) -> float:
        """Absolute contribution change."""
        ...

    @property
    def relative_change(self) -> float:
        """Relative contribution change."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class WhatIfResult:
    """Result of a position what-if scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> WhatIfResult:
        """Deserialize a what-if result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this what-if result to JSON."""
        ...

    @property
    def before(self) -> RiskDecomposition:
        """Baseline risk decomposition."""
        ...

    @property
    def after(self) -> RiskDecomposition:
        """Post-scenario risk decomposition."""
        ...

    @property
    def delta(self) -> list[FactorContributionDelta]:
        """Per-factor contribution changes."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class StressResult:
    """Result of a factor-stress scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> StressResult:
        """Deserialize a stress result from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress result to JSON."""
        ...

    @property
    def total_pnl(self) -> float:
        """Total portfolio P&L under the stress scenario."""
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """Per-position P&L pairs."""
        ...

    @property
    def stressed_decomposition(self) -> RiskDecomposition:
        """Risk decomposition after applying the stress."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class StressPositionEntry:
    """Single position's contribution to tail stress."""

    @classmethod
    def from_json(cls, json_str: str) -> StressPositionEntry:
        """Deserialize a stress position entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress position entry to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def avg_tail_pnl(self) -> float:
        """Average P&L across tail scenarios."""
        ...

    @property
    def pct_of_tail_loss(self) -> float:
        """Share of aggregate tail loss."""
        ...

    @property
    def worst_scenario_pnl(self) -> float:
        """Worst single-scenario P&L for this position."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class TailScenarioBreakdown:
    """Breakdown of a single tail scenario."""

    @classmethod
    def from_json(cls, json_str: str) -> TailScenarioBreakdown:
        """Deserialize a tail scenario breakdown from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this tail scenario breakdown to JSON."""
        ...

    @property
    def scenario_index(self) -> int:
        """Scenario index in the source P&L matrix."""
        ...

    @property
    def portfolio_pnl(self) -> float:
        """Portfolio P&L for this tail scenario."""
        ...

    @property
    def position_pnls(self) -> list[tuple[str, float]]:
        """Per-position P&L pairs for this scenario."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class StressAttribution:
    """Per-position attribution of portfolio losses in tail scenarios."""

    @classmethod
    def from_json(cls, json_str: str) -> StressAttribution:
        """Deserialize stress attribution from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this stress attribution to JSON."""
        ...

    @property
    def var_threshold(self) -> float:
        """VaR threshold used to select tail scenarios."""
        ...

    @property
    def n_tail_scenarios(self) -> int:
        """Number of scenarios classified as tail scenarios."""
        ...

    @property
    def position_contributions(self) -> list[StressPositionEntry]:
        """Per-position tail-loss contributions."""
        ...

    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]:
        """Detailed tail scenario breakdowns."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionAssignment:
    """Matched factor assignments for a single portfolio position.

    The full ``(dependency, factor_id)`` pairs are available as JSON via
    :meth:`mappings_json`; matched factor identifiers are accessible directly
    via the :attr:`factor_ids` property.
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionAssignment:
        """Deserialize a position assignment from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position assignment to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def n_mappings(self) -> int:
        """Number of dependency-to-factor mappings."""
        ...

    def mappings_json(self) -> str:
        """Return detailed dependency-to-factor mappings as JSON."""
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Matched factor identifiers."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class UnmatchedEntry:
    """Single unmatched dependency surfaced during assignment."""

    @classmethod
    def from_json(cls, json_str: str) -> UnmatchedEntry:
        """Deserialize an unmatched dependency entry from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this unmatched entry to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    def dependency_json(self) -> str:
        """Return the unmatched dependency payload as JSON."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class FactorAssignmentReport:
    """Assignment results for a portfolio-level factor mapping pass."""

    @classmethod
    def from_json(cls, json_str: str) -> FactorAssignmentReport:
        """Deserialize a factor assignment report from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this factor assignment report to JSON."""
        ...

    @property
    def assignments(self) -> list[PositionAssignment]:
        """Matched assignments by position."""
        ...

    @property
    def unmatched(self) -> list[UnmatchedEntry]:
        """Dependencies that could not be mapped to factors."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class LevelVolContribution:
    """Aggregated risk contribution for a single hierarchy level."""

    @property
    def level_name(self) -> str:
        """Hierarchy level name."""
        ...

    @property
    def total(self) -> float:
        """Total contribution for this level."""
        ...

    @property
    def by_bucket(self) -> dict[str, float]:
        """Contribution by hierarchy bucket."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionVolContribution:
    """Per-position vol breakdown under :class:`CreditVolReport`."""

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def factor_total(self) -> float:
        """Factor-driven volatility contribution."""
        ...

    @property
    def idiosyncratic(self) -> float:
        """Idiosyncratic volatility contribution."""
        ...

    @property
    def total(self) -> float:
        """Total position volatility contribution."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class CreditVolReport:
    """Aggregated vol report grouped by hierarchy level."""

    @property
    def total(self) -> float:
        """Total portfolio volatility under the report measure."""
        ...

    @property
    def measure_json(self) -> str:
        """Risk measure specification as JSON."""
        ...

    @property
    def generic(self) -> float:
        """Generic factor contribution."""
        ...

    @property
    def idiosyncratic_total(self) -> float:
        """Aggregate idiosyncratic contribution."""
        ...

    @property
    def by_level(self) -> list[LevelVolContribution]:
        """Volatility contribution by hierarchy level."""
        ...

    @property
    def by_position(self) -> list[PositionVolContribution] | None:
        """Optional per-position volatility contributions."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class VolHorizon:
    """Forecast horizon used to scale a calibrated `Sample` vol estimate."""

    @classmethod
    def one_step(cls) -> VolHorizon:
        """Use the calibrated one-step forecast horizon."""
        ...

    @classmethod
    def unconditional(cls) -> VolHorizon:
        """Use the unconditional long-run forecast horizon."""
        ...

    @classmethod
    def n_steps(cls, n: int) -> VolHorizon:
        """Scale the forecast to ``n`` discrete steps."""
        ...

    @classmethod
    def years(cls, years: float) -> VolHorizon:
        """Scale the forecast to a year fraction."""
        ...

    @classmethod
    def parse(cls, s: str) -> VolHorizon:
        """Parse a horizon string accepted by the Rust factor model."""
        ...

    @property
    def kind(self) -> str:
        """Horizon variant label."""
        ...

    @property
    def n(self) -> int | None:
        """Step count for ``n_steps`` horizons."""
        ...

    @property
    def years_value(self) -> float | None:
        """Year fraction for ``years`` horizons."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class DecompositionConfig:
    """Configuration for position-level VaR decomposition."""

    @classmethod
    def parametric_95(cls) -> DecompositionConfig:
        """Default 95% parametric VaR decomposition config."""
        ...

    @classmethod
    def parametric_99(cls) -> DecompositionConfig:
        """Default 99% parametric VaR decomposition config."""
        ...

    @classmethod
    def historical(cls, confidence: float) -> DecompositionConfig:
        """Historical decomposition config at ``confidence``."""
        ...

    def with_incremental(self) -> DecompositionConfig:
        """Return a copy that requests incremental VaR."""
        ...

    def with_seed(self, seed: int) -> DecompositionConfig:
        """Return a copy with deterministic simulation/randomization seed."""
        ...

    @property
    def confidence(self) -> float:
        """VaR/ES confidence level."""
        ...

    @property
    def method(self) -> str:
        """Decomposition method label."""
        ...

    @property
    def compute_incremental(self) -> bool:
        """Whether incremental VaR is requested."""
        ...

    @property
    def seed(self) -> int | None:
        """Optional deterministic seed."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

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

def factor_stress(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    stresses: list[tuple[str, float]],
) -> StressResult:
    """Run a factor-stress scenario and revalue the portfolio under the stressed market."""
    ...

def position_what_if(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: str,
    changes: list[dict[str, Any]],
) -> WhatIfResult:
    """Run position remove/resize what-if analysis from a factor-model config."""
    ...

def build_stress_attribution(
    position_ids: list[str],
    position_pnls: list[list[float]],
    confidence: float = 0.95,
) -> StressAttribution:
    """Build tail-scenario stress attribution from per-position scenario P&Ls."""
    ...

def build_credit_vol_report(
    decomposition: RiskDecomposition,
    model: Any,
    by_position: bool = False,
) -> CreditVolReport:
    """Build a credit volatility report from a risk decomposition and credit model."""
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
    def value_weight(cls) -> WeightingScheme:
        """Weight positions by market value."""
        ...

    @classmethod
    def notional_weight(cls) -> WeightingScheme:
        """Weight positions by notional."""
        ...

    @classmethod
    def unit_scaling(cls) -> WeightingScheme:
        """Use unit scaling rather than value/notional scaling."""
        ...

    @property
    def label(self) -> str:
        """Rust enum label for this weighting scheme."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class MissingMetricPolicy:
    """Policy for handling positions missing required metrics."""

    @classmethod
    def zero(cls) -> MissingMetricPolicy:
        """Treat missing metric values as zero."""
        ...

    @classmethod
    def exclude(cls) -> MissingMetricPolicy:
        """Exclude positions with missing required metrics."""
        ...

    @classmethod
    def strict(cls) -> MissingMetricPolicy:
        """Reject optimization when required metrics are missing."""
        ...

    @property
    def label(self) -> str:
        """Rust enum label for this policy."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class Inequality:
    """Inequality / equality operator (`<=`, `>=`, `==`)."""

    @classmethod
    def le(cls) -> Inequality:
        """Less-than-or-equal inequality."""
        ...

    @classmethod
    def ge(cls) -> Inequality:
        """Greater-than-or-equal inequality."""
        ...

    @classmethod
    def eq(cls) -> Inequality:
        """Equality constraint."""
        ...

    @property
    def label(self) -> str:
        """Operator label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class TradeDirection:
    """Trade direction (buy/sell/hold)."""

    @classmethod
    def buy(cls) -> TradeDirection:
        """Buy direction."""
        ...

    @classmethod
    def sell(cls) -> TradeDirection:
        """Sell direction."""
        ...

    @classmethod
    def hold(cls) -> TradeDirection:
        """Hold/no-trade direction."""
        ...

    @property
    def label(self) -> str:
        """Direction label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class TradeType:
    """Trade type (existing/new-position/close-out)."""

    @classmethod
    def existing(cls) -> TradeType:
        """Trade an existing position."""
        ...

    @classmethod
    def new_position(cls) -> TradeType:
        """Open a new candidate position."""
        ...

    @classmethod
    def close_out(cls) -> TradeType:
        """Close an existing position."""
        ...

    @property
    def label(self) -> str:
        """Trade-type label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PerPositionMetric:
    """Per-position metric source for optimization expressions."""

    @classmethod
    def metric(cls, metric_id: str) -> PerPositionMetric:
        """Use a valuation metric by metric ID."""
        ...

    @classmethod
    def custom_key(cls, key: str) -> PerPositionMetric:
        """Use a custom per-position metric key."""
        ...

    @classmethod
    def pv_base(cls) -> PerPositionMetric:
        """Use present value converted to portfolio base currency."""
        ...

    @classmethod
    def pv_native(cls) -> PerPositionMetric:
        """Use present value in the instrument native currency."""
        ...

    @classmethod
    def attribute(cls, key: str) -> PerPositionMetric:
        """Use a position attribute as a metric source."""
        ...

    @classmethod
    def attribute_indicator(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PerPositionMetric:
        """Use a boolean position-attribute comparison as an indicator metric."""
        ...

    @classmethod
    def constant(cls, value: float) -> PerPositionMetric:
        """Use a constant metric value for every selected position."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PerPositionMetric:
        """Deserialize a per-position metric expression from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this per-position metric expression to JSON."""
        ...

    @property
    def kind(self) -> str:
        """Metric-source variant label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PositionFilter:
    """Declarative filter for selecting which positions a rule applies to."""

    @classmethod
    def all(cls) -> PositionFilter:
        """Select all positions."""
        ...

    @classmethod
    def by_entity_id(cls, entity_id: str) -> PositionFilter:
        """Select positions for one entity ID."""
        ...

    @classmethod
    def by_attribute(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PositionFilter:
        """Select positions by attribute comparison."""
        ...

    @classmethod
    def by_position_ids(cls, position_ids: list[str]) -> PositionFilter:
        """Select positions by explicit position IDs."""
        ...

    @classmethod
    def not_(cls, inner: PositionFilter) -> PositionFilter:
        """Negate another filter."""
        ...

    @classmethod
    def and_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """Select positions matching all child filters."""
        ...

    @classmethod
    def or_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """Select positions matching any child filter."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PositionFilter:
        """Deserialize a position filter from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this position filter to JSON."""
        ...

    @property
    def kind(self) -> str:
        """Filter variant label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class MetricExpr:
    """Portfolio-level metric expression."""

    @classmethod
    def weighted_sum(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """Build a weighted-sum portfolio metric expression."""
        ...

    @classmethod
    def value_weighted_average(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """Build a value-weighted-average portfolio metric expression."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> MetricExpr:
        """Deserialize a metric expression from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this metric expression to JSON."""
        ...

    @property
    def kind(self) -> str:
        """Metric-expression variant label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class Objective:
    """Optimization direction and target."""

    @classmethod
    def maximize(cls, expr: MetricExpr) -> Objective:
        """Maximize the supplied metric expression."""
        ...

    @classmethod
    def minimize(cls, expr: MetricExpr) -> Objective:
        """Minimize the supplied metric expression."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Objective:
        """Deserialize an optimization objective from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization objective to JSON."""
        ...

    @property
    def direction(self) -> str:
        """Optimization direction label."""
        ...

    @property
    def expr(self) -> MetricExpr:
        """Metric expression being optimized."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class Constraint:
    """Declarative constraint specification."""

    @classmethod
    def metric_bound(
        cls,
        metric: MetricExpr,
        op: Inequality,
        rhs: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain a metric expression against a right-hand side."""
        ...

    @classmethod
    def weight_bounds(
        cls,
        filter: PositionFilter,
        min: float,
        max: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain selected position weights to a min/max interval."""
        ...

    @classmethod
    def max_turnover(
        cls,
        max_turnover: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain total portfolio turnover."""
        ...

    @classmethod
    def budget(cls, rhs: float) -> Constraint:
        """Constrain total portfolio budget/weight to ``rhs``."""
        ...

    @classmethod
    def exposure_limit(
        cls,
        key: str,
        value: str,
        max_share: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain maximum exposure share for an attribute key/value."""
        ...

    @classmethod
    def exposure_minimum(
        cls,
        key: str,
        value: str,
        min_share: float,
        label: str | None = None,
    ) -> Constraint:
        """Constrain minimum exposure share for an attribute key/value."""
        ...

    def with_label(self, label: str) -> Constraint:
        """Return a copy with a human-readable label."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Constraint:
        """Deserialize an optimization constraint from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization constraint to JSON."""
        ...

    @property
    def kind(self) -> str:
        """Constraint variant label."""
        ...

    @property
    def label(self) -> str | None:
        """Optional human-readable label."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class CandidatePosition:
    """Candidate instrument that could be added to the portfolio.

    Construction from Python is not yet supported (requires the instrument
    binding bridge). Returned by getters on :class:`TradeUniverse`.
    """

    @property
    def id(self) -> str:
        """Candidate position identifier."""
        ...

    @property
    def entity_id(self) -> str:
        """Candidate entity identifier."""
        ...

    @property
    def max_weight(self) -> float:
        """Maximum allowed candidate weight."""
        ...

    @property
    def min_weight(self) -> float:
        """Minimum allowed candidate weight."""
        ...

    @property
    def instrument_id(self) -> str:
        """Underlying instrument identifier."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class TradeUniverse:
    """Universe of tradeable existing positions and candidate additions."""

    @classmethod
    def all_positions(cls) -> TradeUniverse:
        """Make every existing position tradeable."""
        ...

    @property
    def tradeable_filter(self) -> PositionFilter:
        """Filter selecting tradeable positions."""
        ...

    @property
    def held_filter(self) -> PositionFilter | None:
        """Optional filter selecting held positions."""
        ...

    @property
    def candidates(self) -> list[CandidatePosition]:
        """Candidate new positions."""
        ...

    @property
    def allow_short_candidates(self) -> bool:
        """Whether candidates may receive negative weights."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class OptimizationStatus:
    """Status of an optimization run."""

    @classmethod
    def optimal(cls) -> OptimizationStatus:
        """Successful optimal solve."""
        ...

    @classmethod
    def feasible_but_suboptimal(cls) -> OptimizationStatus:
        """Feasible solution that did not prove optimality."""
        ...

    @classmethod
    def unbounded(cls) -> OptimizationStatus:
        """Optimization problem is unbounded."""
        ...

    @classmethod
    def infeasible(cls, conflicting_constraints: list[str]) -> OptimizationStatus:
        """Optimization problem is infeasible with the listed constraints."""
        ...

    @classmethod
    def error(cls, message: str) -> OptimizationStatus:
        """Solver or model-building error status."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> OptimizationStatus:
        """Deserialize an optimization status from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization status to JSON."""
        ...

    @property
    def kind(self) -> str:
        """Status variant label."""
        ...

    @property
    def is_feasible(self) -> bool:
        """Whether the status includes a feasible solution."""
        ...

    @property
    def conflicting_constraints(self) -> list[str]:
        """Constraint labels implicated in infeasibility."""
        ...

    @property
    def message(self) -> str | None:
        """Error or diagnostic message, if present."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class TradeSpec:
    """Trade specification for a single position."""

    @classmethod
    def from_json(cls, json_str: str) -> TradeSpec:
        """Deserialize a trade specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this trade specification to JSON."""
        ...

    @property
    def position_id(self) -> str:
        """Portfolio position identifier."""
        ...

    @property
    def instrument_id(self) -> str:
        """Underlying instrument identifier."""
        ...

    @property
    def trade_type(self) -> TradeType:
        """Trade type."""
        ...

    @property
    def direction(self) -> TradeDirection:
        """Trade direction."""
        ...

    @property
    def current_quantity(self) -> float:
        """Current quantity."""
        ...

    @property
    def target_quantity(self) -> float:
        """Target quantity."""
        ...

    @property
    def delta_quantity(self) -> float:
        """Target quantity less current quantity."""
        ...

    @property
    def current_weight(self) -> float:
        """Current portfolio weight."""
        ...

    @property
    def target_weight(self) -> float:
        """Target portfolio weight."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PortfolioOptimizationSpec:
    """JSON-serializable portfolio optimization specification."""

    @classmethod
    def new(
        cls,
        portfolio_spec_json: str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec:
        """Create a portfolio optimization specification from portfolio JSON and objective."""
        ...

    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec:
        """Return a copy with an additional constraint."""
        ...

    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement objective."""
        ...

    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement weighting scheme."""
        ...

    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec:
        """Return a copy with a replacement missing-metric policy."""
        ...

    def with_label(self, label: str) -> PortfolioOptimizationSpec:
        """Return a copy with a human-readable label."""
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec:
        """Deserialize an optimization specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this optimization specification to JSON."""
        ...

    @property
    def objective(self) -> Objective:
        """Optimization objective."""
        ...

    @property
    def constraints(self) -> list[Constraint]:
        """Optimization constraints."""
        ...

    @property
    def weighting(self) -> WeightingScheme:
        """Weighting scheme used by the optimization."""
        ...

    @property
    def missing_metric_policy(self) -> MissingMetricPolicy:
        """Policy for missing per-position metrics."""
        ...

    @property
    def label(self) -> str | None:
        """Optional human-readable label."""
        ...

    def portfolio_spec_json(self) -> str:
        """Return the embedded portfolio specification JSON."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class PortfolioOptimizationResult:
    """Result of an optimization run (Serialize-only; no ``from_json``)."""

    def to_json(self) -> str:
        """Serialize this optimization result to JSON."""
        ...

    @property
    def status(self) -> OptimizationStatus:
        """Optimization status."""
        ...

    @property
    def is_feasible(self) -> bool:
        """Whether the solver returned a feasible portfolio."""
        ...

    @property
    def objective_value(self) -> float:
        """Objective value at the solution."""
        ...

    @property
    def current_weights(self) -> dict[str, float]:
        """Current weights by position ID."""
        ...

    @property
    def optimal_weights(self) -> dict[str, float]:
        """Optimized target weights by position ID."""
        ...

    @property
    def weight_deltas(self) -> dict[str, float]:
        """Target less current weight by position ID."""
        ...

    @property
    def implied_quantities(self) -> dict[str, float]:
        """Implied target quantities by position ID."""
        ...

    @property
    def metric_values(self) -> dict[str, float]:
        """Portfolio metric values at the solution."""
        ...

    @property
    def dual_values(self) -> dict[str, float]:
        """Dual values by constraint label when available."""
        ...

    @property
    def constraint_slacks(self) -> dict[str, float]:
        """Constraint slack values by constraint label."""
        ...

    @property
    def turnover(self) -> float:
        """Total turnover implied by the solution."""
        ...

    def to_trade_list(self) -> list[TradeSpec]:
        """Convert weight deltas into trade specifications."""
        ...

    def new_position_trades(self) -> list[TradeSpec]:
        """Return trade specs for new candidate positions only."""
        ...

    def binding_constraints(self) -> list[tuple[str, float]]:
        """Return constraints with near-zero slack as ``(label, slack)`` pairs."""
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

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
        >>> from finstack_quant.portfolio import compute_factor_sensitivities
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

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

class FactorPnlProfile:
    """P&L profile for one factor across a scenario grid.

    Each profile captures the hypothetical P&L for every position at each
    scenario shift, enabling non-linear (gamma, convexity) analysis.

    Construct via :func:`compute_pnl_profiles`.

    Example:
        >>> from finstack_quant.portfolio import compute_pnl_profiles
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

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

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
        >>> from finstack_quant.portfolio import compute_factor_sensitivities
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
        >>> from finstack_quant.portfolio import compute_pnl_profiles
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
        >>> from finstack_quant.portfolio import decompose_factor_risk  # doctest: +SKIP
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

    def __repr__(self) -> str:
        """Return a concise debug representation."""
        ...

def decompose_factor_risk(
    sensitivities: SensitivityMatrix,
    covariance_json: str,
    risk_measure: str | dict[str, Any] | None = None,
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
        >>> from finstack_quant.portfolio import compute_factor_sensitivities, decompose_factor_risk
        >>> sens = compute_factor_sensitivities(pos, fac, mkt, "2025-01-15")  # doctest: +SKIP
        >>> result = decompose_factor_risk(sens, cov_json, "volatility")  # doctest: +SKIP
        >>> result.to_factor_dataframe()  # doctest: +SKIP
    """
    ...
