"""Shared, example-only notebook paths, markets, and fixtures."""

from .fixtures import load_instrument_fixture, load_portfolio_fixture
from .instrument_fixtures import acme_bond
from .market import (
    DEMO_AS_OF,
    build_demo_market,
    usd_ois_2026,
    usd_ois_curve,
    usd_sofr_curve,
    usd_sofr_fixings,
)
from .notebook_helpers import banner, print_metrics, series
from .paths import (
    FIXTURE_ROOT,
    NOTEBOOKS_ROOT,
    PYTHON_PACKAGE_ROOT,
    REPOSITORY_ROOT,
    fixture_path,
)
from .synthetic import (
    DEMO_PL_COGS,
    DEMO_PL_OPEX,
    DEMO_PL_PERIODS,
    DEMO_PL_REVENUE,
    RandomWalkPanel,
    demo_pl_builder,
    demo_pl_model,
    random_walk_panel,
)

__all__ = [
    "DEMO_AS_OF",
    "DEMO_PL_COGS",
    "DEMO_PL_OPEX",
    "DEMO_PL_PERIODS",
    "DEMO_PL_REVENUE",
    "FIXTURE_ROOT",
    "NOTEBOOKS_ROOT",
    "PYTHON_PACKAGE_ROOT",
    "REPOSITORY_ROOT",
    "RandomWalkPanel",
    "acme_bond",
    "banner",
    "build_demo_market",
    "demo_pl_builder",
    "demo_pl_model",
    "fixture_path",
    "load_instrument_fixture",
    "load_portfolio_fixture",
    "print_metrics",
    "random_walk_panel",
    "series",
    "usd_ois_2026",
    "usd_ois_curve",
    "usd_sofr_curve",
    "usd_sofr_fixings",
]
