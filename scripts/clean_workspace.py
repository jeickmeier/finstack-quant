#!/usr/bin/env python3
"""Remove build artifacts, virtualenvs, and generated caches across the workspace.

Invoked by `mise run all-clean` after `cargo clean`.
"""

from __future__ import annotations

from pathlib import Path
import shutil

ROOT_DIRS = (
    ".venv",
    "finstack-quant-wasm/pkg",
    "finstack-quant-wasm/pkg-node",
    "book/book",
)

GLOB_DIRS = ("__pycache__", "*.egg-info")


def main() -> None:
    """Delete configured directories and glob-matched caches under the current working directory."""
    root = Path.cwd()
    for rel in ROOT_DIRS:
        shutil.rmtree(root / rel, ignore_errors=True)
    for pattern in GLOB_DIRS:
        for path in root.rglob(pattern):
            shutil.rmtree(path, ignore_errors=True)
    print("Workspace cleaned.")


if __name__ == "__main__":
    main()
