"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/rollup.rs``."""

from __future__ import annotations

from typing import Any

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
        Rollup method per node id: ``"sum"`` (flows, the default for nodes not
        listed), ``"period_end"`` (balances and trailing metrics such as LTM
        EBITDA take the last source period) or ``"formula"`` (ratios
        re-evaluate their formula on the rolled-up values).

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
            Node id to ``"sum"``, ``"period_end"`` or ``"formula"``; nodes not
            listed are summed.

        Raises
        ------
        ValueError
            If ``frequency`` or a method is not a recognized label.
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
        Rollup method label per listed node id; unlisted nodes are summed.

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
