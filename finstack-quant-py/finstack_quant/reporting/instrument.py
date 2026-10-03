"""Instrument tear sheet: render a priced valuations.ValuationResult as HTML.

Pure formatter — reads the already-priced ``result`` (scalar metrics incl.
``bucketed_*::curve::tenor`` composite keys, covenants), parses ``result.to_json()``
read-only for meta/details, and renders the optional ``cashflows`` DataFrame and
``definition`` JSON. It never prices.

Examples:
--------
>>> from finstack_quant.reporting.instrument import instrument_tearsheet
>>> instrument_tearsheet({"schema": "finstack_quant.instrument/1"})
Traceback (most recent call last):
...
TypeError: instrument_tearsheet requires a precomputed ValuationResult
"""

from __future__ import annotations

import datetime as dt
import html
import json
import math
from typing import Any

from finstack_quant.cashflows.aggregation import calendar_year_ladder
from finstack_quant.core.dates import Tenor
from finstack_quant.models import vanilla_expiry_payoff

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet, _resolve_sections
from .theme import INSTITUTIONAL, Theme

__all__ = [
    "ALL_SECTIONS",
    "instrument_tearsheet",
]


def _parse_result(result: Any) -> dict[str, Any]:
    """Read-only parse of ``result.to_json()`` for provenance/details/covenants."""
    doc = json.loads(result.to_json())
    meta = doc.get("meta") or {}
    return {
        "as_of": doc.get("as_of"),
        "numeric_mode": meta.get("numeric_mode"),
        "fx_policy": meta.get("fx_policy_applied"),
        "version": meta.get("version"),
        "details": doc.get("details"),
        "covenants": doc.get("covenants"),
    }


def _bucketed_series(result: Any, prefix: str) -> tuple[list[str], list[float], str]:
    """Buckets of a composite metric exactly as Rust reports them.

    Reads ``result.metric_series(prefix)``: Rust decodes the composite keys, so
    nothing is re-parsed, summed or filtered here. When every bucket ends in a
    tenor (key-rate DV01/CS01), bars are ordered by the Rust ``Tenor`` year
    fraction, as the portfolio tear sheet does; otherwise (an expiry-by-strike
    vega grid) the Rust order is kept. When every bucket shares its first
    coordinate (one curve or surface), that coordinate becomes the caption and
    bars are labelled by the remaining coordinates.
    """
    series = [(list(components), float(value)) for components, value in result.metric_series(prefix) if components]
    if not series:
        return [], [], ""
    years = [_tenor_years(components[-1]) for components, _ in series]
    if all(y is not None for y in years):
        series = [item for _, item in sorted(zip(years, series, strict=True), key=lambda pair: pair[0])]
    firsts = {components[0] for components, _ in series}
    if len(firsts) == 1 and all(len(components) > 1 for components, _ in series):
        caption = next(iter(firsts))
        labels = [" · ".join(components[1:]) for components, _ in series]
    else:
        caption = ""
        labels = [" · ".join(components) for components, _ in series]
    return labels, [value for _, value in series], caption


def _tenor_years(label: str) -> float | None:
    """Year fraction of a tenor label via the Rust ``Tenor`` parser, or ``None``."""
    try:
        return Tenor.parse(label).to_years()
    except ValueError:
        return None


def _money_str(m: Any) -> str:
    """Format a Money dict ``{"amount": "<string>", "currency": ..}`` or a number."""
    if isinstance(m, dict):
        amt = m.get("amount")
        try:
            return fmt.money(float(amt), m.get("currency"), dp=0)
        except (TypeError, ValueError):
            return str(amt)
    return fmt.money(m, dp=0) if isinstance(m, (int, float)) else str(m)


# metric_id -> display label. Only the label text lives here: each value is
# formatted from the unit family Rust reports for it
# (``ValuationResult.metric_units()``); ids without a label are humanized.
_METRIC_LABELS: dict[str, str] = {
    "dirty_price": "Dirty Price",
    "clean_price": "Clean Price",
    "accrued": "Accrued",
    "ytm": "Yield to Maturity",
    "ytw": "Yield to Worst",
    "z_spread": "Z-Spread",
    "oas": "OAS",
    "i_spread": "I-Spread",
    "asw_par": "ASW (par)",
    "discount_margin": "Discount Margin",
    "duration_mod": "Mod. Duration",
    "duration_mac": "Mac. Duration",
    "convexity": "Convexity",
    "spread_duration": "Spread Duration",
    "dv01": "DV01",
    "pv01": "PV01",
    "par_rate": "Par Rate",
    "annuity": "Annuity",
    "pv_fixed": "Fixed Leg PV",
    "pv_float": "Float Leg PV",
    "par_spread": "Par Spread",
    "risky_pv01": "Risky PV01",
    "risky_annuity": "Risky Annuity",
    "protection_leg_pv": "Protection Leg PV",
    "premium_leg_pv": "Premium Leg PV",
    "cs01": "CS01",
    "jump_to_default": "Jump-to-Default",
    "expected_loss": "Expected Loss",
    "default01": "Default01",
    "recovery01": "Recovery01",
    "delta": "Delta",
    "gamma": "Gamma",
    "vega": "Vega",
    "theta": "Theta",
    "rho": "Rho",
    "implied_vol": "Implied Vol",
    "vanna": "Vanna",
    "volga": "Volga",
    "charm": "Charm",
}


# Desk display convention: spreads that Rust reports as ``decimal`` are shown
# in basis points. This only rescales a value whose unit Rust already
# classified as decimal; any other unit is formatted by its own family.
_DECIMAL_AS_BP = frozenset({"z_spread", "oas", "i_spread", "asw_par", "asw_market", "discount_margin"})


def _humanize(metric_id: str) -> str:
    return metric_id.replace("_", " ").title()


def _fmt_value(metric_id: str, unit: str, v: float) -> str:
    """Format one value by its Rust ``MetricUnit`` wire name."""
    if unit == "currency":
        return fmt.money(v, dp=0)
    if unit == "decimal" and metric_id in _DECIMAL_AS_BP:
        return f"{v * 10000.0:,.0f} bp"
    if unit == "decimal":
        return fmt.pct(v * 100.0, dp=2)
    if unit == "percent":
        return fmt.pct(v, dp=2)
    if unit == "basis_points":
        return f"{v:,.0f} bp"
    # years, dimensionless and unknown (custom) values are plain numbers.
    return fmt.ratio(v, dp=2)


def _metric_cell(metric_id: str, v: float | None, units: dict[str, str]) -> tuple[str, str, str]:
    """Return (label, formatted_value, css_class) for one metric.

    ``units`` is ``result.metric_units()``: the Rust unit family per key.
    Currency amounts carry the P&L sign class.
    """
    label = _METRIC_LABELS.get(metric_id, _humanize(metric_id))
    if v is None:
        return (label, "·", "")
    unit = units.get(metric_id, "unknown")
    return (label, _fmt_value(metric_id, unit, float(v)), fmt.sign_class(v) if unit == "currency" else "")


def _frequency_str(frequency: dict[str, Any]) -> str:
    if not frequency:
        return ""
    count, unit = frequency.get("count"), frequency.get("unit", "")
    mapping = {
        ("6", "months"): "Semi-annual",
        ("3", "months"): "Quarterly",
        ("12", "months"): "Annual",
        ("1", "months"): "Monthly",
    }
    return mapping.get((str(count), unit), f"{count} {unit}")


# Per-type definition extraction: (column label not shown; just grouped kv rows).
def _definition_terms(definition: dict[str, Any]) -> list[list[tuple[str, str]]]:
    """Return up to three columns of (label, value) rows describing the instrument."""
    spec = definition.get("spec", {})
    itype = definition.get("type", "")
    if itype == "bond":
        cf = spec.get("cashflow_spec") or {}
        fixed = cf.get("fixed") or {}
        floating = (cf.get("floating") or {}).get("rate") or {}
        if fixed:
            coupon = f"{float(fixed.get('rate', 0)) * 100:.3f}% Fixed"
            frequency = fixed.get("frequency", {})
            day_count = fixed.get("day_count", "")
        else:
            coupon = f"{floating.get('forward_curve_id', 'float')} + {floating.get('spread_bp', 0)}bp"
            frequency = floating.get("reset_frequency", {})
            day_count = floating.get("day_count", "")
        return [
            [("Notional", _money_str(spec.get("notional"))), ("Coupon", coupon), ("Issuer", spec.get("id", ""))],
            [
                ("Issue Date", spec.get("issue_date", "")),
                ("Maturity", spec.get("maturity", "")),
                ("Frequency", _frequency_str(frequency)),
                ("Day Count", day_count),
            ],
            [
                ("Discount Curve", spec.get("discount_curve_id", "")),
                ("Credit Curve", spec.get("credit_curve_id") or "—"),
                ("Callable", "Yes" if spec.get("call_put") else "No"),
            ],
        ]
    if itype == "credit_default_swap":
        prem, prot = spec.get("premium_leg", {}), spec.get("protection_leg", {})
        return [
            [
                ("Reference", spec.get("id", "")),
                ("Notional", _money_str(spec.get("notional"))),
                ("Side", str(spec.get("side", ""))),
            ],
            [
                ("Running Coupon", f"{prem.get('coupon_bp', 0)} bp"),
                ("Effective", prem.get("start", "")),
                ("Maturity", prem.get("end", "")),
                ("Frequency", _frequency_str(prem.get("frequency", {}))),
            ],
            [
                ("Recovery", f"{float(prot.get('recovery_rate', 0)) * 100:.0f}%"),
                ("Credit Curve", prot.get("credit_curve_id", "")),
                ("Doc Clause", str(spec.get("doc_clause") or "—")),
            ],
        ]
    if itype == "equity_option":
        return [
            [
                ("Underlying", spec.get("underlying_ticker", "")),
                ("Option Type", str(spec.get("option_type", ""))),
                ("Exercise", str(spec.get("exercise_style", ""))),
            ],
            [
                ("Strike", f"{float(spec.get('strike', 0)):,.2f}"),
                ("Expiry", spec.get("expiry", "")),
                ("Settlement", str(spec.get("settlement", ""))),
            ],
            [
                ("Vol Surface", spec.get("vol_surface_id", "")),
                ("Discount Curve", spec.get("discount_curve_id", "")),
                ("Quantity", f"{float(spec.get('quantity', 0)):,.0f} {spec.get('currency', '')}"),
            ],
        ]
    # generic: flatten scalar spec fields into up to three columns
    rows: list[tuple[str, str]] = []
    for k, v in spec.items():
        if isinstance(v, dict) and "amount" in v:
            rows.append((_humanize(k), _money_str(v)))
        elif isinstance(v, (str, int, float)) and not isinstance(v, bool):
            rows.append((_humanize(k), str(v)))
    cols: list[list[tuple[str, str]]] = [[], [], []]
    for i, kv in enumerate(rows[:18]):
        cols[i % 3].append(kv)
    return cols


def _is_nan(x: Any) -> bool:
    return isinstance(x, float) and math.isnan(x)


def _cashflow_blocks(
    cashflows: Any,
) -> tuple[dict[str, list[tuple[str, float, float, float]]], list[dict[str, Any]]]:
    """Render currency-partitioned Rust annual totals and every labelled cashflow."""
    df = cashflows[1] if isinstance(cashflows, tuple) else cashflows
    schedule: list[dict[str, Any]] = []
    grouped: dict[str, tuple[list[Any], list[str], list[float], list[float]]] = {}
    for _, r in df.iterrows():
        currency = r.get("currency")
        if not isinstance(currency, str) or not currency:
            raise ValueError("cashflow rows require an explicit currency")
        dates, kinds, amounts, pvs = grouped.setdefault(currency, ([], [], [], []))
        d = r["date"]
        kind = str(r.get("kind", ""))
        amt = float(r.get("amount") or 0.0)
        pv = float(r.get("pv") or 0.0)
        dates.append(d)
        kinds.append(kind)
        amounts.append(amt)
        pvs.append(pv)
        rate = r.get("rate")
        schedule.append({
            "Date": fmt.fmt_date(d),
            "Currency": currency,
            "Kind": kind,
            "Amount": fmt.money(amt, currency, dp=0),
            "Rate": fmt.pct(float(rate) * 100, dp=3) if rate is not None and not _is_nan(rate) else "—",
            "DF": fmt.ratio(float(r["discount_factor"]), dp=4) if "discount_factor" in r else "—",
            "PV": fmt.money(pv, currency, dp=0),
        })
    ladders = {
        currency: [
            (str(year), coupon / 1e6, principal / 1e6, pv / 1e6)
            for year, coupon, principal, pv in calendar_year_ladder(*columns)
        ]
        for currency, columns in grouped.items()
    }
    return ladders, schedule


# Task 6: Assembly — instrument_tearsheet public API

ALL_SECTIONS = ["definition", "valuation", "keyrate", "cashflows", "schedule", "payoff", "survival", "covenants"]
_MISSING_DEFINITION = object()

# Headline KPI metric ids per type (label comes from _metric_cell).
# Reconciliation: "default_probability" replaced with "default01" for CDS.
_KPI_METRICS: dict[str, list[str]] = {
    "bond": ["dirty_price", "ytm", "duration_mod", "dv01"],
    "credit_default_swap": ["par_spread", "cs01", "jump_to_default", "default01"],
    "equity_option": ["delta", "vega", "gamma", "theta"],
}

# Analytics column groupings per type: list of groups of metric_ids.
# Reconciliation: "default_probability" replaced with "default01" for CDS;
#                 "g_spread" removed from bond (not a real metric).
_ANALYTICS_GROUPS: dict[str, list[list[str]]] = {
    "bond": [
        ["clean_price", "dirty_price", "accrued"],
        ["ytm", "ytw", "z_spread", "oas", "i_spread", "asw_par"],
        ["dv01", "duration_mod", "duration_mac", "convexity", "spread_duration"],
    ],
    "credit_default_swap": [
        ["protection_leg_pv", "premium_leg_pv", "risky_annuity"],
        ["par_spread", "risky_pv01", "default01"],
        ["cs01", "expected_loss", "jump_to_default", "recovery01"],
    ],
    "equity_option": [
        ["implied_vol"],
        ["delta", "gamma", "vega", "theta"],
        ["rho", "vanna", "volga", "charm"],
    ],
}


def _parse_definition_once(definition: Any) -> tuple[Any, json.JSONDecodeError | None]:
    if definition is None:
        return _MISSING_DEFINITION, None
    if not isinstance(definition, str):
        return definition, None
    try:
        return json.loads(definition), None
    except json.JSONDecodeError as error:
        return definition, error


def _kpis(result: Any, itype: str) -> list[KPI]:
    units = result.metric_units()
    ids = _KPI_METRICS.get(itype)
    if not ids:
        # generic: PV + first three present non-composite metrics
        cells: list[tuple[str, str, str]] = [("PV", fmt.money(result.price, result.currency, dp=0), "")]
        count = 0
        for mid in result.metric_keys():
            if "::" in mid:
                continue
            cells.append(_metric_cell(mid, result.get_metric(mid), units))
            count += 1
            if count == 3:
                break
        return [KPI(lbl, val, cls) for lbl, val, cls in cells]
    out = []
    for mid in ids:
        lbl, val, cls = _metric_cell(mid, result.get_metric(mid), units)
        out.append(KPI(lbl, val, cls))
    return out


def _analytics_section(result: Any, itype: str) -> Section:
    units = result.metric_units()
    groups = _ANALYTICS_GROUPS.get(itype)
    if not groups:
        # generic: one column of every present non-composite metric
        present = [(m, result.get_metric(m)) for m in result.metric_keys() if "::" not in m]
        rows = [_metric_cell(m, v, units) for m, v in present]
        cols_html = "".join(tables.kv_table([(lbl, val, cls) for lbl, val, cls in rows[i::3]]) for i in range(3))
        return Section("Valuation & Analytics", f'<div class="statgrid">{cols_html}</div>')
    cols_html = ""
    for group in groups:
        rows = [_metric_cell(m, result.get_metric(m), units) for m in group if result.get_metric(m) is not None]
        cols_html += tables.kv_table([(lbl, val, cls) for lbl, val, cls in rows])
    return Section("Valuation & Analytics", f'<div class="statgrid">{cols_html}</div>')


def _definition_section(definition: Any, definition_error: json.JSONDecodeError | None) -> Section | None:
    if definition is _MISSING_DEFINITION:
        return None
    if definition_error is not None:
        raise definition_error
    cols = _definition_terms(definition)
    cols_html = "".join(tables.kv_table([(k, v, "") for k, v in col]) for col in cols if col)
    return Section("Definition", f'<div class="statgrid">{cols_html}</div>')


def _keyrate_section(result: Any, itype: str, theme: Theme) -> Section | None:
    if itype == "credit_default_swap":
        prefix = "bucketed_cs01"
    elif itype == "equity_option":
        prefix = "bucketed_vega"
    else:
        prefix = "bucketed_dv01"
    labels, vals, caption = _bucketed_series(result, prefix)
    if not labels:
        return None
    title_map = {"bucketed_cs01": "Bucketed CS01", "bucketed_vega": "Bucketed Vega"}
    title = title_map.get(prefix, "Key-Rate (Bucketed) DV01")
    chart = charts.bar_chart(labels, vals, theme=theme, y_pct=False, height=175)
    sub = f'<p class="sub">{html.escape(caption)}</p>' if caption else ""
    return Section(title, sub + chart)


def _cashflow_sections(cashflows: Any, theme: Theme) -> list[Section]:
    if cashflows is None:
        return []
    ladders, schedule = _cashflow_blocks(cashflows)
    out: list[Section] = []
    for currency, ladder in ladders.items():
        periods = [p for p, _, _, _ in ladder]
        coupon = [c for _, c, _, _ in ladder]
        principal = [pr for _, _, pr, _ in ladder]
        pv = [p for _, _, _, p in ladder]
        out.append(
            Section(
                f"Cashflow Ladder · {currency}",
                charts.cashflow_ladder(periods, coupon, principal, theme=theme, pv=pv),
                subtitle=f"Amounts in {currency} millions.",
            )
        )
    if schedule:
        cols = ["Date", "Currency", "Kind", "Amount", "Rate", "DF", "PV"]
        out.append(Section("Cashflow Schedule", tables.scroll(tables.data_table(schedule, columns=cols))))
    return out


def _payoff_section(
    definition: Any,
    definition_error: json.JSONDecodeError | None,
    _result: Any,
    theme: Theme,
) -> Section | None:
    if definition is _MISSING_DEFINITION:
        return None
    if definition_error is not None:
        raise definition_error
    if definition.get("type") != "equity_option":
        return None
    spec = definition.get("spec", {})
    strike = float(spec.get("strike", 0) or 0)
    if strike <= 0:
        return None
    is_call = str(spec.get("option_type", "Call")).lower().startswith("c")
    spots = [strike * (0.6 + 0.04 * i) for i in range(21)]  # 0.6K .. 1.4K
    try:
        payoff = [vanilla_expiry_payoff(s, strike, is_call) for s in spots]
    except ValueError:
        return None
    return Section(
        "Payoff at Expiry",
        charts.line_chart(
            spots,
            payoff,
            theme=theme,
            x_numeric=True,
            zero=True,
            color=theme.ink,
            fill=charts.rgba(theme.accent, 0.12),
            area=True,
        ),
    )


def _survival_section(_result: Any, cashflows: Any, theme: Theme) -> Section | None:
    if cashflows is None:
        return None
    df = cashflows[1] if isinstance(cashflows, tuple) else cashflows
    if "survival_probability" not in getattr(df, "columns", []):
        return None
    sp = [float(v) for v in df["survival_probability"].tolist() if v is not None and not _is_nan(v)]
    if not sp:
        return None
    dates = list(df["date"])[: len(sp)]
    return Section(
        "Survival Probability",
        charts.line_chart(
            dates,
            [v * 100 for v in sp],
            theme=theme,
            y_pct=True,
            ymin=min(sp) * 100 - 2,
            ymax=100,
            color=theme.ink,
            area=True,
        ),
    )


def _covenants_section(parsed: dict[str, Any]) -> Section | None:
    cov = parsed.get("covenants")
    if not cov:
        return None
    rows = []
    for cid, c in cov.items():
        rows.append({
            "Covenant": cid,
            "Type": c.get("covenant_type", ""),
            "Actual": fmt.ratio(c.get("actual_value"), dp=2) if c.get("actual_value") is not None else "·",
            "Threshold": fmt.ratio(c.get("threshold"), dp=2) if c.get("threshold") is not None else "·",
            "Headroom": fmt.ratio(c.get("headroom"), dp=2) if c.get("headroom") is not None else "·",
            "Status": "PASS" if c.get("passed") else "BREACH",
        })
    cols = ["Covenant", "Type", "Actual", "Threshold", "Headroom", "Status"]
    return Section("Covenants", tables.data_table(rows, columns=cols))


def _build_sections(
    result: Any,
    cashflows: Any,
    definition: Any,
    definition_error: json.JSONDecodeError | None,
    parsed: dict[str, Any],
    itype: str,
    wanted: list[str],
    theme: Theme,
) -> list[Section]:
    """Dispatch section builders and collect the ordered list of Section objects."""
    secs: list[Section] = []
    if "definition" in wanted and (s := _definition_section(definition, definition_error)):
        secs.append(s)
    if "valuation" in wanted:
        secs.append(_analytics_section(result, itype))
    if "survival" in wanted and (s := _survival_section(result, cashflows, theme)):
        secs.append(s)
    if "keyrate" in wanted and (s := _keyrate_section(result, itype, theme)):
        secs.append(s)
    if "payoff" in wanted and (s := _payoff_section(definition, definition_error, result, theme)):
        secs.append(s)
    if "cashflows" in wanted or "schedule" in wanted:
        secs.extend(
            s
            for s in _cashflow_sections(cashflows, theme)
            if (s.title.startswith("Cashflow Ladder") and "cashflows" in wanted)
            or (s.title == "Cashflow Schedule" and "schedule" in wanted)
        )
    if "covenants" in wanted and (s := _covenants_section(parsed)):
        secs.append(s)
    return secs


def instrument_tearsheet(
    result: Any,
    *,
    cashflows: Any = None,
    definition: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Render an instrument tear sheet.

    ``result`` must be an already-priced ``valuations.ValuationResult``. Pricing,
    metric selection, and cashflow generation belong to the valuations layer;
    this function only renders their outputs.

    Parameters
    ----------
    result : Any
        Precomputed valuation result to render.
    cashflows : Any
        Optional precomputed cashflow payload with explicit currency on each row; each currency is rendered separately.
    definition : Any
        Optional instrument-definition payload used to enrich the rendered sheet.
    title : str or None
        Optional main report heading; ``None`` uses the instrument identifier.
    subtitle : str or None
        Optional secondary heading; ``None`` derives type, currency, and date.
    sections : list[str] or None
        Optional subset of supported sections; ``None`` renders the full sheet.
    theme : Theme
        Report palette and typography used for the rendered tear sheet.
    generated : datetime.date or None
        Optional generated date; supply a fixed date for reproducible output.

    Returns:
    -------
    TearSheet
        Instrument report with headline value, requested sections, and available metrics.

    Raises:
    ------
    ValueError
        If ``sections`` contains an unknown name or a cashflow row lacks an explicit currency.
    TypeError
        If ``result`` is an instrument JSON string or mapping instead of a
        precomputed valuation result.
    json.JSONDecodeError
        If an instrument, valuation result, or requested definition contains
        malformed JSON.
    AttributeError
        If a requested definition or payoff section receives a definition that
        does not decode to a JSON object.

    Examples:
    --------
    >>> import json
    >>> from finstack_quant.reporting.instrument import instrument_tearsheet
    >>> from finstack_quant.valuations import ValuationResult
    >>> rounding = {
    ...     "mode": "bankers",
    ...     "ingest_scale_by_currency": {},
    ...     "output_scale_by_currency": {},
    ...     "tolerances": {"rate_epsilon": 1e-12, "generic_epsilon": 1e-10},
    ...     "version": 1,
    ... }
    >>> payload = {
    ...     "schema_version": 1,
    ...     "instrument_id": "TEST",
    ...     "as_of": "2026-06-19",
    ...     "value": {"amount": "100", "currency": "USD"},
    ...     "measures": {},
    ...     "meta": {
    ...         "numeric_mode": "f64",
    ...         "rounding": rounding,
    ...         "fx_policy_applied": None,
    ...         "timestamp": "2026-06-20T12:19:44Z",
    ...         "version": "0.9.0",
    ...     },
    ...     "covenants": None,
    ... }
    >>> result = ValuationResult.from_json(json.dumps(payload))
    >>> instrument_tearsheet(result, sections=[]).title
    'TEST'
    """
    if isinstance(result, (str, dict)):
        raise TypeError("instrument_tearsheet requires a precomputed ValuationResult")
    wanted = _resolve_sections(sections, ALL_SECTIONS, valid_label="valid")

    parsed = _parse_result(result)
    definition, definition_error = _parse_definition_once(definition)
    itype = definition.get("type", "") if isinstance(definition, dict) else ""

    secs = _build_sections(result, cashflows, definition, definition_error, parsed, itype, wanted, theme)

    as_of = parsed.get("as_of") or ""
    type_label = itype.replace("_", " ").title() if itype else "Instrument"
    meta_lines = [f"Numeric: {parsed.get('numeric_mode') or '—'}"]
    if parsed.get("fx_policy"):
        meta_lines.append(f"FX: {parsed['fx_policy']}")
    return TearSheet(
        theme=theme,
        eyebrow="Instrument Valuation",
        title=title or result.instrument_id,
        subtitle=subtitle if subtitle is not None else f"{type_label} · {result.currency} · As of {as_of}",
        meta_lines=meta_lines,
        kpis=_kpis(result, itype),
        sections=secs,
        generated=generated,
        footer_left=result.instrument_id,
    )
