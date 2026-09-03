"""Validate lesson dependencies, source mappings and publication evidence."""

from __future__ import annotations

import argparse
import importlib
import json
from pathlib import Path
import re
import sys

from common import (
    BUILD,
    ID_PATTERN,
    LAB_EXECUTION_POLICY,
    NOTEBOOKS,
    REPO,
    SITE,
    contained,
    curriculum,
    digest,
    fixture_digest,
    frontmatter,
    lesson_notebooks,
    runtime_identity,
    write_json,
)
from materialize_notebook_blocks import materialize_lesson
from run_lesson_snippets import extract_blocks, validate_output_references


def _dependency_closure(graph: dict[str, list[str]], requirements: list[str]) -> set[str]:
    """Return all reachable prerequisites, including unknown IDs for diagnostics."""
    pending = list(requirements)
    reached = set()
    while pending:
        identifier = pending.pop()
        if identifier not in reached:
            reached.add(identifier)
            pending.extend(graph.get(identifier, []))
    return reached


def _validate_capstone(capstone: dict, graph: dict[str, list[str]]) -> list[str]:
    """Require the common baseline and the full chosen track without the other track."""
    errors = []
    if capstone.get("requires") != ["4.5"] or set(capstone.get("variants", {})) != {"credit", "volatility"}:
        errors.append("Capstone requires lesson 4.5 plus its chosen track variant")
    for variant, own, other in [("credit", "C", "V"), ("volatility", "V", "C")]:
        requirements = capstone.get("variants", {}).get(variant, [])
        predecessors = _dependency_closure(graph, [*capstone.get("requires", []), *requirements])
        required = {identifier for identifier in graph if identifier.startswith(own)}
        missing = required - predecessors
        if missing:
            errors.append(f"Capstone {variant}: missing track prerequisites {sorted(missing)}")
        if any(identifier.startswith(other) for identifier in predecessors):
            errors.append(f"Capstone {variant}: must not require the other specialist track")
    return errors


def validate_dependencies(records: list[dict]) -> list[str]:
    """Check dependency closure, independent tracks and complete capstone variants."""
    errors = []
    ids = [record["id"] for record in records]
    if len(ids) != len(set(ids)):
        errors.append("Duplicate lesson IDs")
    graph = {
        record["id"]: [
            *record.get("requires", []),
            *(item for requirements in record.get("variants", {}).values() for item in requirements),
        ]
        for record in records
    }
    active, done = set(), set()

    def visit(identifier: str) -> None:
        if identifier in active:
            errors.append(f"Dependency cycle at {identifier}")
            return
        if identifier in done:
            return
        active.add(identifier)
        for requirement in graph[identifier]:
            if requirement not in graph:
                errors.append(f"{identifier}: unknown prerequisite {requirement}")
            else:
                visit(requirement)
        active.remove(identifier)
        done.add(identifier)

    for identifier in graph:
        visit(identifier)

    for record in records:
        identifier = record["id"]
        predecessors = _dependency_closure(graph, graph[identifier])
        if identifier.startswith("C") and any(item.startswith("V") for item in predecessors):
            errors.append(f"{record['id']}: credit lessons must not require the volatility track")
        if identifier.startswith("V") and any(item.startswith("C") for item in predecessors):
            errors.append(f"{record['id']}: volatility lessons must not require the credit track")
        if identifier[:1] in {"1", "2", "3", "4"} and any(item.startswith(("C", "V")) for item in predecessors):
            errors.append(f"{identifier}: common lessons must not require a specialist track")
    capstone = next((record for record in records if record["id"] == "capstone"), None)
    if capstone:
        errors.extend(_validate_capstone(capstone, graph))
    return errors


def reference_anchors(text: str) -> set[str]:
    """Collect explicit and standard Markdown heading anchors."""
    anchors = set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', text))
    for heading in re.findall(r"^#{1,6}\s+(.+)$", text, re.M):
        slug = re.sub(r"[^\w\s-]", "", heading.lower()).strip().replace(" ", "-")
        anchors.add(slug)
    return anchors


def resolve_api(name: str) -> None:
    """Resolve a declared Python symbol against the installed extension."""
    parts = name.split(".")
    for count in range(len(parts), 0, -1):
        try:
            value = importlib.import_module(".".join(parts[:count]))
        except ModuleNotFoundError as error:
            if error.name != ".".join(parts[:count]) and not ".".join(parts[:count]).startswith(f"{error.name}."):
                raise
            continue
        for part in parts[count:]:
            value = getattr(value, part)
        return
    raise ValueError(f"Cannot resolve API symbol {name}")


def check_metadata(
    record: dict, metadata: dict, references: set[str], fixture_ids: set[str], check_api: bool
) -> list[str]:
    """Validate presentation fields, supported symbols and fixture identities."""
    errors = []
    identifier = record["id"]
    required = ("id", "title", "description", "desk_question", "references", "api", "status")
    errors.extend(f"{identifier}: missing frontmatter {field}" for field in required if field not in metadata)
    if metadata.get("id") != identifier:
        errors.append(f"{identifier}: frontmatter ID differs from manifest")
    if metadata.get("status") not in {"draft", "published"}:
        errors.append(f"{identifier}: invalid publication status")
    errors.extend(
        f"{identifier}: unknown fixture {fixture}"
        for fixture in record.get("fixtures", [])
        if fixture not in fixture_ids
    )
    errors.extend(
        f"{identifier}: missing REFERENCES anchor {reference}"
        for reference in metadata.get("references", [])
        if reference.rsplit("#", 1)[-1] not in references
    )
    if check_api:
        for name in metadata.get("api", []):
            try:
                resolve_api(name)
            except (ImportError, AttributeError, ValueError) as error:
                errors.append(f"{identifier}: {name}: {error}")
    return errors


def check_evidence(
    record: dict, path: Path, blocks: list, site: Path, lab_evidence: dict, fixture_hash: str
) -> list[str]:
    """Reject missing or stale execution output for snippets and companion labs."""
    identifier = record["id"]
    from build_labs import notebook_dependencies

    errors = []
    for notebook in record.get("labs", []):
        lab = lab_evidence.get(notebook, {})
        original = contained(NOTEBOOKS, notebook)
        if original.is_file() and (
            lab.get("source_sha256") != digest(original)
            or lab.get("fixtures_sha256") != fixture_hash
            or lab.get("runtime") != runtime_identity()
            or lab.get("dependencies_sha256", {}) != notebook_dependencies(notebook)
        ):
            errors.append(f"{identifier}: absent or stale executed lab evidence: {notebook}")
    output = site / ".build" / "snippets" / f"{identifier}.json"
    if not output.is_file():
        return [*errors, f"{identifier}: no executed snippet evidence"]
    captured = json.loads(output.read_text())
    if (
        captured.get("status") != "passed"
        or captured.get("source_sha256") != digest(path)
        or captured.get("fixtures_sha256") != fixture_hash
        or captured.get("notebooks_sha256") != lesson_notebooks(record)
        or captured.get("runtime") != runtime_identity()
    ):
        errors.append(f"{identifier}: missing, failed or stale snippet evidence")
    if {b["id"] for b in captured.get("blocks", [])} != {b.id for b in blocks}:
        errors.append(f"{identifier}: output coverage differs from executable blocks")
    return errors


def check(site: Path = SITE, check_api: bool = False, require_evidence: bool = False) -> tuple[list[str], list[dict]]:
    """Validate the authored curriculum and return its frontend projection."""
    import tomllib

    records = curriculum(site)
    manifest = tomllib.loads((site / "curriculum.toml").read_text())
    fixture_ids = set(manifest.get("fixtures", []))
    errors = validate_dependencies(records)
    projected = []
    lab_path = site / ".build" / "labs.json"
    lab_report = json.loads(lab_path.read_text()) if lab_path.exists() else {}
    lab_evidence = {item["notebook"]: item for item in lab_report.get("labs", [])}
    if require_evidence and lab_report.get("execution_policy") != LAB_EXECUTION_POLICY:
        errors.append("Lab evidence was not produced by the strict warning and stderr policy")
    references = reference_anchors((REPO / "docs" / "REFERENCES.md").read_text())
    fixture_hash = fixture_digest()
    expected_ids = (
        {f"1.{n}" for n in range(1, 6)}
        | {f"2.{n}" for n in range(1, 9)}
        | {f"3.{n}" for n in range(1, 7)}
        | {f"4.{n}" for n in range(1, 6)}
        | {f"C{n}" for n in range(1, 6)}
        | {f"V{n}" for n in range(1, 4)}
        | {"capstone"}
    )
    if {r["id"] for r in records} != expected_ids:
        errors.append("The curriculum must contain exactly the agreed 33 stable lesson IDs")
    for record in records:
        identifier = record["id"]
        try:
            if not ID_PATTERN.fullmatch(identifier):
                raise ValueError(f"Invalid lesson ID {identifier}")
            path = contained(site / "content" / "learn", record["path"])
            materialize_lesson(record, content_root=site / "content" / "learn", notebook_root=NOTEBOOKS)
            metadata, body = frontmatter(path)
            errors.extend(check_metadata(record, metadata, references, fixture_ids, check_api))
            for notebook in [*record.get("labs", []), *record.get("examples", [])]:
                if not contained(NOTEBOOKS, notebook).is_file():
                    errors.append(f"{identifier}: missing notebook {notebook}")
            blocks = extract_blocks(path.read_text(), str(path))
            validate_output_references(path.read_text(), identifier, blocks, str(path))
            if metadata.get("status") == "published":
                if not record.get("labs"):
                    errors.append(f"{identifier}: published lesson has no lab")
                sections = ["DeskContext", "Standard", "BuildIt", "LabCard", "Exercise", "CheckYourself"]
                positions = [body.find(f"<{section}") for section in sections]
                if min(positions) < 0 or positions != sorted(positions):
                    errors.append(f"{identifier}: missing or out-of-order required lesson sections")
            if metadata.get("status") == "published" or require_evidence:
                errors.extend(check_evidence(record, path, blocks, site, lab_evidence, fixture_hash))
            projected.append({**record, **metadata})
        except (ValueError, TypeError, OSError, KeyError, SyntaxError) as error:
            errors.append(f"{identifier}: {error}")
    return errors, projected


def main() -> int:
    """Validate the manifest and emit the site reader's generated JSON."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-api", action="store_true")
    parser.add_argument("--require-evidence", action="store_true")
    args = parser.parse_args()
    errors, records = check(check_api=args.check_api, require_evidence=args.require_evidence)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    write_json(BUILD / "curriculum.json", {"lessons": records})
    referenced = {p for r in records for p in [*r.get("labs", []), *r.get("examples", [])]}
    unused = sorted(
        str(p.relative_to(NOTEBOOKS))
        for p in NOTEBOOKS.rglob("*.ipynb")
        if str(p.relative_to(NOTEBOOKS)) not in referenced and ".ipynb_checkpoints" not in p.parts
    )
    print(f"Curriculum valid: {len(records)} lessons; {len(unused)} unused notebooks (informational)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
