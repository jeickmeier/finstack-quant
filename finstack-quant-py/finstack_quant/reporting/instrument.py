# finstack-quant-py/finstack_quant/reporting/instrument.py
"""Instrument tear sheet: render a priced valuations.ValuationResult as HTML.

Pure formatter — reads the already-priced ``result`` (scalar metrics incl.
``bucketed_*::curve::tenor`` composite keys, covenants), parses ``result.to_json()``
read-only for meta/details, and renders the optional ``cashflows`` DataFrame and
``definition`` JSON. It never prices; ``recommended_metrics`` is a static mapping.
"""

from __future__ import annotations

import json
import re
from typing import Any

from . import format as fmt

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
        "asw_par",
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
#   "price" 2dp as-is             "money" thousands 0dp
#   "ratio" 2dp                   "ratio4" 4dp
_METRIC_FMT: dict[str, tuple[str, str]] = {
    "dirty_price": ("Dirty Price", "price"),
    "clean_price": ("Clean Price", "price"),
    "accrued": ("Accrued", "price"),
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
