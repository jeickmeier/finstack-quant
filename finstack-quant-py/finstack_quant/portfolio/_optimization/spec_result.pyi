"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/optimization/spec_result.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.portfolio._container import Portfolio
from finstack_quant.portfolio._optimization.enums import MissingMetricPolicy, WeightingScheme
from finstack_quant.portfolio._optimization.expressions import Constraint, Objective, PositionFilter
from finstack_quant.portfolio._optimization.status_trade import OptimizationStatus, TradeSpec

class CandidatePosition:
    """
    Candidate instrument the optimizer may add to the portfolio.

    Starts at weight zero and is bounded by ``min_weight`` / ``max_weight``.
    Attach candidates to a :class:`TradeUniverse` via ``with_candidate``.
    Candidate IDs must be unique among candidates and absent from existing
    portfolio positions; optimization raises ``PortfolioError`` for a collision.

    Examples
    --------
    >>> from finstack_quant.portfolio import CandidatePosition
    >>> try:
    ...     CandidatePosition("C1", "E1", "not-an-instrument")
    ... except ValueError:
    ...     print("invalid instrument payload")
    invalid instrument payload
    """

    def __init__(
        self,
        id: str,
        entity_id: str,
        instrument: Any,
        unit: str | dict[str, Any] | None = None,
        max_weight: float = 1.0,
        min_weight: float = 0.0,
        attributes: dict[str, str | float] | None = None,
    ) -> None:
        """
        Create a candidate.

        Parameters
        ----------
        id : str
            Identifier that becomes the position id if the optimizer trades it.
            Must be unique among candidates and absent from existing positions.
        entity_id : str
            Owning entity for the candidate.
        instrument : Bond | InterestRateSwap | ... | str
            Typed instrument wrapper or canonical instrument-envelope JSON.
        unit : str | dict | None
            Position unit: ``"units"`` (default), ``"face_value"``,
            ``"percentage"``, or the mapping ``{"notional": "USD"}`` /
            ``{"notional": None}``. The bare string ``"notional"`` is
            rejected, as in the JSON spec.
        max_weight : float
            Maximum weight (fraction) the candidate may receive.
        min_weight : float
            Minimum weight when included; ``0.0`` lets the optimizer skip it.
        attributes : dict[str, str | float] | None
            Attributes used by filters and exposure constraints.

        Raises
        ------
        TypeError
            If ``instrument`` is not a typed instrument, an envelope dict or a string.
        ValueError
            If the instrument payload is invalid or ``unit`` is unknown.

        Examples
        --------
        >>> from finstack_quant.portfolio import CandidatePosition
        >>> try:
        ...     CandidatePosition("C1", "E1", "{}")
        ... except ValueError:
        ...     print("invalid instrument payload")
        invalid instrument payload
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> CandidatePosition:
        """
        Parse from JSON (the instrument travels as its canonical tagged payload).

        Parameters
        ----------
        json_str : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CandidatePosition
            Reconstructed candidate.

        Raises
        ------
        ValueError
            If the payload or embedded instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.portfolio import CandidatePosition
        >>> try:
        ...     CandidatePosition.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the instrument has no JSON representation.
        """
        ...

    @property
    def attributes(self) -> dict[str, str | float]:
        """
        Candidate attributes.

        Returns
        -------
        dict[str, str | float]
            Attribute values keyed by name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def id(self) -> str:
        """
        Candidate position identifier.

        Returns
        -------
        str
            Candidate identifier, unique among candidates and absent from
            existing positions; becomes the position identifier when traded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def entity_id(self) -> str:
        """
        Candidate entity identifier.

        Returns
        -------
        str
            Candidate entity identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_weight(self) -> float:
        """
        Maximum allowed candidate weight.

        Returns
        -------
        float
            Maximum allowed candidate weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def min_weight(self) -> float:
        """
        Minimum allowed candidate weight.

        Returns
        -------
        float
            Minimum allowed candidate weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Underlying instrument identifier.

        Returns
        -------
        str
            Underlying instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeUniverse:
    """
    Universe of tradeable existing positions and candidate additions.

    Build with :meth:`all_positions` or :meth:`filtered`, chain
    :meth:`with_candidate` / :meth:`allow_shorting_candidates`, and attach
    it to a :class:`PortfolioOptimizationSpec` via ``with_trade_universe``.

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeUniverse
    >>> TradeUniverse.all_positions().tradeable_filter.kind
    'all'
    """

    @classmethod
    def all_positions(cls) -> TradeUniverse:
        """
        Universe where every existing position is tradeable and no candidates exist.

        Returns
        -------
        TradeUniverse
            Default universe.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.all_positions().allow_short_candidates
        False
        """
        ...

    @classmethod
    def filtered(cls, filter: PositionFilter) -> TradeUniverse:
        """
        Universe where only positions matching ``filter`` may trade.

        Parameters
        ----------
        filter : PositionFilter
            Filter selecting the tradeable existing positions.

        Returns
        -------
        TradeUniverse
            Universe with the supplied tradeable filter and no candidates.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter, TradeUniverse
        >>> TradeUniverse.filtered(PositionFilter.by_entity_id("E1")).tradeable_filter.kind
        'by_entity_id'
        """
        ...

    def with_candidate(self, candidate: CandidatePosition) -> TradeUniverse:
        """
        Return a copy with ``candidate`` appended.

        Parameters
        ----------
        candidate : CandidatePosition
            Instrument the optimizer may add.

        Returns
        -------
        TradeUniverse
            New universe; the receiver is unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    def allow_shorting_candidates(self) -> TradeUniverse:
        """
        Return a copy that lets candidates take negative weights.

        Returns
        -------
        TradeUniverse
            New universe with ``allow_short_candidates`` set.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.all_positions().allow_shorting_candidates().allow_short_candidates
        True
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> TradeUniverse:
        """
        Parse from JSON.

        Parameters
        ----------
        json_str : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        TradeUniverse
            Reconstructed universe.

        Raises
        ------
        ValueError
            If the payload or an embedded candidate instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.from_json("{}").candidates
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If a candidate instrument has no JSON representation.
        """
        ...

    @property
    def tradeable_filter(self) -> PositionFilter:
        """
        Filter selecting tradeable positions.

        Returns
        -------
        PositionFilter
            Filter selecting tradeable positions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def held_filter(self) -> PositionFilter | None:
        """
        Optional filter selecting positions whose weights and exact quantities
        stay fixed under every weighting scheme, including zero-PV holdings.
        Returns
        -------
        PositionFilter or None
            Filter defining held existing positions, or ``None`` if no explicit
            held filter is configured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def candidates(self) -> list[CandidatePosition]:
        """
        Candidate new positions.
        Returns
        -------
        list[CandidatePosition]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def allow_short_candidates(self) -> bool:
        """
        Whether candidates may receive negative weights.

        Returns
        -------
        bool
            Whether candidates may receive negative weights.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationSpec:
    """
    JSON-serializable portfolio optimization specification.

    Examples
    --------
    >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
    >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> PortfolioOptimizationSpec.new(spec_json, objective).objective.direction
    'maximize'
    """

    @classmethod
    def new(
        cls,
        portfolio: Portfolio | str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec:
        """
        Create an optimization specification from a portfolio and an objective.

        Parameters
        ----------
        portfolio : Portfolio | str
            Built :class:`Portfolio` (its canonical spec is captured) or a
            ``PortfolioSpec`` JSON string defining positions and starting
            weights.
        objective : Objective
            Direction and metric expression the optimizer should solve for.

        Returns
        -------
        PortfolioOptimizationSpec
            Spec with no constraints, value weighting, zero-fill missing
            metrics, no label and the default trade universe.

        Raises
        ------
        TypeError
            If ``portfolio`` is neither a ``Portfolio`` nor a string.
        ValueError
            If the JSON is malformed or does not match the ``PortfolioSpec``
            schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
        >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
        >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
        >>> PortfolioOptimizationSpec.new(spec_json, objective).weighting.label
        'value_weight'
        """
        ...

    def with_trade_universe(self, universe: TradeUniverse) -> PortfolioOptimizationSpec:
        """
        Return a copy restricted to ``universe``.

        Parameters
        ----------
        universe : TradeUniverse
            Tradeable/held filters and candidate additions the optimizer
            honours.

        Returns
        -------
        PortfolioOptimizationSpec
            New spec; the receiver is unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def trade_universe(self) -> TradeUniverse | None:
        """
        Trade universe restricting the optimizer.

        Returns
        -------
        TradeUniverse | None
            ``None`` means every position is tradeable and no candidates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec:
        """
        Return a copy with an additional constraint.

        Parameters
        ----------
        constraint : Constraint
            Constraint to append to the current optimization specification. An explicit
            budget replaces the implicit sum-of-weights budget of one when solving;
            ``unit_scaling`` weighting has no implicit budget and requires one.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with ``constraint`` appended after the existing constraints.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement objective.

        Parameters
        ----------
        objective : Objective
            New direction and metric expression that replaces the existing one.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing objective replaced by ``objective``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement weighting scheme.

        Parameters
        ----------
        weighting : WeightingScheme
            Market-value or notional convention used to calculate portfolio weights.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing weighting scheme replaced by ``weighting``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement missing-metric policy.

        Parameters
        ----------
        policy : MissingMetricPolicy
            Policy defining how positions without required metric data are handled.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing missing-metric policy replaced by ``policy``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_label(self, label: str) -> PortfolioOptimizationSpec:
        """
        Return a copy with a human-readable label.

        Parameters
        ----------
        label : str
            Stable reader-facing name for reports, diagnostics, and persistence.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with its audit and reporting label set to ``label``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec:
        """
        Deserialize an optimization specification from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio optimization specification, normally
            produced by ``PortfolioOptimizationSpec.to_json``.

        Returns
        -------
        PortfolioOptimizationSpec
            Validated `PortfolioOptimizationSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
        >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
        >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
        >>> original = PortfolioOptimizationSpec.new(spec_json, objective)
        >>> PortfolioOptimizationSpec.from_json(original.to_json()).objective.direction
        'maximize'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization specification to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def objective(self) -> Objective:
        """
        Objective maximized or minimized by this optimization spec.

        Returns
        -------
        Objective
            Objective maximized or minimized by this optimization spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def constraints(self) -> list[Constraint]:
        """
        Optimization constraints.

        Returns
        -------
        list[Constraint]
            Optimization constraints.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weighting(self) -> WeightingScheme:
        """
        Weighting scheme used by the optimization.

        Returns
        -------
        WeightingScheme
            Weighting scheme used by the optimization.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def missing_metric_policy(self) -> MissingMetricPolicy:
        """
        Policy for missing per-position metrics.
        Returns
        -------
        MissingMetricPolicy

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def label(self) -> str | None:
        """
        Optional human-readable label.

        Returns
        -------
            Optional human-readable label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def portfolio_spec_json(self) -> str:
        """
        Return the embedded portfolio specification JSON.
        Returns
        -------
        str
            Compact canonical JSON for the embedded ``PortfolioSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationResult:
    """
    Result of an optimization run.

    Round-trips through ``to_json`` / ``from_json`` and therefore supports
    ``pickle``, ``copy.deepcopy``, and ``multiprocessing``.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioOptimizationResult
    >>> try:
    ...     PortfolioOptimizationResult()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.PortfolioOptimizationResult' instances
    """

    @property
    def meta(self) -> dict[str, Any]:
        """Return Rust solver configuration and numerical provenance.

        Returns
        -------
        dict[str, Any]
            Canonical ResultsMeta fields, including numeric mode, rounding policy, and execution stamps.

        Raises
        ------
        ValueError
            If metadata cannot be converted to Python.
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> PortfolioOptimizationResult:
        """
        Rebuild a `PortfolioOptimizationResult` from ``to_json`` output.

        Parameters
        ----------
        json_str : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioOptimizationResult
            The reconstructed optimization result, field for field.

        Raises
        ------
        ValueError
            If ``json_str`` is malformed or does not match the wire schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioOptimizationResult
        >>> restored = PortfolioOptimizationResult.from_json(result.to_json())  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def status(self) -> OptimizationStatus:
        """
        Solver status after the portfolio optimization run.
        Returns
        -------
        OptimizationStatus

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """
        Whether the solver returned a feasible portfolio.
        Returns
        -------
        bool
            ``status.is_feasible``; derived from :attr:`status`, never stored.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def objective_value(self) -> float:
        """
        Objective value at the solution.

        Returns
        -------
        float
            Objective value at the solution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_weights(self) -> dict[str, float]:
        """
        Current weights by position ID.

        Returns
        -------
        dict[str, float]
            Current weights by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def optimal_weights(self) -> dict[str, float]:
        """
        Optimized target weights by position ID.

        Returns
        -------
        dict[str, float]
            Optimized target weights by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weight_deltas(self) -> dict[str, float]:
        """
        Target less current weight by position ID.

        Returns
        -------
        dict[str, float]
            Target less current weight by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def implied_quantities(self) -> dict[str, float]:
        """
        Implied target quantities by position ID.

        Returns
        -------
        dict[str, float]
            Quantities in each position's unit convention, including percentage
            points for ``percentage``. Held existing positions retain their exact
            current quantity under every weighting scheme, including zero-PV
            holdings.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def constraint_slacks(self) -> dict[str, float]:
        """
        Constraint slack values by constraint label.

        Returns
        -------
        dict[str, float]
            Constraint slack values by constraint label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def turnover(self) -> float:
        """
        Total turnover implied by the solution.

        Returns
        -------
        float
            Total turnover implied by the solution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_trade_list(self) -> list[TradeSpec]:
        """
        Convert weight deltas into trade specifications.
        Returns
        -------
        list[TradeSpec]
            Material trades sorted by descending absolute quantity change.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def new_position_trades(self) -> list[TradeSpec]:
        """
        Return trade specs for new candidate positions only.
        Returns
        -------
        list[TradeSpec]
            New-candidate subset of ``to_trade_list()``, preserving its order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position weight and quantity solution, indexed by position.

        ``current_weights``, ``optimal_weights``, ``weight_deltas`` and
        ``implied_quantities`` share one position key space, so they are
        joined into a single frame. The row axis is the union of their keys,
        ordered by first appearance (``current_weights`` first); a value
        missing from one map becomes ``None`` in that column — this is how
        candidate positions with no current weight show up.

        Columns: ``current_weight``, ``optimal_weight``, ``weight_delta`` (all
        fractions of the portfolio, not percentages), ``implied_quantity``
        (units / face / notional per the weighting scheme).

        Returns
        -------
        pd.DataFrame
            One row per position; the index holds the position ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_trade_dataframe(self) -> pd.DataFrame:
        """
        Export :meth:`to_trade_list` as a pandas DataFrame.

        One row per trade, in the same order as :meth:`to_trade_list` (sorted
        by absolute quantity delta, largest first).

        Columns: ``position_id``, ``instrument_id``, ``trade_type``
        (``"existing"``, ``"new_position"``, ``"close_out"``),
        ``current_quantity``, ``target_quantity``, ``delta_quantity``,
        ``direction`` (``"buy"``, ``"sell"``, ``"hold"``), ``current_weight``,
        ``target_weight`` (weights are fractions, not percentages).

        Returns
        -------
        pd.DataFrame
            One row per trade; a solution that requires no trades yields a
            zero-row frame that still carries the column schema.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def binding_constraints(self) -> list[tuple[str, float]]:
        """
        Return constraints with near-zero slack as ``(label, slack)`` pairs.

        Reads the stored ``binding_constraints`` names of the JSON wire form and
        looks each slack up in :attr:`constraint_slacks`.

        Returns
        -------
        list[tuple[str, float]]
            Binding constraints in slack order; empty for an infeasible solve.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...
