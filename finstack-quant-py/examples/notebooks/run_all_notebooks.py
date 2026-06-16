#!/usr/bin/env python3
"""Execute all example notebooks and report pass/fail status.

Uses nbclient to run each notebook programmatically.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
import time

from nbclient import NotebookClient
from nbclient.exceptions import CellExecutionError
import nbformat


def find_notebooks(base_dir: Path, subdirectory: str | None = None) -> list[Path]:
    """Find all Jupyter notebooks under *base_dir*, optionally filtered to *subdirectory*."""
    search_root = base_dir / subdirectory if subdirectory else base_dir
    if not search_root.exists():
        return []
    notebooks = sorted(search_root.glob("**/*.ipynb"))
    return [nb for nb in notebooks if ".ipynb_checkpoints" not in str(nb)]


def run_notebook(
    notebook_path: Path, timeout: int
) -> tuple[bool, str, float]:
    """Run a single notebook; return (success, message, elapsed_seconds)."""
    start = time.time()
    try:
        with open(notebook_path, encoding="utf-8") as f:
            nb = nbformat.read(f, as_version=4)

        client = NotebookClient(
            nb,
            timeout=timeout,
            kernel_name="python3",
            resources={"metadata": {"path": str(notebook_path.parent)}},
        )
        client.execute()

        elapsed = time.time() - start
        cell_count = sum(1 for c in nb.cells if c.cell_type == "code")
        return True, f"Executed {cell_count} code cells", elapsed

    except CellExecutionError as e:
        elapsed = time.time() - start
        lines = str(e).split("\n")
        for i, line in enumerate(lines):
            if "Error" in line or "Exception" in line:
                start_idx = max(0, i - 2)
                end_idx = min(len(lines), i + 5)
                return False, "\n".join(lines[start_idx:end_idx]), elapsed
        return False, "\n".join(lines[-5:]), elapsed

    except TimeoutError:
        return False, f"Timed out (>{timeout}s)", time.time() - start

    except Exception as e:
        return False, f"{type(e).__name__}: {e}", time.time() - start


def _fmt(seconds: float) -> str:
    return f"{seconds * 1000:.0f}ms" if seconds < 1 else f"{seconds:.2f}s"


def main() -> int:
    parser = argparse.ArgumentParser(description="Run finstack example notebooks")
    parser.add_argument("--directory", help="Only run notebooks in this subdirectory")
    parser.add_argument("--timeout", type=int, default=300, help="Per-notebook timeout in seconds")
    parser.add_argument("--verbose", action="store_true", help="Show detailed output")
    parser.add_argument(
        "--fail-fast",
        action="store_true",
        help="Stop after the first notebook failure",
    )
    args = parser.parse_args()

    # One-time PYTHONPATH for nbclient/kernel subprocesses (import finstack_quant from workspace).
    repo_root = Path(__file__).resolve().parents[3]
    finstack_py_root = repo_root / "finstack-quant-py"
    extra_paths = [str(finstack_py_root), str(repo_root)]
    existing = os.environ.get("PYTHONPATH", "")
    pieces = [p for p in existing.split(os.pathsep) if p]
    for p in extra_paths:
        if p not in pieces:
            pieces.insert(0, p)
    os.environ["PYTHONPATH"] = os.pathsep.join(pieces)

    base_dir = Path(__file__).parent
    notebooks = find_notebooks(base_dir, args.directory)

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

        ok, msg, elapsed = run_notebook(nb_path, args.timeout)
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
        for nb_path in results:
            ok, msg, elapsed = results[nb_path]
            status = "PASS" if ok else "FAIL"
            print(f"\n{status} {nb_path.relative_to(base_dir)} ({_fmt(elapsed)}):")
            print("-" * 40)
            print(msg)

    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())
