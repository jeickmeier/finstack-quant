"""`mise run all` and CI run the same checks.

`all` is the one pre-PR gate. `checks.yml` runs the same stages split across
runners, and `full-checks.yml` the `--full` tier. These tests fail when either
side gains or loses a stage, so the two cannot drift apart. `mise run slice-gate`
runs a diff-selected subset of the same stages (`test_slice_gate.py`).
"""

from __future__ import annotations

import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

import pytest

from scripts import slice_gate

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"
TASKS = tomllib.loads((ROOT / "mise.toml").read_text(encoding="utf-8"))["tasks"]
# CI steps that prepare a runner (dependencies, the WASM packages ui-check
# imports) rather than check anything; locally `wasm-test` builds the packages.
RUNNER_SETUP = {"ui-sync", "wasm-build", "wasm-build-node"}
# Local build-order stages of `all`: they compile what later stages run, so
# compiling never overlaps ui-check; each CI job builds what it runs itself.
LOCAL_BUILD = {"rust-build-tests"}
MISE_RUN = re.compile(r"mise run ([a-z0-9-]+(?: --[a-z-]+)*)")


@pytest.mark.parametrize(("coverage", "slow"), [(False, False), (True, False), (True, True)])
def test_rust_coverage_selects_its_budget_without_changing_test_selection(
    tmp_path: Path, coverage: bool, slow: bool
) -> None:
    """Coverage overrides the fast timeout profile, not the ignored-test filter."""
    cargo = tmp_path / "cargo"
    cargo.write_text(
        '#!/bin/sh\nprintf "%s\\t%s\\n" "${NEXTEST_PROFILE:-default}" "$*" >> "$FQ_TEST_CARGO_CALLS"\n',
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    calls = tmp_path / "calls"
    env = {key: value for key, value in os.environ.items() if not key.startswith("usage_")}
    env.update({
        "PATH": f"{tmp_path}{os.pathsep}{env.get('PATH', '')}",
        "FQ_TEST_CARGO_CALLS": str(calls),
        "NEXTEST_PROFILE": "ci",
        "CI": "true",
        "usage_coverage": str(coverage).lower(),
        "usage_slow": str(slow).lower(),
    })
    shell = shutil.which("sh")
    assert shell, "a POSIX shell is required"
    result = subprocess.run(  # noqa: S603 -- fixed repository task, with a recording Cargo stub.
        [shell, "-eu", "-c", TASKS["rust-test"]["run"]],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    test_calls = [line.split("\t") for line in calls.read_text().splitlines() if "nextest" in line]
    assert len(test_calls) == 1
    profile, command = test_calls[0]
    assert profile == ("coverage" if coverage else "ci")
    assert command.startswith("llvm-cov nextest" if coverage else "nextest run")
    assert "--workspace --exclude finstack-quant-py --lib --test *" in command
    assert ("--run-ignored only" in command) == slow
    assert ("--profile slow" in command) == slow


def _record_pr_gate(
    tmp_path: Path, resume_from: str = "", failing_stage: str = ""
) -> tuple[list[list[str]], subprocess.CompletedProcess[str]]:
    """Execute the real gate orchestration with a recording mise stub."""
    mise = tmp_path / "mise"
    mise.write_text(
        '#!/bin/sh\nprintf "%s\\n" "$*" >> "$FQ_TEST_MISE_CALLS"\n[ "$2" != "$FQ_TEST_FAILING_STAGE" ]\n',
        encoding="utf-8",
    )
    mise.chmod(0o755)
    calls = tmp_path / "mise-calls"
    calls.touch()
    env = {key: value for key, value in os.environ.items() if not key.startswith("usage_")}
    env.update({
        "PATH": f"{tmp_path}{os.pathsep}{env.get('PATH', '')}",
        "FQ_TEST_MISE_CALLS": str(calls),
        "FQ_TEST_FAILING_STAGE": failing_stage,
        "usage_from": resume_from,
    })
    shell = shutil.which("sh")
    assert shell, "a POSIX shell is required"
    result = subprocess.run(  # noqa: S603 -- fixed repository task with a recording mise stub.
        [shell, "-c", TASKS["all"]["run"]],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    groups = [line.removeprefix("run ").split(" ::: ") for line in calls.read_text().splitlines()]
    return groups, result


def _workflow_runs(name: str) -> list[str]:
    """Every `mise run <task> [--flags]` a workflow executes (comment lines excluded)."""
    lines = (WORKFLOWS / name).read_text(encoding="utf-8").splitlines()
    return MISE_RUN.findall("\n".join(line for line in lines if not line.lstrip().startswith("#")))


def test_all_runs_exactly_the_checks_workflow(tmp_path: Path) -> None:
    """The full gate, CI and the slice gate have the same check stages."""
    groups, result = _record_pr_gate(tmp_path)
    assert result.returncode == 0, result.stderr
    local = {task for group in groups for task in group} - LOCAL_BUILD
    ci = {run.split()[0] for run in _workflow_runs("checks.yml")} - RUNNER_SETUP
    assert local == ci, f"only in `all`: {sorted(local - ci)}; only in checks.yml: {sorted(ci - local)}"
    assert local == set(slice_gate.STAGES.values())
    assert groups == [
        ["pre-commit-run"],
        ["wasm-test"],
        ["rust-build-tests"],
        ["gen-check", "ui-check", "all-audit"],
        ["rust-test"],
        ["python-test"],
    ]


@pytest.mark.parametrize(
    ("start", "expected"),
    [
        ("ui-check", [["ui-check", "all-audit"], ["rust-test"], ["python-test"]]),
        ("rust-test", [["rust-test"], ["python-test"]]),
        ("python-test", [["python-test"]]),
    ],
)
def test_all_resumes_at_the_selected_pr_stage(tmp_path: Path, start: str, expected: list[list[str]]) -> None:
    """Earlier stages are skipped, including earlier members of a parallel group."""
    groups, result = _record_pr_gate(tmp_path, resume_from=start)
    assert result.returncode == 0, result.stderr
    assert groups == expected


def test_all_rejects_an_unknown_stage(tmp_path: Path) -> None:
    """An invalid stage fails before invoking any checks."""
    groups, result = _record_pr_gate(tmp_path, resume_from="typo")
    assert result.returncode == 2
    assert "Unknown PR gate stage: typo" in result.stderr
    assert groups == []


def test_all_stops_on_stage_failure(tmp_path: Path) -> None:
    """A failing stage prevents later groups from starting."""
    groups, result = _record_pr_gate(tmp_path, failing_stage="wasm-test")
    assert result.returncode == 1
    assert groups == [["pre-commit-run"], ["wasm-test"]]


def test_full_tier_runs_the_full_checks_workflow() -> None:
    """Every `full-checks.yml` step except coverage instrumentation runs under `all --full`."""
    local = set(MISE_RUN.findall(TASKS["all"]["run"]))
    ci = {run for run in _workflow_runs("full-checks.yml") if "--coverage" not in run}
    assert ci <= local, f"full-checks.yml runs {sorted(ci - local)}, which `all --full` does not"


def test_all_does_not_regenerate() -> None:
    """A check gate must not rewrite generated files first: `gen-write` stays out of `all`."""
    assert "depends" not in TASKS["all"]
    assert "gen-write" not in TASKS["all"]["run"]


def test_retired_gates_are_gone() -> None:
    """Gates replaced by `all` do not come back under their old names."""
    for name in ("all-ci", "all-lint", "phase-gate", "gen-check-ui", "python-doc"):
        assert name not in TASKS, f"`{name}` was folded into `mise run all`; do not reintroduce it"


def test_checks_workflow_skips_no_hooks() -> None:
    """CI and the local workspace gate run all check-stage hooks."""
    assert "SKIP:" not in (WORKFLOWS / "checks.yml").read_text(encoding="utf-8")


@pytest.mark.parametrize(
    ("check", "invalid"),
    [
        ("trailing-whitespace", b"text \n"),
        ("end-of-file", b"text"),
        ("end-of-file", b"text\n\n"),
        ("lf", b"text\r\n"),
    ],
)
def test_text_format_checks_report_without_editing(tmp_path: Path, check: str, invalid: bytes) -> None:
    """Each text check reports invalid input and leaves the bytes intact."""
    clean = tmp_path / "clean.txt"
    clean.write_bytes(b"text\n")
    dirty = tmp_path / "dirty.txt"
    dirty.write_bytes(invalid)
    result = subprocess.run(  # noqa: S603 -- fixed local checker with test files.
        [sys.executable, str(ROOT / "scripts" / "check_text_format.py"), check, str(clean), str(dirty)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 1
    assert str(dirty) in result.stdout
    assert str(clean) not in result.stdout
    assert dirty.read_bytes() == invalid


def test_every_generated_root_is_marked_generated() -> None:
    """Each path `gen-check` digests carries `linguist-generated` in `.gitattributes`."""
    sys.path.insert(0, str(ROOT / "scripts"))
    from generation_digest import DEFAULT_PATHS

    probes = []
    for path in DEFAULT_PATHS:
        target = ROOT / path
        if target.is_dir():
            files = sorted(p for p in target.rglob("*") if p.is_file())
            assert files, f"{path} has no files"
            target = files[0]
        probes.append(str(target.relative_to(ROOT)))
    vcs = shutil.which("git")
    assert vcs, "git is required"
    out = subprocess.run(  # noqa: S603 -- fixed check-attr subcommand on repository paths; no shell.
        [vcs, "check-attr", "linguist-generated", "--", *probes],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    unmarked = [line.split(":")[0] for line in out.splitlines() if not line.endswith(": true")]
    assert unmarked == [], f"add a linguist-generated pattern to .gitattributes for: {unmarked}"
