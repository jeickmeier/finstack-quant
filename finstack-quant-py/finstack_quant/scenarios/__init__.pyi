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

    Parameters
    ----------
    json_str : str
        JSON-serialized ``ScenarioSpec``.

    Returns
    -------
    str
        Validated canonical JSON string.

    Raises
    ------
    ValueError
        If the JSON is malformed or fails scenario-spec validation.

    Examples
    --------
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

    Parameters
    ----------
    id : str
        Stable scenario identifier.
    operations_json : str
        JSON list of ``OperationSpec``.
    name : str, optional
        Display name.
    description : str, optional
        Long description.
    priority : int, default 0
        Composition priority (lower runs first).

    Returns
    -------
    str
        Validated JSON ``ScenarioSpec``.

    Raises
    ------
    ValueError
        If ``operations_json`` is not valid JSON or fails validation.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_scenario_spec
    >>> build_scenario_spec("s1", "[]")  # doctest: +SKIP
        ''
    """
    ...

def compose_scenarios(specs_json: str) -> str:
    """Merge multiple scenario specs using the scenario engine composer.

    Parameters
    ----------
    specs_json : str
        JSON list of ``ScenarioSpec``.

    Returns
    -------
    str
        JSON-serialized composed ``ScenarioSpec``.

    Raises
    ------
    ValueError
        If ``specs_json`` is not valid JSON or composition fails.

    Examples
    --------
    >>> from finstack_quant.scenarios import compose_scenarios
    >>> compose_scenarios("[]")  # doctest: +SKIP
        ''
    """
    ...

def validate_scenario_spec(json_str: str) -> bool:
    """Return ``True`` after successfully parsing and validating JSON.

    Parameters
    ----------
    json_str : str
        JSON-serialized ``ScenarioSpec``.

    Returns
    -------
    bool
        Always ``True`` on success.

    Raises
    ------
    ValueError
        If ``json_str`` is not valid JSON or fails validation.

    Examples
    --------
    >>> from finstack_quant.scenarios import validate_scenario_spec
    >>> validate_scenario_spec(spec_json)  # doctest: +SKIP
        True
    """
    ...

def list_builtin_templates() -> list[str]:
    """List template IDs from the embedded built-in registry.

    Returns
    -------
    list[str]
        Template identifier strings.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_templates
    >>> isinstance(list_builtin_templates(), list)
        True
    """
    ...

def list_builtin_template_metadata() -> str:
    """Serialize metadata for all built-in templates to JSON.

    Returns
    -------
    str
        JSON list of ``TemplateMetadata`` objects.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_template_metadata
    >>> meta_json = list_builtin_template_metadata()
    """
    ...

def build_from_template(template_id: str) -> str:
    """Instantiate a ``ScenarioSpec`` from a built-in template.

    Parameters
    ----------
    template_id : str
        Registry key for the template.

    Returns
    -------
    str
        JSON-serialized ``ScenarioSpec``.

    Raises
    ------
    ValueError
        If ``template_id`` is not found in the registry.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_from_template
    >>> build_from_template("unknown")  # doctest: +SKIP
        ''
    """
    ...

def list_template_components(template_id: str) -> list[str]:
    """List sub-component IDs for composite templates.

    Parameters
    ----------
    template_id : str
        Parent template identifier.

    Returns
    -------
    list[str]
        Component identifiers.

    Raises
    ------
    ValueError
        If ``template_id`` is not found in the registry.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_template_components
    >>> list_template_components("t")  # doctest: +SKIP
        []
    """
    ...

def build_template_component(template_id: str, component_id: str) -> str:
    """Build a single component spec from a composite template.

    Parameters
    ----------
    template_id : str
        Parent template identifier.
    component_id : str
        Component key inside the template.

    Returns
    -------
    str
        JSON-serialized component ``ScenarioSpec``.

    Raises
    ------
    ValueError
        If ``template_id`` or ``component_id`` is not found.

    Examples
    --------
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

    Parameters
    ----------
    scenario_json : str
        JSON ``ScenarioSpec``.
    market : Any
        ``MarketContext`` object or JSON ``MarketContext`` string.
    model : Any
        ``FinancialModelSpec`` object or JSON ``FinancialModelSpec`` string.
    as_of : str
        ISO 8601 valuation date.

    Returns
    -------
    dict[str, Any]
        Dict with ``market_json``, ``model_json``, ``operations_applied`` (``int``),
        ``user_operations`` (``int``), ``expanded_operations`` (``int``),
        ``warnings`` (``list[str]``, rendered Display form), and
        ``warnings_json`` (``str``, JSON-encoded list of structured ``Warning``
        records — parse with ``json.loads(...)`` for programmatic
        ``kind``-based dispatch).

    Raises
    ------
    ValueError
        If the scenario JSON is malformed or application fails.

    Examples
    --------
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

    Parameters
    ----------
    scenario_json : str
        JSON ``ScenarioSpec``.
    market : Any
        ``MarketContext`` object or JSON ``MarketContext`` string.
    as_of : str
        ISO 8601 valuation date.

    Returns
    -------
    dict[str, Any]
        Dict with ``market_json``, ``operations_applied``, ``user_operations``,
        ``expanded_operations``, ``warnings`` (``list[str]``), and
        ``warnings_json`` (``str``, JSON-encoded list of structured warnings).

    Raises
    ------
    ValueError
        If the scenario JSON is malformed or application fails.

    Examples
    --------
    >>> from finstack_quant.scenarios import apply_scenario_to_market
    >>> apply_scenario_to_market(sj, mj, "2025-01-15")  # doctest: +SKIP
        {}
    """
    ...

class HorizonResult:
    """Horizon total return result with full P&L attribution.

    Produced by :func:`compute_horizon_return`. Access factor-level
    contributions via :meth:`factor_contribution` and the full breakdown
    via :attr:`attribution`.

    Examples
    --------
    >>> from finstack_quant.scenarios import compute_horizon_return
    >>> result = compute_horizon_return(inst, mkt, "2025-01-15", scen)  # doctest: +SKIP
    >>> result.total_return_pct  # doctest: +SKIP
    0.02
    """

    @property
    def attribution(self) -> PnlAttribution:
        """Full P&L attribution breakdown.

        Returns
        -------
        PnlAttribution
            Carry, rate, credit, inflation, FX, volatility, and model-parameter
            contributions.

        Examples
        --------
        >>> result.attribution  # doctest: +SKIP
        PnlAttribution(...)
        """
        ...

    @property
    def initial_value(self) -> float:
        """Initial instrument value.

        Returns
        -------
        float
            Present value at the original valuation date.
        """
        ...

    @property
    def terminal_value(self) -> float:
        """Final instrument value after the scenario is applied.

        Returns
        -------
        float
            Present value after scenario shocks and time roll.
        """
        ...

    @property
    def horizon_days(self) -> int | None:
        """Horizon in calendar days (``None`` if no time-roll).

        Returns
        -------
        int or None
            Number of days rolled forward, or ``None`` when the scenario
            contains no ``time_roll_forward`` operation.
        """
        ...

    @property
    def total_return_pct(self) -> float:
        """Total return as decimal fraction (0.05 = 5%).

        Returns
        -------
        float
            ``(terminal_value - initial_value) / initial_value``.
        """
        ...

    @property
    def annualized_return(self) -> float | None:
        """Annualized return (``None`` if no time-roll).

        Returns
        -------
        float or None
            Annualized total return, or ``None`` when ``horizon_days`` is
            ``None`` or zero.
        """
        ...

    @property
    def operations_applied(self) -> int:
        """Number of scenario operations applied.

        Returns
        -------
        int
            Count of operations executed after hierarchy expansion.
        """
        ...

    @property
    def user_operations(self) -> int:
        """Number of user-provided scenario operations before hierarchy expansion.

        Returns
        -------
        int
            Count of operations in the original ``ScenarioSpec``.
        """
        ...

    @property
    def expanded_operations(self) -> int:
        """Number of direct operations after hierarchy expansion and deduplication.

        Returns
        -------
        int
            Count of unique operations after template hierarchy expansion.
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """Warnings emitted during scenario application (rendered Display form).

        Returns
        -------
        list[str]
            Human-readable warning strings.
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
            JSON array of structured warning objects.
        """
        ...

    def factor_contribution(self, factor: str) -> float:
        """Factor contribution as decimal fraction of initial value.

        Parameters
        ----------
        factor : str
            One of ``"carry"``, ``"rates"``/``"rates_curves"``,
            ``"credit"``/``"credit_curves"``, ``"inflation"``/``"inflation_curves"``,
            ``"correlations"``, ``"fx"``, ``"volatility"``/``"vol"``,
            ``"model_parameters"``/``"model_params"``, or
            ``"market_scalars"``/``"scalars"``.

        Returns
        -------
        float
            Contribution of the given factor as a decimal fraction.

        Raises
        ------
        ValueError
            If ``factor`` is not a recognized factor key.
        """
        ...

    def to_json(self) -> str:
        """Serialize the result to JSON.

        Returns
        -------
        str
            JSON-serialized ``HorizonResult`` envelope.
        """
        ...

    def explain(self) -> str:
        """Human-readable summary of horizon return and attribution.

        Returns
        -------
        str
            Multi-line text suitable for notebook display.
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

    Parameters
    ----------
    instrument_json : str
        JSON-serialized instrument (tagged ``{"type": ..., "spec": {...}}``).
    market : Any
        ``MarketContext`` object or JSON string.
    as_of : str
        Valuation date in ISO 8601 format.
    scenario_json : str
        JSON-serialized ``ScenarioSpec``.
    method : str, default "parallel"
        Attribution method — ``"parallel"``, ``"waterfall"``,
        ``"metrics_based"``, or ``"taylor"``.
    config : str, optional
        JSON-serialized ``FinstackConfig``.

    Returns
    -------
    HorizonResult
        Decomposed total return and factor attribution.

    Raises
    ------
    ValueError
        If any input JSON is malformed or the scenario application fails.

    Examples
    --------
    >>> from finstack_quant.scenarios import compute_horizon_return
    >>> result = compute_horizon_return(inst, mkt, "2025-01-15", scen)  # doctest: +SKIP
    >>> result.total_return_pct  # doctest: +SKIP
    0.02
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
    """Type of market curve targeted by a scenario operation.

    Examples
    --------
    >>> from finstack_quant.scenarios import CurveKind
    >>> CurveKind.discount().value
    'discount'
    """

    @classmethod
    def discount(cls) -> CurveKind:
        """Discount factor curve.

        Returns
        -------
        CurveKind
            The ``discount`` variant.
        """
        ...

    @classmethod
    def forward(cls) -> CurveKind:
        """Forward rate curve.

        Returns
        -------
        CurveKind
            The ``forward`` variant.
        """
        ...

    @classmethod
    def par_cds(cls) -> CurveKind:
        """Par CDS spread curve.

        Returns
        -------
        CurveKind
            The ``par_cds`` variant.
        """
        ...

    @classmethod
    def inflation(cls) -> CurveKind:
        """Inflation index curve.

        Returns
        -------
        CurveKind
            The ``inflation`` variant.
        """
        ...

    @classmethod
    def commodity(cls) -> CurveKind:
        """Commodity forward curve.

        Returns
        -------
        CurveKind
            The ``commodity`` variant.
        """
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"Discount"``.

        Returns
        -------
        str
            Pascal-case variant name.
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.
        """
        ...

class VolSurfaceKind:
    """Category of volatility surface targeted by a scenario operation.

    Examples
    --------
    >>> from finstack_quant.scenarios import VolSurfaceKind
    >>> VolSurfaceKind.equity().value
    'equity'
    """

    @classmethod
    def equity(cls) -> VolSurfaceKind:
        """Equity volatility surface.

        Returns
        -------
        VolSurfaceKind
            The ``equity`` variant.
        """
        ...

    @classmethod
    def credit(cls) -> VolSurfaceKind:
        """Credit volatility surface.

        Returns
        -------
        VolSurfaceKind
            The ``credit`` variant.
        """
        ...

    @classmethod
    def swaption(cls) -> VolSurfaceKind:
        """Swaption volatility surface.

        Returns
        -------
        VolSurfaceKind
            The ``swaption`` variant.
        """
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"Equity"``.

        Returns
        -------
        str
            Pascal-case variant name.
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"equity"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.
        """
        ...

class TenorMatchMode:
    """Tenor-pillar alignment strategy for curve-node operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TenorMatchMode
    >>> TenorMatchMode.exact().value
    'exact'
    """

    @classmethod
    def exact(cls) -> TenorMatchMode:
        """Match curve nodes by exact tenor string.

        Returns
        -------
        TenorMatchMode
            The ``exact`` variant.
        """
        ...

    @classmethod
    def interpolate(cls) -> TenorMatchMode:
        """Interpolate between adjacent curve nodes when tenor is not exact.

        Returns
        -------
        TenorMatchMode
            The ``interpolate`` variant.
        """
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"Exact"``.

        Returns
        -------
        str
            Pascal-case variant name.
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"exact"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.
        """
        ...

class TimeRollMode:
    """Calendar-vs-business-day semantics for time-roll operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TimeRollMode
    >>> TimeRollMode.calendar_days().value
    'calendar_days'
    """

    @classmethod
    def business_days(cls) -> TimeRollMode:
        """Roll by business days using the market calendar.

        Returns
        -------
        TimeRollMode
            The ``business_days`` variant.
        """
        ...

    @classmethod
    def calendar_days(cls) -> TimeRollMode:
        """Roll by calendar days (no holiday adjustment).

        Returns
        -------
        TimeRollMode
            The ``calendar_days`` variant.
        """
        ...

    @classmethod
    def approximate(cls) -> TimeRollMode:
        """Approximate roll (e.g. 30/360 day count).

        Returns
        -------
        TimeRollMode
            The ``approximate`` variant.
        """
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"CalendarDays"``.

        Returns
        -------
        str
            Pascal-case variant name.
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"calendar_days"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.
        """
        ...

class Compounding:
    """Compounding convention for rate-extraction operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import Compounding
    >>> Compounding.continuous().value
    'continuous'
    """

    @classmethod
    def simple(cls) -> Compounding:
        """Simple (zero-rate) compounding.

        Returns
        -------
        Compounding
            The ``simple`` variant.
        """
        ...

    @classmethod
    def continuous(cls) -> Compounding:
        """Continuously compounded rate.

        Returns
        -------
        Compounding
            The ``continuous`` variant.
        """
        ...

    @classmethod
    def annual(cls) -> Compounding:
        """Annual compounding.

        Returns
        -------
        Compounding
            The ``annual`` variant.
        """
        ...

    @classmethod
    def semi_annual(cls) -> Compounding:
        """Semi-annual compounding.

        Returns
        -------
        Compounding
            The ``semi_annual`` variant.
        """
        ...

    @classmethod
    def quarterly(cls) -> Compounding:
        """Quarterly compounding.

        Returns
        -------
        Compounding
            The ``quarterly`` variant.
        """
        ...

    @classmethod
    def monthly(cls) -> Compounding:
        """Monthly compounding.

        Returns
        -------
        Compounding
            The ``monthly`` variant.
        """
        ...

    @property
    def name(self) -> str:
        """Variant name, e.g. ``"Continuous"``.

        Returns
        -------
        str
            Pascal-case variant name.
        """
        ...

    @property
    def value(self) -> str:
        """Serialized wire value, e.g. ``"continuous"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.
        """
        ...

class RateBindingSpec:
    """Configuration linking a statement rate node to a market curve.

    Examples
    --------
    >>> from finstack_quant.scenarios import RateBindingSpec, Compounding
    >>> spec = RateBindingSpec("node_1", "USD-OIS", "5Y", Compounding.continuous())
    >>> spec.to_json()  # doctest: +SKIP
        '{"node_id":"node_1",...}'
    """

    def __init__(
        self,
        node_id: str,
        curve_id: str,
        tenor: str,
        compounding: Compounding | None = None,
        day_count: str | None = None,
    ) -> None:
        """Create a rate binding specification.

        Parameters
        ----------
        node_id : str
            Statement rate node identifier.
        curve_id : str
            Market curve identifier (e.g. ``"USD-OIS"``).
        tenor : str
            Tenor string (e.g. ``"5Y"``).
        compounding : Compounding, optional
            Compounding convention. Defaults to ``None`` (use curve default).
        day_count : str, optional
            Day-count convention string. Defaults to ``None`` (use curve default).

        Raises
        ------
        ValueError
            If required fields are empty or invalid.
        """
        ...

    @property
    def node_id(self) -> str:
        """Statement rate node identifier.

        Returns
        -------
        str
            Node ID string.
        """
        ...

    @property
    def curve_id(self) -> str:
        """Market curve identifier.

        Returns
        -------
        str
            Curve ID string (e.g. ``"USD-OIS"``).
        """
        ...

    @property
    def tenor(self) -> str:
        """Tenor string.

        Returns
        -------
        str
            Tenor label (e.g. ``"5Y"``).
        """
        ...

    @property
    def compounding(self) -> Compounding:
        """Compounding convention.

        Returns
        -------
        Compounding
            Compounding enum value, or the curve default when not specified.
        """
        ...

    @property
    def day_count(self) -> str | None:
        """Day-count convention.

        Returns
        -------
        str or None
            Day-count string, or ``None`` when not specified.
        """
        ...

    def to_json(self) -> str:
        """Serialize to JSON.

        Returns
        -------
        str
            JSON-serialized ``RateBindingSpec``.
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> RateBindingSpec:
        """Deserialize a ``RateBindingSpec`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        RateBindingSpec
            Parsed rate binding specification.

        Raises
        ------
        ValueError
            If the JSON is malformed or fields are invalid.
        """
        ...

class OperationSpec:
    """Typed builder for ``finstack_quant_scenarios::OperationSpec``.

    Each classmethod corresponds to one Rust enum variant; ``to_json()``
    produces the canonical wire form expected by ``build_scenario_spec`` and
    the scenario engine.

    Examples
    --------
    >>> from finstack_quant.scenarios import OperationSpec, CurveKind
    >>> op = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0)
    >>> op.to_json()  # doctest: +SKIP
        '{"kind":"curve_parallel_bp",...}'
    """

    @classmethod
    def market_fx_pct(cls, base: str, quote: str, pct: float) -> OperationSpec:
        """FX rate percent shift (``pct = 5.0`` strengthens ``base`` by 5%).

        Parameters
        ----------
        base : str
            Base currency code.
        quote : str
            Quote currency code.
        pct : float
            Percent shift applied to the FX rate.

        Returns
        -------
        OperationSpec
            The ``market_fx_pct`` operation.
        """
        ...

    @classmethod
    def equity_price_pct(cls, ids: list[str], pct: float) -> OperationSpec:
        """Equity price percent shock applied to all supplied identifiers.

        Parameters
        ----------
        ids : list[str]
            Equity identifier strings.
        pct : float
            Percent shock applied to each price.

        Returns
        -------
        OperationSpec
            The ``equity_price_pct`` operation.
        """
        ...

    @classmethod
    def instrument_price_pct_by_attr(cls, attrs: list[tuple[str, str]], pct: float) -> OperationSpec:
        """Instrument price shock by exact attribute match.

        ``attrs`` is a list of ``(key, value)`` pairs preserving order.

        Parameters
        ----------
        attrs : list[tuple[str, str]]
            Attribute key-value pairs to match.
        pct : float
            Percent shock applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_price_pct_by_attr`` operation.
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
        """Parallel basis-point shift on a rate-style curve.

        Parameters
        ----------
        curve_kind : CurveKind
            Type of curve to shock.
        curve_id : str
            Curve identifier in ``MarketContext``.
        bp : float
            Basis-point shift applied to every node.
        discount_curve_id : str, optional
            Discount curve ID for forward/inflation curves that require one.

        Returns
        -------
        OperationSpec
            The ``curve_parallel_bp`` operation.
        """
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
        """Node-level basis-point shifts on a rate-style curve.

        Parameters
        ----------
        curve_kind : CurveKind
            Type of curve to shock.
        curve_id : str
            Curve identifier in ``MarketContext``.
        nodes : list[tuple[str, float]]
            List of ``(tenor, bp)`` pairs.
        match_mode : TenorMatchMode, optional
            Tenor alignment strategy. Defaults to exact matching.
        discount_curve_id : str, optional
            Discount curve ID for forward/inflation curves.

        Returns
        -------
        OperationSpec
            The ``curve_node_bp`` operation.
        """
        ...

    @classmethod
    def vol_index_parallel_pts(cls, curve_id: str, points: float) -> OperationSpec:
        """Parallel shock to a volatility-index curve in absolute index points.

        Parameters
        ----------
        curve_id : str
            Volatility-index curve identifier.
        points : float
            Absolute index-point shift.

        Returns
        -------
        OperationSpec
            The ``vol_index_parallel_pts`` operation.
        """
        ...

    @classmethod
    def vol_index_node_pts(
        cls,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | None = None,
    ) -> OperationSpec:
        """Node-level shocks to a volatility-index curve in absolute index points.

        Parameters
        ----------
        curve_id : str
            Volatility-index curve identifier.
        nodes : list[tuple[str, float]]
            List of ``(tenor, points)`` pairs.
        match_mode : TenorMatchMode, optional
            Tenor alignment strategy.

        Returns
        -------
        OperationSpec
            The ``vol_index_node_pts`` operation.
        """
        ...

    @classmethod
    def base_corr_parallel_pts(cls, surface_id: str, points: float) -> OperationSpec:
        """Parallel base-correlation shift (absolute correlation points).

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Absolute correlation-point shift.

        Returns
        -------
        OperationSpec
            The ``base_corr_parallel_pts`` operation.
        """
        ...

    @classmethod
    def base_corr_bucket_pts(
        cls,
        surface_id: str,
        points: float,
        detachment_bps: list[int] | None = None,
        maturities: list[str] | None = None,
    ) -> OperationSpec:
        """Bucketed base-correlation shock by detachment and (reserved) maturity.

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Absolute correlation-point shift.
        detachment_bps : list[int], optional
            Detachment points (in bps) to target. ``None`` targets all.
        maturities : list[str], optional
            Maturity tenors to target. ``None`` targets all. Currently
            reserved for future use.

        Returns
        -------
        OperationSpec
            The ``base_corr_bucket_pts`` operation.
        """
        ...

    @classmethod
    def vol_surface_parallel_pct(cls, surface_kind: VolSurfaceKind, surface_id: str, pct: float) -> OperationSpec:
        """Parallel percent shift to a volatility surface.

        Parameters
        ----------
        surface_kind : VolSurfaceKind
            Category of volatility surface.
        surface_id : str
            Surface identifier.
        pct : float
            Percent shift applied to every vol quote.

        Returns
        -------
        OperationSpec
            The ``vol_surface_parallel_pct`` operation.
        """
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
        """Bucketed volatility surface percent shock.

        Parameters
        ----------
        surface_kind : VolSurfaceKind
            Category of volatility surface.
        surface_id : str
            Surface identifier.
        pct : float
            Percent shift applied to matched vol quotes.
        tenors : list[str], optional
            Tenor labels to target. ``None`` targets all.
        strikes : list[float], optional
            Strike levels to target. ``None`` targets all.

        Returns
        -------
        OperationSpec
            The ``vol_surface_bucket_pct`` operation.
        """
        ...

    @classmethod
    def stmt_forecast_percent(cls, node_id: str, pct: float) -> OperationSpec:
        """Statement forecast percent change.

        Parameters
        ----------
        node_id : str
            Statement forecast node identifier.
        pct : float
            Percent change applied to the forecast value.

        Returns
        -------
        OperationSpec
            The ``stmt_forecast_percent`` operation.
        """
        ...

    @classmethod
    def stmt_forecast_assign(cls, node_id: str, value: float) -> OperationSpec:
        """Statement forecast value assignment.

        Parameters
        ----------
        node_id : str
            Statement forecast node identifier.
        value : float
            Absolute value to assign.

        Returns
        -------
        OperationSpec
            The ``stmt_forecast_assign`` operation.
        """
        ...

    @classmethod
    def rate_binding(cls, binding: RateBindingSpec) -> OperationSpec:
        """Bind a statement rate node to a curve for the lifetime of the scenario.

        Parameters
        ----------
        binding : RateBindingSpec
            Rate binding configuration.

        Returns
        -------
        OperationSpec
            The ``rate_binding`` operation.
        """
        ...

    @classmethod
    def instrument_spread_bp_by_attr(cls, attrs: list[tuple[str, str]], bp: float) -> OperationSpec:
        """Instrument spread shock (basis points) by exact attribute match.

        Parameters
        ----------
        attrs : list[tuple[str, str]]
            Attribute key-value pairs to match.
        bp : float
            Basis-point shift applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_spread_bp_by_attr`` operation.
        """
        ...

    @classmethod
    def instrument_price_pct_by_type(cls, instrument_types: list[str], pct: float) -> OperationSpec:
        """Instrument price shock by ``InstrumentType`` (snake_case strings).

        Parameters
        ----------
        instrument_types : list[str]
            Instrument type identifiers in snake_case.
        pct : float
            Percent shock applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_price_pct_by_type`` operation.
        """
        ...

    @classmethod
    def instrument_spread_bp_by_type(cls, instrument_types: list[str], bp: float) -> OperationSpec:
        """Instrument spread shock by ``InstrumentType`` (snake_case strings).

        Parameters
        ----------
        instrument_types : list[str]
            Instrument type identifiers in snake_case.
        bp : float
            Basis-point shift applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_spread_bp_by_type`` operation.
        """
        ...

    @classmethod
    def asset_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """Asset-correlation shock for structured credit.

        Parameters
        ----------
        delta_pts : float
            Absolute correlation-point shift.

        Returns
        -------
        OperationSpec
            The ``asset_correlation_pts`` operation.
        """
        ...

    @classmethod
    def prepay_default_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """Prepay-default correlation shock for structured credit.

        Parameters
        ----------
        delta_pts : float
            Absolute correlation-point shift.

        Returns
        -------
        OperationSpec
            The ``prepay_default_correlation_pts`` operation.
        """
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

        Parameters
        ----------
        curve_kind : CurveKind
            Type of curve to shock.
        target_json : str
            JSON-serialized ``HierarchyTarget``.
        bp : float
            Basis-point shift applied to every node.
        discount_curve_id : str, optional
            Discount curve ID for forward/inflation curves.

        Returns
        -------
        OperationSpec
            The ``hierarchy_curve_parallel_bp`` operation.
        """
        ...

    @classmethod
    def hierarchy_vol_surface_parallel_pct(
        cls, surface_kind: VolSurfaceKind, target_json: str, pct: float
    ) -> OperationSpec:
        """Hierarchy-targeted vol-surface percent shift.

        Parameters
        ----------
        surface_kind : VolSurfaceKind
            Category of volatility surface.
        target_json : str
            JSON-serialized ``HierarchyTarget``.
        pct : float
            Percent shift applied to matched vol quotes.

        Returns
        -------
        OperationSpec
            The ``hierarchy_vol_surface_parallel_pct`` operation.
        """
        ...

    @classmethod
    def hierarchy_equity_price_pct(cls, target_json: str, pct: float) -> OperationSpec:
        """Hierarchy-targeted equity price shift.

        Parameters
        ----------
        target_json : str
            JSON-serialized ``HierarchyTarget``.
        pct : float
            Percent shift applied to matched equity prices.

        Returns
        -------
        OperationSpec
            The ``hierarchy_equity_price_pct`` operation.
        """
        ...

    @classmethod
    def hierarchy_base_corr_parallel_pts(cls, target_json: str, points: float) -> OperationSpec:
        """Hierarchy-targeted base-correlation parallel shift.

        Parameters
        ----------
        target_json : str
            JSON-serialized ``HierarchyTarget``.
        points : float
            Absolute correlation-point shift.

        Returns
        -------
        OperationSpec
            The ``hierarchy_base_corr_parallel_pts`` operation.
        """
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

        Parameters
        ----------
        period : str
            Tenor string for the roll period (e.g. ``"1M"``, ``"3M"``, ``"1Y"``).
        apply_shocks : bool, default True
            Whether to apply scenario shocks after the time roll.
        roll_mode : TimeRollMode, optional
            Calendar-vs-business-day roll mode.

        Returns
        -------
        OperationSpec
            The ``time_roll_forward`` operation.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON-serialized ``OperationSpec``.
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> OperationSpec:
        """Deserialize an ``OperationSpec`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        OperationSpec
            Parsed operation specification.

        Raises
        ------
        ValueError
            If the JSON is malformed or the operation kind is unknown.
        """
        ...

    @property
    def kind(self) -> str:
        """Variant discriminator (the serde ``kind`` tag value).

        Returns
        -------
        str
            Snake-case operation kind string.
        """
        ...
