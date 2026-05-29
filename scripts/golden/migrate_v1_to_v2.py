#!/usr/bin/env python3
"""Migrate golden fixtures from `finstack.golden/1` to `finstack.golden/2`.

The v2 schema replaces the free-form `inputs` blob with strict, named sections:

    schema_version
    metadata        identity + provenance + valuation_date
    kind            "pricing" | "sabr_smile"
    <body>          pricing: model, market, instrument
                    sabr_smile: alpha, beta, nu, rho, shift?, forward,
                                time_to_expiry, strikes
    expected        raw source values (was expected_outputs)
    tolerances      one entry per expected metric

Dropped from v1: `inputs.metrics` (now derived from `expected` keys),
`inputs.source_reference`, `inputs.source_validation`, `inputs.actual_outputs`.
Any `source_reference.zero_metric_reasons` is folded into the matching
`tolerances[metric].tolerance_reason` when that reason is otherwise missing.

Usage:
    uv run scripts/golden/migrate_v1_to_v2.py [--check]

With `--check` the script reports what it would change without writing files.
"""

from __future__ import annotations

import argparse
from collections import OrderedDict
import json
from pathlib import Path
import sys

DATA_ROOT = Path(__file__).resolve().parents[2] / ("finstack/valuations/tests/golden/data")
V1 = "finstack.golden/1"
V2 = "finstack.golden/2"
SABR_DOMAIN = "volatility.sabr"

SABR_BODY_KEYS = ("alpha", "beta", "nu", "rho", "forward", "time_to_expiry", "strikes")


def metric_base(metric: str) -> str:
    """Return the base metric name before any `::` qualifier."""
    return metric.split("::", 1)[0]


def build_metadata(fixture: dict) -> OrderedDict[str, object]:
    """Lift identity, provenance, and valuation_date into a single block."""
    provenance = fixture["provenance"]
    inputs = fixture["inputs"]
    metadata: OrderedDict[str, object] = OrderedDict()
    metadata["name"] = fixture["name"]
    metadata["domain"] = fixture["domain"]
    metadata["description"] = fixture["description"]
    if "valuation_date" in inputs:
        metadata["valuation_date"] = inputs["valuation_date"]
    else:
        metadata["valuation_date"] = provenance["as_of"]
    metadata["source"] = provenance["source"]
    metadata["source_detail"] = provenance["source_detail"]
    metadata["captured_by"] = provenance["captured_by"]
    metadata["captured_on"] = provenance["captured_on"]
    metadata["last_reviewed_by"] = provenance["last_reviewed_by"]
    metadata["last_reviewed_on"] = provenance["last_reviewed_on"]
    metadata["review_interval_months"] = provenance["review_interval_months"]
    metadata["regen_command"] = provenance["regen_command"]
    metadata["screenshots"] = provenance.get("screenshots", [])
    return metadata


def fold_zero_metric_reasons(inputs: dict, tolerances: dict) -> None:
    """Move source_reference.zero_metric_reasons into tolerance_reason in place."""
    source_reference = inputs.get("source_reference")
    if not isinstance(source_reference, dict):
        return
    reasons = source_reference.get("zero_metric_reasons")
    if not isinstance(reasons, dict):
        return
    for metric, reason in reasons.items():
        entry = tolerances.get(metric)
        if entry is None:
            msg = f"zero_metric_reasons names unknown metric '{metric}'"
            raise ValueError(msg)
        existing = entry.get("tolerance_reason")
        if not (isinstance(existing, str) and existing.strip()):
            entry["tolerance_reason"] = reason


def derived_metrics(expected: dict) -> list[str]:
    """Reproduce the requested-metric list from expected keys (npv excluded)."""
    seen: list[str] = []
    for key in expected:
        base = metric_base(key)
        if base != "npv" and base not in seen:
            seen.append(base)
    return seen


def migrate(fixture: dict, path: Path) -> OrderedDict[str, object]:
    """Return the v2 form of a parsed v1 fixture."""
    inputs = fixture["inputs"]
    expected = fixture["expected_outputs"]
    tolerances = json.loads(json.dumps(fixture["tolerances"]))
    fold_zero_metric_reasons(inputs, tolerances)

    out: OrderedDict[str, object] = OrderedDict()
    out["schema_version"] = V2
    out["metadata"] = build_metadata(fixture)

    if fixture["domain"] == SABR_DOMAIN:
        out["kind"] = "sabr_smile"
        for key in ("alpha", "beta", "nu", "rho"):
            out[key] = inputs[key]
        if inputs.get("shift") is not None:
            out["shift"] = inputs["shift"]
        out["forward"] = inputs["forward"]
        out["time_to_expiry"] = inputs["time_to_expiry"]
        out["strikes"] = inputs["strikes"]
    else:
        out["kind"] = "pricing"
        out["model"] = inputs["model"]
        if "market_envelope" in inputs:
            out["market"] = OrderedDict([("kind", "envelope"), ("envelope", inputs["market_envelope"])])
        elif "market" in inputs:
            out["market"] = OrderedDict([("kind", "snapshot"), ("data", inputs["market"])])
        else:
            msg = f"{path}: pricing fixture has neither market nor market_envelope"
            raise ValueError(msg)
        out["instrument"] = inputs["instrument_json"]

        original = inputs.get("metrics", [])
        derived = derived_metrics(expected)
        extras = sorted({metric_base(m) for m in original} - set(derived))
        if extras:
            print(
                f"WARN {path.name}: dropping requested-but-unasserted metrics {extras}",
                file=sys.stderr,
            )

    out["expected"] = expected
    out["tolerances"] = tolerances
    return out


def iter_fixture_paths() -> list[Path]:
    """Collect every committed fixture JSON under the data root."""
    return sorted(p for p in DATA_ROOT.rglob("*.json") if "screenshots" not in p.parts)


def main() -> int:
    """Migrate every committed v1 fixture in place (or report with --check)."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="report changes without writing files",
    )
    args = parser.parse_args()

    changed = 0
    for path in iter_fixture_paths():
        fixture = json.loads(path.read_text(encoding="utf-8"))
        version = fixture.get("schema_version")
        if version == V2:
            continue
        if version != V1:
            print(f"SKIP {path}: unexpected schema_version {version!r}", file=sys.stderr)
            continue
        migrated = migrate(fixture, path)
        rendered = json.dumps(migrated, indent=2) + "\n"
        changed += 1
        if args.check:
            print(f"would migrate {path.relative_to(DATA_ROOT)}")
        else:
            path.write_text(rendered, encoding="utf-8")
            print(f"migrated {path.relative_to(DATA_ROOT)}")

    print(f"\n{changed} fixture(s) {'pending' if args.check else 'migrated'}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
