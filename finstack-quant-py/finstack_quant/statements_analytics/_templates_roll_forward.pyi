"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/templates_roll_forward.rs``."""

from __future__ import annotations

from finstack_quant.statements import FinancialModelSpec

# Roll-forward template

def add_roll_forward(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
) -> FinancialModelSpec:
    """
    Apply the roll-forward template (Beginning + Increases - Decreases = Ending) to a model spec.

    Returns a typed ``FinancialModelSpec`` with ``{name}_beg`` and
    ``{name}_end`` nodes added. The first period opens at zero; use
    ``add_roll_forward_with_opening`` for an explicit opening balance.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing zero-opening beginning- and ending-balance nodes.

    Raises
    ------
    ValueError
        If model JSON or a roll-forward node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_roll_forward(model, "balance", ["adds"], [])
    >>> updated.has_node("balance_end")
    True

    """
    ...

def add_roll_forward_with_opening(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
    opening: float,
) -> FinancialModelSpec:
    """
    Apply the roll-forward template with an explicit first-period opening balance.

    Same as ``add_roll_forward`` except the first period's beginning balance
    is ``opening`` instead of zero. Returns a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.
    opening : float
        Beginning balance assigned to the first modeled period in model units.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the seeded beginning- and ending-balance nodes.

    Raises
    ------
    ValueError
        If model JSON, opening, or a roll-forward node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward_with_opening
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_roll_forward_with_opening(model, "balance", ["adds"], [], 100.0)
    >>> updated.has_node("balance_beg")
    True

    """
    ...
