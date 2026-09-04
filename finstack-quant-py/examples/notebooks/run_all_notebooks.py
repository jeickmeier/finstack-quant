#!/usr/bin/env python3
"""Execute all example notebooks and report pass/fail status.

Uses nbclient to run each notebook programmatically.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import importlib
import json
import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import time
import warnings

from nbclient import NotebookClient
from nbclient.exceptions import CellExecutionError
import nbformat
from traitlets.config import Config

NOTEBOOKS_DIR = Path(__file__).resolve().parent
_ASSERTION_MARKER = "__FINSTACK_ASSERTION_COVERAGE__="


class _AssertionInstrumenter(ast.NodeTransformer):
    """Record a stable location immediately after each assertion succeeds."""

    def __init__(self, cell_number: int) -> None:
        self.cell_number = cell_number
        self.locations: set[str] = set()

    def visit_Assert(self, node: ast.Assert) -> list[ast.stmt]:
        """Insert one execution marker without changing the assertion itself."""
        self.generic_visit(node)
        location = f"{self.cell_number}:{node.lineno}:{node.col_offset}"
        self.locations.add(location)
        marker = ast.Expr(
            value=ast.Call(
                func=ast.Name(id="__finstack_record_assertion__", ctx=ast.Load()),
                args=[ast.Constant(value=location)],
                keywords=[],
            )
        )
        return [node, ast.copy_location(marker, node)]


def _instrument_assertions(notebook: nbformat.NotebookNode) -> tuple[list[str], set[str]]:
    """Instrument assertion statements and return authored sources plus expected hits."""
    sources = [cell.source for cell in notebook.cells]
    expected: set[str] = set()
    for cell_number, cell in enumerate(notebook.cells, start=1):
        if cell.cell_type != "code" or not cell.source.strip():
            continue
        tree = ast.parse(cell.source, filename=f"notebook-cell-{cell_number}")
        instrumenter = _AssertionInstrumenter(cell_number)
        tree = instrumenter.visit(tree)
        expected.update(instrumenter.locations)
        if instrumenter.locations:
            cell.source = ast.unparse(ast.fix_missing_locations(tree)) + "\n"
    return sources, expected


def _assertion_hits(output_cell: nbformat.NotebookNode) -> set[str]:
    """Decode the private assertion coverage emitted by the final guard cell."""
    payloads = []
    for output in output_cell.get("outputs", []):
        if output.output_type != "stream" or output.get("name") != "stdout":
            continue
        payloads.extend(
            line.removeprefix(_ASSERTION_MARKER)
            for line in str(output.get("text", "")).splitlines()
            if line.startswith(_ASSERTION_MARKER)
        )
    if len(payloads) != 1:
        raise ValueError("Notebook assertion coverage guard did not return exactly one result")
    hits = json.loads(payloads[0])
    if not isinstance(hits, list) or not all(isinstance(item, str) for item in hits):
        raise TypeError("Notebook assertion coverage result must be a list of locations")
    return set(hits)


def _configure_pythonpath(notebook_root: Path = NOTEBOOKS_DIR) -> None:
    """Expose the selected notebook tree, Python package and repository."""
    repo_root = NOTEBOOKS_DIR.parents[2]
    extra_paths = [
        str(notebook_root.resolve()),
        str(repo_root / "finstack-quant-py"),
        str(repo_root),
    ]
    existing = os.environ.get("PYTHONPATH", "")
    pieces = [path for path in existing.split(os.pathsep) if path and path not in extra_paths]
    os.environ["PYTHONPATH"] = os.pathsep.join([*extra_paths, *pieces])


def find_notebooks(base_dir: Path, subdirectory: str | None = None) -> list[Path]:
    """Find notebooks under *base_dir*, optionally filtered to *subdirectory*.

    *subdirectory* may name either a directory to search recursively or a single
    ``.ipynb`` file, and is resolved relative to *base_dir*.
    """
    search_root = base_dir / subdirectory if subdirectory else base_dir
    if not search_root.exists():
        return []
    if search_root.is_file():
        return [search_root] if search_root.suffix == ".ipynb" else []
    notebooks = sorted(search_root.glob("**/*.ipynb"))
    return [nb for nb in notebooks if ".ipynb_checkpoints" not in str(nb)]


def run_notebook(
    notebook_path: Path,
    timeout: int,
    save_outputs: bool = False,
    notebook_root: Path = NOTEBOOKS_DIR,
) -> tuple[bool, str, float]:
    """Run a single notebook; return (success, message, elapsed_seconds).

    When *save_outputs* is set, a successfully executed notebook is written back
    with its freshly computed outputs. Notebooks that fail are never written, so
    a broken run cannot overwrite a good file.
    """
    _configure_pythonpath(notebook_root)
    start = time.time()
    try:
        # A verified notebook must already be valid on disk.  In particular,
        # do not let nbformat silently repair missing cell IDs while reading.
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            with open(notebook_path, encoding="utf-8") as f:
                nb = nbformat.read(f, as_version=4)
            nbformat.validate(nb)
        if any("skip-execution" in cell.get("metadata", {}).get("tags", []) for cell in nb.cells):
            raise ValueError("skip-execution tags are not allowed in verified notebooks")

        with TemporaryDirectory(prefix="finstack-notebook-") as ipc_dir:
            config = Config()
            config.KernelManager.transport = "ipc"
            config.KernelManager.ip = str(Path(ipc_dir) / "kernel")
            ipython_dir = Path(ipc_dir) / "ipython"
            ipython_dir.mkdir()
            previous_ipython_dir = os.environ.get("IPYTHONDIR")
            os.environ["IPYTHONDIR"] = str(ipython_dir)
            try:
                client = NotebookClient(
                    nb,
                    timeout=timeout,
                    kernel_name="python3",
                    resources={"metadata": {"path": str(notebook_path.parent)}},
                    config=config,
                    force_raise_errors=True,
                )
                authored_sources, expected_assertions = _instrument_assertions(nb)
                # Financial acceptance assertions must run even under an optimized caller.
                kernel_env = os.environ.copy()
                kernel_env["PYTHONOPTIMIZE"] = "0"
                kernel_env["FINSTACK_EXPECTED_PYTHON"] = str(Path(sys.executable).resolve())
                kernel_env["FINSTACK_ALLOWED_NOTEBOOK_ROOT"] = str(notebook_root.resolve())
                extension = importlib.import_module("finstack_quant.finstack_quant")
                kernel_env["FINSTACK_EXPECTED_EXTENSION_SHA256"] = hashlib.sha256(
                    Path(extension.__file__).read_bytes()
                ).hexdigest()
                # Configure warnings around each authored cell, after Jupyter has
                # booted and before it shuts down. This keeps kernel lifecycle
                # deprecations separate from warnings caused by lesson code. The
                # same guard proves the kernel uses this interpreter and extension.
                guard = nbformat.v4.new_code_cell(
                    "import hashlib as _finstack_hashlib\n"
                    "import importlib as _finstack_importlib\n"
                    "import os as _finstack_os\n"
                    "from pathlib import Path as _FinstackPath\n"
                    "import sys as _finstack_sys\n"
                    "import warnings as _finstack_warnings\n"
                    "__finstack_assertion_hits__ = set()\n"
                    "def __finstack_record_assertion__(location):\n"
                    "    __finstack_assertion_hits__.add(location)\n"
                    "_finstack_allowed_notebook_root = _FinstackPath(\n"
                    "    _finstack_os.environ['FINSTACK_ALLOWED_NOTEBOOK_ROOT']\n"
                    ").resolve()\n"
                    "def _finstack_restrict_notebook_open(\n"
                    "    event, args, _allowed=_finstack_allowed_notebook_root,\n"
                    "    _Path=_FinstackPath, _os=_finstack_os, _error=PermissionError\n"
                    "):\n"
                    "    if event != 'open' or not args:\n"
                    "        return\n"
                    "    raw_path = args[0]\n"
                    "    if not isinstance(raw_path, (str, bytes, _os.PathLike)):\n"
                    "        return\n"
                    "    candidate = _Path(_os.fsdecode(raw_path))\n"
                    "    if candidate.suffix.lower() != '.ipynb':\n"
                    "        return\n"
                    "    resolved = candidate.resolve()\n"
                    "    if not resolved.is_relative_to(_allowed):\n"
                    "        raise _error(\n"
                    "            f'Notebook access outside declared root: {resolved}'\n"
                    "        )\n"
                    "_finstack_sys.addaudithook(_finstack_restrict_notebook_open)\n"
                    "_finstack_expected_python = _FinstackPath("
                    "_finstack_os.environ['FINSTACK_EXPECTED_PYTHON']).resolve()\n"
                    "assert _FinstackPath(_finstack_sys.executable).resolve() == _finstack_expected_python, "
                    "'Notebook kernel interpreter mismatch: ' "
                    "+ f'{_finstack_sys.executable} != {_finstack_expected_python}'\n"
                    "_finstack_extension = _finstack_importlib.import_module('finstack_quant.finstack_quant')\n"
                    "_finstack_extension_sha256 = _finstack_hashlib.sha256("
                    "_FinstackPath(_finstack_extension.__file__).read_bytes()).hexdigest()\n"
                    "assert _finstack_extension_sha256 == "
                    "_finstack_os.environ['FINSTACK_EXPECTED_EXTENSION_SHA256'], "
                    "'Notebook kernel loaded a different finstack extension'\n"
                    "_finstack_warning_filters = list(_finstack_warnings.filters)\n"
                    "def _finstack_warning_start(info):\n"
                    "    global _finstack_warning_filters\n"
                    "    _finstack_warning_filters = list(_finstack_warnings.filters)\n"
                    "    _finstack_warnings.simplefilter('error')\n"
                    "def _finstack_warning_end(result):\n"
                    "    _finstack_warnings.filters[:] = _finstack_warning_filters\n"
                    "get_ipython().events.register('pre_run_cell', _finstack_warning_start)\n"
                    "get_ipython().events.register('post_run_cell', _finstack_warning_end)"
                )
                coverage = nbformat.v4.new_code_cell(
                    "import json as _finstack_json\n"
                    f"print('{_ASSERTION_MARKER}' + "
                    "_finstack_json.dumps(sorted(__finstack_assertion_hits__)))"
                )
                nb.cells.insert(0, guard)
                nb.cells.append(coverage)
                try:
                    client.execute(env=kernel_env)
                    hits = _assertion_hits(nb.cells[-1])
                    if hits != expected_assertions:
                        missing = sorted(expected_assertions - hits)
                        unexpected = sorted(hits - expected_assertions)
                        raise ValueError(
                            "Notebook did not execute every assertion statement: "
                            f"missing={missing[:20]}, unexpected={unexpected[:20]}"
                        )
                finally:
                    del nb.cells[-1]
                    del nb.cells[0]
                    for cell, source in zip(nb.cells, authored_sources, strict=True):
                        cell.source = source
                nb.metadata["finstack_execution"] = {
                    "assertions_total": len(expected_assertions),
                    "assertions_executed": len(hits),
                    "assertion_locations": sorted(hits),
                }
            finally:
                if previous_ipython_dir is None:
                    os.environ.pop("IPYTHONDIR", None)
                else:
                    os.environ["IPYTHONDIR"] = previous_ipython_dir
            # The temporary guard consumed execution count 1. Keep the saved
            # build copy numbered from the first authored code cell.
            for cell in nb.cells:
                if isinstance(cell.get("execution_count"), int):
                    cell.execution_count -= 1
                for output in cell.get("outputs", []):
                    if isinstance(output.get("execution_count"), int):
                        output.execution_count -= 1

        for cell_number, cell in enumerate(nb.cells, start=1):
            if cell.cell_type != "code" or not cell.source.strip():
                continue
            if cell.execution_count is None or any(output.output_type == "error" for output in cell.outputs):
                raise ValueError("Notebook contains a skipped or failed code cell")
            for output in cell.outputs:
                if output.output_type != "stream" or output.get("name") != "stderr":
                    continue
                stderr = str(output.get("text", "")).strip()
                if stderr:
                    preview = " ".join(stderr.splitlines())[:500]
                    raise ValueError(f"Notebook emitted stderr in cell {cell_number}: {preview}")

        elapsed = time.time() - start
        cell_count = sum(1 for c in nb.cells if c.cell_type == "code")
        if save_outputs:
            with open(notebook_path, "w", encoding="utf-8") as f:
                nbformat.write(nb, f)
            return True, f"Executed {cell_count} code cells; outputs saved", elapsed
        return True, f"Executed {cell_count} code cells", elapsed

    except CellExecutionError as e:
        elapsed = time.time() - start
        lines = str(e).split("\n")
        # Skip the leading "An error occurred..." banner so we surface the
        # actual exception (AttributeError, ValueError, ...) rather than the
        # cell source that always contains the word "error".
        for i in range(len(lines) - 1, -1, -1):
            line = lines[i]
            if line.startswith(("-----", "An error occurred")):
                continue
            if "Error" in line or "Exception" in line:
                start_idx = max(0, i - 2)
                end_idx = min(len(lines), i + 8)
                return False, "\n".join(lines[start_idx:end_idx]), elapsed
        return False, "\n".join(lines[-12:]), elapsed

    except TimeoutError as exc:
        detail = str(exc).strip()
        message = f"Timed out (>{timeout}s)"
        if detail:
            message = f"{message}\n{detail}"
        return False, message, time.time() - start

    except Exception as e:
        return False, f"{type(e).__name__}: {e}", time.time() - start


def _fmt(seconds: float) -> str:
    return f"{seconds * 1000:.0f}ms" if seconds < 1 else f"{seconds:.2f}s"


def main() -> int:
    parser = argparse.ArgumentParser(description="Run finstack example notebooks")
    parser.add_argument(
        "--notebook-root",
        type=Path,
        default=NOTEBOOKS_DIR,
        help="Notebook tree to execute and use for shared-fixture and notebook-dependency imports",
    )
    parser.add_argument(
        "--directory",
        help="Only run notebooks in this subdirectory, or a single .ipynb path",
    )
    parser.add_argument("--timeout", type=int, default=300, help="Per-notebook timeout in seconds")
    parser.add_argument("--verbose", action="store_true", help="Show detailed output")
    parser.add_argument(
        "--fail-fast",
        action="store_true",
        help="Stop after the first notebook failure",
    )
    parser.add_argument(
        "--save-outputs",
        action="store_true",
        help="Write each successfully executed notebook back with fresh outputs",
    )
    args = parser.parse_args()

    base_dir = args.notebook_root.resolve()
    if not base_dir.is_dir():
        parser.error(f"Notebook root is not a directory: {base_dir}")
    notebooks = find_notebooks(base_dir, args.directory)
    if any(not notebook.resolve().is_relative_to(base_dir) for notebook in notebooks):
        parser.error("--directory must remain inside --notebook-root")

    if not notebooks:
        print("No notebooks found!")
        return 1

    print(f"Found {len(notebooks)} notebooks to run:\n")
    for nb in notebooks:
        print(f"  - {nb.relative_to(base_dir)}")
    print()

    results: dict[Path, tuple[bool, str, float]] = {}
    successful: list[Path] = []
    failed: list[Path] = []
    t0 = time.time()

    for i, nb_path in enumerate(notebooks, 1):
        rel = nb_path.relative_to(base_dir)
        print(f"[{i}/{len(notebooks)}] Running {rel}...", end=" ", flush=True)

        ok, msg, elapsed = run_notebook(nb_path, args.timeout, args.save_outputs, base_dir)
        results[nb_path] = (ok, msg, elapsed)

        if ok:
            successful.append(nb_path)
            print(f"PASS ({_fmt(elapsed)})")
        else:
            failed.append(nb_path)
            print(f"FAIL ({_fmt(elapsed)})")
            if args.fail_fast:
                print("\nStopped early (--fail-fast).")
                break

    total = time.time() - t0
    executed = len(successful) + len(failed)
    print("\n" + "=" * 60)
    print(f"SUMMARY: {len(successful)}/{executed} passed in {_fmt(total)}")
    if executed < len(notebooks):
        print(f"({len(notebooks) - executed} notebook(s) not run)")
    print("=" * 60)

    if successful:
        print(f"\nPASS ({len(successful)}):")
        for nb_path in successful:
            _, _, elapsed = results[nb_path]
            print(f"  {nb_path.relative_to(base_dir)} ({_fmt(elapsed)})")

    if failed:
        print(f"\nFAIL ({len(failed)}):")
        for nb_path in failed:
            _, error, elapsed = results[nb_path]
            print(f"  {nb_path.relative_to(base_dir)} ({_fmt(elapsed)})")
            for line in error.split("\n"):
                print(f"    {line}")

    if args.verbose:
        print("\n" + "=" * 60)
        print("DETAILED OUTPUT")
        print("=" * 60)
        for nb_path, (ok, msg, elapsed) in results.items():
            status = "PASS" if ok else "FAIL"
            print(f"\n{status} {nb_path.relative_to(base_dir)} ({_fmt(elapsed)}):")
            print("-" * 40)
            print(msg)

    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
