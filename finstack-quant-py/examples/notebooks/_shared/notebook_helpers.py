"""Presentation and boilerplate helpers shared by example notebooks.

These helpers only format or reshape values that notebooks already have; they
never wrap ``finstack_quant`` business logic.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any


def series(periods: Sequence[str], values: Sequence[float]) -> list[tuple[str, float]]:
    """Zip period identifiers with values for ``ModelBuilder.value``.

    Args:
        periods: Period identifiers, e.g. ``["2025Q1", "2025Q2"]``.
        values: One value per period, in the same order.

    Returns:
        Pairs accepted by ``ModelBuilder.value``.

    Raises:
        ValueError: If ``periods`` and ``values`` have different lengths.

    Examples:
        >>> series(["2025Q1", "2025Q2"], [100.0, 105.0])
        [('2025Q1', 100.0), ('2025Q2', 105.0)]
    """
    if len(periods) != len(values):
        msg = f"Expected {len(periods)} values for {len(periods)} periods, got {len(values)}"
        raise ValueError(msg)
    return list(zip(periods, values, strict=True))


def banner(title: str) -> None:
    """Print *title* between 72-character rules, preceded by a blank line."""
    print(f"\n{'=' * 72}")
    print(title)
    print("=" * 72)


def print_metrics(result: Any, metric_ids: Sequence[str]) -> None:
    """Print each requested metric of a ``ValuationResult`` that is present.

    Metrics are printed in the order given, indented by two spaces, and metrics
    returning ``None`` are skipped.

    Args:
        result: A ``finstack_quant.valuations.ValuationResult``.
        metric_ids: Metric identifiers to look up via ``result.get_metric``.
    """
    for metric_id in metric_ids:
        value = result.get_metric(metric_id)
        if value is not None:
            print(f"  {metric_id}: {value}")
