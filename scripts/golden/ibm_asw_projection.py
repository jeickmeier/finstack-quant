"""Rebuild IBM's projection curve from the captured SWYC discount factors.

Run ``uv run --no-sync python scripts/golden/ibm_asw_projection.py --write``.
Without ``--write``, check that the materialized input is reproducible.

The image named ``ibm_eur_2034_callable__coupon_info.png`` actually contains
the SWYC stripped curve. Its discount factors are transcribed in the fixture's
``EUR-SWAP-514`` object. The market-rate column mixes deposit, FRA and swap
quotes; copying it into a ForwardCurve does not reconstruct that curve.
Derive each simple forward as ``(P(start) / P(end) - 1) / ACT360(start,end)``.
The projection grid includes every captured pillar and subdivisions no longer
than the index tenor. No expected valuation or tolerance is used or changed.
"""

import argparse
from bisect import bisect_right
from itertools import pairwise
import json
import math
from pathlib import Path
from typing import Any

FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant/valuations/tests/golden/data/pricing/bloomberg/bond"
    / "ibm_eur_2034_callable_bloomberg.json"
)


def _projection(source: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    """Convert captured ACT/365F DFs into an ACT/360 simple-forward grid."""
    if source["day_count"] != "act_365f" or source["interp_style"] != "log_linear":
        raise ValueError("IBM stripped-curve source must use ACT/365F and log-linear DFs")
    times = [point[0] * 365.0 / 360.0 for point in source["knot_points"]]
    log_dfs = [math.log(point[1]) for point in source["knot_points"]]
    tenor = current["tenor"]
    grid = sorted(set(times) | {i * tenor for i in range(math.ceil(times[-1] / tenor))})

    def log_df(time: float) -> float:
        """Interpolate the captured log discount factors on the new clock."""
        left = min(bisect_right(times, time) - 1, len(times) - 2)
        weight = (time - times[left]) / (times[left + 1] - times[left])
        return log_dfs[left] + weight * (log_dfs[left + 1] - log_dfs[left])

    knots = [[start, math.expm1(log_df(start) - log_df(end)) / (end - start)] for start, end in pairwise(grid)]
    knots.append([grid[-1], knots[-1][1]])
    return {
        **current,
        "base": source["base"],
        "day_count": "act_360",
        "knot_points": knots,
        "projection_grid": grid,
        # This is an observed-DF materialization, not a curve calibrated to
        # the old placeholder deposit and two zero-basis quotes.
        "rate_calibration": None,
    }


def _main() -> None:
    """Check or explicitly rewrite only the IBM projection input."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="replace the materialized projection input")
    args = parser.parse_args()
    fixture = json.loads(FIXTURE.read_text())
    curves = fixture["market"]["envelope"]["prior_market"]
    source = next(curve for curve in curves if curve["id"] == "EUR-SWAP-514")
    index = next(i for i, curve in enumerate(curves) if curve["id"] == "EUR-EURIBOR-6M")
    rebuilt = _projection(source, curves[index])
    if args.write:
        curves[index] = rebuilt
        FIXTURE.write_text(json.dumps(fixture, indent=2) + "\n")
    elif curves[index] != rebuilt:
        raise SystemExit("IBM projection input is stale; rerun with --write")
    print("IBM projection input reproduces the captured stripped-curve pillars.")


if __name__ == "__main__":
    _main()
