# finstack-quant-py/finstack_quant/reporting/instrument.py
"""Instrument tear sheet: render a priced valuations.ValuationResult as HTML.

Pure formatter — reads the already-priced ``result`` (scalar metrics incl.
``bucketed_*::curve::tenor`` composite keys, covenants), parses ``result.to_json()``
read-only for meta/details, and renders the optional ``cashflows`` DataFrame and
``definition`` JSON. It never prices; ``recommended_metrics`` is a static mapping.
"""

from __future__ import annotations

import datetime as dt
import json
import math
import re
from typing import Any

from . import charts, format as fmt, tables
from .document import KPI, Section, TearSheet
from .theme import INSTITUTIONAL, Theme

_TENOR_ORDER = ["3m", "6m", "1y", "2y", "3y", "5y", "7y", "10y", "15y", "20y", "30y"]

# What to request from price_instrument_with_metrics for a full sheet, by type.
_RECOMMENDED: dict[str, list[str]] = {
    "bond": [
        "dirty_price",
        "clean_price",
        "accrued",
        "ytm",
        "ytw",
        "z_spread",
        "oas",
        "i_spread",
        "duration_mod",
        "duration_mac",
        "convexity",
        "dv01",
        "bucketed_dv01",
        "spread_duration",
    ],
    "interest_rate_swap": [
        "pv_fixed",
        "pv_float",
        "par_rate",
        "annuity",
        "dv01",
        "bucketed_dv01",
        "pv01",
    ],
    "credit_default_swap": [
        "par_spread",
        "risky_pv01",
        "risky_annuity",
        "protection_leg_pv",
        "premium_leg_pv",
        "cs01",
        "bucketed_cs01",
        "jump_to_default",
        "expected_loss",
        "default01",
        "recovery_01",
    ],
    "equity_option": [
        "delta",
        "gamma",
        "vega",
        "theta",
        "rho",
        "implied_vol",
        "vanna",
        "volga",
        "charm",
        "bucketed_vega",
    ],
}


_NEEDS_QUOTE: frozenset[str] = frozenset({"oas", "ytw"})  # metrics that require a quoted market price to solve


def recommended_metrics(instrument_type: str) -> list[str]:
    """Metric IDs to pass to ``price_instrument_with_metrics`` for a full sheet of ``instrument_type``.

    Returns ``[]`` for unrecognised types (the sheet then renders whatever metrics are present).
    """
    return list(_RECOMMENDED.get(instrument_type, []))


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


def _bucketed_series(result: Any, prefix: str) -> list[tuple[str, float]]:
    """Per-tenor sum of a bucketed metric, ordered by the standard tenor grid.

    Parses ``{prefix}::{curve}::{tenor}`` composite keys from ``result.metric_keys()``.
    """
    pat = re.compile(rf"^{re.escape(prefix)}::.+?::(\w+)$")
    by_tenor: dict[str, float] = {}
    for key in result.metric_keys():
        m = pat.match(key)
        if not m:
            continue
        tenor = m.group(1)
        val = result.get_metric(key)
        if val is None:
            continue
        by_tenor[tenor] = by_tenor.get(tenor, 0.0) + float(val)
    return [(t, by_tenor[t]) for t in _TENOR_ORDER if t in by_tenor]


def _money_str(m: Any) -> str:
    """Format a Money dict ``{"amount": "<string>", "currency": ..}`` or a number."""
    if isinstance(m, dict):
        amt = m.get("amount")
        try:
            return fmt.money(float(amt), m.get("currency"), dp=0)
        except (TypeError, ValueError):
            return str(amt)
    return fmt.money(m, dp=0) if isinstance(m, (int, float)) else str(m)


# metric_id -> (display label, unit kind). Unit kinds:
#   "pct"   decimal -> N.NN%      "bp"   decimal -> NN bp
#   "money" full-value 0dp        "ratio" 2dp
#   "ratio4" 4dp
# Spot-check (2026-06-19): dirty/clean/accrued are full dollar values (not per-100),
# so "money" kind is used (0dp). Yields/vols are decimal (×100 for %). Spreads decimal (×10000 for bp).
_METRIC_FMT: dict[str, tuple[str, str]] = {
    "dirty_price": ("Dirty Price", "money"),
    "clean_price": ("Clean Price", "money"),
    "accrued": ("Accrued", "money"),
    "ytm": ("Yield to Maturity", "pct"),
    "ytw": ("Yield to Worst", "pct"),
    "z_spread": ("Z-Spread", "bp"),
    "oas": ("OAS", "bp"),
    "i_spread": ("I-Spread", "bp"),
    "asw_par": ("ASW (par)", "bp"),
    "g_spread": ("G-Spread", "bp"),
    "discount_margin": ("Discount Margin", "bp"),
    "duration_mod": ("Mod. Duration", "ratio"),
    "duration_mac": ("Mac. Duration", "ratio"),
    "convexity": ("Convexity", "ratio"),
    "spread_duration": ("Spread Duration", "ratio"),
    "dv01": ("DV01", "money"),
    "pv01": ("PV01", "money"),
    "par_rate": ("Par Rate", "pct"),
    "annuity": ("Annuity", "money"),
    "pv_fixed": ("Fixed Leg PV", "money"),
    "pv_float": ("Float Leg PV", "money"),
    "par_spread": ("Par Spread", "bp"),
    "risky_pv01": ("Risky PV01", "ratio"),
    "risky_annuity": ("Risky Annuity", "money"),
    "protection_leg_pv": ("Protection Leg PV", "money"),
    "premium_leg_pv": ("Premium Leg PV", "money"),
    "cs01": ("CS01", "money"),
    "jump_to_default": ("Jump-to-Default", "money"),
    "expected_loss": ("Expected Loss", "money"),
    "default_probability": ("Default Probability", "pct"),
    "default01": ("Default01", "money"),
    "recovery_01": ("Recovery01", "money"),
    "delta": ("Delta", "ratio4"),
    "gamma": ("Gamma", "ratio4"),
    "vega": ("Vega", "ratio"),
    "theta": ("Theta", "ratio"),
    "rho": ("Rho", "ratio"),
    "implied_vol": ("Implied Vol", "pct"),
    "vanna": ("Vanna", "ratio"),
    "volga": ("Volga", "ratio"),
    "charm": ("Charm", "ratio4"),
}


def _humanize(metric_id: str) -> str:
    return metric_id.replace("_", " ").title()


def _fmt_value(kind: str, v: float) -> str:
    if kind == "pct":
        return fmt.pct(v * 100.0, dp=2)
    if kind == "bp":
        return f"{v * 10000.0:,.0f} bp"
    if kind == "price":
        return f"{v:,.2f}"
    if kind == "money":
        return fmt.money(v, dp=0)
    if kind == "ratio4":
        return fmt.ratio(v, dp=4)
    return fmt.ratio(v, dp=2)


def _metric_cell(metric_id: str, v: float | None) -> tuple[str, str, str]:
    """Return (label, formatted_value, css_class) for one metric."""
    label, kind = _METRIC_FMT.get(metric_id, (_humanize(metric_id), "ratio"))
    if v is None:
        return (label, "·", "")
    return (label, _fmt_value(kind, float(v)), fmt.sign_class(v) if kind == "money" else "")


def _freq_str(freq: dict[str, Any]) -> str:
    if not freq:
        return ""
    count, unit = freq.get("count"), freq.get("unit", "")
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
        fixed = cf.get("Fixed") or {}
        floating = (cf.get("Floating") or {}).get("rate") or {}
        if fixed:
            coupon = f"{fixed.get('rate', 0) * 100:.3f}% Fixed"
            freq = fixed.get("freq", {})
            dc = fixed.get("dc", "")
        else:
            coupon = f"{floating.get('index_id', 'float')} + {floating.get('spread_bp', 0)}bp"
            freq = floating.get("reset_freq", {})
            dc = floating.get("dc", "")
        return [
            [("Notional", _money_str(spec.get("notional"))), ("Coupon", coupon), ("Issuer", spec.get("id", ""))],
            [
                ("Issue Date", spec.get("issue_date", "")),
                ("Maturity", spec.get("maturity", "")),
                ("Frequency", _freq_str(freq)),
                ("Day Count", dc),
            ],
            [
                ("Discount Curve", spec.get("discount_curve_id", "")),
                ("Credit Curve", spec.get("credit_curve_id") or "—"),
                ("Callable", "Yes" if spec.get("call_put") else "No"),
            ],
        ]
    if itype == "credit_default_swap":
        prem, prot = spec.get("premium", {}), spec.get("protection", {})
        return [
            [
                ("Reference", spec.get("id", "")),
                ("Notional", _money_str(spec.get("notional"))),
                ("Side", str(spec.get("side", ""))),
            ],
            [
                ("Running Spread", f"{prem.get('spread_bp', 0)} bp"),
                ("Effective", prem.get("start", "")),
                ("Maturity", prem.get("end", "")),
                ("Frequency", _freq_str(prem.get("frequency", {}))),
            ],
            [
                ("Recovery", f"{prot.get('recovery_rate', 0) * 100:.0f}%"),
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
                ("Strike", f"{spec.get('strike', 0):,.2f}"),
                ("Expiry", spec.get("expiry", "")),
                ("Settlement", str(spec.get("settlement", ""))),
            ],
            [
                ("Vol Surface", spec.get("vol_surface_id", "")),
                ("Discount Curve", spec.get("discount_curve_id", "")),
                ("Notional", _money_str(spec.get("notional"))),
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


_PRINCIPAL_KINDS = {"principal", "notional"}


def _is_nan(x: Any) -> bool:
    return isinstance(x, float) and math.isnan(x)


def _cashflow_blocks(
    cashflows: Any,
) -> tuple[list[tuple[str, float, float, float]], list[dict[str, Any]]]:
    """Shape a cashflow DataFrame (or ``(envelope, df)``) into ladder rows and schedule rows.

    Ladder rows: ``(period_label, coupon_sum, principal_sum, pv_sum)`` grouped by calendar
    year (values scaled to millions). Schedule rows: per-flow dicts for the scroll table.
    """
    df = cashflows[1] if isinstance(cashflows, tuple) else cashflows
    by_year: dict[int, list[float]] = {}
    schedule: list[dict[str, Any]] = []
    for _, r in df.iterrows():
        d = r["date"]
        year = d.year if hasattr(d, "year") else int(str(d)[:4])
        kind = str(r.get("kind", ""))
        amt = float(r.get("amount") or 0.0)
        pv = float(r.get("pv") or 0.0)
        slot = by_year.setdefault(year, [0.0, 0.0, 0.0])  # coupon, principal, pv
        if kind in _PRINCIPAL_KINDS:
            slot[1] += amt
        else:
            slot[0] += amt
        slot[2] += pv
        rate = r.get("rate")
        schedule.append({
            "Date": fmt.fmt_date(d),
            "Kind": kind,
            "Amount": fmt.money(amt, dp=0),
            "Rate": fmt.pct(float(rate) * 100, dp=3) if rate is not None and not _is_nan(rate) else "—",
            "DF": fmt.ratio(float(r["discount_factor"]), dp=4) if "discount_factor" in r else "—",
            "PV": fmt.money(pv, dp=0),
        })
    ladder = [(str(y), by_year[y][0] / 1e6, by_year[y][1] / 1e6, by_year[y][2] / 1e6) for y in sorted(by_year)]
    return ladder, schedule


# ---------------------------------------------------------------------------
# Task 6: Assembly — instrument_tearsheet public API
# ---------------------------------------------------------------------------

ALL_SECTIONS = ["definition", "valuation", "keyrate", "cashflows", "schedule", "payoff", "survival", "covenants"]

# Headline KPI metric ids per type (label comes from _metric_cell).
# Reconciliation: "default_probability" replaced with "default01" for CDS.
_KPI_METRICS: dict[str, list[str]] = {
    "bond": ["dirty_price", "ytm", "duration_mod", "dv01"],
    "credit_default_swap": ["par_spread", "cs01", "jump_to_default", "default01"],
    "equity_option": ["delta", "vega", "implied_vol", "theta"],
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
        ["cs01", "expected_loss", "jump_to_default", "recovery_01"],
    ],
    "equity_option": [
        ["implied_vol"],
        ["delta", "gamma", "vega", "theta"],
        ["rho", "vanna", "volga", "charm"],
    ],
}


def _instrument_type(definition: Any) -> str:
    if definition is None:
        return ""
    if isinstance(definition, str):
        try:
            definition = json.loads(definition)
        except json.JSONDecodeError:
            return ""
    return definition.get("type", "") if isinstance(definition, dict) else ""


def _kpis(result: Any, itype: str) -> list[KPI]:
    ids = _KPI_METRICS.get(itype)
    if not ids:
        # generic: PV + first three present non-composite metrics
        cells: list[tuple[str, str, str]] = [("PV", fmt.money(result.price, result.currency, dp=0), "")]
        count = 0
        for mid in result.metric_keys():
            if "::" in mid:
                continue
            cells.append(_metric_cell(mid, result.get_metric(mid)))
            count += 1
            if count == 3:
                break
        return [KPI(lbl, val, cls) for lbl, val, cls in cells]
    out = []
    for mid in ids:
        lbl, val, cls = _metric_cell(mid, result.get_metric(mid))
        out.append(KPI(lbl, val, cls))
    return out


def _analytics_section(result: Any, itype: str) -> Section:
    groups = _ANALYTICS_GROUPS.get(itype)
    if not groups:
        # generic: one column of every present non-composite metric
        present = [(m, result.get_metric(m)) for m in result.metric_keys() if "::" not in m]
        rows = [_metric_cell(m, v) for m, v in present]
        cols_html = "".join(
            tables.kv_table([(lbl, val, cls) for lbl, val, cls in rows[i::3]], theme=INSTITUTIONAL) for i in range(3)
        )
        return Section("Valuation & Analytics", f'<div class="statgrid">{cols_html}</div>')
    cols_html = ""
    for group in groups:
        rows = [_metric_cell(m, result.get_metric(m)) for m in group if result.get_metric(m) is not None]
        cols_html += tables.kv_table([(lbl, val, cls) for lbl, val, cls in rows], theme=INSTITUTIONAL)
    return Section("Valuation & Analytics", f'<div class="statgrid">{cols_html}</div>')


def _definition_section(definition: Any, theme: Theme) -> Section | None:
    if definition is None:
        return None
    if isinstance(definition, str):
        definition = json.loads(definition)
    cols = _definition_terms(definition)
    cols_html = "".join(tables.kv_table([(k, v, "") for k, v in col], theme=theme) for col in cols if col)
    return Section("Definition", f'<div class="statgrid">{cols_html}</div>')


def _keyrate_section(result: Any, itype: str, theme: Theme) -> Section | None:
    if itype == "credit_default_swap":
        prefix = "bucketed_cs01"
    elif itype == "equity_option":
        prefix = "bucketed_vega"
    else:
        prefix = "bucketed_dv01"
    series = _bucketed_series(result, prefix)
    if not series:
        return None
    labels = [t for t, _ in series]
    vals = [v for _, v in series]
    title_map = {"bucketed_cs01": "Bucketed CS01", "bucketed_vega": "Bucketed Vega"}
    title = title_map.get(prefix, "Key-Rate (Bucketed) DV01")
    return Section(title, charts.bar_chart(labels, vals, theme=theme, y_pct=False, height=175))


def _cashflow_sections(cashflows: Any, theme: Theme) -> list[Section]:
    if cashflows is None:
        return []
    ladder, schedule = _cashflow_blocks(cashflows)
    out: list[Section] = []
    if ladder:
        periods = [p for p, _, _, _ in ladder]
        coupon = [c for _, c, _, _ in ladder]
        principal = [pr for _, _, pr, _ in ladder]
        pv = [p for _, _, _, p in ladder]
        out.append(Section("Cashflow Ladder", charts.cashflow_ladder(periods, coupon, principal, theme=theme, pv=pv)))
    if schedule:
        cols = ["Date", "Kind", "Amount", "Rate", "DF", "PV"]
        out.append(Section("Cashflow Schedule", tables.scroll(tables.data_table(schedule, columns=cols, theme=theme))))
    return out


def _payoff_section(definition: Any, _result: Any, theme: Theme) -> Section | None:
    if definition is None:
        return None
    if isinstance(definition, str):
        definition = json.loads(definition)
    if definition.get("type") != "equity_option":
        return None
    spec = definition.get("spec", {})
    strike = float(spec.get("strike", 0) or 0)
    if strike <= 0:
        return None
    is_call = str(spec.get("option_type", "Call")).lower().startswith("c")
    spots = [strike * (0.6 + 0.04 * i) for i in range(21)]  # 0.6K .. 1.4K
    payoff = [max(s - strike, 0.0) if is_call else max(strike - s, 0.0) for s in spots]
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


def _covenants_section(parsed: dict[str, Any], theme: Theme) -> Section | None:
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
    return Section("Covenants", tables.data_table(rows, columns=cols, theme=theme))


def _build_sections(
    result: Any,
    cashflows: Any,
    definition: Any,
    parsed: dict[str, Any],
    itype: str,
    wanted: list[str],
    theme: Theme,
) -> list[Section]:
    """Dispatch section builders and collect the ordered list of Section objects."""
    secs: list[Section] = []
    if "definition" in wanted and (s := _definition_section(definition, theme)):
        secs.append(s)
    if "valuation" in wanted:
        secs.append(_analytics_section(result, itype))
    if "survival" in wanted and (s := _survival_section(result, cashflows, theme)):
        secs.append(s)
    if "keyrate" in wanted and (s := _keyrate_section(result, itype, theme)):
        secs.append(s)
    if "payoff" in wanted and (s := _payoff_section(definition, result, theme)):
        secs.append(s)
    if "cashflows" in wanted or "schedule" in wanted:
        secs.extend(
            s
            for s in _cashflow_sections(cashflows, theme)
            if (s.title == "Cashflow Ladder" and "cashflows" in wanted)
            or (s.title == "Cashflow Schedule" and "schedule" in wanted)
        )
    if "covenants" in wanted and (s := _covenants_section(parsed, theme)):
        secs.append(s)
    return secs


def _price_path(
    instrument: str | dict,
    market: Any,
    as_of: str | None,
    model: str,
    market_price: float | None,
    cashflows: Any,
) -> tuple[Any, Any, dict]:
    """Price an instrument JSON and return ``(result, cashflows, definition_dict)``.

    Deliberate, documented relaxation of the "reporting never prices" rule, confined
    to this one entry point.

    When ``market_price`` is given, OAS/YTW are computed in a separate call with
    ``quoted_clean_price`` injected, then merged into the main result.  This avoids
    a known limitation where the quote-override path zeroes bucketed DV01 values.
    """
    if market is None or as_of is None:
        raise ValueError("instrument_tearsheet: pricing an instrument JSON requires market= and as_of=")
    spec_obj = json.loads(instrument) if isinstance(instrument, str) else json.loads(json.dumps(instrument))
    itype = spec_obj.get("type", "")
    all_metrics = recommended_metrics(itype)
    base_metrics = [m for m in all_metrics if m not in _NEEDS_QUOTE]
    instrument_json = json.dumps(spec_obj)
    market_arg = market.to_json() if hasattr(market, "to_json") else market
    # Lazy import keeps `import finstack_quant.reporting` light and makes the dependency explicit.
    from finstack_quant.valuations import (
        ValuationResult,
        instrument_cashflows,
        price_instrument_with_metrics,
    )

    # Main pricing call — excludes quote-gated metrics to preserve bucketed DV01.
    result_json = price_instrument_with_metrics(instrument_json, market_arg, as_of, model=model, metrics=base_metrics)

    if market_price is not None:
        # Second call with the market quote injected: solves OAS/YTW.
        # Injecting quoted_clean_price into pricing_overrides zeros bucketed DV01,
        # so we only request the quote-gated metrics and merge them in.
        import copy

        spec_with_quote = copy.deepcopy(spec_obj)
        spec_with_quote.setdefault("spec", {}).setdefault("pricing_overrides", {})["quoted_clean_price"] = market_price
        quote_metrics = [m for m in all_metrics if m in _NEEDS_QUOTE]
        if quote_metrics:
            quote_json = price_instrument_with_metrics(
                json.dumps(spec_with_quote), market_arg, as_of, model=model, metrics=quote_metrics
            )
            d_base = json.loads(result_json)
            d_quote = json.loads(quote_json)
            d_base["measures"].update({k: v for k, v in d_quote["measures"].items() if k in _NEEDS_QUOTE})
            result_json = json.dumps(d_base)

    result = ValuationResult.from_json(result_json)
    if cashflows is None:
        cf_model = "hazard_rate" if model == "hazard_rate" else "discounting"
        cashflows = instrument_cashflows(instrument_json, market_arg, as_of, model=cf_model)
    return result, cashflows, spec_obj


def instrument_tearsheet(
    result: Any,
    *,
    market: Any = None,
    as_of: str | None = None,
    model: str = "discounting",
    market_price: float | None = None,
    cashflows: Any = None,
    definition: Any = None,
    title: str | None = None,
    subtitle: str | None = None,
    sections: list[str] | None = None,
    theme: Theme = INSTITUTIONAL,
    generated: dt.date | None = None,
) -> TearSheet:
    """Render an instrument tear sheet.

    ``result`` is either an already-priced ``valuations.ValuationResult`` (pure
    formatter path) **or** an instrument JSON (``str``/``dict``). For the latter,
    pass ``market=`` and ``as_of=``; the instrument is priced with
    :func:`recommended_metrics` (plus ``oas``/``ytw`` when ``market_price`` is
    given) and its cashflows fetched, then rendered.
    """
    if isinstance(result, (str, dict)):
        result, cashflows, definition = _price_path(result, market, as_of, model, market_price, cashflows)
    wanted = sections if sections is not None else ALL_SECTIONS
    unknown = set(wanted) - set(ALL_SECTIONS)
    if unknown:
        raise ValueError(f"unknown section(s): {sorted(unknown)}; valid: {ALL_SECTIONS}")

    parsed = _parse_result(result)
    itype = _instrument_type(definition)

    secs = _build_sections(result, cashflows, definition, parsed, itype, wanted, theme)

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
