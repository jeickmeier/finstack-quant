"""Protect numerical tranche pins while reducing their stored representation."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
_SPEC = importlib.util.spec_from_file_location(
    "tranche_reference_storage", ROOT / "scripts/golden/tranche_reference_storage.py"
)
assert _SPEC is not None
assert _SPEC.loader is not None
_STORAGE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_STORAGE)


@pytest.mark.parametrize("input_name", ["pins.json", "pins.report.json"])
def test_output_and_companion_cannot_alias_the_input_export(tmp_path: Path, input_name: str) -> None:
    """Both primary-output and new-sidecar collisions fail before generation."""
    source = tmp_path / input_name
    source.write_text("source export\n", encoding="utf-8")

    with pytest.raises(ValueError, match="must not overwrite the input export"):
        _STORAGE._report_path(source, tmp_path / "pins.json")

    assert source.read_text(encoding="utf-8") == "source export\n"


@pytest.mark.parametrize("alias_name", ["pins.json", "pins.report.json"])
def test_resolved_destinations_cannot_alias_the_input_export(tmp_path: Path, alias_name: str) -> None:
    """A sidecar or primary destination cannot conceal an input alias behind a symlink."""
    source = tmp_path / "export.json"
    source.write_text("source export\n", encoding="utf-8")
    (tmp_path / alias_name).symlink_to(source)

    with pytest.raises(ValueError, match="must not overwrite the input export"):
        _STORAGE._report_path(source, tmp_path / "pins.json")


def test_report_path_allows_distinct_artifacts_and_normalizes_input_aliases(tmp_path: Path) -> None:
    """Distinct source and output files retain the documented companion name."""
    source = tmp_path / "export.json"
    destination = tmp_path / "pins.json"
    assert _STORAGE._report_path(source, destination) == tmp_path / "pins.report.json"

    with pytest.raises(ValueError, match="must not overwrite the input export"):
        _STORAGE._report_path(tmp_path / "nested/../pins.report.json", destination)


def test_projection_removes_only_report_fields_without_mutating_evidence() -> None:
    """Unknown metadata, pins, gates and provenance survive a nonmutating projection."""
    retained = {"provenance": {"script_sha256": "original"}, "gate": 5e-13, "future_field": [None, False, "keep"]}
    report = {
        **retained,
        "baseline_acceptance": {"passed": True},
        "cases": [{"conditional": [{"expected_caps": [0.1], "baseline_errors_with_uncertainty": [0.0]}]}],
        "baseline_grid_deviations": [],
    }
    original = copy.deepcopy(report)

    assert _STORAGE.regression_pins(report) == {
        **retained,
        "cases": [{"conditional": [{"expected_caps": [0.1]}]}],
    }
    assert report == original


def test_compact_writer_preserves_binary64_bits_and_json_strings(tmp_path: Path) -> None:
    """Short node lines remain lossless for extreme floats, signed zero and escaped text."""
    values = [-0.0, 5e-324, sys.float_info.max, 5e-13]
    node = {"conditioning_hash": 'quotes " slash \\ newline\n\u00e9', "expected_caps": values}
    payload = {"conditional": [node, node], "empty": [], "metadata": {}, "flag": True, "optional": None}
    path = tmp_path / "nested" / "pins.json"

    _STORAGE.write_json(path, payload)

    encoded = path.read_text(encoding="utf-8")
    decoded = json.loads(encoded)
    assert decoded == payload
    assert [struct.pack(">d", value) for value in decoded["conditional"][0]["expected_caps"]] == [
        struct.pack(">d", value) for value in values
    ]
    node_lines = [line.strip().rstrip(",") for line in encoded.splitlines() if '"conditioning_hash":' in line]
    assert len(node_lines) == 2
    assert all(json.loads(line) == node for line in node_lines)
    assert encoded.endswith("\n")


@pytest.mark.parametrize("invalid", [float("nan"), float("inf"), float("-inf")])
def test_invalid_numbers_cannot_replace_a_previous_artifact(tmp_path: Path, invalid: float) -> None:
    """Serialization failure leaves the previous file intact with no temporary residue."""
    path = tmp_path / "pins.json"
    path.write_text("previous evidence\n", encoding="utf-8")

    with pytest.raises(ValueError, match="Out of range float values"):
        _STORAGE.write_json(path, {"conditional": [{"expected_caps": [invalid]}]})

    assert path.read_text(encoding="utf-8") == "previous evidence\n"
    assert list(tmp_path.iterdir()) == [path]


def test_checked_in_fixture_retains_the_pinned_payload_and_coverage() -> None:
    """A separately pinned digest protects every retained reference value and gate."""
    manifest = json.loads((ROOT / "benchmarks/valuation-evidence.json").read_text(encoding="utf-8"))
    fixture = manifest["regression_fixture"]
    pins = json.loads((ROOT / fixture["path"]).read_text(encoding="utf-8"))
    canonical = json.dumps(pins, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    assert hashlib.sha256(canonical).hexdigest() == fixture["canonical_sha256"]
    scenarios = [scenario for case in pins["cases"] for scenario in case["scenarios"]]
    periods = [period for scenario in scenarios for period in scenario["periods"]]
    assert {
        "cases": len(pins["cases"]),
        "scenarios": len(scenarios),
        "periods": len(periods),
        "conditional_nodes": sum(len(period["conditional"]) for period in periods),
    } == fixture["coverage"]

    pending = [pins]
    report_keys = {"baseline_errors_with_uncertainty", "baseline_grid_deviations", "baseline_acceptance"}
    while pending:
        value = pending.pop()
        if isinstance(value, dict):
            assert report_keys.isdisjoint(value)
            pending.extend(value.values())
        elif isinstance(value, list):
            pending.extend(value)
