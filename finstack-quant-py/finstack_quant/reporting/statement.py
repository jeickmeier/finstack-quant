"""Statement (P&L) tear sheet: render statement results into HTML.

Pure presentation — reads pre-computed node values (including margin/growth
formula nodes) and lays them out. No financial calculation; the only value
transform is decimal-to-percent scaling for ratios, growth and variance percentages, matching the
``performance.py`` idiom.

Examples:
--------
>>> from finstack_quant.reporting.statement import statement_tearsheet
>>> statement_tearsheet({"nodes": {}}, sections=[]).title
'Financial Statements'
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt
from .document import KPI, Section, TearSheet, _resolve_sections
from .statements_common import _section_variance, parse_statement, pl_matrix_table
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "statement_tearsheet",
]

ALL_SECTIONS = ["summary", "trend", "margins", "variance"]

# (label, node_id, formatter). Money rows first, then decimal margin/growth nodes.
_PL_ROWS: list[tuple[str, str, Any]] = [
    ("Revenue", "revenue", fmt.money),
    ("COGS", "cogs", fmt.money),
    ("Gross Profit", "gross_profit", fmt.money),
    ("Operating Expenses", "opex", fmt.money),
    ("EBITDA", "ebitda", fmt.money),
    ("EBIT", "ebit", fmt.money),
    ("Net Income", "net_income", fmt.money),
    ("Gross Margin", "gross_margin", lambda v: fmt.pct(v * 100 if v is not None else None)),
    ("EBITDA Margin", "ebitda_margin", lambda v: fmt.pct(v * 100 if v is not None else None)),
    ("Net Margin", "net_margin", lambda v: fmt.pct(v * 100 if v is not None else None)),
    ("Revenue Growth", "revenue_growth", lambda v: fmt.pct(v * 100 if v is not None else None, signed=True)),
]


def _summary_rows(view: Any, line_items: list[str] | None) -> list[tuple[str, str, Any]]:
    catalog = {node_id: (label, node_id, formatter) for label, node_id, formatter in _PL_ROWS}
    if line_items is not None:
        ids = line_items
    else:
        present = set(view.node_ids())
        ids = [node_id for _, node_id, _ in _PL_ROWS if node_id in present]
    return [catalog.get(node_id, (node_id, node_id, fmt.money)) for node_id in ids]


def _section_trend(view: Any, periods: list[str], theme: Theme) -> Section:
    rev = [view.get("revenue", p) for p in periods]
    ebitda = [view.get("ebitda", p) for p in periods]
    rev_svg = charts.bar_chart(list(periods), rev, theme=theme)
    ebitda_svg = charts.bar_chart(list(periods), ebitda, theme=theme)
    return Section("Revenue & EBITDA", f'<div class="grid2"><div>{rev_svg}</div><div>{ebitda_svg}</div></div>')


def _section_margins(view: Any, periods: list[str], theme: Theme) -> Section | None:
    gm = [v * 100 if (v := view.get("gross_margin", p)) is not None else None for p in periods]
    em = [v * 100 if (v := view.get("ebitda_margin", p)) is not None else None for p in periods]
    if all(v is None for v in (*gm, *em)):
        return None
    gm_svg = charts.line_chart(list(periods), gm, theme=theme, y_pct=True, color=theme.ink)
    em_svg = charts.line_chart(list(periods), em, theme=theme, y_pct=True, color="#3a5a82")
    return Section("Margins", f'<div class="grid2"><div>{gm_svg}</div><div>{em_svg}</div></div>')


def statement_tearsheet(
    results: Any,
    *,
    line_items: list[str] | None = None,
    periods: list[str] | None = None,
    variance: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a P&L / statement summary :class:`TearSheet`.

    Parameters
    ----------
    results : StatementResult | str | dict
        Evaluated statement results (object, JSON, or parsed dict).
    line_items : list[str], optional
        Node ids to show in the summary table (default: the standard P&L nodes
        present in ``results``, in canonical order).
    periods : list[str], optional
        Periods (columns) to show (default: all present, ascending).
    variance : dict, optional
        A ``run_variance`` result (``{"rows": [...]}``); enables the variance section.
    title : str, optional
        Optional main report heading; defaults derive from the statement data.
    subtitle : str, optional
        Optional secondary heading shown below ``title``.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` (default: all).
    theme : Theme
        Visual theme (default :data:`INSTITUTIONAL`).
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.

    Raises:
        ValueError: If ``sections`` contains an unknown section name.

    Returns:
    -------
    TearSheet
        Statement report with the selected summary, trend, margin, and variance sections.

    Examples:
    --------
    >>> from finstack_quant.reporting.statement import statement_tearsheet
    >>> statement_tearsheet({"nodes": {}}, sections=[]).title
    'Financial Statements'
    """
    wanted = _resolve_sections(sections, ALL_SECTIONS)

    view = parse_statement(results)
    pers = periods if periods is not None else view.periods()

    secs: list[Section] = []
    if "summary" in wanted:
        rows = _summary_rows(view, line_items)
        secs.append(Section("Income Statement", pl_matrix_table(view, rows, pers)))
    if "trend" in wanted:
        secs.append(_section_trend(view, pers, theme))
    if "margins" in wanted and (s := _section_margins(view, pers, theme)) is not None:
        secs.append(s)
    if "variance" in wanted and (s := _section_variance(variance)) is not None:
        secs.append(s)

    latest = pers[-1] if pers else None

    def _latest(node: str) -> float | None:
        return view.get(node, latest) if latest is not None else None

    kpis: list[KPI] = []
    if "summary" in wanted:
        net_income = _latest("net_income")
        kpis = [
            KPI("Revenue", fmt.money(_latest("revenue")), ""),
            KPI("EBITDA", fmt.money(_latest("ebitda")), ""),
            KPI("EBITDA Margin", fmt.pct(v * 100 if (v := _latest("ebitda_margin")) is not None else None), ""),
            KPI("Net Income", fmt.money(net_income), fmt.sign_class(net_income)),
        ]

    auto_subtitle = f"{pers[0]} - {pers[-1]} ({len(pers)} periods)" if pers else None
    return TearSheet(
        theme=theme,
        eyebrow="Statement Review",
        title=title or "Financial Statements",
        subtitle=subtitle if subtitle is not None else auto_subtitle,
        meta_lines=["Decimal mode"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "Financial Statements",
    )
