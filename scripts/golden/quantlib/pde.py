"""Finite-difference model golden builders for products that already have a closed-form fixture.

Each fixture reprices an existing instrument with one of Finstack's PDE
models and compares it with the QuantLib closed-form value of the base
fixture. The PDE grids are fixed in the pricers, so the tolerance is taken
from a grid-refinement run recorded here: the price on the production grid
and on a grid with twice the points in every direction. The schemes are
second order, so the production-grid error is about 4/3 of the difference
between the two; the tolerance is twice that difference. It was fixed from
the two Finstack prices alone, before any comparison with the reference.

The 2D ADI Heston model has no fixture. The same run on the Heston call
gave 44247.34 on the production grid (200 by 80 space points, 100 time
steps) and 44233.10 with every dimension doubled, against QuantLib's
44214.36: the gap closes at about first order, so the rule above does not
bound it, and the production grid sits 7.5 basis points of price away.

Refining one dimension at a time (error against QuantLib, 2026-11-25):
time steps 100, 200, 400 give +33.0, +33.3, +33.4, so time is converged;
variance points 80, 160, 320 give +33.0, +1.6, -9.0; spot points 200, 400,
800 give +33.0, +49.7, +46.4. All three doubled twice gives +18.7 then +5.0,
which is second order. Two causes follow. The payoff kink is sampled at
nodes that are not aligned with the strike, so the spot error is not
monotone; averaging the payoff over each spot cell makes it monotone but
removes a cancellation (production error +54.7). The remaining error is in
the variance direction (order about 1.5), where the zero-variance boundary
is a linear extrapolation instead of the degenerate equation, on parameters
that violate the Feller condition.
"""

from __future__ import annotations

from collections.abc import Callable
import copy
from typing import Any

from .options import build_barrier_option, build_european_equity_option

CAPTURED_ON = "2026-11-25"

# product -> (grid description, price on the production grid, price on the doubled grid).
REFINEMENT: dict[str, tuple[str, float, float]] = {
    "equity_option_pde": ("200 space points, 100 time steps", 873.954521237558, 873.950127209540),
    "barrier_option_pde": ("200 space points, 100 time steps", 9.224650159571, 9.225016899444),
}


def _retarget(base: dict[str, Any], *, product: str, model: str, name: str, description: str) -> dict[str, Any]:
    grid, production, doubled = REFINEMENT[product]
    fixture = copy.deepcopy(base)
    fixture["model"] = model
    meta = fixture["metadata"]
    meta["name"] = name
    meta["description"] = description
    meta["regen_command"] = f"uv run python -m scripts.golden.quantlib.generate --product {product}"
    meta["captured_on"] = meta["last_reviewed_on"] = CAPTURED_ON
    meta["source_detail"] = (
        f"{meta['source_detail']} Reference value {base['expected']['npv']!r}. Finstack grid refinement "
        f"({grid}): {production!r} on the production grid, {doubled!r} with every dimension doubled."
    )
    fixture["instrument"]["instrument"]["spec"]["id"] = name.upper().replace("_", "-")
    fixture["expected"] = {"npv": base["expected"]["npv"]}
    fixture["tolerances"] = {
        "npv": {
            "abs": 2.0 * abs(production - doubled),
            "tolerance_reason": (
                "Twice the change in Finstack's own price when every grid dimension is doubled; a second-order "
                "scheme's production-grid error is about 4/3 of that change."
            ),
        }
    }
    return fixture


def build_european_equity_option_pde() -> dict[str, Any]:
    """Build the European equity call fixture priced by the 1D Crank-Nicolson PDE."""
    return _retarget(
        build_european_equity_option(),
        product="equity_option_pde",
        model="pde_crank_nicolson_1d",
        name="spx_atm_call_1y_pde_quantlib",
        description="One-year European equity call priced by the 1D Crank-Nicolson PDE against Black-Scholes.",
    )


def build_barrier_option_pde() -> dict[str, Any]:
    """Build the continuous down-and-out call fixture priced by the 1D Crank-Nicolson PDE."""
    return _retarget(
        build_barrier_option(),
        product="barrier_option_pde",
        model="pde_crank_nicolson_1d",
        name="spx_down_out_call_1y_pde_quantlib",
        description="One-year continuous down-and-out equity call priced by the 1D PDE against the closed form.",
    )


PDE_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "equity_option_pde": ("equity_option/spx_atm_call_1y_pde_quantlib.json", build_european_equity_option_pde),
    "barrier_option_pde": ("barrier_option/spx_down_out_call_1y_pde_quantlib.json", build_barrier_option_pde),
}
