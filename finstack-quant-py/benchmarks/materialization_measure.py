"""Record raw release-Python materialization latency samples."""

from __future__ import annotations

import argparse
from collections.abc import Callable
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess  # nosec B404 # fixed Cargo benchmark command, no shell
from time import perf_counter_ns
from typing import Any

from finstack_quant.portfolio import InstrumentArtifactCache, Portfolio


def sample_count() -> int:
    """Return the validated acceptance or explicit smoke sample count."""
    raw = os.environ.get("FQ_MATERIALIZATION_P95_SAMPLES", "100")
    try:
        value = int(raw)
    except ValueError as error:
        raise ValueError(f"sample count must be a finite integer, got {raw!r}") from error
    minimum = 1 if os.environ.get("FQ_MATERIALIZATION_SMOKE") == "1" else 100
    if value < minimum:
        raise ValueError(f"sample count must be at least {minimum}, got {value}")
    return value


def timed(call: Callable[[], Any]) -> tuple[float, Any]:
    """Measure one call and return elapsed milliseconds and its result."""
    started = perf_counter_ns()
    result = call()
    return (perf_counter_ns() - started) / 1_000_000.0, result


def counters(report: Any) -> dict[str, Any]:
    """Extract stable materialization counters from a Python report."""
    return {
        "input_bytes": report.input_bytes,
        "unique_instruments": report.unique_instruments,
        "positions": report.positions,
        "dependencies": report.dependencies,
        "cache_hits": report.cache_hits,
        "phase_nanos": report.phase_nanos,
    }


def measure(payload: bytes, capacity: int, samples: int, warm: bool) -> tuple[list[float], Any]:
    """Measure materialization with cache setup strictly outside each timer."""
    if warm:
        cache = InstrumentArtifactCache(capacity)
        Portfolio.from_materialization(payload, cache)
        caches = [cache] * samples
    else:
        caches = [InstrumentArtifactCache(capacity) for _ in range(samples)]

    elapsed: list[float] = []
    last_report = None
    for cache in caches:
        duration, loaded = timed(lambda cache=cache: Portfolio.from_materialization(payload, cache))
        portfolio, last_report = loaded
        elapsed.append(duration)
        del portfolio
    return elapsed, last_report


def main() -> int:
    """Measure all comparable Python cases and write one raw JSON fragment."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture_directory", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    samples = sample_count()
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required to regenerate materialization fixtures")
    subprocess.run(  # nosec B603 # noqa: S603 -- resolved executable and fixed argument vector
        [
            cargo,
            "run",
            "--release",
            "--quiet",
            "-p",
            "finstack-quant-portfolio",
            "--example",
            "materialization_fixtures",
            "--",
            str(args.fixture_directory),
        ],
        cwd=Path(__file__).resolve().parents[2],
        check=True,
    )
    fixture_a = (args.fixture_directory / "materialization-a-5000-unique.json").read_bytes()
    fixture_b = (args.fixture_directory / "materialization-b-5000-50.json").read_bytes()
    cold_a, report_a = measure(fixture_a, 5_000, samples, warm=False)
    cold_b, report_b = measure(fixture_b, 50, samples, warm=False)
    warm_b, report_warm = measure(fixture_b, 50, samples, warm=True)

    payload = {
        "language": "python",
        "timing_boundary": (
            "fixture bytes and explicitly-sized cache exist before start; elapsed surrounds "
            "Portfolio.from_materialization through creation of Python portfolio/report wrappers"
        ),
        "fixture_sha256": {
            "a": hashlib.sha256(fixture_a).hexdigest(),
            "b": hashlib.sha256(fixture_b).hexdigest(),
        },
        "cases": {
            "cold_a_5000_unique": cold_a,
            "cold_b_5000_50": cold_b,
            "warm_b_5000_50": warm_b,
        },
        "counters": {
            "cold_a_5000_unique": counters(report_a),
            "cold_b_5000_50": counters(report_b),
            "warm_b_5000_50": counters(report_warm),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
