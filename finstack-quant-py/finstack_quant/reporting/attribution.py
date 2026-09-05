"""P&L attribution tear sheet: render a single instrument's T0->T1 PnlAttribution as HTML.

Pure formatter — reads an already-computed ``attribution`` (a
``finstack_quant.attribution.PnlAttribution``) and renders a contribution waterfall
plus factor and carry/credit detail tables. It never computes attribution.

Examples:
--------
>>> from finstack_quant.reporting.attribution import attribution_tearsheet
>>> attribution_tearsheet(None)
Traceback (most recent call last):
...
ValueError: attribution_tearsheet requires a precomputed PnlAttribution
"""

from __future__ import annotations

import json
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet, _resolve_sections
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "attribution_tearsheet",
]

ALL_SECTIONS = ["waterfall", "factors", "carry", "credit"]

# Canonical waterfall-order factor name -> (display label, PnlAttribution attribute).
# Order matches `attribution.default_waterfall_order()`; see the drift guard test.
_WF_FIELD: dict[str, tuple[str, str]] = {
    "carry": ("Carry", "carry"),
    "rates_curves": ("Rates", "rates_curves_pnl"),
    "credit_curves": ("Credit", "credit_curves_pnl"),
    "inflation_curves": ("Inflation", "inflation_curves_pnl"),
    "correlations": ("Correlations", "correlations_pnl"),
    "fx": ("FX", "fx_pnl"),
    "volatility": ("Vol", "vol_pnl"),
    "model_parameters": ("Model Params", "model_params_pnl"),
    "market_scalars": ("Market Scalars", "market_scalars_pnl"),
}
# Factors not in default_waterfall_order() but present on PnlAttribution; appended so
# the bars still sum to total_pnl.
_WF_TAIL: list[tuple[str, str]] = [
    ("FX Translation", "fx_translation_pnl"),
    ("Cross-Factor", "cross_factor_pnl"),
    ("Residual", "residual"),
]
_EPS = 1e-6


def _factor_rows(attribution: Any) -> list[tuple[str, float]]:
    """Ordered (label, signed amount) for each non-zero factor; sum = total_pnl.

    Uses the static canonical order (matching ``default_waterfall_order()``) so the
    pure-format path imports no engine code.
    """
    rows: list[tuple[str, float]] = []
    for label, attr_name in _WF_FIELD.values():
        v = float(getattr(attribution, attr_name, 0.0) or 0.0)
        if abs(v) > _EPS:
            rows.append((label, v))
    for label, attr_name in _WF_TAIL:
        v = float(getattr(attribution, attr_name, 0.0) or 0.0)
        if abs(v) > _EPS:
            rows.append((label, v))
    return rows


def _waterfall_section(attribution: Any, theme: Theme) -> Section | None:
    rows = _factor_rows(attribution)
    if not rows:
        return None
    labels = [lab for lab, _ in rows]
    deltas = [v for _, v in rows]
    return Section(
        "P&L Attribution",
        charts.waterfall_chart(labels, deltas, theme=theme, total_label="Total P&L", height=210),
    )


def _factors_section(attribution: Any) -> Section | None:
    rows = _factor_rows(attribution)
    if not rows:
        return None
    total = float(attribution.total_pnl)
    cur = attribution.currency
    data: list[dict[str, Any]] = []
    for label, v in sorted(rows, key=lambda r: abs(r[1]), reverse=True):
        share = (v / total * 100.0) if abs(total) > _EPS else 0.0
        data.append({"Factor": label, "Amount": v, "% of Total": share})
    data.append({"Factor": "Total P&L", "Amount": total, "% of Total": 100.0 if abs(total) > _EPS else 0.0})
    body = tables.data_table(
        data,
        columns=["Factor", "Amount", "% of Total"],
        formats={
            "Factor": str,
            "Amount": lambda x: fmt.money(x, cur, dp=0),
            "% of Total": lambda x: fmt.pct(x, dp=1, signed=True),
        },
        neg_columns={"Amount", "% of Total"},
    )
    return Section("Factor Contributions", body)


def _detail_section(df: Any, title: str, drop_total_kind: str, currency: str) -> Section | None:
    if df is None or len(df) == 0:
        return None
    data: list[dict[str, Any]] = []
    for _idx, r in df.iterrows():
        kind = str(r["kind"])
        if kind == drop_total_kind:
            continue
        comp = kind.rsplit(".", maxsplit=1)[-1].replace("_", " ").title()
        data.append({"Component": comp, "Amount": float(r["amount"])})
    if not data:
        return None
    body = tables.data_table(
        data,
        columns=["Component", "Amount"],
        formats={"Component": str, "Amount": lambda x: fmt.money(x, currency, dp=0)},
        neg_columns={"Amount"},
    )
    return Section(title, body)


def _build_sections(attribution: Any, wanted: list[str], theme: Theme) -> list[Section]:
    out: list[Section] = []
    if "waterfall" in wanted:
        s = _waterfall_section(attribution, theme)
        if s is not None:
            out.append(s)
    if "factors" in wanted:
        s = _factors_section(attribution)
        if s is not None:
            out.append(s)
    if "carry" in wanted:
        s = _detail_section(
            attribution.to_carry_detail_dataframe(), "Carry & Roll-Down", "carry.total", attribution.currency
        )
        if s is not None:
            out.append(s)
    if "credit" in wanted and abs(float(attribution.credit_curves_pnl)) > _EPS:
        s = _detail_section(
            attribution.to_credit_factor_dataframe(),
            "Credit Factor Detail",
            "credit_factor.total",
            attribution.currency,
        )
        if s is not None:
            out.append(s)
    return out


def attribution_tearsheet(
    attribution: Any,
    *,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: Any = None,
) -> TearSheet:
    """Render a single instrument's T0->T1 P&L attribution as a tear sheet.

    Pass a precomputed ``PnlAttribution`` object or its canonical JSON/dict
    payload. ``sections`` selects a subset of ``ALL_SECTIONS``.

    Parameters
    ----------
    attribution : Any
        Precomputed ``PnlAttribution`` object or equivalent canonical payload.
    title : str or None
        Optional main report heading; ``None`` uses the attributed instrument.
    subtitle : str or None
        Optional secondary heading displayed below ``title``.
    sections : list[str] or None
        Optional subset of supported sections; ``None`` renders the full sheet.
    theme : Theme
        Report palette and typography used for the rendered tear sheet.
    generated : Any
        Optional generated date or timestamp; supply a fixed value for reproducibility.

    Returns:
    -------
    TearSheet
        Attribution report containing reconciliation KPIs and selected factor sections.

    Raises:
    ------
    ValueError
        If no precomputed attribution is supplied, ``sections`` contains an
        unknown name, or an attribution JSON payload cannot be decoded.

    Examples:
    --------
    >>> from finstack_quant.reporting.attribution import attribution_tearsheet
    >>> attribution_tearsheet(None)
    Traceback (most recent call last):
    ...
    ValueError: attribution_tearsheet requires a precomputed PnlAttribution
    """
    if attribution is None:
        raise ValueError("attribution_tearsheet requires a precomputed PnlAttribution")
    if isinstance(attribution, (str, dict)):
        from finstack_quant.attribution import PnlAttribution

        payload = attribution if isinstance(attribution, str) else json.dumps(attribution)
        attribution = PnlAttribution.from_json(payload)

    wanted = _resolve_sections(sections, ALL_SECTIONS, valid_label="valid")
    cur = attribution.currency
    kpis = [
        KPI("Total P&L", fmt.money(attribution.total_pnl, cur, dp=0), fmt.sign_class(attribution.total_pnl)),
        KPI(
            "Mark-to-Market",
            fmt.money(attribution.mark_to_market_pnl, cur, dp=0),
            fmt.sign_class(attribution.mark_to_market_pnl),
        ),
        KPI("Carry", fmt.money(attribution.carry, cur, dp=0), fmt.sign_class(attribution.carry)),
        KPI("Residual", fmt.money(attribution.residual, cur, dp=2), ""),
        KPI("Repricings", str(attribution.num_repricings), ""),
    ]
    meta_lines = [
        f"Instrument {attribution.instrument_id}",
        f"Method {str(attribution.method).replace('_', ' ').title()}",
        f"{attribution.t0} → {attribution.t1}",
        f"Currency {cur}",
    ]
    if attribution.result_invalid:
        meta_lines.append("⚠ Attribution calculation flagged invalid")
    elif not attribution.residual_within_tolerance():
        meta_lines.append("⚠ Attribution residual outside tolerance")
    if attribution.notes:
        meta_lines.append("⚠ " + "; ".join(str(n) for n in attribution.notes))

    return TearSheet(
        theme=theme,
        title=title or "P&L Attribution",
        eyebrow="Instrument Attribution",
        subtitle=subtitle or f"{attribution.instrument_id} · {attribution.t0} → {attribution.t1}",
        meta_lines=meta_lines,
        kpis=kpis,
        sections=_build_sections(attribution, wanted, theme),
        generated=generated,
    )
