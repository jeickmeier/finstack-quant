"""Private subprocess entry point for isolated lesson execution."""

from __future__ import annotations

import ast
from contextlib import redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import time
import warnings

SITE = Path(__file__).resolve().parents[1]
_ASSERTION_HIT_FUNCTION = "__finstack_record_assertion_hit__"


def path_segment(value: object, label: str) -> str:
    """Return one path segment, rejecting separators and parent-directory names."""
    if not isinstance(value, str) or value in {"", ".", ".."} or "/" in value or "\\" in value or "\x00" in value:
        raise RuntimeError(f"{label} must be a single path segment")
    return value


def confined_directory(root: Path, lesson_id: str, block_id: str) -> Path:
    """Return the block asset directory, which must resolve inside ``root``."""
    lesson = path_segment(lesson_id, "lesson_id")
    block = path_segment(block_id, "block id")
    resolved_root = root.resolve()
    asset_path = resolved_root / lesson / block
    if not asset_path.resolve().is_relative_to(resolved_root):
        raise RuntimeError(f"asset path escapes {resolved_root}: {asset_path}")
    return asset_path


def require_segments(request: dict) -> str:
    """Reject lesson and block ids that are not single path segments."""
    lesson_id = path_segment(request["lesson_id"], "lesson_id")
    for block in request["blocks"]:
        path_segment(block["id"], "block id")
    return lesson_id


def files() -> dict[Path, str]:
    """Snapshot exportable files without modifying the lesson's working state."""
    return {
        file: hashlib.sha256(file.read_bytes()).hexdigest()
        for file in Path.cwd().rglob("*")
        if file.is_file() and file.suffix.lower() in {".html", ".svg", ".png", ".pdf", ".csv", ".tsv", ".json"}
    }


def capture_figures(
    asset_path: Path,
    asset_prefix: str,
    figure_hashes: dict[int, str],
    *,
    capture: bool,
    publish_assets: bool,
) -> tuple[list[str], dict[str, str]]:
    """Capture newly created or changed Matplotlib figures for one block."""
    assets: list[str] = []
    hashes: dict[str, str] = {}
    if "matplotlib.pyplot" not in sys.modules:
        return assets, hashes
    pyplot = sys.modules["matplotlib.pyplot"]
    active = pyplot.gcf() if pyplot.get_fignums() else None
    for number in pyplot.get_fignums():
        name = f"figure-{number}.png"
        buffer = io.BytesIO()
        pyplot.figure(number).savefig(buffer, format="png", dpi=144, bbox_inches="tight")
        data = buffer.getvalue()
        fingerprint = hashlib.sha256(data).hexdigest()
        if capture and figure_hashes.get(number) != fingerprint:
            asset = asset_prefix + name
            if publish_assets:
                (asset_path / name).write_bytes(data)
            assets.append(asset)
            hashes[asset] = fingerprint
        figure_hashes[number] = fingerprint
    if active is not None:
        pyplot.figure(active.number)
    return assets, hashes


def capture_files(
    before: dict[Path, str],
    asset_path: Path,
    asset_prefix: str,
    asset_root: Path,
    *,
    publish_assets: bool,
) -> tuple[list[str], dict[str, str]]:
    """Capture exportable files created or changed by one block."""
    assets = []
    hashes = {}
    resolved_root = asset_root.resolve()
    for file, fingerprint in sorted(files().items()):
        if before.get(file) == fingerprint:
            continue
        name = file.relative_to(Path.cwd())
        asset = asset_prefix + name.as_posix()
        if publish_assets:
            target = asset_path / name
            if not target.resolve().is_relative_to(resolved_root):
                raise RuntimeError(f"captured file escapes {resolved_root}: {target}")
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, target)
        assets.append(asset)
        hashes[asset] = fingerprint
    return assets, hashes


class _AssertionInstrumenter(ast.NodeTransformer):
    """Insert one coverage hit immediately after each successful canonical assertion."""

    def __init__(self, locations: dict[tuple[int, int], str]) -> None:
        self.locations = locations

    def visit_Assert(self, node: ast.Assert) -> list[ast.stmt]:
        """Append the hit associated with one assertion's original coordinates."""
        self.generic_visit(node)
        identifier = self.locations[(node.lineno, node.col_offset)]
        hit = ast.Expr(
            value=ast.Call(
                func=ast.Name(id=_ASSERTION_HIT_FUNCTION, ctx=ast.Load()),
                args=[ast.Constant(value=identifier)],
                keywords=[],
            )
        )
        return [node, ast.copy_location(hit, node)]


def compile_block(block: dict, lesson_id: str, locations: list[dict] | None) -> object:
    """Compile one block, instrumenting exact assertion locations when requested."""
    filename = f"lesson:{lesson_id}/{block['id']}"
    if locations is None:
        return compile("\n" * (block["line"] - 1) + block["code"], filename, "exec", optimize=0)
    tree = ast.parse(block["code"], filename=filename)
    expected = {(location["line"], location["column"]): location["id"] for location in locations}
    actual = {(node.lineno, node.col_offset) for node in ast.walk(tree) if isinstance(node, ast.Assert)}
    if len(expected) != len(locations) or actual != set(expected):
        raise RuntimeError(f"{lesson_id}:{block['id']}: canonical assertion locations differ from the request")
    tree = _AssertionInstrumenter(expected).visit(tree)
    ast.fix_missing_locations(tree)
    ast.increment_lineno(tree, block["line"] - 1)
    return compile(tree, filename, "exec", optimize=0)


def restrict_notebook_access(allowed_root: Path) -> set[str]:
    """Reject notebook opens that resolve outside the run's declared projection."""
    allowed_root = allowed_root.resolve()
    violations: set[str] = set()

    def audit(event: str, args: tuple) -> None:
        if event != "open" or not args:
            return
        try:
            path = Path(os.fsdecode(os.fspath(args[0])))
        except TypeError:
            return
        if path.suffix.lower() != ".ipynb":
            return
        resolved = path.resolve()
        if resolved.is_relative_to(allowed_root):
            return
        violations.add(str(resolved))
        raise PermissionError(f"Notebook access must remain inside {allowed_root}: {path}")

    sys.addaudithook(audit)
    return violations


def main() -> None:
    """Execute the supplied sequence and capture designated block outputs."""
    request = json.loads(Path(sys.argv[1]).read_text())
    lesson_id = require_segments(request)
    expected = request["expected_runtime"]
    if Path(sys.executable).resolve() != Path(expected["python_executable"]):
        raise RuntimeError("Snippet worker interpreter does not match the recorded runtime")
    extension = __import__("finstack_quant.finstack_quant", fromlist=["finstack_quant"])
    extension_sha256 = hashlib.sha256(Path(extension.__file__).read_bytes()).hexdigest()
    if extension_sha256 != expected["extension_sha256"]:
        raise RuntimeError("Snippet worker loaded a different finstack extension")
    warnings.simplefilter("error")
    notebook_violations = restrict_notebook_access(Path(request["allowed_notebook_root"]))
    publish_assets = request.get("publish_assets", True)
    assertion_locations = request.get("assertion_locations")
    if assertion_locations is not None and set(assertion_locations) != {block["id"] for block in request["blocks"]}:
        raise RuntimeError("Canonical assertion coverage must describe every requested block")
    assertion_hits: set[str] = set()
    namespace = {"__name__": "__main__"}
    if assertion_locations is not None:
        namespace[_ASSERTION_HIT_FUNCTION] = assertion_hits.add
    asset_root = Path(request.get("asset_root", SITE / "public" / "snippet-assets")).resolve()
    results = []
    figure_hashes = {}
    for block in request["blocks"]:
        before = files()
        stdout = io.StringIO()
        started = time.monotonic()
        with redirect_stdout(stdout):
            exec(
                compile_block(
                    block,
                    lesson_id,
                    None if assertion_locations is None else assertion_locations[block["id"]],
                ),
                namespace,
            )
        if notebook_violations:
            raise RuntimeError(f"Notebook access escaped the declared projection: {sorted(notebook_violations)}")
        capture = block["id"] in request["capture"]
        asset_path = confined_directory(asset_root, lesson_id, block["id"])
        asset_prefix = f"/snippet-assets/{lesson_id}/{block['id']}/"
        if capture and publish_assets:
            if asset_path.is_dir():
                shutil.rmtree(asset_path)
            asset_path.mkdir(parents=True, exist_ok=True)
        assets, asset_hashes = capture_figures(
            asset_path,
            asset_prefix,
            figure_hashes,
            capture=capture,
            publish_assets=publish_assets,
        )
        if not capture:
            continue
        file_assets, file_hashes = capture_files(
            before,
            asset_path,
            asset_prefix,
            asset_root,
            publish_assets=publish_assets,
        )
        assets.extend(file_assets)
        asset_hashes.update(file_hashes)
        assets.sort()
        asset_hashes = {asset: asset_hashes[asset] for asset in assets}
        result = {
            "id": block["id"],
            "role": block["role"],
            "stdout": stdout.getvalue(),
            "assets": assets,
            "asset_sha256": asset_hashes,
            "duration_seconds": time.monotonic() - started,
        }
        if assertion_locations is not None:
            result["assertion_hits"] = sorted(assertion_hits)
        results.append(result)
    Path(sys.argv[2]).write_text(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
