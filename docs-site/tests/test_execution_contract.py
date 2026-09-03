"""Exercise real failure propagation, replay isolation and publication contracts."""

from copy import deepcopy
import json
from pathlib import Path

from check_curriculum import check_metadata, validate_dependencies
from check_links import check as check_links
from common import contained
import pytest
from run_lesson_snippets import Block, extract_blocks, run_process, validate_output_references


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


def test_fences_require_unique_ids_and_checks() -> None:
    text = "```python exec id=proof\nassert True\n```\n"
    assert extract_blocks(text)[0].id == "proof"
    with pytest.raises(ValueError, match="duplicate"):
        extract_blocks(text + text)
    with pytest.raises(ValueError, match="assertion"):
        extract_blocks("```python exec id=proof\nprint(1)\n```\n")
    assert not extract_blocks("```rust display-only\nassert!(true);\n```\n")


@pytest.mark.parametrize("language", ["python", "python3", "py"])
def test_displayed_python_cannot_bypass_execution(language: str) -> None:
    visible = (
        "```python exec id=proof\nassert True\n```\n"
        f"```{language}\nfrom nonexistent_finance_api import UnsupportedPricer\n```\n"
    )
    with pytest.raises(ValueError, match="displayed Python must use"):
        extract_blocks(visible)


def test_output_references_cover_every_block_and_ignore_displayed_literals() -> None:
    text = (
        '```python exec id=proof\ntext = \'<ExecutedOutput lesson="other" block="literal" />\'\nassert text\n```\n'
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
    text = "```python exec id=proof\nassert True\n```\n" + references
    with pytest.raises(ValueError, match=message):
        validate_output_references(text, "1.1", extract_blocks(text))


def test_output_reference_failure_removes_stale_evidence(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import run_lesson_snippets as runner

    source = tmp_path / "lesson.mdx"
    source.write_text(
        "---\nstatus: published\n---\n```python exec id=proof\nassert True\n```\n"
        '<ExecutedOutput lesson="1.1" block="typo" />\n'
    )
    output = tmp_path / "snippets/1.1.json"
    output.parent.mkdir()
    output.write_text('{"status":"passed"}')
    monkeypatch.setattr(runner, "CONTENT", tmp_path)
    monkeypatch.setattr(runner, "BUILD", tmp_path)
    with pytest.raises(ValueError, match=r"unknown=.*typo"):
        runner.run_lesson({"id": "1.1", "path": source.name})
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
        {"id": "capstone", "requires": ["4.5"], "variants": {"credit": ["C2"], "volatility": ["V2"]}},
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
    path.write_text(
        json.dumps({"cells": [{"cell_type": "code", "source": "assert True", "outputs": [], "execution_count": 1}]})
    )
    with pytest.raises(ValueError, match="clear source outputs"):
        build_labs.require_clean_source("lesson.ipynb")
    path.write_text(
        json.dumps({"cells": [{"cell_type": "code", "source": "assert True", "outputs": [], "execution_count": None}]})
    )
    build_labs.require_clean_source("lesson.ipynb")


def test_lab_report_requires_complete_current_per_entry_evidence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A strict top-level label cannot hide failed, partial or stale lab entries."""
    import build_labs
    from common import digest

    notebooks = tmp_path / "notebooks"
    notebooks.mkdir()
    source = notebooks / "lesson.ipynb"
    source.write_text(json.dumps({"cells": [], "metadata": {"analyst_dependencies": []}}))
    (notebooks / "run_all_notebooks.py").write_text("# strict runner\n")
    build = tmp_path / ".build"
    executed = build / "notebooks" / "lesson.ipynb"
    executed.parent.mkdir(parents=True)
    executed.write_text("executed copy")
    monkeypatch.setattr(build_labs, "NOTEBOOKS", notebooks)
    monkeypatch.setattr(build_labs, "BUILD", build)
    monkeypatch.setattr(build_labs, "fixture_digest", lambda: "fixtures")
    monkeypatch.setattr(build_labs, "runtime_identity", lambda: {"python": "current"})
    provenance = build_labs.lab_provenance()
    entry = {
        "notebook": "lesson.ipynb",
        "source_sha256": digest(source),
        "executed_sha256": digest(executed),
        "fixtures_sha256": "fixtures",
        "dependencies_sha256": {},
        "runtime": {"python": "current"},
        **provenance,
    }
    report = {**provenance, "labs": [entry], "failed": []}

    assert build_labs.lab_report_errors(report) == []
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

    other_source = notebooks / "other.ipynb"
    other_source.write_text(json.dumps({"cells": [], "metadata": {"analyst_dependencies": []}}))
    other_executed = build / "notebooks" / "other.ipynb"
    other_executed.write_text("other executed copy")
    other_entry = {
        "notebook": "other.ipynb",
        "source_sha256": digest(other_source),
        "executed_sha256": digest(other_executed),
        "fixtures_sha256": "fixtures",
        "dependencies_sha256": {},
        "runtime": {"python": "current"},
        **provenance,
    }
    evidence_path = build / "labs.json"
    evidence_path.write_text(json.dumps({**report, "labs": [entry, other_entry]}))
    source.write_text(json.dumps({"cells": [], "metadata": {"analyst_dependencies": []}, "changed": True}))

    preserved = build_labs.preserved_lab_entries(evidence_path, ["lesson.ipynb"], build_labs.source_notebooks())

    assert [item["notebook"] for item in preserved] == ["other.ipynb"]


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
