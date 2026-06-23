# finstack-quant-py/finstack_quant/reporting/scenario.py
"""Scenario & sensitivity tear sheet.

Driver tornado, scenario comparison, Monte-Carlo percentile fan, and variance
vs baseline.

Pure presentation — every input is a pre-built shape (tornado entries, a
``{scenario: value}`` dict, a Monte-Carlo fan dict, a ``run_variance`` dict).
No engine wiring or financial calculation; the only transforms are a magnitude
sort key and display-unit percent scaling.
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt
from .document import KPI, Section, TearSheet
from .statements_common import variance_table
from .theme import INSTITUTIONAL, Theme

ALL_SECTIONS = ["tornado", "scenarios", "montecarlo", "variance"]


def _section_tornado(tornado: Any, theme: Theme) -> Section | None:
    if not tornado or not isinstance(tornado, list):
        return None
    entries = [(e.get("parameter_id"), e.get("downside"), e.get("upside")) for e in tornado]
    entries.sort(key=lambda t: abs(t[1] or 0.0) + abs(t[2] or 0.0), reverse=True)
    return Section(
        "Driver Sensitivity",
        charts.tornado_chart(entries, theme=theme),
        subtitle="Impact of low/high driver shifts on the target metric.",
    )


def _section_scenarios(scenarios: Any, theme: Theme) -> Section | None:
    if not scenarios or not isinstance(scenarios, dict):
        return None
    labels = list(scenarios.keys())
    values = [scenarios[k] for k in labels]
    return Section("Scenario Comparison", charts.bar_chart(labels, values, theme=theme))


def _section_montecarlo(monte_carlo: Any, breach_probability: Any, theme: Theme) -> Section | None:
    if not monte_carlo or not isinstance(monte_carlo, dict):
        return None
    periods = monte_carlo.get("periods") or []
    if not periods:
        return None
    body = charts.fan_chart(
        list(periods),
        list(monte_carlo.get("p_low") or []),
        list(monte_carlo.get("p_mid") or []),
        list(monte_carlo.get("p_high") or []),
        theme=theme,
    )
    subtitle = None
    if isinstance(breach_probability, (int, float)):
        subtitle = f"Breach probability: {fmt.pct(breach_probability * 100)}"
    return Section("Monte Carlo Distribution", body, subtitle=subtitle)


def _section_variance(variance: Any, theme: Theme) -> Section | None:
    body = variance_table(variance, theme=theme)
    return Section("Variance vs Baseline", body) if body is not None else None


def _mc_last(monte_carlo: Any, key: str) -> float | None:
    if not isinstance(monte_carlo, dict):
        return None
    arr = monte_carlo.get(key) or []
    return arr[-1] if arr else None


def scenario_tearsheet(
    *,
    tornado: Any = None,
    scenarios: Any = None,
    monte_carlo: Any = None,
    variance: Any = None,
    breach_probability: float | None = None,
    target_metric: str | None = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a scenario & sensitivity :class:`TearSheet`.

    All data inputs are optional; each section renders only when its input is
    supplied (graceful degradation). ``sections`` controls inclusion, not order.

    Parameters
    ----------
    tornado : list[dict], optional
        ``generate_tornado_entries`` output (``parameter_id``/``downside``/``upside``).
    scenarios : dict, optional
        ``{scenario_name: target_metric_value}`` for the comparison bars.
    monte_carlo : dict, optional
        A pre-extracted fan: ``{"periods": [...], "p_low": [...], "p_mid": [...], "p_high": [...]}``.
    variance : dict, optional
        A ``run_variance`` result (``{"rows": [...]}``).
    breach_probability : float, optional
        Fraction (e.g. ``0.12``) used for the breach KPI and the MC caption.
    target_metric : str, optional
        Name of the metric being analysed (header subtitle only).
    title, subtitle : str, optional
        Header text.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` (default: all).
    theme : Theme
        Visual theme.
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.

    Raises:
        ValueError: If ``sections`` contains an unknown section name.
    """
    wanted = sections if sections is not None else ALL_SECTIONS
    unknown = set(wanted) - set(ALL_SECTIONS)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; valid sections: {ALL_SECTIONS}")

    secs: list[Section] = []
    if "tornado" in wanted and (s := _section_tornado(tornado, theme)) is not None:
        secs.append(s)
    if "scenarios" in wanted and (s := _section_scenarios(scenarios, theme)) is not None:
        secs.append(s)
    if "montecarlo" in wanted and (s := _section_montecarlo(monte_carlo, breach_probability, theme)) is not None:
        secs.append(s)
    if "variance" in wanted and (s := _section_variance(variance, theme)) is not None:
        secs.append(s)

    breach = fmt.pct(breach_probability * 100) if isinstance(breach_probability, (int, float)) else "·"
    kpis = [
        KPI("P5 (Downside)", fmt.money(_mc_last(monte_carlo, "p_low")), ""),
        KPI("Median (P50)", fmt.money(_mc_last(monte_carlo, "p_mid")), ""),
        KPI("P95 (Upside)", fmt.money(_mc_last(monte_carlo, "p_high")), ""),
        KPI("Breach Prob.", breach, ""),
    ]

    return TearSheet(
        theme=theme,
        eyebrow="Scenario & Sensitivity",
        title=title or "Scenario & Sensitivity",
        subtitle=subtitle if subtitle is not None else (f"Target metric: {target_metric}" if target_metric else None),
        meta_lines=["Decimal mode"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=title or "Scenario & Sensitivity",
    )
