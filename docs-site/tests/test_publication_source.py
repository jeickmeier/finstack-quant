"""Learner-facing source keeps calculations and removes validation statements."""

import ast

import nbformat
from publication_source import assertion_count, notebook_assertion_count, remove_notebook_assertions, without_assertions


def test_assertion_filter_preserves_surrounding_source_and_valid_suites() -> None:
    source = """# Keep this explanation.
label = "assert remains data"
value = 2
assert value == 2, "validation only"
other = 3; assert other == 3
assert value < other; total = value + other
if value:
    assert other > value
try:
    int("bad")
except ValueError:
    assert value == 2
if other: assert total == 5
"""

    published = without_assertions(source, "lesson-cell")

    ast.parse(published)
    assert assertion_count(published) == 0
    assert "# Keep this explanation." in published
    assert 'label = "assert remains data"' in published
    assert "value = 2" in published
    assert "other = 3" in published
    assert "total = value + other" in published
    assert published.count("pass") == 3


def test_notebook_filter_preserves_outputs_and_reports_removed_count() -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[
            nbformat.v4.new_markdown_cell("Keep this prose."),
            nbformat.v4.new_code_cell(
                "value = 6 * 7\nassert value == 42\nprint(value)\n",
                execution_count=1,
                outputs=[nbformat.v4.new_output("stream", name="stdout", text="42\n")],
            ),
        ]
    )

    assert remove_notebook_assertions(notebook, "lesson.ipynb") == 1
    assert notebook_assertion_count(notebook, "lesson.ipynb") == 0
    assert notebook.cells[0].source == "Keep this prose."
    assert notebook.cells[1].source == "value = 6 * 7\nprint(value)\n"
    assert notebook.cells[1].outputs[0].text == "42\n"
    assert notebook.cells[1].execution_count == 1


def test_assertion_filter_removes_entire_validation_lines_without_trailing_space() -> None:
    source = """def answer():
    value = 42
    assert value == 42  # validation-only comment
    return value
"""

    published = without_assertions(source)

    assert published == "def answer():\n    value = 42\n    return value\n"
    assert all(line == line.rstrip() for line in published.splitlines())
