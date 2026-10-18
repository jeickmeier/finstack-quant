"""Tests for the schema-generation driver's inventory and binary plumbing."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "check_schema_generation.py"
_SPEC = importlib.util.spec_from_file_location("check_schema_generation", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)

_SCHEMA = Path("finstack-quant/example/schemas/example/1/example.schema.json")
_INDEX = Path("finstack-quant/example/schemas/index.json")
_FIXTURE = Path("finstack-quant/valuations/tests/instruments/json_examples/rates/swap.json")


def _write(root: Path, *paths: Path) -> None:
    for path in paths:
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text("{}", encoding="utf-8")


def test_ownership_accepts_a_tree_that_matches_the_registries(tmp_path: Path) -> None:
    """Listed artifacts that all exist, and nothing else owned, pass."""
    _write(tmp_path, _SCHEMA, _INDEX, _FIXTURE)
    assert _MODULE.ownership_mismatch({_SCHEMA, _INDEX, _FIXTURE}, tmp_path) is None


def test_ownership_reports_an_orphaned_instrument_fixture(tmp_path: Path) -> None:
    """A fixture no registry lists is stale, as the generator's `--check` reports."""
    orphan = _FIXTURE.with_name("retired.json")
    _write(tmp_path, _SCHEMA, _INDEX, _FIXTURE, orphan)
    mismatch = _MODULE.ownership_mismatch({_SCHEMA, _INDEX, _FIXTURE}, tmp_path)
    assert mismatch is not None
    assert "missing=none" in mismatch
    assert str(orphan) in mismatch


def test_ownership_reports_an_orphaned_schema(tmp_path: Path) -> None:
    """A `*.schema.json` no registry lists is stale."""
    orphan = _SCHEMA.with_name("retired.schema.json")
    _write(tmp_path, _SCHEMA, _INDEX, orphan)
    mismatch = _MODULE.ownership_mismatch({_SCHEMA, _INDEX}, tmp_path)
    assert mismatch is not None
    assert str(orphan) in mismatch


def test_ownership_reports_a_missing_artifact(tmp_path: Path) -> None:
    """A listed artifact absent from the tree is missing."""
    _write(tmp_path, _SCHEMA)
    mismatch = _MODULE.ownership_mismatch({_SCHEMA, _INDEX}, tmp_path)
    assert mismatch is not None
    assert f"missing=[{_INDEX!r}]" in mismatch


def test_ownership_ignores_unowned_json(tmp_path: Path) -> None:
    """JSON outside the owned roots, such as golden data, is not the generators' business."""
    _write(tmp_path, _SCHEMA, Path("finstack-quant/valuations/tests/golden/case.json"))
    assert _MODULE.ownership_mismatch({_SCHEMA}, tmp_path) is None


def test_command_executes_the_binary_from_the_bin_dir(tmp_path: Path) -> None:
    """Generators run directly from the bin dir, never through `cargo run`."""
    generator = _MODULE.GENERATORS[0]
    (tmp_path / generator.binary).write_text("", encoding="utf-8")
    assert _MODULE.command(generator, tmp_path, "--list") == [str(tmp_path / generator.binary), "--list"]


def test_command_names_the_build_task_when_a_binary_is_missing(tmp_path: Path) -> None:
    """A missing binary fails with the task that builds it."""
    with pytest.raises(FileNotFoundError, match="rust-build-schema-generators"):
        _MODULE.command(_MODULE.GENERATORS[0], tmp_path, "--list")


def test_generator_table_names_each_binary_once() -> None:
    """Two rows for one binary would generate a registry twice."""
    binaries = [generator.binary for generator in _MODULE.GENERATORS]
    assert len(binaries) == len(set(binaries))
