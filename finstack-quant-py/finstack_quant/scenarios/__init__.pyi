"""
Scenario specification, validation, composition, application, and built-in templates.

Examples
--------
>>> from finstack_quant.scenarios import list_builtin_templates
>>> list_builtin_templates()[:2]
['gfc_2008', 'covid_2020']
"""

from __future__ import annotations

import datetime

import pandas as pd

from collections.abc import Mapping, Sequence
from typing import Any, Literal, overload

from finstack_quant.attribution import PnlAttribution
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.statements import FinancialModelSpec
from finstack_quant.scenarios import schema as schema

__all__ = [
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
    "ApplicationReport",
    "ApplicationResult",
    "HierarchyTarget",
    "HorizonResult",
    "OperationSpec",
    "RateBindingSpec",
    "ScenarioSpec",
    "TemplateMetadata",
    "CurveKind",
    "TenorMatchMode",
    "TimeRollMode",
    "Compounding",
    "schema",
]

class ScenarioSpec:
    """
    Validated scenario specification executed by the scenario engine.

    Parameters
    ----------
    id : str
        Stable scenario identifier used for lookup and serialization.
    operations : list[OperationSpec]
        Ordered operations applied by the scenario engine.
    name : str, optional
        Human-readable scenario name.
    description : str, optional
        Human-readable explanation of the scenario.
    priority : int, default 0
        Composition priority; lower values execute first.
    resolution_mode : {"most_specific_wins", "cumulative"}
        Hierarchy conflict policy.
    hazard_bump_mode : {"solve_to_par", "first_order_shift"}
        ParCDS delivery. ``solve_to_par`` re-bootstraps hazard from shocked
        par spreads; ``first_order_shift`` shifts hazard knots in place.

    Raises
    ------
    ValueError
        If the resolution mode, hazard bump mode, or resulting scenario is invalid.

    Examples
    --------
    >>> from finstack_quant.scenarios import CurveKind, OperationSpec, ScenarioSpec
    >>> operation = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 25.0)
    >>> spec = ScenarioSpec("rates_up", [operation])
    >>> spec.id
    'rates_up'
    >>> spec == ScenarioSpec.from_json(spec.to_json())
    True
    """

    def __init__(
        self,
        id: str,
        operations: list[OperationSpec],
        name: str | None = None,
        description: str | None = None,
        priority: int = 0,
        resolution_mode: Literal["most_specific_wins", "cumulative"] = "most_specific_wins",
        hazard_bump_mode: Literal["solve_to_par", "first_order_shift"] = "solve_to_par",
    ) -> None: ...
    @staticmethod
    def from_json(json: str) -> ScenarioSpec:
        """Deserialize and validate canonical scenario JSON.

        Parameters
        ----------
        json : str
            JSON object matching the Rust ``ScenarioSpec`` serde contract.

        Returns
        -------
        ScenarioSpec
            Validated typed scenario specification.

        Raises
        ------
        ValueError
            If JSON parsing or scenario validation fails.

        Examples
        --------
        >>> from finstack_quant.scenarios import ScenarioSpec
        >>> ScenarioSpec.from_json('{"id":"typed","operations":[]}').id
        'typed'
        """
        ...

    def to_json(self) -> str:
        """Return compact JSON matching the canonical Rust serde contract.

        Returns
        -------
        str
            Canonical scenario JSON.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def validate(self) -> None:
        """Validate identifiers, operations, numeric fields, and composition rules.

        Raises
        ------
        ValueError
            If the scenario violates a canonical Rust validation rule.
        """
        ...

    def requires_instruments(self) -> bool:
        """Whether applying this scenario needs instruments in the execution context.

        Returns
        -------
        bool
            ``True`` when any operation is instrument-scoped or a
            ``time_roll_forward`` (which reads instruments for carry).

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec, ScenarioSpec
        >>> ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M")]).requires_instruments()
        True
        """
        ...

    def mutates_instruments(self) -> bool:
        """Whether applying this scenario can replace or mutate instruments.

        Returns
        -------
        bool
            ``True`` for instrument price, spread, or structured-credit
            correlation shocks; ``False`` for market-only scenarios and time
            rolls. ``apply_scenario*`` raise ``ValueError`` when this is
            ``True`` and no ``instruments`` are supplied.

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec, ScenarioSpec
        >>> ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M")]).mutates_instruments()
        False
        """
        ...

    def with_hazard_bump_mode(self, mode: Literal["solve_to_par", "first_order_shift"]) -> ScenarioSpec:
        """Return a copy with a different ParCDS hazard delivery mode.

        Parameters
        ----------
        mode : {"solve_to_par", "first_order_shift"}
            ``solve_to_par`` re-bootstraps hazard from shocked par spreads;
            ``first_order_shift`` shifts hazard knots in place.

        Returns
        -------
        ScenarioSpec
            New specification with ``hazard_bump_mode`` replaced.

        Raises
        ------
        ValueError
            If ``mode`` is not one of the accepted labels.

        Examples
        --------
        >>> from finstack_quant.scenarios import ScenarioSpec
        >>> ScenarioSpec("s", []).with_hazard_bump_mode("first_order_shift").hazard_bump_mode
        'first_order_shift'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on every field (id, operations, priority, modes).

        Parameters
        ----------
        other : object
            Value to compare; non-``ScenarioSpec`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both specs serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @property
    def id(self) -> str:
        """Return the stable scenario identifier.

        Returns
        -------
        str
            Identifier used for lookup and serialization.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def name(self) -> str | None:
        """Return the optional human-readable scenario name.

        Returns
        -------
        str or None
            Display name, or ``None`` when absent.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def description(self) -> str | None:
        """Return the optional human-readable scenario explanation.

        Returns
        -------
        str or None
            Scenario description, or ``None`` when absent.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def operations(self) -> list[OperationSpec]:
        """Return independent typed operations in execution order.

        Returns
        -------
        list[OperationSpec]
            Ordered operations applied by the scenario engine.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def priority(self) -> int:
        """Return the scenario composition priority.

        Returns
        -------
        int
            Priority where lower values execute first.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def resolution_mode(self) -> Literal["most_specific_wins", "cumulative"]:
        """Return the canonical hierarchy conflict policy.

        Returns
        -------
        {"most_specific_wins", "cumulative"}
            Policy used when targeted operations overlap.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def hazard_bump_mode(self) -> Literal["solve_to_par", "first_order_shift"]:
        """Return the canonical ParCDS hazard delivery mode.

        Returns
        -------
        {"solve_to_par", "first_order_shift"}
            ``solve_to_par`` re-bootstraps hazard from shocked par spreads;
            ``first_order_shift`` shifts hazard knots in place.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

class TemplateMetadata:
    """
    Discovery metadata for one built-in historical scenario template.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_template_metadata
    >>> list_builtin_template_metadata()[0].id
    'gfc_2008'
    """

    @property
    def id(self) -> str:
        """Return the stable built-in template identifier.

        Returns
        -------
        str
            Identifier accepted by template build functions.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def name(self) -> str:
        """Return the human-readable template name.

        Returns
        -------
        str
            Display name from the embedded registry.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def description(self) -> str:
        """Return the historical event and modeled-effects description.

        Returns
        -------
        str
            Narrative description from the embedded registry.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def event_date(self) -> datetime.date:
        """Return the primary historical event date.

        Returns
        -------
        datetime.date
            Date of the modeled market dislocation.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def asset_classes(self) -> list[str]:
        """Return canonical affected asset-class labels.

        Returns
        -------
        list[str]
            Labels drawn from rates, credit, equity, fx, volatility, and commodity.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def tags(self) -> list[str]:
        """Return freeform discovery tags.

        Returns
        -------
        list[str]
            Tags in embedded registry order.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def severity(self) -> Literal["mild", "moderate", "severe"]:
        """Return the canonical scenario severity label.

        Returns
        -------
        {"mild", "moderate", "severe"}
            Discovery severity assigned by the template registry.

        Raises
        ------
        None
            This property does not raise.
        """
        ...
    @property
    def components(self) -> list[str]:
        """Return component identifiers in deterministic build order.

        Returns
        -------
        list[str]
            IDs accepted by :func:`build_template_component`.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on every metadata field.

        Parameters
        ----------
        other : object
            Value to compare; non-``TemplateMetadata`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both values serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> TemplateMetadata:
        """Deserialize template metadata from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical metadata JSON produced by :meth:`to_json`.

        Returns
        -------
        TemplateMetadata
            Typed metadata reconstructed from the wire representation.

        Raises
        ------
        ValueError
            If ``json`` is malformed or incompatible with the metadata schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import TemplateMetadata, list_builtin_template_metadata
        >>> original = list_builtin_template_metadata()[0]
        >>> TemplateMetadata.from_json(original.to_json()).id == original.id
        True
        """
        ...

    def to_json(self) -> str:
        """Return compact JSON matching the canonical Rust serde contract.

        Returns
        -------
        str
            Canonical template-metadata JSON.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

def compose_scenarios(specs: list[ScenarioSpec]) -> ScenarioSpec:
    """
    Merge multiple scenario specs using the scenario engine composer.

    Parameters
    ----------
    specs : list[ScenarioSpec]
        Typed scenario specifications to compose.

    Returns
    -------
    ScenarioSpec
        Typed composed scenario specification.

    Raises
    ------
    ValueError
        If scenario composition fails.

    Examples
    --------
    >>> from finstack_quant.scenarios import compose_scenarios
    >>> compose_scenarios([]).operations
    []
    """
    ...

def validate_scenario_spec(scenario: ScenarioSpec | str) -> None:
    """
    Validate a scenario specification without applying it.

    Parameters
    ----------
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.

    Returns
    -------
    None
        Returns nothing on success. An invalid spec raises instead, so
        ``if validate_scenario_spec(s):`` is not a validity check.

    Raises
    ------
    ValueError
        If ``scenario`` is not valid JSON or fails validation; the message is
        the one ``ScenarioSpec.validate()`` raises.

    Examples
    --------
    >>> from finstack_quant.scenarios import validate_scenario_spec
    >>> spec = '{"id":"s","name":"S","operations":[]}'
    >>> validate_scenario_spec(spec) is None
    True
    """
    ...

def list_builtin_templates() -> list[str]:
    """
    List template IDs from the embedded built-in registry.

    Returns
    -------
    list[str]
        Template identifier strings.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_templates
    >>> list_builtin_templates()[:2]
    ['gfc_2008', 'covid_2020']
    """
    ...

def list_builtin_template_metadata() -> list[TemplateMetadata]:
    """
    Return typed metadata for all built-in templates.

    Returns
    -------
    list[TemplateMetadata]
        Metadata in deterministic registry order.

    Raises
    ------
    ValueError
        If the example or catalog cannot be produced.

    Examples
    --------
    >>> from finstack_quant.scenarios import list_builtin_template_metadata
    >>> list_builtin_template_metadata()[0].id
    'gfc_2008'
    """
    ...

def build_from_template(template_id: str) -> ScenarioSpec:
    """
    Instantiate a ``ScenarioSpec`` from a built-in template.

    Parameters
    ----------
    template_id : str
        Registry key for the template.

    Returns
    -------
    ScenarioSpec
        Typed scenario specification built from the template.

    Raises
    ------
    ValueError
        If ``template_id`` is not found in the registry.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_from_template
    >>> build_from_template("gfc_2008").id
    'gfc_2008'
    """
    ...

def list_template_components(template_id: str) -> list[str]:
    """
    List sub-component IDs for composite templates.

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
    >>> list_template_components("gfc_2008")[:2]
    ['gfc_2008_rates', 'gfc_2008_credit']
    """
    ...

def build_template_component(template_id: str, component_id: str) -> ScenarioSpec:
    """
    Build a single component spec from a composite template.

    Parameters
    ----------
    template_id : str
        Parent template identifier.
    component_id : str
        Component key inside the template.

    Returns
    -------
    ScenarioSpec
        Typed component scenario specification.

    Raises
    ------
    ValueError
        If ``template_id`` or ``component_id`` is not found.

    Examples
    --------
    >>> from finstack_quant.scenarios import build_template_component
    >>> component = build_template_component("gfc_2008", "gfc_2008_rates")
    >>> component.id
    'gfc_2008_rates'
    """
    ...

# Scenario application

class ApplicationReport:
    """
    Report describing what a scenario application changed.

    Exposed as the :attr:`ApplicationResult.report` attribute of the result
    returned by :func:`apply_scenario` and :func:`apply_scenario_to_market`,
    and as the second element of the tuples returned by
    :func:`finstack_quant.portfolio.scenario_pnl` and
    :func:`finstack_quant.portfolio.apply_scenario_and_revalue`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario_to_market, compose_scenarios
    >>> report = apply_scenario_to_market(compose_scenarios([]), MarketContext(), "2025-01-15").report
    >>> (report.operations_applied, report.user_operations, report.warnings)
    (0, 0, [])
    """

    @property
    def operations_applied(self) -> int:
        """
        Number of effects successfully applied to the execution context.

        Returns
        -------
        int
            Low-level effect count. One user-level operation can produce zero,
            one, or many effects; inspect ``changes`` and ``warnings`` for
            coverage.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def user_operations(self) -> int:
        """
        Number of user-provided operations before hierarchy expansion.

        Returns
        -------
        int
            Count of operations as written in the ``ScenarioSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expanded_operations(self) -> int:
        """
        Number of operations the engine attempted after hierarchy expansion.

        Returns
        -------
        int
            Count of post-expansion operations, which may exceed
            :attr:`user_operations` when an operation fans out over a
            hierarchy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Non-fatal warnings raised while applying the scenario.

        Returns
        -------
        list[dict[str, Any]]
            Structured warnings in emission order. Each dict carries a
            ``kind`` discriminator (``"equity_not_found"``,
            ``"discount_curve_heuristic"``, ``"commodity_shock_outside_range"``,
            ...) plus variant-specific fields. Empty when the scenario applied
            cleanly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings_json(self) -> str:
        """
        The structured warnings as one JSON-encoded array.

        Returns
        -------
        str
            JSON array; ``json.loads`` gives the same list as ``warnings``.

        Raises
        ------
        ValueError
            If the warnings cannot be serialized.
        """
        ...

    @property
    def warning_count(self) -> int:
        """
        Number of warnings raised while applying the scenario.

        Returns
        -------
        int
            ``len(warnings)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any] | None:
        """
        Audit stamp: numeric mode, rounding context, and FX policy in force.

        Returns
        -------
        dict[str, Any] or None
            Policy stamp with ``numeric_mode``, ``rounding``,
            ``fx_policy_applied``, and ``version`` keys, or ``None`` when the
            engine recorded no stamp.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def changes(self) -> dict[str, Any]:
        """
        Metadata describing exactly which market state the effects changed.

        Returns
        -------
        dict[str, Any]
            Invalidation record with ``market_targets``,
            ``changed_instrument_indices``, ``as_of_changed``,
            ``portfolio_shape_changed``, and ``all_dirty`` keys, used
            downstream for precise cache invalidation.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def time_roll(self) -> dict[str, Any] | None:
        """
        Roll-forward report, present only when the scenario contained a
        ``time_roll_forward`` operation.

        Returns
        -------
        dict[str, Any] or None
            Roll details, or ``None`` when the scenario performed no time roll.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report counters as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``operations_applied``, ``user_operations``,
            ``expanded_operations``, ``warning_count``, ``as_of_changed`` and
            ``all_dirty``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario_to_market
        >>> frame = apply_scenario_to_market(ScenarioSpec("s", []), MarketContext(), "2025-01-15").report.to_dataframe()
        >>> list(frame.columns)[:4]
        ['operations_applied', 'user_operations', 'expanded_operations', 'warning_count']
        """
        ...

    def changes_to_dataframe(self) -> pd.DataFrame:
        """
        Export the market targets the scenario actually changed, one row each.

        Returns
        -------
        pd.DataFrame
            Columns ``kind`` (``curve``, ``volatility_index``,
            ``base_correlation``, ``vol_surface``, ``equity_price``, ``fx``),
            ``id`` (identifier, or ``BASE/QUOTE`` for FX) and ``curve_kind``
            (curve family for ``curve`` rows, else ``None``). Empty, with the
            same columns, when nothing changed.

        Raises
        ------
        ValueError
            If the manifest cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario_to_market
        >>> frame = apply_scenario_to_market(
        ...     ScenarioSpec("s", []), MarketContext(), "2025-01-15"
        ... ).report.changes_to_dataframe()
        >>> list(frame.columns)
        ['kind', 'id', 'curve_kind']
        """
        ...

    def carry_to_dataframe(self) -> pd.DataFrame:
        """
        Export per-instrument carry from the time roll, one row per
        instrument and currency.

        Returns
        -------
        pd.DataFrame
            Columns ``instrument_id``, ``amount`` (carry P&L as a float) and
            ``currency`` (ISO-4217 code). Empty when the scenario had no
            ``time_roll_forward`` or no instruments were supplied.

        Raises
        ------
        ValueError
            If the carry rows cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario_to_market
        >>> frame = apply_scenario_to_market(
        ...     ScenarioSpec("s", []), MarketContext(), "2025-01-15"
        ... ).report.carry_to_dataframe()
        >>> list(frame.columns)
        ['instrument_id', 'amount', 'currency']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this report to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ApplicationReport:
        """
        Deserialize an ``ApplicationReport`` from JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`.

        Returns
        -------
        ApplicationReport
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ApplicationReport`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import ApplicationReport
        >>> try:
        ...     ApplicationReport.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

class ApplicationResult:
    """
    Result of applying a scenario: the mutated market, the mutated model (when
    one was supplied), and the application report.

    Returned by :func:`apply_scenario` and :func:`apply_scenario_to_market`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario_to_market, compose_scenarios
    >>> applied = apply_scenario_to_market(compose_scenarios([]), MarketContext(), "2025-01-15")
    >>> (type(applied.market).__name__, applied.model, applied.report.operations_applied)
    ('MarketContext', None, 0)
    """

    @property
    def market(self) -> MarketContext:
        """
        The mutated market context.

        Returns
        -------
        MarketContext
            The market after every scenario effect was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model(self) -> FinancialModelSpec | None:
        """
        The mutated financial model, or ``None`` when no model was supplied.

        Returns
        -------
        FinancialModelSpec or None
            Always ``None`` for :func:`apply_scenario_to_market`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instruments(self) -> list[str] | None:
        """Shocked copies of the supplied instrument inventory.

        Returns
        -------
        list[str] or None
            Canonical instrument-envelope JSON strings in input order, accepted
            by subsequent scenario calls and instrument ``from_json`` constructors.
            ``None`` means no inventory was supplied; an empty list stays empty.
            The original Python objects are never mutated.

        Raises
        ------
        ValueError
            If an instrument cannot be serialized to its canonical envelope.
        """
        ...

    @property
    def report(self) -> ApplicationReport:
        """
        What the scenario changed.

        Returns
        -------
        ApplicationReport
            Operation counters, warnings, invalidation metadata, and the
            policy stamp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the application report counters as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Same columns as :meth:`ApplicationReport.to_dataframe`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.scenarios import ScenarioSpec, apply_scenario_to_market
        >>> len(apply_scenario_to_market(ScenarioSpec("s", []), MarketContext(), "2025-01-15").to_dataframe())
        1
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Emits the canonical ``ApplicationEnvelope`` shape, with ``market`` and
        ``model`` as nested objects alongside the report fields.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ApplicationResult:
        """
        Deserialize an ``ApplicationResult`` from JSON.

        Parameters
        ----------
        json : str
            Canonical ``ApplicationEnvelope`` payload produced by
            :meth:`to_json`; the market, model, and report are rebuilt from it.

        Returns
        -------
        ApplicationResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ApplicationEnvelope`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import ApplicationResult
        >>> try:
        ...     ApplicationResult.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

def apply_scenario(
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
    model: FinancialModelSpec | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    instruments: Sequence[Any] | None = None,
    config: FinstackConfig | str | None = None,
) -> ApplicationResult:
    """
    Apply a scenario to both market data and a financial model.

    Parameters
    ----------
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.
    market : MarketContext | str
        ``MarketContext`` object or JSON ``MarketContext`` string. Never
        mutated; the result carries a modified copy.
    model : FinancialModelSpec | str
        ``FinancialModelSpec`` object or JSON ``FinancialModelSpec`` string.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date (ISO 8601 accepted).
    instruments : Sequence[Instrument | str] | None, default None
        Typed instruments (``Bond``, ``CreditDefaultSwap``, ...) or canonical
        instrument-envelope JSON strings. Required when the scenario contains
        instrument-scoped operations; also used for carry under
        ``time_roll_forward``. Shocked copies are returned as canonical envelope JSON strings in
        ``ApplicationResult.instruments``.
    config : FinstackConfig | str | None, default None
        Library configuration (rounding policy stamped into ``report.meta``);
        ``None`` uses the library default.

    Returns
    -------
    ApplicationResult
        Typed result exposing :attr:`~ApplicationResult.market`,
        :attr:`~ApplicationResult.model` and :attr:`~ApplicationResult.report`.

    Notes
    -----
    No holiday calendar is supplied, so ``time_roll_forward`` in
    ``business_days`` mode adjusts against a weekends-only calendar. Quote
    replay operations use a fresh cached recalibration provider.

    Raises
    ------
    ValueError
        If an input fails to parse or validate, or the scenario mutates
        instruments and ``instruments`` is ``None``.
    KeyError
        If the scenario references market data, statement nodes, tenors or
        instruments that do not exist.
    RuntimeError
        If the engine fails internally.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import apply_scenario, compose_scenarios
    >>> model = (
    ...     '{"schema_version":1,"id":"m","periods":['
    ...     '{"id":"2025Q1","start":"2025-01-01","end":"2025-04-01",'
    ...     '"is_actual":false}],"nodes":{}}'
    ... )
    >>> applied = apply_scenario(compose_scenarios([]), MarketContext(), model, "2025-01-15")
    >>> applied.report.operations_applied
    0
    """
    ...

def apply_scenario_to_market(
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    instruments: Sequence[Any] | None = None,
    config: FinstackConfig | str | None = None,
) -> ApplicationResult:
    """
    Apply a scenario to market data only (no model mutations returned).

    Parameters
    ----------
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.
    market : MarketContext | str
        ``MarketContext`` object or JSON ``MarketContext`` string. Never
        mutated.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date (ISO 8601 accepted).
    instruments : Sequence[Instrument | str] | None, default None
        Typed instruments or canonical envelope JSON strings; required for
        instrument-scoped operations, used for carry under
        ``time_roll_forward``. Shocked copies are returned in ``ApplicationResult.instruments``.
    config : FinstackConfig | str | None, default None
        Library configuration; ``None`` uses the default.

    Returns
    -------
    ApplicationResult
        Typed result whose :attr:`~ApplicationResult.model` attribute is
        ``None``.

    Notes
    -----
    No holiday calendar is supplied, so business-day time rolls adjust
    against a weekends-only calendar.

    Raises
    ------
    ValueError
        If an input fails to parse or validate, or the scenario mutates
        instruments and ``instruments`` is ``None``.
    KeyError
        If the scenario references market data, tenors or instruments that
        do not exist.
    RuntimeError
        If the engine fails internally.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.scenarios import OperationSpec, ScenarioSpec, apply_scenario_to_market
    >>> market = MarketContext()
    >>> market.insert(
    ...     DiscountCurve(
    ...         "USD-OIS",
    ...         dt.date(2025, 1, 15),
    ...         [(0.0, 1.0), (1.0, 0.96), (2.0, 0.92)],
    ...         day_count="act_365f",
    ...     )
    ... )
    MarketContext(discount=['USD-OIS'], fx=False)
    >>> spec = ScenarioSpec("up25", [OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)])
    >>> applied = apply_scenario_to_market(spec, market, "2025-01-15")
    >>> applied.report.user_operations
    1
    """
    ...

class HorizonResult:
    """
    Horizon total return result with full P&L attribution.

    Produced by :func:`compute_horizon_return`. Access factor-level
    contributions via :meth:`factor_contribution` and the full breakdown
    via :attr:`attribution`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import compose_scenarios, compute_horizon_return
    >>> try:
    ...     compute_horizon_return("{}", MarketContext(), "2025-01-15", compose_scenarios([]))
    ... except ValueError as exc:
    ...     print(str(exc).split(":")[0])
    Validation error
    """

    @property
    def attribution(self) -> PnlAttribution:
        """
        Full P&L attribution breakdown.

        Returns
        -------
        PnlAttribution
            Carry, rate, credit, inflation, FX, volatility, and model-parameter
            contributions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def initial_value(self) -> float:
        """
        Initial instrument value.

        Returns
        -------
        float
            Present value at the original valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def terminal_value(self) -> float:
        """
        Final instrument value after the scenario is applied.

        Returns
        -------
        float
            Present value after scenario shocks and time roll, as a bare
            amount in ``currency``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 currency of ``initial_value`` and ``terminal_value``.

        Returns
        -------
        str
            Currency code, e.g. ``"USD"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def horizon_days(self) -> int | None:
        """
        Horizon in calendar days (``None`` if no time-roll).

        Returns
        -------
        int or None
            Number of days rolled forward, or ``None`` when the scenario
            contains no ``time_roll_forward`` operation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_return(self) -> float:
        """
        Total return as a decimal fraction (``0.05`` = +5%).

        Returns
        -------
        float
            ``total_pnl / initial_value``; ``nan`` when the initial value and
            total P&L are in different currencies (no implicit FX) or the
            initial value is zero or negative (no positive capital base).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def annualized_return(self) -> float | None:
        """
        Annualized return (``None`` if no time-roll).

        Returns
        -------
        float or None
            Annualized total return, or ``None`` when ``horizon_days`` is
            ``None`` or zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenario_report(self) -> ApplicationReport:
        """
        Report from applying the scenario to the market copy.

        Returns
        -------
        ApplicationReport
            Operation counters, change manifest, structured warnings and the
            time-roll report (``horizon_days`` comes from it).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Structured warnings emitted during scenario application.

        Returns
        -------
        list[dict[str, Any]]
            Same as ``scenario_report.warnings``: dicts with a ``kind``
            discriminator plus variant-specific fields.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings_json(self) -> str:
        """
        JSON-encoded structured warnings.

        Returns
        -------
        str
            JSON array; ``json.loads`` gives the same list as ``warnings``.

        Raises
        ------
        ValueError
            If the warnings cannot be serialized.
        """
        ...

    def factor_contribution(self, factor: str) -> float:
        """
        Factor contribution as decimal fraction of initial value.

        Parameters
        ----------
        factor : str
            Canonical ``AttributionFactor`` serde name: one of ``"carry"``,
            ``"rates_curves"``, ``"credit_curves"``, ``"inflation_curves"``,
            ``"correlations"``, ``"fx"``, ``"volatility"``,
            ``"market_scalars"``, or ``"model_parameters"``. Historical
            Python-only aliases (``"rates"``, ``"credit"``, ``"vol"``, ...)
            are no longer accepted.

        Returns
        -------
        float
            Contribution of the given factor as a decimal fraction. Returns ``nan``
            for a zero or negative initial value, or mismatched currencies.

        Raises
        ------
        ValueError
            If ``factor`` is not a canonical factor name.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the result to JSON.

        Returns
        -------
        str
            JSON-serialized ``HorizonResult`` envelope.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HorizonResult:
        """
        Deserialize from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON-serialized ``HorizonResult`` envelope.

        Returns
        -------
        HorizonResult
            The deserialized result.

        Raises
        ------
        ValueError
            If *json* does not match the ``HorizonResult`` schema.

        Examples
        --------
        >>> from finstack_quant.scenarios import HorizonResult
        >>> try:
        ...     HorizonResult.from_json("{}")
        ... except ValueError as exc:
        ...     print(type(exc).__name__)
        ValueError
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the horizon summary as a single-row pandas ``DataFrame``.

        Columns: ``initial_value``, ``terminal_value``, ``currency``,
        ``total_pnl``, ``total_return``, ``annualized_return``,
        ``horizon_days``, ``user_operations``, ``expanded_operations``,
        ``operations_applied``, ``warning_count``.

        ``total_return`` and ``annualized_return`` are decimal fractions
        (``0.05`` = +5%). For the factor-level breakdown use
        ``result.attribution.to_dataframe()``.

        Returns
        -------
        pandas.DataFrame
            One-row summary frame.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def explain(self) -> str:
        """
        Human-readable summary of horizon return and attribution.

        Returns
        -------
        str
            Multi-line text (total and annualized return, horizon, values,
            and the carry / rates / credit / residual legs) rendered by the
            Rust ``Display`` implementation.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        Render as an HTML table in Jupyter notebooks.

        Returns
        -------
        str or None
            HTML for the frame from :meth:`to_dataframe`, or ``None`` when the
            frame cannot be built (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise.
        """
        ...

def compute_horizon_return(
    instrument: Any,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    scenario: ScenarioSpec | str,
    method: Literal["parallel", "waterfall", "metrics_based", "taylor"] = "parallel",
    config: FinstackConfig | str | None = None,
    calendar_id: str | None = None,
) -> HorizonResult:
    """
    Compute horizon total return under a scenario.

    Parameters
    ----------
    instrument : Instrument | str
        Typed instrument (``Bond``, ``CreditDefaultSwap``,
        ``InterestRateSwap``, ...) or a canonical v1 instrument envelope JSON
        string.
    market : MarketContext | str
        ``MarketContext`` object or JSON string; never mutated.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date (ISO 8601 accepted).
    scenario : ScenarioSpec | str
        Typed scenario or JSON-serialized ``ScenarioSpec``.
    method : {"parallel", "waterfall", "metrics_based", "taylor"}
        Attribution method. ``"metrics_based"`` re-prices the instrument with
        the default attribution metric set (DV01, CS01, vega, ...) using the
        same configuration and recalibration provider as the scenario
        engine; instruments lacking one of those metrics raise
        ``RuntimeError`` instead of silently dropping the factor.
    config : FinstackConfig | str | None, default None
        Library configuration threaded into both the scenario engine and the
        attribution pricing; ``None`` uses the default.
    calendar_id : str, optional
        Holiday calendar used to business-day adjust ``time_roll_forward``
        targets under ``TimeRollMode.business_days`` (e.g. ``"nyse"``,
        ``"target"``). Defaults to a weekends-only calendar, so business-day
        rolls always avoid weekends but not market holidays. Raises
        ``ValueError`` if the identifier is not a built-in calendar.

    Returns
    -------
    HorizonResult
        Decomposed total return and factor attribution, with the scenario
        ``ApplicationReport`` as ``scenario_report``.

    Raises
    ------
    ValueError
        If an input fails to parse or validate, ``method`` is unknown,
        ``calendar_id`` is not a built-in calendar, or the scenario contains
        an instrument-scoped operation (horizon analysis prices one
        instrument instance at both dates).
    KeyError
        If the scenario references market data or tenors that do not exist.
    RuntimeError
        If pricing or attribution fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.scenarios import compose_scenarios, compute_horizon_return
    >>> try:
    ...     compute_horizon_return("{}", MarketContext(), "2025-01-15", compose_scenarios([]))
    ... except ValueError as exc:
    ...     print(str(exc).split(":")[0])
    Validation error
    """
    ...

# Typed operation builders
#
# These mirror the Rust ``OperationSpec`` enum and its supporting enums. They
# replace the raw-JSON authoring path so quants can write
# ``OperationSpec.curve_parallel_bp(...)`` and feed the result straight into
# ``ScenarioSpec(...)``.

class CurveKind:
    """
    Type of market curve targeted by a scenario operation.

    Examples
    --------
    >>> from finstack_quant.scenarios import CurveKind
    >>> CurveKind.discount().value
    'discount'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"discount"``, ``"forward"``, ``"par_cds"``, ``"inflation"``, ``"commodity"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> CurveKind("par_cds").value
        'par_cds'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``CurveKind`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def discount(cls) -> CurveKind:
        """
        Curve kind for a discount-factor (zero) curve.

        Returns
        -------
        CurveKind
            The ``discount`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.discount())
        'CurveKind.Discount'
        """
        ...

    @classmethod
    def forward(cls) -> CurveKind:
        """
        Curve kind for a forward/projection rate curve.

        Returns
        -------
        CurveKind
            The ``forward`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.forward())
        'CurveKind.Forward'
        """
        ...

    @classmethod
    def par_cds(cls) -> CurveKind:
        """
        Curve kind for a par CDS spread (hazard) curve.

        Returns
        -------
        CurveKind
            The ``par_cds`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.par_cds())
        'CurveKind.ParCDS'
        """
        ...

    @classmethod
    def inflation(cls) -> CurveKind:
        """
        Curve kind for an inflation/CPI index curve.

        Returns
        -------
        CurveKind
            The ``inflation`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.inflation())
        'CurveKind.Inflation'
        """
        ...

    @classmethod
    def commodity(cls) -> CurveKind:
        """
        Commodity forward curve.

        Returns
        -------
        CurveKind
            The ``commodity`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind
        >>> str(CurveKind.commodity())
        'CurveKind.Commodity'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"Discount"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"discount"`` or ``"par_cds"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class TenorMatchMode:
    """
    Tenor-pillar alignment strategy for curve-node operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TenorMatchMode
    >>> TenorMatchMode.exact().value
    'exact'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"exact"``, ``"interpolate"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> TenorMatchMode("interpolate").value
        'interpolate'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``TenorMatchMode`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def exact(cls) -> TenorMatchMode:
        """
        Match curve nodes by exact tenor string.

        Returns
        -------
        TenorMatchMode
            The ``exact`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> str(TenorMatchMode.exact())
        'TenorMatchMode.Exact'
        """
        ...

    @classmethod
    def interpolate(cls) -> TenorMatchMode:
        """
        Interpolate between adjacent curve nodes when tenor is not exact.

        Returns
        -------
        TenorMatchMode
            The ``interpolate`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TenorMatchMode
        >>> str(TenorMatchMode.interpolate())
        'TenorMatchMode.Interpolate'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"Exact"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"exact"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class TimeRollMode:
    """
    Calendar-vs-business-day semantics for time-roll operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import TimeRollMode
    >>> TimeRollMode.calendar_days().value
    'calendar_days'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"business_days"``, ``"calendar_days"``, ``"approximate"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> TimeRollMode("calendar_days").value
        'calendar_days'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``TimeRollMode`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def business_days(cls) -> TimeRollMode:
        """
        Roll by business days using the market calendar.

        Returns
        -------
        TimeRollMode
            The ``business_days`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.business_days())
        'TimeRollMode.BusinessDays'
        """
        ...

    @classmethod
    def calendar_days(cls) -> TimeRollMode:
        """
        Roll by calendar days (no holiday adjustment).

        Returns
        -------
        TimeRollMode
            The ``calendar_days`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.calendar_days())
        'TimeRollMode.CalendarDays'
        """
        ...

    @classmethod
    def approximate(cls) -> TimeRollMode:
        """
        Approximate roll (e.g. 30/360 day count).

        Returns
        -------
        TimeRollMode
            The ``approximate`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import TimeRollMode
        >>> str(TimeRollMode.approximate())
        'TimeRollMode.Approximate'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"CalendarDays"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"calendar_days"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class Compounding:
    """
    Compounding convention for rate-extraction operations.

    Examples
    --------
    >>> from finstack_quant.scenarios import Compounding
    >>> Compounding.continuous().value
    'continuous'
    """

    def __init__(self, label: str) -> None:
        """
        Construct from the canonical snake-case wire label.

        Parameters
        ----------
        label : str
            One of ``"simple"``, ``"continuous"``, ``"annual"``, ``"semi_annual"``, ``"quarterly"``, ``"monthly"``.

        Raises
        ------
        ValueError
            If ``label`` is not an accepted wire label.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> Compounding("annual").value
        'annual'
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Variant equality; non-``Compounding`` values compare unequal.

        Parameters
        ----------
        other : object
            Value to compare.

        Returns
        -------
        bool
            ``True`` when both are the same variant.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def __hash__(self) -> int:
        """
        Hash consistent with ``__eq__`` so values work as dict keys.

        Returns
        -------
        int
            Variant hash.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @classmethod
    def simple(cls) -> Compounding:
        """
        Simple (zero-rate) compounding.

        Returns
        -------
        Compounding
            The ``simple`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.simple())
        'Compounding.Simple'
        """
        ...

    @classmethod
    def continuous(cls) -> Compounding:
        """
        Continuously compounded rate.

        Returns
        -------
        Compounding
            The ``continuous`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.continuous())
        'Compounding.Continuous'
        """
        ...

    @classmethod
    def annual(cls) -> Compounding:
        """
        Annual compounding convention for the bound rate.

        Returns
        -------
        Compounding
            The ``annual`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.annual())
        'Compounding.Annual'
        """
        ...

    @classmethod
    def semi_annual(cls) -> Compounding:
        """
        Semi-annual compounding.

        Returns
        -------
        Compounding
            The ``semi_annual`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.semi_annual())
        'Compounding.SemiAnnual'
        """
        ...

    @classmethod
    def quarterly(cls) -> Compounding:
        """
        Quarterly compounding convention for the bound rate.

        Returns
        -------
        Compounding
            The ``quarterly`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.quarterly())
        'Compounding.Quarterly'
        """
        ...

    @classmethod
    def monthly(cls) -> Compounding:
        """
        Monthly compounding convention for the bound rate.

        Returns
        -------
        Compounding
            The ``monthly`` variant.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.scenarios import Compounding
        >>> str(Compounding.monthly())
        'Compounding.Monthly'
        """
        ...

    @property
    def name(self) -> str:
        """
        Variant name, e.g. ``"Continuous"``.

        Returns
        -------
        str
            Pascal-case variant name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def value(self) -> str:
        """
        Serialized wire value, e.g. ``"continuous"``.

        Returns
        -------
        str
            Snake-case wire value used in JSON serialization.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class RateBindingSpec:
    """
    Configuration linking a statement rate node to a market curve.

    Examples
    --------
    >>> from finstack_quant.scenarios import RateBindingSpec, Compounding
    >>> spec = RateBindingSpec("node_1", "USD-OIS", "5Y", Compounding.continuous())
    >>> (spec.curve_id, spec.tenor, spec.compounding.value)
    ('USD-OIS', '5Y', 'continuous')
    >>> spec == RateBindingSpec("node_1", "USD-OIS", "5Y", "continuous")
    True
    """

    def __init__(
        self,
        node_id: str,
        curve_id: str,
        tenor: str,
        compounding: Compounding | str | None = None,
        day_count: DayCount | None = None,
    ) -> None:
        """
        Create a rate binding specification.

        Parameters
        ----------
        node_id : str
            Statement rate node identifier.
        curve_id : str
            Market curve identifier (e.g. ``"USD-OIS"``).
        tenor : str
            Tenor string (e.g. ``"5Y"``); parsed eagerly by :meth:`validate`.
        compounding : Compounding | str, optional
            Output compounding convention, typed or as its wire label
            (``"continuous"``, ``"annual"``, ...). Defaults to continuous.
            The extracted rate stays a decimal annualized rate.
        day_count : DayCount, optional
            Output quote day-count convention; ``None`` uses the curve default.
            Maturity dates and curve-implied accumulation factors are preserved.

        Raises
        ------
        ValueError
            If ``compounding`` is not an accepted label.
        """
        ...

    def validate(self) -> None:
        """
        Validate identifiers and eagerly parse the tenor.

        Raises
        ------
        ValueError
            If ``node_id`` or ``curve_id`` is blank, or ``tenor`` is not a
            valid tenor string.

        Examples
        --------
        >>> from finstack_quant.scenarios import RateBindingSpec
        >>> RateBindingSpec("node_1", "USD-OIS", "5Y").validate() is None
        True
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare; non-``RateBindingSpec`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both bindings serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @property
    def node_id(self) -> str:
        """
        Statement rate node identifier.

        Returns
        -------
        str
            Node ID string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def curve_id(self) -> str:
        """
        Market curve identifier.

        Returns
        -------
        str
            Curve ID string (e.g. ``"USD-OIS"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenor(self) -> str:
        """
        Tenor label used when binding the rate (for example ``"5Y"``).

        Returns
        -------
        str
            Tenor label (e.g. ``"5Y"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def compounding(self) -> Compounding:
        """
        Compounding convention used when converting the bound rate.

        Returns
        -------
        Compounding
            Compounding enum value, or the curve default when not specified.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> DayCount | None:
        """
        Day-count convention used when converting the bound rate.

        Returns
        -------
        DayCount or None
            Typed day-count convention, or ``None`` when not specified.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this rate binding to a JSON-compatible dict.

        Returns
        -------
        str
            JSON-serialized ``RateBindingSpec``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RateBindingSpec:
        """
        Deserialize a ``RateBindingSpec`` from JSON.

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

        Examples
        --------
        >>> from finstack_quant.scenarios import RateBindingSpec
        >>> spec = RateBindingSpec.from_json(
        ...     '{"node_id":"node_1","curve_id":"USD-OIS","tenor":"5Y","compounding":"continuous","day_count":null}'
        ... )
        >>> spec.tenor
        '5Y'
        """
        ...

class HierarchyTarget:
    """
    Path into the market-data hierarchy, with an optional tag filter, that a
    hierarchy-targeted operation resolves against the execution context's
    ``MarketDataHierarchy``.

    Parameters
    ----------
    path : list[str]
        Hierarchy path from the root, e.g. ``["Credit", "US", "IG"]``; every
        curve in that subtree is targeted.
    tag_filter : dict[str, str] | None, default None
        ``{key: value}`` equality predicates (AND semantics) a node must
        satisfy for its subtree to be included. Use :meth:`from_json` for
        ``in`` / ``exists`` predicates.

    Raises
    ------
    TypeError
        If ``path`` is not a list of strings or ``tag_filter`` values are not
        strings.

    Examples
    --------
    >>> from finstack_quant.scenarios import HierarchyTarget
    >>> target = HierarchyTarget(["Credit", "US"], {"sector": "financials"})
    >>> target.path
    ['Credit', 'US']
    >>> HierarchyTarget.from_json(target.to_json()) == target
    True
    """

    def __init__(self, path: list[str], tag_filter: Mapping[str, str] | None = None) -> None: ...
    @property
    def path(self) -> list[str]:
        """
        Hierarchy path from the root.

        Returns
        -------
        list[str]
            Node labels from the root to the targeted subtree.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tag_filter(self) -> list[tuple[str, str]] | None:
        """
        Equality tag predicates, or ``None`` when no filter is set.

        Returns
        -------
        list[tuple[str, str]] or None
            ``(key, value)`` pairs for ``equals`` predicates; ``in`` /
            ``exists`` predicates are only visible via :meth:`to_json`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``{"path": [...], "tag_filter": {...}}``).

        Returns
        -------
        str
            Canonical ``HierarchyTarget`` JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HierarchyTarget:
        """
        Deserialize from canonical JSON, including ``in`` / ``exists`` tag
        predicates the constructor does not express.

        Parameters
        ----------
        json : str
            Canonical ``HierarchyTarget`` JSON.

        Returns
        -------
        HierarchyTarget
            Parsed target.

        Raises
        ------
        ValueError
            If the JSON does not match the ``HierarchyTarget`` contract.

        Examples
        --------
        >>> from finstack_quant.scenarios import HierarchyTarget
        >>> HierarchyTarget.from_json('{"path":["Credit"]}').path
        ['Credit']
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on path and tag filter.

        Parameters
        ----------
        other : object
            Value to compare; non-``HierarchyTarget`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both targets serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

class OperationSpec:
    """
    One shock, time roll, or binding inside a ``ScenarioSpec``.

    Each classmethod corresponds to one Rust ``OperationSpec`` variant;
    ``to_json()`` produces the canonical wire form. Units follow the
    constructor name:

    - ``*_pct`` fields are percentage points (``5.0`` = +5%).
    - ``*_bp`` fields are additive basis points (1 bp = 1e-4) — except on
      ``CurveKind.commodity()`` curves, where ``bp`` is **percent of the
      forward** (a commodity price curve has no rate to shift).
    - Vol-index ``*_pts`` are index points (``1.0`` on 18.5 → 19.5).
    - Correlation and base-correlation ``*_pts`` are **decimal correlation**
      (``0.02`` = +0.02, not percentage points).

    Every enum-valued argument accepts the typed wrapper or its snake-case
    label (``CurveKind.discount()`` or ``"discount"``).

    Examples
    --------
    >>> from finstack_quant.scenarios import OperationSpec, CurveKind
    >>> op = OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0)
    >>> op.kind
    'curve_parallel_bp'
    >>> op == OperationSpec.curve_parallel_bp("discount", "USD-OIS", 10.0)
    True
    """

    @classmethod
    def market_fx_pct(cls, base: str, quote: str, pct: float) -> OperationSpec:
        """
        FX rate percent shift (``pct = 5.0`` strengthens ``base`` by 5%).

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

        Raises
        ------
        ValueError
            If ``base`` or ``quote`` is not a recognized ISO currency code.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.market_fx_pct("USD", "EUR", -5.0).kind
        'market_fx_pct'
        """
        ...

    @classmethod
    def equity_price_pct(cls, ids: list[str], pct: float) -> OperationSpec:
        """
        Equity price percent shock applied to all supplied identifiers.

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

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.equity_price_pct(["SPY"], -10.0).kind
        'equity_price_pct'
        """
        ...

    @classmethod
    def instrument_price_pct_by_attr(
        cls, attrs: Mapping[str, str] | Sequence[tuple[str, str]], pct: float
    ) -> OperationSpec:
        """
        Instrument price percent shock by exact attribute match.

        Parameters
        ----------
        attrs : Mapping[str, str] | Sequence[tuple[str, str]]
            Attribute key-value pairs that must all match; insertion order is
            preserved. Must be non-empty (validated on apply).
        pct : float
            Percent shock applied to matched instruments (``-5.0`` = -5%).

        Returns
        -------
        OperationSpec
            The ``instrument_price_pct_by_attr`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_price_pct_by_attr({"sector": "tech"}, -5.0).kind
        'instrument_price_pct_by_attr'
        """
        ...

    @overload
    @classmethod
    def curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Parallel basis-point shift on a single named curve.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family: ``"discount"``, ``"forward"``, ``"par_cds"``,
            ``"inflation"`` or ``"commodity"``.
        curve_id : str
            Identifier of the one curve to shock, as registered in the
            market context (for example ``"USD-OIS"``).
        bp : float
            Additive shift in basis points applied to every node; for
            ``CurveKind.commodity()`` the value is **percent of the
            forward** rather than basis points.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS
            quotes. ``None`` (the default) leaves the curve's own
            discounting unchanged.

        Returns
        -------
        OperationSpec
            A single scenario operation describing the parallel shift.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, OperationSpec
        >>> OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0).kind
        'curve_parallel_bp'
        """
        ...
    @overload
    @classmethod
    def curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: list[str],
        bp: float,
        discount_curve_id: str | None = None,
    ) -> list[OperationSpec]:
        """
        Parallel basis-point shift expanded across several curves.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family: ``"discount"``, ``"forward"``, ``"par_cds"``,
            ``"inflation"`` or ``"commodity"``.
        curve_id : list[str]
            Identifiers of the curves to shock; one operation is produced
            per identifier, in the order given.
        bp : float
            Additive shift in basis points applied to every node of each
            curve; for ``CurveKind.commodity()`` the value is **percent of
            the forward** rather than basis points.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS
            quotes. ``None`` (the default) leaves discounting unchanged.

        Returns
        -------
        list[OperationSpec]
            One operation per entry of ``curve_id``, same length and order.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label or ``curve_id`` holds
            a non-string entry.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> len(OperationSpec.curve_parallel_bp("discount", ["USD-OIS", "EUR-OIS"], 10.0))
        2
        """
        ...
    @classmethod
    def curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: str | list[str],
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec | list[OperationSpec]:
        """
        Parallel basis-point shift on a curve.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family (``"discount"``, ``"forward"``, ``"par_cds"``,
            ``"inflation"``, ``"commodity"``).
        curve_id : str | list[str]
            One curve identifier, or several: a list expands to one operation
            per identifier (``ScenarioSpec::parallel_bp_many`` in Rust).
        bp : float
            Additive shift in basis points applied to every node; for
            ``CurveKind.commodity()`` this is **percent of the forward**.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS quotes.

        Returns
        -------
        OperationSpec | list[OperationSpec]
            A single operation for a ``str`` curve id, a list for a list.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label or ``curve_id`` is
            neither a string nor a list of strings.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, OperationSpec
        >>> OperationSpec.curve_parallel_bp(CurveKind.discount(), "USD-OIS", 10.0).kind
        'curve_parallel_bp'
        >>> len(OperationSpec.curve_parallel_bp("discount", ["USD-OIS", "EUR-OIS"], 10.0))
        2
        """
        ...

    @classmethod
    def curve_node_bp(
        cls,
        curve_kind: CurveKind | str,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | str | None = None,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Node-level basis-point shifts on a curve.

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family, typed or as its wire label.
        curve_id : str
            Curve identifier in ``MarketContext``.
        nodes : list[tuple[str, float]]
            List of ``(tenor, bp)`` pairs (percent of forward for commodity
            curves).
        match_mode : TenorMatchMode | str, optional
            Tenor alignment strategy (``"exact"`` or ``"interpolate"``).
            Defaults to exact matching.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS quotes.

        Returns
        -------
        OperationSpec
            The ``curve_node_bp`` operation.

        Raises
        ------
        ValueError
            If ``curve_kind`` or ``match_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, OperationSpec
        >>> OperationSpec.curve_node_bp(CurveKind.discount(), "USD-OIS", [("5Y", 10.0)]).kind
        'curve_node_bp'
        """
        ...

    @classmethod
    def vol_index_parallel_pts(cls, curve_id: str, points: float) -> OperationSpec:
        """
        Parallel shock to a volatility-index curve in absolute index points.

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

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_index_parallel_pts("VIX", 2.0).kind
        'vol_index_parallel_pts'
        """
        ...

    @classmethod
    def vol_index_node_pts(
        cls,
        curve_id: str,
        nodes: list[tuple[str, float]],
        match_mode: TenorMatchMode | str | None = None,
    ) -> OperationSpec:
        """
        Node-level shocks to a volatility-index curve in absolute index points.

        Parameters
        ----------
        curve_id : str
            Volatility-index curve identifier.
        nodes : list[tuple[str, float]]
            List of ``(tenor, points)`` pairs.
        match_mode : TenorMatchMode | str, optional
            Tenor alignment strategy (``"exact"`` or ``"interpolate"``).

        Returns
        -------
        OperationSpec
            The ``vol_index_node_pts`` operation.

        Raises
        ------
        ValueError
            If ``match_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_index_node_pts("VIX", [("1M", 2.0)]).kind
        'vol_index_node_pts'
        """
        ...

    @classmethod
    def base_corr_parallel_pts(cls, surface_id: str, points: float) -> OperationSpec:
        """
        Parallel base-correlation shift in decimal correlation.

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02, not
            percentage points).

        Returns
        -------
        OperationSpec
            The ``base_corr_parallel_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.base_corr_parallel_pts("CDX", 0.01).kind
        'base_corr_parallel_pts'
        """
        ...

    @classmethod
    def base_corr_bucket_pts(
        cls,
        surface_id: str,
        points: float,
        detachment_bp: list[int] | None = None,
    ) -> OperationSpec:
        """
        Bucketed base-correlation shock by detachment.

        Parameters
        ----------
        surface_id : str
            Base-correlation surface identifier.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02).
        detachment_bp : list[int], optional
            Detachment points (in bp) to target. ``None`` targets all.

        Returns
        -------
        OperationSpec
            The ``base_corr_bucket_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.base_corr_bucket_pts("CDX", 0.01).kind
        'base_corr_bucket_pts'
        """
        ...

    @classmethod
    def vol_surface_parallel_pct(cls, vol_surface_id: str, pct: float) -> OperationSpec:
        """
        Parallel percent shift to a volatility surface.

        Parameters
        ----------
        vol_surface_id : str
            Volatility-surface identifier.
        pct : float
            Percent shift applied to every vol quote.

        Returns
        -------
        OperationSpec
            The ``vol_surface_parallel_pct`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_surface_parallel_pct("SPX", 10.0).kind
        'vol_surface_parallel_pct'
        """
        ...

    @classmethod
    def vol_surface_bucket_pct(
        cls,
        vol_surface_id: str,
        pct: float,
        tenors: list[str] | None = None,
        strikes: list[float] | None = None,
    ) -> OperationSpec:
        """
        Bucketed volatility surface percent shock.

        Parameters
        ----------
        vol_surface_id : str
            Volatility-surface identifier.
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

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.vol_surface_bucket_pct("SPX", 10.0).kind
        'vol_surface_bucket_pct'
        """
        ...

    @classmethod
    def stmt_forecast_percent(cls, node_id: str, pct: float) -> OperationSpec:
        """
        Statement forecast percent change.

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

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.stmt_forecast_percent("revenue", 5.0).kind
        'stmt_forecast_percent'
        """
        ...

    @classmethod
    def stmt_forecast_assign(cls, node_id: str, value: float) -> OperationSpec:
        """
        Statement forecast value assignment.

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

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.stmt_forecast_assign("revenue", 100.0).kind
        'stmt_forecast_assign'
        """
        ...

    @classmethod
    def rate_binding(cls, binding: RateBindingSpec) -> OperationSpec:
        """
        Bind a statement rate node to a curve for the lifetime of the scenario.

        Parameters
        ----------
        binding : RateBindingSpec
            Rate binding configuration.

        Returns
        -------
        OperationSpec
            The ``rate_binding`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec, RateBindingSpec
        >>> binding = RateBindingSpec("revenue", "USD-OIS", "5Y")
        >>> OperationSpec.rate_binding(binding).kind
        'rate_binding'
        """
        ...

    @classmethod
    def instrument_spread_bp_by_attr(
        cls, attrs: Mapping[str, str] | Sequence[tuple[str, str]], bp: float
    ) -> OperationSpec:
        """
        Instrument spread shock (additive basis points) by exact attribute match.

        Parameters
        ----------
        attrs : Mapping[str, str] | Sequence[tuple[str, str]]
            Attribute key-value pairs that must all match; insertion order is
            preserved.
        bp : float
            Additive basis-point shift applied to matched instruments.

        Returns
        -------
        OperationSpec
            The ``instrument_spread_bp_by_attr`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_spread_bp_by_attr([("sector", "tech")], 20.0).kind
        'instrument_spread_bp_by_attr'
        """
        ...

    @classmethod
    def instrument_price_pct_by_type(cls, instrument_types: list[str], pct: float) -> OperationSpec:
        """
        Instrument price shock by ``InstrumentType`` (snake_case strings).

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

        Raises
        ------
        ValueError
            If any entry in ``instrument_types`` is not a recognized instrument type.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_price_pct_by_type(["bond"], -5.0).kind
        'instrument_price_pct_by_type'
        """
        ...

    @classmethod
    def instrument_spread_bp_by_type(cls, instrument_types: list[str], bp: float) -> OperationSpec:
        """
        Instrument spread shock by ``InstrumentType`` (snake_case strings).

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

        Raises
        ------
        ValueError
            If any entry in ``instrument_types`` is not a recognized instrument type.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.instrument_spread_bp_by_type(["bond"], 20.0).kind
        'instrument_spread_bp_by_type'
        """
        ...

    @classmethod
    def asset_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """
        Structured-credit asset-correlation shock in decimal correlation.

        Parameters
        ----------
        delta_pts : float
            Additive decimal correlation shift (``0.05`` adds 0.05 to the
            correlation). Requires instruments in the execution context.

        Returns
        -------
        OperationSpec
            The ``asset_correlation_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.asset_correlation_pts(0.05).kind
        'asset_correlation_pts'
        """
        ...

    @classmethod
    def prepay_default_correlation_pts(cls, delta_pts: float) -> OperationSpec:
        """
        Structured-credit prepay/default correlation shock in decimal
        correlation.

        Parameters
        ----------
        delta_pts : float
            Additive decimal correlation shift (``0.05`` adds 0.05). Requires
            instruments in the execution context.

        Returns
        -------
        OperationSpec
            The ``prepay_default_correlation_pts`` operation.

        Notes
        -----
        This factory constructs a spec object and does not raise; validation occurs when the spec is applied.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.prepay_default_correlation_pts(0.05).kind
        'prepay_default_correlation_pts'
        """
        ...

    @classmethod
    def hierarchy_curve_parallel_bp(
        cls,
        curve_kind: CurveKind | str,
        target: HierarchyTarget | str,
        bp: float,
        discount_curve_id: str | None = None,
    ) -> OperationSpec:
        """
        Hierarchy-targeted parallel curve shift (basis points; percent of
        forward for commodity curves).

        Parameters
        ----------
        curve_kind : CurveKind | str
            Curve family, typed or as its wire label.
        target : HierarchyTarget | str
            Typed target or its JSON string
            (``{"path": [...], "tag_filter": {...}}``).
        bp : float
            Additive basis-point shift applied to every node of every curve
            in the targeted subtree.
        discount_curve_id : str, optional
            Discount curve used when re-bootstrapping shocked ParCDS quotes.

        Returns
        -------
        OperationSpec
            The ``hierarchy_curve_parallel_bp`` operation.

        Raises
        ------
        ValueError
            If ``curve_kind`` is not an accepted label or ``target`` is not
            valid ``HierarchyTarget`` JSON.

        Examples
        --------
        >>> from finstack_quant.scenarios import CurveKind, HierarchyTarget, OperationSpec
        >>> OperationSpec.hierarchy_curve_parallel_bp(CurveKind.discount(), HierarchyTarget(["Credit"]), 10.0).kind
        'hierarchy_curve_parallel_bp'
        """
        ...

    @classmethod
    def hierarchy_vol_surface_parallel_pct(cls, target: HierarchyTarget | str, pct: float) -> OperationSpec:
        """
        Hierarchy-targeted vol-surface percent shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        pct : float
            Percent shift applied to matched vol quotes.

        Returns
        -------
        OperationSpec
            The ``hierarchy_vol_surface_parallel_pct`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_vol_surface_parallel_pct('{"path":["Equity"]}', 10.0).kind
        'hierarchy_vol_surface_parallel_pct'
        """
        ...

    @classmethod
    def hierarchy_equity_price_pct(cls, target: HierarchyTarget | str, pct: float) -> OperationSpec:
        """
        Hierarchy-targeted equity price percent shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        pct : float
            Percent shift applied to matched equity prices.

        Returns
        -------
        OperationSpec
            The ``hierarchy_equity_price_pct`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_equity_price_pct('{"path":["Equity"]}', -5.0).kind
        'hierarchy_equity_price_pct'
        """
        ...

    @classmethod
    def hierarchy_base_corr_parallel_pts(cls, target: HierarchyTarget | str, points: float) -> OperationSpec:
        """
        Hierarchy-targeted base-correlation parallel shift.

        Parameters
        ----------
        target : HierarchyTarget | str
            Typed target or its JSON string.
        points : float
            Additive decimal correlation shift (``0.02`` = +0.02).

        Returns
        -------
        OperationSpec
            The ``hierarchy_base_corr_parallel_pts`` operation.

        Raises
        ------
        ValueError
            If ``target`` is not valid JSON for a ``HierarchyTarget``.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.hierarchy_base_corr_parallel_pts('{"path":["Credit"]}', 0.01).kind
        'hierarchy_base_corr_parallel_pts'
        """
        ...

    @classmethod
    def time_roll_forward(
        cls,
        period: str,
        apply_shocks: bool = True,
        roll_mode: TimeRollMode | str | None = None,
    ) -> OperationSpec:
        """
        Roll the valuation horizon forward (e.g. ``"1M"``).

        ``apply_shocks`` defaults to ``True`` to mirror the Rust
        ``#[serde(default = "default_true")]`` attribute.

        Parameters
        ----------
        period : str
            Tenor string for the roll period (e.g. ``"1M"``, ``"3M"``,
            ``"1Y"``); rejected by :meth:`validate` / ``ScenarioSpec`` when
            it does not parse as a tenor.
        apply_shocks : bool, default True
            Whether to apply scenario shocks after the time roll.
        roll_mode : TimeRollMode | str, optional
            Calendar-vs-business-day roll mode (``"business_days"``,
            ``"calendar_days"``, ``"approximate"``). Defaults to business days.

        Returns
        -------
        OperationSpec
            The ``time_roll_forward`` operation.

        Raises
        ------
        ValueError
            If ``roll_mode`` is not an accepted label.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.time_roll_forward("1M").kind
        'time_roll_forward'
        """
        ...

    def validate(self) -> None:
        """
        Validate this operation with the canonical Rust rules.

        Raises
        ------
        ValueError
            If an identifier is empty, a numeric field is non-finite, a
            variant-specific floor is violated (FX ``pct <= -100``, price
            ``pct < -100``), or a tenor / time-roll period does not parse.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.time_roll_forward("3M").validate() is None
        True
        """
        ...

    def requires_instruments(self) -> bool:
        """
        Whether this operation needs instruments in the execution context.

        Returns
        -------
        bool
            ``True`` for instrument-scoped shocks and ``time_roll_forward``.

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.market_fx_pct("USD", "EUR", 1.0).requires_instruments()
        False
        """
        ...

    def mutates_instruments(self) -> bool:
        """
        Whether this operation can replace or mutate instruments.

        Returns
        -------
        bool
            ``True`` for instrument price, spread, and structured-credit
            correlation shocks; ``False`` otherwise (a time roll only reads).

        Raises
        ------
        None
            This method does not raise.

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.asset_correlation_pts(0.05).mutates_instruments()
        True
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on the variant and every field.

        Parameters
        ----------
        other : object
            Value to compare; non-``OperationSpec`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both operations serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON-serialized ``OperationSpec``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> OperationSpec:
        """
        Deserialize an ``OperationSpec`` from JSON.

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

        Examples
        --------
        >>> from finstack_quant.scenarios import OperationSpec
        >>> OperationSpec.from_json('{"kind":"market_fx_pct","base":"USD","quote":"EUR","pct":-5.0}').kind
        'market_fx_pct'
        """
        ...

    @property
    def kind(self) -> str:
        """
        Variant discriminator (the serde ``kind`` tag value).

        Returns
        -------
        str
            Snake-case operation kind string.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
