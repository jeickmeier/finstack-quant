"""Establish or compare strict Python materialization benchmark baselines."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import time
from typing import Any

from baseline_identity import require_baseline_name, require_sha256, require_tree_revision
from prepare_materialization_criterion_run import combined_fixture_digest, tree_revision

EXPECTED_CASES = ("cold_a_5000_unique", "cold_b_5000_50", "warm_b_5000_50")
SCHEMA = "finstack_quant.python_materialization_baseline/1"


def guard_establishment(output: Path, replace: bool) -> None:
    """Reject accidental replacement of an existing named Python baseline."""
    if output.exists() and not replace:
        raise FileExistsError(
            f"named Python baseline already exists at {output}; pass --replace only for intentional replacement"
        )


def _load(path: Path) -> dict[str, Any]:
    payload: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    return payload


def _cases(fragment: dict[str, Any]) -> dict[str, list[float]]:
    cases = fragment.get("cases")
    if not isinstance(cases, dict) or set(cases) != set(EXPECTED_CASES):
        actual = sorted(cases) if isinstance(cases, dict) else type(cases).__name__
        raise ValueError(f"Python benchmark case mismatch: expected={list(EXPECTED_CASES)} actual={actual}")
    normalized = {}
    for name in EXPECTED_CASES:
        values = cases[name]["samples"] if isinstance(cases[name], dict) else cases[name]
        if not isinstance(values, list) or len(values) < 100:
            raise ValueError(f"{name} requires at least 100 samples")
        samples = [float(value) for value in values]
        if not all(math.isfinite(value) and value >= 0.0 for value in samples):
            raise ValueError(f"{name} contains invalid samples")
        normalized[name] = samples
    return normalized


def _fixture_digests(fixture_a: Path, fixture_b: Path) -> tuple[dict[str, str], str]:
    individual = {
        "a": hashlib.sha256(fixture_a.read_bytes()).hexdigest(),
        "b": hashlib.sha256(fixture_b.read_bytes()).hexdigest(),
    }
    return individual, combined_fixture_digest(fixture_a, fixture_b)


def write_run_metadata(fixture_a: Path, fixture_b: Path, output: Path) -> None:
    """Record fixture and revision identity immediately before a measurement."""
    _, combined = _fixture_digests(fixture_a, fixture_b)
    payload = {
        "current_revision": tree_revision(),
        "fixture_sha256": combined,
        "started_at_ns": time.time_ns(),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def write_baseline(
    measurement_path: Path,
    metadata_path: Path,
    fixture_a: Path,
    fixture_b: Path,
    output: Path,
    artifact_root: Path,
    baseline_name: str,
    replace: bool = False,
) -> None:
    """Validate a fresh measurement and write a compact named baseline."""
    guard_establishment(output, replace)
    _ensure_runtime_artifact(artifact_root, measurement_path, "measurement")
    measurement = _load(measurement_path)
    metadata = _load(metadata_path)
    cases = _cases(measurement)
    individual, combined = _fixture_digests(fixture_a, fixture_b)
    if measurement.get("fixture_sha256") != individual:
        raise ValueError("measurement fixture digests do not match the supplied fixture paths")
    if metadata.get("fixture_sha256") != combined:
        raise ValueError("run metadata fixture digest does not match supplied fixtures")
    revision = tree_revision()
    if metadata.get("current_revision") != revision:
        raise ValueError("run metadata current revision is stale")
    started_at_ns = metadata.get("started_at_ns")
    if not isinstance(started_at_ns, int):
        raise TypeError("run metadata requires integer started_at_ns")
    if measurement_path.stat().st_mtime_ns < started_at_ns:
        raise ValueError("stale Python measurement predates baseline establishment")
    payload = {
        "schema": SCHEMA,
        "baseline_name": require_baseline_name(baseline_name),
        "tree_revision": revision,
        "fixture_sha256": combined,
        "fixtures": individual,
        "expected_cases": list(EXPECTED_CASES),
        "medians_ms": {name: _median(samples) for name, samples in cases.items()},
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _ensure_runtime_artifact(root: Path, path: Path, label: str) -> None:
    resolved_root = root.resolve()
    resolved = path.resolve()
    if not resolved.is_relative_to(resolved_root):
        raise ValueError(f"{label} path must be isolated under {resolved_root}")


def _median(samples: list[float]) -> float:
    ordered = sorted(samples)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2.0


def compact_baseline_from_record(
    record: dict[str, Any],
    baseline_name: str,
) -> dict[str, Any]:
    """Recover a compact immutable baseline from a source-backed result record."""
    current_cases = {
        name: [float(value) for value in values]
        for name, values in record["languages"]["python"]["raw_samples_ms"].items()
    }
    if set(current_cases) != set(EXPECTED_CASES):
        raise ValueError("record Python case set mismatch")
    changes = record["gates"]["python_median_changes"]
    if set(changes) != set(EXPECTED_CASES):
        raise ValueError("record Python median-change case set mismatch")
    baseline = record["baseline"]
    fixtures = record["fixtures"]
    return {
        "schema": SCHEMA,
        "baseline_name": require_baseline_name(baseline_name),
        "tree_revision": require_tree_revision(baseline["python_baseline_tree_revision"]),
        "fixture_sha256": require_sha256(baseline["python_baseline_fixture_sha256"], "fixture_sha256"),
        "fixtures": {
            "a": require_sha256(fixtures["a"]["sha256"], "fixtures.a"),
            "b": require_sha256(fixtures["b"]["sha256"], "fixtures.b"),
        },
        "expected_cases": list(EXPECTED_CASES),
        "medians_ms": {name: _median(current_cases[name]) / (1.0 + float(changes[name])) for name in EXPECTED_CASES},
    }


def _require_baseline_shape(baseline: dict[str, Any]) -> None:
    """Reject a compact baseline whose schema, cases, medians, or identity are wrong."""
    if baseline.get("schema") != SCHEMA:
        raise ValueError("unsupported Python materialization baseline schema")
    if baseline.get("expected_cases") != list(EXPECTED_CASES):
        raise ValueError("baseline expected case set is stale")
    medians = baseline.get("medians_ms")
    if not isinstance(medians, dict) or set(medians) != set(EXPECTED_CASES):
        raise ValueError("baseline median case set is stale")
    if not all(
        isinstance(value, (int, float)) and math.isfinite(float(value)) and float(value) > 0.0
        for value in medians.values()
    ):
        raise ValueError("baseline medians must be positive finite numbers")
    require_baseline_name(baseline.get("baseline_name"))
    require_tree_revision(baseline.get("tree_revision"))
    require_sha256(baseline.get("fixture_sha256"), "fixture_sha256")
    fixtures = baseline.get("fixtures")
    if not isinstance(fixtures, dict) or set(fixtures) != {"a", "b"}:
        raise ValueError("baseline fixtures must name fixture a and fixture b")
    require_sha256(fixtures["a"], "fixtures.a")
    require_sha256(fixtures["b"], "fixtures.b")


def verify_baseline(
    baseline_path: Path,
    fixture_a: Path,
    fixture_b: Path,
    manifest_path: Path | None = None,
) -> dict[str, Any]:
    """Verify the immutable Python baseline schema, cases, and fixture identity."""
    baseline = _load(baseline_path)
    _require_baseline_shape(baseline)
    individual, combined = _fixture_digests(fixture_a, fixture_b)
    if baseline.get("fixture_sha256") != combined or baseline.get("fixtures") != individual:
        raise ValueError("baseline fixture digest does not match supplied fixtures")
    if manifest_path is not None:
        manifest = _load(manifest_path)
        expected = {
            "baseline_name": baseline["baseline_name"],
            "tree_revision": baseline["tree_revision"],
            "fixture_sha256": baseline["fixture_sha256"],
            "sha256": hashlib.sha256(baseline_path.read_bytes()).hexdigest(),
            "expected_cases": list(EXPECTED_CASES),
        }
        actual = {
            "baseline_name": manifest.get("python_baseline_name"),
            "tree_revision": manifest.get("python_baseline_tree_revision"),
            "fixture_sha256": manifest.get("python_baseline_fixture_sha256"),
            "sha256": manifest.get("python_baseline_sha256"),
            "expected_cases": manifest.get("python_baseline_cases"),
        }
        if actual != expected:
            raise ValueError("immutable Python baseline manifest mismatch")
    return baseline


def seal_baseline_manifest(baseline_path: Path, manifest_path: Path) -> None:
    """Record the immutable Python baseline digest and provenance."""
    baseline = _load(baseline_path)
    _require_baseline_shape(baseline)
    manifest = _load(manifest_path)
    manifest.update({
        "python_baseline_name": baseline["baseline_name"],
        "python_baseline_tree_revision": baseline["tree_revision"],
        "python_baseline_fixture_sha256": baseline["fixture_sha256"],
        "python_baseline_sha256": hashlib.sha256(baseline_path.read_bytes()).hexdigest(),
        "python_baseline_cases": list(EXPECTED_CASES),
    })
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def check_comparison(
    baseline_path: Path,
    current_path: Path,
    metadata_path: Path,
    fixture_a: Path,
    fixture_b: Path,
    artifact_root: Path,
    threshold: float,
    result_output: Path | None = None,
    baseline_manifest_path: Path | None = None,
) -> list[tuple[str, float]]:
    """Validate provenance/freshness and return median regressions."""
    if baseline_path.resolve() == current_path.resolve():
        raise ValueError("baseline and current measurements must use distinct paths")
    _ensure_runtime_artifact(artifact_root, current_path, "current")
    baseline = verify_baseline(
        baseline_path,
        fixture_a,
        fixture_b,
        baseline_manifest_path,
    )
    current = _load(current_path)
    metadata = _load(metadata_path)
    current_cases = _cases(current)

    individual, combined = _fixture_digests(fixture_a, fixture_b)
    if current.get("fixture_sha256") != individual:
        raise ValueError("current measurement fixture digest does not match supplied fixtures")
    if metadata.get("fixture_sha256") != combined:
        raise ValueError("run metadata fixture digest does not match supplied fixtures")
    if metadata.get("baseline_name") != baseline.get("baseline_name"):
        raise ValueError("run metadata baseline name mismatch")
    if metadata.get("baseline_revision") != baseline.get("tree_revision"):
        raise ValueError("run metadata baseline revision mismatch")
    if metadata.get("current_revision") != tree_revision():
        raise ValueError("run metadata current revision is stale")
    started_at_ns = metadata.get("started_at_ns")
    if not isinstance(started_at_ns, int):
        raise TypeError("run metadata requires integer started_at_ns")
    if current_path.stat().st_mtime_ns < started_at_ns:
        raise ValueError("stale Python measurement predates this comparison run")

    changes = {}
    regressions = []
    for name in EXPECTED_CASES:
        baseline_median = float(baseline["medians_ms"][name])
        current_median = _median(current_cases[name])
        change = current_median / baseline_median - 1.0
        changes[name] = change
        if change > threshold:
            regressions.append((name, change))
    if result_output is not None:
        result = {
            "schema": "finstack_quant.python_materialization_comparison/1",
            "baseline": {
                "baseline_name": baseline["baseline_name"],
                "tree_revision": baseline["tree_revision"],
                "fixture_sha256": baseline["fixture_sha256"],
            },
            "current": {
                "tree_revision": metadata["current_revision"],
                "fixture_sha256": combined,
            },
            "expected_cases": list(EXPECTED_CASES),
            "median_changes": changes,
            "threshold": threshold,
            "passed": not regressions,
        }
        result_output.parent.mkdir(parents=True, exist_ok=True)
        result_output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return regressions


def main() -> int:
    """Run baseline establishment or strict comparison."""
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="operation", required=True)
    prepare = subparsers.add_parser("prepare")
    prepare.add_argument("--fixture-a", type=Path, required=True)
    prepare.add_argument("--fixture-b", type=Path, required=True)
    prepare.add_argument("--output", type=Path, required=True)
    guard = subparsers.add_parser("guard")
    guard.add_argument("--baseline", type=Path, required=True)
    guard.add_argument("--replace", action="store_true")
    verify = subparsers.add_parser("verify")
    verify.add_argument("--baseline", type=Path, required=True)
    verify.add_argument("--fixture-a", type=Path, required=True)
    verify.add_argument("--fixture-b", type=Path, required=True)
    verify.add_argument("--manifest", type=Path, required=True)
    seal = subparsers.add_parser("seal")
    seal.add_argument("--baseline", type=Path, required=True)
    seal.add_argument("--manifest", type=Path, required=True)
    import_record = subparsers.add_parser("import-record")
    import_record.add_argument("--record", type=Path, required=True)
    import_record.add_argument("--output", type=Path, required=True)
    import_record.add_argument("--baseline-name", required=True)
    import_record.add_argument("--replace", action="store_true")
    establish = subparsers.add_parser("establish")
    establish.add_argument("--measurement", type=Path, required=True)
    establish.add_argument("--run-metadata", type=Path, required=True)
    establish.add_argument("--fixture-a", type=Path, required=True)
    establish.add_argument("--fixture-b", type=Path, required=True)
    establish.add_argument("--output", type=Path, required=True)
    establish.add_argument("--artifact-root", type=Path, required=True)
    establish.add_argument("--baseline-name", required=True)
    establish.add_argument("--replace", action="store_true")
    compare = subparsers.add_parser("compare")
    compare.add_argument("--baseline", type=Path, required=True)
    compare.add_argument("--current", type=Path, required=True)
    compare.add_argument("--run-metadata", type=Path, required=True)
    compare.add_argument("--fixture-a", type=Path, required=True)
    compare.add_argument("--fixture-b", type=Path, required=True)
    compare.add_argument("--artifact-root", type=Path, required=True)
    compare.add_argument("--threshold", type=float, default=0.10)
    compare.add_argument("--result-output", type=Path, required=True)
    compare.add_argument("--baseline-manifest", type=Path, required=True)
    args = parser.parse_args()

    try:
        if args.operation == "guard":
            guard_establishment(args.baseline, args.replace)
        elif args.operation == "prepare":
            write_run_metadata(args.fixture_a, args.fixture_b, args.output)
            print(f"Python materialization run metadata prepared: {args.output}")
        elif args.operation == "verify":
            baseline = verify_baseline(
                args.baseline,
                args.fixture_a,
                args.fixture_b,
                args.manifest,
            )
            print(
                "Immutable Python materialization baseline verified: "
                f"{baseline['baseline_name']} {baseline['tree_revision']}"
            )
        elif args.operation == "seal":
            seal_baseline_manifest(args.baseline, args.manifest)
            print(f"Python materialization baseline manifest sealed: {args.manifest}")
        elif args.operation == "import-record":
            guard_establishment(args.output, args.replace)
            record = _load(args.record)
            baseline = compact_baseline_from_record(record, args.baseline_name)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(baseline, indent=2) + "\n", encoding="utf-8")
            print(f"Python materialization baseline imported: {args.output}")
        elif args.operation == "establish":
            write_baseline(
                args.measurement,
                args.run_metadata,
                args.fixture_a,
                args.fixture_b,
                args.output,
                args.artifact_root,
                args.baseline_name,
                args.replace,
            )
            print(f"Python materialization baseline established: {args.output}")
        else:
            regressions = check_comparison(
                args.baseline,
                args.current,
                args.run_metadata,
                args.fixture_a,
                args.fixture_b,
                args.artifact_root,
                args.threshold,
                args.result_output,
                args.baseline_manifest,
            )
    except (OSError, TypeError, ValueError, KeyError, ZeroDivisionError) as error:
        parser.error(str(error))
    if args.operation != "compare":
        return 0
    if regressions:
        for case, change in regressions:
            print(f"REGRESSION {case}: median change {change:+.2%}")
        return 1
    print(f"Python materialization comparison passed at {args.threshold:.0%} threshold")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
