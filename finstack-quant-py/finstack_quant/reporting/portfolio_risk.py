# finstack-quant-py/finstack_quant/reporting/portfolio_risk.py
"""Portfolio risk tear sheet: Euler VaR/ES contributions and risk budget.

Pure presentation — reads pre-computed ``*_var_decomposition`` /
``*_es_decomposition`` / ``evaluate_risk_budget`` results and lays them out.
Risk shares come from the engine's ``pct_contribution``; no calculation here.
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet
from .statements_common import json_or_dict
from .theme import INSTITUTIONAL, Theme

ALL_SECTIONS = ["contributions", "es", "budget"]
_TOP_N = 12


def _pct(x: Any) -> str:
    return fmt.pct(x * 100) if isinstance(x, (int, float)) else "·"


def _section_contributions(decomp: dict[str, Any], theme: Theme) -> Section | None:
    contribs = [c for c in (decomp.get("contributions") or []) if isinstance(c, dict)]
    if not contribs:
        return None
    contribs = sorted(contribs, key=lambda c: c.get("component_var") or 0.0, reverse=True)
    top = contribs[:_TOP_N]
    chart = charts.bar_chart(
        [str(c.get("position_id")) for c in top], [c.get("component_var") for c in top], theme=theme
    )
    rows = [
        {
            "Position": c.get("position_id"),
            "Component VaR": fmt.money(c.get("component_var")),
            "% of Total": _pct(c.get("pct_contribution")),
            "Marginal": fmt.money(c.get("marginal_var")),
            "Incremental": fmt.money(c.get("incremental_var")),
        }
        for c in contribs
    ]
    table = tables.data_table(
        rows, columns=["Position", "Component VaR", "% of Total", "Marginal", "Incremental"], theme=theme
    )
    sub = f"Top {len(top)} of {len(contribs)} positions by component VaR." if len(contribs) > len(top) else None
    return Section("VaR Contributions", f"{chart}{table}", subtitle=sub)


def _section_es(es: dict[str, Any] | None, theme: Theme) -> Section | None:
    contribs = [c for c in (es or {}).get("contributions") or [] if isinstance(c, dict)]
    if not contribs:
        return None
    contribs = sorted(contribs, key=lambda c: c.get("component_es") or 0.0, reverse=True)
    rows = [
        {
            "Position": c.get("position_id"),
            "Component ES": fmt.money(c.get("component_es")),
            "% of Total": _pct(c.get("pct_contribution")),
        }
        for c in contribs
    ]
    return Section(
        "ES Contributions", tables.data_table(rows, columns=["Position", "Component ES", "% of Total"], theme=theme)
    )


def _section_budget(budget: dict[str, Any] | None, theme: Theme) -> Section | None:
    positions = [p for p in (budget or {}).get("positions") or [] if isinstance(p, dict)]
    if not positions:
        return None
    rows = [
        {
            "Position": p.get("position_id"),
            "Actual": fmt.money(p.get("actual_component_var")),
            "Target": fmt.money(p.get("target_component_var")),
            "Utilization": _pct(p.get("utilization")),
            "Excess": fmt.money(p.get("excess")),
            "Status": "Breach" if p.get("breach") else "OK",
        }
        for p in positions
    ]
    table = tables.data_table(
        rows, columns=["Position", "Actual", "Target", "Utilization", "Excess", "Status"], theme=theme
    )
    b = budget or {}
    sub = f"Breach — total over-budget {fmt.money(b.get('total_overbudget'))}." if b.get("has_breach") else None
    return Section("Risk Budget", table, subtitle=sub)


def portfolio_risk_tearsheet(
    decomposition: Any,
    *,
    es: Any = None,
    budget: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a portfolio-risk :class:`TearSheet` (Euler VaR/ES + risk budget).

    Parameters
    ----------
    decomposition : dict | str
        A ``parametric_/historical_var_decomposition`` result (dict or JSON).
    es : dict | str, optional
        A ``*_es_decomposition`` result; enables the ES-contributions section.
    budget : dict | str, optional
        An ``evaluate_risk_budget`` result; enables the risk-budget section.
    title, subtitle : str, optional
        Header text.
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
        If ``decomposition`` is neither a dict nor a JSON string.
    """
    wanted = sections if sections is not None else ALL_SECTIONS
    unknown = set(wanted) - set(ALL_SECTIONS)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; valid sections: {ALL_SECTIONS}")

    decomp = json_or_dict(decomposition, noun="decomposition")
    es_d = json_or_dict(es, noun="es") if es is not None else None
    budget_d = json_or_dict(budget, noun="budget") if budget is not None else None

    secs: list[Section] = []
    if "contributions" in wanted and (s := _section_contributions(decomp, theme)) is not None:
        secs.append(s)
    if "es" in wanted and (s := _section_es(es_d, theme)) is not None:
        secs.append(s)
    if "budget" in wanted and (s := _section_budget(budget_d, theme)) is not None:
        secs.append(s)

    conf = decomp.get("confidence")
    method = decomp.get("method")
    kpis = [
        KPI("Portfolio VaR", fmt.money(decomp.get("portfolio_var")), ""),
        KPI("Portfolio ES", fmt.money(decomp.get("portfolio_es")), ""),
        KPI("Confidence", _pct(conf), ""),
        KPI("Method" if method else "Positions", str(method) if method else str(decomp.get("n_positions") or "·"), ""),
    ]

    return TearSheet(
        theme=theme,
        eyebrow="Portfolio Risk",
        title=title or "Portfolio Risk",
        subtitle=subtitle
        if subtitle is not None
        else (f"{_pct(conf)} confidence" if isinstance(conf, (int, float)) else None),
        meta_lines=["Euler VaR/ES decomposition"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "Portfolio Risk",
    )
