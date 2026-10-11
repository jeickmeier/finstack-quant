"""Tests for strict Python materialization baseline comparison."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import sys
import time

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "check_python_materialization_regressions.py"
sys.path.insert(0, str(_SCRIPT_PATH.parent))
_SPEC = importlib.util.spec_from_file_location("check_python_materialization_regressions", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)


def _fragment(fixture_a: Path, fixture_b: Path, value: float = 1.0) -> dict[str, object]:
    return {
        "language": "python",
        "fixture_sha256": {
            "a": _MODULE.hashlib.sha256(fixture_a.read_bytes()).hexdigest(),
            "b": _MODULE.hashlib.sha256(fixture_b.read_bytes()).hexdigest(),
        },
        "cases": {name: [value] * 100 for name in _MODULE.EXPECTED_CASES},
    }


def _setup(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> dict[str, Path]:
    monkeypatch.setattr(_MODULE, "tree_revision", lambda: "tree-current")
    fixture_a = tmp_path / "fixture-a.json"
    fixture_b = tmp_path / "fixture-b.json"
    fixture_a.write_text("a", encoding="utf-8")
    fixture_b.write_text("b", encoding="utf-8")
    root = tmp_path / "isolated"
    baseline_measurement = root / "baseline-measurement.json"
    baseline_metadata = root / "baseline-run.json"
    baseline = tmp_path / "docs" / "materialization-python-baseline.json"
    current = root / "current.json"
    metadata = root / "run.json"
    root.mkdir()
    _MODULE.write_run_metadata(fixture_a, fixture_b, baseline_metadata)
    baseline_measurement.write_text(json.dumps(_fragment(fixture_a, fixture_b)), encoding="utf-8")
    _MODULE.write_baseline(
        baseline_measurement,
        baseline_metadata,
        fixture_a,
        fixture_b,
        baseline,
        root,
        "python-materialization-v1",
    )
    assert "measurement" not in json.loads(baseline.read_text(encoding="utf-8"))
    metadata.write_text(
        json.dumps({
            "baseline_name": "python-materialization-v1",
            "baseline_revision": "tree-current",
            "current_revision": "tree-current",
            "fixture_sha256": _MODULE.combined_fixture_digest(fixture_a, fixture_b),
            "started_at_ns": time.time_ns(),
        }),
        encoding="utf-8",
    )
    current.write_text(json.dumps(_fragment(fixture_a, fixture_b)), encoding="utf-8")
    return {
        "fixture_a": fixture_a,
        "fixture_b": fixture_b,
        "root": root,
        "baseline": baseline,
        "baseline_measurement": baseline_measurement,
        "baseline_metadata": baseline_metadata,
        "current": current,
        "metadata": metadata,
    }


def _check(paths: dict[str, Path]) -> list[tuple[str, float]]:
    return _MODULE.check_comparison(
        paths["baseline"],
        paths["current"],
        paths["metadata"],
        paths["fixture_a"],
        paths["fixture_b"],
        paths["root"],
        0.10,
    )


def test_strict_python_comparison_accepts_exact_fresh_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An isolated fresh run with exact provenance passes."""
    assert _check(_setup(tmp_path, monkeypatch)) == []


def test_checked_in_python_baseline_may_live_outside_runtime_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Comparison reads its immutable baseline outside ignored target output."""
    paths = _setup(tmp_path, monkeypatch)
    assert not paths["baseline"].is_relative_to(paths["root"])
    assert _check(paths) == []


def test_compact_baseline_can_be_migrated_from_a_source_backed_record() -> None:
    """A prior acceptance record preserves enough data to compact its baseline."""
    record = {
        "baseline": {
            "python_baseline_tree_revision": "tree-baseline",
            "python_baseline_fixture_sha256": "f" * 64,
        },
        "fixtures": {
            "a": {"sha256": "a" * 64},
            "b": {"sha256": "b" * 64},
        },
        "languages": {"python": {"raw_samples_ms": {name: [10.0] * 100 for name in _MODULE.EXPECTED_CASES}}},
        "gates": {"python_median_changes": dict.fromkeys(_MODULE.EXPECTED_CASES, 0.25)},
    }

    baseline = _MODULE.compact_baseline_from_record(record, "python-materialization-v1")

    assert baseline["schema"] == _MODULE.SCHEMA
    assert baseline["medians_ms"] == dict.fromkeys(_MODULE.EXPECTED_CASES, 8.0)
    assert "measurement" not in baseline


def test_named_python_baseline_is_immutable_without_replace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Normal establishment cannot overwrite an existing named baseline."""
    paths = _setup(tmp_path, monkeypatch)
    original = paths["baseline"].read_bytes()
    with pytest.raises(FileExistsError, match="already exists"):
        _MODULE.write_baseline(
            paths["baseline_measurement"],
            paths["baseline_metadata"],
            paths["fixture_a"],
            paths["fixture_b"],
            paths["baseline"],
            paths["root"],
            "python-materialization-v1",
        )
    assert paths["baseline"].read_bytes() == original


def test_python_baseline_verification_rejects_missing_file(tmp_path: Path) -> None:
    """Normal comparison cannot proceed without the named baseline."""
    fixture_a = tmp_path / "a.json"
    fixture_b = tmp_path / "b.json"
    fixture_a.write_text("a", encoding="utf-8")
    fixture_b.write_text("b", encoding="utf-8")
    with pytest.raises(FileNotFoundError):
        _MODULE.verify_baseline(tmp_path / "missing.json", fixture_a, fixture_b)


def test_python_comparison_writes_machine_result(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Strict comparison exports per-case changes and provenance."""
    paths = _setup(tmp_path, monkeypatch)
    result = paths["root"] / "comparison.json"
    regressions = _MODULE.check_comparison(
        paths["baseline"],
        paths["current"],
        paths["metadata"],
        paths["fixture_a"],
        paths["fixture_b"],
        paths["root"],
        0.10,
        result,
    )
    payload = json.loads(result.read_text(encoding="utf-8"))
    assert regressions == []
    assert payload["passed"] is True
    assert set(payload["median_changes"]) == set(_MODULE.EXPECTED_CASES)
    assert payload["baseline"]["tree_revision"] == "tree-current"


def test_strict_python_comparison_rejects_stale_and_wrong_cases(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Stale output and case-set drift cannot satisfy the gate."""
    paths = _setup(tmp_path, monkeypatch)
    os.utime(paths["current"], ns=(1, 1))
    with pytest.raises(ValueError, match="stale"):
        _check(paths)

    fragment = _fragment(paths["fixture_a"], paths["fixture_b"])
    fragment["cases"].pop(_MODULE.EXPECTED_CASES[-1])
    paths["current"].write_text(json.dumps(fragment), encoding="utf-8")
    with pytest.raises(ValueError, match="case mismatch"):
        _check(paths)


def test_strict_python_comparison_rejects_digest_revision_and_path_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Fixture, revision, and path identities are all mandatory."""
    paths = _setup(tmp_path, monkeypatch)
    metadata = json.loads(paths["metadata"].read_text(encoding="utf-8"))
    metadata["current_revision"] = "stale-tree"
    paths["metadata"].write_text(json.dumps(metadata), encoding="utf-8")
    with pytest.raises(ValueError, match="revision"):
        _check(paths)

    metadata["current_revision"] = "tree-current"
    paths["metadata"].write_text(json.dumps(metadata), encoding="utf-8")
    current = json.loads(paths["current"].read_text(encoding="utf-8"))
    current["fixture_sha256"]["a"] = "0" * 64
    paths["current"].write_text(json.dumps(current), encoding="utf-8")
    with pytest.raises(ValueError, match="fixture digest"):
        _check(paths)

    with pytest.raises(ValueError, match="distinct paths"):
        _MODULE.check_comparison(
            paths["baseline"],
            paths["baseline"],
            paths["metadata"],
            paths["fixture_a"],
            paths["fixture_b"],
            paths["root"],
            0.10,
        )


def test_strict_python_comparison_flags_median_regression(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A median increase above the configured threshold fails."""
    paths = _setup(tmp_path, monkeypatch)
    paths["current"].write_text(
        json.dumps(_fragment(paths["fixture_a"], paths["fixture_b"], value=1.1001)),
        encoding="utf-8",
    )
    assert _check(paths) == [(name, pytest.approx(0.1001)) for name in _MODULE.EXPECTED_CASES]


def _sealable_baseline(name: str = "python-materialization-v1", digest: str = "a" * 64) -> dict[str, object]:
    return {
        "schema": _MODULE.SCHEMA,
        "baseline_name": name,
        "tree_revision": "tree-current",
        "fixture_sha256": digest,
        "fixtures": {"a": "b" * 64, "b": "c" * 64},
        "expected_cases": list(_MODULE.EXPECTED_CASES),
        "medians_ms": dict.fromkeys(_MODULE.EXPECTED_CASES, 1.0),
    }


@pytest.mark.parametrize(
    ("name", "digest"),
    [
        ("../escape", "a" * 64),
        ("python-materialization-v1", "not-a-digest"),
    ],
)
def test_seal_rejects_malformed_identity_without_rewriting_the_manifest(tmp_path: Path, name: str, digest: str) -> None:
    """A bad baseline name or digest is rejected before the manifest is rewritten."""
    baseline = tmp_path / "baseline.json"
    baseline.write_text(json.dumps(_sealable_baseline(name, digest)), encoding="utf-8")
    manifest = tmp_path / "manifest.json"
    original = {
        "provenance": "Captured from base revision.",
        "python_baseline_path": "benchmarks/materialization/materialization-python-baseline.json",
    }
    manifest.write_text(json.dumps(original), encoding="utf-8")
    before = manifest.read_bytes()

    with pytest.raises(ValueError, match=r"baseline_name|fixture_sha256"):
        _MODULE.seal_baseline_manifest(baseline, manifest)

    assert manifest.read_bytes() == before


def test_seal_preserves_existing_manifest_keys(tmp_path: Path) -> None:
    """Sealing updates baseline identity and leaves provenance and path fields in place."""
    baseline = tmp_path / "baseline.json"
    baseline.write_text(json.dumps(_sealable_baseline()), encoding="utf-8")
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps({
            "provenance": "Captured from base revision.",
            "python_baseline_path": "benchmarks/materialization/materialization-python-baseline.json",
        }),
        encoding="utf-8",
    )

    _MODULE.seal_baseline_manifest(baseline, manifest)

    sealed = json.loads(manifest.read_text(encoding="utf-8"))
    assert sealed["provenance"] == "Captured from base revision."
    assert sealed["python_baseline_path"] == "benchmarks/materialization/materialization-python-baseline.json"
    assert sealed["python_baseline_name"] == "python-materialization-v1"
    assert sealed["python_baseline_tree_revision"] == "tree-current"
    assert sealed["python_baseline_fixture_sha256"] == "a" * 64
