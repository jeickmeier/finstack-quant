"""Benchmark-relative tear sheet: alpha/beta, capture, rolling greeks, relative series.

Pure presentation over ``analytics.Performance`` (the engine computes; this
formats). Decimal metrics are scaled x100 only for percent display, matching the
``performance.py`` idiom. Mirrors ``performance_tearsheet``: pass a ``Performance``
built with a benchmark column and select the fund with ``ticker``.

Examples:
--------
>>> import pandas as pd
>>> from finstack_quant.analytics import Performance
>>> from finstack_quant.reporting.benchmark import benchmark_tearsheet
>>> dates = pd.bdate_range("2025-01-02", periods=80)
>>> fund = [0.001 if i % 2 else -0.0004 for i in range(80)]
>>> bench = [0.0008 if i % 3 else -0.0003 for i in range(80)]
>>> perf = Performance.from_returns(
...     pd.DataFrame({"Fund": fund, "Benchmark": bench}, index=dates), benchmark_ticker="Benchmark"
... )
>>> benchmark_tearsheet(perf, sections=[]).title
'Fund'
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet, _resolve_sections
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "benchmark_tearsheet",
]

ALL_SECTIONS = ["summary", "relative", "rolling", "multifactor"]

_SECONDARY = "#3a5a82"


def _mf_get(mf: Any, key: str) -> Any:
    """Read a field from a ``MultiFactorResult`` object or a dict."""
    return mf.get(key) if isinstance(mf, dict) else getattr(mf, key, None)


def _section_summary(perf: Any, t: int, rf: float) -> Section:
    g = perf.greeks(rf)[t]
    b = perf.beta()[t]
    regression = tables.kv_table(
        [
            ("Alpha (ann.)", fmt.pct(g.alpha * 100.0, signed=True), fmt.sign_class(g.alpha)),
            ("Beta", fmt.ratio(g.beta), ""),
            ("Beta 95% CI", f"[{fmt.ratio(b.ci_lower)}, {fmt.ratio(b.ci_upper)}]", ""),
            ("R-Squared", fmt.ratio(g.r_squared), ""),
            ("Adjusted R-Squared", fmt.ratio(g.adjusted_r_squared), ""),
        ],
    )
    active = tables.kv_table(
        [
            ("Tracking Error", fmt.pct(perf.tracking_error().iloc[t] * 100.0), ""),
            ("Information Ratio", fmt.ratio(perf.information_ratio().iloc[t]), ""),
            ("Treynor", fmt.pct(perf.treynor(rf).iloc[t] * 100.0, signed=True), ""),
            ("M-Squared", fmt.pct(perf.m_squared(rf).iloc[t] * 100.0, signed=True), ""),
        ],
    )
    capture = tables.kv_table(
        [
            ("Up Capture", fmt.ratio(perf.up_capture().iloc[t]), ""),
            ("Down Capture", fmt.ratio(perf.down_capture().iloc[t]), ""),
            ("Capture Ratio", fmt.ratio(perf.capture_ratio().iloc[t]), ""),
            ("Batting Average", fmt.pct(perf.batting_average().iloc[t] * 100.0), ""),
        ],
    )
    return Section("Benchmark-Relative Statistics", f'<div class="statgrid">{regression}{active}{capture}</div>')


def _section_relative(perf: Any, t: int, theme: Theme) -> Section:
    dates = fmt._dates_of(perf.to_cumulative_returns_dataframe())
    op = perf.cumulative_returns_outperformance()[t]
    dd = perf.drawdown_difference()[t]
    n_op = min(len(dates), len(op))
    n_dd = min(len(dates), len(dd))
    op_svg = charts.line_chart(
        dates[:n_op],
        [v * 100.0 for v in op[:n_op]],
        theme=theme,
        area=True,
        y_pct=True,
        zero=True,
        color=theme.ink,
        fill=charts.rgba(theme.ink, 0.12),
        height=190,
    )
    dd_svg = charts.line_chart(
        dates[:n_dd],
        [v * 100.0 for v in dd[:n_dd]],
        theme=theme,
        area=True,
        y_pct=True,
        zero=True,
        color=_SECONDARY,
        fill=charts.rgba(_SECONDARY, 0.13),
        height=190,
    )
    return Section(
        "Relative to Benchmark",
        f'<div class="grid2"><div><p class="sub">Cumulative excess return</p>{op_svg}</div>'
        f'<div><p class="sub">Relative drawdown</p>{dd_svg}</div></div>',
    )


def _section_rolling(perf: Any, t: int, window: int, theme: Theme) -> Section | None:
    rg = perf.rolling_greeks(t, window=window)
    if len(rg.dates) == 0:
        return None
    dates = list(rg.dates)
    a_svg = charts.line_chart(
        dates, [float(v) * 100.0 for v in rg.alphas], theme=theme, y_pct=True, color=theme.ink, height=170
    )
    b_svg = charts.line_chart(dates, [float(v) for v in rg.betas], theme=theme, color=_SECONDARY, height=170)
    return Section(
        f"Rolling {window}-Period Alpha & Beta",
        f'<div class="grid2"><div><p class="sub">Rolling alpha (ann.)</p>{a_svg}</div>'
        f'<div><p class="sub">Rolling beta</p>{b_svg}</div></div>',
    )


def _section_multifactor(mf: Any, factor_names: list[str] | None) -> Section | None:
    if mf is None:
        return None
    alpha = _mf_get(mf, "alpha")
    betas = _mf_get(mf, "betas")
    if betas is None:
        betas = []
    rows = [
        (
            "Alpha (ann.)",
            fmt.pct(alpha * 100.0, signed=True) if isinstance(alpha, (int, float)) else "·",
            fmt.sign_class(alpha) if isinstance(alpha, (int, float)) else "",
        )
    ]
    names = factor_names or [f"Factor {i + 1}" for i in range(len(betas))]
    for i, beta in enumerate(betas):
        label = names[i] if i < len(names) else f"Factor {i + 1}"
        rows.append((f"{label} Beta", fmt.ratio(float(beta)), ""))
    rows.append(("R-Squared", fmt.ratio(_mf_get(mf, "r_squared")), ""))
    rows.append(("Adjusted R-Squared", fmt.ratio(_mf_get(mf, "adjusted_r_squared")), ""))
    rv = _mf_get(mf, "residual_vol")
    rows.append(("Residual Vol", fmt.pct(rv * 100.0) if isinstance(rv, (int, float)) else "·", ""))
    return Section("Multi-Factor Attribution", tables.kv_table(rows))


def benchmark_tearsheet(
    perf: Any,
    *,
    ticker: int | None = None,
    risk_free_rate: float = 0.0,
    window: int = 63,
    multi_factor: Any = None,
    factor_names: list[str] | None = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a benchmark-relative :class:`TearSheet` from an ``analytics.Performance``.

    Parameters
    ----------
    perf : analytics.Performance
        A performance engine built with a benchmark column (``benchmark_ticker=``).
    ticker : int, optional
        Zero-based column index of the fund (default: the first non-benchmark column).
    risk_free_rate : float, default 0.0
        Used for ``greeks``/``treynor``/``m_squared``.
    window : int, default 63
        Rolling window (periods) for ``rolling_greeks``.
    multi_factor : MultiFactorResult | dict, optional
        A pre-computed ``multi_factor_greeks`` result; enables the multi-factor section.
    factor_names : list[str], optional
        Labels for the multi-factor betas (default ``Factor 1..n``).
    title : str, optional
        Optional main report heading; defaults derive from the ticker names.
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

    Returns:
    -------
    TearSheet
        Benchmark-relative report for the selected fund series and sections.

    Examples:
    --------
    >>> import pandas as pd
    >>> from finstack_quant.analytics import Performance
    >>> from finstack_quant.reporting.benchmark import benchmark_tearsheet
    >>> dates = pd.bdate_range("2025-01-02", periods=80)
    >>> fund = [0.001 if i % 2 else -0.0004 for i in range(80)]
    >>> bench = [0.0008 if i % 3 else -0.0003 for i in range(80)]
    >>> perf = Performance.from_returns(
    ...     pd.DataFrame({"Fund": fund, "Benchmark": bench}, index=dates), benchmark_ticker="Benchmark"
    ... )
    >>> benchmark_tearsheet(perf, sections=[]).title
    'Fund'
    """
    wanted = _resolve_sections(sections, ALL_SECTIONS)

    names = list(perf.ticker_names)
    bench_idx = perf.benchmark_idx
    t = ticker if ticker is not None else next((i for i in range(len(names)) if i != bench_idx), 0)

    secs: list[Section] = []
    if "summary" in wanted:
        secs.append(_section_summary(perf, t, risk_free_rate))
    if "relative" in wanted:
        secs.append(_section_relative(perf, t, theme))
    if "rolling" in wanted and (s := _section_rolling(perf, t, window, theme)) is not None:
        secs.append(s)
    if "multifactor" in wanted and (s := _section_multifactor(multi_factor, factor_names)) is not None:
        secs.append(s)

    g = perf.greeks(risk_free_rate)[t]
    kpis = [
        KPI("Alpha (ann.)", fmt.pct(g.alpha * 100.0, signed=True), fmt.sign_class(g.alpha)),
        KPI("Beta", fmt.ratio(g.beta), ""),
        KPI("Information Ratio", fmt.ratio(perf.information_ratio().iloc[t]), ""),
        KPI("Capture Ratio", fmt.ratio(perf.capture_ratio().iloc[t]), ""),
    ]

    fund_name = names[t] if t < len(names) else "Fund"
    bench_name = names[bench_idx] if 0 <= bench_idx < len(names) else "Benchmark"
    return TearSheet(
        theme=theme,
        eyebrow="Benchmark-Relative Review",
        title=title or str(fund_name),
        subtitle=subtitle if subtitle is not None else f"vs {bench_name}",
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=str(title or fund_name),
    )
