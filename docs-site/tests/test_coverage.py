"""Publication fails if a required solution or its source mapping disappears."""

from copy import deepcopy

from check_coverage import validate_coverage


def fixture() -> tuple[list[dict], list[dict], dict[str, str]]:
    records = [{"id": "1.1"}]
    entries = [
        {
            "lesson_id": "1.1",
            "concepts": ["Exact money"],
            "authoring_finished_utc": "2026-09-03T04:00:00Z",
            "build_workflows": [{"notebook": "money.ipynb", "cell_id": "money", "status": "proven"}],
            "exercises": [
                {"notebook": "money.ipynb", "cell_id": "fx", "status": "proven", "original_requirement": "Reconcile FX"}
            ],
        }
    ]
    bodies = {
        "1.1": '{/* notebook-block notebook="money.ipynb" cell="money" role="build" */}\n<Exercise>{/* notebook-block notebook="money.ipynb" cell="fx" role="exercise" */}</Exercise>'
    }
    return records, entries, bodies


def test_missing_required_solution_fails_publication() -> None:
    records, entries, bodies = fixture()
    assert validate_coverage(records, entries, bodies) == []
    removed = deepcopy(entries)
    removed[0]["exercises"] = []
    assert any("coverage/source mismatch" in e for e in validate_coverage(records, removed, bodies))


def test_unproven_or_unmapped_workflow_fails_publication() -> None:
    records, entries, bodies = fixture()
    entries[0]["build_workflows"][0]["status"] = "authored"
    assert any("not proven" in e for e in validate_coverage(records, entries, bodies))
    entries[0]["build_workflows"][0]["status"] = "proven"
    entries[0]["build_workflows"][0]["cell_id"] = "nonexistent"
    assert any("coverage/source mismatch" in e for e in validate_coverage(records, entries, bodies))


def test_written_judgment_requires_a_solution_and_visible_exercise() -> None:
    records, entries, bodies = fixture()
    entries[0]["exercises"].append({
        "role": "prose",
        "status": "complete",
        "original_requirement": "Discuss model limits",
        "solution_and_assertions": "Complete written interpretation.",
    })
    assert any("absent" in e for e in validate_coverage(records, entries, bodies))
    bodies["1.1"] += "<Exercise>Complete written interpretation.</Exercise>"
    assert validate_coverage(records, entries, bodies) == []
