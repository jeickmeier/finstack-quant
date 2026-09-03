"""Check full-depth lesson coverage records against the executable source inventory."""

from __future__ import annotations

import json
import re

from common import CONTENT, SITE, curriculum


def declared_sources(identifier: str, entry: dict) -> tuple[set[tuple], list[str]]:
    """Validate the recorded source identities and written exercise solutions."""
    declared = set()
    errors = []
    for group, role in [("build_workflows", "build"), ("exercises", "exercise")]:
        for item in entry.get(group, []):
            if role == "exercise" and item.get("role") == "prose":
                if item.get("status") != "complete" or not item.get("solution_and_assertions"):
                    errors.append(f"{identifier}: written exercise solution is incomplete")
                if not item.get("original_requirement"):
                    errors.append(f"{identifier}: missing original written exercise requirement")
                continue
            name = item.get("notebook", item.get("planned_notebook"))
            cell = item.get("cell_id", item.get("planned_cell_id"))
            if not name or not cell:
                errors.append(f"{identifier}: missing source identity in {group}")
            declared.add((name, cell, role))
            if item.get("status") != "proven":
                errors.append(f"{identifier}/{cell}: coverage is not proven")
            if role == "exercise" and not item.get("original_requirement"):
                errors.append(f"{identifier}/{cell}: missing original exercise requirement")
    return declared, errors


def validate_coverage(records: list[dict], entries: list[dict], bodies: dict[str, str]) -> list[str]:
    """Require one concept/exercise inventory per lesson and exact displayed cell coverage."""
    errors = []
    expected = {record["id"] for record in records}
    indexed = {}
    for entry in entries:
        identifier = entry.get("lesson_id")
        if identifier in indexed:
            errors.append(f"{identifier}: duplicate coverage entry")
        indexed[identifier] = entry
    errors.extend(
        f"{identifier}: missing full-depth coverage inventory" for identifier in sorted(expected - indexed.keys())
    )
    errors.extend(f"{identifier}: unknown coverage lesson" for identifier in sorted(indexed.keys() - expected))
    for identifier in sorted(expected & indexed.keys()):
        entry = indexed[identifier]
        if not entry.get("concepts") or not entry.get("exercises"):
            errors.append(f"{identifier}: concepts and required exercises must be recorded")
        declared, source_errors = declared_sources(identifier, entry)
        errors.extend(source_errors)
        displayed = set(
            re.findall(
                r'notebook-block notebook="([^"]+)" cell="([^"]+)" role="([^"]+)"',
                bodies.get(identifier, ""),
            )
        )
        if declared != displayed:
            errors.append(
                f"{identifier}: coverage/source mismatch; missing={displayed - declared}; extra={declared - displayed}"
            )
        if len(re.findall(r"<Exercise\b", bodies.get(identifier, ""))) < len(entry.get("exercises", [])):
            errors.append(f"{identifier}: a required exercise is absent from the lesson")
        if not entry.get("authoring_finished_utc"):
            errors.append(f"{identifier}: authoring completion has not been recorded")
    return errors


def main() -> int:
    """Fail publication if a required inventory or executable solution is missing."""
    records = curriculum()
    entries = [entry for path in sorted((SITE / "coverage").glob("*.json")) for entry in json.loads(path.read_text())]
    errors = validate_coverage(records, entries, {r["id"]: (CONTENT / r["path"]).read_text() for r in records})
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Full-depth coverage inventories passed: {len(records)} lessons.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
