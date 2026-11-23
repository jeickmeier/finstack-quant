"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/typed/results.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements import StatementResult
from finstack_quant.statements_analytics._typed.config import ParameterSpec, ScenarioSet, SensitivityConfig

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
    def to_arrow(self, metrics: list[str] | None = None) -> ArrowTable:
        """
        Export the run as a long ``ArrowTable``.

        Columns: ``scenario`` (0-based index), one column per perturbed
        parameter named ``node_id@period`` (null where a scenario does not
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
        ArrowTable
            Long-format sensitivity table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self, metrics: list[str] | None = None) -> pd.DataFrame:
        """
        Export the run as a long pandas ``DataFrame``.

        Parameters
        ----------
        metrics : list[str] | None
            Node identifiers to emit; ``None`` uses the config's
            ``target_metrics``.

        Returns
        -------
        pd.DataFrame
            Long-format sensitivity DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self, metrics: list[str] | None = None) -> pl.DataFrame:
        """
        Export the run as a long Polars ``DataFrame``.

        Parameters
        ----------
        metrics : list[str] | None
            Node identifiers to emit; ``None`` uses the config's
            ``target_metrics``.

        Returns
        -------
        polars.DataFrame
            Long-format sensitivity Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self, metrics: list[str] | None = None) -> Any:
        """
        Export the run as a long ``pyarrow.Table``.

        Parameters
        ----------
        metrics : list[str] | None
            Node identifiers to emit; ``None`` uses the config's
            ``target_metrics``.

        Returns
        -------
        pyarrow.Table
            Long-format sensitivity PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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
    def to_arrow(self) -> ArrowTable:
        """
        Export the variance rows as an ``ArrowTable``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``. One row per
        (metric, period) pair, in report order; an empty report still carries
        the full column schema.

        ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own
        units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is
        null where the baseline is effectively zero.

        Returns
        -------
        ArrowTable
            Variance rows table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the variance rows as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Returns
        -------
        pd.DataFrame
            Variance rows DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the variance rows as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Variance rows Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the variance rows as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Variance rows PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...

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
    def to_comparison_table(self, metrics: list[str]) -> ArrowTable:
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
    def to_arrow(self, metrics: list[str]) -> ArrowTable:
        """
        Export the scenario comparison as an ``ArrowTable``.

        Columns: ``period``, ``metric``, one column per scenario name holding
        that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac``
        column per non-baseline scenario holding the relative change as a
        decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The
        ``_frac`` suffix states the unit: multiply by 100 for percent.
        One row per (metric, period) pair.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        ArrowTable
            Scenario comparison table with one row per (metric, period) pair.

        Raises
        ------
        ValueError
            If the result set or ``metrics`` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
        """
    def to_pandas(self, metrics: list[str]) -> pd.DataFrame:
        """
        Export the scenario comparison as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        pd.DataFrame
            Scenario comparison DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result set or ``metrics`` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
        """
    def to_polars(self, metrics: list[str]) -> pl.DataFrame:
        """
        Export the scenario comparison as a Polars ``DataFrame``.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        polars.DataFrame
            Scenario comparison Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result set or ``metrics`` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
        """
    def to_pyarrow(self, metrics: list[str]) -> Any:
        """
        Export the scenario comparison as a ``pyarrow.Table``.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        pyarrow.Table
            Scenario comparison PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result set or ``metrics`` is empty.
        KeyError
            If a metric in ``metrics`` is not a node of the baseline scenario.
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
    def to_arrow(self) -> ArrowTable:
        """
        Export the underlying variance rows as an ``ArrowTable``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report
        order; an empty diff still carries the full column schema.

        This is the same table as ``variance.to_arrow()`` — both call one
        implementation, so the two cannot drift apart.

        Returns
        -------
        ArrowTable
            Underlying variance rows table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the underlying variance rows as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Returns
        -------
        pd.DataFrame
            Underlying variance rows DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the underlying variance rows as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Underlying variance rows Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the underlying variance rows as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Underlying variance rows PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...

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
    def to_arrow(self) -> ArrowTable:
        """
        Export the driver steps as an ``ArrowTable``.

        Columns: ``driver``, ``contribution``. One row per bridge step, in
        decomposition order; an empty bridge still carries both columns.
        Contributions are raw deltas in each driver's own units.

        Returns
        -------
        ArrowTable
            Bridge steps table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the driver steps as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Returns
        -------
        pd.DataFrame
            Bridge steps DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the driver steps as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Bridge steps Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the driver steps as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Bridge steps PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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
