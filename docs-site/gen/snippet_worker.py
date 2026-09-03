"""Private subprocess entry point for isolated lesson execution."""

from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import shutil
import sys
import time
import warnings

SITE = Path(__file__).resolve().parents[1]


def files() -> dict[Path, str]:
    """Snapshot exportable files without modifying the lesson's working state."""
    return {
        file: hashlib.sha256(file.read_bytes()).hexdigest()
        for file in Path.cwd().rglob("*")
        if file.is_file() and file.suffix.lower() in {".html", ".svg", ".png", ".pdf", ".csv", ".tsv", ".json"}
    }


def main() -> None:
    """Execute the supplied sequence and capture designated block outputs."""
    request = json.loads(Path(sys.argv[1]).read_text())
    expected = request["expected_runtime"]
    if Path(sys.executable).resolve() != Path(expected["python_executable"]):
        raise RuntimeError("Snippet worker interpreter does not match the recorded runtime")
    extension = __import__("finstack_quant.finstack_quant", fromlist=["finstack_quant"])
    extension_sha256 = hashlib.sha256(Path(extension.__file__).read_bytes()).hexdigest()
    if extension_sha256 != expected["extension_sha256"]:
        raise RuntimeError("Snippet worker loaded a different finstack extension")
    warnings.simplefilter("error")
    namespace = {"__name__": "__main__"}
    results = []
    figure_hashes = {}
    for block in request["blocks"]:
        before = files()
        stdout = io.StringIO()
        started = time.monotonic()
        with redirect_stdout(stdout):
            exec(
                compile(
                    "\n" * (block["line"] - 1) + block["code"],
                    f"lesson:{request['lesson_id']}/{block['id']}",
                    "exec",
                    optimize=0,
                ),
                namespace,
            )
        capture = block["id"] in request["capture"]
        assets = []
        asset_path = SITE / "public" / "snippet-assets" / request["lesson_id"] / block["id"]
        if capture:
            asset_path.mkdir(parents=True, exist_ok=True)
        if "matplotlib.pyplot" in sys.modules:
            pyplot = sys.modules["matplotlib.pyplot"]
            active = pyplot.gcf() if pyplot.get_fignums() else None
            for number in pyplot.get_fignums():
                name = f"figure-{number}.png"
                buffer = io.BytesIO()
                pyplot.figure(number).savefig(buffer, format="png", dpi=144, bbox_inches="tight")
                data = buffer.getvalue()
                fingerprint = hashlib.sha256(data).hexdigest()
                if capture and figure_hashes.get(number) != fingerprint:
                    (asset_path / name).write_bytes(data)
                    assets.append(f"/snippet-assets/{request['lesson_id']}/{block['id']}/{name}")
                figure_hashes[number] = fingerprint
            if active is not None:
                pyplot.figure(active.number)
        if not capture:
            continue
        for file, fingerprint in files().items():
            if before.get(file) != fingerprint:
                name = file.relative_to(Path.cwd())
                target = asset_path / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(file, target)
                assets.append(f"/snippet-assets/{request['lesson_id']}/{block['id']}/{name.as_posix()}")
        results.append({
            "id": block["id"],
            "role": block["role"],
            "stdout": stdout.getvalue(),
            "assets": assets,
            "duration_seconds": time.monotonic() - started,
        })
    Path(sys.argv[2]).write_text(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
