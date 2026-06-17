"""Finstack Quant: Python bindings for the Rust quantitative-finance toolkit.

The public API mirrors the Rust umbrella crate structure exactly.
Import subpackages by domain::

    from finstack_quant import core, analytics, valuations

Submodules are loaded lazily — importing ``finstack_quant`` does not pull in every
domain, which reduces cold-start time in CLIs, notebooks, and serverless
contexts.
"""

from __future__ import annotations

import importlib as _importlib
from types import ModuleType
from typing import TYPE_CHECKING

__all__ = [
    "analytics",
    "attribution",
    "cashflows",
    "core",
    "covenants",
    "factor_model",
    "features",
    "margin",
    "monte_carlo",
    "portfolio",
    "scenarios",
    "statements",
    "statements_analytics",
    "valuations",
]

_SUBMODULES: frozenset[str] = frozenset(__all__)

if TYPE_CHECKING:
    from . import (
        analytics as analytics,
        attribution as attribution,
        cashflows as cashflows,
        core as core,
        covenants as covenants,
        factor_model as factor_model,
        features as features,
        margin as margin,
        monte_carlo as monte_carlo,
        portfolio as portfolio,
        scenarios as scenarios,
        statements as statements,
        statements_analytics as statements_analytics,
        valuations as valuations,
    )


def __getattr__(name: str) -> ModuleType:
    if name in _SUBMODULES:
        mod = _importlib.import_module(f".{name}", __name__)
        globals()[name] = mod
        return mod
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
