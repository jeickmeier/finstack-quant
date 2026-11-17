"""Rebuild IBM's projection curve from the captured SWYC discount factors.

Run ``uv run --no-sync python scripts/golden/ibm_asw_projection.py --write``.
Without ``--write``, check that the materialized input is reproducible.

The image named ``ibm_eur_2034_callable__coupon_info.png`` actually contains
the SWYC stripped curve. Its discount factors are transcribed in the fixture's
``EUR-SWAP-514`` object. The market-rate column mixes deposit, FRA and swap
quotes; copying it into a ForwardCurve does not reconstruct that curve.
The projection curve's factors are the captured DFs, moved from the ACT/365F
stripped-curve clock to the ACT/360 index clock, so every simple forward is
``(P(start) / P(end) - 1) / ACT360(start,end)``. Log-linear interpolation is
linear in the elapsed time, so the clock change preserves the DF between pillars.
No expected valuation or tolerance is used or changed.
"""

import argparse
import json
from pathlib import Path
from typing import Any

FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant/valuations/tests/golden/data/pricing/bloomberg/bond"
    / "ibm_eur_2034_callable_bloomberg.json"
)


def _projection(source: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    """Re-clock captured ACT/365F DFs as ACT/360 projection factors."""
    if source["day_count"] != "act_365f" or source["interp_style"] != "log_linear":
        raise ValueError("IBM stripped-curve source must use ACT/365F and log-linear DFs")
    return {
        **current,
        "base": source["base"],
        "day_count": "act_360",
        "knot_points": [[time * 365.0 / 360.0, df] for time, df in source["knot_points"]],
        "interp_style": "log_linear",
        "extrapolation": source["extrapolation"],
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
