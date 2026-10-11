"""Tests for the Criterion median-regression review gate."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "check_criterion_regressions.py"
sys.path.insert(0, str(_SCRIPT_PATH.parent))
_SPEC = importlib.util.spec_from_file_location("check_criterion_regressions", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)
collect_regressions = _MODULE.collect_regressions
load_run_metadata = _MODULE.load_run_metadata
MATERIALIZATION_EXPECTED_BENCHMARKS = _MODULE.MATERIALIZATION_EXPECTED_BENCHMARKS


def _write_change(root: Path, benchmark: str, estimate: float) -> None:
    destination = root / benchmark / "change"
    destination.mkdir(parents=True)
    (destination / "estimates.json").write_text(
        json.dumps({"median": {"point_estimate": estimate}}),
        encoding="utf-8",
    )


def _write_current(root: Path, case: str, estimate_ns: float) -> None:
    destination = root / "portfolio_materialization" / case / "new"
    destination.mkdir(parents=True)
    (destination / "estimates.json").write_text(
        json.dumps({
            "median": {
                "point_estimate": estimate_ns,
                "confidence_interval": {
                    "lower_bound": estimate_ns * 0.99,
                    "upper_bound": estimate_ns * 1.01,
                },
            }
        }),
        encoding="utf-8",
    )


def test_materialization_comparison_uses_checked_in_compact_baseline(tmp_path: Path) -> None:
    """Fresh Criterion outputs compare directly with durable baseline medians."""
    baseline = {
        "schema": "finstack_quant.materialization_rust_baseline/1",
        "baseline_name": "materialization-v1",
        "expected_cases": list(_MODULE.MATERIALIZATION_CASES),
        "medians_ms": {case: {"point": 10.0, "lower": 9.0, "upper": 11.0} for case in _MODULE.MATERIALIZATION_CASES},
    }
    for case in _MODULE.MATERIALIZATION_CASES:
        _write_current(tmp_path, case, 11_001_000.0 if case == _MODULE.MATERIALIZATION_CASES[-1] else 11_000_000.0)

    comparison = _MODULE.materialization_comparison(tmp_path, baseline)

    assert comparison[_MODULE.MATERIALIZATION_CASES[0]]["relative_median_change"]["point"] == pytest.approx(0.10)
    assert _MODULE.materialization_regressions(comparison, 0.10) == [
        (_MODULE.MATERIALIZATION_CASES[-1], pytest.approx(0.1001))
    ]


def test_materialization_comparison_rejects_stale_or_extra_current_outputs(tmp_path: Path) -> None:
    """Strict comparison accepts only four fresh root-target Criterion outputs."""
    baseline = {
        "schema": "finstack_quant.materialization_rust_baseline/1",
        "baseline_name": "materialization-v1",
        "expected_cases": list(_MODULE.MATERIALIZATION_CASES),
        "medians_ms": {case: {"point": 10.0, "lower": 9.0, "upper": 11.0} for case in _MODULE.MATERIALIZATION_CASES},
    }
    for case in _MODULE.MATERIALIZATION_CASES:
        _write_current(tmp_path, case, 10_000_000.0)
    extra = tmp_path / "portfolio_materialization" / "obsolete" / "new"
    extra.mkdir(parents=True)
    (extra / "estimates.json").write_text("{}", encoding="utf-8")

    with pytest.raises(ValueError, match=r"unexpected=.*obsolete"):
        _MODULE.materialization_comparison(tmp_path, baseline)

    (extra / "estimates.json").unlink()
    os.utime(
        tmp_path / "portfolio_materialization" / _MODULE.MATERIALIZATION_CASES[0] / "new" / "estimates.json",
        ns=(1, 1),
    )
    with pytest.raises(ValueError, match="stale"):
        _MODULE.materialization_comparison(tmp_path, baseline, started_at_ns=2)


def test_collect_regressions_flags_only_median_changes_above_threshold(tmp_path: Path) -> None:
    """Changes equal to the threshold pass; larger medians fail."""
    paths = ("faster", "boundary", "regressed")
    _write_change(tmp_path, paths[0], -0.25)
    _write_change(tmp_path, paths[1], 0.10)
    _write_change(tmp_path, paths[2], 0.1001)

    regressions = collect_regressions(
        tmp_path,
        threshold=0.10,
        expected_paths=tuple(f"{path}/change/estimates.json" for path in paths),
    )

    assert regressions == [("regressed", 0.1001)]


def test_generic_checker_discovers_all_available_comparisons(tmp_path: Path) -> None:
    """The generic rust-bench task needs no materialization arguments."""
    _write_change(tmp_path, "crate_a/bench_one", 0.0)
    _write_change(tmp_path, "crate_b/bench_two", 0.11)

    assert collect_regressions(tmp_path, threshold=0.10) == [("crate_b/bench_two", 0.11)]


def test_materialization_checker_requires_its_exact_case_set(tmp_path: Path) -> None:
    """The materialization task rejects incomplete or contaminated output."""
    for relative_path in MATERIALIZATION_EXPECTED_BENCHMARKS[:-1]:
        benchmark = relative_path.removesuffix("/change/estimates.json")
        _write_change(tmp_path, benchmark, 0.0)

    with pytest.raises(ValueError, match="missing"):
        collect_regressions(
            tmp_path,
            threshold=0.10,
            expected_paths=MATERIALIZATION_EXPECTED_BENCHMARKS,
        )


def test_collect_regressions_rejects_missing_and_unexpected_artifacts(tmp_path: Path) -> None:
    """Only the exact benchmark path set is accepted."""
    _write_change(tmp_path, "unexpected", 0.0)
    with pytest.raises(ValueError, match=r"missing=.*expected.*unexpected=.*unexpected"):
        collect_regressions(
            tmp_path,
            threshold=0.10,
            expected_paths=("expected/change/estimates.json",),
        )


def test_collect_regressions_rejects_stale_artifacts(tmp_path: Path) -> None:
    """An estimate older than this run cannot satisfy the gate."""
    _write_change(tmp_path, "case", 0.0)
    estimate = tmp_path / "case" / "change" / "estimates.json"
    os.utime(estimate, ns=(1, 1))
    with pytest.raises(ValueError, match="stale"):
        collect_regressions(
            tmp_path,
            threshold=0.10,
            expected_paths=("case/change/estimates.json",),
            started_at_ns=2,
        )


def test_run_metadata_requires_revisions_and_fixture_digest(tmp_path: Path) -> None:
    """Gate provenance records baseline/current identities and fixture bytes."""
    path = tmp_path / "run.json"
    path.write_text(
        json.dumps({
            "baseline_name": "materialization-2026-07-29",
            "baseline_revision": "tree:abc",
            "current_revision": "tree:def",
            "fixture_sha256": "a" * 64,
            "started_at_ns": 123,
        }),
        encoding="utf-8",
    )
    assert load_run_metadata(path)["fixture_sha256"] == "a" * 64


@pytest.mark.parametrize(
    ("baseline_name", "digest"),
    [
        ("../escape", "a" * 64),
        ("materialization-v1", "not-a-digest"),
    ],
)
def test_malformed_baseline_identity_is_rejected_before_comparison_write(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    baseline_name: str,
    digest: str,
) -> None:
    """A bad baseline name or digest never creates the comparison artifact."""
    baseline = tmp_path / "baseline.json"
    baseline.write_text(
        json.dumps({
            "baseline_name": baseline_name,
            "tree_revision": "tree-current",
            "fixture_sha256": digest,
        }),
        encoding="utf-8",
    )
    output = tmp_path / "comparison.json"
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "check_criterion_regressions.py",
            "--baseline",
            str(baseline),
            "--comparison-output",
            str(output),
        ],
    )

    with pytest.raises(SystemExit) as caught:
        _MODULE.main()

    assert caught.value.code == 2
    assert not output.exists()
