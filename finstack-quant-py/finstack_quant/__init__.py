"""Finstack Quant: Python bindings for the Rust quantitative-finance toolkit.

The public API mirrors the Rust umbrella crate structure exactly.
Import subpackages by domain::

    from finstack_quant import calibration, core, analytics, valuations

Submodules are loaded lazily — importing ``finstack_quant`` does not pull in every
domain, which reduces cold-start time in CLIs, notebooks, and serverless
contexts.

The installed version is available as ``finstack_quant.__version__`` — record it
alongside results so a notebook stays reproducible.

Examples:
>>> from finstack_quant import core
>>> core.dates.Tenor.parse("3M").months
3

"""

import importlib as _importlib
from types import ModuleType
from typing import TYPE_CHECKING

__all__ = [
    "__version__",
    "analytics",
    "attribution",
    "calibration",
    "cashflows",
    "core",
    "covenants",
    "features",
    "margin",
    "models",
    "portfolio",
    "reporting",
    "scenarios",
    "schema",
    "statements",
    "statements_analytics",
    "valuations",
]

# Lazily importable domains. `__all__` also carries `__version__`, which is a
# plain attribute served by `__getattr__` rather than the package importer.
_SUBMODULES: frozenset[str] = frozenset(__all__) - {"__version__"}

if TYPE_CHECKING:
    # Declared for type checkers and IDEs; resolved lazily via `__getattr__`.
    __version__: str

    from . import (
        analytics as analytics,
        attribution as attribution,
        calibration as calibration,
        cashflows as cashflows,
        core as core,
        covenants as covenants,
        features as features,
        margin as margin,
        models as models,
        portfolio as portfolio,
        reporting as reporting,
        scenarios as scenarios,
        schema as schema,
        statements as statements,
        statements_analytics as statements_analytics,
        valuations as valuations,
    )


def __getattr__(name: str) -> ModuleType | str:
    if name == "__version__":
        # Served lazily for the same reason the domains are: reading it must not
        # drag the compiled extension (and every domain's registration) into a
        # bare `import finstack_quant`.
        from finstack_quant.finstack_quant import __version__ as version

        globals()["__version__"] = version
        return version
    if name in _SUBMODULES:
        mod = _importlib.import_module(f".{name}", __name__)
        globals()[name] = mod
        return mod
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
