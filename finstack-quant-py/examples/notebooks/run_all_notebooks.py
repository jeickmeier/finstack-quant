#!/usr/bin/env python3
"""Execute all example notebooks and report pass/fail status.

Uses nbclient to run each notebook programmatically.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib
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


def _configure_pythonpath() -> None:
    """Expose the Python package, repository, and shared notebook helpers."""
    repo_root = NOTEBOOKS_DIR.parents[2]
    extra_paths = [
        str(NOTEBOOKS_DIR),
        str(repo_root / "finstack-quant-py"),
        str(repo_root),
    ]
    existing = os.environ.get("PYTHONPATH", "")
    pieces = [path for path in existing.split(os.pathsep) if path]
    for path in reversed(extra_paths):
        if path not in pieces:
            pieces.insert(0, path)
    os.environ["PYTHONPATH"] = os.pathsep.join(pieces)


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


def run_notebook(notebook_path: Path, timeout: int, save_outputs: bool = False) -> tuple[bool, str, float]:
    """Run a single notebook; return (success, message, elapsed_seconds).

    When *save_outputs* is set, a successfully executed notebook is written back
    with its freshly computed outputs. Notebooks that fail are never written, so
    a broken run cannot overwrite a good file.
    """
    _configure_pythonpath()
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
            client = NotebookClient(
                nb,
                timeout=timeout,
                kernel_name="python3",
                resources={"metadata": {"path": str(notebook_path.parent)}},
                config=config,
                force_raise_errors=True,
            )
            # Financial acceptance assertions must run even under an optimized caller.
            kernel_env = os.environ.copy()
            kernel_env["PYTHONOPTIMIZE"] = "0"
            ipython_dir = Path(ipc_dir) / "ipython"
            ipython_dir.mkdir()
            kernel_env["IPYTHONDIR"] = str(ipython_dir)
            kernel_env["FINSTACK_EXPECTED_PYTHON"] = str(Path(sys.executable).resolve())
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
            nb.cells.insert(0, guard)
            try:
                client.execute(env=kernel_env)
            finally:
                del nb.cells[0]
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
        help="Notebook tree to execute; package and shared-fixture imports still use the source checkout",
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

    _configure_pythonpath()

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

        ok, msg, elapsed = run_notebook(nb_path, args.timeout, args.save_outputs)
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
