"""Verify Rust-owned TypeScript declarations in an isolated directory."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_DIR = ROOT / "finstack-quant-wasm/types/generated"


def files_by_relative_path(directory: Path) -> dict[str, bytes]:
    """Read every generated file keyed by its portable relative path."""
    return {
        path.relative_to(directory).as_posix(): path.read_bytes()
        for path in sorted(directory.rglob("*"))
        if path.is_file()
    }


def git_worktree_relative_paths(directory: Path, *, root: Path = ROOT) -> list[str]:
    """List present tracked and untracked files, preserving git-recorded case."""
    git = shutil.which("git")
    if git is None:
        raise RuntimeError("git is required to compare tracked TypeScript declaration names")
    rel = directory.relative_to(root).as_posix()
    result = subprocess.run(  # noqa: S603
        [git, "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", rel],
        cwd=root,
        check=True,
        capture_output=True,
    )
    prefix = f"{rel}/"
    return sorted({
        text[len(prefix) :]
        for raw in result.stdout.split(b"\0")
        if (text := raw.decode()) and text.startswith(prefix) and (root / text).is_file()
    })


def files_by_git_path(directory: Path, *, root: Path = ROOT) -> dict[str, bytes]:
    """Read worktree declarations using git-recorded names for tracked files."""
    return {name: (directory / name).read_bytes() for name in git_worktree_relative_paths(directory, root=root)}


def inventory_drift(expected: dict[str, bytes], actual: dict[str, bytes]) -> tuple[list[str], list[str], list[str]]:
    """Return missing, extra, and content-changed relative paths."""
    missing = sorted(expected.keys() - actual.keys())
    extra = sorted(actual.keys() - expected.keys())
    changed = sorted(path for path in expected.keys() & actual.keys() if expected[path] != actual[path])
    return missing, extra, changed


def run_export(command: list[str], output_dir: Path) -> None:
    """Run one exporter with all output redirected to ``output_dir``."""
    environment = os.environ.copy()
    environment["FINSTACK_TS_OUTPUT_DIR"] = str(output_dir)
    subprocess.run(command, cwd=ROOT, env=environment, check=True)  # noqa: S603


def main() -> int:
    """Generate declarations in a temporary tree and compare exact bytes."""
    with tempfile.TemporaryDirectory(prefix="finstack-ts-") as temporary:
        output_dir = Path(temporary)
        run_export(
            [
                "cargo",
                "test",
                "-p",
                "finstack-quant-calibration",
                "--test",
                "ts_export",
                "--features",
                "ts_export",
                "--",
                "--test-threads=1",
            ],
            output_dir,
        )
        run_export(
            [
                "cargo",
                "test",
                "-p",
                "finstack-quant-portfolio",
                "--test",
                "ts_export",
                "--features",
                "ts_export",
            ],
            output_dir,
        )
        subprocess.run(  # noqa: S603
            [
                sys.executable,
                str(ROOT / "scripts/sync_generated_ts_index.py"),
                "--directory",
                str(output_dir),
            ],
            cwd=ROOT,
            check=True,
        )

        expected = files_by_git_path(EXPECTED_DIR)
        actual = files_by_relative_path(output_dir)
        missing, extra, changed = inventory_drift(expected, actual)
        if missing or extra or changed:
            print(
                "generated TypeScript declarations are stale: "
                f"missing={missing or 'none'} extra={extra or 'none'} "
                f"changed={changed or 'none'}",
                file=sys.stderr,
            )
            return 1

    print(f"Generated TypeScript declarations are current ({len(expected)} files).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
