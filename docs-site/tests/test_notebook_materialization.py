"""Source reuse must preserve authored prose and invalidate changed calculations."""

import json
from pathlib import Path

from check_curriculum import check_evidence
from common import digest
from materialize_notebook_blocks import materialize_lesson, render_notebook_blocks
import pytest
from run_lesson_snippets import extract_blocks


def tagged_cell(identifier: str = "proof", code: str = "value = 2\nassert value == 2\n", **tag: str) -> dict:
    return {
        "cell_type": "code",
        "metadata": {"analyst_program": {"lesson": "2.6", "id": identifier, "role": "build", **tag}},
        "source": code.splitlines(keepends=True),
    }


def write_notebook(root: Path, cells: list[dict]) -> Path:
    path = root / "lab.ipynb"
    path.write_text(json.dumps({"cells": cells}))
    return path


def record() -> dict:
    return {"id": "2.6", "path": "lesson.mdx", "labs": ["lab.ipynb"]}


def region(identifier: str = "proof", role: str = "build", notebook: str = "lab.ipynb") -> str:
    return (
        f'<!-- notebook-block notebook="{notebook}" cell="{identifier}" role="{role}" -->\n'
        "replace only this region\n<!-- /notebook-block -->\n"
    )


def test_exact_cell_text_and_prose_survive_idempotent_materialization(tmp_path: Path) -> None:
    code = '# Keep comments and indentation.\ntext = "```"\nassert text == "```"\n'
    write_notebook(tmp_path, [tagged_cell(code=code)])
    before = "Intro with CRLF.\r\n" + region() + "\r\n<Exercise>Hand-authored prose.</Exercise>\r\n"
    rendered = render_notebook_blocks(before, record(), tmp_path)
    assert rendered.startswith("Intro with CRLF.\r\n")
    assert rendered.endswith("\r\n<Exercise>Hand-authored prose.</Exercise>\r\n")
    assert extract_blocks(rendered)[0].code == code
    assert "````python exec id=proof role=build" in rendered
    assert '{/* notebook-block notebook="lab.ipynb" cell="proof" role="build" */}' in rendered
    assert render_notebook_blocks(rendered, record(), tmp_path) == rendered


def test_source_drift_updates_displayed_code_and_invalidates_evidence(tmp_path: Path) -> None:
    source = tmp_path / "lesson.mdx"
    source.write_text(region())
    write_notebook(tmp_path, [tagged_cell()])
    assert materialize_lesson(record(), tmp_path, tmp_path)
    old_hash = digest(source)
    evidence = tmp_path / ".build/snippets/2.6.json"
    evidence.parent.mkdir(parents=True)
    evidence.write_text(
        json.dumps({
            "source_sha256": old_hash,
            "fixtures_sha256": "fixed",
            "status": "passed",
            "blocks": [{"id": "proof"}],
        })
    )
    write_notebook(tmp_path, [tagged_cell(code="value = 3\nassert value == 3\n")])
    with pytest.raises(ValueError, match="stale"):
        materialize_lesson(record(), tmp_path, tmp_path, check_only=True)
    assert digest(source) == old_hash
    materialize_lesson(record(), tmp_path, tmp_path)
    errors = check_evidence({**record(), "labs": []}, source, extract_blocks(source.read_text()), tmp_path, {}, "fixed")
    assert any("stale snippet evidence" in error for error in errors)
    assert "value = 3" in source.read_text()


@pytest.mark.parametrize("tag", [{"lesson": "2.7"}, {"role": "exercise"}])
def test_cell_metadata_must_match_lesson_and_role(tmp_path: Path, tag: dict) -> None:
    write_notebook(tmp_path, [tagged_cell(**tag)])
    with pytest.raises(ValueError, match="lesson/role"):
        render_notebook_blocks(region(), record(), tmp_path)


def test_sources_must_be_mapped_contained_code_cells(tmp_path: Path) -> None:
    write_notebook(tmp_path, [tagged_cell()])
    with pytest.raises(ValueError, match="not mapped"):
        render_notebook_blocks(region(), {**record(), "labs": []}, tmp_path)
    with pytest.raises(ValueError, match="remain inside"):
        render_notebook_blocks(region(notebook="../escape.ipynb"), record(), tmp_path)
    outside = tmp_path.parent / "escaped-notebook.ipynb"
    outside.write_text('{"cells":[]}')
    (tmp_path / "link.ipynb").symlink_to(outside)
    with pytest.raises(ValueError, match="remain inside"):
        render_notebook_blocks(region(notebook="link.ipynb"), {**record(), "labs": ["link.ipynb"]}, tmp_path)
    cell = tagged_cell()
    cell["cell_type"] = "markdown"
    write_notebook(tmp_path, [cell])
    with pytest.raises(ValueError, match="Python code"):
        render_notebook_blocks(region(), record(), tmp_path)


def test_duplicate_cells_includes_and_authored_ids_are_rejected(tmp_path: Path) -> None:
    write_notebook(tmp_path, [tagged_cell(), tagged_cell()])
    with pytest.raises(ValueError, match="duplicate analyst_program"):
        render_notebook_blocks(region(), record(), tmp_path)
    write_notebook(tmp_path, [tagged_cell()])
    with pytest.raises(ValueError, match="duplicate included"):
        render_notebook_blocks(region() + region(), record(), tmp_path)
    with pytest.raises(ValueError, match="duplicate executable"):
        render_notebook_blocks(region() + "```python exec id=proof\nassert True\n```\n", record(), tmp_path)


@pytest.mark.parametrize(
    "text",
    [
        '<!-- notebook-block notebook="lab.ipynb" cell="proof" role="build" -->\n',
        "<!-- /notebook-block -->\n",
        '<!-- notebook-block notebook="lab.ipynb" cell="proof" role="build"\n',
        '<!-- notebook-block notebook="lab.ipynb" cell="proof" role="build" -->\n' + region(),
    ],
)
def test_invalid_regions_never_overwrite_the_lesson(tmp_path: Path, text: str) -> None:
    write_notebook(tmp_path, [tagged_cell()])
    path = tmp_path / "lesson.mdx"
    path.write_text(text)
    with pytest.raises(ValueError, match="marker"):
        materialize_lesson(record(), tmp_path, tmp_path)
    assert path.read_text() == text


def test_setup_cells_are_displayed_but_build_and_each_exercise_need_checks(tmp_path: Path) -> None:
    write_notebook(
        tmp_path,
        [
            tagged_cell("setup", "import math\n"),
            tagged_cell(),
            tagged_cell("exercise", "assert math.sqrt(value + 2) == 2\n", role="exercise"),
        ],
    )
    text = region("setup") + region() + region("exercise", "exercise")
    assert [block.id for block in extract_blocks(render_notebook_blocks(text, record(), tmp_path))] == [
        "setup",
        "proof",
        "exercise",
    ]
    with pytest.raises(ValueError, match="build sequence"):
        render_notebook_blocks(region("setup"), record(), tmp_path)
    write_notebook(tmp_path, [tagged_cell(), tagged_cell("exercise", "print(value)\n", role="exercise")])
    with pytest.raises(ValueError, match="exercise source requires"):
        render_notebook_blocks(region() + region("exercise", "exercise"), record(), tmp_path)


def test_snippet_runner_materializes_before_execution(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import run_lesson_snippets as runner

    write_notebook(tmp_path, [tagged_cell(code="value = 41 + 1\nassert value == 42\nprint(value)\n")])
    path = tmp_path / "lesson.mdx"
    path.write_text(
        "---\nstatus: published\n---\n\n" + region() + '<ExecutedOutput lesson="2.6" block="proof" />\n'
    )
    monkeypatch.setattr(runner, "CONTENT", tmp_path)
    monkeypatch.setattr(runner, "NOTEBOOKS", tmp_path)
    monkeypatch.setattr(runner, "BUILD", tmp_path / ".build")
    result = runner.run_lesson(record(), timeout=30)
    assert result["status"] == "passed"
    assert result["blocks"][0]["stdout"] == "42\n"
    assert result["source_sha256"] == digest(path)
