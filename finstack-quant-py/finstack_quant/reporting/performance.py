# finstack-quant-py/finstack_quant/reporting/performance.py
"""Performance tear sheet: render an analytics.Performance into HTML.

Reads only the engine's exported DataFrames — it never recomputes analytics.
The primary series is selected positionally by ``ticker``.
"""

from __future__ import annotations

import datetime as dt
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet
from .theme import INSTITUTIONAL, Theme

ALL_SECTIONS = ["summary", "stats", "cumulative", "drawdown", "rolling", "monthly", "annual", "drawdowns"]


def _dates_of(df: Any) -> list[Any]:
    return [ix.date() if hasattr(ix, "date") else ix for ix in df.index]


def _section_cumulative(cum_dates: list[Any], cum_series: list[float], theme: Theme) -> Section:
    return Section(
        "Cumulative Return",
        charts.line_chart(
            cum_dates,
            cum_series,
            theme=theme,
            area=True,
            y_pct=True,
            zero=True,
            color=theme.ink,
            fill=charts.rgba(theme.ink, 0.12),
            height=210,
        ),
    )


def _section_stats(row: Any, total_return: float, theme: Theme) -> Section:
    col_a = tables.kv_table(
        [
            ("Total Return", fmt.pct(total_return, signed=True), fmt.sign_class(total_return)),
            ("Annualised Return", fmt.pct(row["cagr"] * 100, signed=True), fmt.sign_class(row["cagr"])),
            (
                "Geometric Mean",
                fmt.pct(row["geometric_mean"] * 100, signed=True),
                fmt.sign_class(row["geometric_mean"]),
            ),
            ("Mean Return", fmt.pct(row["mean_return"] * 100, signed=True), fmt.sign_class(row["mean_return"])),
        ],
        theme=theme,
    )
    col_b = tables.kv_table(
        [
            ("Annualised Volatility", fmt.pct(row["volatility"] * 100), ""),
            ("Sharpe Ratio", fmt.ratio(row["sharpe"]), ""),
            ("Sortino Ratio", fmt.ratio(row["sortino"]), ""),
            ("Calmar Ratio", fmt.ratio(row["calmar"]), ""),
            ("Max Drawdown", fmt.pct(row["max_drawdown"] * 100, signed=True), "neg"),
        ],
        theme=theme,
    )
    col_c = tables.kv_table(
        [
            ("Skewness", fmt.ratio(row["skewness"]), ""),
            ("Excess Kurtosis", fmt.ratio(row["kurtosis"]), ""),
            ("Value-at-Risk (95%)", fmt.pct(row["value_at_risk"] * 100, signed=True), "neg"),
            ("Expected Shortfall", fmt.pct(row["expected_shortfall"] * 100, signed=True), "neg"),
            ("Ulcer Index", fmt.ratio(row["ulcer_index"]), ""),
        ],
        theme=theme,
    )
    return Section("Risk & Return Statistics", f'<div class="statgrid">{col_a}{col_b}{col_c}</div>')


def _section_drawdown(perf: Any, ticker: int, theme: Theme) -> Section:
    dd = perf.drawdown_series_to_dataframe()
    dd_series = [v * 100.0 for v in dd[dd.columns[ticker]].tolist()]
    return Section(
        "Drawdown",
        charts.line_chart(
            _dates_of(dd),
            dd_series,
            theme=theme,
            area=True,
            y_pct=True,
            ymax=0.0,
            color=theme.neg,
            fill=charts.rgba(theme.neg, 0.13),
            height=150,
        ),
    )


def _section_rolling(perf: Any, ticker: int, theme: Theme) -> Section:
    rs = perf.rolling_sharpe(ticker, window=252).to_dataframe()
    rv = perf.rolling_volatility(ticker, window=252).to_dataframe()
    rs_svg = charts.line_chart(_dates_of(rs), rs.iloc[:, 0].tolist(), theme=theme, color=theme.ink, height=170)
    rv_svg = charts.line_chart(
        _dates_of(rv),
        [v * 100.0 for v in rv.iloc[:, 0].tolist()],
        theme=theme,
        y_pct=True,
        color="#3a5a82",
        height=170,
    )
    return Section(
        "Rolling 12-Month Sharpe & Volatility",
        f'<div class="grid2"><div>{rs_svg}</div><div>{rv_svg}</div></div>',
    )


def _build_heatmap_data(perf: Any, ticker: int) -> tuple[dict[int, list[Any]], dict[int, float]]:
    monthly = perf.periodic_returns_to_dataframe("monthly")
    annual = perf.periodic_returns_to_dataframe("annual")
    m_col = monthly[monthly.columns[ticker]]
    a_col = annual[annual.columns[ticker]]
    annual_by_year = {ix.year: v * 100.0 for ix, v in a_col.items()}
    by_year: dict[int, list[Any]] = {}
    for ix, v in m_col.items():
        by_year.setdefault(ix.year, [None] * 12)[ix.month - 1] = v * 100.0
    return by_year, annual_by_year


def _section_drawdowns(perf: Any, ticker: int, theme: Theme) -> Section:
    det = perf.drawdown_details_to_dataframe(ticker, n=5)
    dd_rows = []
    for _, r in det.iterrows():
        end = r["end"]
        dd_rows.append({
            "Peak → Trough": f"{fmt.fmt_date(r['start'])} → {fmt.fmt_date(r['valley'])}",
            "Depth": r["max_drawdown"] * 100.0,
            "Length": f"{int(r['duration_days'])} days",
            "Recovery": "ongoing" if end is None else fmt.fmt_date(end),
        })
    return Section(
        "Worst Drawdowns",
        tables.data_table(
            dd_rows,
            columns=["Peak → Trough", "Depth", "Length", "Recovery"],
            formats={"Depth": lambda v: fmt.pct(v, signed=True)},
            neg_columns={"Depth"},
            theme=theme,
        ),
    )


def performance_tearsheet(
    perf: Any,
    *,
    ticker: int = 0,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Build a performance :class:`TearSheet` from an ``analytics.Performance``.

    Parameters
    ----------
    perf : analytics.Performance
        A constructed performance engine (the caller owns its config).
    ticker : int, default 0
        Zero-based column index of the primary series.
    title : str, optional
        Optional main report heading; defaults derive from the primary series.
    subtitle : str, optional
        Optional secondary heading shown below ``title``.
    sections : list[str], optional
        Subset of :data:`ALL_SECTIONS` to render (default: all).
    theme : Theme
        Visual theme (default :data:`INSTITUTIONAL`).
    generated : datetime.date, optional
        "Generated" stamp; pass a fixed date for reproducible output.
    """
    wanted = sections if sections is not None else ALL_SECTIONS
    unknown = set(wanted) - set(ALL_SECTIONS)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; valid sections: {ALL_SECTIONS}")

    summary = perf.summary_to_dataframe()
    cum = perf.cumulative_returns_to_dataframe()
    col = cum.columns[ticker]
    row = summary.iloc[ticker]

    cum_series = [v * 100.0 for v in cum[col].tolist()]
    cum_dates = _dates_of(cum)
    total_return = cum_series[-1] if cum_series else float("nan")

    secs: list[Section] = []

    if "cumulative" in wanted:
        secs.append(_section_cumulative(cum_dates, cum_series, theme))
    if "stats" in wanted:
        secs.append(_section_stats(row, total_return, theme))
    if "drawdown" in wanted:
        secs.append(_section_drawdown(perf, ticker, theme))
    if "rolling" in wanted:
        secs.append(_section_rolling(perf, ticker, theme))

    if "monthly" in wanted or "annual" in wanted:
        by_year, annual_by_year = _build_heatmap_data(perf, ticker)
        years = sorted(by_year)
        if "monthly" in wanted:
            rows = [(y, by_year[y], annual_by_year.get(y)) for y in years]
            secs.append(
                Section(
                    "Monthly & Annual Returns",
                    tables.heatmap(rows, theme=theme),
                    subtitle="Green = positive, red = negative; shade scales with magnitude. Final column is the compounded year.",
                )
            )
        if "annual" in wanted:
            labels = [str(y) for y in years]
            values = [annual_by_year.get(y, float("nan")) for y in years]
            secs.append(Section("Annual Returns", charts.bar_chart(labels, values, theme=theme, y_pct=True)))

    if "drawdowns" in wanted:
        secs.append(_section_drawdowns(perf, ticker, theme))

    # Header / KPIs
    start_d, end_d = (cum_dates[0], cum_dates[-1]) if cum_dates else (None, None)
    auto_subtitle = (
        f"{fmt.fmt_date(start_d)} - {fmt.fmt_date(end_d)} ({len(cum_dates)} observations)"
        if start_d is not None
        else None
    )
    kpis = []
    if "summary" in wanted:
        kpis = [
            KPI("Total Return", fmt.pct(total_return, signed=True), fmt.sign_class(total_return)),
            KPI("CAGR", fmt.pct(row["cagr"] * 100, signed=True), fmt.sign_class(row["cagr"])),
            KPI("Sharpe", fmt.ratio(row["sharpe"]), ""),
            KPI("Max Drawdown", fmt.pct(row["max_drawdown"] * 100, signed=True), "neg"),
        ]

    return TearSheet(
        theme=theme,
        eyebrow="Performance Review",
        title=title or str(col),
        subtitle=subtitle if subtitle is not None else auto_subtitle,
        meta_lines=["Decimal mode · Bankers rounding"],
        kpis=kpis,
        sections=secs,
        generated=generated,
        footer_left=str(title or col),
    )
