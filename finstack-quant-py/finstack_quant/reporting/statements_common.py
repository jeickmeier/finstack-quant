"""Shared statement-result access + the P&L matrix table (items × periods).

Pure presentation: this module only reads pre-computed values and lays them out.
It performs no financial calculation — margins, growth, and ratios must already
exist as statement nodes or structured engine results.

Examples:
--------
>>> from finstack_quant.reporting.statements_common import StatementView
>>> StatementView({"revenue": {"2025": 100.0}}).get("revenue", "2025")
100.0
"""

from __future__ import annotations

from collections.abc import Callable
import json
from typing import Any

from . import format as fmt, tables
from .document import Section

__all__ = [
    "StatementView",
    "json_or_dict",
    "parse_statement",
    "pl_matrix_table",
    "tornado_entries",
    "variance_table",
]


class StatementView:
    """Read-only view over a ``StatementResult``'s scalar node values.

    Wraps the ``{node_id: {period_str: value}}`` map so callers can read values
    by string keys uniformly, regardless of the source representation.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import StatementView
    >>> StatementView({"revenue": {"2025": 100.0}}).periods()
    ['2025']
    """

    def __init__(self, nodes: dict[str, dict[str, float]], numeric_mode: str | None = None) -> None:
        """Store a statement node-value map.

        Parameters
        ----------
        nodes : dict[str, dict[str, float]]
            Mapping from statement node ID to period-label/value mappings.
        numeric_mode : str or None
            The result's own numeric-mode stamp (``meta.numeric_mode``, e.g.
            ``"float64"``), or ``None`` when the source carries none.

        Notes:
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        self._nodes = nodes
        self.numeric_mode = numeric_mode

    def meta_lines(self) -> list[str]:
        """Provenance header lines read from the result's own stamp.

        Returns:
        -------
        list[str]
            ``["Numeric: <mode>"]`` when the result carried a numeric-mode
            stamp, otherwise an empty list (no provenance is asserted).

        Notes:
            This method does not raise.

        Examples:
        --------
        >>> from finstack_quant.reporting.statements_common import StatementView
        >>> StatementView({}, numeric_mode="float64").meta_lines()
        ['Numeric: float64']
        >>> StatementView({}).meta_lines()
        []
        """
        return [f"Numeric: {self.numeric_mode}"] if self.numeric_mode else []

    def get(self, node_id: str, period: str) -> float | None:
        """Return the value at ``(node_id, period)``, or ``None`` if absent.

        Parameters
        ----------
        node_id : str
            Statement node identifier to look up.
        period : str
            Model period label within the requested node's value series.

        Returns:
        -------
        float | None
            Stored scalar value, or ``None`` when the node or period is absent.

        Notes:
            This method does not raise; a missing result is ``None`` rather than an exception.
        """
        node = self._nodes.get(node_id)
        return node.get(period) if node else None

    def node_ids(self) -> list[str]:
        """Return all node ids present (in source order).

        Returns:
        -------
        list[str]
            Node identifiers in source insertion order.

        Notes:
            This method does not raise; it returns the stored or derived value.
        """
        return list(self._nodes)

    def periods(self) -> list[str]:
        """Return all period strings present across nodes, sorted ascending.

        Returns:
        -------
        list[str]
            Distinct period labels in ascending order.

        Notes:
            This method does not raise; it returns the stored or derived value.
        """
        seen: dict[str, None] = {}
        for series in self._nodes.values():
            for period in series:
                seen[period] = None
        return sorted(seen)


def json_or_dict(obj: Any, *, noun: str = "value") -> dict[str, Any]:
    """Normalise a typed contract, dict, or JSON-object string into a dict.

    Raises:
        TypeError: if ``obj`` is not a typed contract, dict, or string, or its
            JSON does not decode to an object.

    Parameters
    ----------
    obj : Any
        Typed contract exposing ``to_json()``, mapping object, or JSON-object
        string to normalize into a dictionary.
    noun : str
        Reader-facing noun used in type and JSON-decoding error messages.

    Returns:
    -------
    dict[str, Any]
        Parsed object mapping, preserving the canonical payload fields.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import json_or_dict
    >>> json_or_dict('{"id":"demo"}')
    {'id': 'demo'}
    """
    if isinstance(obj, dict):
        return obj
    if isinstance(obj, str):
        data = json.loads(obj)
    elif hasattr(obj, "to_json"):
        data = json.loads(obj.to_json())
    else:
        raise TypeError(f"{noun} must be a typed contract, dict, or JSON string; got {type(obj).__name__}")
    if not isinstance(data, dict):
        raise TypeError(f"{noun} JSON must decode to an object; got {type(data).__name__}")
    return data


def tornado_entries(tornado: Any, *, noun: str = "tornado") -> list[tuple[Any, Any, Any]]:
    """Read ``(parameter_id, downside, upside)`` rows from tornado entries.

    Each entry is a typed ``TornadoEntry`` (``generate_tornado_entries``
    output), a dict, or a JSON-object string, normalised through
    :func:`json_or_dict`. Rows keep the input order, which for Rust output is
    the swing ranking.

    Parameters
    ----------
    tornado : Any
        List or tuple of tornado entries, each carrying ``parameter_id``,
        ``downside`` and ``upside``.
    noun : str
        Reader-facing noun used in error messages.

    Returns:
    -------
    list[tuple[Any, Any, Any]]
        ``(parameter_id, downside, upside)`` rows in input order.

    Raises:
    ------
    TypeError
        If ``tornado`` is not a list or tuple, or an entry is not a typed
        entry, dict, or JSON-object string.
    json.JSONDecodeError
        If an entry is a string that is not valid JSON.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import tornado_entries
    >>> tornado_entries([{"parameter_id": "wacc", "downside": -1.0, "upside": 2.0}])
    [('wacc', -1.0, 2.0)]
    """
    if not isinstance(tornado, (list, tuple)):
        raise TypeError(f"{noun} must be a list of tornado entries; got {type(tornado).__name__}")
    rows = [json_or_dict(entry, noun=f"{noun} entry") for entry in tornado]
    return [(row.get("parameter_id"), row.get("downside"), row.get("upside")) for row in rows]


def parse_statement(results: Any) -> StatementView:
    """Build a :class:`StatementView` from a ``StatementResult``, JSON, or dict.

    Accepts a ``StatementResult`` object (anything exposing ``to_json()``), a
    JSON string, an already-parsed ``dict``, or a :class:`StatementView`
    (returned unchanged).

    Raises:
        TypeError: if ``results`` is none of the accepted types.
        json.JSONDecodeError: if ``results`` is a string that is not valid JSON.

    Parameters
    ----------
    results : Any
        ``StatementResult``, canonical JSON, parsed dictionary, or existing
        ``StatementView`` to expose through the common lookup interface.

    Returns:
    -------
    StatementView
        Read-only view over the payload's ``nodes`` mapping, carrying the
        payload's ``meta.numeric_mode`` stamp when present.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import parse_statement
    >>> parse_statement({"nodes": {"revenue": {"2025": 100.0}}}).node_ids()
    ['revenue']
    """
    if isinstance(results, StatementView):
        return results
    if not isinstance(results, (str, dict)) and not hasattr(results, "to_json"):
        raise TypeError(
            f"results must be a StatementResult, JSON string, dict, or StatementView; got {type(results).__name__}"
        )
    data = json_or_dict(results, noun="results")
    meta = data.get("meta")
    numeric_mode = meta.get("numeric_mode") if isinstance(meta, dict) else None
    return StatementView(data.get("nodes", {}), numeric_mode=numeric_mode)


def pl_matrix_table(
    view: StatementView,
    rows: list[tuple[str, str, Callable[[Any], str]]],
    periods: list[str],
) -> str:
    """Render line items × periods as a ``<table class="dd">``.

    ``rows`` is ``[(label, node_id, formatter)]``; each cell is
    ``formatter(view.get(node_id, period))``. Pure presentation — the formatter
    handles ``None`` (missing) values. Reuses the existing data-table CSS so the
    first column is left-aligned and value columns are right-aligned.

    Parameters
    ----------
    view : StatementView
        Statement value view supplying node values by ID and period.
    rows : list[tuple[str, str, Callable[[Any], str]]]
        Ordered ``(display_label, node_id, formatter)`` line-item definitions.
    periods : list[str]
        Ordered model period labels rendered as table columns.

    Returns:
    -------
    str
        HTML table containing the selected line items and periods.

    Notes:
    -----
    This helper does not raise; cell values are escaped and missing numbers use the report placeholder.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import StatementView, pl_matrix_table
    >>> view = StatementView({"revenue": {"2025": 100.0}})
    >>> "Revenue" in pl_matrix_table(view, [("Revenue", "revenue", str)], ["2025"])
    True
    """
    head = "<th></th>" + "".join(f"<th>{fmt._escape_html(p)}</th>" for p in periods)
    body_rows: list[str] = []
    for label, node_id, formatter in rows:
        cells = [f"<td>{fmt._escape_html(label)}</td>"]
        cells.extend(f"<td>{formatter(view.get(node_id, p))}</td>" for p in periods)
        body_rows.append(f"<tr>{''.join(cells)}</tr>")
    return f'<table class="dd"><thead><tr>{head}</tr></thead><tbody>{"".join(body_rows)}</tbody></table>'


def variance_table(variance: Any) -> str | None:
    """Render a ``run_variance`` result (``{"rows": [...]}``) as an HTML table.

    Returns ``None`` when there are no rows. Pure presentation; ``pct_var`` is
    scaled to percent for display.

    Parameters
    ----------
    variance : Any
        Typed ``VarianceReport`` (``run_variance`` output), its dict form, or
        its JSON text, normalised through :func:`json_or_dict`; ``None``
        renders nothing.

    Returns:
    -------
    str | None
        HTML variance table, or ``None`` when no valid rows are supplied.

    Raises:
    ------
    TypeError
        If ``variance`` is not a typed report, dict, or JSON-object string.
    json.JSONDecodeError
        If ``variance`` is a string that is not valid JSON.

    Examples:
    --------
    >>> from finstack_quant.reporting.statements_common import variance_table
    >>> row = {"period": "2025", "metric": "ebitda", "baseline": 10, "comparison": 12, "abs_var": 2, "pct_var": 0.2}
    >>> "ebitda" in variance_table({"rows": [row]})
    True
    """
    if variance is None:
        return None
    rows = json_or_dict(variance, noun="variance").get("rows")
    if not rows:
        return None

    def _pct_disp(v: Any) -> Any:
        return v * 100.0 if isinstance(v, (int, float)) else None

    table_rows = [
        {
            "Period": r.get("period"),
            "Metric": r.get("metric"),
            "Baseline": r.get("baseline"),
            "Comparison": r.get("comparison"),
            "Abs Δ": r.get("abs_var"),
            "% Δ": _pct_disp(r.get("pct_var")),
        }
        for r in rows
        if isinstance(r, dict)
    ]
    if not table_rows:
        return None
    return tables.data_table(
        table_rows,
        columns=["Period", "Metric", "Baseline", "Comparison", "Abs Δ", "% Δ"],
        formats={
            "Baseline": fmt.money,
            "Comparison": fmt.money,
            "Abs Δ": fmt.money,
            "% Δ": lambda v: fmt.pct(v, signed=True),
        },
        neg_columns={"Abs Δ", "% Δ"},
    )


def _section_variance(variance: Any) -> Section | None:
    body = variance_table(variance)
    return Section("Variance vs Baseline", body) if body is not None else None
