"""Tests for durable materialization baselines and record orchestration."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "manage_materialization_rust_baseline.py"
sys.path.insert(0, str(_SCRIPT_PATH.parent))
_SPEC = importlib.util.spec_from_file_location("manage_materialization_rust_baseline", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)

_FLAG_SCRIPT_PATH = Path(__file__).parents[1] / "materialization_baseline_replace_flag.py"
_FLAG_SPEC = importlib.util.spec_from_file_location("materialization_baseline_replace_flag", _FLAG_SCRIPT_PATH)
assert _FLAG_SPEC is not None
assert _FLAG_SPEC.loader is not None
_FLAG_MODULE = importlib.util.module_from_spec(_FLAG_SPEC)
_FLAG_SPEC.loader.exec_module(_FLAG_MODULE)

_DOCS_SCRIPT_PATH = Path(__file__).parents[1] / "check_materialization_benchmark_docs.py"
_DOCS_SPEC = importlib.util.spec_from_file_location("check_materialization_benchmark_docs", _DOCS_SCRIPT_PATH)
assert _DOCS_SPEC is not None
assert _DOCS_SPEC.loader is not None
_DOCS_MODULE = importlib.util.module_from_spec(_DOCS_SPEC)
_DOCS_SPEC.loader.exec_module(_DOCS_MODULE)


def _criterion_current(root: Path, value_ms: float = 10.0) -> None:
    for case in _MODULE.EXPECTED_CASES:
        destination = root / "portfolio_materialization" / case / "new" / "estimates.json"
        destination.parent.mkdir(parents=True)
        destination.write_text(
            json.dumps({
                "median": {
                    "point_estimate": value_ms * 1_000_000,
                    "confidence_interval": {
                        "lower_bound": value_ms * 0.99 * 1_000_000,
                        "upper_bound": value_ms * 1.01 * 1_000_000,
                    },
                }
            }),
            encoding="utf-8",
        )


def _fixtures(tmp_path: Path) -> tuple[Path, Path]:
    fixture_a = tmp_path / "a.json"
    fixture_b = tmp_path / "b.json"
    fixture_a.write_text("a", encoding="utf-8")
    fixture_b.write_text("b", encoding="utf-8")
    return fixture_a, fixture_b


def test_rust_baseline_guard_rejects_existing_checked_in_baseline(tmp_path: Path) -> None:
    """Existing compact baselines require an explicit replacement opt-in."""
    baseline = tmp_path / "materialization-rust-baseline.json"
    baseline.write_text("{}\n", encoding="utf-8")

    with pytest.raises(FileExistsError, match="already exists"):
        _MODULE.guard_establishment(baseline, replace=False)

    _MODULE.guard_establishment(baseline, replace=True)


@pytest.mark.parametrize("value", ["", "0", "true", "yes", "01", " 1"])
def test_replacement_flag_rejects_every_value_except_exact_one(value: str) -> None:
    """Truthy-looking and empty values cannot accidentally enable replacement."""
    with pytest.raises(ValueError, match="unset or exactly '1'"):
        _FLAG_MODULE.replacement_requested(value)


def test_replacement_flag_accepts_only_unset_or_exact_one() -> None:
    """Unset preserves the baseline; exact ``1`` enables intentional replacement."""
    assert _FLAG_MODULE.replacement_requested(None) is False
    assert _FLAG_MODULE.replacement_requested("1") is True


def test_rust_baseline_establishment_writes_compact_durable_artifact(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Establishment stores medians and provenance, not Criterion scratch files."""
    monkeypatch.setattr(_MODULE, "tree_revision", lambda: "tree-current")
    monkeypatch.setattr(_MODULE, "_git_head", lambda: "head")
    criterion = tmp_path / "target" / "materialization-criterion" / "criterion"
    _criterion_current(criterion)
    fixture_a, fixture_b = _fixtures(tmp_path)
    metadata = tmp_path / "run.json"
    metadata.write_text(
        json.dumps({
            "current_revision": "tree-current",
            "fixture_sha256": _MODULE.combined_fixture_digest(fixture_a, fixture_b),
            "started_at_ns": 1,
        }),
        encoding="utf-8",
    )
    output = tmp_path / "docs" / "benchmarks" / "materialization-rust-baseline.json"

    _MODULE.write_baseline(
        criterion,
        metadata,
        fixture_a,
        fixture_b,
        output,
        "materialization-v1",
    )

    baseline = json.loads(output.read_text(encoding="utf-8"))
    assert baseline["schema"] == "finstack_quant.materialization_rust_baseline/1"
    assert baseline["expected_cases"] == list(_MODULE.EXPECTED_CASES)
    assert baseline["medians_ms"]["cold_a_5000_unique"]["point"] == 10.0
    assert "criterion_dir" not in baseline


def test_rust_baseline_verification_is_read_only_and_rejects_manifest_mismatch(
    tmp_path: Path,
) -> None:
    """Verification consumes only checked-in compact baseline content."""
    fixture_a, fixture_b = _fixtures(tmp_path)
    baseline = tmp_path / "baseline.json"
    payload = {
        "schema": "finstack_quant.materialization_rust_baseline/1",
        "baseline_name": "materialization-v1",
        "base_revision": "head",
        "tree_revision": "tree",
        "fixture_sha256": _MODULE.combined_fixture_digest(fixture_a, fixture_b),
        "fixtures": {
            "a": hashlib.sha256(fixture_a.read_bytes()).hexdigest(),
            "b": hashlib.sha256(fixture_b.read_bytes()).hexdigest(),
        },
        "expected_cases": list(_MODULE.EXPECTED_CASES),
        "medians_ms": {case: {"point": 10.0, "lower": 9.0, "upper": 11.0} for case in _MODULE.EXPECTED_CASES},
    }
    baseline.write_text(json.dumps(payload), encoding="utf-8")
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps({
            "rust_baseline_name": "materialization-v1",
            "rust_baseline_tree_revision": "tree",
            "rust_baseline_fixture_sha256": payload["fixture_sha256"],
            "rust_baseline_sha256": hashlib.sha256(baseline.read_bytes()).hexdigest(),
            "rust_baseline_cases": list(_MODULE.EXPECTED_CASES),
        }),
        encoding="utf-8",
    )
    before = baseline.read_bytes()

    _MODULE.verify_baseline(baseline, fixture_a, fixture_b, manifest)
    assert baseline.read_bytes() == before

    manifest.write_text("{}\n", encoding="utf-8")
    with pytest.raises(ValueError, match="manifest mismatch"):
        _MODULE.verify_baseline(baseline, fixture_a, fixture_b, manifest)


def test_record_tasks_consume_checked_in_baselines_and_root_target() -> None:
    """Record and compare steps never depend on crate-local or ignored baselines."""
    root = Path(__file__).parents[2]
    script = (root / "scripts" / "materialization_bench.sh").read_text(encoding="utf-8")
    rust_compare = script.split("rust_compare() {", 1)[1].split("\n}\n", 1)[0]
    python_compare = script.split("python_compare() {", 1)[1].split("\n}\n", 1)[0]

    assert "finstack-quant/portfolio/target/" not in script
    assert "RUST_BASELINE=benchmarks/materialization/materialization-rust-baseline.json" in script
    assert "PYTHON_BASELINE=benchmarks/materialization/materialization-python-baseline.json" in script
    assert '--baseline "$RUST_BASELINE"' in rust_compare
    assert '--baseline "$PYTHON_BASELINE"' in python_compare
    assert "--criterion-dir target/materialization-criterion/criterion" in rust_compare
    assert 'FQ_MATERIALIZATION_CRITERION_DIR="$PWD/target/materialization-criterion/criterion"' in script
    assert "target/materialization-python/baseline.json" not in script


def test_documented_toolchains_match_machine_artifact() -> None:
    """Human-readable toolchain versions stay synchronized with the JSON record."""
    root = Path(__file__).parents[2]
    artifact = json.loads(
        (root / "benchmarks/materialization/materialization-benchmark-results.json").read_text(encoding="utf-8")
    )
    document = (root / "benchmarks/MATERIALIZATION_BENCHMARKS.md").read_text(encoding="utf-8")
    _DOCS_MODULE.verify_document(document, artifact)


def test_benchmark_record_links_exact_checked_in_baselines() -> None:
    """Cheap docs checks reject baseline path, identity, and digest drift."""
    root = Path(__file__).parents[2]
    manifest_path = Path("benchmarks/materialization/materialization-benchmark-baseline.json")
    rust_path = Path("benchmarks/materialization/materialization-rust-baseline.json")
    python_path = Path("benchmarks/materialization/materialization-python-baseline.json")
    artifact = json.loads(
        (root / "benchmarks/materialization/materialization-benchmark-results.json").read_text(encoding="utf-8")
    )
    manifest = json.loads((root / manifest_path).read_text(encoding="utf-8"))

    _DOCS_MODULE.verify_baseline_links(
        artifact,
        manifest,
        manifest_path,
        rust_path,
        python_path,
    )


@pytest.mark.parametrize("language", ["rust", "python"])
def test_benchmark_checker_rejects_tampered_manifest_baseline_paths(language: str) -> None:
    """The manifest cannot redirect either durable baseline to another path."""
    root = Path(__file__).parents[2]
    manifest_path = Path("benchmarks/materialization/materialization-benchmark-baseline.json")
    rust_path = Path("benchmarks/materialization/materialization-rust-baseline.json")
    python_path = Path("benchmarks/materialization/materialization-python-baseline.json")
    artifact = json.loads(
        (root / "benchmarks/materialization/materialization-benchmark-results.json").read_text(encoding="utf-8")
    )
    manifest = json.loads((root / manifest_path).read_text(encoding="utf-8"))
    manifest[f"{language}_baseline_path"] = f"benchmarks/materialization/tampered-{language}.json"

    with pytest.raises(ValueError, match=rf"{language} baseline path mismatch"):
        _DOCS_MODULE.verify_baseline_links(
            artifact,
            manifest,
            manifest_path,
            rust_path,
            python_path,
        )


def _sealable_rust_baseline(name: str = "materialization-v1", digest: str = "a" * 64) -> dict[str, object]:
    return {
        "schema": _MODULE.SCHEMA,
        "baseline_name": name,
        "tree_revision": "tree-current",
        "fixture_sha256": digest,
        "fixtures": {"a": "b" * 64, "b": "c" * 64},
        "expected_cases": list(_MODULE.EXPECTED_CASES),
        "medians_ms": {case: {"point": 10.0, "lower": 9.0, "upper": 11.0} for case in _MODULE.EXPECTED_CASES},
    }


@pytest.mark.parametrize(
    ("name", "digest"),
    [
        ("../escape", "a" * 64),
        ("materialization-v1", "not-a-digest"),
    ],
)
def test_rust_seal_rejects_malformed_identity_without_rewriting_the_manifest(
    tmp_path: Path, name: str, digest: str
) -> None:
    """A bad baseline name or digest is rejected before the manifest is rewritten."""
    baseline = tmp_path / "baseline.json"
    baseline.write_text(json.dumps(_sealable_rust_baseline(name, digest)), encoding="utf-8")
    manifest = tmp_path / "manifest.json"
    original = {
        "provenance": "Captured from base revision.",
        "rust_baseline_path": "benchmarks/materialization/materialization-rust-baseline.json",
    }
    manifest.write_text(json.dumps(original), encoding="utf-8")
    before = manifest.read_bytes()

    with pytest.raises(ValueError, match=r"baseline_name|fixture_sha256"):
        _MODULE.seal_baseline_manifest(baseline, manifest)

    assert manifest.read_bytes() == before


def test_rust_seal_preserves_existing_manifest_keys(tmp_path: Path) -> None:
    """Sealing updates baseline identity and leaves provenance and path fields in place."""
    baseline = tmp_path / "baseline.json"
    baseline.write_text(json.dumps(_sealable_rust_baseline()), encoding="utf-8")
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps({
            "provenance": "Captured from base revision.",
            "rust_baseline_path": "benchmarks/materialization/materialization-rust-baseline.json",
        }),
        encoding="utf-8",
    )

    _MODULE.seal_baseline_manifest(baseline, manifest)

    sealed = json.loads(manifest.read_text(encoding="utf-8"))
    assert sealed["provenance"] == "Captured from base revision."
    assert sealed["rust_baseline_path"] == "benchmarks/materialization/materialization-rust-baseline.json"
    assert sealed["rust_baseline_name"] == "materialization-v1"
    assert sealed["rust_baseline_tree_revision"] == "tree-current"
    assert sealed["rust_baseline_fixture_sha256"] == "a" * 64
