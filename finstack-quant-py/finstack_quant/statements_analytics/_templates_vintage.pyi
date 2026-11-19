"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/templates_vintage.rs``."""

from __future__ import annotations

from finstack_quant.statements import ModelBuilder

# Vintage template

def add_vintage_buildup(
    builder: ModelBuilder,
    name: str,
    new_volume_node: str,
    decay_curve: list[float],
) -> None:
    """
    Add a vintage cohort buildup to an active model builder.

    Parameters
    ----------
    builder : ModelBuilder
        Active builder receiving the generated cohort-convolution node.
    name : str
        Prefix used to name the generated vintage buildup nodes.
    new_volume_node : str
        Existing node ID that supplies new volume for each cohort period.
    decay_curve : list[float]
        Ordered cohort-retention factors by elapsed period, expressed as decimal
        multipliers of original volume.

    Raises
    ------
    ValueError
        If the builder has been consumed or a decay coefficient is non-finite.
        Building the model checks the generated node and its references.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_vintage_buildup
    >>> from finstack_quant.statements import ModelBuilder, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("new_volume", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> add_vintage_buildup(builder, "customers", "new_volume", [1.0, 0.8])
    >>> builder.build().node_count
    2

    """
    ...
