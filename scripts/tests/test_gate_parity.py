"""`mise run all` and CI run the same checks.

`all` is the one pre-PR gate. `checks.yml` runs the same stages split across
runners, and `full-checks.yml` the `--full` tier. These tests fail when either
side gains or loses a stage, so the two cannot drift apart. `mise run slice-gate`
runs a diff-selected subset of the same stages (`test_slice_gate.py`).
"""

from __future__ import annotations

from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

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


def _stage_tasks(run: list[object]) -> list[str]:
    """Task names referenced by `{ task = ... }` and `{ tasks = [...] }` steps."""
    names: list[str] = []
    for step in run:
        if isinstance(step, dict):
            if "task" in step:
                names.append(step["task"])
            names.extend(step.get("tasks", []))
    return names


def _tier_script() -> str:
    """The shell step of `all` that runs the `--full` and `--release` tiers."""
    scripts = [step for step in TASKS["all"]["run"] if isinstance(step, str)]
    assert len(scripts) == 1, "`all` must keep its --full/--release tiers in one shell step"
    return scripts[0]


def _workflow_runs(name: str) -> list[str]:
    """Every `mise run <task> [--flags]` a workflow executes (comment lines excluded)."""
    lines = (WORKFLOWS / name).read_text(encoding="utf-8").splitlines()
    return MISE_RUN.findall("\n".join(line for line in lines if not line.lstrip().startswith("#")))


def test_all_runs_exactly_the_checks_workflow() -> None:
    """Each `checks.yml` job is one stage of `mise run all`, and vice versa."""
    local = set(_stage_tasks(TASKS["all"]["run"])) - LOCAL_BUILD
    ci = {run.split()[0] for run in _workflow_runs("checks.yml")} - RUNNER_SETUP
    assert local == ci, f"only in `all`: {sorted(local - ci)}; only in checks.yml: {sorted(ci - local)}"


def test_full_tier_runs_the_full_checks_workflow() -> None:
    """Every `full-checks.yml` step except coverage instrumentation runs under `all --full`."""
    local = set(MISE_RUN.findall(_tier_script()))
    ci = {run for run in _workflow_runs("full-checks.yml") if "--coverage" not in run}
    assert ci <= local, f"full-checks.yml runs {sorted(ci - local)}, which `all --full` does not"


def test_all_does_not_regenerate() -> None:
    """A check gate must not rewrite generated files first: `gen-write` stays out of `all`."""
    assert "depends" not in TASKS["all"]
    assert "gen-write" not in _stage_tasks(TASKS["all"]["run"])
    assert "gen-write" not in _tier_script()


def test_retired_gates_are_gone() -> None:
    """Gates replaced by `all` do not come back under their old names."""
    for name in ("all-ci", "all-lint", "phase-gate", "gen-check-ui", "python-doc"):
        assert name not in TASKS, f"`{name}` was folded into `mise run all`; do not reintroduce it"


def test_checks_workflow_skips_no_hooks() -> None:
    """CI's Lint job runs every pre-commit hook, as `mise run all` does locally."""
    assert "SKIP:" not in (WORKFLOWS / "checks.yml").read_text(encoding="utf-8")


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
