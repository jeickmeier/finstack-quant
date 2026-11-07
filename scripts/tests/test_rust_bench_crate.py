"""Check scoped Criterion task arguments without building or measuring Rust."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tomllib

import pytest

ROOT = Path(__file__).resolve().parents[2]
TASK = tomllib.loads((ROOT / "mise.toml").read_text(encoding="utf-8"))["tasks"]["rust-bench-crate"]
CARGO_PREFIX = ["bench", "-p", "finstack-quant-valuations", "--bench", "mc_exotics_pricing", "--"]
DEFAULT_TIMING = [
    "--sample-size",
    "10",
    "--warm-up-time",
    "1",
    "--measurement-time",
    "1",
    "--nresamples",
    "1000",
    "--noplot",
]


def _run_task(
    tmp_path: Path, overrides: dict[str, str] | None = None
) -> tuple[subprocess.CompletedProcess[str], list[str] | None]:
    """Record task arguments and model Cargo appending --bench to the binary arguments."""
    cargo = tmp_path / "cargo"
    cargo.write_text(
        "#!/bin/sh\n"
        'printf "%s\\0" "$@" > "$FQ_TEST_CARGO_ARGS"\n'
        'while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do shift; done\n'
        'if [ "$#" -gt 0 ]; then shift; fi\n'
        'printf "%s\\0" "$@" --bench > "$FQ_TEST_CRITERION_ARGS"\n'
        'exit "${FQ_TEST_CARGO_EXIT:-0}"\n',
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    argument_file = tmp_path / "cargo-args"
    env = {key: value for key, value in os.environ.items() if not key.startswith(("usage_", "FQ_BENCH_"))}
    env.update({
        "PATH": f"{tmp_path}{os.pathsep}{env.get('PATH', '')}",
        "FQ_TEST_CARGO_ARGS": str(argument_file),
        "FQ_TEST_CRITERION_ARGS": str(tmp_path / "criterion-args"),
        "FQ_TEST_CARGO_EXIT": "0",
        "usage_package": "finstack-quant-valuations",
        "usage_bench": "mc_exotics_pricing",
    })
    env.update(overrides or {})
    shell = shutil.which("sh")
    assert shell, "a POSIX shell is required"
    result = subprocess.run(  # noqa: S603 -- reviewed task source; user values enter only through the environment.
        [shell, "-eu", "-c", TASK["run"]],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    arguments = argument_file.read_bytes().decode().split("\0")[:-1] if argument_file.exists() else None
    return result, arguments


def test_default_scoped_benchmark_preserves_timing_defaults(tmp_path: Path) -> None:
    """The unflagged task remains the same scoped timing run."""
    result, arguments = _run_task(tmp_path)
    assert result.returncode == 0, result.stderr
    assert arguments == CARGO_PREFIX + DEFAULT_TIMING


def test_timing_environment_overrides_are_forwarded(tmp_path: Path) -> None:
    """The existing FQ_BENCH controls still set all four Criterion defaults."""
    result, arguments = _run_task(
        tmp_path,
        {
            "FQ_BENCH_SAMPLE_SIZE": "50",
            "FQ_BENCH_WARM_UP_TIME": "3",
            "FQ_BENCH_MEASUREMENT_TIME": "5",
            "FQ_BENCH_NRESAMPLES": "10000",
        },
    )
    assert result.returncode == 0, result.stderr
    assert arguments == [
        *CARGO_PREFIX,
        "--sample-size",
        "50",
        "--warm-up-time",
        "3",
        "--measurement-time",
        "5",
        "--nresamples",
        "10000",
        "--noplot",
    ]


@pytest.mark.parametrize(
    ("case_filter", "forwarded_filter"),
    [
        (r"^(asian|lookback)_mc/(2500|10000)$", r"^(asian|lookback)_mc/(2500|10000)$"),
        ("name with spaces", "name with spaces"),
        ("--test", "(?:)--test"),
        ("-never|^scratch/valid$", "(?:)-never|^scratch/valid$"),
        ("(?i)^scratch/valid$", "(?i)^scratch/valid$"),
        ("(?x)^scratch/valid$ # trailing comment", "(?x)^scratch/valid$ # trailing comment"),
        ("-(?x)never # trailing comment", "(?:)-(?x)never # trailing comment"),
        ("*foo", "*foo"),
        ("$(touch injected); `touch injected` *", "$(touch injected); `touch injected` *"),
    ],
)
def test_filter_preserves_regex_and_cargo_benchmark_flag(
    tmp_path: Path, case_filter: str, forwarded_filter: str
) -> None:
    """Filters stay one safe positional while Cargo's appended --bench remains an option."""
    result, arguments = _run_task(tmp_path, {"usage_filter": case_filter})
    assert result.returncode == 0, result.stderr
    assert arguments == CARGO_PREFIX + DEFAULT_TIMING + [forwarded_filter]
    binary_arguments = (tmp_path / "criterion-args").read_bytes().decode().split("\0")[:-1]
    assert binary_arguments == [*DEFAULT_TIMING, forwarded_filter, "--bench"]
    assert "--" not in binary_arguments
    assert not forwarded_filter.startswith("-")
    assert not (tmp_path / "injected").exists()


def test_filtered_smoke_mode_is_explicit(tmp_path: Path) -> None:
    """Smoke forwards Criterion's --test and cannot silently become a timing run."""
    result, arguments = _run_task(tmp_path, {"usage_test": "true", "usage_filter": "asian_option_mc"})
    assert result.returncode == 0, result.stderr
    assert arguments == CARGO_PREFIX + DEFAULT_TIMING + ["--test", "asian_option_mc"]


@pytest.mark.parametrize("mode", ["save_baseline", "baseline"])
def test_named_baseline_mode_preserves_literal_name(tmp_path: Path, mode: str) -> None:
    """Each baseline mode keeps its name as one value even with shell metacharacters."""
    name = "-valuation b0 $(touch injected); *"
    result, arguments = _run_task(tmp_path, {f"usage_{mode}": name, "usage_filter": "asian_option_mc"})
    assert result.returncode == 0, result.stderr
    assert arguments == CARGO_PREFIX + DEFAULT_TIMING + [
        f"--{mode.replace('_', '-')}={name}",
        "asian_option_mc",
    ]
    assert not (tmp_path / "injected").exists()


@pytest.mark.parametrize(
    ("options", "message"),
    [
        ({"usage_save_baseline": "b0", "usage_baseline": "b0"}, "mutually exclusive"),
        ({"usage_test": "true", "usage_save_baseline": "b0"}, "--test cannot be combined"),
        ({"usage_test": "true", "usage_baseline": "b0"}, "--test cannot be combined"),
    ],
)
def test_incompatible_modes_fail_before_cargo(tmp_path: Path, options: dict[str, str], message: str) -> None:
    """Conflicting baseline operations or smoke/baseline combinations never start cargo."""
    result, arguments = _run_task(tmp_path, options)
    assert result.returncode == 2
    assert message in result.stderr
    assert arguments is None


def test_failed_benchmark_exit_status_is_preserved(tmp_path: Path) -> None:
    """A failing benchmark cannot be reported as a successful task."""
    result, _ = _run_task(tmp_path, {"FQ_TEST_CARGO_EXIT": "17"})
    assert result.returncode == 17
