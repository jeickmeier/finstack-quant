"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/types/model.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
from typing import Any, Literal
import pandas as pd
import polars as pl
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements._builder import ModelBuilder
from finstack_quant.statements._types.forecast import ForecastSpec

class NodeType:
    """
    How a node combines explicit values, forecasts, and formulas.

    Examples
    --------
    >>> from finstack_quant.statements import NodeType
    >>> NodeType.calculated() == NodeType.calculated()
    True

    """

    @staticmethod
    def value() -> NodeType:
        """
        Value node: explicit per-period values only (actuals, assumptions).

        The node has no formula and no forecast, so periods without an
        explicit value stay unset.

        Returns
        -------
        NodeType
            Value-only node type.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.value() == NodeType.value()
        True
        """
        ...

    @staticmethod
    def calculated() -> NodeType:
        """
        Calculated node: derived from its formula in every period.

        Returns
        -------
        NodeType
            Calculated node type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.calculated() == NodeType.calculated()
        True
        """
        ...

    @staticmethod
    def mixed() -> NodeType:
        """
        Mixed node: explicit value, else forecast, else formula.

        This is the crate's core precedence invariant — **Value > Forecast >
        Formula** — resolved independently for each period, so a node can be
        an actual in historical periods, a forecast in projected periods, and
        a formula wherever neither is supplied.

        Returns
        -------
        NodeType
            Mixed node type.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NodeType
        >>> NodeType.mixed() == NodeType.mixed()
        True
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical snake-case wire discriminant for this node type.

        Returns
        -------
        str
            ``"value"``, ``"calculated"``, or ``"mixed"``, matching the JSON
            schema rename for :class:`NodeType`.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two node type tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this node type.
        Returns
        -------
        str
        """
        ...

class NodeId:
    """
    Type-safe identifier for a node in a financial model.

    Examples
    --------
    >>> from finstack_quant.statements import NodeId
    >>> str(NodeId("revenue"))
    'revenue'

    """

    def __init__(self, id: str) -> None:
        """
        Wrap a raw node identifier string.

        Parameters
        ----------
        id:
            Node identifier as it appears in the model graph and in formulas
            (for example ``"revenue"``). The value is stored verbatim — no
            case folding or trimming — so identifiers are matched exactly.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.statements import NodeId
        >>> NodeId("ebitda").as_str()
        'ebitda'
        """
        ...

    def as_str(self) -> str:
        """
        Return the underlying identifier string.

        Returns
        -------
        str
            The identifier exactly as supplied at construction.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.statements import NodeId
        >>> NodeId("cogs").as_str()
        'cogs'
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-literal style representation.
        Returns
        -------
        str
        """
        ...

    def __str__(self) -> str:
        """Return the identifier as a plain string.
        Returns
        -------
        str
        """
        ...

class NumericMode:
    """
    Numeric evaluation mode for statement evaluation.

    Examples
    --------
    >>> from finstack_quant.statements import NumericMode
    >>> NumericMode.float64() == NumericMode.float64()
    True

    """

    @staticmethod
    def float64() -> NumericMode:
        """
        Use 64-bit floating point arithmetic.

        Returns
        -------
        NumericMode
            IEEE-754 double-precision mode.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.statements import NumericMode
        >>> NumericMode.float64() == NumericMode.float64()
        True
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two numeric mode tokens are equal."""
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this numeric mode.
        Returns
        -------
        str
        """
        ...

class NodeSpec:
    """One node of a :class:`FinancialModelSpec`: type, values, formula, forecast.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> b = ModelBuilder("demo")
    >>> _ = b.periods("2025Q1..Q2", "2025Q1")
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> b.build().get_node("revenue").node_type.kind
    'value'

    """

    def __init__(
        self,
        *,
        node_id: str,
        node_type: NodeType | str,
        name: str | None = None,
        values: Mapping[str, float | Money] | None = None,
        availability_dates: Mapping[str, date | str] = ...,
        forecast: ForecastSpec | None = None,
        formula_text: str | None = None,
        where_text: str | None = None,
        tags: list[str] = ...,
        value_type: Any | None = None,
        rollup: str | None = None,
        meta: Mapping[str, Any] = ...,
    ) -> None:
        """Construct a node specification from keyword-only fields.

        Rust reads the keywords as the ``NodeSpec`` serde struct: each field
        takes its wire form (or a typed object written as that form), Rust
        supplies the defaults and an unknown keyword raises.

        Parameters
        ----------
        node_id : str
            Node identifier; must not use a reserved prefix, DSL keyword, or the export timeline name ``period_id``.
        node_type : NodeType | str
            ``NodeType.value()``, ``NodeType.calculated()`` or ``NodeType.mixed()``,
            or the serde name ``"value"`` / ``"calculated"`` / ``"mixed"``.
        name : str, optional
            Human-readable label.
        values : Mapping[str, float | Money], optional
            Explicit observations keyed by period id (e.g. ``"2025Q1"``);
            ``Money`` cells make the node monetary, floats make it scalar.
        availability_dates : Mapping[str, date | str], default {}
            Publication date of each explicit observation, keyed by period id;
            an omitted period is available at its period end.
        forecast : ForecastSpec, optional
            Forecast rule applied to forecast periods.
        formula_text : str, optional
            Statements DSL expression for calculated and mixed nodes.
        where_text : str, optional
            DSL guard; periods where it is false evaluate to 0.
        tags : list[str], default []
            Free-form labels for grouping and filtering. They do not affect rollups.
        value_type : dict, optional
            Serde ``NodeValueType`` (``{"type": "monetary", "currency": "USD"}``
            or ``{"type": "scalar"}``); ``None`` infers it from the values.
        rollup : str, optional
            How the line rolls up to a coarser frequency: ``"sum"`` (flows),
            ``"period_end"`` (balances, trailing metrics) or ``"formula"``
            (ratios, which need a formula). Without it the line needs a
            :class:`RollupSpec` entry, which also overrides it.
        meta : Mapping[str, Any], default {}
            Free-form JSON metadata.

        Raises
        ------
        ValueError
            If a required field is missing, a keyword is unknown, a value does
            not have its wire form, a period id does not parse, or ``rollup``
            is not a recognized method.
        TypeError
            If a positional argument is passed.

        Examples
        --------
        >>> from finstack_quant.statements import NodeSpec, NodeType
        >>> node = NodeSpec(node_id="revenue", node_type=NodeType.value(), values={"2025Q1": 100.0}, rollup="sum")
        >>> (node.node_id, node.values, node.rollup)
        ('revenue', {'2025Q1': 100.0}, 'sum')

        """
        ...

    @property
    def node_id(self) -> str:
        """
        Identifier of this node.

        This property does not raise.

        Returns
        -------
        str
            Identifier of this node.
        """
        ...

    @property
    def name(self) -> str | None:
        """
        Human-readable label, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Human-readable label, or ``None``.
        """
        ...

    @property
    def node_type(self) -> NodeType:
        """
        Whether the node is ``value``, ``calculated`` or ``mixed``.

        This property does not raise.

        Returns
        -------
        NodeType
            Whether the node is ``value``, ``calculated`` or ``mixed``.
        """
        ...

    @property
    def values(self) -> dict[str, float] | None:
        """
        Explicit observations keyed by period id, or ``None``.

        This property does not raise.

        Returns
        -------
        dict[str, float] | None
            Explicit observations keyed by period id, or ``None``.
        """
        ...

    @property
    def availability_dates(self) -> dict[str, date]:
        """
        Publication date of each explicit observation, keyed by period id.

        This property does not raise.

        Returns
        -------
        dict[str, date]
            Publication date of each explicit observation, keyed by period id.
        """
        ...

    @property
    def forecast(self) -> ForecastSpec | None:
        """
        Forecast rule applied to forecast periods, or ``None``.

        This property does not raise.

        Returns
        -------
        ForecastSpec | None
            Forecast rule applied to forecast periods, or ``None``.
        """
        ...

    @property
    def formula_text(self) -> str | None:
        """
        Statements DSL expression, or ``None`` for pure value nodes.

        This property does not raise.

        Returns
        -------
        str | None
            Statements DSL expression, or ``None`` for pure value nodes.
        """
        ...

    @property
    def where_text(self) -> str | None:
        """
        DSL guard restricting the periods the node applies to, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            DSL guard restricting the periods the node applies to, or ``None``.
        """
        ...

    @property
    def tags(self) -> list[str]:
        """
        Free-form classification tags.

        This property does not raise.

        Returns
        -------
        list[str]
            Free-form classification tags.
        """
        ...

    @property
    def rollup(self) -> str | None:
        """
        Declared rollup method: ``"sum"``, ``"period_end"`` or ``"formula"``, or ``None``.

        Returns
        -------
        str | None
            The declared method, or ``None`` when the line declares none and so
            needs a :class:`RollupSpec` entry to be rolled up.

        Raises
        ------
        ValueError
            If the method cannot be rendered as its label.
        """
        ...

    @property
    def value_type(self) -> str | None:
        """
        ``"scalar"`` or ``"money"``, or ``None`` when unset.

        This property does not raise.

        Returns
        -------
        str | None
            ``"scalar"`` or ``"money"``, or ``None`` when unset.
        """
        ...

    @property
    def currency(self) -> str | None:
        """
        ISO-4217 code for money-typed nodes, otherwise ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            ISO-4217 code for money-typed nodes, otherwise ``None``.
        """
        ...

    @property
    def meta(self) -> dict[str, object]:
        """
        Arbitrary JSON metadata attached to the node.

        This property does not raise.

        Returns
        -------
        dict[str, object]
            Arbitrary JSON metadata attached to the node.
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Payload accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.

        """
        ...

    @staticmethod
    def from_json(json: str, /) -> NodeSpec:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        NodeSpec
            Reconstructed node specification.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import NodeSpec
        >>> NodeSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...

class FinancialModelSpec:
    """
    Top-level financial model specification (wire format).

    Typically built with ``ModelBuilder`` or loaded from JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> builder.build().id
    'demo'

    """

    def to_builder(self) -> ModelBuilder:
        """Resume work on an independent builder with the model's existing inputs.

        Returns
        -------
        ModelBuilder
            Editor holding the current timeline, nodes, settings and capital structure.

        Raises
        ------
        ValueError
            If this model cannot seed a builder.
        """
        ...
    def with_meta(self, key: str, value: object) -> FinancialModelSpec:
        """Insert or replace one literal metadata value on a model copy.

        Parameters
        ----------
        key : str
            Exact metadata key.
        value : object
            JSON-compatible value. A string is stored as a string, even if it looks like JSON.

        Returns
        -------
        FinancialModelSpec
            Independent model carrying the changed metadata.

        Raises
        ------
        ValueError
            If the value is not representable as finite JSON data.
        """
        ...
    def with_currency(self, currency: Currency | str) -> FinancialModelSpec:
        """Set the monetary unit metadata on a model copy.

        Parameters
        ----------
        currency : Currency or str
            ISO currency recorded for the model. Existing amounts keep their currencies.

        Returns
        -------
        FinancialModelSpec
            Independent model with the updated currency metadata.

        Raises
        ------
        ValueError
            If the currency code is invalid.
        """
        ...
    def with_reporting_currency(self, currency: Currency | str) -> FinancialModelSpec:
        """Set the capital-structure reporting currency on a model copy.

        Parameters
        ----------
        currency : Currency or str
            ISO reporting currency of the existing capital structure.

        Returns
        -------
        FinancialModelSpec
            Independent model with the updated capital-structure reporting policy.

        Raises
        ------
        ValueError
            If the currency code is invalid or the model has no capital structure.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FinancialModelSpec:
        """
        Load and validate a model specification from JSON text.

        This is the one loading path Rust, Python and JavaScript share
        (Rust ``FinancialModelSpec::from_json``): the document may be at most
        64 MiB and nested 96 levels deep, an object may not repeat a key or
        carry an unknown field, ``schema_version`` must be the integer ``1``,
        and the decoded model must be runnable.

        Parameters
        ----------
        json:
            JSON document matching the statements model schema.

        Returns
        -------
        FinancialModelSpec
            The validated specification.

        Raises
        ------
        finstack_quant.portfolio.ContractValidationError
            A ``ValueError``. If ``json`` is malformed, repeats a key, has a
            missing or different ``schema_version``, does not match the model
            schema, or decodes to a model that is not runnable (an empty or
            out-of-order timeline, a reserved node id, an invalid formula or
            units, an unknown reference, a dependency cycle or an invalid
            capital structure). ``report`` lists the diagnostics with their
            ``code`` and the JSON ``pointer`` of the field concerned, and the
            message carries the first diagnostic's text. An input over a size
            or depth limit raises the subclass ``ContractLimitExceededError``.

        Examples
        --------
        >>> from finstack_quant.statements import FinancialModelSpec
        >>> payload = (
        ...     '{"id":"demo","periods":{"calendar": "gregorian", "periods": [{"id":"2025Q1","start":"2025-01-01",'
        ...     '"end":"2025-04-01","is_actual":false}]},'
        ...     '"nodes":{"revenue":{"node_id":"revenue","node_type":"value","value_type":{"type":"scalar"}}},"schema_version":1}'
        ... )
        >>> (FinancialModelSpec.from_json(payload).id, FinancialModelSpec.from_json(payload).node_count)
        ('demo', 1)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this specification to compact JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `FinancialModelSpec`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> '"id":"demo"' in builder.build().to_json()
        True

        """
        ...

    def content_hash(self) -> str:
        """
        Versioned SHA-256 content hash of the validated model's canonical JSON.

        Mirrors Rust ``FinancialModelSpec::content_hash`` (WASM
        ``statements.financialModelContentHash``). The model is validated
        first, and the hash is taken over the canonical JSON of the validated
        model, so it does not depend on key order or on how typed numbers
        were spelled in the source document, and a model that is not runnable
        has no hash.

        Returns
        -------
        str
            ``"sha256:<hex>"`` content hash.

        Raises
        ------
        ValueError
            If the model is not runnable or contains a non-finite number.
        RuntimeError
            If the model's formulas form a dependency cycle.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> builder.build().content_hash().startswith("sha256:")
        True
        """
        ...

    @property
    def source_snapshot(self) -> str | None:
        """
        Identity of the source snapshot the observations were taken from.

        Returns
        -------
        str | None
            The snapshot identifier, or ``None`` when the model names none. An evaluation with an observation cutoff needs it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def apply_edits(self, edits: list[dict[str, Any]]) -> FinancialModelSpec:
        """
        Apply named edits atomically and return the edited model.

        Edits apply in order to a copy, which must pass the runnable-model
        validation. This model is never changed.

        Parameters
        ----------
        edits:
            Edit objects, each tagged by ``"edit"``: ``add_node`` and ``replace_node`` (``node``); ``remove_node``, ``remove_forecast`` and ``remove_where`` (``node_id``); ``set_observation`` (``node_id``, ``period``, ``value``, optional ``availability_date``); ``remove_observation`` (``node_id``, ``period``); ``set_formula`` (``node_id``, ``formula``); ``set_forecast`` (``node_id``, ``forecast``; the node becomes mixed); ``set_where`` (``node_id``, ``predicate``).

        Returns
        -------
        FinancialModelSpec
            A new model with every edit applied.

        Raises
        ------
        ValueError
            If an edit is malformed, adds an identifier that is already defined, names a node or observation that does not exist, or the edited model fails validation.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> edited = builder.build().apply_edits([
        ...     {"edit": "set_observation", "node_id": "revenue", "period": "2025Q1", "value": 110.0}
        ... ])
        >>> edited.get_node("revenue").values
        {'2025Q1': 110.0}

        """
        ...

    @property
    def id(self) -> str:
        """
        Model identifier string.
        Returns
        -------
        str
            Unique model identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def period_count(self) -> int:
        """
        Number of periods in the model timeline.

        Returns
        -------
        int
            Count in **periods** on the model's own cadence (quarters, months,
            years), not months. Their declared order *is* the evaluation
            timeline.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def node_count(self) -> int:
        """
        Number of nodes defined on the model.
        Returns
        -------
        int
            Number of nodes (line items / metrics) declared in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def node_ids(self) -> list[str]:
        """
        List node identifiers in declaration order.

        Returns
        -------
        list[str]
            Ordered node id strings.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> builder.build().node_ids()
        ['revenue']
        """
        ...

    def has_node(self, node_id: str) -> bool:
        """
        Return whether a node with the given id exists.

        Parameters
        ----------
        node_id:
            Node identifier to test.

        Returns
        -------
        bool
            ``True`` if present.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> from finstack_quant.statements import ModelBuilder
        >>> builder = ModelBuilder("demo")
        >>> _ = builder.periods("2025Q1..Q1")
        >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
        >>> model = builder.build()
        >>> (model.has_node("revenue"), model.has_node("cogs"))
        (True, False)
        """
        ...

    @property
    def schema_version(self) -> int:
        """
        Wire-format schema version of this model spec.

        Returns
        -------
        int
            Only version ``1`` is accepted today; the field exists so persisted
            models can be migrated rather than silently misread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary including id, period count, and node count.
        Returns
        -------
        str
        """
        ...

    @property
    def periods(self) -> list[str]:
        """Return periods from the model timeline.

        Returns
        -------
        list[str]
            All model period identifiers in timeline order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def actual_periods(self) -> list[str]:
        """Return actual periods from the model timeline.

        Returns
        -------
        list[str]
            Period identifiers in the historical prefix, in timeline order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def forecast_periods(self) -> list[str]:
        """Return forecast periods from the model timeline.

        Returns
        -------
        list[str]
            Non-actual period identifiers in evaluation order.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def nodes(self) -> list[NodeSpec]:
        """Return copies of all node specifications in declaration order.

        Returns
        -------
        list[NodeSpec]
            Configured node specifications; changing a returned copy does not mutate the model.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """Return model metadata as plain Python containers.

        Returns
        -------
        dict[str, Any]
            Metadata in its JSON shape, including any reporting-currency hint.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    @property
    def capital_structure(self) -> dict[str, Any] | None:
        """Return the capital-structure specification in its canonical JSON shape.

        Returns
        -------
        dict[str, Any] | None
            Debt instruments, reporting currency, FX policy, waterfall, and metadata; None when the model has no capital structure.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def get_node(self, node_id: str) -> NodeSpec | None:
        """Look up a configured statement node.

        Parameters
        ----------
        node_id:
            Exact node identifier declared in this model.

        Returns
        -------
        NodeSpec | None
            A copy of the node configuration, or None if the identifier is absent.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export one descriptive row for each configured model node as an ``ArrowTable``.

        Returns
        -------
        ArrowTable
            Rows in declaration order with ``node_id``, ``node_type``, ``name``,
            ``formula_text``, ``forecast_method``, ``value_type``, ``currency``,
            and ``where_text`` columns; optional fields are null.

        Raises
        ------
        ValueError
            If a node description cannot be converted to the documented column schema.
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export one descriptive row for each configured model node as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Rows in declaration order with ``node_id``, ``node_type``, ``name``,
            ``formula_text``, ``forecast_method``, ``value_type``, ``currency``,
            and ``where_text`` columns; optional fields are null.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If a node description cannot be converted to the documented column schema.
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export one descriptive row for each configured model node as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Rows in declaration order with ``node_id``, ``node_type``, ``name``,
            ``formula_text``, ``forecast_method``, ``value_type``, ``currency``,
            and ``where_text`` columns; optional fields are null.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If a node description cannot be converted to the documented column schema.
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export one descriptive row for each configured model node as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Rows in declaration order with ``node_id``, ``node_type``, ``name``,
            ``formula_text``, ``forecast_method``, ``value_type``, ``currency``,
            and ``where_text`` columns; optional fields are null.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If a node description cannot be converted to the documented column schema.
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
