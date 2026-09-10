"""Portfolio book-summary tear sheet: holdings, exposure, sensitivities, cashflows.

Pure presentation — reads pre-computed ``value_portfolio`` / ``aggregate_metrics``
/ ``aggregate_full_cashflows`` outputs and lays them out. The only value handling
is parsing decimal-string ``Money`` amounts, sorting, top-N selection, and
formatting; no financial calculation.

Examples:
--------
>>> from finstack_quant.reporting.portfolio import portfolio_tearsheet
>>> valuation = {"total_base_currency": {"amount": "0", "currency": "USD"}}
>>> portfolio_tearsheet(valuation, sections=[]).title
'Portfolio'
"""

from __future__ import annotations

import datetime as dt
from html import escape
import json
import math
from typing import Any

from finstack_quant.core.dates import Tenor
from finstack_quant.portfolio import PortfolioMetrics, net_in_currency_by_date

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet, _resolve_sections
from .statements_common import json_or_dict
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "portfolio_tearsheet",
]

ALL_SECTIONS = ["holdings", "exposure", "sensitivities", "buckets", "cashflows"]
_HOLDINGS_TOP_N = 15
_HEADLINE_METRICS = ["dv01", "cs01", "theta", "delta", "gamma", "vega", "rho", "pv01"]


def _money(m: Any) -> tuple[float, str]:
    """Parse a Money dict ``{amount, currency}`` into ``(float, currency)``."""
    if not isinstance(m, dict):
        return (float("nan"), "")
    try:
        amt = float(m.get("amount"))
    except (TypeError, ValueError):
        amt = float("nan")
    return (amt, m.get("currency") or "")


def _abs_key(x: float) -> float:
    return abs(x) if not math.isnan(x) else 0.0


def _tenor_years(tenor: str) -> float:
    try:
        return Tenor.parse(tenor).to_years()
    except ValueError:
        return 1e9  # unknown tenors sort last


def _section_holdings(val: dict[str, Any]) -> Section | None:
    pvs = val.get("position_values") or {}
    if not pvs:
        return None
    degraded = set(val.get("degraded_positions") or [])
    rows = []
    for pid, p in pvs.items():
        base_amt, base_currency = _money(p.get("value_base"))
        nat_amt, nat_currency = _money(p.get("value_native"))
        label = f"{pid} (PV only)" if pid in degraded else str(pid)
        rows.append((base_amt, base_currency, label, p.get("entity_id"), fmt.money(nat_amt, nat_currency)))
    rows.sort(key=lambda r: _abs_key(r[0]), reverse=True)
    shown = rows[:_HOLDINGS_TOP_N]
    table_rows = [{"Position": r[2], "Entity": r[3], "Native": r[4], "Base": r[0]} for r in shown]
    body = tables.data_table(
        table_rows,
        columns=["Position", "Entity", "Native", "Base"],
        formats={"Base": fmt.money},
        neg_columns={"Base"},
    )
    sub = f"Top {len(shown)} of {len(rows)} positions by base value." if len(rows) > len(shown) else None
    return Section("Holdings", body, subtitle=sub)


def _section_exposure(val: dict[str, Any], theme: Theme) -> Section | None:
    by_ent = val.get("by_entity") or {}
    if not by_ent:
        return None
    items = sorted(((ent, _money(m)[0]) for ent, m in by_ent.items()), key=lambda kv: _abs_key(kv[1]), reverse=True)
    chart = charts.bar_chart([k for k, _ in items], [v for _, v in items], theme=theme)
    table = tables.data_table(
        [{"Entity": k, "Base": v} for k, v in items],
        columns=["Entity", "Base"],
        formats={"Base": fmt.money},
        neg_columns={"Base"},
    )
    return Section("Exposure by Entity", f'<div class="grid2"><div>{chart}</div><div>{table}</div></div>')


def _section_sensitivities(metrics: dict[str, Any] | None) -> Section | None:
    agg = (metrics or {}).get("aggregated") or {}
    present = [m for m in _HEADLINE_METRICS if m in agg]
    if not present:
        return None
    entities: list[str] = []
    seen: set[str] = set()
    for m in present:
        for e in agg[m].get("by_entity") or {}:
            if e not in seen:
                seen.add(e)
                entities.append(e)
    entities.sort()
    rows = []
    for m in present:
        a = agg[m]
        be = a.get("by_entity") or {}
        row = {"Metric": m.upper(), "Total": fmt.money(a.get("total"))}
        for e in entities:
            row[e] = fmt.money(be.get(e))
        rows.append(row)
    return Section(
        "Aggregated Sensitivities",
        tables.data_table(rows, columns=["Metric", "Total", *entities]),
    )


def _bucket_chart(metrics: PortfolioMetrics, base: str, theme: Theme) -> str | None:
    items = [(components[-1], total) for components, total, _ in metrics.metric_series(base) if components]
    if not items:
        return None
    items.sort(key=lambda kv: _tenor_years(kv[0]))
    return charts.bar_chart([t for t, _ in items], [v for _, v in items], theme=theme)


def _section_buckets(metrics: dict[str, Any] | None, theme: Theme) -> Section | None:
    typed = PortfolioMetrics.from_json(json.dumps(metrics or {"aggregated": {}, "by_position": {}}))
    dv = _bucket_chart(typed, "bucketed_dv01", theme)
    cs = _bucket_chart(typed, "bucketed_cs01", theme)
    parts = []
    if dv is not None:
        parts.append(f'<div><p class="sub">Bucketed DV01 by tenor</p>{dv}</div>')
    if cs is not None:
        parts.append(f'<div><p class="sub">Bucketed CS01 by tenor</p>{cs}</div>')
    if not parts:
        return None
    body = f'<div class="grid2">{"".join(parts)}</div>' if len(parts) == 2 else parts[0]
    return Section("Tenor Risk Profile", body)


def _section_cashflows(cashflows: dict[str, Any] | None, theme: Theme) -> Section | None:
    by_date = (cashflows or {}).get("by_date") or {}
    if not by_date:
        return None
    currencies = sorted({currency for per_currency in by_date.values() for currency in per_currency})
    payload = json.dumps({"by_date": by_date})
    parts = []
    rows = []
    for currency in currencies:
        pairs = net_in_currency_by_date(payload, currency)
        if pairs:
            dates, values = zip(*pairs, strict=True)
            parts.append(
                f'<p class="sub">{escape(currency)} cashflows</p>'
                + charts.bar_chart(list(dates), list(values), theme=theme)
            )
    for date, per_currency in by_date.items():
        for currency, per_kind in per_currency.items():
            for kind, amount in per_kind.items():
                value, label = _money(amount)
                if label != currency:
                    raise ValueError("cashflow currency key must match its Money currency")
                rows.append({"Date": date, "Currency": currency, "Kind": kind, "Amount": fmt.money(value, currency)})
    parts.append(tables.data_table(rows, columns=["Date", "Currency", "Kind", "Amount"]))
    return Section(
        "Cashflow Ladder", "".join(parts), subtitle="Native-currency cashflows, shown separately by currency."
    )


def portfolio_tearsheet(
    valuation: Any,
    *,
    metrics: Any = None,
    cashflows: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a portfolio book-summary :class:`TearSheet`.

    Parameters
    ----------
    valuation : PortfolioValuation | dict | str
        A typed ``value_portfolio`` result, parsed dictionary, or canonical JSON string.
    metrics : PortfolioMetrics | dict | str, optional
        A typed ``aggregate_metrics`` result, parsed dictionary, or canonical JSON;
        enables the sensitivities and buckets sections.
    cashflows : object | dict | str, optional
        A typed ``aggregate_full_cashflows`` result, parsed dictionary, or canonical
        JSON; enables the cashflow ladder.
    title : str, optional
        Optional main report heading for the portfolio summary.
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
        If a contract argument is neither typed, a dictionary, nor a JSON string.

    Returns:
    -------
    TearSheet
        Portfolio report with headline value, position counts, and selected sections.

    Examples:
    --------
    >>> from finstack_quant.reporting.portfolio import portfolio_tearsheet
    >>> valuation = {"total_base_currency": {"amount": "0", "currency": "USD"}}
    >>> portfolio_tearsheet(valuation, sections=[]).title
    'Portfolio'
    """
    wanted = _resolve_sections(sections, ALL_SECTIONS)

    val = json_or_dict(valuation, noun="valuation")
    metrics_d = json_or_dict(metrics, noun="metrics") if metrics is not None else None
    cashflows_d = json_or_dict(cashflows, noun="cashflows") if cashflows is not None else None
    base_amt, base_currency = _money(val.get("total_base_currency"))

    secs: list[Section] = []
    if "holdings" in wanted and (s := _section_holdings(val)) is not None:
        secs.append(s)
    if "exposure" in wanted and (s := _section_exposure(val, theme)) is not None:
        secs.append(s)
    if "sensitivities" in wanted and (s := _section_sensitivities(metrics_d)) is not None:
        secs.append(s)
    if "buckets" in wanted and (s := _section_buckets(metrics_d, theme)) is not None:
        secs.append(s)
    if "cashflows" in wanted and (s := _section_cashflows(cashflows_d, theme)) is not None:
        secs.append(s)

    pvs = val.get("position_values") or {}
    by_ent = val.get("by_entity") or {}
    if by_ent:
        top_ent, top_m = max(by_ent.items(), key=lambda kv: _abs_key(_money(kv[1])[0]))
        top_kpi = f"{top_ent}: {fmt.money(_money(top_m)[0])}"
    else:
        top_kpi = "·"
    kpis = [
        KPI("Total Value", fmt.money(base_amt, base_currency), fmt.sign_class(base_amt)),
        KPI("Positions", str(len(pvs)), ""),
        KPI("Entities", str(len(by_ent)), ""),
        KPI("Top Entity", top_kpi, ""),
    ]

    fx = val.get("fx_collapse_policy")
    return TearSheet(
        theme=theme,
        eyebrow="Portfolio Summary",
        title=title or "Portfolio",
        subtitle=subtitle if subtitle is not None else (f"As of {val.get('as_of')}" if val.get("as_of") else None),
        meta_lines=[f"Base {base_currency} · FX: {fx}" if base_currency else "Decimal mode"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "Portfolio",
    )
