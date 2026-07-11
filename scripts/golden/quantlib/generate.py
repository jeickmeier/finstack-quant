"""Generate native QuantLib pricing goldens."""

from __future__ import annotations

import argparse
from collections.abc import Callable
from pathlib import Path
from typing import Any

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
from .fx_exotics import build_fx_barrier_option, build_fx_digital_option, build_quanto_option
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
DEFAULT_OUTPUT_ROOT = WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/data/pricing/quantlib"
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


def generate(product: str, output_root: Path, *, check: bool = False) -> list[Path]:
    """Generate one product or every registered product."""
    require_supported_quantlib()
    all_products = PRODUCTS | DEFERRED_PRODUCTS
    selected = PRODUCTS.items() if product == "all" else [(product, all_products[product])]
    paths = []
    for _, (relative_path, builder) in selected:
        path = output_root / relative_path
        write_or_check(path, builder(), check=check)
        paths.append(path)
    return paths


def main() -> None:
    """Run the QuantLib golden generator CLI."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--product",
        choices=["all", *PRODUCTS, *DEFERRED_PRODUCTS],
        default="all",
    )
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for path in generate(args.product, args.output_root, check=args.check):
        print(f"checked {path}" if args.check else f"wrote {path}")


if __name__ == "__main__":
    main()
