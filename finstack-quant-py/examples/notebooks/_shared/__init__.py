"""Shared, example-only notebook paths, markets, and fixtures."""

from .fixtures import load_instrument_fixture, load_portfolio_fixture
from .market import DEMO_AS_OF, build_demo_market
from .paths import (
    FIXTURE_ROOT,
    NOTEBOOKS_ROOT,
    PYTHON_PACKAGE_ROOT,
    REPOSITORY_ROOT,
    fixture_path,
)

__all__ = [
    "DEMO_AS_OF",
    "FIXTURE_ROOT",
    "NOTEBOOKS_ROOT",
    "PYTHON_PACKAGE_ROOT",
    "REPOSITORY_ROOT",
    "build_demo_market",
    "fixture_path",
    "load_instrument_fixture",
    "load_portfolio_fixture",
]
