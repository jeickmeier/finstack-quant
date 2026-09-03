"""Execute displayed Python lesson blocks with isolated exercise baselines."""

from __future__ import annotations

import argparse
import ast
from collections import Counter
from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time

from common import (
    BUILD,
    CONTENT,
    NOTEBOOKS,
    REPO,
    SITE,
    curriculum,
    digest,
    fixture_digest,
    frontmatter,
    lesson_notebooks,
    runtime_identity,
    write_json,
)


@dataclass(frozen=True)
class Block:
    """One displayed, executable code block and its source location."""

    id: str
    role: str
    code: str
    line: int


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
                parsed = ast.parse(code, filename=source)
                if role == "exercise" and not any(isinstance(node, ast.Assert) for node in ast.walk(parsed)):
                    raise ValueError(f"{source}:{start}: exercise blocks require an explicit assertion")
                blocks.append(Block(identifier, role, code, start))
                seen.add(identifier)
            fence = None
        else:
            lines.append(line)
    if fence is not None:
        raise ValueError(f"{source}:{start}: unclosed Markdown fence")
    if blocks and not any(
        isinstance(node, ast.Assert)
        for block in blocks
        if block.role == "build"
        for node in ast.walk(ast.parse(block.code))
    ):
        raise ValueError(f"{source}: the build sequence requires at least one explicit assertion")
    return blocks


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
        if (
            attribute.sub("", tag).strip() != "/"
            or Counter(name for name, _, _ in fields) != {"lesson": 1, "block": 1}
        ):
            raise ValueError(f"{source}: ExecutedOutput requires literal lesson and block attributes and a closing '/>'")
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


def run_process(lesson_id: str, blocks: list[Block], capture: list[str], timeout: int) -> list[dict]:
    """Execute one build sequence or exercise replay in a fresh interpreter."""
    payload = {"lesson_id": lesson_id, "blocks": [asdict(block) for block in blocks], "capture": capture}
    env = os.environ.copy()
    env["PYTHONOPTIMIZE"] = "0"
    env["PYTHONPATH"] = os.pathsep.join([str(NOTEBOOKS), str(REPO / "finstack-quant-py"), str(REPO)])
    env["MPLBACKEND"] = "Agg"
    plot_cache = BUILD / "cache" / "matplotlib"
    plot_cache.mkdir(parents=True, exist_ok=True)
    env.setdefault("MPLCONFIGDIR", str(plot_cache))
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
        if completed.returncode:
            raise RuntimeError(f"{lesson_id}: snippet process failed\n{completed.stderr}\n{completed.stdout}")
        return json.loads(result.read_text())


def run_lesson(record: dict, timeout: int = 600, require_blocks: bool = False) -> dict:
    """Run a lesson and write outputs only after every requested block succeeds."""
    from materialize_notebook_blocks import materialize_lesson

    started = time.monotonic()
    output_path = BUILD / "snippets" / f"{record['id']}.json"
    output_path.unlink(missing_ok=True)
    materialize_lesson(record, content_root=CONTENT, notebook_root=NOTEBOOKS)
    path = CONTENT / record["path"]
    metadata, _ = frontmatter(path)
    blocks = extract_blocks(path.read_text(), str(path))
    validate_output_references(path.read_text(), record["id"], blocks, str(path))
    source_before = digest(path)
    fixtures_before = fixture_digest()
    notebooks_before = lesson_notebooks(record, NOTEBOOKS)
    runtime_before = runtime_identity()
    if not blocks and (metadata["status"] == "published" or require_blocks):
        raise ValueError(f"{record['id']}: no executable proof blocks")
    builds = [block for block in blocks if block.role == "build"]
    output = run_process(record["id"], builds, [block.id for block in builds], timeout) if builds else []
    for exercise in (block for block in blocks if block.role == "exercise"):
        output.extend(run_process(record["id"], [*builds, exercise], [exercise.id], timeout))
    if (
        digest(path) != source_before
        or fixture_digest() != fixtures_before
        or lesson_notebooks(record, NOTEBOOKS) != notebooks_before
        or runtime_identity() != runtime_before
    ):
        raise RuntimeError(f"{record['id']}: lesson or fixture sources changed during execution; repeat the run")
    result = {
        "lesson_id": record["id"],
        "source_sha256": source_before,
        "fixtures_sha256": fixtures_before,
        "notebooks_sha256": notebooks_before,
        "runtime": runtime_before,
        "status": "passed" if blocks else "draft",
        "duration_seconds": time.monotonic() - started,
        "blocks": output,
    }
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
