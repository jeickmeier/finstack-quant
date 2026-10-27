"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/spec.rs``."""

from __future__ import annotations

import datetime
from typing import Literal
from finstack_quant.scenarios._operation_spec.spec import OperationSpec

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
        par spreads; ``first_order_shift`` applies ``delta_hazard = delta_spread / (1 - recovery)``
        and reports an approximation warning.

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
            ``first_order_shift`` applies ``delta_hazard = delta_spread / (1 - recovery)``
            and reports an approximation warning.

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
