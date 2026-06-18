"""Scenario specification, validation, composition, application, and built-in templates."""

from __future__ import annotations

from typing import Any

from finstack_quant.attribution import PnlAttribution

__all__ = [
    "parse_scenario_spec",
    "build_scenario_spec",
    "compose_scenarios",
    "validate_scenario_spec",
    "list_builtin_templates",
    "list_builtin_template_metadata",
    "build_from_template",
    "list_template_components",
    "build_template_component",
    "apply_scenario",
    "apply_scenario_to_market",
    "compute_horizon_return",
    "HorizonResult",
    "OperationSpec",
    "RateBindingSpec",
    "CurveKind",
    "VolSurfaceKind",
    "TenorMatchMode",
    "TimeRollMode",
    "Compounding",
]

def parse_scenario_spec(json_str: str) -> str:
    """Parse, validate, and re-serialize a ``ScenarioSpec`` from JSON.

    Args:
        json_str: JSON-serialized ``ScenarioSpec``.

    Returns:
        Validated canonical JSON string.

    Example:
        >>> from finstack_quant.scenarios import parse_scenario_spec
        >>> parse_scenario_spec(spec_json)  # doctest: +SKIP
        ''
    """
    ...

def build_scenario_spec(
    id: str,
    operations_json: str,
    name: str | None = None,
    description: str | None = None,
    priority: int = 0,
) -> str:
    """Construct a ``ScenarioSpec`` from fields plus a JSON operations list.

    Args:
        id: Stable scenario identifier.
        operations_json: JSON list of ``OperationSpec``.
        name: Optional display name.
        description: Optional long description.
        priority: Composition priority (lower runs first). Defaults to ``0``.

    Returns:
        Validated JSON ``ScenarioSpec``.

    Example:
        >>> from finstack_quant.scenarios import build_scenario_spec
        >>> build_scenario_spec("s1", "[]")  # doctest: +SKIP
        ''
    """
    ...

def compose_scenarios(specs_json: str) -> str:
    """Merge multiple scenario specs using the scenario engine composer.

    Args:
        specs_json: JSON list of ``ScenarioSpec``.

    Returns:
        JSON-serialized composed ``ScenarioSpec``.

    Example:
        >>> from finstack_quant.scenarios import compose_scenarios
        >>> compose_scenarios("[]")  # doctest: +SKIP
        ''
    """
    ...

def validate_scenario_spec(json_str: str) -> bool:
    """Return ``True`` after successfully parsing and validating JSON.

    Args:
        json_str: JSON-serialized ``ScenarioSpec``.

    Returns:
        Always ``True`` on success.

    Example:
        >>> from finstack_quant.scenarios import validate_scenario_spec
        >>> validate_scenario_spec(spec_json)  # doctest: +SKIP
        True
    """
    ...

def list_builtin_templates() -> list[str]:
    """List template IDs from the embedded built-in registry.

    Args:
        (none)

    Returns:
        Template identifier strings.

    Example:
        >>> from finstack_quant.scenarios import list_builtin_templates
        >>> isinstance(list_builtin_templates(), list)
        True
    """
    ...

def list_builtin_template_metadata() -> str:
    """Serialize metadata for all built-in templates to JSON.

    Args:
        (none)

    Returns:
        JSON list of ``TemplateMetadata`` objects.

    Example:
        >>> from finstack_quant.scenarios import list_builtin_template_metadata
        >>> meta_json = list_builtin_template_metadata()
    """
    ...

def build_from_template(template_id: str) -> str:
    """Instantiate a ``ScenarioSpec`` from a built-in template.

    Args:
        template_id: Registry key for the template.

    Returns:
        JSON-serialized ``ScenarioSpec``.

    Example:
        >>> from finstack_quant.scenarios import build_from_template
        >>> build_from_template("unknown")  # doctest: +SKIP
        ''
    """
    ...

def list_template_components(template_id: str) -> list[str]:
    """List sub-component IDs for composite templates.

    Args:
        template_id: Parent template identifier.

    Returns:
        Component identifiers.

    Example:
        >>> from finstack_quant.scenarios import list_template_components
        >>> list_template_components("t")  # doctest: +SKIP
        []
    """
    ...

def build_template_component(template_id: str, component_id: str) -> str:
    """Build a single component spec from a composite template.

    Args:
        template_id: Parent template identifier.
        component_id: Component key inside the template.

    Returns:
        JSON-serialized component ``ScenarioSpec``.

    Example:
        >>> from finstack_quant.scenarios import build_template_component
        >>> build_template_component("t", "c")  # doctest: +SKIP
        ''
    """
    ...

def apply_scenario(
    scenario_json: str,
    market: Any,
    model: Any,
    as_of: str,
) -> dict[str, Any]:
    """Apply a scenario to both market data and a financial model.

    Args:
        scenario_json: JSON ``ScenarioSpec``.
        market: ``MarketContext`` object or JSON ``MarketContext`` string.
        model: ``FinancialModelSpec`` object or JSON ``FinancialModelSpec`` string.
        as_of: ISO 8601 valuation date.

    Returns:
        Dict with ``market_json``, ``model_json``, ``operations_applied`` (``int``),
        ``user_operations`` (``int``), ``expanded_operations`` (``int``),
        ``warnings`` (``list[str]``, rendered Display form), and
        ``warnings_json`` (``str``, JSON-encoded list of structured ``Warning``
        records — parse with ``json.loads(...)`` for programmatic
        ``kind``-based dispatch).

    Example:
        >>> from finstack_quant.scenarios import apply_scenario
        >>> apply_scenario(sj, mj, fj, "2025-01-15")  # doctest: +SKIP
        {}
    """
    ...

def apply_scenario_to_market(
    scenario_json: str,
    market: Any,
    as_of: str,
) -> dict[str, Any]:
    """Apply a scenario to market data only (no model mutations returned).

    Args:
        scenario_json: JSON ``ScenarioSpec``.
        market: ``MarketContext`` object or JSON ``MarketContext`` string.
        as_of: ISO 8601 valuation date.

    Returns:
        Dict with ``market_json``, ``operations_applied``, ``user_operations``,
        ``expanded_operations``, ``warnings`` (``list[str]``), and
        ``warnings_json`` (``str``, JSON-encoded list of structured warnings).

    Example:
        >>> from finstack_quant.scenarios import apply_scenario_to_market
        >>> apply_scenario_to_market(sj, mj, "2025-01-15")  # doctest: +SKIP
        {}
    """
    ...

class HorizonResult:
    """Horizon total return result with full P&L attribution."""

    @property
    def attribution(self) -> PnlAttribution:
        """Full P&L attribution breakdown.
        Returns
        -------
        PnlAttribution
        """
        ...

    @property
    def initial_value(self) -> float:
        """Initial instrument value.
        Returns
        -------
        float
        """
        ...

    @property
    def terminal_value(self) -> float:
        """Final instrument value after the scenario is applied.
        Returns
        -------
        float
        """
        ...

    @property
    def horizon_days(self) -> int | None:
        """Horizon in calendar days (``None`` if no time-roll).
        Returns
        -------
        int or None
        """
        ...

    @property
    def total_return_pct(self) -> float:
        """Total return as decimal fraction (0.05 = 5%).
        Returns
        -------
        float
        """
        ...

    @property
    def annualized_return(self) -> float | None:
        """Annualized return (``None`` if no time-roll).
        Returns
        -------
        float or None
        """
        ...

    @property
    def operations_applied(self) -> int:
        """Number of scenario operations applied.
        Returns
        -------
        int
        """
        ...

    @property
    def user_operations(self) -> int:
        """Number of user-provided scenario operations before hierarchy expansion.
        Returns
        -------
        int
        """
        ...

    @property
    def expanded_operations(self) -> int:
        """Number of direct operations after hierarchy expansion and deduplication.
        Returns
        -------
        int
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """Warnings emitted during scenario application (rendered Display form).
        Returns
        -------
        list[str]
        """
        ...

    @property
    def warnings_json(self) -> str:
        """JSON-encoded structured warnings.

        Each entry is a `Warning` record with a ``kind`` discriminator plus
        variant-specific fields, mirroring the WASM binding. Parse with
        ``json.loads(...)`` to dispatch on ``kind`` programmatically.

        Returns
        -------
        str
        """
        ...

    def factor_contribution(self, factor: str) -> float:
        """Factor contribution as decimal fraction of initial value.

        Args:
            factor: One of ``"carry"``, ``"rates"``/``"rates_curves"``,
                ``"credit"``/``"credit_curves"``, ``"inflation"``/``"inflation_curves"``,
                ``"correlations"``, ``"fx"``, ``"volatility"``/``"vol"``,
                ``"model_parameters"``/``"model_params"``, or
                ``"market_scalars"``/``"scalars"``.

        Returns:
            Contribution of the given factor as a decimal fraction.
        """
        ...

    def to_json(self) -> str:
        """Serialize the result to JSON.
        Returns
        -------
        str
        """
        ...

    def explain(self) -> str:
        """Human-readable summary of horizon return and attribution.
        Returns
        -------
        str
        """
        ...

def compute_horizon_return(
    instrument_json: str,
    market: Any,
    as_of: str,
    scenario_json: str,
    method: str = "parallel",
    config: str | None = None,
) -> HorizonResult:
    """Compute horizon total return under a scenario.

    Args:
        instrument_json: JSON-serialized instrument (tagged ``{"type": ..., "spec": {...}}``).
        market: ``MarketContext`` object or JSON string.
        as_of: Valuation date in ISO 8601 format.
        scenario_json: JSON-serialized ``ScenarioSpec``.
        method: Attribution method — ``"parallel"`` (default), ``"waterfall"``,
            ``"metrics_based"``, or ``"taylor"``.
        config: Optional JSON-serialized ``FinstackConfig``.

    Returns:
        ``HorizonResult`` with decomposed total return and factor attribution.
    """
    ...

# ---------------------------------------------------------------------------
# Typed operation builders
#
# These mirror the Rust ``OperationSpec`` enum and its supporting enums. They
# replace the raw-JSON authoring path so quants can write
# ``OperationSpec.curve_parallel_bp(...)`` and feed the result straight into
# ``build_scenario_spec`` via ``op.to_json()``.
# ---------------------------------------------------------------------------

class CurveKind:
    """Type of market curve targeted by a scenario operation."""

    @classmethod
    def discount(cls) -> CurveKind:
        """Discount factor curve."""
        ...

    @classmethod
    def forward(cls) -> CurveKind:
        """Forward rate curve."""
        ...

    @classmethod
    def par_cds(cls) -> CurveKind:
        """Par CDS spread curve."""
        ...

    @classmethod
    def inflation(cls) -> CurveKind:
        """Inflation index curve."""
        ...

    @classmethod
    def commodity(cls) -> CurveKind:
        """Commodity forward curve."""
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"Discount"``.
        Returns
        -------
        str
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.
        Returns
        -------
        str
        """
        ...

class VolSurfaceKind:
    """Category of volatility surface targeted by a scenario operation."""

    @classmethod
    def equity(cls) -> VolSurfaceKind: ...
    @classmethod
    def credit(cls) -> VolSurfaceKind: ...
    @classmethod
    def swaption(cls) -> VolSurfaceKind: ...
    @property
    def name(self) -> str: ...
    @property
    def value(self) -> str: ...

class TenorMatchMode:
    """Tenor-pillar alignment strategy for curve-node operations."""

    @classmethod
    def exact(cls) -> TenorMatchMode: ...
    @classmethod
    def interpolate(cls) -> TenorMatchMode: ...
    @property
    def name(self) -> str: ...
    @property
    def value(self) -> str: ...

class TimeRollMode:
    """Calendar-vs-business-day semantics for time-roll operations."""

    @classmethod
    def business_days(cls) -> TimeRollMode: ...
    @classmethod
    def calendar_days(cls) -> TimeRollMode: ...
    @classmethod
    def approximate(cls) -> TimeRollMode: ...
    @property
    def name(self) -> str: ...
    @property
    def value(self) -> str: ...

class Compounding:
    """Compounding convention for rate-extraction operations."""

    @classmethod
    def simple(cls) -> Compounding: ...
    @classmethod
    def continuous(cls) -> Compounding: ...
    @classmethod
    def annual(cls) -> Compounding: ...
    @classmethod
    def semi_annual(cls) -> Compounding: ...
    @classmethod
    def quarterly(cls) -> Compounding: ...
    @classmethod
    def monthly(cls) -> Compounding: ...
    @property
    def name(self) -> str: ...
    @property
    def value(self) -> str: ...

class RateBindingSpec:
    """Configuration linking a statement rate node to a market curve."""

    def __init__(
        self,
        node_id: str,
        curve_id: str,
        tenor: str,
        compounding: Compounding | None = None,
        day_count: str | None = None,
    ) -> None: ...
    @property
    def node_id(self) -> str: ...
    @property
    def curve_id(self) -> str: ...
    @property
    def tenor(self) -> str: ...
    @property
    def compounding(self) -> Compounding: ...
    @property
    def day_count(self) -> str | None: ...
    def to_json(self) -> str:
        """Serialize to JSON.
        Returns
        -------
        str
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> RateBindingSpec:
        """Deserialize a ``RateBindingSpec`` from JSON."""
        ...

class OperationSpec:
    """Typed builder for ``finstack_quant_scenarios::OperationSpec``.

    Each classmethod corresponds to one Rust enum variant; ``to_json()``
    produces the canonical wire form expected by ``build_scenario_spec`` and
    the scenario engine.
    """

    @classmethod
    def market_fx_pct(cls, base: str, quote: str, pct: float) -> OperationSpec:
        """FX rate percent shift (``pct = 5.0`` strengthens ``base`` by 5%)."""
        ...

    @classmethod
    def equity_price_pct(cls, ids: list[str], pct: float) -> OperationSpec:
        """Equity price percent shock applied to all supplied identifiers."""
        ...

    @classmethod
    def instrument_price_pct_by_attr(cls, attrs: list[tuple[str, str]], pct: float) -> OperationSpec:
        """Instrument price shock by exact attribute match.

        ``attrs`` is a list of ``(key, value)`` pairs preserving order.
        """
        ...

    @classmethod
    def curve_parallel_bp(
        cls,
        curve_kind: CurveKind,
        curve_id: str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """Parallel basis-point shift on a rate-style curve."""
        ...

    @classmethod
    def curve_node_bp(
        cls,
        curve_kind: CurveKind,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | None = None,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """Node-level basis-point shifts on a rate-style curve."""
        ...

    @classmethod
    def vol_index_parallel_pts(cls, curve_id: str, points: float) -> OperationSpec:
        """Parallel shock to a volatility-index curve in absolute index points."""
        ...

    @classmethod
    def vol_index_node_pts(
        cls,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | None = None,
    ) -> OperationSpec:
        """Node-level shocks to a volatility-index curve in absolute index points."""
        ...

    @classmethod
    def base_corr_parallel_pts(cls, surface_id: str, points: float) -> OperationSpec:
        """Parallel base-correlation shift (absolute correlation points)."""
        ...

    @classmethod
    def base_corr_bucket_pts(
        cls,
        surface_id: str,
        points: float,
        detachment_bps: list[int] | None = None,
        maturities: list[str] | None = None,
    ) -> OperationSpec:
        """Bucketed base-correlation shock by detachment and (reserved) maturity."""
        ...

    @classmethod
    def vol_surface_parallel_pct(cls, surface_kind: VolSurfaceKind, surface_id: str, pct: float) -> OperationSpec:
        """Parallel percent shift to a volatility surface."""
        ...

    @classmethod
    def vol_surface_bucket_pct(
        cls,
        surface_kind: VolSurfaceKind,
        surface_id: str,
        pct: float,
        tenors: list[str] | None = None,
        strikes: list[float] | None = None,
    ) -> OperationSpec:
        """Bucketed volatility surface percent shock."""
        ...

    @classmethod
    def stmt_forecast_percent(cls, node_id: str, pct: float) -> OperationSpec:
        """Statement forecast percent change."""
        ...

    @classmethod
    def stmt_forecast_assign(cls, node_id: str, value: float) -> OperationSpec:
        """Statement forecast value assignment."""
        ...

    @classmethod
    def rate_binding(cls, binding: RateBindingSpec) -> OperationSpec:
        """Bind a statement rate node to a curve for the lifetime of the scenario."""
        ...

    @classmethod
    def instrument_spread_bp_by_attr(cls, attrs: list[tuple[str, str]], bp: float) -> OperationSpec:
        """Instrument spread shock (basis points) by exact attribute match."""
        ...

    @classmethod
    def instrument_price_pct_by_type(cls, instrument_types: list[str], pct: float) -> OperationSpec:
        """Instrument price shock by ``InstrumentType`` (snake_case strings)."""
        ...

    @classmethod
    def instrument_spread_bp_by_type(cls, instrument_types: list[str], bp: float) -> OperationSpec:
        """Instrument spread shock by ``InstrumentType`` (snake_case strings)."""
        ...

    @classmethod
    def asset_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """Asset-correlation shock for structured credit."""
        ...

    @classmethod
    def prepay_default_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """Prepay-default correlation shock for structured credit."""
        ...

    @classmethod
    def hierarchy_curve_parallel_bp(
        cls,
        curve_kind: CurveKind,
        target_json: str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """Hierarchy-targeted parallel curve shift.

        ``target_json`` is a JSON-serialized ``HierarchyTarget``
        (``{"path": [...], "tag_filter": {...}}``).
        """
        ...

    @classmethod
    def hierarchy_vol_surface_parallel_pct(
        cls, surface_kind: VolSurfaceKind, target_json: str, pct: float
    ) -> OperationSpec:
        """Hierarchy-targeted vol-surface percent shift."""
        ...

    @classmethod
    def hierarchy_equity_price_pct(cls, target_json: str, pct: float) -> OperationSpec:
        """Hierarchy-targeted equity price shift."""
        ...

    @classmethod
    def hierarchy_base_corr_parallel_pts(cls, target_json: str, points: float) -> OperationSpec:
        """Hierarchy-targeted base-correlation parallel shift."""
        ...

    @classmethod
    def time_roll_forward(
        cls,
        period: str,
        apply_shocks: bool = True,
        roll_mode: TimeRollMode | None = None,
    ) -> OperationSpec:
        """Roll the valuation horizon forward (e.g. ``"1M"``).

        ``apply_shocks`` defaults to ``True`` to mirror the Rust
        ``#[serde(default = "default_true")]`` attribute.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.
        Returns
        -------
        str
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> OperationSpec:
        """Deserialize an ``OperationSpec`` from JSON."""
        ...

    @property
    def kind(self) -> str:
        """Variant discriminator (the serde ``kind`` tag value).
        Returns
        -------
        str
        """
        ...
