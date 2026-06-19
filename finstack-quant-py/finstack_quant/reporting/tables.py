# finstack-quant-py/finstack_quant/reporting/tables.py
"""HTML table primitives: key/value fact tables, generic data tables, heatmaps."""

from __future__ import annotations

from collections.abc import Callable
import html
from typing import Any

from . import charts
from .theme import Theme

_MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def _esc(x: Any) -> str:
    return html.escape(str(x))


def scroll(inner_html: str) -> str:
    """Wrap a (tall) table in a fixed-height vertical scroll container."""
    return f'<div class="fq-scroll">{inner_html}</div>'


def kv_table(rows: list[tuple[str, str, str]], *, theme: Theme) -> str:  # noqa: ARG001
    """Render key/value rows. Each row is ``(label, value_str, value_css_class)``."""
    body = "".join(f'<tr><td class="k">{_esc(k)}</td><td class="v {cls}">{_esc(v)}</td></tr>' for k, v, cls in rows)
    return f'<table class="kv"><tbody>{body}</tbody></table>'


def data_table(
    rows: list[dict[str, Any]],
    *,
    columns: list[str],
    theme: Theme,  # noqa: ARG001
    formats: dict[str, Callable[[Any], str]] | None = None,
    neg_columns: set[str] | None = None,
) -> str:
    """Render a generic table from row dicts, applying per-column formatters."""
    formats = formats or {}
    neg_columns = neg_columns or set()
    head = "".join(f"<th>{_esc(c)}</th>" for c in columns)
    body_rows = []
    for row in rows:
        cells = []
        for c in columns:
            raw = row.get(c)
            text = formats[c](raw) if c in formats and raw is not None else _esc(raw)
            cls = "neg" if (c in neg_columns and isinstance(raw, (int, float)) and raw < 0) else ""
            cells.append(f'<td class="{cls}">{text}</td>')
        body_rows.append(f"<tr>{''.join(cells)}</tr>")
    return f'<table class="dd"><thead><tr>{head}</tr></thead><tbody>{"".join(body_rows)}</tbody></table>'


def heatmap(rows: list[tuple[int, list[Any], Any]], *, theme: Theme) -> str:
    """Render a monthly/annual return heatmap.

    ``rows`` is ``[(year, [12 month values in percent], year_total_in_percent), ...]``;
    values may be ``None`` for missing months. Magnitude shading via
    :func:`charts.color_scale`.
    """
    head = '<tr><th class="yr"></th>' + "".join(f"<th>{m}</th>" for m in _MONTHS) + '<th class="ytd">Year</th></tr>'
    body_rows = []
    for year, months, total in rows:
        cells = [f'<td class="yr">{year}</td>']
        for v in months:
            bg, fg = charts.color_scale(v, theme)
            txt = "·" if v is None else f"{'+' if v >= 0 else ''}{v:.1f}"
            cells.append(f'<td style="background:{bg};color:{fg}">{txt}</td>')
        bg, fg = charts.color_scale(total, theme)
        ttxt = "·" if total is None else f"{'+' if total >= 0 else ''}{total:.1f}"
        cells.append(f'<td class="ytd" style="background:{bg};color:{fg}">{ttxt}</td>')
        body_rows.append(f"<tr>{''.join(cells)}</tr>")
    return f'<table class="hm"><thead>{head}</thead><tbody>{"".join(body_rows)}</tbody></table>'
