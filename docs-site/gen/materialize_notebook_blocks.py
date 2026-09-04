"""Keep displayed lesson code identical to explicitly tagged notebook cells."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shlex
import sys

from common import CONTENT, NOTEBOOKS, cell_source as raw_cell_source, contained, curriculum, lesson_notebook_names
from publication_source import assertion_count, without_assertions
from run_lesson_snippets import extract_blocks

MARKER = re.compile(r"^[ \t]*(?:<!--\s*(/?notebook-block\b.*?)\s*-->|\{/\*\s*(/?notebook-block\b.*?)\s*\*/\})[ \t]*$")
MARKER_START = re.compile(r"^[ \t]*(?:<!--|\{/\*)\s*/?notebook-block\b")
CELL_ID = re.compile(r"[a-z0-9][a-z0-9_-]*\Z")


def cell_source(
    record: dict, fields: dict[str, str], notebook_root: Path, cache: dict[str, dict] | None = None
) -> str:
    """Resolve one mapped code cell and validate its author-owned metadata."""
    if set(fields) != {"notebook", "cell", "role"}:
        raise ValueError("Notebook block requires exactly notebook, cell and role attributes")
    relative, identifier, role = fields["notebook"], fields["cell"], fields["role"]
    path = contained(notebook_root, relative)
    if path.suffix != ".ipynb" or relative not in lesson_notebook_names(record):
        raise ValueError(f"{record['id']}: notebook is not mapped in labs/examples: {relative}")
    if not CELL_ID.fullmatch(identifier) or role not in {"build", "exercise"}:
        raise ValueError(f"{record['id']}: invalid notebook cell ID or role")
    if cache is not None and relative in cache:
        notebook = cache[relative]
    else:
        notebook = json.loads(path.read_text())
        if cache is not None:
            cache[relative] = notebook
    tagged = {}
    for cell in notebook.get("cells", []):
        tag = cell.get("metadata", {}).get("analyst_program")
        if tag is None:
            continue
        if not isinstance(tag, dict) or not isinstance(tag.get("id"), str) or not CELL_ID.fullmatch(tag["id"]):
            raise ValueError(f"{relative}: invalid analyst_program cell metadata")
        if tag["id"] in tagged:
            raise ValueError(f"{relative}: duplicate analyst_program cell ID {tag['id']}")
        tagged[tag["id"]] = cell
    if identifier not in tagged:
        raise ValueError(f"{relative}: missing tagged cell {identifier}")
    cell = tagged[identifier]
    tag = cell["metadata"]["analyst_program"]
    if cell.get("cell_type") != "code":
        raise ValueError(f"{relative}:{identifier}: tagged cell must contain Python code")
    if tag.get("lesson") != record["id"] or tag.get("role") != role:
        raise ValueError(f"{relative}:{identifier}: notebook metadata lesson/role differs from requested block")
    source = cell.get("source", "")
    if not isinstance(source, (str, list)) or (
        isinstance(source, list) and not all(isinstance(line, str) for line in source)
    ):
        raise TypeError(f"{relative}:{identifier}: cell source must be text")
    source = raw_cell_source(cell)
    if role == "exercise" and not assertion_count(source, f"{relative}:{identifier}"):
        raise ValueError(f"{relative}:{identifier}: exercise source requires an explicit assertion")
    return source


def _marker_fields(value: str, lesson_id: str) -> dict[str, str]:
    """Parse marker attributes without accepting duplicates or comment escapes."""
    fields = {}
    for word in shlex.split(value)[1:]:
        if "=" not in word:
            raise ValueError(f"{lesson_id}: invalid notebook-block attribute")
        key, field = word.split("=", 1)
        if key in fields:
            raise ValueError(f"{lesson_id}: duplicate notebook-block attribute {key}")
        if any(part in field for part in ("\n", "\r", "*/", "-->")):
            raise ValueError(f"{lesson_id}: invalid notebook-block attribute value")
        fields[key] = field
    return fields


def _render_cell(
    record: dict, fields: dict[str, str], notebook_root: Path, cache: dict[str, dict] | None = None
) -> tuple[str, bool]:
    """Render one sanitized notebook cell and report its canonical build gate."""
    source = cell_source(record, fields, notebook_root, cache)
    cell_name = f"{fields['notebook']}:{fields['cell']}"
    has_build_assertion = fields["role"] == "build" and bool(assertion_count(source, cell_name))
    published_source = without_assertions(source, cell_name)
    ticks = max([2, *(len(match[0]) for match in re.finditer(r"`+", published_source))]) + 1
    delimiter = "`" * ticks
    attributes = " ".join(
        f"{key}={json.dumps(fields[key], ensure_ascii=False)}" for key in ("notebook", "cell", "role")
    )
    rendered = (
        f"{{/* notebook-block {attributes} */}}\n"
        f"{delimiter}python exec id={fields['cell']} role={fields['role']}\n"
        + published_source
        + ("" if published_source.endswith("\n") else "\n")
        + f"{delimiter}\n{{/* /notebook-block */}}"
    )
    return rendered, has_build_assertion


def render_notebook_blocks(text: str, record: dict, notebook_root: Path = NOTEBOOKS) -> str:
    """Replace marked regions only; reject malformed, nested or duplicate blocks."""
    output = []
    opening = None
    seen = set()
    fence = None
    canonical_build_has_assertion = False
    notebook_cache: dict[str, dict] = {}
    for line in text.splitlines(keepends=True):
        stripped = line.rstrip("\r\n")
        marker = MARKER.fullmatch(stripped)
        if fence is not None:
            closing = re.match(r"^ {0,3}(`{3,}|~{3,})\s*$", stripped)
            if closing and closing[1][0] == fence[0] and len(closing[1]) >= len(fence):
                fence = None
            if opening is None:
                output.append(line)
            continue
        fence_start = re.match(r"^ {0,3}(`{3,}|~{3,})", stripped)
        if fence_start:
            fence = fence_start[1]
        if marker is None:
            if MARKER_START.match(stripped):
                raise ValueError(f"{record['id']}: malformed notebook-block marker")
            if opening is None:
                output.append(line)
            continue
        value = (marker[1] or marker[2]).strip()
        if value.startswith("/"):
            if value != "/notebook-block" or opening is None:
                raise ValueError(f"{record['id']}: unexpected notebook-block closing marker")
            fields = opening
            rendered, has_build_assertion = _render_cell(record, fields, notebook_root, notebook_cache)
            canonical_build_has_assertion |= has_build_assertion
            ending = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            output.append(rendered + ending)
            opening = None
            continue
        if opening is not None:
            raise ValueError(f"{record['id']}: nested notebook-block markers")
        fields = _marker_fields(value, record["id"])
        if fields.get("cell") in seen:
            raise ValueError(f"{record['id']}: duplicate included cell ID {fields.get('cell')}")
        seen.add(fields.get("cell"))
        opening = fields
    if opening is not None:
        raise ValueError(f"{record['id']}: unclosed notebook-block marker")
    result = "".join(output)
    # This also rejects collisions with hand-authored executable block IDs and
    # any assertion that did not originate from a canonical notebook cell.
    blocks = extract_blocks(result, f"lesson:{record['id']}")
    if blocks and not canonical_build_has_assertion:
        raise ValueError(f"lesson:{record['id']}: canonical build sequence requires an explicit assertion")
    return result


def materialize_lesson(
    record: dict, content_root: Path = CONTENT, notebook_root: Path = NOTEBOOKS, *, check_only: bool = False
) -> bool:
    """Refresh one lesson's marked code, or fail if a read-only check finds drift."""
    path = contained(content_root, record["path"])
    with path.open(newline="") as handle:
        before = handle.read()
    after = render_notebook_blocks(before, record, notebook_root)
    if after == before:
        return False
    if check_only:
        raise ValueError(f"{record['id']}: notebook blocks are stale; materialize them before validation")
    with path.open("w", newline="") as handle:
        handle.write(after)
    return True


def main() -> int:
    """Materialize all lessons or a selected stable ID before execution."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lesson", action="append", help="Stable lesson ID; repeat to select several")
    parser.add_argument("--check", action="store_true", help="Reject drift without updating marked regions")
    args = parser.parse_args()
    records = curriculum()
    if args.lesson:
        unknown = set(args.lesson) - {record["id"] for record in records}
        if unknown:
            parser.error(f"Unknown lesson IDs: {sorted(unknown)}")
        records = [record for record in records if record["id"] in args.lesson]
    errors = []
    changed = 0
    for record in records:
        try:
            changed += materialize_lesson(record, check_only=args.check)
        except (ValueError, OSError, SyntaxError, TypeError) as error:
            errors.append(f"{record['id']}: {error}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Notebook blocks current: {len(records)} lessons checked; {changed} updated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
