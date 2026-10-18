"""Fail review when Criterion median estimates regress beyond a threshold."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

try:
    # Imported as `scripts.check_criterion_regressions` (the PR benchmark runner
    # and its tests): resolve the sibling through the package.
    from .baseline_identity import require_baseline_name, require_sha256, require_tree_revision
except ImportError:
    # Run as a script from `scripts/`, where the sibling is a top-level module.
    from baseline_identity import require_baseline_name, require_sha256, require_tree_revision

MATERIALIZATION_CASES = (
    "cold_a_5000_unique",
    "cold_b_5000_50",
    "warm_b_5000_50",
    "validation_b_5000_50",
)
MATERIALIZATION_EXPECTED_BENCHMARKS = tuple(
    f"portfolio_materialization/{case}/change/estimates.json" for case in MATERIALIZATION_CASES
)


def _median_change(path: Path) -> float:
    payload: Any = json.loads(path.read_text(encoding="utf-8"))
    return float(payload["median"]["point_estimate"])


def _median_estimate(path: Path, divisor: float) -> dict[str, float]:
    payload: Any = json.loads(path.read_text(encoding="utf-8"))
    median = payload["median"]
    interval = median["confidence_interval"]
    return {
        "point": float(median["point_estimate"]) / divisor,
        "lower": float(interval["lower_bound"]) / divisor,
        "upper": float(interval["upper_bound"]) / divisor,
    }


def materialization_comparison(
    root: Path,
    baseline: dict[str, Any],
    started_at_ns: int | None = None,
) -> dict[str, dict[str, Any]]:
    """Compare exact fresh Criterion outputs with a compact immutable baseline."""
    if baseline.get("schema") != "finstack_quant.materialization_rust_baseline/1":
        raise ValueError("unsupported Rust materialization baseline schema")
    if baseline.get("expected_cases") != list(MATERIALIZATION_CASES):
        raise ValueError("Rust materialization baseline case set mismatch")
    medians = baseline.get("medians_ms")
    if not isinstance(medians, dict) or set(medians) != set(MATERIALIZATION_CASES):
        raise ValueError("Rust materialization baseline medians are incomplete")

    expected_paths = {f"portfolio_materialization/{case}/new/estimates.json" for case in MATERIALIZATION_CASES}
    discovered = {
        path.relative_to(root).as_posix() for path in root.glob("portfolio_materialization/*/new/estimates.json")
    }
    missing = sorted(expected_paths - discovered)
    unexpected = sorted(discovered - expected_paths)
    if missing or unexpected:
        raise ValueError(f"Criterion output mismatch: missing={missing or 'none'} unexpected={unexpected or 'none'}")

    comparison = {}
    for case in MATERIALIZATION_CASES:
        relative_path = Path("portfolio_materialization") / case / "new" / "estimates.json"
        estimate_path = root / relative_path
        if started_at_ns is not None and estimate_path.stat().st_mtime_ns < started_at_ns:
            raise ValueError(f"stale Criterion artifact predates this run: {relative_path.as_posix()}")
        current = _median_estimate(estimate_path, 1_000_000.0)
        baseline_estimate = medians[case]
        baseline_point = float(baseline_estimate["point"])
        comparison[case] = {
            "benchmark_id": f"portfolio_materialization/{case}",
            "paths": {
                "baseline": "benchmarks/materialization/materialization-rust-baseline.json",
                "current": relative_path.as_posix(),
            },
            "baseline_median_ms": baseline_estimate,
            "current_median_ms": current,
            "relative_median_change": {
                "point": current["point"] / baseline_point - 1.0,
                "lower": current["lower"] / float(baseline_estimate["upper"]) - 1.0,
                "upper": current["upper"] / float(baseline_estimate["lower"]) - 1.0,
            },
        }
    return comparison


def materialization_regressions(
    comparison: dict[str, dict[str, Any]],
    threshold: float,
) -> list[tuple[str, float]]:
    """Return compact-baseline materialization medians above the threshold."""
    return [
        (case, float(result["relative_median_change"]["point"]))
        for case, result in comparison.items()
        if (change := float(result["relative_median_change"]["point"])) > threshold
        and not math.isclose(change, threshold, rel_tol=0.0, abs_tol=1e-12)
    ]


def collect_regressions(
    root: Path,
    threshold: float,
    expected_paths: tuple[str, ...] | None = None,
    started_at_ns: int | None = None,
) -> list[tuple[str, float]]:
    """Collect regressions, with exact-output validation when paths are supplied."""
    discovered = {path.relative_to(root).as_posix() for path in root.glob("**/change/estimates.json")}
    if expected_paths is None:
        if not discovered:
            raise ValueError(f"no Criterion comparison outputs found under {root}")
        selected_paths = tuple(sorted(discovered))
    else:
        expected = set(expected_paths)
        missing = sorted(expected - discovered)
        unexpected = sorted(discovered - expected)
        if missing or unexpected:
            raise ValueError(
                f"Criterion output mismatch: missing={missing or 'none'} unexpected={unexpected or 'none'}"
            )
        selected_paths = expected_paths

    regressions = []
    for relative_path in selected_paths:
        estimate_path = root / relative_path
        if started_at_ns is not None and estimate_path.stat().st_mtime_ns < started_at_ns:
            raise ValueError(f"stale Criterion artifact predates this run: {relative_path}")
        change = _median_change(estimate_path)
        if change > threshold:
            benchmark = estimate_path.parent.parent.relative_to(root).as_posix()
            regressions.append((benchmark, change))
    return regressions


def load_baseline(path: Path) -> dict[str, Any]:
    """Load a compact baseline and reject malformed identity fields before any write."""
    payload: object = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise TypeError("compact baseline must be a JSON object")
    require_baseline_name(payload.get("baseline_name"))
    require_tree_revision(payload.get("tree_revision"))
    require_sha256(payload.get("fixture_sha256"), "fixture_sha256")
    return payload


def load_run_metadata(path: Path) -> dict[str, Any]:
    """Load and validate benchmark provenance used by the comparison gate."""
    payload: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    require_baseline_name(payload.get("baseline_name"))
    for key in ("baseline_revision", "current_revision"):
        if not isinstance(payload.get(key), str) or not payload[key]:
            raise TypeError(f"run metadata requires non-empty {key}")
    require_sha256(payload.get("fixture_sha256"), "run metadata fixture_sha256")
    if not isinstance(payload.get("started_at_ns"), int):
        raise TypeError("run metadata requires integer started_at_ns")
    return payload


def main() -> int:
    """Run the Criterion comparison gate and return a process exit code."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=Path("target/criterion"),
        help="Criterion output directory (default: target/criterion)",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        help="Compact checked-in materialization Rust baseline",
    )
    parser.add_argument(
        "--comparison-output",
        type=Path,
        help="Optional compact materialization comparison artifact",
    )
    parser.add_argument(
        "--run-metadata",
        type=Path,
        help="Optional JSON provenance created immediately before a strict comparison run",
    )
    parser.add_argument(
        "--expected-path",
        action="append",
        default=[],
        help="Exact relative change/estimates.json path; repeat for strict output matching",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.10,
        help="Maximum allowed relative median increase (default: 0.10)",
    )
    args = parser.parse_args()

    try:
        metadata = load_run_metadata(args.run_metadata) if args.run_metadata else None
        if args.baseline:
            baseline = load_baseline(args.baseline)
            comparison = materialization_comparison(
                args.criterion_dir,
                baseline,
                started_at_ns=metadata["started_at_ns"] if metadata else None,
            )
            regressions = materialization_regressions(comparison, args.threshold)
            if args.comparison_output:
                args.comparison_output.parent.mkdir(parents=True, exist_ok=True)
                args.comparison_output.write_text(
                    json.dumps(
                        {
                            "schema": "finstack_quant.materialization_rust_comparison/1",
                            "baseline": {
                                "baseline_name": baseline["baseline_name"],
                                "tree_revision": baseline["tree_revision"],
                                "fixture_sha256": baseline["fixture_sha256"],
                            },
                            "current": {
                                "tree_revision": metadata["current_revision"] if metadata else None,
                                "fixture_sha256": metadata["fixture_sha256"] if metadata else None,
                            },
                            "threshold": args.threshold,
                            "passed": not regressions,
                            "cases": comparison,
                        },
                        indent=2,
                    )
                    + "\n",
                    encoding="utf-8",
                )
        else:
            regressions = collect_regressions(
                args.criterion_dir,
                args.threshold,
                expected_paths=tuple(args.expected_path) or None,
                started_at_ns=metadata["started_at_ns"] if metadata else None,
            )
    except (OSError, TypeError, ValueError, json.JSONDecodeError, KeyError) as error:
        parser.error(str(error))
    if regressions:
        for benchmark, change in regressions:
            print(f"REGRESSION {benchmark}: median change {change:+.2%}")
        print(f"Criterion review gate failed: {len(regressions)} regression(s)")
        return 1

    provenance = ""
    if metadata:
        provenance = (
            f", baseline={metadata['baseline_name']} "
            f"baseline_revision={metadata['baseline_revision']} "
            f"current_revision={metadata['current_revision']} "
            f"fixture_sha256={metadata['fixture_sha256']}"
        )
    print(
        f"Criterion review gate passed: "
        f"{len(MATERIALIZATION_CASES) if args.baseline else len(tuple(args.expected_path)) or 'discovered'} "
        f"comparison(s){provenance}, "
        f"median regression threshold {args.threshold:.0%}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
