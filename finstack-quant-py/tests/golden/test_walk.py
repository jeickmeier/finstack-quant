"""Walk-test validating every committed golden fixture."""

from __future__ import annotations

from pathlib import Path
import re

import pytest

from .conftest import DATA_ROOTS, WORKSPACE_ROOT, validate_fixture
from .schema import GoldenFixture

VALUATION_DATA_ROOT = WORKSPACE_ROOT / "finstack-quant/valuations/tests/golden/data"
PYTHON_DISCOVER_FIXTURES_RE = re.compile(r'discover_fixtures(?:_with_marks)?\("([^"]+)"\)')


def _all_fixtures() -> list[Path]:
    paths: list[Path] = []
    seen: set[Path] = set()
    for root in DATA_ROOTS.values():
        if root in seen or not root.exists():
            continue
        seen.add(root)
        paths.extend(path for path in root.rglob("*.json") if "screenshots" not in path.parts)
    return sorted(paths)


def _declared_python_fixture_roots() -> set[str]:
    declared: set[str] = set()
    for source in (WORKSPACE_ROOT / "finstack-quant-py/tests/golden").glob("test_*.py"):
        declared.update(PYTHON_DISCOVER_FIXTURES_RE.findall(source.read_text(encoding="utf-8")))
    return declared


@pytest.mark.parametrize("path", _all_fixtures(), ids=lambda path: str(path.relative_to(WORKSPACE_ROOT)))
def test_fixture_well_formed(path: Path) -> None:
    fixture = GoldenFixture.from_path(path)
    validate_fixture(path, fixture)


def test_python_discovery_covers_all_golden_json_fixtures() -> None:
    declared_roots = _declared_python_fixture_roots()
    discovered: set[str] = set()
    for root in declared_roots:
        root_path = VALUATION_DATA_ROOT / root
        if root_path.exists():
            discovered.update(
                str(path.relative_to(VALUATION_DATA_ROOT))
                for path in root_path.rglob("*.json")
                if "screenshots" not in path.parts
            )

    all_fixtures = {
        str(path.relative_to(VALUATION_DATA_ROOT))
        for path in VALUATION_DATA_ROOT.rglob("*.json")
        if "screenshots" not in path.parts
    }
    missing = sorted(all_fixtures - discovered)
    assert not missing, (
        "fixtures missing Python discover_fixtures() coverage:\n"
        + "\n".join(missing)
        + "\nAdd/expand discover_fixtures(...) usage so new JSON fixtures run automatically."
    )
