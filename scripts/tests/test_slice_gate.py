"""`mise run slice-gate` selects stages of `mise run all` from a diff.

The selector must never skip a stage a change can break, and its stage table
must stay equal to `all`, so the two gates cannot drift apart.
"""

from __future__ import annotations

from pathlib import Path
import tomllib

import pytest

from scripts import slice_gate

ROOT = Path(__file__).resolve().parents[2]
TASKS = tomllib.loads((ROOT / "mise.toml").read_text(encoding="utf-8"))["tasks"]
ALL = tuple(slice_gate.STAGES)
RUST_LIBRARY = ["lint", "rust-test", "gen-check", "python-test", "wasm-test", "ui-check"]


def _all_stage_tasks() -> set[str]:
    """Task names `mise run all` runs as `{ task = ... }` / `{ tasks = [...] }` steps."""
    names: set[str] = set()
    for step in TASKS["all"]["run"]:
        if isinstance(step, dict):
            names |= {step["task"]} if "task" in step else set(step["tasks"])
    return names


def test_stages_are_exactly_the_stages_of_all() -> None:
    """Every check stage of `all` is a slice-gate stage, and nothing else is."""
    # rust-build-tests only compiles what rust-test and gen-check then run.
    assert set(slice_gate.STAGES.values()) == _all_stage_tasks() - {"rust-build-tests"}


def test_every_stage_task_exists() -> None:
    """The tasks the runner invokes are defined in mise.toml."""
    for task in (*slice_gate.STAGES.values(), *slice_gate.WASM_PACKAGES, "slice-gate"):
        assert task in TASKS


@pytest.mark.parametrize(
    ("path", "expected"),
    [
        ("finstack-quant/core/src/money/rounding.rs", RUST_LIBRARY),
        ("finstack-quant/valuations/rates/src/swaption/pricer.rs", RUST_LIBRARY),
        ("finstack-quant/core/Cargo.toml", RUST_LIBRARY),
        ("finstack-quant/core/data/calendars/asx.json", RUST_LIBRARY),
        # A JSON fixture under tests/ can be a generated artifact other hosts read.
        ("finstack-quant/valuations/tests/instruments/json_examples/bond.json", RUST_LIBRARY),
        ("finstack-quant/valuations/tests/golden/theta.rs", ["lint", "rust-test"]),
        ("finstack-quant/calibration/benches/calibration.rs", ["lint", "rust-test"]),
        ("finstack-quant-py/src/bindings/core/types.rs", ["lint", "python-test"]),
        ("finstack-quant-py/finstack_quant/core/__init__.pyi", ["lint", "python-test"]),
        ("finstack-quant-py/parity_contract.toml", ["lint", "python-test"]),
        ("finstack-quant-wasm/exports/core.js", ["lint", "gen-check", "python-test", "wasm-test", "ui-check"]),
        (
            "finstack-quant-wasm/src/api/core/types.rs",
            ["lint", "rust-test", "gen-check", "python-test", "wasm-test", "ui-check"],
        ),
        ("finstack-quant-ui/src/components/Bond.tsx", ["lint", "ui-check"]),
        ("docs-site/content/docs/index.mdx", ["lint", "ui-check"]),
        ("scripts/check_loc.py", ["lint", "gen-check", "python-test"]),
        ("docs/decisions/financial/README.md", ["lint"]),
        ("INVARIANTS.md", ["lint"]),
        (".agents/rules/selective-test-running.mdc", ["lint"]),
    ],
)
def test_one_path_selects_its_stages(path: str, expected: list[str]) -> None:
    """Each area of the repository selects the stages that read it."""
    assert list(slice_gate.select([path])) == expected


@pytest.mark.parametrize(
    "path",
    [
        "mise.toml",
        "Cargo.lock",
        "uv.lock",
        "pyproject.toml",
        ".github/workflows/checks.yml",
        ".config/nextest.toml",
        "finstack-quant-ui/package-lock.json",
        "scripts/slice_gate.py",
        "some-new-directory/file.txt",
        "unknown.cfg",
    ],
)
def test_build_inputs_and_unknown_paths_select_everything(path: str) -> None:
    """A build input, or a path no rule names, runs every stage."""
    assert tuple(slice_gate.select([path])) == ALL


def test_generated_artifact_adds_gen_check() -> None:
    """A changed generated file is checked against its generator."""
    path = "finstack-quant-ui/registry.json"
    assert list(slice_gate.select([path])) == ["lint", "ui-check"]
    assert list(slice_gate.select([path], frozenset({path}))) == ["lint", "gen-check", "ui-check"]


def test_selection_is_the_union_in_run_order_with_reasons() -> None:
    """Several paths select the union of their stages, each with its first cause."""
    selected = slice_gate.select(["finstack-quant-ui/src/a.tsx", "finstack-quant-py/tests/test_a.py"])
    assert selected == {
        "lint": "finstack-quant-ui/src/a.tsx",
        "python-test": "finstack-quant-py/tests/test_a.py",
        "ui-check": "finstack-quant-ui/src/a.tsx",
    }
    assert list(selected) == ["lint", "python-test", "ui-check"]
    assert slice_gate.select([]) == {}


def test_ui_check_builds_the_wasm_packages_unless_wasm_test_does() -> None:
    """ui-check never runs against missing or stale WASM packages."""
    alone = slice_gate.commands_for("ui-check", {"lint": "", "ui-check": ""}, [])
    assert alone == [["mise", "run", "wasm-build"], ["mise", "run", "wasm-build-node"], ["mise", "run", "ui-check"]]
    after_wasm = slice_gate.commands_for("ui-check", {"wasm-test": "", "ui-check": ""}, [])
    assert after_wasm == [["mise", "run", "ui-check"]]


def test_lint_runs_the_hooks_on_changed_files_or_everywhere_after_a_delete() -> None:
    """Hooks see the changed files; a deletion falls back to the full Lint stage."""
    existing = ["mise.toml", "AGENTS.md"]
    assert slice_gate.lint_command(existing)[-3:] == ["--files", *existing]
    assert slice_gate.lint_command([*existing, "no/such/file.rs"]) == ["mise", "run", "pre-commit-run"]


def test_the_gate_never_regenerates() -> None:
    """Like `all`, the slice gate only checks generated files."""
    commands = [slice_gate.commands_for(stage, dict.fromkeys(ALL, ""), []) for stage in ALL]
    assert "gen-write" not in str(commands)
