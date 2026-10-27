"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/templates_vintage.rs``."""

from __future__ import annotations

from finstack_quant.statements import FinancialModelSpec

# Vintage template

def add_vintage_buildup(
    model: FinancialModelSpec | str,
    name: str,
    new_volume_node: str,
    decay_curve: list[float],
) -> FinancialModelSpec:
    """
    Apply the vintage (cohort) buildup template to a model spec.

    Returns a typed ``FinancialModelSpec`` with the convolution
    node added.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with the cohort schedule.
    name : str
        Prefix used to name the generated vintage buildup nodes.
    new_volume_node : str
        Existing node ID that supplies new volume for each cohort period.
    decay_curve : list[float]
        Ordered cohort-retention factors by elapsed period, expressed as decimal
        multipliers of original volume.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the generated vintage-convolution node.

    Raises
    ------
    ValueError
        If model JSON, a node identifier, or decay_curve is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_vintage_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("new_volume", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_vintage_buildup(model, "customers", "new_volume", [1.0, 0.8])
    >>> updated.node_count > model.node_count
    True

    """
    ...
