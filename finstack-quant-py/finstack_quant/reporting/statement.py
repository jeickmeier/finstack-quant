# finstack-quant-py/finstack_quant/reporting/statement.py
"""Statement (P&L) tear sheet: render statement results into HTML.

Pure presentation — reads pre-computed node values (including margin/growth
formula nodes) and lays them out. No financial calculation; the only value
transform is display-unit scaling for variance percentages, matching the
``performance.py`` idiom.
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet
from .statements_common import parse_statement, pl_matrix_table
from .theme import INSTITUTIONAL, Theme

ALL_SECTIONS = ["summary", "trend", "margins", "variance"]

# (label, node_id, formatter). Money rows first, then percent-valued margin/growth nodes.
_PL_ROWS: list[tuple[str, str, Any]] = [
    ("Revenue", "revenue", fmt.money),
    ("COGS", "cogs", fmt.money),
    ("Gross Profit", "gross_profit", fmt.money),
    ("Operating Expenses", "opex", fmt.money),
    ("EBITDA", "ebitda", fmt.money),
    ("EBIT", "ebit", fmt.money),
    ("Net Income", "net_income", fmt.money),
    ("Gross Margin", "gross_margin", fmt.pct),
    ("EBITDA Margin", "ebitda_margin", fmt.pct),
    ("Net Margin", "net_margin", fmt.pct),
    ("Revenue Growth", "revenue_growth", lambda v: fmt.pct(v, signed=True)),
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
    gm = [view.get("gross_margin", p) for p in periods]
    em = [view.get("ebitda_margin", p) for p in periods]
    if all(v is None for v in (*gm, *em)):
        return None
    gm_svg = charts.line_chart(list(periods), gm, theme=theme, y_pct=True, color=theme.ink)
    em_svg = charts.line_chart(list(periods), em, theme=theme, y_pct=True, color="#3a5a82")
    return Section("Margins", f'<div class="grid2"><div>{gm_svg}</div><div>{em_svg}</div></div>')


def _section_variance(variance: Any, theme: Theme) -> Section | None:
    rows = variance.get("rows") if isinstance(variance, dict) else None
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
    ]
    return Section(
        "Variance vs Baseline",
        tables.data_table(
            table_rows,
            columns=["Period", "Metric", "Baseline", "Comparison", "Abs Δ", "% Δ"],
            formats={
                "Baseline": fmt.money,
                "Comparison": fmt.money,
                "Abs Δ": fmt.money,
                "% Δ": lambda v: fmt.pct(v, signed=True),
            },
            neg_columns={"Abs Δ", "% Δ"},
            theme=theme,
        ),
    )


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
    title, subtitle : str, optional
        Header text; defaults derive from the data.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` (default: all).
    theme : Theme
        Visual theme (default :data:`INSTITUTIONAL`).
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.

    Raises:
        ValueError: If ``sections`` contains an unknown section name.
    """
    wanted = sections if sections is not None else ALL_SECTIONS
    unknown = set(wanted) - set(ALL_SECTIONS)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; valid sections: {ALL_SECTIONS}")

    view = parse_statement(results)
    pers = periods if periods is not None else view.periods()

    secs: list[Section] = []
    if "summary" in wanted:
        rows = _summary_rows(view, line_items)
        secs.append(Section("Income Statement", pl_matrix_table(view, rows, pers, theme=theme)))
    if "trend" in wanted:
        secs.append(_section_trend(view, pers, theme))
    if "margins" in wanted and (s := _section_margins(view, pers, theme)) is not None:
        secs.append(s)
    if "variance" in wanted and (s := _section_variance(variance, theme)) is not None:
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
            KPI("EBITDA Margin", fmt.pct(_latest("ebitda_margin")), ""),
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
