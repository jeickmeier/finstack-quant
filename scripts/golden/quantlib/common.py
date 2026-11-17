"""Shared utilities for deterministic QuantLib golden fixtures."""

from __future__ import annotations

from collections.abc import Callable
from itertools import pairwise
import json
import math
from pathlib import Path
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

SCHEMA = "finstack_quant.golden/1"
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


def ql_date_to_iso(d: ql.Date) -> str:
    """Convert a QuantLib date to an ISO8601 calendar date string."""
    return f"{d.year():04d}-{d.month():02d}-{d.dayOfMonth():02d}"


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
        "day_count": "act_365f",
        "knot_points": [[0.0, 1.0], [30.0, math.exp(-rate * 30.0)]],
        "interp_style": "log_linear",
        "extrapolation": "extend",
        "rate_calibration": None,
    }


def chained_simple_discounts(dates: list[ql.Date], rate: float, accrual_day_count: ql.DayCounter) -> list[float]:
    """Return projection discount factors on ``dates`` for a flat simple index forward.

    ``dates[0]`` is the curve reference date; each consecutive date pair
    accrues ``rate`` with ``accrual_day_count``, so every contractual index
    period on the grid projects exactly ``rate``.
    """
    discounts = [1.0]
    for start, end in pairwise(dates):
        discounts.append(discounts[-1] / (1.0 + rate * accrual_day_count.yearFraction(start, end)))
    return discounts


def projection_knots(dates: list[ql.Date], discounts: list[float], *, shift: float = 0.0) -> list[tuple[float, float]]:
    """Return the Act/365F ``(time, factor)`` knots of a QuantLib projection curve.

    ``dates[0]`` is the curve reference date. ``shift`` is the parallel
    continuously compounded zero bump Finstack's DV01 applies to every curve,
    ``P(t) * exp(-shift * t)``, so both engines bump identical factors.
    """
    day_count = ql.Actual365Fixed()
    knots = []
    for knot_date, discount in zip(dates, discounts, strict=True):
        time = day_count.yearFraction(dates[0], knot_date)
        knots.append((time, discount * math.exp(-shift * time)))
    return knots


def ql_projection_curve(
    dates: list[ql.Date], discounts: list[float], *, shift: float = 0.0
) -> ql.YieldTermStructureHandle:
    """Build the log-linear QuantLib projection curve matching ``projection_knots``."""
    factors = [factor for _, factor in projection_knots(dates, discounts, shift=shift)]
    return ql.YieldTermStructureHandle(ql.DiscountCurve(dates, factors, ql.Actual365Fixed(), ql.NullCalendar()))


def forward_curve(curve_id: str, knots: list[tuple[float, float]], *, index_months: int) -> dict[str, Any]:
    """Build a Finstack projection curve from Act/365F projection-factor knots."""
    return {
        "type": "forward",
        "id": curve_id,
        "base": VALUATION_DATE,
        "day_count": "act_365f",
        "index_tenor": {"count": index_months, "unit": "months"},
        "knot_points": [[time, factor] for time, factor in knots],
        "interp_style": "log_linear",
        "extrapolation": "extend",
        "rate_calibration": None,
    }


def flat_forward_curve(curve_id: str, continuous_rate: float, *, index_months: int = 3) -> dict[str, Any]:
    """Build the Finstack twin of a QuantLib Act/365F continuous ``FlatForward`` projection curve."""
    return forward_curve(
        curve_id,
        [(0.0, 1.0), (30.0, math.exp(-continuous_rate * 30.0))],
        index_months=index_months,
    )


def constant_vol_surface(
    vol_surface_id: str,
    volatility: float,
    *,
    quote_type: str = "black_lognormal",
    strikes: list[float] | None = None,
) -> dict[str, Any]:
    """Build a constant volatility surface."""
    surface_strikes = strikes or [0.5, 1.0, 2.0]
    return {
        "id": vol_surface_id,
        "expiries": [0.25, 1.0, 2.0],
        "secondary_values": surface_strikes,
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
            "schema_version": 1,
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
            "hierarchy": None,
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
    """Write or check a fixture, preserving adjacent-float reference roundoff.

    Only finite float metrics directly under ``expected`` may differ by one
    representable float. Inputs, tolerances, types, keys and formatting remain
    exact. This machine-roundoff rule does not use financial test tolerances.
    Both modes retain the existing bytes when this is the only difference.
    """
    rendered = serialize_fixture(fixture)
    if path.exists():
        existing = path.read_text(encoding="utf-8")
        if existing == rendered:
            return
        try:
            stored = json.loads(existing)
        except json.JSONDecodeError:
            stored = None
        if isinstance(stored, dict) and isinstance(stored.get("expected"), dict):
            expected = fixture.get("expected")
            if isinstance(expected, dict):
                retained = expected.copy()
                for metric, value in expected.items():
                    previous = stored["expected"].get(metric)
                    if (
                        type(previous) is float
                        and type(value) is float
                        and previous != value
                        and math.isfinite(previous)
                        and math.isfinite(value)
                        and math.nextafter(previous, value) == value
                    ):
                        retained[metric] = previous
                if serialize_fixture({**fixture, "expected": retained}) == existing:
                    print(f"retained adjacent-float reference roundoff: {path}")
                    return
    if check:
        raise RuntimeError(f"QuantLib golden fixture is stale: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(rendered, encoding="utf-8")
