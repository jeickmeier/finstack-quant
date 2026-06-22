# finstack-quant-py/finstack_quant/reporting/statements_common.py
"""Shared statement-result access + the P&L matrix table (items × periods).

Pure presentation: this module only reads pre-computed values and lays them out.
It performs no financial calculation — margins, growth, and ratios must already
exist as statement nodes or structured engine results.
"""

from __future__ import annotations

from collections.abc import Callable
import html
import json
from typing import Any

from .theme import Theme


def _esc(x: Any) -> str:
    return html.escape(str(x))


class StatementView:
    """Read-only view over a ``StatementResult``'s scalar node values.

    Wraps the ``{node_id: {period_str: value}}`` map so callers can read values
    by string keys uniformly, regardless of the source representation.
    """

    def __init__(self, nodes: dict[str, dict[str, float]]) -> None:
        """Store the node map; callers should use :func:`parse_statement` instead."""
        self._nodes = nodes

    def get(self, node_id: str, period: str) -> float | None:
        """Return the value at ``(node_id, period)``, or ``None`` if absent."""
        node = self._nodes.get(node_id)
        return node.get(period) if node else None

    def node_ids(self) -> list[str]:
        """Return all node ids present (in source order)."""
        return list(self._nodes)

    def periods(self) -> list[str]:
        """Return all period strings present across nodes, sorted ascending."""
        seen: dict[str, None] = {}
        for series in self._nodes.values():
            for period in series:
                seen[period] = None
        return sorted(seen)


def parse_statement(results: Any) -> StatementView:
    """Build a :class:`StatementView` from a ``StatementResult``, JSON, or dict.

    Accepts a ``StatementResult`` object (anything exposing ``to_json()``), a
    JSON string, an already-parsed ``dict``, or a :class:`StatementView`
    (returned unchanged).
    """
    if isinstance(results, StatementView):
        return results
    if isinstance(results, str):
        data = json.loads(results)
    elif isinstance(results, dict):
        data = results
    elif hasattr(results, "to_json"):
        data = json.loads(results.to_json())
    else:
        raise TypeError(
            f"results must be a StatementResult, JSON string, dict, or StatementView; got {type(results).__name__}"
        )
    return StatementView(data.get("nodes", {}))


def pl_matrix_table(
    view: StatementView,
    rows: list[tuple[str, str, Callable[[Any], str]]],
    periods: list[str],
    *,
    theme: Theme,  # noqa: ARG001  (styling comes from the scoped `table.dd` CSS)
) -> str:
    """Render line items × periods as a ``<table class="dd">``.

    ``rows`` is ``[(label, node_id, formatter)]``; each cell is
    ``formatter(view.get(node_id, period))``. Pure presentation — the formatter
    handles ``None`` (missing) values. Reuses the existing data-table CSS so the
    first column is left-aligned and value columns are right-aligned.
    """
    head = "<th></th>" + "".join(f"<th>{_esc(p)}</th>" for p in periods)
    body_rows: list[str] = []
    for label, node_id, formatter in rows:
        cells = [f"<td>{_esc(label)}</td>"]
        cells.extend(f"<td>{formatter(view.get(node_id, p))}</td>" for p in periods)
        body_rows.append(f"<tr>{''.join(cells)}</tr>")
    return f'<table class="dd"><thead><tr>{head}</tr></thead><tbody>{"".join(body_rows)}</tbody></table>'
