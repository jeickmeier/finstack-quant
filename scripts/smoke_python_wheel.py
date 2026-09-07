"""Smoke-test the installed ``finstack_quant`` wheel."""

from __future__ import annotations

import importlib
import math
from types import ModuleType

import finstack_quant
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money


def _require(condition: bool, message: str) -> None:
    """Raise a useful failure when a packaged-wheel contract is not met."""
    if not condition:
        raise AssertionError(message)


def _import_public_packages() -> dict[str, ModuleType]:
    """Import every public top-level package advertised by the wheel.

    ``finstack_quant.__all__`` also lists ``__version__``, which is a plain
    attribute rather than an importable submodule — skip it here.
    """
    package_names = tuple(name for name in finstack_quant.__all__ if name != "__version__")
    _require(bool(package_names), "finstack_quant.__all__ must advertise packages")
    _require(
        len(package_names) == len(set(package_names)),
        f"finstack_quant.__all__ contains duplicate packages: {package_names}",
    )
    _require(
        "__version__" in finstack_quant.__all__,
        "finstack_quant.__all__ must include __version__",
    )

    return {name: importlib.import_module(f"finstack_quant.{name}") for name in package_names}


def main() -> None:
    """Exercise the installed wheel's package surface and pricing primitives."""
    packages = _import_public_packages()

    version = finstack_quant.__version__
    _require(isinstance(version, str) and bool(version), "finstack_quant.__version__ must be a non-empty str")

    usd = Currency("USD")
    cash = Money(100.0, usd)
    _require(cash.currency == usd, "Money did not preserve its Currency")
    _require(cash.amount == 100.0, f"Money amount changed during construction: {cash.amount}")

    spot = 100.0
    strike = 100.0
    rate = 0.05
    dividend_yield = 0.02
    volatility = 0.20
    expiry = 1.0

    closed_form_price = packages["models"].bs_price(
        spot,
        strike,
        rate,
        dividend_yield,
        volatility,
        expiry,
        True,
    )
    estimate = (
        packages["models"]
        .monte_carlo.EuropeanPricer(
            num_paths=10_000,
            seed=42,
            use_parallel=False,
        )
        .price_call(
            spot,
            strike,
            rate,
            dividend_yield,
            volatility,
            expiry,
            num_steps=1,
        )
    )
    monte_carlo_price = estimate.mean.amount

    for label, price in (
        ("models.bs_price", closed_form_price),
        ("models.monte_carlo.EuropeanPricer.price_call", monte_carlo_price),
    ):
        _require(math.isfinite(price), f"{label} returned a non-finite price: {price}")
        _require(price > 0.0, f"{label} returned a non-positive price: {price}")

    _require(
        math.isfinite(estimate.stderr) and estimate.stderr > 0.0, "Monte Carlo stderr must be a positive finite number"
    )
    _require(
        abs(closed_form_price - monte_carlo_price) < 4.0 * estimate.stderr,
        "Black-Scholes Monte Carlo drifted from closed form: "
        f"closed_form={closed_form_price}, monte_carlo={monte_carlo_price}, stderr={estimate.stderr}",
    )

    print(
        f"Packaged wheel smoke passed: imported {len(packages)} public packages; "
        f"Black-Scholes call={closed_form_price:.12g}"
    )


if __name__ == "__main__":
    main()
