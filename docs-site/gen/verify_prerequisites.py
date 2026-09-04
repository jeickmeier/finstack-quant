"""Verify the 33 supported workflow contracts against executed notebook copies."""

from __future__ import annotations

import argparse
import ast
import json
import sys

from build_labs import lab_report_errors
from check_curriculum import resolve_api
from common import (
    BUILD,
    NOTEBOOKS,
    SITE,
    cell_source,
    contained,
    curriculum,
    digest,
    fixture_digest,
    lab_entries_by_notebook,
    lesson_notebook_names,
    notebook_dependencies,
    read_lab_report,
    runtime_identity,
    write_json,
)


def inspect_cells(
    name: str, identifier: str, labs: dict, runtime: dict, fixtures: str, require_execution: bool
) -> tuple[list, int, list]:
    """Check a notebook's tagged cells and its recorded execution against source."""
    cells, assertion_count, errors = [], 0, []
    original = contained(NOTEBOOKS, name)
    source = json.loads(original.read_text())
    tagged = [
        cell
        for cell in source["cells"]
        if cell.get("metadata", {}).get("analyst_program", {}).get("lesson") == identifier
    ]
    if not tagged:
        return [], 0, []
    execution = labs.get(name, {})
    original_digest = digest(original)
    if require_execution and (
        execution.get("source_sha256") != original_digest
        or execution.get("fixtures_sha256") != fixtures
        or execution.get("runtime") != runtime
        or execution.get("dependencies_sha256", {}) != notebook_dependencies(name)
    ):
        errors.append(f"{identifier}: missing or stale notebook execution: {name}")
    copy = BUILD / "notebooks" / name
    executed = json.loads(copy.read_text()) if require_execution and copy.exists() else {"cells": []}
    output_cells = {cell.get("metadata", {}).get("analyst_program", {}).get("id"): cell for cell in executed["cells"]}
    for cell in tagged:
        tag = cell["metadata"]["analyst_program"]
        code = cell_source(cell)
        if cell["cell_type"] != "code":
            errors.append(f"{identifier}: tagged non-code cell {tag['id']}")
            continue
        assertion_count += sum(isinstance(node, ast.Assert) for node in ast.walk(ast.parse(code)))
        result = output_cells.get(tag["id"], {})
        if require_execution and (
            result.get("execution_count") is None
            or any(item.get("output_type") == "error" for item in result.get("outputs", []))
        ):
            errors.append(f"{identifier}: proof cell did not execute successfully: {name}:{tag['id']}")
        cells.append({
            "notebook": name,
            "id": tag["id"],
            "role": tag.get("role", "build"),
            "source_sha256": original_digest,
        })
    return cells, assertion_count, errors


def verify(require_execution: bool = True) -> dict:
    """Validate contract coverage, runtime names and immutable execution evidence."""
    records = {record["id"]: record for record in curriculum()}
    contracts = [
        entry for path in sorted((SITE / "contracts").glob("*.json")) for entry in json.loads(path.read_text())
    ]
    identifiers = [entry["lesson_id"] for entry in contracts]
    errors = []
    if len(identifiers) != len(set(identifiers)) or set(identifiers) != set(records):
        errors.append(
            f"Expected one contract per lesson; missing={sorted(set(records) - set(identifiers))}, extra={sorted(set(identifiers) - set(records))}"
        )
    lab_report = read_lab_report(BUILD)
    labs = lab_entries_by_notebook(lab_report)
    if require_execution:
        errors.extend(lab_report_errors(lab_report))
    runtime, fixtures = runtime_identity(), fixture_digest()
    verified = []
    for contract in contracts:
        identifier = contract["lesson_id"]
        if identifier not in records:
            continue
        record = records[identifier]
        allowed = set(lesson_notebook_names(record))
        errors.extend(
            f"{identifier}: missing contract field {field}"
            for field in (
                "notebook",
                "cell_ids",
                "imports",
                "models",
                "metrics",
                "returned_types",
                "expected_exceptions",
            )
            if field not in contract
        )
        if contract.get("notebook") not in allowed:
            errors.append(f"{identifier}: principal notebook is not mapped")
        for name in contract.get("imports", []):
            try:
                resolve_api(name)
            except (ImportError, AttributeError, ValueError) as error:
                errors.append(f"{identifier}: import {name}: {error}")
        cells, assertion_count = [], 0
        for name in sorted(allowed):
            found, count, issues = inspect_cells(name, identifier, labs, runtime, fixtures, require_execution)
            cells.extend(found)
            assertion_count += count
            errors.extend(issues)
        if not cells or not assertion_count:
            errors.append(f"{identifier}: principal workflow lacks executable assertions")
        if not set(contract.get("cell_ids", [])) <= {cell["id"] for cell in cells}:
            errors.append(f"{identifier}: declared principal cell IDs are absent from mapped source notebooks")
        verified.append({**contract, "proof_cells": cells, "assertions": assertion_count})
    if errors:
        raise ValueError("\n".join(errors))
    return {
        "status": "passed" if require_execution else "source_checked",
        "runtime": runtime,
        "fixtures_sha256": fixtures,
        "lessons": verified,
    }


def main() -> int:
    """Write evidence only after all requested prerequisite checks pass."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-only", action="store_true", help="Inspect mappings and imports without claiming an execution gate"
    )
    args = parser.parse_args()
    sys.path.insert(0, str(NOTEBOOKS))
    try:
        result = verify(not args.source_only)
    except (ValueError, OSError, SyntaxError) as error:
        print(error, file=sys.stderr)
        return 1
    write_json(BUILD / "prerequisites.json", result)
    print(f"Prerequisites {result['status']}: {len(result['lessons'])} lesson workflows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
