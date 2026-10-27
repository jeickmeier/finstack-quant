"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/registry.rs``."""

from __future__ import annotations

class Registry:
    """
    Reusable statement metric registry.

    Examples
    --------
    >>> from finstack_quant.statements import Registry
    >>> registry = Registry.with_builtins()
    >>> len(registry) > 0
    True

    """

    def __init__(self) -> None:
        """
        Create an empty metric registry with no built-in metrics loaded.

        Call :meth:`load_builtins` or use :meth:`with_builtins` to populate it.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @staticmethod
    def with_builtins() -> Registry:
        """
        Create a registry preloaded with built-in metrics.

        Returns
        -------
        Registry
            A registry containing all built-in statement metrics.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.statements import Registry
        >>> len(Registry.with_builtins()) > 0
        True
        """
        ...

    def load_builtins(self) -> None:
        """
        Load the library's built-in statement metrics into this registry.

        Existing entries with the same names are replaced by the built-in
        definitions.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def load_from_json_str(self, json: str) -> None:
        """
        Load metrics from a JSON document.

        Parameters
        ----------
        json:
            JSON string containing metric definitions.

        Raises
        ------
        ValueError
            If json is malformed or contains an invalid metric definition.
        KeyError
            If a referenced registry metric cannot be resolved.

        """
        ...

    def has(self, qualified_id: str) -> bool:
        """
        Return whether a fully qualified metric exists.

        Parameters
        ----------
        qualified_id:
            Fully qualified metric identifier.

        Returns
        -------
        bool
            ``True`` if the metric is registered.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def __len__(self) -> int:
        """Return the number of metrics.
        Returns
        -------
        int
        """
        ...

    def metric_ids(self) -> list[str]:
        """List every registered metric in load order.

        Returns
        -------
        list[str]
            Fully qualified metric identifiers such as "fin.gross_profit".

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def get(self, qualified_id: str) -> MetricDefinition:
        """Look up a registered metric definition.

        Parameters
        ----------
        qualified_id:
            Exact namespace.metric identifier, such as "fin.gross_margin".

        Returns
        -------
        MetricDefinition
            A copy of the stored metric definition.

        Raises
        ------
        KeyError
            If the identifier is not registered.
        """
        ...

    def get_dependencies(self, qualified_id: str) -> list[str]:
        """List transitive registry dependencies in construction order.

        Parameters
        ----------
        qualified_id:
            Fully qualified identifier of the metric whose registry dependencies to resolve.

        Returns
        -------
        list[str]
            Dependencies first, excluding the requested metric and ordinary model input nodes.

        Raises
        ------
        KeyError
            If the metric or a required registry dependency is absent.
        ValueError
            If registry dependencies are cyclic or malformed.
        """
        ...

class MetricDefinition:
    """A registry metric: its formula, dependencies and classification.

    Examples
    --------
    >>> from finstack_quant.statements import Registry
    >>> Registry.with_builtins().get("fin.gross_margin").id
    'gross_margin'

    """

    @property
    def id(self) -> str:
        """
        Unqualified metric identifier.

        This property does not raise.

        Returns
        -------
        str
            Unqualified metric identifier.
        """
        ...

    @property
    def name(self) -> str:
        """
        Human-readable metric name.

        This property does not raise.

        Returns
        -------
        str
            Human-readable metric name.
        """
        ...

    @property
    def formula(self) -> str:
        """
        Statements DSL expression computing the metric.

        This property does not raise.

        Returns
        -------
        str
            Statements DSL expression computing the metric.
        """
        ...

    @property
    def description(self) -> str | None:
        """
        Prose description, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Prose description, or ``None``.
        """
        ...

    @property
    def category(self) -> str | None:
        """
        Classification bucket (e.g. ``"profitability"``), or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Classification bucket (e.g. ``"profitability"``), or ``None``.
        """
        ...

    @property
    def unit_type(self) -> str | None:
        """
        Result unit (e.g. ``"ratio"``, ``"percent"``), or ``None``.

        Returns
        -------
        str | None
            Result unit (e.g. ``"ratio"``, ``"percent"``), or ``None``.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    @property
    def requires(self) -> list[str]:
        """
        Node ids the formula reads.

        This property does not raise.

        Returns
        -------
        list[str]
            Node ids the formula reads.
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
    def meta(self) -> dict[str, object]:
        """
        Arbitrary JSON metadata attached to the definition.

        This property does not raise.

        Returns
        -------
        dict[str, object]
            Arbitrary JSON metadata attached to the definition.
        """
        ...

    def to_json(self) -> str:
        """Serialize to canonical JSON.

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
    def from_json(json: str, /) -> MetricDefinition:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        MetricDefinition
            Reconstructed metric definition.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import MetricDefinition
        >>> MetricDefinition.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...
