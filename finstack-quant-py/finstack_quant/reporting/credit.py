"""Credit profile tear sheet.

Renders leverage/coverage trend, per-instrument coverage, covenant compliance,
and an EBITDA build.

Pure presentation — reads the structured ``credit_assessment`` result, its
per-period series, caller-supplied coverage/covenant rows, and statement nodes.
No financial calculation.

Examples:
--------
>>> from finstack_quant.reporting.credit import credit_tearsheet
>>> credit_tearsheet({}, sections=[]).title
'Credit Assessment'
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet, _resolve_sections
from .statements_common import StatementView, json_or_dict, parse_statement, pl_matrix_table
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "credit_tearsheet",
]

ALL_SECTIONS = ["ratios", "coverage", "covenants", "pl"]

# EBITDA-build rows (label, node_id, formatter) for the P&L section.
_EBITDA_BUILD: list[tuple[str, str, Any]] = [
    ("Revenue", "revenue", fmt.money),
    ("COGS", "cogs", fmt.money),
    ("Gross Profit", "gross_profit", fmt.money),
    ("Operating Expenses", "opex", fmt.money),
    ("EBITDA", "ebitda", fmt.money),
    ("Interest Expense", "interest_expense", fmt.money),
]


def _section_ratios(assessment: dict[str, Any], theme: Theme) -> Section | None:
    series = [pt for pt in (assessment.get("series") or []) if isinstance(pt, dict)]
    if not series:
        return None
    periods = [pt.get("period") for pt in series]
    lev = [pt.get("leverage_ratio") for pt in series]
    cov = [pt.get("interest_coverage") for pt in series]
    lev_svg = charts.line_chart(periods, lev, theme=theme, color=theme.ink)
    cov_svg = charts.line_chart(periods, cov, theme=theme, color="#3a5a82")
    return Section(
        "Leverage & Coverage",
        f'<div class="grid2"><div>{lev_svg}</div><div>{cov_svg}</div></div>',
        subtitle="Net Debt / TTM EBITDA (left); TTM EBITDA / TTM Interest (right).",
    )


def _section_coverage(coverage: Any) -> Section | None:
    if not coverage:
        return None
    rows = [
        {
            "Instrument": r.get("instrument"),
            "DSCR": r.get("dscr"),
            "Int. Cov.": r.get("interest_coverage"),
            "LTV": r.get("ltv"),
        }
        for r in coverage
        if isinstance(r, dict)
    ]
    if not rows:
        return None
    return Section(
        "Per-Instrument Coverage",
        tables.data_table(
            rows,
            columns=["Instrument", "DSCR", "Int. Cov.", "LTV"],
            formats={"DSCR": fmt.ratio, "Int. Cov.": fmt.ratio, "LTV": fmt.pct},
        ),
    )


def _section_covenants(covenants: Any) -> Section | None:
    if not covenants:
        return None
    rows = [
        {
            "Covenant": r.get("covenant"),
            "Threshold": r.get("threshold"),
            "Current": r.get("current"),
            "Headroom": r.get("headroom"),
            "Status": r.get("status"),
        }
        for r in covenants
        if isinstance(r, dict)
    ]
    if not rows:
        return None
    return Section(
        "Covenant Compliance",
        tables.data_table(
            rows,
            columns=["Covenant", "Threshold", "Current", "Headroom", "Status"],
            formats={"Threshold": fmt.ratio, "Current": fmt.ratio, "Headroom": fmt.ratio},
            neg_columns={"Headroom"},
        ),
    )


def _section_pl(view: StatementView | None) -> Section | None:
    if view is None:
        return None
    present = set(view.node_ids())
    rows = [row for row in _EBITDA_BUILD if row[1] in present]
    if not rows:
        return None
    return Section("EBITDA Build", pl_matrix_table(view, rows, view.periods()))


def credit_tearsheet(
    assessment: Any,
    *,
    results: Any = None,
    coverage: Any = None,
    covenants: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a credit-profile :class:`TearSheet`.

    Parameters
    ----------
    assessment : dict | str
        A structured ``credit_assessment`` result (dict or JSON string) with
        ``leverage_ratio``, ``interest_coverage``, ``free_cash_flow``, ``as_of``,
        and a ``series`` of per-period points.
    results : StatementResult | str | dict, optional
        Statement results for the EBITDA-build section.
    coverage : list[dict], optional
        Per-instrument rows (``instrument``/``dscr``/``interest_coverage``/``ltv``).
    covenants : list[dict], optional
        Covenant rows (``covenant``/``threshold``/``current``/``headroom``/``status``).
    title : str, optional
        Optional main report heading; defaults derive from the assessment.
    subtitle : str, optional
        Optional secondary heading shown below ``title``.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` (default: all).
    theme : Theme
        Visual theme.
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.

    Raises:
    ------
    ValueError
        If ``sections`` contains an unknown section name.
    TypeError
        If ``assessment`` is neither a dict nor a JSON string.

    Returns:
    -------
    TearSheet
        Credit report with KPI values and the selected available sections.

    Examples:
    --------
    >>> from finstack_quant.reporting.credit import credit_tearsheet
    >>> credit_tearsheet({}, sections=[]).title
    'Credit Assessment'
    """
    wanted = _resolve_sections(sections, ALL_SECTIONS)

    asmt = json_or_dict(assessment, noun="assessment")
    view = parse_statement(results) if results is not None else None

    secs: list[Section] = []
    if "ratios" in wanted and (s := _section_ratios(asmt, theme)) is not None:
        secs.append(s)
    if "coverage" in wanted and (s := _section_coverage(coverage)) is not None:
        secs.append(s)
    if "covenants" in wanted and (s := _section_covenants(covenants)) is not None:
        secs.append(s)
    if "pl" in wanted and (s := _section_pl(view)) is not None:
        secs.append(s)

    lev = asmt.get("leverage_ratio")
    cov = asmt.get("interest_coverage")
    fcf = asmt.get("free_cash_flow")
    as_of = asmt.get("as_of")
    kpis = [
        KPI("Leverage", f"{fmt.ratio(lev)}x" if lev is not None else "·", ""),
        KPI("Interest Coverage", f"{fmt.ratio(cov)}x" if cov is not None else "·", ""),
        KPI("Free Cash Flow", fmt.money(fcf), fmt.sign_class(fcf)),
        KPI("As Of", str(as_of) if as_of is not None else "·", ""),
    ]

    return TearSheet(
        theme=theme,
        eyebrow="Credit Profile",
        title=title or "Credit Assessment",
        subtitle=subtitle if subtitle is not None else (f"As of {as_of}" if as_of else None),
        meta_lines=view.meta_lines() if view is not None else [],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "Credit Assessment",
    )
