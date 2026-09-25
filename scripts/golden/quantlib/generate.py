"""Generate native QuantLib fixtures (pricing goldens or attribution parity)."""

from __future__ import annotations

import argparse
from collections.abc import Callable
from pathlib import Path
from typing import Any

from .attribution import build_bond_fixture, build_fx_forward_fixture, build_irs_fixture
from .bonds import (
    build_fixed_callable_oas_bond,
    build_fixed_hazard_bond,
    build_fixed_risk_free_bond,
    build_floating_hazard_bond,
    build_floating_risk_free_bond,
)
from .common import require_supported_quantlib, write_or_check
from .credit import build_single_name_cds
from .deposits import build_deposit
from .fx import build_fx_forward
from .fx_exotics import (
    build_fx_barrier_option,
    build_fx_barrier_option_rebate_at_expiry,
    build_fx_barrier_option_rebate_at_hit,
    build_fx_digital_option,
    build_quanto_option,
)
from .options import (
    build_arithmetic_asian_option,
    build_barrier_option,
    build_european_equity_option,
    build_european_fx_option,
    build_fixed_lookback_option,
    build_floating_lookback_option,
    build_geometric_asian_option,
)
from .rate_options import (
    build_bachelier_floorlet,
    build_bachelier_swaption,
    build_black_cap,
    build_black_caplet,
    build_black_swaption,
)
from .rates import build_fra, build_irs, build_sofr_future

WORKSPACE_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_PRICING_ROOT = WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/data/pricing/quantlib"
DEFAULT_ATTRIBUTION_ROOT = WORKSPACE_ROOT / "finstack-quant/valuations/tests/data/quantlib_parity"

PRODUCTS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "deposit": ("deposit/usd_deposit_3m.json", build_deposit),
    "single_name_cds": (
        "cds/cds_quantlib_flat_hazard_decomposition.json",
        build_single_name_cds,
    ),
    "fra": ("fra/usd_fra_3x6_quantlib.json", build_fra),
    "sofr_future": (
        "ir_future/sofr_3m_quarterly_quantlib.json",
        build_sofr_future,
    ),
    "fixed_risk_free_bond": (
        "bond/usd_fixed_10y_risk_free_quantlib.json",
        build_fixed_risk_free_bond,
    ),
    "fixed_hazard_bond": (
        "bond/usd_fixed_5y_hazard_quantlib.json",
        build_fixed_hazard_bond,
    ),
    "fixed_callable_oas_bond": (
        "bond/usd_fixed_callable_8y_oas_quantlib.json",
        build_fixed_callable_oas_bond,
    ),
    "floating_risk_free_bond": (
        "bond/usd_floating_5y_risk_free_quantlib.json",
        build_floating_risk_free_bond,
    ),
    "floating_hazard_bond": (
        "bond/usd_floating_5y_hazard_quantlib.json",
        build_floating_hazard_bond,
    ),
    "european_equity_option": (
        "equity_option/spx_atm_call_1y_quantlib.json",
        build_european_equity_option,
    ),
    "european_fx_option": (
        "fx_option/eurusd_atm_call_3m_quantlib.json",
        build_european_fx_option,
    ),
    "black_caplet": (
        "cap_floor/usd_black_caplet_quantlib.json",
        build_black_caplet,
    ),
    "black_cap": (
        "cap_floor/usd_black_cap_1y_quantlib.json",
        build_black_cap,
    ),
    "bachelier_floorlet": (
        "cap_floor/usd_bachelier_floorlet_quantlib.json",
        build_bachelier_floorlet,
    ),
    "black_swaption": (
        "swaption/usd_black_1y1y_payer_swaption_quantlib.json",
        build_black_swaption,
    ),
    "bachelier_swaption": (
        "swaption/usd_bachelier_1y1y_payer_swaption_quantlib.json",
        build_bachelier_swaption,
    ),
    "fx_forward": (
        "fx_forward/eurusd_1y_forward_quantlib.json",
        build_fx_forward,
    ),
    "fx_digital_option": (
        "fx_digital_option/eurusd_cash_digital_call_3m_quantlib.json",
        build_fx_digital_option,
    ),
    "fx_barrier_option": (
        "fx_barrier_option/eurusd_up_out_call_3m_quantlib.json",
        build_fx_barrier_option,
    ),
    "fx_barrier_option_rebate_at_hit": (
        "fx_barrier_option/eurusd_up_out_call_rebate_at_hit_3m_quantlib.json",
        build_fx_barrier_option_rebate_at_hit,
    ),
    "fx_barrier_option_rebate_at_expiry": (
        "fx_barrier_option/eurusd_up_in_call_rebate_at_expiry_3m_quantlib.json",
        build_fx_barrier_option_rebate_at_expiry,
    ),
    "quanto_option": (
        "quanto_option/nky_usd_quanto_call_1y_quantlib.json",
        build_quanto_option,
    ),
    "barrier_option": (
        "barrier_option/spx_down_out_call_1y_quantlib.json",
        build_barrier_option,
    ),
    "geometric_asian_option": (
        "asian_option/spx_geometric_asian_call_1y_quantlib.json",
        build_geometric_asian_option,
    ),
    "arithmetic_asian_option": (
        "asian_option/spx_arithmetic_asian_call_1y_quantlib.json",
        build_arithmetic_asian_option,
    ),
    "fixed_lookback_option": (
        "lookback_option/spx_fixed_lookback_call_1y_quantlib.json",
        build_fixed_lookback_option,
    ),
    "floating_lookback_option": (
        "lookback_option/spx_floating_lookback_call_1y_quantlib.json",
        build_floating_lookback_option,
    ),
}
DEFERRED_PRODUCTS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "irs": ("irs/usd_sofr_5y_quantlib.json", build_irs),
}
ATTRIBUTION_PRODUCTS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "bond": ("bond_5pct_10y_usd.json", build_bond_fixture),
    "irs": ("irs_5y_usd.json", build_irs_fixture),
    "fx_forward": ("fx_forward_1y_eurusd.json", build_fx_forward_fixture),
}

FamilyConfig = tuple[
    Path, dict[str, tuple[str, Callable[[], dict[str, Any]]]], dict[str, tuple[str, Callable[[], dict[str, Any]]]]
]
FAMILIES: dict[str, FamilyConfig] = {
    "pricing": (DEFAULT_PRICING_ROOT, PRODUCTS, DEFERRED_PRODUCTS),
    "attribution": (DEFAULT_ATTRIBUTION_ROOT, ATTRIBUTION_PRODUCTS, {}),
}


def generate(
    product: str,
    output_root: Path,
    *,
    family: str = "pricing",
    check: bool = False,
) -> list[Path]:
    """Generate one product or every registered product for a fixture family.

    # Arguments
    - `product`: Product key, or ``all`` for every non-deferred product in the family.
    - `output_root`: Directory that receives relative fixture paths.
    - `family`: ``pricing`` (golden schema) or ``attribution`` (T0/T1 parity JSON).
    - `check`: When true, fail if committed content differs instead of writing.
    """
    require_supported_quantlib()
    if family not in FAMILIES:
        msg = f"Unknown QuantLib fixture family: {family}"
        raise ValueError(msg)
    _, products, deferred = FAMILIES[family]
    all_products = products | deferred
    if product == "all":
        selected = products.items()
    elif product in all_products:
        selected = [(product, all_products[product])]
    else:
        known = ", ".join(["all", *sorted(all_products)])
        msg = f"Unknown {family} product {product!r}; expected one of: {known}"
        raise ValueError(msg)
    paths = []
    for _, (relative_path, builder) in selected:
        path = output_root / relative_path
        write_or_check(path, builder(), check=check)
        paths.append(path)
    return paths


def main() -> None:
    """Run the QuantLib fixture generator CLI."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--family",
        choices=sorted(FAMILIES),
        default="pricing",
        help="pricing goldens (default) or attribution T0/T1 parity fixtures",
    )
    parser.add_argument(
        "--product",
        default="all",
        help="product key within the selected family, or 'all'",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=None,
        help="override the family's default output directory",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    default_root, _, _ = FAMILIES[args.family]
    output_root = args.output_root if args.output_root is not None else default_root
    for path in generate(args.product, output_root, family=args.family, check=args.check):
        print(f"checked {path}" if args.check else f"wrote {path}")


if __name__ == "__main__":
    main()
