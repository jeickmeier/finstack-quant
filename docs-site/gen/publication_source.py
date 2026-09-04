"""Remove validation-only assertions from learner-facing Python sources."""

from __future__ import annotations

import ast
from collections import defaultdict
from typing import Any

from common import cell_source

_MUTATING_ASSERT_METHODS = {
    "__delitem__",
    "__setitem__",
    "add",
    "append",
    "clear",
    "difference_update",
    "discard",
    "extend",
    "insert",
    "intersection_update",
    "mkdir",
    "pop",
    "popitem",
    "remove",
    "rename",
    "reverse",
    "save",
    "savefig",
    "seed",
    "setdefault",
    "sort",
    "symmetric_difference_update",
    "touch",
    "unlink",
    "update",
    "write",
    "write_bytes",
    "write_text",
    "writelines",
}
_MUTATING_ASSERT_FUNCTIONS = {"delattr", "eval", "exec", "input", "open", "print", "setattr"}


def assertion_count(source: str, filename: str = "<python>") -> int:
    """Count Python assertion statements in one source string."""
    return sum(isinstance(node, ast.Assert) for node in ast.walk(ast.parse(source, filename=filename)))


def _is_explicit_assertion_error(node: ast.AST | None) -> bool:
    """Return whether an exception expression names the built-in AssertionError directly."""
    if isinstance(node, ast.Call):
        node = node.func
    return (isinstance(node, ast.Name) and node.id == "AssertionError") or (
        isinstance(node, ast.Attribute)
        and isinstance(node.value, ast.Name)
        and node.value.id == "builtins"
        and node.attr == "AssertionError"
    )


def learner_assertion_count(source: str, filename: str = "<python>") -> int:
    """Count assertion statements and explicit AssertionError raises visible to learners."""
    tree = ast.parse(source, filename=filename)
    return sum(
        isinstance(node, ast.Assert) or (isinstance(node, ast.Raise) and _is_explicit_assertion_error(node.exc))
        for node in ast.walk(tree)
    )


def assertion_locations(source: str, filename: str = "<python>") -> list[tuple[int, int]]:
    """Return stable line and byte-column locations for assertion statements."""
    tree = ast.parse(source, filename=filename)
    return sorted((node.lineno, node.col_offset) for node in ast.walk(tree) if isinstance(node, ast.Assert))


def validate_assertions_are_observational(source: str, filename: str = "<python>") -> None:
    """Reject validation expressions that visibly mutate state or perform I/O."""
    tree = ast.parse(source, filename=filename)
    for statement in (node for node in ast.walk(tree) if isinstance(node, ast.Assert)):
        for node in ast.walk(statement.test):
            if isinstance(node, (ast.Await, ast.NamedExpr, ast.Yield, ast.YieldFrom)):
                construct = node.__class__.__name__
                raise TypeError(f"{filename}:{statement.lineno}: assertions must be observational; found {construct}")
            if not isinstance(node, ast.Call):
                continue
            if isinstance(node.func, ast.Attribute):
                name = node.func.attr
                mutating = name in _MUTATING_ASSERT_METHODS
            elif isinstance(node.func, ast.Name):
                name = node.func.id
                mutating = name in _MUTATING_ASSERT_FUNCTIONS
            else:
                name = "dynamic call"
                mutating = False
            if mutating:
                raise ValueError(
                    f"{filename}:{statement.lineno}: assertions must be observational; "
                    f"{name}() can change state or perform I/O"
                )


def _character_column(line: str, byte_column: int) -> int:
    """Convert an AST UTF-8 byte column to a Python string column."""
    return len(line.encode("utf-8")[:byte_column].decode("utf-8"))


def _assertions_requiring_pass(tree: ast.AST) -> set[ast.Assert]:
    """Find assertion-only suites that need a placeholder after filtering."""
    suite_members: dict[tuple[int, str], list[ast.stmt]] = {}
    suite_parents: dict[tuple[int, str], ast.AST] = {}
    suite_for_assertion: dict[ast.Assert, tuple[int, str]] = {}
    for parent in ast.walk(tree):
        for field, value in ast.iter_fields(parent):
            if not isinstance(value, list) or not value or not all(isinstance(item, ast.stmt) for item in value):
                continue
            key = (id(parent), field)
            suite_members[key] = value
            suite_parents[key] = parent
            for item in value:
                if isinstance(item, ast.Assert):
                    suite_for_assertion[item] = key

    keep_as_pass: set[ast.Assert] = set()
    grouped: dict[tuple[int, str], list[ast.Assert]] = defaultdict(list)
    for node, key in suite_for_assertion.items():
        grouped[key].append(node)
    for key, nodes in grouped.items():
        members = suite_members[key]
        if len(nodes) == len(members) and not isinstance(suite_parents[key], ast.Module):
            keep_as_pass.add(min(nodes, key=lambda node: (node.lineno, node.col_offset)))
    return keep_as_pass


def _source_offsets(lines: list[str]) -> list[int]:
    """Return each source line's absolute character offset."""
    offsets = []
    position = 0
    for line in lines:
        offsets.append(position)
        position += len(line)
    return offsets


def _replacement(
    node: ast.Assert, source: str, lines: list[str], offsets: list[int], keep_as_pass: set[ast.Assert], filename: str
) -> tuple[int, int, str]:
    """Locate one assertion and choose removal or a required pass statement."""
    if node.end_lineno is None or node.end_col_offset is None:
        raise ValueError(f"{filename}:{node.lineno}: assertion has no complete source span")
    start_line = lines[node.lineno - 1]
    end_line = lines[node.end_lineno - 1]
    start = offsets[node.lineno - 1] + _character_column(start_line, node.col_offset)
    end = offsets[node.end_lineno - 1] + _character_column(end_line, node.end_col_offset)
    replacement = "pass" if node in keep_as_pass else ""

    if replacement:
        return start, end, replacement
    line_start = offsets[node.lineno - 1]
    before = source[line_start:start]
    semicolon = before.rfind(";")
    if semicolon >= 0 and not before[semicolon + 1 :].strip():
        return line_start + semicolon, end, replacement
    end_line_start = offsets[node.end_lineno - 1]
    content_end = end_line_start + len(end_line.rstrip("\r\n"))
    after = source[end:content_end]
    stripped_after = after.lstrip(" \t")
    if stripped_after.startswith(";"):
        semicolon_start = end + len(after) - len(stripped_after)
        following = source[semicolon_start + 1 : content_end]
        if following.strip() and not following.lstrip().startswith("#"):
            consumed = len(following) - len(following.lstrip(" \t"))
            return start, semicolon_start + 1 + consumed, ""
    if not before.strip() and (not stripped_after or stripped_after.startswith((";", "#"))):
        next_line = offsets[node.end_lineno] if node.end_lineno < len(offsets) else len(source)
        return line_start, next_line, ""
    return start, end, replacement


def without_assertions(source: str, filename: str = "<python>") -> str:
    """Return source with assertion statements removed while preserving other text."""
    tree = ast.parse(source, filename=filename)
    assertions = [node for node in ast.walk(tree) if isinstance(node, ast.Assert)]
    if not assertions:
        return source
    validate_assertions_are_observational(source, filename)
    keep_as_pass = _assertions_requiring_pass(tree)
    lines = source.splitlines(keepends=True)
    offsets = _source_offsets(lines)
    replacements = [_replacement(node, source, lines, offsets, keep_as_pass, filename) for node in assertions]

    result = source
    for start, end, replacement in sorted(replacements, reverse=True):
        result = result[:start] + replacement + result[end:]

    published = ast.parse(result, filename=filename)
    if any(isinstance(node, ast.Assert) for node in ast.walk(published)):
        raise ValueError(f"{filename}: publication source still contains an assertion")
    return result


_cell_source = cell_source


def notebook_assertion_count(notebook: Any, filename: str = "<notebook>") -> int:
    """Count assertions across non-empty Python cells in a notebook object."""
    return sum(
        assertion_count(_cell_source(cell), f"{filename}:cell-{number}")
        for number, cell in enumerate(notebook.get("cells", []), start=1)
        if cell.get("cell_type") == "code" and _cell_source(cell).strip()
    )


def notebook_learner_assertion_count(notebook: Any, filename: str = "<notebook>") -> int:
    """Count learner-visible assertions and explicit AssertionError raises in a notebook."""
    return sum(
        learner_assertion_count(_cell_source(cell), f"{filename}:cell-{number}")
        for number, cell in enumerate(notebook.get("cells", []), start=1)
        if cell.get("cell_type") == "code" and _cell_source(cell).strip()
    )


def notebook_assertion_locations(notebook: Any, filename: str = "<notebook>") -> list[str]:
    """Return stable cell, line and byte-column locations for notebook assertions."""
    locations = []
    for cell_number, cell in enumerate(notebook.get("cells", []), start=1):
        if cell.get("cell_type") != "code" or not _cell_source(cell).strip():
            continue
        locations.extend(
            f"{cell_number}:{line}:{column}"
            for line, column in assertion_locations(_cell_source(cell), f"{filename}:cell-{cell_number}")
        )
    return sorted(locations)


def remove_notebook_assertions(notebook: Any, filename: str = "<notebook>") -> int:
    """Strip assertions from notebook code cells and return the removed count."""
    removed = 0
    for number, cell in enumerate(notebook.get("cells", []), start=1):
        if cell.get("cell_type") != "code" or not _cell_source(cell).strip():
            continue
        source = _cell_source(cell)
        cell_name = f"{filename}:cell-{number}"
        removed += assertion_count(source, cell_name)
        cell["source"] = without_assertions(source, cell_name)
    return removed
