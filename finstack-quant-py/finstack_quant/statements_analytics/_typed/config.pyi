"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/typed/config.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements import StatementResult

class ParameterSpec:
    """
    One parameter to vary in a sensitivity run.

    Each perturbation replaces the node's value in ``period`` only; it does
    not re-anchor the node's forecast in later periods (see
    ``run_sensitivity``).

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    node_id : str
        Node identifier to perturb.
    period : str
        Period-id string of the perturbed value (e.g. ``"2025Q2"``).
    base_value : float
        Unperturbed value, recorded for reference.
    perturbations : list[float]
        Absolute replacement values applied one at a time.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ParameterSpec
    >>> ParameterSpec.with_percentages("revenue", "2025Q2", 100.0, [-10.0, 10.0]).perturbations
    [90.0, 110.00000000000001]
    """
    def __init__(self, node_id: str, period: str, base_value: float, perturbations: list[float]) -> None: ...
    @staticmethod
    def with_percentages(node_id: str, period: str, base_value: float, pct_range: list[float]) -> ParameterSpec:
        """
        Build a spec whose perturbations are ``base_value * (1 + pct / 100)``.

        Parameters
        ----------
        node_id : str
            Node identifier to perturb.
        period : str
            Period-id string of the perturbed value.
        base_value : float
            Unperturbed value the percentages are applied to.
        pct_range : list[float]
            Percentage bumps (``[-10.0, 0.0, 10.0]`` = -10%, 0%, +10%).

        Returns
        -------
        ParameterSpec
            Build a spec whose perturbations are ``base_value * (1 + pct / 100)``.

        Raises
        ------
        ValueError
            If ``period`` does not parse, ``base_value`` or a percentage is not
            finite (``NaN`` or infinite), or a converted shock overflows.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ParameterSpec
        >>> ParameterSpec.with_percentages("revenue", "2025Q1", 100.0, [-0.1, 0.1]).perturbations
        [99.9, 100.1]
        """
    @property
    def node_id(self) -> str:
        """
        Node identifier to perturb.

        This property does not raise.

        Returns
        -------
        str
            Node identifier to perturb.
        """
    @property
    def period(self) -> str:
        """
        Period-id string of the perturbed value.

        This property does not raise.

        Returns
        -------
        str
            Period-id string of the perturbed value.
        """
    @property
    def base_value(self) -> float:
        """
        Unperturbed value.

        This property does not raise.

        Returns
        -------
        float
            Unperturbed value.
        """
    @property
    def perturbations(self) -> list[float]:
        """
        Absolute replacement values applied one at a time.

        This property does not raise.

        Returns
        -------
        list[float]
            Absolute replacement values applied one at a time.
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
    def from_json(json: str) -> ParameterSpec:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ParameterSpec
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ParameterSpec`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ParameterSpec
        >>> ParameterSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class SensitivityConfig:
    """
    Configuration for statement sensitivity analysis.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    mode : str
        ``"diagonal"`` (one-at-a-time), ``"full_grid"`` or ``"tornado"``.
    parameters : list[ParameterSpec | tuple[str, str, float, list[float]]]
        Parameters to vary; tuples are ``(node_id, period, base_value,
        perturbations)`` with absolute replacement values.
    target_metrics : list[str]
        Node identifiers tracked across scenarios.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig
    >>> cfg = SensitivityConfig("diagonal", target_metrics=["profit"])
    >>> cfg.add_parameter("revenue", "2025Q2", 100.0, pct=[-10.0, 10.0])
    >>> cfg.parameters[0].perturbations
    [90.0, 110.00000000000001]
    """
    def __init__(self, mode: str, parameters: list[Any] = ..., target_metrics: list[str] = ...) -> None: ...
    def add_parameter(
        self,
        node_id: str,
        period: str,
        base_value: float,
        perturbations: list[float] | None = None,
        pct: list[float] | None = None,
    ) -> None:
        """Append a parameter to vary.

        Parameters
        ----------
        node_id : str
            Node identifier to perturb.
        period : str
            Period-id string of the perturbed value.
        base_value : float
            Unperturbed value.
        perturbations : list[float] | None
            Absolute replacement values.
        pct : list[float] | None
            Percentage bumps applied to ``base_value`` (``[-10.0, 10.0]`` =
            -10% / +10%). Exactly one of ``perturbations`` and ``pct`` must be
            given.

        Raises
        ------
        ValueError
            If ``period`` does not parse, neither/both of ``perturbations``
            and ``pct`` are supplied, or ``pct`` is given with a non-finite
            ``base_value`` or percentage or a shock that overflows.
        """
    @property
    def parameters(self) -> list[ParameterSpec]:
        """
        Configured parameters in insertion order.

        This property does not raise.

        Returns
        -------
        list[ParameterSpec]
            Configured parameters in insertion order.
        """
    @staticmethod
    def from_json(json: str) -> SensitivityConfig:
        """
        Rebuild a ``SensitivityConfig`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        SensitivityConfig
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``SensitivityConfig`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import SensitivityConfig
        >>> SensitivityConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``SensitivityConfig``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def mode(self) -> str:
        """
                Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the
        serde name, identical to ``to_json``).

                This property does not raise.

                Returns
                -------
                str
                    Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the serde name, identical to ``to_json``).
        """
    @property
    def target_metrics(self) -> list[str]:
        """
        Node identifiers of the statement metrics tracked across scenarios.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the statement metrics tracked across scenarios.
        """
    @property
    def parameter_count(self) -> int:
        """
        Number of configured parameters (one `ParameterSpec` per entry).

        This property does not raise.

        Returns
        -------
        int
            Number of configured parameters (one `ParameterSpec` per entry).
        """

class VarianceConfig:
    """
    Configuration for comparing two statement results.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    baseline_label : str
        Label reported for the baseline side of the comparison.
    comparison_label : str
        Label reported for the comparison side.
    metrics : list[str]
        Statement node ids compared, in report order.
    periods : list[str]
        Period ids (for example ``"2025Q1"``) the comparison covers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceConfig
    >>> VarianceConfig.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    def __init__(self, baseline_label: str, comparison_label: str, metrics: list[str], periods: list[str]) -> None: ...
    @staticmethod
    def from_json(json: str) -> VarianceConfig:
        """
        Rebuild a ``VarianceConfig`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        VarianceConfig
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``VarianceConfig`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import VarianceConfig
        >>> VarianceConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``VarianceConfig``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def baseline_label(self) -> str:
        """
        Label for the baseline scenario (e.g. ``"management_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the baseline scenario (e.g. ``"management_case"``).
        """
    @property
    def comparison_label(self) -> str:
        """
        Label for the comparison scenario (e.g. ``"bank_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the comparison scenario (e.g. ``"bank_case"``).
        """
    @property
    def metrics(self) -> list[str]:
        """
        Node identifiers of the metrics compared between the two scenarios.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the metrics compared between the two scenarios.
        """
    @property
    def periods(self) -> list[str]:
        """
        Periods to compare, as period-id strings (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        list[str]
            Periods to compare, as period-id strings (e.g. ``"2025Q1"``).
        """

class ScenarioSet:
    """
    Named scenario definitions for statement-model evaluation.

    Parameters
    ----------
    scenarios : dict[str, dict[str, float | Money | dict[str, float | Money] | ForecastSpec]]
        Scenario name to overrides. Each override value is a model-wide value
        applied to every forecast period (``{"revenue": 90.0}``), a
        ``{period: value}`` dict applied to the named forecast periods only
        (``{"growth": {"2025Q3": 0.02}}``), or a ``ForecastSpec`` that
        replaces the node's forecast (``{"revenue": ForecastSpec.growth(0.02)}``):
        its method and every parameter, such as curves, seeds and
        volatilities. Per-period values win over a model-wide value for that
        period, and explicit values win over the forecast. Across a parent
        chain the child's forecast replaces the parent's for the same node.
    parents : dict[str, str] | None
        Optional scenario name to parent scenario name for inheritance.

    Raises
    ------
    ValueError
        If a scenario override cannot be read or a parent name is not a string.

    Examples
    --------
    >>> from finstack_quant.statements import ForecastSpec
    >>> from finstack_quant.statements_analytics import ScenarioSet
    >>> ScenarioSet({"base": {}, "down": {"revenue": {"2025Q2": 90.0}}}, parents={"down": "base"}).names
    ['base', 'down']
    >>> ScenarioSet({"slow": {"revenue": ForecastSpec.growth(0.01)}}).names
    ['slow']
    """
    def __init__(self, scenarios: Any, parents: Any | None = None) -> None: ...
    @staticmethod
    def from_json(json: str) -> ScenarioSet:
        """
        Rebuild a ``ScenarioSet`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioSet
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``ScenarioSet`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioSet
        >>> ScenarioSet.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``ScenarioSet``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def names(self) -> list[str]:
        """
        Scenario names in definition (insertion) order.

        This property does not raise.

        Returns
        -------
        list[str]
            Scenario names in definition (insertion) order.
        """
    def trace(self, scenario: str) -> list[str]:
        """Resolve a scenario's inheritance lineage, root-first.

        Parameters
        ----------
        scenario : str
            Name of the scenario to trace.

        Returns
        -------
        list[str]
            Scenario names from the root ancestor through to `scenario`.

        Raises
        ------
        ValueError
            If the scenario is unknown or its parent chain contains a cycle.
        """
