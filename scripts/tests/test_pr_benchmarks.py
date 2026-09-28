"""Check that the PR gate cannot silently skip a selected benchmark."""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from scripts.pr_benchmarks import CASES, regressions, run_cases


def _write_change(root: Path, benchmark: str, change: float, modified_ns: int) -> None:
    path = root / benchmark / "change" / "estimates.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"median": {"point_estimate": change}}))
    os.utime(path, ns=(modified_ns, modified_ns))


def test_pr_comparison_requires_every_fresh_case_and_flags_regression(tmp_path: Path) -> None:
    """All four selected cases must run on the head after the start time."""
    for _, _, benchmark in CASES:
        _write_change(tmp_path, benchmark, 0.0, 20)

    assert regressions(tmp_path, 10) == []

    regressed = CASES[-1][2]
    _write_change(tmp_path, regressed, 0.11, 20)
    assert regressions(tmp_path, 10) == [(regressed, 0.11)]

    (tmp_path / regressed / "change" / "estimates.json").unlink()
    with pytest.raises(ValueError, match="missing"):
        regressions(tmp_path, 10)


def test_pr_comparison_rejects_stale_and_unexpected_rows(tmp_path: Path) -> None:
    """Cached output and an unexpectedly broad Criterion filter cannot pass."""
    for _, _, benchmark in CASES:
        _write_change(tmp_path, benchmark, 0.0, 20)

    _write_change(tmp_path, CASES[0][2], 0.0, 5)
    with pytest.raises(ValueError, match="stale"):
        regressions(tmp_path, 10)

    _write_change(tmp_path, CASES[0][2], 0.0, 20)
    _write_change(tmp_path, "unselected_case", 0.0, 20)
    with pytest.raises(ValueError, match="extra"):
        regressions(tmp_path, 10)

    _write_change(tmp_path, "unselected_case", 0.0, 5)
    with pytest.raises(ValueError, match="extra"):
        regressions(tmp_path, 10)


def test_pr_comparison_rejects_nonfinite_median(tmp_path: Path) -> None:
    """A malformed Criterion change cannot pass the numeric threshold."""
    for _, _, benchmark in CASES:
        _write_change(tmp_path, benchmark, 0.0, 20)
    _write_change(tmp_path, CASES[0][2], float("nan"), 20)
    with pytest.raises(ValueError, match="invalid Criterion median"):
        regressions(tmp_path, 10)


def test_pr_cases_use_the_same_fast_link_profile(monkeypatch: pytest.MonkeyPatch) -> None:
    """Both baseline and comparison must override the checkout's Cargo profile."""
    calls: list[dict[str, object]] = []
    monkeypatch.setattr("scripts.pr_benchmarks.shutil.which", lambda _: "/usr/bin/cargo")
    monkeypatch.setattr("scripts.pr_benchmarks.subprocess.run", lambda *_, **kwargs: calls.append(kwargs))

    run_cases("baseline")
    run_cases("compare")

    assert len(calls) == 2 * len(CASES)
    assert all(call["env"]["CARGO_PROFILE_BENCH_LTO"] == "thin" for call in calls)
    assert all(call["env"]["CARGO_PROFILE_BENCH_CODEGEN_UNITS"] == "16" for call in calls)
