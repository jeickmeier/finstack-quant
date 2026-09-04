"""Execute displayed Python lesson blocks with isolated exercise baselines."""

from __future__ import annotations

import argparse
import ast
from collections import Counter
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import asdict, dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

import common
from common import (
    BUILD,
    CONTENT,
    NOTEBOOKS,
    REPO,
    SITE,
    SNIPPET_EXECUTION_POLICY,
    cell_source,
    contained,
    curriculum,
    digest,
    execution_environment,
    fixture_digest,
    frontmatter,
    lesson_notebook_names,
    lesson_notebooks,
    notebook_closure,
    runtime_identity,
    write_json,
)
import publication_source
from publication_source import (
    assertion_count,
    learner_assertion_count,
    remove_notebook_assertions,
    without_assertions,
)


@dataclass(frozen=True)
class Block:
    """One displayed, executable code block and its source location."""

    id: str
    role: str
    code: str
    line: int


def snippet_provenance() -> dict:
    """Bind captured outputs to the strict runner, worker and policy."""
    return {
        "execution_policy": SNIPPET_EXECUTION_POLICY,
        "runner_sha256": digest(Path(__file__).resolve()),
        "worker_sha256": digest(SITE / "gen" / "snippet_worker.py"),
        "publication_filter_sha256": digest(Path(publication_source.__file__).resolve()),
        "common_sha256": digest(Path(common.__file__).resolve()),
    }


def extract_blocks(text: str, source: str = "<lesson>") -> list[Block]:
    """Extract top-level Markdown fences and validate executable block metadata."""
    blocks: list[Block] = []
    seen: set[str] = set()
    fence = None
    info = ""
    lines: list[str] = []
    start = 0
    for number, line in enumerate(text.splitlines(), 1):
        match = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
        if fence is None:
            if match:
                fence, info, start = match[1], match[2].strip(), number + 1
                lines = []
            continue
        if match and match[1][0] == fence[0] and len(match[1]) >= len(fence) and not match[2].strip():
            words = shlex.split(info)
            if words and words[0].lower() in {"python", "python3", "py"} and words[:2] != ["python", "exec"]:
                raise ValueError(f"{source}:{start}: displayed Python must use a 'python exec id=...' fence")
            if words[:2] == ["python", "exec"]:
                fields = dict(item.split("=", 1) for item in words[2:] if "=" in item)
                identifier = fields.get("id", "")
                role = fields.get("role", "build")
                if not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", identifier) or identifier in seen:
                    raise ValueError(f"{source}:{start}: missing, invalid or duplicate executable block id")
                if role not in {"build", "exercise"}:
                    raise ValueError(f"{source}:{start}: unsupported block role {role!r}")
                code = "\n".join(lines) + "\n"
                if learner_assertion_count(code, source):
                    raise ValueError(
                        f"{source}:{start}: learner-facing Python cannot contain assertions "
                        "or explicit raise AssertionError"
                    )
                blocks.append(Block(identifier, role, code, start))
                seen.add(identifier)
            fence = None
        else:
            lines.append(line)
    if fence is not None:
        raise ValueError(f"{source}:{start}: unclosed Markdown fence")
    return blocks


def canonical_execution_blocks(record: dict, blocks: list[Block], notebook_root: Path = NOTEBOOKS) -> list[Block]:
    """Resolve assertion-bearing notebook sources for assertion-free displayed blocks."""
    expected = {block.id: block for block in blocks}
    resolved: dict[str, Block] = {}
    for relative in sorted(set(lesson_notebook_names(record))):
        notebook = json.loads(contained(notebook_root, relative).read_text())
        for cell in notebook.get("cells", []):
            tag = cell.get("metadata", {}).get("analyst_program")
            if not isinstance(tag, dict) or tag.get("lesson") != record["id"] or tag.get("id") not in expected:
                continue
            identifier = tag["id"]
            displayed = expected[identifier]
            if identifier in resolved:
                raise ValueError(f"{record['id']}: duplicate canonical source cell {identifier}")
            if cell.get("cell_type") != "code" or tag.get("role") != displayed.role:
                raise ValueError(f"{record['id']}:{identifier}: canonical source type or role differs from display")
            raw_source = cell.get("source", "")
            if not isinstance(raw_source, (str, list)) or (
                isinstance(raw_source, list) and not all(isinstance(line, str) for line in raw_source)
            ):
                raise TypeError(f"{record['id']}:{identifier}: canonical cell source must be text")
            source = cell_source(cell)
            published = without_assertions(source, f"{relative}:{identifier}")
            published = published + ("" if published.endswith("\n") else "\n")
            if published != displayed.code:
                raise ValueError(
                    f"{record['id']}:{identifier}: displayed code differs from canonical publication source"
                )
            if displayed.role == "exercise" and not assertion_count(source, f"{relative}:{identifier}"):
                raise ValueError(f"{record['id']}:{identifier}: canonical exercise lacks an assertion")
            resolved[identifier] = Block(identifier, displayed.role, source, displayed.line)
    missing = expected.keys() - resolved.keys()
    if missing:
        raise ValueError(f"{record['id']}: displayed blocks lack canonical sources: {sorted(missing)}")
    ordered = [resolved[block.id] for block in blocks]
    if ordered and not any(block.role == "build" and assertion_count(block.code) for block in ordered):
        raise ValueError(f"{record['id']}: canonical build sequence lacks an assertion")
    return ordered


def canonical_assertion_locations(blocks: list[Block]) -> dict[str, list[dict[str, str | int]]]:
    """Return stable block-local coordinates for every canonical assertion."""
    locations = {}
    for block in blocks:
        assertions = sorted(
            (node for node in ast.walk(ast.parse(block.code)) if isinstance(node, ast.Assert)),
            key=lambda node: (node.lineno, node.col_offset),
        )
        locations[block.id] = [
            {
                "id": f"{block.id}:{node.lineno}:{node.col_offset}",
                "line": node.lineno,
                "column": node.col_offset,
            }
            for node in assertions
        ]
    return locations


def canonical_validation_manifest(blocks: list[Block], executed: set[str] | None = None) -> dict:
    """Describe the exact hidden sources and per-assert execution coverage."""
    locations = canonical_assertion_locations(blocks)
    expected = {location["id"] for block_locations in locations.values() for location in block_locations}
    executed = expected if executed is None else executed
    entries = [
        {
            "id": block.id,
            "role": block.role,
            "source_sha256": hashlib.sha256(block.code.encode()).hexdigest(),
            "assertions": len(locations[block.id]),
            "assertion_ids": [location["id"] for location in locations[block.id]],
        }
        for block in blocks
    ]
    coverage = [
        {**location, "block": block_id, "executed": location["id"] in executed}
        for block_id, block_locations in locations.items()
        for location in block_locations
    ]
    return {
        "assertions": sum(entry["assertions"] for entry in entries),
        "assertion_coverage": coverage,
        "blocks": entries,
    }


def validate_output_references(text: str, lesson_id: str, blocks: list[Block], source: str = "<lesson>") -> None:
    """Require one literal output reference to this lesson for every displayed block."""
    prose = []
    fence = None
    for line in text.splitlines():
        match = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
        if fence is None:
            if match:
                fence = match[1]
            else:
                prose.append(line)
        elif match and match[1][0] == fence[0] and len(match[1]) >= len(fence) and not match[2].strip():
            fence = None
    visible = re.sub(r"\{/\*.*?\*/\}|<!--.*?-->", "", "\n".join(prose), flags=re.S)
    tags = re.findall(r"<ExecutedOutput\b([^>]*)>", visible, re.S)
    if len(tags) != len(re.findall(r"<ExecutedOutput\b", visible)):
        raise ValueError(f"{source}: malformed ExecutedOutput component")
    references = []
    attribute = re.compile(r"\b(lesson|block)\s*=\s*([\"'])(.*?)\2", re.S)
    for tag in tags:
        fields = attribute.findall(tag)
        if attribute.sub("", tag).strip() != "/" or Counter(name for name, _, _ in fields) != {"lesson": 1, "block": 1}:
            raise ValueError(
                f"{source}: ExecutedOutput requires literal lesson and block attributes and a closing '/>'"
            )
        values = {name: value for name, _, value in fields}
        if values["lesson"] != lesson_id:
            raise ValueError(f"{source}: ExecutedOutput must reference its own lesson {lesson_id}")
        references.append(values["block"])
    counts = Counter(references)
    expected = {block.id for block in blocks}
    missing, unknown = expected - counts.keys(), counts.keys() - expected
    repeated = {identifier for identifier, count in counts.items() if count != 1}
    if missing or unknown or repeated:
        raise ValueError(
            f"{source}: ExecutedOutput coverage differs from displayed blocks; "
            f"missing={sorted(missing)}, unknown={sorted(unknown)}, repeated={sorted(repeated)}"
        )


def run_process(
    lesson_id: str,
    blocks: list[Block],
    capture: list[str],
    timeout: int,
    notebook_root: Path | None = None,
    *,
    publish_assets: bool = True,
    asset_root: Path | None = None,
    track_assertions: bool = False,
) -> list[dict]:
    """Execute one build sequence or exercise replay in a fresh interpreter."""
    notebook_root = NOTEBOOKS if notebook_root is None else notebook_root
    payload = {
        "lesson_id": lesson_id,
        "blocks": [asdict(block) for block in blocks],
        "capture": capture,
        "expected_runtime": runtime_identity(),
        "publish_assets": publish_assets,
        "allowed_notebook_root": str(notebook_root.resolve()),
    }
    if asset_root is not None:
        payload["asset_root"] = str(asset_root)
    if track_assertions:
        payload["assertion_locations"] = canonical_assertion_locations(blocks)
    env = execution_environment(BUILD)
    env["PYTHONOPTIMIZE"] = "0"
    env["PYTHONPATH"] = os.pathsep.join([str(notebook_root), str(REPO / "finstack-quant-py"), str(REPO)])
    with tempfile.TemporaryDirectory(prefix="analyst-snippet-") as directory:
        root = Path(directory)
        request = root / "request.json"
        result = root / "result.json"
        request.write_text(json.dumps(payload))
        completed = subprocess.run(
            [sys.executable, str(SITE / "gen" / "snippet_worker.py"), str(request), str(result)],
            cwd=root,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        if completed.returncode or completed.stderr.strip():
            reason = "failed" if completed.returncode else "emitted stderr"
            raise RuntimeError(f"{lesson_id}: snippet process {reason}\n{completed.stderr}\n{completed.stdout}")
        return json.loads(result.read_text())


def validate_assertion_coverage(blocks: list[Block], results: list[dict], lesson_id: str) -> set[str]:
    """Require every canonical assertion to execute in at least one isolated run."""
    locations = canonical_assertion_locations(blocks)
    expected = {location["id"] for block_locations in locations.values() for location in block_locations}
    executed = {
        identifier
        for result in results
        for identifier in result.get("assertion_hits", [])
        if isinstance(identifier, str)
    }
    missing = sorted(expected - executed)
    unexpected = sorted(executed - expected)
    if missing or unexpected:
        raise RuntimeError(
            f"{lesson_id}: canonical assertion coverage is incomplete; missing={missing}, unexpected={unexpected}"
        )
    return executed


def validate_execution_equivalence(published: list[dict], canonical: list[dict], lesson_id: str) -> None:
    """Require hidden checks to leave every captured learner result unchanged."""

    def observable(result: dict) -> dict:
        return {key: result.get(key) for key in ("id", "role", "stdout", "assets", "asset_sha256")}

    published_by_id = {result["id"]: observable(result) for result in published}
    canonical_by_id = {result["id"]: observable(result) for result in canonical}
    if published_by_id != canonical_by_id:
        changed = sorted(
            identifier
            for identifier in published_by_id.keys() | canonical_by_id.keys()
            if published_by_id.get(identifier) != canonical_by_id.get(identifier)
        )
        raise RuntimeError(f"{lesson_id}: canonical checks change learner-visible results for blocks {changed}")


@contextmanager
def projected_notebook_root(
    record: dict, notebook_root: Path | None = None, *, remove_assertions: bool
) -> Iterator[Path]:
    """Yield one declared notebook closure with the requested assertion policy."""
    notebook_root = NOTEBOOKS if notebook_root is None else notebook_root
    resource_sources = [
        source
        for source in notebook_root.rglob("*")
        if source.is_file()
        and source.suffix != ".ipynb"
        and "_shared" not in source.relative_to(notebook_root).parts
        and not any(part in {"__pycache__", ".ipynb_checkpoints"} for part in source.parts)
        and source.suffix.lower() not in {".html", ".pdf"}
    ]
    # Keep the same repository-relative depth as the lab builder. Shared
    # fixtures deliberately resolve canonical calibration inputs from their
    # checked-out repository location.
    with tempfile.TemporaryDirectory(prefix=".snippet-publication-", dir=SITE) as directory:
        projected = Path(directory) / "notebooks"
        projected.mkdir()
        shared = notebook_root / "_shared"
        if shared.is_dir():
            shutil.copytree(
                shared,
                projected / "_shared",
                ignore=shutil.ignore_patterns("__pycache__", ".ipynb_checkpoints"),
            )
        for source in resource_sources:
            target = contained(projected, str(source.relative_to(notebook_root)))
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        names = notebook_closure(lesson_notebook_names(record), notebook_root)
        for relative in names:
            source = json.loads(contained(notebook_root, relative).read_text())
            if remove_assertions:
                remove_notebook_assertions(source, relative)
            target = contained(projected, relative)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(json.dumps(source, indent=1, ensure_ascii=False) + "\n")
        yield projected


@contextmanager
def publication_notebook_root(record: dict, notebook_root: Path | None = None) -> Iterator[Path]:
    """Yield the assertion-free declared notebook closure visible to learners."""
    with projected_notebook_root(record, notebook_root, remove_assertions=True) as projected:
        yield projected


@contextmanager
def canonical_notebook_root(record: dict, notebook_root: Path | None = None) -> Iterator[Path]:
    """Yield the assertion-retaining declared notebook closure used for validation."""
    with projected_notebook_root(record, notebook_root, remove_assertions=False) as projected:
        yield projected


@contextmanager
def staged_snippet_assets(identifier: str) -> Iterator[Path]:
    """Yield an empty asset root that can replace one lesson only after validation."""
    public_root = SITE / "public" / "snippet-assets"
    public_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f".{identifier}-", dir=public_root) as directory:
        root = Path(directory)
        (root / identifier).mkdir()
        yield root


def publish_lesson_assets(identifier: str, staged_root: Path) -> None:
    """Replace one lesson's asset tree while retaining the prior tree for rollback."""
    public_root = SITE / "public" / "snippet-assets"
    public_root.mkdir(parents=True, exist_ok=True)
    staged = staged_root / identifier
    published = public_root / identifier
    backup_root = None
    backup = None
    if published.exists() or published.is_symlink():
        backup_root = Path(tempfile.mkdtemp(prefix=f".{identifier}-previous-", dir=public_root))
        backup = backup_root / identifier
        published.rename(backup)
    try:
        staged.rename(published)
    except OSError:
        if backup is not None:
            backup.rename(published)
        raise
    if backup_root is not None:
        shutil.rmtree(backup_root, ignore_errors=True)


def run_lesson(record: dict, timeout: int = 600, require_blocks: bool = False) -> dict:
    """Run a lesson and write outputs only after every requested block succeeds."""
    from materialize_notebook_blocks import materialize_lesson

    started = time.monotonic()
    output_path = BUILD / "snippets" / f"{record['id']}.json"
    output_path.unlink(missing_ok=True)
    materialize_lesson(record, content_root=CONTENT, notebook_root=NOTEBOOKS)
    path = CONTENT / record["path"]
    metadata, _ = frontmatter(path)
    text = path.read_text()
    blocks = extract_blocks(text, str(path))
    validate_output_references(text, record["id"], blocks, str(path))
    execution_blocks = canonical_execution_blocks(record, blocks, NOTEBOOKS)
    source_before = digest(path)
    fixtures_before = fixture_digest()
    notebooks_before = lesson_notebooks(record, NOTEBOOKS)
    runtime_before = runtime_identity()
    provenance_before = snippet_provenance()
    if not blocks and (metadata["status"] == "published" or require_blocks):
        raise ValueError(f"{record['id']}: no executable proof blocks")
    displayed_builds = [block for block in blocks if block.role == "build"]
    with staged_snippet_assets(record["id"]) as asset_root:
        with publication_notebook_root(record, NOTEBOOKS) as published_notebooks:
            output = (
                run_process(
                    record["id"],
                    displayed_builds,
                    [block.id for block in displayed_builds],
                    timeout,
                    published_notebooks,
                    asset_root=asset_root,
                )
                if displayed_builds
                else []
            )
            for exercise in (block for block in blocks if block.role == "exercise"):
                output.extend(
                    run_process(
                        record["id"],
                        [*displayed_builds, exercise],
                        [exercise.id],
                        timeout,
                        published_notebooks,
                        asset_root=asset_root,
                    )
                )

        canonical_output = []
        canonical_builds = [block for block in execution_blocks if block.role == "build"]
        with canonical_notebook_root(record, NOTEBOOKS) as canonical_notebooks:
            if canonical_builds:
                canonical_output.extend(
                    run_process(
                        record["id"],
                        canonical_builds,
                        [block.id for block in canonical_builds],
                        timeout,
                        canonical_notebooks,
                        publish_assets=False,
                        track_assertions=True,
                    )
                )
            for exercise in (block for block in execution_blocks if block.role == "exercise"):
                canonical_output.extend(
                    run_process(
                        record["id"],
                        [*canonical_builds, exercise],
                        [exercise.id],
                        timeout,
                        canonical_notebooks,
                        publish_assets=False,
                        track_assertions=True,
                    )
                )
        validate_execution_equivalence(output, canonical_output, record["id"])
        executed = validate_assertion_coverage(execution_blocks, canonical_output, record["id"])
        canonical_validation = {
            "status": "passed",
            **canonical_validation_manifest(execution_blocks, executed),
        }
        if (
            digest(path) != source_before
            or fixture_digest() != fixtures_before
            or lesson_notebooks(record, NOTEBOOKS) != notebooks_before
            or runtime_identity() != runtime_before
            or snippet_provenance() != provenance_before
        ):
            raise RuntimeError(f"{record['id']}: lesson or fixture sources changed during execution; repeat the run")
        result = {
            "lesson_id": record["id"],
            "source_sha256": source_before,
            "fixtures_sha256": fixtures_before,
            "notebooks_sha256": notebooks_before,
            "runtime": runtime_before,
            **provenance_before,
            "status": "passed" if blocks else "draft",
            "canonical_validation": canonical_validation,
            "duration_seconds": time.monotonic() - started,
            "blocks": output,
        }
        publish_lesson_assets(record["id"], asset_root)
    write_json(output_path, result)
    return result


def main() -> int:
    """Run all lessons, or the selected stable lesson ID."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lesson", action="append", help="Stable lesson ID; repeat to select several")
    parser.add_argument("--timeout", type=int, default=600, help="Seconds per isolated lesson or exercise process")
    parser.add_argument("--require-all", action="store_true", help="Require proof blocks even in drafts")
    args = parser.parse_args()
    records = curriculum()
    if args.lesson:
        unknown = set(args.lesson) - {record["id"] for record in records}
        if unknown:
            parser.error(f"Unknown lesson IDs: {sorted(unknown)}")
        records = [record for record in records if record["id"] in args.lesson]
    failures = []
    for record in records:
        try:
            result = run_lesson(record, args.timeout, args.require_all)
            print(f"{record['id']}: {result['status']} ({len(result['blocks'])} blocks)", flush=True)
        except (ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
            failures.append(record["id"])
            print(str(error), file=sys.stderr, flush=True)
    if failures:
        print(f"Failed lessons: {', '.join(failures)}", file=sys.stderr)
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
