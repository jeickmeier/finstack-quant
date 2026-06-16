"""Shared pytest configuration for the Python binding tests."""

from __future__ import annotations

import pytest

SLOW_TEST_PATHS = {
    "golden",
}
SLOW_TEST_FILES = {
    "test_monte_carlo.py",
}


def pytest_collection_modifyitems(items: list[pytest.Item]) -> None:
    """Mark expensive Python test groups as slow for the default dev loop."""
    slow = pytest.mark.slow
    for item in items:
        path = item.path
        if path.name in SLOW_TEST_FILES or SLOW_TEST_PATHS.intersection(path.parts):
            item.add_marker(slow)
