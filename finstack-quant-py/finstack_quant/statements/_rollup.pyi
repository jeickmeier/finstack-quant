"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/rollup.rs``."""

from __future__ import annotations

from typing import Any

from finstack_quant.statements._types import FinancialModelSpec

class RollupSpec:
    """
    Target frequency and per-node methods for :meth:`StatementResult.rollup`.

    Parameters
    ----------
    frequency : str
        Target frequency: ``"quarterly"``, ``"semi_annual"`` or ``"annual"``
        using canonical spellings only. It must be
        coarser than the model's periods and hold a whole number of them.
    methods : dict[str, str] or None
        Rollup method per node id, overriding the node's declared
        :attr:`NodeSpec.rollup`: ``"sum"`` (flows), ``"period_end"`` (balances
        and trailing metrics such as LTM EBITDA take the last source period) or
        ``"formula"`` (ratios re-evaluate their formula on the rolled-up
        values). A node not listed uses its declaration, and sums without one.

    Examples
    --------
    >>> from finstack_quant.statements import RollupSpec
    >>> spec = RollupSpec("annual", {"cash": "period_end", "margin": "formula"})
    >>> (spec.frequency, spec.methods)
    ('annual', {'cash': 'period_end', 'margin': 'formula'})

    """
    def __init__(self, frequency: str, methods: dict[str, str] | None = ...) -> None:
        """
        Build a rollup specification.

        Parameters
        ----------
        frequency : str
            Target frequency label (``"quarterly"``, ``"semi_annual"``,
            ``"annual"``; surrounding whitespace and ASCII case are ignored).
        methods : dict[str, str] or None, default None
            Node id to ``"sum"``, ``"period_end"`` or ``"formula"``, overriding
            the node's declared method; a node not listed uses its declaration
            and sums without one.

        Raises
        ------
        ValueError
            If ``frequency`` or a method is not a recognized label.
        """
        ...

    @staticmethod
    def default_for(model: FinancialModelSpec | str, frequency: str) -> RollupSpec:
        """
        List the rollup methods a model's lines declare.

        Only each node's ``rollup`` is read: a line that declares nothing is not
        listed and sums, whatever its tags say. Use it to seed the method
        choices a host shows; :meth:`StatementResult.rollup` already applies
        the declarations when given a spec with no methods.

        Parameters
        ----------
        model : FinancialModelSpec | str
            Model whose node declarations are listed, typed or its canonical
            JSON. Nothing is evaluated.
        frequency : str
            Target frequency stored on the spec (``"quarterly"``,
            ``"semi_annual"``, ``"annual"``; surrounding whitespace and ASCII
            case are ignored). It is checked against the model's periods by
            :meth:`StatementResult.rollup`, not here.

        Returns
        -------
        RollupSpec
            The ``frequency`` and every declared node with its method, in model
            order.

        Raises
        ------
        ValueError
            If ``model`` is malformed JSON or ``frequency`` is not a recognized
            label.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.statements import ModelBuilder, RollupSpec
        >>> b = ModelBuilder("m")
        >>> _ = b.periods("2025Q1..Q4", None)
        >>> _ = b.value("revenue", [("2025Q1", 100.0)])
        >>> _ = b.value("cash", [("2025Q1", 10.0)])
        >>> _ = b.compute("margin", "revenue / revenue")
        >>> model = json.loads(b.build().to_json())
        >>> model["nodes"]["cash"]["rollup"] = "period_end"
        >>> model["nodes"]["margin"]["rollup"] = "formula"
        >>> RollupSpec.default_for(json.dumps(model), "annual").methods
        {'cash': 'period_end', 'margin': 'formula'}

        """
        ...

    @staticmethod
    def from_json(json: str) -> RollupSpec:
        """
        Deserialize a rollup specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON with ``frequency`` and an optional ``methods`` map, as produced
            by :meth:`to_json`.

        Returns
        -------
        RollupSpec
            The reconstructed specification.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or names an
            unknown frequency or method.

        Examples
        --------
        >>> from finstack_quant.statements import RollupSpec
        >>> RollupSpec.from_json('{"frequency": "annual", "methods": {"cash": "period_end"}}').methods
        {'cash': 'period_end'}

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this specification to canonical JSON.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json` and
            :meth:`StatementResult.rollup`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def frequency(self) -> str:
        """
        Target frequency label.

        Returns
        -------
        str
            ``"quarterly"``, ``"semi_annual"`` or ``"annual"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def methods(self) -> dict[str, str]:
        """
        Rollup method label per listed node id; unlisted nodes use their declaration or sum.

        Returns
        -------
        dict[str, str]
            Node id to ``"sum"``, ``"period_end"`` or ``"formula"``.

        Raises
        ------
        ValueError
            If a method cannot be rendered as its label.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
