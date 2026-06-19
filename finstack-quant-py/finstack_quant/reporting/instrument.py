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
