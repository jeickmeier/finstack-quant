"""Run all Python example scripts.

This mirrors the notebook runner at a smaller scale: discover standalone
``.py`` examples, skip checkpoints and this runner, execute each script, and
report a concise pass/fail summary.
"""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

PYTHON_RUNNER = (sys.executable,)


def find_scripts(root: Path) -> list[Path]:
    """Return example scripts under ``root`` in deterministic order."""
    scripts: list[Path] = []
    for path in root.rglob("*.py"):
        if path.name == "run_all_scripts.py":
            continue
        if ".ipynb_checkpoints" in path.parts:
            continue
        scripts.append(path)
    return sorted(scripts)


def run_script(script: Path, timeout: int = 120) -> tuple[bool, str, float]:
    """Run one script and return ``(ok, output, elapsed_seconds)``."""
    start = time.perf_counter()
    proc = subprocess.run(
        (*PYTHON_RUNNER, str(script)),
        cwd=script.parent,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    elapsed = time.perf_counter() - start
    output = proc.stdout if proc.returncode == 0 else proc.stderr or proc.stdout
    return proc.returncode == 0, output, elapsed


def main() -> int:
    """Run discovered scripts and return a process exit code."""
    root = Path(__file__).resolve().parent
    scripts = find_scripts(root)
    failures = 0
    for index, script in enumerate(scripts, start=1):
        ok, message, elapsed = run_script(script)
        status = "PASS" if ok else "FAIL"
        print(f"[{index}/{len(scripts)}] {script.relative_to(root)}... {status} ({elapsed:.2f}s)")
        if not ok:
            failures += 1
            print(message)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
