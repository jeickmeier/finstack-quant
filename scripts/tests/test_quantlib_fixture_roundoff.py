"""Generator freshness separates adjacent reference floats from financial drift."""

from copy import deepcopy
import math
from pathlib import Path

import pytest

from scripts.golden.quantlib.common import serialize_fixture, write_or_check


def fixture() -> dict:
    """Return distinct model-input, reference-output and tolerance sections."""
    return {
        "market": {"rate": 0.04},
        "expected": {"delta": 517970.8009361119, "npv": 100.0},
        "tolerances": {"delta": {"abs": 1e-7}},
    }


def test_adjacent_reference_float_preserves_committed_bytes(tmp_path: Path) -> None:
    """Checking and regeneration retain an adjacent finite reference float."""
    path = tmp_path / "fixture.json"
    original = fixture()
    path.write_text(serialize_fixture(original))
    generated = deepcopy(original)
    generated["expected"]["delta"] = math.nextafter(original["expected"]["delta"], -math.inf)
    for check in [True, False]:
        write_or_check(path, generated, check=check)
        assert path.read_text() == serialize_fixture(original)


@pytest.mark.parametrize("changed", ["two_ulps", "market", "tolerance", "metric", "type"])
def test_roundoff_does_not_hide_other_drift(tmp_path: Path, changed: str) -> None:
    """Changes beyond reference rounding remain stale and never mutate on check."""
    path = tmp_path / "fixture.json"
    original = fixture()
    path.write_text(serialize_fixture(original))
    generated = deepcopy(original)
    if changed == "two_ulps":
        v = original["expected"]["delta"]
        generated["expected"]["delta"] = math.nextafter(math.nextafter(v, -math.inf), -math.inf)
    elif changed == "market":
        generated["market"]["rate"] = math.nextafter(0.04, math.inf)
    elif changed == "tolerance":
        generated["tolerances"]["delta"]["abs"] = math.nextafter(1e-7, math.inf)
    elif changed == "metric":
        generated["expected"]["extra"] = 0.0
    else:
        generated["expected"]["npv"] = 100
    with pytest.raises(RuntimeError, match="stale"):
        write_or_check(path, generated, check=True)
    assert path.read_text() == serialize_fixture(original)
