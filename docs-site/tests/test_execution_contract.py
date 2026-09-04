"""Exercise real failure propagation, replay isolation and publication contracts."""

import ast
from copy import deepcopy
import json
from pathlib import Path
import shutil
import tomllib
from types import ModuleType

from check_curriculum import (
    check_metadata,
    validate_book_stage_calls,
    validate_dependencies,
    validate_fixture_progression,
    validate_fixture_sources,
    validate_lab_stage_exposure,
    validate_shared_source_usage,
    validate_track_fixture_calls,
)
from check_links import check as check_links
from common import NOTEBOOKS, contained, curriculum, lesson_notebook_names
import nbformat
from publication_source import notebook_assertion_count
import pytest
from run_lesson_snippets import Block, extract_blocks, run_process, validate_output_references


def test_publication_entrypoint_rebuilds_current_python_extension() -> None:
    """A release build cannot reuse a native extension from an older checkout."""
    manifest = tomllib.loads((Path(__file__).parents[2] / "mise.toml").read_text())
    commands = manifest["tasks"]["docs-site-build"]["run"]
    assert commands.index("mise run python-sync") < commands.index("mise run python-build")
    assert commands.index("mise run python-build") < commands.index("docs-site/gen/build_site.py")


def test_order_and_fresh_exercise_processes() -> None:
    baseline = Block("baseline", "build", "items = [1]\nassert items == [1]\n", 1)
    first = Block("first", "exercise", "items.append(2)\nassert items == [1,2]\nprint(items)\n", 10)
    second = Block("second", "exercise", "assert items == [1]\nprint(items)\n", 20)
    assert run_process("test", [baseline, first], ["first"], 30)[0]["stdout"].strip() == "[1, 2]"
    assert run_process("test", [baseline, second], ["second"], 30)[0]["stdout"].strip() == "[1]"


def test_numerical_failure_is_a_build_failure() -> None:
    with pytest.raises(RuntimeError, match="AssertionError"):
        run_process(
            "test", [Block("fails", "build", "assert 2 + 2 == 5, 'deliberate numerical failure'\n", 1)], ["fails"], 30
        )


def test_optimized_environment_cannot_disable_checks(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("PYTHONOPTIMIZE", "1")
    with pytest.raises(RuntimeError, match="AssertionError"):
        run_process("test", [Block("optimized", "build", "assert 2 + 2 == 5\n", 1)], ["optimized"], 30)


def test_python_warning_is_a_snippet_failure() -> None:
    with pytest.raises(RuntimeError, match="visible warning"):
        run_process(
            "test",
            [Block("warning", "build", "import warnings\nwarnings.warn('visible warning', UserWarning)\n", 1)],
            ["warning"],
            30,
        )


def test_stderr_is_a_snippet_failure() -> None:
    with pytest.raises(RuntimeError, match="plain stderr diagnostic"):
        run_process(
            "test",
            [Block("stderr", "build", "import sys\nprint('plain stderr diagnostic', file=sys.stderr)\n", 1)],
            ["stderr"],
            30,
        )


def test_asset_capture_preserves_build_state_and_exercise_baseline(isolated_snippet_site: Path) -> None:
    create = Block(
        "create-file",
        "build",
        "from pathlib import Path\nPath('ledger.csv').write_text('42')\nassert Path('ledger.csv').is_file()\n",
        1,
    )
    read = Block("read-file", "build", "assert Path('ledger.csv').read_text() == '42'\n", 10)
    result = run_process("test", [create, read], ["create-file", "read-file"], 30)
    assert len(result[0]["assets"]) == 1
    captured = isolated_snippet_site / "public" / result[0]["assets"][0].lstrip("/")
    assert captured.read_text() == "42"
    assert result[1]["assets"] == []
    exercise = Block(
        "use-ledger",
        "exercise",
        "assert Path('ledger.csv').read_text() == '42'\nPath('solution.csv').write_text('84')\n",
        20,
    )
    result = run_process("test", [create, read, exercise], ["use-ledger"], 30)
    assert len(result[0]["assets"]) == 1
    assert result[0]["assets"][0].endswith("/solution.csv")
    captured = isolated_snippet_site / "public" / result[0]["assets"][0].lstrip("/")
    assert captured.read_text() == "84"


def test_figure_capture_preserves_active_figure(isolated_snippet_site: Path) -> None:
    setup = Block(
        "figures",
        "build",
        "import matplotlib.pyplot as plt\na = plt.figure(1)\nb = plt.figure(2)\nplt.figure(1)\nassert plt.gcf() is a\n",
        1,
    )
    continue_plot = Block(
        "continue-figure",
        "build",
        "assert plt.gcf() is a\nplt.plot([0,1],[0,1])\nassert len(a.axes) == 1\nassert len(b.axes) == 0\n",
        10,
    )
    result = run_process("test", [setup, continue_plot], ["figures", "continue-figure"], 30)
    assert len(result[0]["assets"]) == 2
    assert len(result[1]["assets"]) == 1
    for block in result:
        for asset in block["assets"]:
            assert (isolated_snippet_site / "public" / asset.lstrip("/")).read_bytes().startswith(b"\x89PNG\r\n\x1a\n")


def test_cli_failure_removes_stale_evidence(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import run_lesson_snippets as runner

    source = tmp_path / "lesson.mdx"
    source.write_text(
        "---\nstatus: published\n---\n\n```python exec id=deliberate-failure\nassert 2 + 2 == 5\n```\n"
        '<ExecutedOutput lesson="1.1" block="deliberate-failure" />\n'
    )
    output = tmp_path / "snippets" / "1.1.json"
    output.parent.mkdir()
    output.write_text('{"status":"passed"}')
    monkeypatch.setattr(runner, "CONTENT", tmp_path)
    monkeypatch.setattr(runner, "BUILD", tmp_path)
    monkeypatch.setattr(runner, "curriculum", lambda: [{"id": "1.1", "path": source.name}])
    monkeypatch.setattr("sys.argv", ["run_lesson_snippets.py", "--require-all"])
    assert runner.main() == 1
    assert not output.exists()


def test_unknown_fixture_and_reference_are_rejected() -> None:
    metadata = {
        "id": "1.1",
        "title": "Money",
        "description": "Money",
        "desk_question": "How exact?",
        "references": ["#missing"],
        "api": [],
        "status": "draft",
    }
    errors = check_metadata({"id": "1.1", "fixtures": ["typo"]}, metadata, set(), {"foundations"}, False)
    assert any("unknown fixture typo" in error for error in errors)
    assert any("missing REFERENCES anchor" in error for error in errors)


def test_fixtures_must_be_introduced_once_before_use() -> None:
    records = [
        {"id": "1.1", "requires": [], "fixtures": ["foundations"], "introduces": ["foundations"]},
        {"id": "1.2", "requires": ["1.1"], "fixtures": ["foundations"]},
        {"id": "2.3", "requires": ["1.2"], "fixtures": ["rates"], "introduces": ["rates"]},
    ]
    assert validate_fixture_progression(records, {"foundations", "rates"}) == []
    records[0]["fixtures"] = ["foundations", "rates"]
    errors = validate_fixture_progression(records, {"foundations", "rates"})
    assert any("consumed before its introduction" in error for error in errors)
    records[0]["introduces"].append("rates")
    errors = validate_fixture_progression(records, {"foundations", "rates"})
    assert any("introduced by both" in error for error in errors)


def test_fixture_source_map_is_complete_and_contained(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import check_curriculum

    shared = tmp_path / "_shared"
    shared.mkdir()
    (shared / "book.py").write_text("# fixture source\n")
    monkeypatch.setattr(check_curriculum, "NOTEBOOKS", tmp_path)
    assert validate_fixture_sources({"fixture_sources": {"foundations": ["_shared/book.py"]}}, {"foundations"}) == []
    errors = validate_fixture_sources(
        {"fixture_sources": {"foundations": ["../book.py"], "unknown": ["_shared/book.py"]}},
        {"foundations"},
    )
    assert any("source map differs" in error for error in errors)
    assert any("invalid fixture source" in error for error in errors)


def test_every_shared_python_source_requires_fixture_ownership(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import check_curriculum

    shared = tmp_path / "_shared"
    shared.mkdir()
    (shared / "owned.py").write_text("VALUE = 1\n")
    (shared / "unowned.py").write_text("VALUE = 2\n")
    monkeypatch.setattr(check_curriculum, "NOTEBOOKS", tmp_path)

    errors = validate_fixture_sources(
        {"fixture_sources": {"foundations": ["_shared/owned.py"]}},
        {"foundations"},
    )

    assert any("missing=['_shared/unowned.py']" in error for error in errors)


def test_shared_source_import_must_follow_its_fixture_introduction(tmp_path: Path) -> None:
    shared = tmp_path / "_shared"
    shared.mkdir()
    (shared / "__init__.py").write_text("")
    (shared / "later.py").write_text("VALUE = 42\n")
    notebook = nbformat.v4.new_notebook(
        cells=[nbformat.v4.new_code_cell("from _shared.later import VALUE\nprint(VALUE)\n")],
        metadata={"analyst_dependencies": []},
    )
    nbformat.write(notebook, tmp_path / "early.ipynb")
    records = [
        {"id": "1.1", "requires": [], "fixtures": ["foundations"], "labs": ["early.ipynb"]},
        {"id": "2.1", "requires": [], "fixtures": ["later"]},
    ]
    manifest = {
        "fixture_sources": {
            "foundations": ["_shared/__init__.py"],
            "later": ["_shared/later.py"],
        }
    }

    errors = validate_shared_source_usage(records, manifest, tmp_path)
    assert any("imports _shared/later.py before fixture sources ['later']" in error for error in errors)

    records[0]["requires"] = ["2.1"]
    assert validate_shared_source_usage(records, manifest, tmp_path) == []


def test_only_scale_notebook_with_shared_imports_is_execution_only() -> None:
    records = curriculum()
    record_by_id = {record["id"]: record for record in records}
    owners = {notebook: record["id"] for record in records for notebook in lesson_notebook_names(record)}
    unowned = []
    tag_conflicts = []
    for path in NOTEBOOKS.rglob("*.ipynb"):
        notebook = json.loads(path.read_text())
        relative = str(path.relative_to(NOTEBOOKS))
        imports_shared = False
        tags = set()
        for cell in notebook.get("cells", []):
            if cell.get("cell_type") != "code":
                continue
            source = "".join(cell.get("source", []))
            tree = ast.parse(source, filename=relative)
            imports_shared |= any(
                (isinstance(node, ast.ImportFrom) and (node.module or "").startswith("_shared"))
                or (
                    isinstance(node, ast.Import)
                    and any(alias.name == "_shared" or alias.name.startswith("_shared.") for alias in node.names)
                )
                for node in ast.walk(tree)
            )
            tag = cell.get("metadata", {}).get("analyst_program", {})
            if tag.get("lesson"):
                tags.add(tag["lesson"])
        if imports_shared and relative not in owners:
            unowned.append(relative)
        if any(
            lesson not in record_by_id or relative not in lesson_notebook_names(record_by_id[lesson]) for lesson in tags
        ):
            tag_conflicts.append((relative, sorted(tags)))
    assert unowned == ["05_portfolio/multi_asset_portfolio_at_scale.ipynb"]
    assert tag_conflicts == []


def test_fences_require_unique_ids_and_assertion_free_publication() -> None:
    text = "```python exec id=proof\nprint(1)\n```\n"
    assert extract_blocks(text)[0].id == "proof"
    with pytest.raises(ValueError, match="duplicate"):
        extract_blocks(text + text)
    with pytest.raises(ValueError, match="learner-facing Python"):
        extract_blocks("```python exec id=checked\nassert True\n```\n")
    assert not extract_blocks("```rust display-only\nassert!(true);\n```\n")


@pytest.mark.parametrize("language", ["python", "python3", "py"])
def test_displayed_python_cannot_bypass_execution(language: str) -> None:
    visible = (
        "```python exec id=proof\nvalue = True\n```\n"
        f"```{language}\nfrom nonexistent_finance_api import UnsupportedPricer\n```\n"
    )
    with pytest.raises(ValueError, match="displayed Python must use"):
        extract_blocks(visible)


def test_output_references_cover_every_block_and_ignore_displayed_literals() -> None:
    text = (
        '```python exec id=proof\ntext = \'<ExecutedOutput lesson="other" block="literal" />\'\n```\n'
        '<ExecutedOutput\n block="proof"\n lesson="1.1"\n />\n'
        '{/* <ExecutedOutput lesson="other" block="comment" /> */}\n'
    )
    validate_output_references(text, "1.1", extract_blocks(text))


@pytest.mark.parametrize(
    ("references", "message"),
    [
        ("", "missing=.*proof"),
        ('<ExecutedOutput lesson="1.1" block="typo" />', "unknown=.*typo"),
        ('<ExecutedOutput lesson="1.2" block="proof" />', "own lesson 1.1"),
        ('<ExecutedOutput lesson="1.1" block="proof" />' * 2, "repeated=.*proof"),
        ('<ExecutedOutput lesson="1.1" block={selected} />', "literal lesson and block"),
        ('<ExecutedOutput lesson="1.1" block="proof" {...overrides} />', "literal lesson and block"),
        ('<ExecutedOutput lesson="1.1" block="proof"', "malformed"),
    ],
)
def test_missing_or_wrong_output_reference_is_rejected(references: str, message: str) -> None:
    text = "```python exec id=proof\nvalue = True\n```\n" + references
    with pytest.raises(ValueError, match=message):
        validate_output_references(text, "1.1", extract_blocks(text))


def test_output_reference_failure_removes_stale_evidence(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import run_lesson_snippets as runner

    notebook = tmp_path / "lab.ipynb"
    notebook.write_text(
        json.dumps({
            "cells": [
                {
                    "cell_type": "code",
                    "metadata": {"analyst_program": {"lesson": "1.1", "id": "proof", "role": "build"}},
                    "source": "value = True\nassert value\n",
                }
            ]
        })
    )
    source = tmp_path / "lesson.mdx"
    source.write_text(
        "---\nstatus: published\n---\n"
        '<!-- notebook-block notebook="lab.ipynb" cell="proof" role="build" -->\n'
        "stale\n<!-- /notebook-block -->\n"
        '<ExecutedOutput lesson="1.1" block="typo" />\n'
    )
    output = tmp_path / "snippets/1.1.json"
    output.parent.mkdir()
    output.write_text('{"status":"passed"}')
    monkeypatch.setattr(runner, "CONTENT", tmp_path)
    monkeypatch.setattr(runner, "NOTEBOOKS", tmp_path)
    monkeypatch.setattr(runner, "BUILD", tmp_path)
    with pytest.raises(ValueError, match=r"unknown=.*typo"):
        runner.run_lesson({"id": "1.1", "path": source.name, "labs": ["lab.ipynb"]})
    assert not output.exists()


def test_dependencies_reject_missing_cycle_and_cross_track() -> None:
    errors = validate_dependencies([{"id": "C1", "requires": ["V1"]}, {"id": "V1", "requires": ["C1", "missing"]}])
    assert any("cycle" in item for item in errors)
    assert any("unknown" in item for item in errors)
    assert any("credit lessons" in item for item in errors)
    assert any("volatility lessons" in item for item in errors)


def independent_tracks() -> list[dict]:
    return [
        {"id": "4.1", "requires": []},
        {"id": "4.3", "requires": ["4.1"]},
        {"id": "4.5", "requires": ["4.3"]},
        {"id": "C1", "requires": ["4.3"]},
        {"id": "C2", "requires": ["C1"]},
        {"id": "V1", "requires": ["4.1"]},
        {"id": "V2", "requires": ["V1"]},
        {
            "id": "capstone",
            "requires": ["4.5"],
            "labs": [],
            "variants": {"credit": ["C2"], "volatility": ["V2"]},
            "variant_labs": {"credit": ["credit.ipynb"], "volatility": ["volatility.ipynb"]},
        },
    ]


def test_tracks_are_independent_through_the_whole_dependency_chain() -> None:
    records = independent_tracks()
    assert validate_dependencies(records) == []
    records[1]["requires"].append("V1")
    errors = validate_dependencies(records)
    assert any("4.3: common lessons" in item for item in errors)
    assert any("C1: credit lessons" in item for item in errors)
    assert not any("cycle" in item for item in errors)


def test_capstone_variant_requires_complete_own_track_only() -> None:
    records = independent_tracks()
    missing_mapping = deepcopy(records)
    missing_mapping[-1]["variant_labs"].pop("volatility")
    assert any("labs must be mapped explicitly" in item for item in validate_dependencies(missing_mapping))
    swapped = deepcopy(records)
    swapped[-1]["variants"] = {"credit": ["V2"], "volatility": ["C2"]}
    errors = validate_dependencies(swapped)
    assert any("Capstone credit: must not require the other" in item for item in errors)
    assert any("Capstone volatility: must not require the other" in item for item in errors)
    incomplete = deepcopy(records)
    incomplete[-1]["variants"]["credit"] = ["C1"]
    assert any(
        "Capstone credit: missing track prerequisites ['C2']" in item for item in validate_dependencies(incomplete)
    )
    records[-1]["variants"]["credit"] = ["C1", "C2"]
    assert validate_dependencies(records) == []


def test_track_fixture_calls_require_declared_dynamic_and_structured_families(tmp_path: Path) -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell("from _shared import analyst_tracks as tracks\n"),
            nbformat.v4.new_code_cell(
                "stage = selected_stage\ntracks.build_market(stage)\ntracks.structured_index_inputs()\n",
                metadata={"analyst_program": {"lesson": "2.8", "id": "track-calls", "role": "build"}},
            ),
        ]
    )
    nbformat.write(notebook, tmp_path / "track.ipynb")
    records = [
        {
            "id": "2.8",
            "requires": [],
            "fixtures": ["base", "common", "credit", "volatility", "waterfall"],
            "labs": ["track.ipynb"],
        }
    ]
    assert validate_track_fixture_calls(records, tmp_path) == []

    records[0]["fixtures"] = ["base", "common", "credit", "waterfall"]
    errors = validate_track_fixture_calls(records, tmp_path)
    assert any("build_market" in error and "volatility" in error for error in errors)

    records[0]["fixtures"] = ["base", "common", "credit", "volatility"]
    errors = validate_track_fixture_calls(records, tmp_path)
    assert any("build_market" in error and "waterfall" in error for error in errors)
    assert any("structured_index_inputs" in error and "waterfall" in error for error in errors)


def test_track_core_stage_cannot_bypass_common_fixture_introduction(tmp_path: Path) -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell("from _shared import analyst_tracks as tracks\n"),
            nbformat.v4.new_code_cell(
                'book = tracks.build_book("vol", core_stage="common")\n',
                metadata={"analyst_program": {"lesson": "V1", "id": "vol-book", "role": "build"}},
            ),
        ]
    )
    nbformat.write(notebook, tmp_path / "vol.ipynb")
    records = [
        {"id": "4.3", "requires": [], "fixtures": ["common"]},
        {"id": "V1", "requires": [], "fixtures": ["volatility", "base"], "labs": ["vol.ipynb"]},
    ]

    fixture_errors = validate_track_fixture_calls(records, tmp_path)
    exposure_errors = validate_lab_stage_exposure(records, tmp_path)
    assert any("requires fixtures ['common']" in error for error in fixture_errors)
    assert any("before fixtures ['common'] are available" in error for error in exposure_errors)

    records[1]["requires"] = ["4.3"]
    records[1]["fixtures"].append("common")
    assert validate_track_fixture_calls(records, tmp_path) == []
    assert validate_lab_stage_exposure(records, tmp_path) == []


def test_book_stage_calls_require_stage_owner_in_dependency_closure(tmp_path: Path) -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell("from _shared.analyst_book import build_book as make_book\n"),
            nbformat.v4.new_code_cell(
                'book = make_book("common")\n',
                metadata={"analyst_program": {"lesson": "4.5", "id": "common-book", "role": "build"}},
            ),
        ]
    )
    nbformat.write(notebook, tmp_path / "book.ipynb")
    records = [
        {"id": "4.3", "requires": [], "introduces": ["common"]},
        {"id": "4.4", "requires": []},
        {"id": "4.5", "requires": ["4.4"], "labs": ["book.ipynb"]},
    ]
    errors = validate_book_stage_calls(records, tmp_path)
    assert errors == ["4.5: analyst_book.build_book('common') in book.ipynb precedes common introduction in 4.3"]

    records[1]["requires"] = ["4.3"]
    assert validate_book_stage_calls(records, tmp_path) == []


@pytest.mark.parametrize(
    ("early", "later", "fixtures", "source"),
    [
        ("4.1", "4.3", ["common"], 'from _shared import analyst_book as shared\nshared.build_book("common")\n'),
        (
            "2.8",
            "C1",
            ["credit", "waterfall"],
            "from _shared import analyst_tracks as shared\nshared.credit_index_inputs()\n",
        ),
    ],
)
def test_lab_stage_exposure_requires_later_cell_and_fixture_owner_in_closure(
    tmp_path: Path,
    early: str,
    later: str,
    fixtures: list[str],
    source: str,
) -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                source,
                metadata={"analyst_program": {"lesson": later, "id": "later-stage", "role": "build"}},
            )
        ]
    )
    nbformat.write(notebook, tmp_path / "later.ipynb")
    records = [
        {"id": early, "requires": [], "fixtures": [], "labs": ["later.ipynb"]},
        {"id": later, "requires": [], "fixtures": fixtures},
    ]

    errors = validate_lab_stage_exposure(records, tmp_path)
    assert any(f"exposes {later} code" in error for error in errors)
    assert any("before fixture" in error for error in errors)

    records[0]["requires"] = [later]
    assert validate_lab_stage_exposure(records, tmp_path) == []


def test_lab_stage_exposure_checks_recursive_notebook_dependencies(tmp_path: Path) -> None:
    dependency = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell("from _shared import analyst_book as shared\n"),
            nbformat.v4.new_code_cell(
                'book = shared.build_book("common")\n',
                metadata={"analyst_program": {"lesson": "4.3", "id": "term-loan", "role": "build"}},
            ),
        ],
        metadata={"analyst_dependencies": []},
    )
    nbformat.write(dependency, tmp_path / "dependency.ipynb")
    main = nbformat.v4.new_notebook(
        cells=[nbformat.v4.new_code_cell("print('early lesson')\n")],
        metadata={"analyst_dependencies": ["dependency.ipynb"]},
    )
    nbformat.write(main, tmp_path / "main.ipynb")
    records = [
        {"id": "4.1", "requires": [], "fixtures": ["borrower", "base"], "labs": ["main.ipynb"]},
        {"id": "4.3", "requires": [], "fixtures": ["common"]},
    ]

    errors = validate_lab_stage_exposure(records, tmp_path)
    assert any("dependency dependency.ipynb exposes 4.3 code" in error for error in errors)
    assert any("dependency dependency.ipynb" in error and "before fixture common" in error for error in errors)

    records[0]["requires"] = ["4.3"]
    assert validate_lab_stage_exposure(records, tmp_path) == []


def test_fixture_and_notebook_paths_cannot_escape(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="remain inside"):
        contained(tmp_path, "../outside.ipynb")


def test_rendered_links_catch_missing_guides_and_fragments(tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text('<a href="/guide/">Guide</a><a href="/learn/#missing">Learn</a>')
    (tmp_path / "learn").mkdir()
    (tmp_path / "learn/index.html").write_text('<h1 id="ready">Ready</h1>')
    errors = check_links(tmp_path)
    assert any("missing destination /guide/" in item for item in errors)
    assert any("missing fragment /learn/#missing" in item for item in errors)


def test_rendered_links_accept_literal_and_decoded_fragments(tmp_path: Path) -> None:
    (tmp_path / "index.html").write_text(
        '<h2 id="Rates-%E2%80%94">Rates</h2><a href="#Rates-%E2%80%94">Link</a>'
        '<h2 id="Credit—">Credit</h2><a href="#Credit%E2%80%94">Link</a>'
    )
    assert check_links(tmp_path) == []


def test_bibliography_preserves_syllabus_heading_slugs(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import build_references

    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/REFERENCES.md").write_text('### Sharpe 1966\n\nReference.\n<a id="explicit"></a>\n')
    monkeypatch.setattr(build_references, "REPO", tmp_path)
    monkeypatch.setattr(build_references, "SITE", tmp_path)
    assert build_references.main() == 0
    rendered = (tmp_path / "public/references.html").read_text()
    assert 'id="sharpe-1966"' in rendered
    assert 'id="explicit"' in rendered


def test_source_execution_state_is_rejected(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import build_labs

    monkeypatch.setattr(build_labs, "NOTEBOOKS", tmp_path)
    path = tmp_path / "lesson.ipynb"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell("assert True", execution_count=1)],
            metadata={"analyst_dependencies": []},
        ),
        path,
    )
    with pytest.raises(ValueError, match="clear source outputs"):
        build_labs.require_clean_source("lesson.ipynb")
    path.write_text(
        json.dumps({"cells": [{"cell_type": "code", "source": "assert True", "outputs": [], "execution_count": None}]})
    )
    build_labs.require_clean_source("lesson.ipynb")


def test_lab_export_keeps_canonical_gate_and_strips_learner_copies(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import build_labs

    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    source = notebooks / "lesson.ipynb"
    code = "value = 6 * 7\nassert value == 42\nprint(value)\n"
    canonical = nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell(code)])
    nbformat.write(canonical, source)
    executed = tmp_path / "executed.ipynb"
    result = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                code,
                execution_count=1,
                outputs=[nbformat.v4.new_output("stream", name="stdout", text="42\n")],
            )
        ],
        metadata={
            "finstack_execution": {
                "assertions_total": 1,
                "assertions_executed": 1,
                "assertion_locations": ["1:2:0"],
            }
        },
    )
    nbformat.write(result, executed)
    site = tmp_path / "site"
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "SITE", site)
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "test"})

    canonical_execution = build_labs.prepare_publication_notebook(executed, "lesson.ipynb")
    canonical_copy = executed.with_suffix(".canonical.ipynb")
    public_result = nbformat.read(executed, as_version=4)
    public_result.cells[0].execution_count = 1
    public_result.cells[0].outputs = [nbformat.v4.new_output("stream", name="stdout", text="42\n")]
    public_result.metadata["finstack_execution"] = {
        "assertions_total": 0,
        "assertions_executed": 0,
        "assertion_locations": [],
    }
    nbformat.write(public_result, executed)
    evidence = build_labs.export_notebook(executed, "lesson.ipynb", {"builder": "test"}, canonical_execution)

    assert notebook_assertion_count(nbformat.read(source, as_version=4)) == 1
    assert notebook_assertion_count(nbformat.read(executed, as_version=4)) == 0
    download = site / "public/lab-assets/lesson.ipynb"
    assert notebook_assertion_count(nbformat.read(download, as_version=4)) == 0
    assert evidence["canonical_execution"]["assertions"] == 1
    assert evidence["canonical_execution"]["sha256"] == build_labs.digest(canonical_copy)
    assert evidence["publication_execution"] == {
        "status": "passed",
        "sha256": build_labs.digest(executed),
        "assertions": 0,
        "assertions_executed": 0,
    }
    assert "assert value == 42" not in (site / "public/lab-assets/lesson.md").read_text()


@pytest.mark.parametrize(
    "statement",
    ["raise AssertionError('visible validation')", "raise builtins.AssertionError('visible validation')"],
)
def test_lab_export_rejects_explicit_assertion_error(tmp_path: Path, statement: str) -> None:
    import build_labs

    executed = tmp_path / "executed.ipynb"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[
                nbformat.v4.new_code_cell(
                    f"if False:\n    {statement}\n",
                    execution_count=1,
                )
            ],
            metadata={
                "finstack_execution": {
                    "assertions_total": 0,
                    "assertions_executed": 0,
                    "assertion_locations": [],
                }
            },
        ),
        executed,
    )

    with pytest.raises(ValueError, match="explicit raise AssertionError"):
        build_labs.export_notebook(executed, "lesson.ipynb", {}, {})


def test_lab_publication_rejects_a_changed_canonical_assertion(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import build_labs

    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    canonical = nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell("value = 42\nassert value == 42\n")])
    nbformat.write(canonical, notebooks / "lesson.ipynb")
    changed = nbformat.v4.new_notebook(
        cells=[nbformat.v4.new_code_cell("value = 42\nassert value != 42\n", execution_count=1)]
    )
    executed = tmp_path / "executed.ipynb"
    nbformat.write(changed, executed)
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)

    with pytest.raises(ValueError, match="changed the authored notebook source"):
        build_labs.prepare_publication_notebook(executed, "lesson.ipynb")


def test_lab_build_rejects_an_obviously_mutating_assertion(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import build_labs

    original_runner = build_labs.NOTEBOOKS / "run_all_notebooks.py"
    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    shutil.copy2(original_runner, notebooks / "run_all_notebooks.py")
    source = notebooks / "side_effect.ipynb"
    code = "items = []\nassert items.append(1) is None\nprint(items[0])\n"
    nbformat.write(
        nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell(code)], metadata={"analyst_dependencies": []}),
        source,
    )
    site = tmp_path / "site"
    site.mkdir()
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "SITE", site)
    monkeypatch.setattr(build_labs, "BUILD", site / ".build")
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "test"})
    monkeypatch.setattr(build_labs, "published_notebooks", lambda: {"side_effect.ipynb"})
    monkeypatch.setattr("sys.argv", ["build_labs.py", "--notebook", "side_effect.ipynb", "--timeout", "30"])

    with pytest.raises(ValueError, match=r"assertions must be observational; append\(\)"):
        build_labs.main()
    assert not (site / "public/lab-assets/side_effect.ipynb").exists()


def test_lab_build_replays_declared_dependency_in_canonical_and_public_forms(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import build_labs

    original_runner = build_labs.NOTEBOOKS / "run_all_notebooks.py"
    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    shutil.copy2(original_runner, notebooks / "run_all_notebooks.py")
    dependency_code = (
        "items = []\n"
        "def populate():\n"
        "    items.append('canonical')\n"
        "    return True\n"
        "assert populate()\n"
        "dependency_value = len(items)\n"
    )
    nbformat.write(
        nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell(dependency_code)]),
        notebooks / "dependency.ipynb",
    )
    main_code = """import json
from pathlib import Path
import sys

dependency_path = next(
    Path(entry) / "dependency.ipynb"
    for entry in sys.path
    if (Path(entry) / "dependency.ipynb").is_file()
)
dependency_notebook = json.loads(dependency_path.read_text())
dependency_scope = {}
for dependency_cell in dependency_notebook["cells"]:
    dependency_source = dependency_cell["source"]
    if isinstance(dependency_source, list):
        dependency_source = "".join(dependency_source)
    exec(compile(dependency_source, str(dependency_path), "exec"), dependency_scope)
dependency_value = dependency_scope["dependency_value"]
print(dependency_value)
assert dependency_value == 1
"""
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(main_code)],
            metadata={"analyst_dependencies": ["dependency.ipynb"]},
        ),
        notebooks / "main.ipynb",
    )
    site = tmp_path / "site"
    site.mkdir()
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)
    canonical_root = tmp_path / "canonical"
    publication_root = tmp_path / "publication"
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "SITE", site)
    monkeypatch.setattr(build_labs, "BUILD", site / ".build")
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "test"})

    with pytest.raises(ValueError, match="canonical checks change learner-visible output in cell 1"):
        build_labs.build_notebook_copy(
            copies,
            canonical_root,
            publication_root,
            "main.ipynb",
            30,
            {"builder": "test"},
        )

    canonical = nbformat.read(canonical_root / "main.ipynb", as_version=4)
    publication = nbformat.read(publication_root / "main.ipynb", as_version=4)
    assert notebook_assertion_count(canonical) == 1
    assert notebook_assertion_count(publication) == 0
    assert canonical.cells[0].outputs[0].text == "1\n"
    assert publication.cells[0].outputs[0].text == "0\n"
    assert not (site / "public/lab-assets/main.ipynb").exists()
    restored_dependency = nbformat.read(copies / "dependency.ipynb", as_version=4)
    assert notebook_assertion_count(restored_dependency) == 1


def test_lab_execution_equivalence_ignores_adjacent_stream_chunking() -> None:
    import build_labs

    canonical = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                "print('stable')",
                outputs=[
                    nbformat.v4.new_output(output_type="stream", name="stdout", text="stable"),
                    nbformat.v4.new_output(output_type="stream", name="stdout", text="\n"),
                ],
            )
        ]
    )
    published = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                "print('stable')",
                outputs=[nbformat.v4.new_output(output_type="stream", name="stdout", text="stable\n")],
            )
        ]
    )

    build_labs.validate_execution_equivalence(canonical, published, "stable.ipynb")


def test_lab_execution_equivalence_preserves_display_boundaries() -> None:
    import build_labs

    display = nbformat.v4.new_output(
        output_type="display_data",
        data={"text/plain": "middle"},
        metadata={},
    )
    canonical = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                "print('before display after')",
                outputs=[
                    nbformat.v4.new_output(output_type="stream", name="stdout", text="before"),
                    display,
                    nbformat.v4.new_output(output_type="stream", name="stdout", text="after"),
                ],
            )
        ]
    )
    published = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                "print('before display after')",
                outputs=[
                    nbformat.v4.new_output(output_type="stream", name="stdout", text="beforeafter"),
                    display,
                ],
            )
        ]
    )

    with pytest.raises(ValueError, match="canonical checks change learner-visible output in cell 1"):
        build_labs.validate_execution_equivalence(canonical, published, "changed.ipynb")


def test_lab_source_preflight_rejects_duplicate_cell_ids(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import build_labs

    notebook = {
        "cells": [
            {"cell_type": "markdown", "id": "duplicate", "metadata": {}, "source": "# First"},
            {"cell_type": "markdown", "id": "duplicate", "metadata": {}, "source": "# Second"},
        ],
        "metadata": {},
        "nbformat": 4,
        "nbformat_minor": 5,
    }
    path = tmp_path / "duplicate.ipynb"
    path.write_text(json.dumps(notebook))
    monkeypatch.setattr(build_labs, "NOTEBOOKS", tmp_path)

    with pytest.raises(Warning, match="Non-unique cell id 'duplicate'"):
        build_labs.require_clean_source("duplicate.ipynb")


def _dynamic_notebook_loader(name: str, result: str) -> str:
    """Return notebook code that executes a sibling notebook and reads one result."""
    return f"""import json
from pathlib import Path

dependency_path = Path("{name}")
dependency_notebook = json.loads(dependency_path.read_text())
dependency_scope = {{}}
for dependency_cell in dependency_notebook["cells"]:
    if dependency_cell["cell_type"] != "code":
        continue
    dependency_source = dependency_cell["source"]
    if isinstance(dependency_source, list):
        dependency_source = "".join(dependency_source)
    exec(compile(dependency_source, str(dependency_path), "exec"), dependency_scope)
dependency_value = dependency_scope["{result}"]
"""


def _strict_lab_test_tree(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[ModuleType, Path, Path]:
    """Create an isolated source tree backed by the repository's strict runner."""
    import build_labs

    original_runner = build_labs.NOTEBOOKS / "run_all_notebooks.py"
    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    shutil.copy2(original_runner, notebooks / "run_all_notebooks.py")
    site = tmp_path / "site"
    site.mkdir()
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "SITE", site)
    monkeypatch.setattr(build_labs, "BUILD", site / ".build")
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "test"})
    return build_labs, notebooks, site


def test_lab_build_blocks_undeclared_dynamic_notebook_access(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    build_labs, notebooks, site = _strict_lab_test_tree(tmp_path, monkeypatch)
    nbformat.write(
        nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell("hidden_value = 42\n")]),
        notebooks / "hidden.ipynb",
    )
    source = _dynamic_notebook_loader("hidden.ipynb", "hidden_value")
    source += "print(dependency_value)\nassert dependency_value == 42\n"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(source)],
            metadata={"analyst_dependencies": []},
        ),
        notebooks / "main.ipynb",
    )
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)

    entry, failure = build_labs.build_notebook_copy(
        copies,
        tmp_path / "canonical",
        tmp_path / "publication",
        "main.ipynb",
        30,
        {"builder": "test"},
    )

    assert entry is None
    assert failure is not None
    assert failure.returncode != 0
    assert "hidden.ipynb" in failure.stdout + failure.stderr
    assert not (copies / "hidden.ipynb").exists()
    assert not (site / "public/lab-assets/main.ipynb").exists()


def test_lab_build_blocks_absolute_access_to_canonical_notebooks(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    build_labs, notebooks, site = _strict_lab_test_tree(tmp_path, monkeypatch)
    hidden = notebooks / "hidden.ipynb"
    nbformat.write(
        nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell("hidden_value = 42\n")]),
        hidden,
    )
    source = (
        "from pathlib import Path\n"
        f"_finstack_allowed_notebook_root = Path({str(hidden.parent)!r})\n"
        f"Path({str(hidden)!r}).read_text()\n"
    )
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(source)],
            metadata={"analyst_dependencies": []},
        ),
        notebooks / "main.ipynb",
    )
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)

    entry, failure = build_labs.build_notebook_copy(
        copies,
        tmp_path / "canonical",
        tmp_path / "publication",
        "main.ipynb",
        30,
        {"builder": "test"},
    )

    assert entry is None
    assert failure is not None
    assert "Notebook access outside declared root" in failure.stdout + failure.stderr
    assert not (site / "public/lab-assets/main.ipynb").exists()


def test_lab_build_rejects_an_unreachable_canonical_check(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    build_labs, notebooks, site = _strict_lab_test_tree(tmp_path, monkeypatch)
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell("if False:\n    assert False\nprint('no check ran')\n")],
            metadata={"analyst_dependencies": []},
        ),
        notebooks / "unreachable.ipynb",
    )
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)

    entry, failure = build_labs.build_notebook_copy(
        copies,
        tmp_path / "canonical",
        tmp_path / "publication",
        "unreachable.ipynb",
        30,
        {"builder": "test"},
    )

    assert entry is None
    assert failure is not None
    assert "did not execute every assertion statement" in failure.stdout + failure.stderr
    assert not (site / "public/lab-assets/unreachable.ipynb").exists()


def test_lab_build_allows_declared_recursive_dynamic_notebook_access(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    build_labs, notebooks, site = _strict_lab_test_tree(tmp_path, monkeypatch)
    nbformat.write(
        nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell("leaf_value = 42\n")]),
        notebooks / "leaf.ipynb",
    )
    middle_source = _dynamic_notebook_loader("leaf.ipynb", "leaf_value")
    middle_source += "middle_value = dependency_value\n"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(middle_source)],
            metadata={"analyst_dependencies": ["leaf.ipynb"]},
        ),
        notebooks / "middle.ipynb",
    )
    main_source = _dynamic_notebook_loader("middle.ipynb", "middle_value")
    main_source += "print(dependency_value)\nassert dependency_value == 42\n"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(main_source)],
            metadata={"analyst_dependencies": ["middle.ipynb"]},
        ),
        notebooks / "main.ipynb",
    )
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)
    canonical_root = tmp_path / "canonical"
    publication_root = tmp_path / "publication"

    entry, failure = build_labs.build_notebook_copy(
        copies,
        canonical_root,
        publication_root,
        "main.ipynb",
        30,
        {"builder": "test"},
    )

    assert failure is None
    assert entry is not None
    assert entry["dependencies_sha256"] == {
        "leaf.ipynb": build_labs.digest(notebooks / "leaf.ipynb"),
        "middle.ipynb": build_labs.digest(notebooks / "middle.ipynb"),
    }
    assert nbformat.read(canonical_root / "main.ipynb", as_version=4).cells[0].outputs[0].text == "42\n"
    published = nbformat.read(publication_root / "main.ipynb", as_version=4)
    assert published.cells[0].outputs[0].text == "42\n"
    assert notebook_assertion_count(published) == 0
    assert (site / "public/lab-assets/main.ipynb").is_file()
    assert (copies / "leaf.ipynb").is_file()
    assert (copies / "middle.ipynb").is_file()


def test_lab_build_rejects_opaque_assertion_mutation_observed_in_later_cell(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import build_labs

    original_runner = build_labs.NOTEBOOKS / "run_all_notebooks.py"
    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    shutil.copy2(original_runner, notebooks / "run_all_notebooks.py")
    mutate = """items = []
def populate():
    items.append("canonical")
    return True
assert populate()
"""
    observe = "print(len(items))\n"
    nbformat.write(
        nbformat.v4.new_notebook(
            cells=[nbformat.v4.new_code_cell(mutate), nbformat.v4.new_code_cell(observe)],
            metadata={"analyst_dependencies": []},
        ),
        notebooks / "opaque.ipynb",
    )
    site = tmp_path / "site"
    site.mkdir()
    copies = tmp_path / "copies"
    shutil.copytree(notebooks, copies)
    canonical_root = tmp_path / "canonical"
    publication_root = tmp_path / "publication"
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "SITE", site)
    monkeypatch.setattr(build_labs, "BUILD", site / ".build")
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "test"})

    with pytest.raises(ValueError, match="canonical checks change learner-visible output in cell 2"):
        build_labs.build_notebook_copy(
            copies,
            canonical_root,
            publication_root,
            "opaque.ipynb",
            30,
            {"builder": "test"},
        )

    canonical = nbformat.read(canonical_root / "opaque.ipynb", as_version=4)
    publication = nbformat.read(publication_root / "opaque.ipynb", as_version=4)
    assert canonical.cells[1].outputs[0].text == "1\n"
    assert publication.cells[1].outputs[0].text == "0\n"
    assert not (site / "public/lab-assets/opaque.ipynb").exists()


def _lab_evidence_environment(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[ModuleType, dict]:
    """Configure one isolated lab-evidence tree and return its builder and provenance."""
    import build_labs

    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    (notebooks / "run_all_notebooks.py").write_text("# strict runner\n")
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "BUILD", tmp_path / ".build")
    monkeypatch.setattr(build_labs, "SITE", tmp_path)
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "current"})
    monkeypatch.setattr(build_labs, "published_notebooks", lambda: set(build_labs.source_notebooks()))
    return build_labs, build_labs.lab_provenance()


def _write_current_lab_entry(build_labs: ModuleType, relative: str, provenance: dict) -> tuple[dict, dict[str, Path]]:
    """Create a complete current lab entry and every artifact it fingerprints."""
    source = build_labs.NOTEBOOKS / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    execution_metadata = {
        "assertions_total": 0,
        "assertions_executed": 0,
        "assertion_locations": [],
    }
    nbformat.write(
        nbformat.v4.new_notebook(metadata={"analyst_dependencies": [], "finstack_execution": execution_metadata}),
        source,
    )
    executed = build_labs.BUILD / "notebooks" / relative
    executed.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, executed)
    canonical_executed = build_labs.BUILD / "canonical-notebooks" / relative
    canonical_executed.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, canonical_executed)
    required, resource_dir = build_labs.lab_artifact_paths(relative)
    for path in required:
        path.parent.mkdir(parents=True, exist_ok=True)
    required[0].write_text("<html><body>lab</body></html>\n")
    required[1].write_text("# Executed lab\n")
    shutil.copy2(executed, required[2])
    required[3].write_text("---\ntitle: Executed lab\n---\n")
    resource_dir.mkdir(parents=True)
    resource = resource_dir / "figure.png"
    resource.write_bytes(b"figure")
    entry = {
        "notebook": relative,
        "source_sha256": build_labs.digest(source),
        "canonical_execution": {
            "status": "passed",
            "sha256": build_labs.digest(canonical_executed),
            "assertions": 0,
            "assertions_executed": 0,
        },
        "publication_execution": {
            "status": "passed",
            "sha256": build_labs.digest(executed),
            "assertions": 0,
            "assertions_executed": 0,
        },
        "fixtures_sha256": "fixtures",
        "dependencies_sha256": {},
        "runtime": {"python": "current"},
        "published": True,
        **provenance,
    }
    entry["artifacts_sha256"] = build_labs.current_lab_artifacts(relative)
    paths = {
        "source": source,
        "executed": executed,
        "canonical": canonical_executed,
        "html": required[0],
        "markdown": required[1],
        "download": required[2],
        "page": required[3],
        "resource": resource,
    }
    return entry, paths


def test_lab_report_requires_complete_current_per_entry_evidence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A strict top-level label cannot hide failed, partial or stale lab entries."""
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    entry, paths = _write_current_lab_entry(build_labs, "lesson.ipynb", provenance)
    report = {**provenance, "labs": [entry], "failed": []}

    assert build_labs.lab_report_errors(report) == []
    required, _ = build_labs.lab_artifact_paths("lesson.ipynb")
    required_keys = {str(path.relative_to(tmp_path)) for path in required}
    assert required_keys.issubset(entry["artifacts_sha256"])
    assert str(paths["resource"].relative_to(tmp_path)) in entry["artifacts_sha256"]
    bad_canonical = {
        **entry,
        "canonical_execution": {**entry["canonical_execution"], "sha256": "0" * 64},
    }
    assert any(
        "canonical executed notebook" in error
        for error in build_labs.lab_report_errors({**report, "labs": [bad_canonical]})
    )
    assert any(
        "complete source notebook set" in error for error in build_labs.lab_report_errors({**report, "labs": []})
    )
    assert any(
        "failed notebooks" in error for error in build_labs.lab_report_errors({**report, "failed": ["lesson.ipynb"]})
    )
    assert any(
        "Stale lab evidence" in error
        for error in build_labs.lab_report_errors({**report, "labs": [{**entry, "builder_sha256": "old"}]})
    )
    assert any(
        "Stale lab evidence" in error
        for error in build_labs.lab_report_errors({**report, "labs": [{**entry, "common_sha256": "old"}]})
    )
    dirty = nbformat.v4.new_notebook(cells=[nbformat.v4.new_code_cell("assert True\n", execution_count=1)])
    nbformat.write(dirty, paths["executed"])
    shutil.copy2(paths["executed"], paths["download"])
    dirty_entry = {
        **entry,
        "publication_execution": {
            "status": "passed",
            "sha256": build_labs.digest(paths["executed"]),
            "assertions": 0,
            "assertions_executed": 0,
        },
    }
    assert any(
        "Learner-facing executed notebook contains assertions" in error
        for error in build_labs.lab_report_errors({**report, "labs": [dirty_entry]})
    )
    shutil.copy2(paths["source"], paths["executed"])
    shutil.copy2(paths["executed"], paths["download"])

    other_entry, _ = _write_current_lab_entry(build_labs, "other.ipynb", provenance)
    evidence_path = build_labs.BUILD / "labs.json"
    evidence_path.write_text(json.dumps({**report, "labs": [entry, other_entry]}))
    nbformat.write(
        nbformat.v4.new_notebook(metadata={"analyst_dependencies": [], "changed": True}),
        paths["source"],
    )

    preserved = build_labs.preserved_lab_entries(evidence_path, ["lesson.ipynb"], build_labs.source_notebooks())

    assert [item["notebook"] for item in preserved] == ["other.ipynb"]


def test_lab_report_rejects_explicit_assertion_error(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    entry, paths = _write_current_lab_entry(build_labs, "lesson.ipynb", provenance)
    execution_metadata = {
        "assertions_total": 0,
        "assertions_executed": 0,
        "assertion_locations": [],
    }
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_code_cell(
                "if False:\n    raise AssertionError('visible validation')\n",
                execution_count=1,
            )
        ],
        metadata={"analyst_dependencies": [], "finstack_execution": execution_metadata},
    )
    for path in (paths["source"], paths["canonical"], paths["executed"], paths["download"]):
        nbformat.write(notebook, path)
    entry = {
        **entry,
        "source_sha256": build_labs.digest(paths["source"]),
        "canonical_execution": {
            **entry["canonical_execution"],
            "sha256": build_labs.digest(paths["canonical"]),
        },
        "publication_execution": {
            **entry["publication_execution"],
            "sha256": build_labs.digest(paths["executed"]),
        },
    }
    entry["artifacts_sha256"] = build_labs.current_lab_artifacts("lesson.ipynb")

    errors = build_labs.lab_report_errors({**provenance, "labs": [entry], "failed": []})

    assert any("explicit raise AssertionError" in error for error in errors)


@pytest.mark.parametrize("artifact", ["html", "markdown", "download", "page", "resource"])
@pytest.mark.parametrize("operation", ["mutate", "delete"])
def test_lab_artifact_manifest_rejects_mutation_deletion_and_focused_preservation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    artifact: str,
    operation: str,
) -> None:
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    entry, paths = _write_current_lab_entry(build_labs, "lesson.ipynb", provenance)
    nbformat.write(
        nbformat.v4.new_notebook(metadata={"analyst_dependencies": []}),
        build_labs.NOTEBOOKS / "selected.ipynb",
    )
    report = {**provenance, "labs": [entry], "failed": []}
    assert build_labs.lab_report_errors(report, require_complete=False) == []

    target = paths[artifact]
    if operation == "mutate":
        target.write_bytes(target.read_bytes() + b"changed")
    else:
        target.unlink()

    errors = build_labs.lab_report_errors(report, require_complete=False)
    assert any("learner-facing lab artifacts" in error for error in errors)
    evidence_path = build_labs.BUILD / "labs.json"
    evidence_path.write_text(json.dumps(report))
    with pytest.raises(ValueError, match="Cannot preserve unrelated lab evidence"):
        build_labs.preserved_lab_entries(
            evidence_path,
            ["selected.ipynb"],
            build_labs.source_notebooks(),
        )


def test_complete_lab_artifact_tree_rejects_and_prunes_deleted_source_outputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    entry, _paths = _write_current_lab_entry(build_labs, "current.ipynb", provenance)
    obsolete_asset = tmp_path / "public" / "lab-assets" / "deleted.html"
    obsolete_asset.parent.mkdir(parents=True, exist_ok=True)
    obsolete_asset.write_text("obsolete")
    obsolete_page = tmp_path / "content" / "labs" / "deleted.mdx"
    obsolete_page.parent.mkdir(parents=True, exist_ok=True)
    obsolete_page.write_text("obsolete")
    report = {**provenance, "labs": [entry], "failed": []}

    assert any("artifact tree differs" in error for error in build_labs.lab_report_errors(report))

    build_labs.prune_unrecorded_lab_artifacts([entry])

    assert build_labs.lab_report_errors(report) == []
    assert not obsolete_asset.exists()
    assert not obsolete_page.exists()


def test_execution_only_notebook_keeps_strict_copies_without_public_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An unmapped notebook remains execution-gated but disappears from the learner site."""
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    original_entry, paths = _write_current_lab_entry(build_labs, "repository_only.ipynb", provenance)
    monkeypatch.setattr(build_labs, "published_notebooks", set)
    entry = build_labs.export_notebook(
        paths["executed"],
        "repository_only.ipynb",
        provenance,
        original_entry["canonical_execution"],
        publish=False,
    )

    report = {**provenance, "labs": [entry], "failed": []}

    assert build_labs.lab_report_errors(report) == []
    required, resource_dir = build_labs.lab_artifact_paths("repository_only.ipynb")
    assert all(not path.exists() for path in required)
    assert not resource_dir.exists()
    assert (build_labs.BUILD / "notebooks/repository_only.ipynb").is_file()
    assert (build_labs.BUILD / "canonical-notebooks/repository_only.ipynb").is_file()


@pytest.mark.parametrize("kind", ["file", "directory", "broken"])
def test_lab_artifact_tree_rejects_and_prunes_symlinks(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, kind: str
) -> None:
    build_labs, provenance = _lab_evidence_environment(tmp_path, monkeypatch)
    monkeypatch.setattr(build_labs, "published_notebooks", lambda: {"current.ipynb"})
    entry, _paths = _write_current_lab_entry(build_labs, "current.ipynb", provenance)
    external = tmp_path / "external"
    if kind == "file":
        external.write_text("external")
    elif kind == "directory":
        external.mkdir()
        (external / "outside.html").write_text("external")
    link = tmp_path / "public" / "lab-assets" / f"{kind}-link"
    link.symlink_to(external, target_is_directory=kind == "directory")
    report = {**provenance, "labs": [entry], "failed": []}

    errors = build_labs.lab_report_errors(report)
    assert any("must not contain symlinks" in error for error in errors)

    build_labs.prune_unrecorded_lab_artifacts([entry])
    assert not link.is_symlink()
    assert build_labs.lab_report_errors(report) == []


def test_prerequisite_gate_rejects_unexecuted_copy(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    from common import digest
    import verify_prerequisites as verifier

    monkeypatch.setattr(verifier, "NOTEBOOKS", tmp_path)
    monkeypatch.setattr(verifier, "BUILD", tmp_path / ".build")
    monkeypatch.setattr(verifier, "notebook_dependencies", lambda _: {})
    source = tmp_path / "proof.ipynb"
    source.write_text(
        json.dumps({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "assert 1 == 1",
                    "metadata": {"analyst_program": {"lesson": "1.1", "id": "proof"}},
                    "execution_count": None,
                    "outputs": [],
                }
            ]
        })
    )
    runtime = {"extension_sha256": "current"}
    lab = {"proof.ipynb": {"source_sha256": digest(source), "fixtures_sha256": "fixture", "runtime": runtime}}
    _, _, errors = verifier.inspect_cells("proof.ipynb", "1.1", lab, runtime, "fixture", True)
    assert any("did not execute successfully" in error for error in errors)
    lab["proof.ipynb"]["runtime"] = {"extension_sha256": "obsolete"}
    _, _, errors = verifier.inspect_cells("proof.ipynb", "1.1", lab, runtime, "fixture", True)
    assert any("stale notebook execution" in error for error in errors)
