"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/templates_roll_forward.rs``."""

from __future__ import annotations

from finstack_quant.statements import ModelBuilder

# Roll-forward template

def add_roll_forward(
    builder: ModelBuilder,
    name: str,
    increases: list[str],
    decreases: list[str],
) -> None:
    """
    Apply the roll-forward template (Beginning + Increases - Decreases = Ending) to a model builder.

    Adds ``{name}_beg`` and ``{name}_end`` nodes to the builder. The first period opens at zero; use
    ``add_roll_forward_with_opening`` for an explicit opening balance.

    Parameters
    ----------
    builder : ModelBuilder
        Active builder to which the beginning and ending balance nodes are added.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.

    Raises
    ------
    ValueError
        If the builder has already been consumed. Build the model to validate generated identifiers and references.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> add_roll_forward(builder, "balance", ["adds"], [])
    >>> builder.build().has_node("balance_end")
    True

    """
    ...

def add_roll_forward_with_opening(
    builder: ModelBuilder,
    name: str,
    increases: list[str],
    decreases: list[str],
    opening: float,
) -> None:
    """
    Apply the roll-forward template with an explicit first-period opening balance.

    Same as ``add_roll_forward`` except the first period's beginning balance
    is ``opening`` instead of zero.

    Parameters
    ----------
    builder : ModelBuilder
        Active builder to which the beginning and ending balance nodes are added.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.
    opening : float
        Beginning balance assigned to the first modeled period in model units.

    Raises
    ------
    ValueError
        If the builder has already been consumed. Build the model to validate the opening balance and generated formulas.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward_with_opening
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> add_roll_forward_with_opening(builder, "balance", ["adds"], [], 100.0)
    >>> builder.build().has_node("balance_beg")
    True

    """
    ...
