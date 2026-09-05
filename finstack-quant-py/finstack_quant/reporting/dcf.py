"""Corporate valuation (DCF) tear sheet.

EV→equity bridge, UFCF projection, sensitivity tornado, and a forecast summary.

Pure presentation — reads the ``evaluate_dcf`` result dict, statement nodes, and
a caller-supplied tornado-entries list. The only value transform is negating net
debt for the bridge's downward step (a display-direction sign).

Examples:
--------
>>> from finstack_quant.reporting.dcf import dcf_tearsheet
>>> dcf_tearsheet({}, sections=[]).title
'DCF Valuation'
"""

from __future__ import annotations

import datetime as dt
import math
from typing import Any

from . import charts, format as fmt
from .document import KPI, Section, TearSheet, _resolve_sections
from .statements_common import json_or_dict, parse_statement, pl_matrix_table
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "dcf_tearsheet",
]

ALL_SECTIONS = ["bridge", "ufcf", "sensitivity", "pl"]

_PL_ROWS: list[tuple[str, str, Any]] = [
    ("Revenue", "revenue", fmt.money),
    ("EBITDA", "ebitda", fmt.money),
    ("EBIT", "ebit", fmt.money),
    ("Unlevered FCF", "ufcf", fmt.money),
    ("Net Income", "net_income", fmt.money),
]


def _money_parts(value: Any) -> tuple[float | None, str | None]:
    """Split a ``Money`` wire object ``{"amount", "currency"}`` into ``(float, code)``.

    Returns ``(None, None)`` when the value is missing.
    """
    if not isinstance(value, dict) or value.get("amount") is None:
        return None, None
    return float(value["amount"]), value.get("currency")


def _section_bridge(val: dict[str, Any], theme: Theme) -> Section | None:
    ev, _ = _money_parts(val.get("enterprise_value"))
    nd, _ = _money_parts(val.get("net_debt"))
    if ev is None or nd is None:
        return None
    return Section(
        "EV → Equity Bridge",
        charts.waterfall_chart(["Enterprise Value", "− Net Debt"], [ev, -nd], theme=theme, total_label="Equity Value"),  # noqa: RUF001
        subtitle="Enterprise value less net debt equals equity value.",
    )


def _section_ufcf(results: Any, ufcf_node: str, theme: Theme) -> Section | None:
    if results is None:
        return None
    view = parse_statement(results)
    periods = view.periods()
    vals = [view.get(ufcf_node, p) for p in periods]
    if all(v is None or (isinstance(v, float) and math.isnan(v)) for v in vals):
        return None
    return Section("Unlevered Free Cash Flow", charts.bar_chart(periods, vals, theme=theme))


def _section_sensitivity(sensitivity: Any, theme: Theme) -> Section | None:
    if not sensitivity or not isinstance(sensitivity, list):
        return None
    entries: list[tuple[Any, Any, Any]] = []
    for e in sensitivity:
        if isinstance(e, dict):
            entries.append((e.get("parameter_id"), e.get("downside"), e.get("upside")))
        else:
            parameter_id = getattr(e, "parameter_id", None)
            if parameter_id is None:
                continue
            entries.append((parameter_id, getattr(e, "downside", None), getattr(e, "upside", None)))
    if not entries:
        return None
    entries.sort(key=lambda t: abs(t[1] or 0.0) + abs(t[2] or 0.0), reverse=True)
    return Section(
        "Equity Value Sensitivity",
        charts.tornado_chart(entries, theme=theme),
        subtitle="Impact on equity value of low/high parameter shifts.",
    )


def _section_pl(results: Any) -> Section | None:
    if results is None:
        return None
    view = parse_statement(results)
    present = set(view.node_ids())
    rows = [row for row in _PL_ROWS if row[1] in present]
    if not rows:
        return None
    return Section("Forecast Summary", pl_matrix_table(view, rows, view.periods()))


def dcf_tearsheet(
    valuation: Any,
    *,
    results: Any = None,
    sensitivity: Any = None,
    ufcf_node: str = "ufcf",
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a corporate-valuation (DCF) :class:`TearSheet`.

    Parameters
    ----------
    valuation : dict | str
        An ``evaluate_dcf`` result (dict or JSON string). ``enterprise_value``,
        ``equity_value`` and ``net_debt`` are ``Money`` wire objects
        (``{"amount": "<decimal string>", "currency": "USD"}``);
        ``equity_value_per_share`` is a plain float.
    results : StatementResult | str | dict, optional
        Statement results for the UFCF and forecast-summary sections.
    sensitivity : list[dict], optional
        ``generate_tornado_entries`` output (``parameter_id``/``downside``/``upside``)
        for the sensitivity tornado.
    ufcf_node : str, default "ufcf"
        Node id holding unlevered free cash flow.
    title : str, optional
        Optional main report heading for the DCF valuation summary.
    subtitle : str, optional
        Optional secondary heading shown below ``title``.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` (default: all). ``sections`` controls
        which sections are included, not their order (order is fixed).
    theme : Theme
        Visual theme.
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.

    Raises:
    ------
    ValueError
        If ``sections`` contains an unknown section name.
    TypeError
        If ``valuation`` is neither a dict nor a JSON string.

    Returns:
    -------
    TearSheet
        DCF report with headline valuation KPIs and the selected available sections.

    Examples:
    --------
    >>> from finstack_quant.reporting.dcf import dcf_tearsheet
    >>> dcf_tearsheet({}, sections=[]).title
    'DCF Valuation'
    """
    wanted = _resolve_sections(sections, ALL_SECTIONS)

    val = json_or_dict(valuation, noun="valuation")

    secs: list[Section] = []
    if "bridge" in wanted and (s := _section_bridge(val, theme)) is not None:
        secs.append(s)
    if "ufcf" in wanted and (s := _section_ufcf(results, ufcf_node, theme)) is not None:
        secs.append(s)
    if "sensitivity" in wanted and (s := _section_sensitivity(sensitivity, theme)) is not None:
        secs.append(s)
    if "pl" in wanted and (s := _section_pl(results)) is not None:
        secs.append(s)

    ev, ccy = _money_parts(val.get("enterprise_value"))
    eq, equity_currency = _money_parts(val.get("equity_value"))
    nd, _ = _money_parts(val.get("net_debt"))
    ccy = ccy or equity_currency
    per_share = val.get("equity_value_per_share")
    kpis = [
        KPI("Enterprise Value", fmt.money(ev, ccy), ""),
        KPI("Equity Value", fmt.money(eq, ccy), fmt.sign_class(eq)),
        KPI("Equity / Share", fmt.money(per_share, ccy), ""),
        KPI("Net Debt", fmt.money(nd, ccy), ""),
    ]

    return TearSheet(
        theme=theme,
        eyebrow="Corporate Valuation",
        title=title or "DCF Valuation",
        subtitle=subtitle,
        meta_lines=["Decimal mode"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "DCF Valuation",
    )
