"""Run the stages of `mise run all` that a diff can affect, cheapest first.

The per-slice gate. It selects stages from the changed paths, runs each as the
same `mise` task `all` runs, stops at the first failure and prints how long
each stage took. It never regenerates: when `gen-check` fails, run
`mise run gen-write` and resume. `mise run all` stays the pre-PR gate.

A path no rule recognises selects every stage, so a new directory is checked
in full until a rule names it.

Usage: slice_gate.py [--base REF] [--from STAGE] [--dry-run] [--paths PATH...]
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
LOG_DIR = ROOT / "target" / "slice-gate"
GENERATED_ARTIFACTS = ROOT / "scripts" / "generated-artifacts.txt"
GIT = "/usr/bin/git"

# Run order: the stages most likely to fail on a code change come first. Each
# value is the stage of `mise run all` it stands for; `lint` runs that stage's
# hooks on the changed files only (scripts/tests/test_slice_gate.py keeps this
# table equal to `all`).
STAGES = {
    "lint": "pre-commit-run",
    "rust-test": "rust-test",
    "gen-check": "gen-check",
    "python-test": "python-test",
    "wasm-test": "wasm-test",
    "ui-check": "ui-check",
    "all-audit": "all-audit",
}
# ui-check imports the WASM packages; wasm-test rebuilds them, and without it
# these two tasks do (a few seconds when the packages are current).
WASM_PACKAGES = ("wasm-build", "wasm-build-node")

RUST_LIBRARY = ("lint", "rust-test", "gen-check", "python-test", "wasm-test", "ui-check")
RUST_TESTS_ONLY = ("lint", "rust-test")
RUST_TEST_DIRS = {"tests", "benches", "examples"}

# Build, toolchain, dependency and CI inputs: any stage can change with them.
EVERYTHING = (
    "mise.toml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "pyproject.toml",
    "uv.lock",
    ".pre-commit-config.yaml",
    ".cargo/",
    ".config/",
    ".github/",
    "scripts/slice_gate.py",
)
# Prose and assistant configuration: only the lint hooks read them.
LINT_ONLY = ("docs/", ".agents/", ".claude/", ".cursor/", ".kilo/", "LICENSE", ".gitignore")
# First match wins; the Rust workspace rule is in `stages_for`.
PREFIX_RULES = (
    ("finstack-quant-py/", ("lint", "python-test")),
    # The Python parity ledger and schema golden read the WASM facade surface.
    ("finstack-quant-wasm/", ("lint", "gen-check", "python-test", "wasm-test", "ui-check")),
    ("finstack-quant-ui/", ("lint", "ui-check")),
    ("docs-site/", ("lint", "ui-check")),
    # scripts/tests runs in the Python suite; the generators live here too.
    ("scripts/", ("lint", "gen-check", "python-test")),
)


def stages_for(path: str) -> tuple[str, ...]:
    """Return the stages one changed path selects.

    Args:
        path: Repository-relative path with forward slashes.

    Returns:
        Stage names from `STAGES`; every stage when no rule recognises the path.
    """
    if path.startswith(EVERYTHING) or path.endswith("package-lock.json"):
        return tuple(STAGES)
    if path.startswith("finstack-quant/"):
        # A test, bench or example source cannot change a binding or a schema.
        tests_only = path.endswith(".rs") and not RUST_TEST_DIRS.isdisjoint(path.split("/")[:-1])
        return RUST_TESTS_ONLY if tests_only else RUST_LIBRARY
    for prefix, stages in PREFIX_RULES:
        if path.startswith(prefix):
            # The .rs sources of the WASM binding are a workspace crate rust-test runs.
            if prefix == "finstack-quant-wasm/" and path.endswith(".rs"):
                return (*stages, "rust-test")
            return stages
    if path.startswith(LINT_ONLY) or ("/" not in path and path.endswith(".md")):
        return ("lint",)
    return tuple(STAGES)


def select(paths: list[str], generated: frozenset[str] = frozenset()) -> dict[str, str]:
    """Map changed paths to the stages to run.

    Args:
        paths: Repository-relative changed paths.
        generated: Checked-in generated files (`scripts/generated-artifacts.txt`);
            a change to one also selects `gen-check`.

    Returns:
        Selected stages in run order, each with the first path that selected it.
    """
    reasons: dict[str, str] = {}
    for path in paths:
        stages = stages_for(path)
        if path in generated:
            stages = (*stages, "gen-check")
        for stage in stages:
            reasons.setdefault(stage, path)
    return {stage: reasons[stage] for stage in STAGES if stage in reasons}


def changed_paths(base: str) -> list[str]:
    """List the paths that differ from `base`: committed, staged, unstaged and untracked.

    Args:
        base: Git ref to diff against; `HEAD` selects only uncommitted work.

    Returns:
        Sorted repository-relative paths, including deleted files.
    """

    def git(*args: str) -> list[str]:
        result = subprocess.run([GIT, "-C", str(ROOT), *args], capture_output=True, text=True, check=True)  # noqa: S603
        return [line for line in result.stdout.splitlines() if line]

    return sorted({
        *git("diff", "--name-only", base),
        *git("ls-files", "--others", "--exclude-standard"),
    })


def lint_command(paths: list[str]) -> list[str]:
    """Return the command for the lint stage.

    Args:
        paths: Changed paths; the hooks run on those that still exist.

    Returns:
        pre-commit on the changed files, or the full `pre-commit-run` task when
        a file was deleted, since no remaining file would trigger its hooks.
    """
    existing = [path for path in paths if (ROOT / path).exists()]
    if len(existing) < len(paths):
        return ["mise", "run", "pre-commit-run"]
    return ["uv", "run", "--no-sync", "pre-commit", "run", "--files", *existing]


def commands_for(stage: str, selected: dict[str, str], paths: list[str]) -> list[list[str]]:
    """Return the commands one stage runs, in order.

    Args:
        stage: Stage name from `STAGES`.
        selected: Every selected stage, to tell whether wasm-test builds the packages.
        paths: Changed paths, for the lint stage.

    Returns:
        Argument lists to run from the repository root.
    """
    if stage == "lint":
        return [lint_command(paths)]
    commands = [["mise", "run", STAGES[stage]]]
    if stage == "ui-check" and "wasm-test" not in selected:
        commands = [*(["mise", "run", task] for task in WASM_PACKAGES), *commands]
    return commands


def run_stage(stage: str, commands: list[list[str]], env: dict[str, str]) -> tuple[bool, float]:
    """Run one stage, logging its output to `target/slice-gate/<stage>.log`.

    Args:
        stage: Stage name, used for the log file.
        commands: Argument lists to run in order; the first failure stops the stage.
        env: Environment for the commands.

    Returns:
        Whether every command exited 0, and the elapsed seconds.
    """
    start = time.monotonic()
    with (LOG_DIR / f"{stage}.log").open("w", encoding="utf-8") as log:
        for command in commands:
            log.write(f"$ {' '.join(command[:8])}\n")
            log.flush()
            code = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=False)  # noqa: S603
            if code.returncode != 0:
                return False, time.monotonic() - start
    return True, time.monotonic() - start


def main() -> int:
    """Select the stages for the diff and run them until one fails."""
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", default="HEAD", help="Diff base ref (default: HEAD, the uncommitted slice)")
    parser.add_argument("--from", dest="start", choices=tuple(STAGES), help="Skip the stages before this one")
    parser.add_argument("--dry-run", action="store_true", help="Print the selection without running it")
    parser.add_argument("--paths", nargs="*", help="Use these paths instead of the git diff")
    args = parser.parse_args()

    paths = args.paths if args.paths is not None else changed_paths(args.base)
    generated = frozenset(GENERATED_ARTIFACTS.read_text(encoding="utf-8").split())
    selected = select(paths, generated)
    print(f"slice-gate: {len(paths)} changed paths against {args.base}")
    for stage in STAGES:
        print(f"  {stage:<12} " + (f"run   ({selected[stage]})" if stage in selected else "skip"))
    if args.dry_run or not selected:
        return 0

    todo = list(selected)
    if args.start:
        todo = todo[todo.index(args.start) :] if args.start in todo else []
    # A worktree under the main checkout would otherwise also load its mise.toml.
    env = {"MISE_CEILING_PATHS": str(ROOT.parent), **os.environ}
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    total = 0.0
    for stage in todo:
        passed, seconds = run_stage(stage, commands_for(stage, selected, paths), env)
        total += seconds
        print(f"  {stage:<12} {'ok' if passed else 'FAILED':<6} {seconds:6.1f}s", flush=True)
        if not passed:
            log = LOG_DIR / f"{stage}.log"
            print("\n".join(log.read_text(encoding="utf-8", errors="replace").splitlines()[-40:]))
            print(f"\nslice-gate: {stage} failed after {total:.0f}s; full log: {log.relative_to(ROOT)}")
            if stage == "gen-check":
                print("If a generated file is stale, run `mise run gen-write` first.")
            print(f"Resume with: mise run slice-gate --from {stage}")
            return 1
    print(f"slice-gate: {len(todo)} stages passed in {total:.0f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
