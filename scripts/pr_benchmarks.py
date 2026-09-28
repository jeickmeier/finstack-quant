"""Run a small, exact PR benchmark set against the base commit on one host."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

from scripts.check_criterion_regressions import _median_change

CASES = (
    ("finstack-quant-core", "correlation_apply", "correlation_factor_apply/triangular/32"),
    ("finstack-quant-models", "mc_hot_paths", "european_pricer/paths/10000"),
    ("finstack-quant-valuations", "bond_pricing", "bond_pv/10Y"),
    ("finstack-quant-portfolio", "portfolio_valuation", "portfolio_valuation/250pos"),
)
BASELINE = "pr-base"
THRESHOLD = 0.10
BUILD_PROFILE = {"lto": "thin", "codegen_units": 16}
CRITERION_ROOT = Path("target/criterion")
MANIFEST = Path("target/pr-benchmarks.json")


def expected_paths(stage: str) -> set[str]:
    """Return the exact Criterion estimate paths for the selected cases."""
    return {f"{benchmark}/{stage}/estimates.json" for _, _, benchmark in CASES}


def require_exact_outputs(root: Path, stage: str, started_at_ns: int) -> None:
    """Reject skipped cases, extra cases, and stale selected outputs."""
    expected = expected_paths(stage)
    actual = {path.relative_to(root).as_posix() for path in root.glob(f"**/{stage}/estimates.json")}
    if actual != expected:
        raise ValueError(
            f"PR benchmark outputs differ: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )
    stale = sorted(relative for relative in expected if (root / relative).stat().st_mtime_ns < started_at_ns)
    if stale:
        raise ValueError(f"stale PR benchmark outputs: {stale}")


def regressions(root: Path, started_at_ns: int) -> list[tuple[str, float]]:
    """Validate fresh comparisons and return median increases above 10%."""
    require_exact_outputs(root, "change", started_at_ns)
    found = []
    for _, _, benchmark in CASES:
        change = _median_change(root / benchmark / "change" / "estimates.json")
        if not math.isfinite(change):
            raise ValueError(f"invalid Criterion median change for {benchmark}: {change}")
        if change > THRESHOLD:
            found.append((benchmark, change))
    return found


def run_cases(mode: str) -> int:
    """Run the fixed benchmark set and return the start timestamp."""
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required for PR benchmarks")
    started_at_ns = time.time_ns()
    criterion_args = [
        "--sample-size",
        os.environ.get("FQ_BENCH_SAMPLE_SIZE", "10"),
        "--warm-up-time",
        os.environ.get("FQ_BENCH_WARM_UP_TIME", "1"),
        "--measurement-time",
        os.environ.get("FQ_BENCH_MEASUREMENT_TIME", "1"),
        "--nresamples",
        os.environ.get("FQ_BENCH_NRESAMPLES", "1000"),
        "--noplot",
    ]
    baseline_args = ["--save-baseline", BASELINE] if mode == "baseline" else ["--baseline", BASELINE]
    # Apply the same faster link profile to the base and head, including when
    # the base checkout predates this script. The weekly suite keeps fat LTO.
    build_env = os.environ | {
        "CARGO_PROFILE_BENCH_LTO": BUILD_PROFILE["lto"],
        "CARGO_PROFILE_BENCH_CODEGEN_UNITS": str(BUILD_PROFILE["codegen_units"]),
    }
    for package, target, benchmark in CASES:
        command = [
            cargo,
            "bench",
            "-p",
            package,
            "--profile",
            "bench",
            "--bench",
            target,
            "--",
            f"^{re.escape(benchmark)}$",
            *baseline_args,
            *criterion_args,
        ]
        # Arguments come only from the fixed case list and numeric CI settings.
        subprocess.run(command, check=True, env=build_env)  # noqa: S603
    return started_at_ns


def main() -> int:
    """Create the base benchmark or compare the PR head against it."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("baseline", "compare"))
    args = parser.parse_args()

    rustc_path = shutil.which("rustc")
    if rustc_path is None:
        raise RuntimeError("rustc is required for PR benchmarks")
    rustc = subprocess.check_output([rustc_path, "--version"], text=True).strip()  # noqa: S603
    if args.mode == "baseline":
        for _, _, benchmark in CASES:
            case_dir = CRITERION_ROOT / benchmark
            if case_dir.exists():
                shutil.rmtree(case_dir)
        started_at_ns = run_cases("baseline")
        require_exact_outputs(CRITERION_ROOT, BASELINE, started_at_ns)
        MANIFEST.parent.mkdir(parents=True, exist_ok=True)
        MANIFEST.write_text(
            json.dumps({"rustc": rustc, "benchmarks": [case[2] for case in CASES], "profile": BUILD_PROFILE}) + "\n"
        )
        print(f"PR benchmark baseline ready: {len(CASES)} cases, {rustc}")
        return 0

    manifest = json.loads(MANIFEST.read_text())
    if manifest != {"rustc": rustc, "benchmarks": [case[2] for case in CASES], "profile": BUILD_PROFILE}:
        raise ValueError("PR benchmark selection or Rust toolchain changed between base and head")
    for relative in expected_paths(BASELINE):
        if not (CRITERION_ROOT / relative).is_file():
            raise ValueError(f"missing PR benchmark baseline: {relative}")
    started_at_ns = run_cases("compare")
    found = regressions(CRITERION_ROOT, started_at_ns)
    for benchmark, change in found:
        print(f"REGRESSION {benchmark}: median change {change:+.2%}")
    if found:
        return 1
    print(f"PR benchmark gate passed: {len(CASES)} fresh comparisons, threshold {THRESHOLD:.0%}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
