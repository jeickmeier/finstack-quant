"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/analysis/explain.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from datetime import date
from typing import Any, Literal, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.statements import FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable, LabeledSeries
from finstack_quant.statements_analytics._typed.results import (
    BridgeChart,
    ScenarioDiff,
    ScenarioResults,
    SensitivityResult,
    TornadoEntry,
    VarianceReport,
)

class DependencyTracer:
    """
    Reusable dependency tracer for a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import DependencyTracer
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> _ = builder.value("cost", [("2025Q1", 60.0)])
    >>> _ = builder.compute("profit", "revenue - cost")
    >>> sorted(DependencyTracer(builder.build()).direct_dependencies("profit"))
    ['cost', 'revenue']

    """

    def __init__(self, model: FinancialModelSpec | str) -> None:
        """
        Create a dependency tracer for the given model.

        Parameters
        ----------
        model : FinancialModelSpec or str
            ``FinancialModelSpec`` object or JSON string.

        Raises
        ------
        ValueError
            If model JSON is malformed or its dependency graph is invalid.

        """
        ...
    def dependency_tree(self, node_id: str) -> DependencyTree:
        """
        Return the dependency tree for ``node_id`` (Rust ``DependencyTracer::dependency_tree``).

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        DependencyTree
            Typed tree rooted at ``node_id`` with its complete upstream
            dependency hierarchy; ``to_json()`` matches the WASM
            ``DependencyTracer.dependencyTree`` result.

        Raises
        ------
        KeyError
            If ``node_id`` or a reachable dependency is not in the model.
        ValueError
            If the dependency graph is invalid.

        """
        ...
    def dependency_tree_text(self, node_id: str) -> str:
        """
        Return the ASCII rendering of ``node_id``'s dependency tree.

        The root is on the first line; every dependency follows on its own
        line behind ``├──`` / ``└──`` connectors, indented by depth, with its
        formula in parentheses. Identical to the WASM ``DependencyTracer.dependencyTreeText``.

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        str
            Multi-line ASCII tree ending with a newline.

        Raises
        ------
        KeyError
            If ``node_id`` or a reachable dependency is not in the model.
        ValueError
            If the dependency graph is invalid.

        """
        ...

    def dependency_tree_detailed(self, results: StatementResult | str, node_id: str, period: str) -> str:
        """
        Return an ASCII dependency tree annotated with values for one period.

        Parameters
        ----------
        results : StatementResult or str
            Statement results to annotate with.
        node_id : str
            Root node to trace.
        period : str
            Period to annotate.

        Returns
        -------
        str
            Multi-line ASCII dependency tree whose nodes are annotated with
            values from ``results`` for ``period``.

        Raises
        ------
        ValueError
            If results JSON or period is invalid, or node_id cannot be traced.

        """
        ...

    def direct_dependencies(self, node_id: str) -> list[str]:
        """
        List immediate dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose directly referenced inputs are requested.

        Returns
        -------
        list[str]
            Statement node IDs referenced directly by ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...
    def all_dependencies(self, node_id: str) -> list[str]:
        """
        List all transitive dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose complete upstream dependency set is requested.

        Returns
        -------
        list[str]
            Direct and transitive upstream node IDs in dependency order, with
            dependencies preceding their dependents.

        Raises
        ------
        ValueError
            If node_id is unknown or the dependency traversal fails.

        """
        ...
    def dependents(self, node_id: str) -> list[str]:
        """
        List nodes that depend on ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose downstream dependents are requested.

        Returns
        -------
        list[str]
            Statement node IDs that directly depend on ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...

class DependencyTree:
    """
    Dependency tree of one statement node (the Rust ``DependencyTree``).

    Returned by :meth:`DependencyTracer.dependency_tree`; its JSON form is the
    WASM ``DependencyTracer.dependencyTree`` result.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DependencyTree
    >>> tree = DependencyTree.from_json(
    ...     '{"node_id":"profit","formula":"revenue * 0.5","children":'
    ...     '[{"node_id":"revenue","formula":null,"children":[]}]}'
    ... )
    >>> tree.node_id, tree.formula, [child.node_id for child in tree.children]
    ('profit', 'revenue * 0.5', ['revenue'])
    """
    @property
    def node_id(self) -> str:
        """
        Node identifier.

        A dependency already on the current path appears as a leaf named
        ``"<id> (cycle)"``. This property does not raise.

        Returns
        -------
        str
            Node identifier of this tree's root.
        """
    @property
    def formula(self) -> str | None:
        """
        Formula text of the node.

        This property does not raise.

        Returns
        -------
        str | None
            Formula text when the node is calculated, ``None`` for a value node.
        """
    @property
    def children(self) -> list[DependencyTree]:
        """
        Direct dependencies of the node.

        This property does not raise.

        Returns
        -------
        list[DependencyTree]
            One tree per direct dependency, in formula reference order.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding (``node_id``, ``formula``, ``children``),
            accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> DependencyTree:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json` or the WASM
            ``DependencyTracer.dependencyTree`` result; unknown fields are rejected.

        Returns
        -------
        DependencyTree
            The tree decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``DependencyTree`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import DependencyTree
        >>> DependencyTree.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ExplanationStep:
    """
    One component of a formula explanation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ExplanationStep
    >>> [field for field in ("component", "value") if hasattr(ExplanationStep, field)]
    ['component', 'value']
    """
    @property
    def component(self) -> str:
        """
        Component node id or literal text.

        This property does not raise.

        Returns
        -------
        str
            Component node id or literal text.
        """
    @property
    def value(self) -> float:
        """
        Component value at the explained period.

        This property does not raise.

        Returns
        -------
        float
            Component value at the explained period.
        """
    @property
    def operation(self) -> str | None:
        """
        Sign with which the component enters the formula.

        Set to ``"+"`` or ``"-"`` only when the formula is a pure
        sum/difference of references, in which case the signed step values sum
        to the explained value.

        This property does not raise.

        Returns
        -------
        str | None
            ``"+"`` or ``"-"``, or ``None`` for any other formula.
        """

class Explanation:
    """Explanation of how a node's value was derived at one period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Explanation
    >>> e = Explanation.from_json(
    ...     '{"node_id":"profit","period_id":"2025Q1","final_value":50.0,'
    ...     '"node_type":"calculated","formula_text":"revenue * 0.5","breakdown":[]}'
    ... )
    >>> e.node_id, e.final_value
    ('profit', 50.0)
    """
    @property
    def node_id(self) -> str:
        """
        Explained node id.

        This property does not raise.

        Returns
        -------
        str
            Explained node id.
        """
    @property
    def period_id(self) -> str:
        """
        Period-id string of the explanation.

        This property does not raise.

        Returns
        -------
        str
            Period-id string of the explanation.
        """
    @property
    def final_value(self) -> float:
        """
        Node value at the period.

        This property does not raise.

        Returns
        -------
        float
            Node value at the period.
        """
    @property
    def node_type(self) -> str:
        """
        Node type serde name (e.g. ``"calculated"``, ``"input"``).

        This property does not raise.

        Returns
        -------
        str
            Node type serde name (e.g. ``"calculated"``, ``"input"``).
        """
    @property
    def source(self) -> str | None:
        """
        Value source selected for the cell under the
        ``Value > Forecast > Formula`` precedence: ``"value"`` (the explicit
        value stored on the node), ``"forecast"`` or ``"formula"``. ``None``
        for a masked or missing cell, which has no source.

        This property does not raise.

        Returns
        -------
        str | None
            The source label, or ``None`` when the results carry no source for the cell (hand-built results, or JSON written before sources were recorded).
        """
    @property
    def forecast(self) -> dict[str, Any] | None:
        """
        The node's forecast specification when ``source`` is ``"forecast"``.

        The dict is the serde form ``{"method": ..., "params": {...}}``, for
        example ``{"method": "growth_pct", "params": {"rate": 0.05}}``.

        This property does not raise.

        Returns
        -------
        dict[str, Any] | None
            The forecast spec, or ``None`` for every other source.
        """
    @property
    def formula_text(self) -> str | None:
        """
        Formula text, or ``None`` for non-formula nodes.

        This property does not raise.

        Returns
        -------
        str | None
            Formula text, or ``None`` for non-formula nodes.
        """
    @property
    def breakdown(self) -> list[ExplanationStep]:
        """
        Formula components with the values they resolved to at the period,
        including ``cs.<component>.<instrument_or_total>`` capital-structure
        references.

        Populated only when the formula produced the value (``source`` is
        ``"formula"`` or ``None``); empty when ``source`` is ``"value"`` or
        ``"forecast"``. When the formula is a pure
        sum/difference of references there is one step per term in formula
        order, each with ``operation`` ``"+"`` or ``"-"``, and the signed
        values sum to ``final_value``; for any other formula each referenced
        component is listed once with ``operation`` ``None``.

        This property does not raise.

        Returns
        -------
        list[ExplanationStep]
            Formula components in formula order; empty when the value did not come from the formula.
        """
    def to_text(self) -> str:
        """
        Human-readable multi-line explanation.

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            Human-readable multi-line explanation.
        """
    def to_arrow(self) -> ArrowTable:
        """
        Export the breakdown as an ``ArrowTable``.

        Columns: ``component``, ``value``, ``operation`` (null when absent).
        One row per breakdown step in evaluation order.

        Returns
        -------
        ArrowTable
            Formula breakdown table with one row per step.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the breakdown as a pandas ``DataFrame``.

        Columns: ``component``, ``value``, ``operation`` (``None`` when absent).
        One row per breakdown step in evaluation order.

        Returns
        -------
        pd.DataFrame
            Breakdown frame with one row per step.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the breakdown as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Breakdown frame with one row per step.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the breakdown as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Breakdown table with one row per step.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        A non-finite ``final_value`` or step ``value`` is written as
        ``"nan"``, ``"inf"`` or ``"-inf"`` so the document round-trips.

        Returns
        -------
        str
            Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> Explanation:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        Explanation
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``Explanation`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import Explanation
        >>> Explanation.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def explain_formula(model: Any, results: Any, node_id: str, period: str) -> Explanation:
    """Explain a formula for a specific node and period.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    node_id : str
        Node whose formula to explain.
    period : str
        Period string.

    Returns
    -------
    Explanation
        Typed explanation with the cell ``source``, the ``forecast`` spec of a
        forecast cell, ``breakdown`` steps, ``to_text()``, ``to_arrow()``, and
        ``to_pandas()``; ``to_json()`` matches the WASM ``explainFormula``.

    Raises
    ------
    KeyError
        If ``node_id`` is not in the model or has no value at ``period``.
    ValueError
        If ``period`` does not parse or a payload is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import explain_formula
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> model = b.build()
    >>> explain_formula(model, Evaluator().evaluate(model), "profit", "2025Q1").final_value
    50.0
    """
