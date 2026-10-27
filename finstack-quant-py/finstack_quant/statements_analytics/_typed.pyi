"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/typed.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.statements import StatementResult

class BridgeChart:
    """
    Bridge decomposition of a metric's variance across named drivers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import BridgeChart
    >>> BridgeChart.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> BridgeChart:
        """
        Rebuild a ``BridgeChart`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        BridgeChart
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``BridgeChart`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import BridgeChart
        >>> BridgeChart.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``BridgeChart``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def target_metric(self) -> str:
        """
                Node identifier of the metric this bridge decomposes (e.g.
        ``"ebitda"``).

                This property does not raise.

                Returns
                -------
                str
                    Node identifier of the metric this bridge decomposes (e.g. ``"ebitda"``).
        """
    @property
    def period(self) -> str:
        """
        Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        str
            Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).
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
    def baseline_value(self) -> float:
        """
        Target-metric value in the baseline scenario, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Target-metric value in the baseline scenario, in the metric's units.
        """
    @property
    def comparison_value(self) -> float:
        """
        Target-metric value in the comparison scenario, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Target-metric value in the comparison scenario, in the metric's units.
        """
    @property
    def steps(self) -> list[BridgeStep]:
        """
        Ordered driver contributions making up the bridge.

        This property does not raise.

        Returns
        -------
        list[BridgeStep]
            Ordered driver contributions making up the bridge.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the driver steps as a pandas ``DataFrame``.

        Columns: ``driver``, ``contribution``. One row per bridge step, in
        decomposition order; an empty bridge still carries both columns.
        Contributions are raw deltas in each driver's own units.

        The scalar header fields (``target_metric``, ``period``,
        ``baseline_label``, ``comparison_label``, ``baseline_value``,
        ``comparison_value``, ``unexplained``) are chart metadata and are not
        repeated on every row.

        Returns
        -------
        pd.DataFrame
            Export the driver steps as a pandas ``DataFrame``. Columns: ``driver``, ``contribution``. One row per bridge step, in decomposition order; an empty bridge still carries both columns. Contributions are raw deltas in each driver's own units. The scalar header fields (``target_metric``, ``period``, ``baseline_label``, ``comparison_label``, ``baseline_value``, ``comparison_value``, ``unexplained``) are chart metadata and are not repeated on every row.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    @property
    def unexplained(self) -> float:
        """
                Residual variance not explained by the driver deltas.

        Driver contributions are raw deltas in driver units rather than
        sensitivities of the target metric, so they generally do not sum to
        the target variance. This term makes that gap explicit.

                This property does not raise.

                Returns
                -------
                float
                    Residual variance not explained by the driver deltas. Driver contributions are raw deltas in driver units rather than sensitivities of the target metric, so they generally do not sum to the target variance. This term makes that gap explicit.
        """

class BridgeStep:
    """
    One driver step in a bridge decomposition.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import BridgeStep
    >>> [field for field in ("driver", "contribution") if hasattr(BridgeStep, field)]
    ['driver', 'contribution']
    """
    @property
    def driver(self) -> str:
        """
        Driver node identifier (e.g. ``"revenue"``).

        This property does not raise.

        Returns
        -------
        str
            Driver node identifier (e.g. ``"revenue"``).
        """
    @property
    def contribution(self) -> float:
        """
                This driver's raw delta between the two scenarios, in the *driver's*
        own units.

        Contributions are not sensitivities of the target metric, so they
        generally do not sum to the target variance — see
        ``BridgeChart.unexplained``.

                This property does not raise.

                Returns
                -------
                float
                    This driver's raw delta between the two scenarios, in the *driver's* own units. Contributions are not sensitivities of the target metric, so they generally do not sum to the target variance — see ``BridgeChart.unexplained``.
        """

class ParameterSpec:
    """
    One parameter to vary in a sensitivity run.

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
            If ``period`` does not parse.

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

class ScenarioDiff:
    """
    Variance between two named scenarios in an evaluated scenario set.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScenarioDiff
    >>> ScenarioDiff.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> ScenarioDiff:
        """
        Deserialize a scenario diff from its canonical JSON form.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioDiff
            Deserialize a scenario diff from its canonical JSON form.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioDiff
        >>> ScenarioDiff.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``baseline``, ``comparison``, ``variance``).

        Returns
        -------
        str
            Serialize to canonical JSON (``baseline``, ``comparison``, ``variance``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def baseline(self) -> str:
        """
        Name of the scenario used as the baseline of the diff.

        This property does not raise.

        Returns
        -------
        str
            Name of the scenario used as the baseline of the diff.
        """
    @property
    def comparison(self) -> str:
        """
        Name of the scenario compared against the baseline.

        This property does not raise.

        Returns
        -------
        str
            Name of the scenario compared against the baseline.
        """
    @property
    def variance(self) -> VarianceReport:
        """
        Underlying variance report between the two named scenarios.

        This property does not raise.

        Returns
        -------
        VarianceReport
            Underlying variance report between the two named scenarios.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the underlying variance rows as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report
        order; an empty diff still carries the full column schema.

        This is the same table as ``variance.to_dataframe()`` — both call one
        implementation, so the two cannot drift apart. The two scenario *names*
        are diff metadata (the ``baseline`` / ``comparison`` getters) and are
        not repeated per row; the ``baseline`` and ``comparison`` columns hold
        the metric *values* in each scenario.

        Returns
        -------
        pd.DataFrame
            Export the underlying variance rows as a pandas ``DataFrame``. Columns: ``period``, ``metric``, ``baseline``, ``comparison``, ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report order; an empty diff still carries the full column schema. This is the same table as ``variance.to_dataframe()`` — both call one implementation, so the two cannot drift apart. The two scenario *names* are diff metadata (the ``baseline`` / ``comparison`` getters) and are not repeated per row; the ``baseline`` and ``comparison`` columns hold the metric *values* in each scenario.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class ScenarioResults:
    """
    Typed evaluated results for a set of named scenarios.

    Named after the canonical Rust type
    (`finstack_quant_statements_analytics::analysis::ScenarioResults`).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScenarioResults
    >>> ScenarioResults.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> ScenarioResults:
        """
        Rebuild a ``ScenarioResults`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioResults
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``ScenarioResults`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioResults
        >>> ScenarioResults.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``ScenarioResults``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def names(self) -> list[str]:
        """
        Evaluated scenario names, in the order the scenario set defined them.

        This property does not raise.

        Returns
        -------
        list[str]
            Evaluated scenario names, in the order the scenario set defined them.
        """
    def get(self, name: str) -> StatementResult | None:
        """
        Look up one scenario result by scenario name.

        Parameters
        ----------
        name : str
            Scenario name as declared in the evaluated ``ScenarioSet``.

        Returns
        -------
        StatementResult | None
            Evaluated statements for that scenario, or ``None`` when no
            scenario carries the name.

        This lookup returns ``None`` for an unknown name and does not raise.
        """
    def to_comparison_table(self, metrics: list[str]) -> Any:
        """Build a side-by-side comparison table across every evaluated scenario.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        ArrowTable
            One column per scenario, one row per (metric, period).

        Raises
        ------
        ValueError
            If the result set or `metrics` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
        """
    def to_dataframe(self, metrics: list[str]) -> pd.DataFrame:
        """
        Export the scenario comparison as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, one column per scenario name holding
        that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac``
        column per non-baseline scenario holding the relative change as a
        decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The
        ``_frac`` suffix states the unit: multiply by 100 for percent.
        One row per (metric, period) pair.

        This is the same table as ``to_comparison_table`` — both call one Rust
        implementation, so the two exports cannot drift apart. The baseline is
        the scenario named ``"base"`` when present, otherwise the first
        scenario.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        pd.DataFrame
            Export the scenario comparison as a pandas ``DataFrame``. Columns: ``period``, ``metric``, one column per scenario name holding that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac`` column per non-baseline scenario holding the relative change as a decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The ``_frac`` suffix states the unit: multiply by 100 for percent. One row per (metric, period) pair. This is the same table as ``to_comparison_table`` — both call one Rust implementation, so the two exports cannot drift apart. The baseline is the scenario named ``"base"`` when present, otherwise the first scenario.

        Raises
        ------
        ValueError
            If the result set or ``metrics`` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
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
            If ``period`` does not parse or neither/both of ``perturbations``
            and ``pct`` are supplied.
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

class SensitivityResult:
    """
    Typed root result for statement sensitivity analysis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import SensitivityResult
    >>> SensitivityResult.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> SensitivityResult:
        """
        Rebuild a ``SensitivityResult`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        SensitivityResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``SensitivityResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import SensitivityResult
        >>> SensitivityResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``SensitivityResult``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def target_metrics(self) -> list[str]:
        """
        Node identifiers of the metrics tracked by the originating config.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the metrics tracked by the originating config.
        """
    @property
    def config(self) -> SensitivityConfig:
        """
        Configuration the run was generated from.

        This property does not raise.

        Returns
        -------
        SensitivityConfig
            Configuration the run was generated from.
        """
    @property
    def baseline(self) -> StatementResult | None:
        """
        Unperturbed baseline evaluation retained by every sensitivity run.

        This property does not raise.

        Returns
        -------
        StatementResult | None
            Unperturbed model evaluation, or ``None`` when absent from an imported
            result document.
        """
    @property
    def scenarios(self) -> list[tuple[Any, StatementResult]]:
        """
                Per-scenario ``(parameter_values, results)`` pairs in generation order,
        where ``parameter_values`` is a ``{"node_id@period": value}`` dict.

                This property does not raise.

                Returns
                -------
                list[tuple[Any, StatementResult]]
                    Per-scenario ``(parameter_values, results)`` pairs in generation order, where ``parameter_values`` is a ``{"node_id@period": value}`` dict.
        """
    def to_dataframe(self, metrics: list[str] | None = None) -> pd.DataFrame:
        """
        Export the run as a long pandas ``DataFrame``.

        Columns: ``scenario`` (0-based index), one column per perturbed
        parameter named ``node_id@period`` (``NaN`` where a scenario does not
        perturb it), then ``node_id``, ``period`` and ``value`` holding each
        tracked metric's value in that scenario. One row per (scenario, metric,
        period); when no metric is tracked, one row per scenario with the
        metric columns null so the parameter grid stays visible.

        Parameters
        ----------
        metrics : list[str] | None
            Node identifiers to emit; ``None`` uses the config's
            ``target_metrics``.
        Returns
        -------
        pd.DataFrame
            Export the run as a long pandas ``DataFrame``. Columns: ``scenario`` (0-based index), one column per perturbed parameter named ``node_id@period`` (``NaN`` where a scenario does not perturb it), then ``node_id``, ``period`` and ``value`` holding each tracked metric's value in that scenario. One row per (scenario, metric, period); when no metric is tracked, one row per scenario with the metric columns null so the parameter grid stays visible.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def get_parameter_value(self, scenario_index: int, parameter: str) -> float | None:
        """
        Read the perturbed parameter value used by one sensitivity scenario.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the scenario in ``scenarios``.
        parameter : str
            Parameter name as declared on the ``ParameterSpec``.

        Returns
        -------
        float | None
            Value applied to that parameter, or ``None`` when the scenario
            did not perturb it.

        Raises
        ------
        ValueError
            If ``scenario_index`` is outside the evaluated scenario range.
        """
    def get_value(self, scenario_index: int, node_id: str, period: str) -> float | None:
        """
        Read one evaluated node value from a sensitivity scenario.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the scenario in ``scenarios``.
        node_id : str
            Statement node whose value is read.
        period : str
            Period id such as ``"2025Q4"``.

        Returns
        -------
        float | None
            Node value in that scenario and period, or ``None`` when the
            node was not evaluated there.

        Raises
        ------
        ValueError
            If ``scenario_index`` is outside the evaluated scenario range, or
            ``period`` is not a parsable period id.
        """

class TornadoEntry:
    """One parameter's downside and upside impact in a tornado chart.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import TornadoEntry
    >>> entry = TornadoEntry.from_json('{"parameter_id":"revenue","downside":-5.0,"upside":7.0}')
    >>> entry.swing
    12.0
    """
    @staticmethod
    def from_json(json: str) -> TornadoEntry:
        """
        Deserialize one tornado entry from canonical JSON.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        TornadoEntry
            Deserialize one tornado entry from canonical JSON.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TornadoEntry
        >>> TornadoEntry.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize this entry to canonical JSON.

        Returns
        -------
        str
            Serialize this entry to canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def parameter_id(self) -> str:
        """
        Statement sensitivity key in ``node@period`` form; DCF entries use
        assumption identifiers such as ``wacc`` or ``g``.

        This property does not raise.

        Returns
        -------
        str
            Perturbed node and period, or a DCF assumption identifier.
        """
    @property
    def downside(self) -> float:
        """
        Metric change at the parameter's minimum perturbation.

        This property does not raise.

        Returns
        -------
        float
            Metric change at the parameter's minimum perturbation.
        """
    @property
    def upside(self) -> float:
        """
        Metric change at the parameter's maximum perturbation.

        This property does not raise.

        Returns
        -------
        float
            Metric change at the parameter's maximum perturbation.
        """
    @property
    def swing(self) -> float:
        """
        Total swing magnitude, calculated as `upside - downside`.

        This property does not raise.

        Returns
        -------
        float
            Total swing magnitude, calculated as `upside - downside`.
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

class VarianceReport:
    """
    Typed root variance report.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceReport
    >>> VarianceReport.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> VarianceReport:
        """
        Rebuild a ``VarianceReport`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        VarianceReport
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``VarianceReport`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import VarianceReport
        >>> VarianceReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``VarianceReport``, accepted by
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
    def rows(self) -> list[VarianceRow]:
        """
        Per-metric, per-period variance rows, in report order.

        This property does not raise.

        Returns
        -------
        list[VarianceRow]
            Per-metric, per-period variance rows, in report order.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the variance rows as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``. One row per
        (metric, period) pair, in report order; an empty report still carries
        the full column schema.

        ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own
        units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is
        ``NaN`` where the baseline is effectively zero. The scenario labels are report metadata
        (``baseline_label`` / ``comparison_label``) and are not repeated per row.

        Returns
        -------
        pd.DataFrame
            Export the variance rows as a pandas ``DataFrame``. Columns: ``period``, ``metric``, ``baseline``, ``comparison``, ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report order; an empty report still carries the full column schema. ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is ``NaN`` where the baseline is effectively zero. The scenario labels are report metadata (``baseline_label`` / ``comparison_label``) and are not repeated per row.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class VarianceRow:
    """
    One typed variance-report row.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceRow
    >>> [field for field in ("period", "metric") if hasattr(VarianceRow, field)]
    ['period', 'metric']
    """
    @property
    def period(self) -> str:
        """
        Period this row covers, as a period-id string (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        str
            Period this row covers, as a period-id string (e.g. ``"2025Q1"``).
        """
    @property
    def metric(self) -> str:
        """
        Node identifier of the compared metric.

        This property does not raise.

        Returns
        -------
        str
            Node identifier of the compared metric.
        """
    @property
    def baseline(self) -> float:
        """
        Metric value in the baseline scenario, in the metric's own units.

        This property does not raise.

        Returns
        -------
        float
            Metric value in the baseline scenario, in the metric's own units.
        """
    @property
    def comparison(self) -> float:
        """
        Metric value in the comparison scenario, in the metric's own units.

        This property does not raise.

        Returns
        -------
        float
            Metric value in the comparison scenario, in the metric's own units.
        """
    @property
    def abs_var(self) -> float:
        """
        Absolute variance ``comparison - baseline``, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Absolute variance ``comparison - baseline``, in the metric's units.
        """
    @property
    def pct_var(self) -> float | None:
        """
                Percentage variance ``abs_var / baseline`` as a decimal fraction
        (``0.1`` = +10%).

        ``None`` when the baseline is effectively zero, where a ratio would be
        undefined rather than zero; fall back to ``abs_var`` in that case.

                This property does not raise.

                Returns
                -------
                float | None
                    Percentage variance ``abs_var / baseline`` as a decimal fraction (``0.1`` = +10%). ``None`` when the baseline is effectively zero, where a ratio would be undefined rather than zero; fall back to ``abs_var`` in that case.
        """
