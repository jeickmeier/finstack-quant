"""Canonical snippet assertions require statement-level runtime coverage."""

import json
from pathlib import Path

import common
import pytest
import run_lesson_snippets as runner
from run_lesson_snippets import (
    Block,
    canonical_validation_manifest,
    run_process,
    validate_assertion_coverage,
)


def test_snippet_provenance_hashes_the_imported_common_module(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    site = tmp_path / "site"
    worker = site / "gen" / "snippet_worker.py"
    worker.parent.mkdir(parents=True)
    worker.write_text("# worker\n")
    monkeypatch.setattr(runner, "SITE", site)

    provenance = runner.snippet_provenance()

    assert provenance["common_sha256"] == runner.digest(Path(common.__file__).resolve())


def test_canonical_worker_records_stable_block_local_assertion_hits() -> None:
    block = Block(
        "proof",
        "build",
        "value = 42\nassert value == 42\nif value:\n    assert value > 0\n",
        20,
    )

    results = run_process("2.6", [block], ["proof"], 30, publish_assets=False, track_assertions=True)
    executed = validate_assertion_coverage([block], results, "2.6")
    manifest = canonical_validation_manifest([block], executed)

    assert results[0]["assertion_hits"] == ["proof:2:0", "proof:4:4"]
    assert manifest["assertion_coverage"] == [
        {"id": "proof:2:0", "line": 2, "column": 0, "block": "proof", "executed": True},
        {"id": "proof:4:4", "line": 4, "column": 4, "block": "proof", "executed": True},
    ]


def test_dead_canonical_assertion_is_rejected() -> None:
    block = Block(
        "proof",
        "build",
        "value = 42\nif False:\n    assert False\nassert value == 42\n",
        1,
    )
    results = run_process("2.6", [block], ["proof"], 30, publish_assets=False, track_assertions=True)

    with pytest.raises(RuntimeError, match=r"missing=\['proof:3:4'\]"):
        validate_assertion_coverage([block], results, "2.6")


def test_swallowed_assertion_failure_does_not_count_as_covered() -> None:
    block = Block(
        "proof",
        "build",
        "try:\n    assert False\nexcept AssertionError:\n    pass\nassert True\n",
        1,
    )
    results = run_process("2.6", [block], ["proof"], 30, publish_assets=False, track_assertions=True)

    with pytest.raises(RuntimeError, match=r"missing=\['proof:2:4'\]"):
        validate_assertion_coverage([block], results, "2.6")


def test_run_lesson_rejects_dead_assertion_without_replacing_last_good_assets(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, isolated_snippet_site: Path
) -> None:
    code = "value = 42\nif False:\n    assert False\nassert value == 42\nprint(value)\n"
    notebook = {
        "cells": [
            {
                "cell_type": "code",
                "metadata": {"analyst_program": {"lesson": "2.6", "id": "proof", "role": "build"}},
                "source": code.splitlines(keepends=True),
            }
        ]
    }
    (tmp_path / "lab.ipynb").write_text(json.dumps(notebook))
    lesson = tmp_path / "lesson.mdx"
    lesson.write_text(
        "---\nstatus: published\n---\n\n"
        '<!-- notebook-block notebook="lab.ipynb" cell="proof" role="build" -->\n'
        "replace\n"
        "<!-- /notebook-block -->\n"
        '<ExecutedOutput lesson="2.6" block="proof" />\n'
    )
    monkeypatch.setattr(runner, "CONTENT", tmp_path)
    monkeypatch.setattr(runner, "NOTEBOOKS", tmp_path)
    monkeypatch.setattr(runner, "BUILD", tmp_path / ".build")
    last_good = isolated_snippet_site / "public" / "snippet-assets" / "2.6" / "old" / "last-good.svg"
    last_good.parent.mkdir(parents=True)
    last_good.write_text("last good")

    with pytest.raises(RuntimeError, match=r"missing=\['proof:3:4'\]"):
        runner.run_lesson({"id": "2.6", "path": lesson.name, "labs": ["lab.ipynb"]}, timeout=30)

    assert last_good.read_text() == "last good"


def test_assertion_coverage_combines_build_and_fresh_exercise_runs() -> None:
    build = Block("setup", "build", "value = 42\nassert value == 42\n", 1)
    exercise = Block("exercise", "exercise", "if value:\n    assert value > 0\n", 10)

    build_results = run_process(
        "2.6",
        [build],
        ["setup"],
        30,
        publish_assets=False,
        track_assertions=True,
    )
    exercise_results = run_process(
        "2.6",
        [build, exercise],
        ["exercise"],
        30,
        publish_assets=False,
        track_assertions=True,
    )

    executed = validate_assertion_coverage([build, exercise], [*build_results, *exercise_results], "2.6")

    assert executed == {"setup:2:0", "exercise:2:4"}


def test_public_worker_result_has_no_assertion_coverage() -> None:
    block = Block("proof", "build", "value = 42\nprint(value)\n", 1)

    results = run_process("2.6", [block], ["proof"], 30, publish_assets=False)

    assert "assertion_hits" not in results[0]


@pytest.mark.parametrize("track_assertions", [False, True], ids=["public", "canonical"])
def test_worker_rejects_absolute_canonical_notebook_access(tmp_path: Path, track_assertions: bool) -> None:
    canonical = tmp_path / "canonical.ipynb"
    canonical.write_text('{"cells": []}')
    projected = tmp_path / "projected"
    projected.mkdir()
    code = (
        f"from pathlib import Path\ntry:\n    Path({str(canonical)!r}).read_text()\nexcept PermissionError:\n    pass\n"
    )
    block = Block("proof", "build", code, 1)

    with pytest.raises(RuntimeError, match="Notebook access escaped the declared projection"):
        run_process(
            "2.6",
            [block],
            ["proof"],
            30,
            projected,
            publish_assets=False,
            track_assertions=track_assertions,
        )


def test_canonical_projection_keeps_asserts_only_for_declared_notebook_closure(tmp_path: Path) -> None:
    dependency = tmp_path / "dependency.ipynb"
    dependency.write_text(json.dumps({"cells": [], "metadata": {}}))
    mapped = tmp_path / "mapped.ipynb"
    mapped.write_text(
        json.dumps({
            "cells": [{"cell_type": "code", "metadata": {}, "source": ["assert True\n"]}],
            "metadata": {"analyst_dependencies": [dependency.name]},
        })
    )
    unlisted = tmp_path / "unlisted.ipynb"
    unlisted.write_text(json.dumps({"cells": [], "metadata": {}}))
    record = {"id": "2.6", "labs": [mapped.name], "examples": []}

    with runner.canonical_notebook_root(record, tmp_path) as projected:
        assert "assert True" in (projected / mapped.name).read_text()
        assert (projected / dependency.name).is_file()
        assert not (projected / unlisted.name).exists()


def test_instrumentation_preserves_assertion_failure_behavior() -> None:
    block = Block("proof", "build", "assert 2 + 2 == 5\n", 1)

    with pytest.raises(RuntimeError, match="AssertionError"):
        run_process("2.6", [block], ["proof"], 30, publish_assets=False, track_assertions=True)
