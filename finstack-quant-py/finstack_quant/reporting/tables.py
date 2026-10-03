"""HTML table primitives: key/value fact tables, generic data tables, heatmaps.

Examples:
--------
>>> from finstack_quant.reporting.tables import scroll
>>> scroll("<table></table>")
'<div class="fq-scroll"><table></table></div>'
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from . import charts, format as fmt
from .theme import Theme

__all__ = [
    "data_table",
    "heatmap",
    "kv_table",
    "scroll",
]

_MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def scroll(inner_html: str) -> str:
    """Wrap tall HTML in a fixed-height vertical scroll container.

    Parameters
    ----------
    inner_html : str
        Already escaped and rendered HTML content, normally a report table.

    Returns:
    -------
    str
        HTML ``div`` containing the supplied markup with the scrolling CSS class.

    Notes:
    -----
    This helper does not raise; it wraps the supplied markup without parsing it.

    Examples:
    --------
    >>> from finstack_quant.reporting.tables import scroll
    >>> scroll("<table></table>")
    '<div class="fq-scroll"><table></table></div>'
    """
    return f'<div class="fq-scroll">{inner_html}</div>'


def kv_table(rows: list[tuple[str, str, str]]) -> str:
    """Render a key/value HTML table.

    Parameters
    ----------
    rows : list[tuple[str, str, str]]
        ``(label, display_value, CSS_class)`` rows in desired display order.

    Returns:
    -------
    str
        Escaped two-column HTML table in the supplied row order.

    Notes:
    -----
    This helper does not raise; cell values are escaped and missing numbers use the report placeholder.

    Examples:
    --------
    >>> from finstack_quant.reporting.tables import kv_table
    >>> "PV" in kv_table([("PV", "1.2m USD", "pos")])
    True
    """
    body = "".join(
        f'<tr><td class="k">{fmt._escape_html(k)}</td><td class="v {cls}">{fmt._escape_html(v)}</td></tr>'
        for k, v, cls in rows
    )
    return f'<table class="kv"><tbody>{body}</tbody></table>'


def data_table(
    rows: list[dict[str, Any]],
    *,
    columns: list[str],
    formats: dict[str, Callable[[Any], str]] | None = None,
    neg_columns: set[str] | None = None,
) -> str:
    """Render a generic HTML table from row dictionaries.

    Parameters
    ----------
    rows : list[dict[str, Any]]
        Display rows whose keys are selected by ``columns``.
    columns : list[str]
        Ordered column names to render from every row dictionary.
    formats : dict[str, Callable[[Any], str]] or None
        Optional per-column display functions; unlisted values are HTML escaped.
    neg_columns : set[str] or None
        Columns whose negative numeric values receive the ``neg`` CSS class.

    Returns:
    -------
    str
        HTML table with the requested columns, formatting, and negative-value classes.

    Notes:
    -----
    This helper does not raise; cell values are escaped and missing numbers use the report placeholder.

    Examples:
    --------
    >>> from finstack_quant.reporting.tables import data_table
    >>> html = data_table([{"Name": "Alpha", "PnL": -2}], columns=["Name", "PnL"], neg_columns={"PnL"})
    >>> 'class="neg"' in html
    True
    """
    formats = formats or {}
    neg_columns = neg_columns or set()
    head = "".join(f"<th>{fmt._escape_html(c)}</th>" for c in columns)
    body_rows = []
    for row in rows:
        cells = []
        for c in columns:
            raw = row.get(c)
            if raw is None:
                text = fmt._PLACEHOLDER
            elif c in formats:
                text = formats[c](raw)
            else:
                text = fmt._escape_html(raw)
            cls = "neg" if (c in neg_columns and isinstance(raw, (int, float)) and raw < 0) else ""
            cells.append(f'<td class="{cls}">{text}</td>')
        body_rows.append(f"<tr>{''.join(cells)}</tr>")
    return f'<table class="dd"><thead><tr>{head}</tr></thead><tbody>{"".join(body_rows)}</tbody></table>'


def heatmap(rows: list[tuple[int, list[Any], Any]], *, theme: Theme) -> str:
    """Render a monthly/annual return heatmap.

    ``rows`` is ``[(year, [12 month values in percent], year_total_in_percent), ...]``;
    values may be ``None`` for missing months. Magnitude shading via
    :func:`charts.color_scale`.

    Parameters
    ----------
    rows : list[tuple[int, list[Any], Any]]
        ``(year, monthly_percentage_points, annual_percentage_points)`` rows;
        each monthly series has twelve entries and may contain missing values.
    theme : Theme
        Report palette used to shade positive and negative performance cells.

    Returns:
    -------
    str
        HTML table containing month cells and the annual total for each year.

    Notes:
    -----
    This helper does not raise; cell values are escaped and missing numbers use the report placeholder.

    Examples:
    --------
    >>> from finstack_quant.reporting.tables import heatmap
    >>> from finstack_quant.reporting.theme import INSTITUTIONAL
    >>> html = heatmap([(2025, [1.0] * 12, 12.7)], theme=INSTITUTIONAL)
    >>> "2025" in html and "Year" in html
    True
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
