"""Finite-difference model golden builders for products that already have a closed-form fixture.

Each fixture reprices an existing instrument with one of Finstack's PDE
models and compares it with the QuantLib closed-form value of the base
fixture. The PDE grids are fixed in the pricers, so the tolerance is taken
from a grid-refinement run recorded here: the price on the production grid
and on a grid with twice the points in every direction. The schemes are
second order, so the production-grid error is about 4/3 of the difference
between the two; the tolerance is twice that difference. It was fixed from
the two Finstack prices alone, before any comparison with the reference.

The 2D ADI Heston fixture follows the same rule on the Heston call: 200 by
80 space points and 100 time steps in production, every dimension doubled
for the second price. Before 2026-11-25 the model had no fixture. It priced
44247.34 on the production grid and 44233.10 doubled, against QuantLib's
44214.36: 7.5 basis points of price away and closing at about first order,
so the rule did not bound it. Four changes to the scheme made it second
order (the change in price falls by a factor of 3.53 from one doubling to
the next), each chosen from Finstack's own refinement differences:

- zero variance is solved as part of the system with the degenerate Heston
  equation and a one-sided variance derivative, where it was a linear
  extrapolation (order about 1.5 on parameters that violate the Feller
  condition);
- the spot convection on that zero-variance line is central, since an upwind
  difference there adds diffusion to a line that has none (first order in
  the spot step);
- the payoff is averaged over each spot cell, so the kink need not sit on a
  node;
- both axes carry a node at the point the price is read from, where a
  bilinear read-off between nodes carried about 1.3 basis points of price
  on the production grid, an error that depends on where the point falls
  inside its cell and so does not shrink evenly from grid to grid.
"""

from __future__ import annotations

from collections.abc import Callable
import copy
from typing import Any

from .options import build_barrier_option, build_european_equity_option
from .stochastic_vol import build_heston_equity_call

CAPTURED_ON = "2026-11-25"

# product -> (grid description, price on the production grid, price on the doubled grid).
REFINEMENT: dict[str, tuple[str, float, float]] = {
    "equity_option_pde": ("200 space points, 100 time steps", 873.954521237558, 873.950127209540),
    "barrier_option_pde": ("200 space points, 100 time steps", 9.224650159571, 9.225016899444),
    "heston_equity_call_pde": (
        "200 by 80 space points, 100 time steps",
        44217.744307716581,
        44215.286346737623,
    ),
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


def build_heston_equity_call_pde() -> dict[str, Any]:
    """Build the Heston equity call fixture priced by the 2D ADI PDE."""
    return _retarget(
        build_heston_equity_call(),
        product="heston_equity_call_pde",
        model="pde_adi_2d",
        name="spx_heston_atm_call_1y_pde_quantlib",
        description="One-year Heston equity call priced by the 2D ADI PDE against the semi-analytic value.",
    )


PDE_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "equity_option_pde": ("equity_option/spx_atm_call_1y_pde_quantlib.json", build_european_equity_option_pde),
    "barrier_option_pde": ("barrier_option/spx_down_out_call_1y_pde_quantlib.json", build_barrier_option_pde),
    "heston_equity_call_pde": (
        "equity_option/spx_heston_atm_call_1y_pde_quantlib.json",
        build_heston_equity_call_pde,
    ),
}
