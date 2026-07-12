"""Shared utilities for deterministic QuantLib golden fixtures."""

from __future__ import annotations

from collections.abc import Callable
from datetime import date
from itertools import pairwise
import json
import math
from pathlib import Path
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

SCHEMA_VERSION = "finstack_quant.golden/2"
VALUATION_DATE = "2026-04-30"
CAPTURE_DATE = "2026-07-11"
MIN_QUANTLIB_VERSION = (1, 41)


def require_supported_quantlib() -> None:
    """Reject QuantLib versions below the project's supported floor."""
    version = tuple(int(part) for part in ql.__version__.split(".")[:2])
    if version < MIN_QUANTLIB_VERSION:
        msg = f"QuantLib >= 1.41 is required, found {ql.__version__}"
        raise RuntimeError(msg)


def ql_date(iso_date: str) -> ql.Date:
    """Convert an ISO calendar date to a QuantLib date."""
    year, month, day = (int(part) for part in iso_date.split("-"))
    return ql.Date(day, month, year)


def metadata(
    *,
    name: str,
    domain: str,
    description: str,
    product: str,
    valuation_date: str = VALUATION_DATE,
    source_detail: str | None = None,
) -> dict[str, Any]:
    """Build deterministic QuantLib provenance metadata."""
    command = f"uv run python -m scripts.golden.quantlib.generate --product {product}"
    return {
        "name": name,
        "domain": domain,
        "description": description,
        "valuation_date": valuation_date,
        "source": "quantlib",
        "source_detail": source_detail
        or f"QuantLib {ql.__version__}; deterministic flat-curve native instrument benchmark.",
        "captured_by": "finstack-quant",
        "captured_on": CAPTURE_DATE,
        "last_reviewed_by": "finstack-quant",
        "last_reviewed_on": CAPTURE_DATE,
        "review_interval_months": 6,
        "regen_command": command,
        "screenshots": [],
    }


def flat_discount_curve(curve_id: str, rate: float) -> dict[str, Any]:
    """Build a Finstack flat continuously compounded discount curve."""
    return {
        "type": "discount",
        "id": curve_id,
        "base": VALUATION_DATE,
        "day_count": "Act365F",
        "knot_points": [[0.0, 1.0], [30.0, math.exp(-rate * 30.0)]],
        "interp_style": "log_linear",
        "extrapolation": "flat_forward",
        "min_forward_rate": None,
        "allow_non_monotonic": False,
        "min_forward_tenor": 1e-6,
        "rate_calibration": None,
    }


def flat_forward_curve(
    curve_id: str,
    rate: float,
    *,
    projection_dates: list[str] | None = None,
) -> dict[str, Any]:
    """Build a Finstack flat simple forward curve."""
    curve = {
        "type": "forward",
        "id": curve_id,
        "base": VALUATION_DATE,
        "reset_lag": 2,
        "day_count": "Act360",
        "tenor": 0.25,
        "knot_points": [[0.0, rate], [30.0, rate]],
        "interp_style": "linear",
        "extrapolation": "flat_forward",
        "rate_calibration": None,
    }
    if projection_dates is not None:
        base = date.fromisoformat(VALUATION_DATE)
        contractual_times = [
            (date.fromisoformat(projection_date) - base).days / 360.0 for projection_date in projection_dates
        ]
        projection_grid = [0.0, *contractual_times, 30.0]
        if any(right <= left for left, right in pairwise(projection_grid)):
            msg = "projection_dates must be strictly increasing, after the valuation date, and before 30 years"
            raise ValueError(msg)
        curve["projection_grid"] = projection_grid
    return curve


def constant_vol_surface(
    surface_id: str,
    volatility: float,
    *,
    quote_type: str = "black_lognormal",
    strikes: list[float] | None = None,
) -> dict[str, Any]:
    """Build a constant volatility surface."""
    surface_strikes = strikes or [0.5, 1.0, 2.0]
    return {
        "id": surface_id,
        "expiries": [0.25, 1.0, 2.0],
        "strikes": surface_strikes,
        "secondary_axis": "strike",
        "quote_type": quote_type,
        "interpolation_mode": "vol",
        "vols_row_major": [volatility] * (3 * len(surface_strikes)),
    }


def market_snapshot(
    curves: list[dict[str, Any]],
    *,
    fx: dict[str, Any] | None = None,
    prices: dict[str, Any] | None = None,
    surfaces: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    """Build a materialized market snapshot accepted by the golden runner."""
    return {
        "kind": "snapshot",
        "data": {
            "version": 2,
            "curves": curves,
            "fx": fx,
            "surfaces": surfaces or [],
            "prices": prices or {},
            "series": [],
            "inflation_indices": [],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "collateral": {},
        },
    }


def central_difference(value: Callable[[float], float], base: float, bump: float = 1e-4) -> float:
    """Return the signed value change for one positive bump."""
    return (value(base + bump) - value(base - bump)) / 2.0


def tolerance(abs_tolerance: float, reason: str | None = None) -> dict[str, Any]:
    """Build one absolute tolerance entry."""
    entry: dict[str, Any] = {"abs": abs_tolerance}
    if reason is not None:
        entry["tolerance_reason"] = reason
    return entry


def serialize_fixture(fixture: dict[str, Any]) -> str:
    """Serialize a fixture in stable, human-readable form."""
    return json.dumps(fixture, indent=2, sort_keys=True, allow_nan=False) + "\n"


def write_or_check(path: Path, fixture: dict[str, Any], *, check: bool) -> None:
    """Write a fixture or fail if committed content differs."""
    rendered = serialize_fixture(fixture)
    if check:
        if not path.exists() or path.read_text(encoding="utf-8") != rendered:
            raise RuntimeError(f"QuantLib golden fixture is stale: {path}")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(rendered, encoding="utf-8")
