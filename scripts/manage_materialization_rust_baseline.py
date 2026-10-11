"""Establish and verify a compact immutable Rust materialization baseline."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
from typing import Any

from baseline_identity import require_baseline_name, require_sha256, require_tree_revision
from prepare_materialization_criterion_run import combined_fixture_digest, tree_revision

EXPECTED_CASES = (
    "cold_a_5000_unique",
    "cold_b_5000_50",
    "warm_b_5000_50",
    "validation_b_5000_50",
)
SCHEMA = "finstack_quant.materialization_rust_baseline/1"


def guard_establishment(output: Path, replace: bool) -> None:
    """Reject accidental replacement of an existing checked-in baseline."""
    if output.exists() and not replace:
        raise FileExistsError(
            f"named Rust baseline already exists at {output}; pass --replace only for intentional replacement"
        )


def _git_head() -> str:
    git = shutil.which("git")
    if git is None:
        raise RuntimeError("git is required to record baseline provenance")
    return subprocess.check_output((git, "rev-parse", "HEAD"), text=True).strip()  # noqa: S603


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _median_estimate(path: Path) -> dict[str, float]:
    payload: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    median = payload["median"]
    interval = median["confidence_interval"]
    return {
        "point": float(median["point_estimate"]) / 1_000_000.0,
        "lower": float(interval["lower_bound"]) / 1_000_000.0,
        "upper": float(interval["upper_bound"]) / 1_000_000.0,
    }


def _medians(criterion_dir: Path, started_at_ns: int | None = None) -> dict[str, dict[str, float]]:
    expected = {f"portfolio_materialization/{case}/new/estimates.json" for case in EXPECTED_CASES}
    discovered = {
        path.relative_to(criterion_dir).as_posix()
        for path in criterion_dir.glob("portfolio_materialization/*/new/estimates.json")
    }
    missing = sorted(expected - discovered)
    unexpected = sorted(discovered - expected)
    if missing or unexpected:
        raise ValueError(f"Criterion output mismatch: missing={missing or 'none'} unexpected={unexpected or 'none'}")
    medians = {}
    for case in EXPECTED_CASES:
        path = criterion_dir / "portfolio_materialization" / case / "new" / "estimates.json"
        if started_at_ns is not None and path.stat().st_mtime_ns < started_at_ns:
            raise ValueError(f"stale Criterion artifact predates this run: {path}")
        medians[case] = _median_estimate(path)
    return medians


def write_baseline(
    criterion_dir: Path,
    metadata_path: Path,
    fixture_a: Path,
    fixture_b: Path,
    output: Path,
    baseline_name: str,
    replace: bool = False,
) -> None:
    """Write a compact baseline from exact fresh root-target Criterion outputs."""
    guard_establishment(output, replace)
    metadata: dict[str, Any] = json.loads(metadata_path.read_text(encoding="utf-8"))
    fixture_sha256 = combined_fixture_digest(fixture_a, fixture_b)
    if metadata.get("fixture_sha256") != fixture_sha256:
        raise ValueError("run metadata fixture digest mismatch")
    revision = tree_revision()
    if metadata.get("current_revision") != revision:
        raise ValueError("run metadata current revision is stale")
    started_at_ns = metadata.get("started_at_ns")
    if not isinstance(started_at_ns, int):
        raise TypeError("run metadata requires integer started_at_ns")
    payload = {
        "schema": SCHEMA,
        "baseline_name": require_baseline_name(baseline_name),
        "base_revision": _git_head(),
        "tree_revision": revision,
        "fixture_sha256": fixture_sha256,
        "fixtures": {"a": _sha256(fixture_a), "b": _sha256(fixture_b)},
        "expected_cases": list(EXPECTED_CASES),
        "medians_ms": _medians(criterion_dir, started_at_ns),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _require_baseline_shape(baseline: dict[str, Any]) -> None:
    """Reject a compact baseline whose schema, cases, medians, or identity are wrong."""
    if baseline.get("schema") != SCHEMA:
        raise ValueError("unsupported Rust materialization baseline schema")
    if baseline.get("expected_cases") != list(EXPECTED_CASES):
        raise ValueError("Rust materialization baseline case set mismatch")
    medians = baseline.get("medians_ms")
    if not isinstance(medians, dict) or set(medians) != set(EXPECTED_CASES):
        raise ValueError("Rust materialization baseline medians are incomplete")
    for estimate in medians.values():
        if (
            not isinstance(estimate, dict)
            or set(estimate) != {"point", "lower", "upper"}
            or not all(
                isinstance(value, (int, float)) and math.isfinite(float(value)) and float(value) > 0.0
                for value in estimate.values()
            )
        ):
            raise ValueError("Rust materialization baseline estimates must be positive finite ranges")
    require_baseline_name(baseline.get("baseline_name"))
    require_tree_revision(baseline.get("tree_revision"))
    require_sha256(baseline.get("fixture_sha256"), "fixture_sha256")
    fixtures = baseline.get("fixtures")
    if not isinstance(fixtures, dict) or set(fixtures) != {"a", "b"}:
        raise ValueError("Rust materialization baseline fixtures must name fixture a and fixture b")
    require_sha256(fixtures["a"], "fixtures.a")
    require_sha256(fixtures["b"], "fixtures.b")


def verify_baseline(
    baseline_path: Path,
    fixture_a: Path,
    fixture_b: Path,
    manifest_path: Path | None = None,
) -> dict[str, Any]:
    """Verify compact baseline shape, fixture identity, and optional manifest."""
    baseline: dict[str, Any] = json.loads(baseline_path.read_text(encoding="utf-8"))
    _require_baseline_shape(baseline)
    fixtures = {"a": _sha256(fixture_a), "b": _sha256(fixture_b)}
    if baseline.get("fixture_sha256") != combined_fixture_digest(fixture_a, fixture_b):
        raise ValueError("Rust materialization baseline fixture digest mismatch")
    if baseline.get("fixtures") != fixtures:
        raise ValueError("Rust materialization baseline individual fixture digest mismatch")
    if manifest_path is not None:
        manifest: dict[str, Any] = json.loads(manifest_path.read_text(encoding="utf-8"))
        expected = {
            "baseline_name": baseline["baseline_name"],
            "tree_revision": baseline["tree_revision"],
            "fixture_sha256": baseline["fixture_sha256"],
            "sha256": _sha256(baseline_path),
            "expected_cases": list(EXPECTED_CASES),
        }
        actual = {
            "baseline_name": manifest.get("rust_baseline_name"),
            "tree_revision": manifest.get("rust_baseline_tree_revision"),
            "fixture_sha256": manifest.get("rust_baseline_fixture_sha256"),
            "sha256": manifest.get("rust_baseline_sha256"),
            "expected_cases": manifest.get("rust_baseline_cases"),
        }
        if actual != expected:
            raise ValueError("immutable Rust baseline manifest mismatch")
    return baseline


def seal_baseline_manifest(baseline_path: Path, manifest_path: Path) -> None:
    """Record checked-in Rust baseline identity in the benchmark manifest."""
    baseline: dict[str, Any] = json.loads(baseline_path.read_text(encoding="utf-8"))
    _require_baseline_shape(baseline)
    manifest: dict[str, Any] = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest.update({
        "rust_baseline_name": baseline["baseline_name"],
        "rust_baseline_tree_revision": baseline["tree_revision"],
        "rust_baseline_fixture_sha256": baseline["fixture_sha256"],
        "rust_baseline_sha256": _sha256(baseline_path),
        "rust_baseline_cases": list(EXPECTED_CASES),
    })
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """Run one explicit compact-baseline management operation."""
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="operation", required=True)
    guard = subparsers.add_parser("guard")
    guard.add_argument("--baseline", type=Path, required=True)
    guard.add_argument("--replace", action="store_true")
    establish = subparsers.add_parser("establish")
    establish.add_argument("--criterion-dir", type=Path, required=True)
    establish.add_argument("--run-metadata", type=Path, required=True)
    establish.add_argument("--fixture-a", type=Path, required=True)
    establish.add_argument("--fixture-b", type=Path, required=True)
    establish.add_argument("--output", type=Path, required=True)
    establish.add_argument("--baseline-name", required=True)
    establish.add_argument("--replace", action="store_true")
    verify = subparsers.add_parser("verify")
    verify.add_argument("--baseline", type=Path, required=True)
    verify.add_argument("--fixture-a", type=Path, required=True)
    verify.add_argument("--fixture-b", type=Path, required=True)
    verify.add_argument("--manifest", type=Path, required=True)
    seal = subparsers.add_parser("seal")
    seal.add_argument("--baseline", type=Path, required=True)
    seal.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()

    try:
        if args.operation == "guard":
            guard_establishment(args.baseline, args.replace)
        elif args.operation == "establish":
            write_baseline(
                args.criterion_dir,
                args.run_metadata,
                args.fixture_a,
                args.fixture_b,
                args.output,
                args.baseline_name,
                args.replace,
            )
            print(f"Rust materialization baseline established: {args.output}")
        elif args.operation == "seal":
            seal_baseline_manifest(args.baseline, args.manifest)
            print(f"Rust materialization baseline manifest sealed: {args.manifest}")
        else:
            baseline = verify_baseline(
                args.baseline,
                args.fixture_a,
                args.fixture_b,
                args.manifest,
            )
            print(
                "Immutable Rust materialization baseline verified: "
                f"{baseline['baseline_name']} {baseline['tree_revision']}"
            )
    except (OSError, RuntimeError, TypeError, ValueError, KeyError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
