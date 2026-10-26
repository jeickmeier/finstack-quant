"""Stamp a version into the workspace manifests.

CI runs this in a throwaway checkout before building nightly and release
artifacts, so build versions are never committed; maintainers run it to bump
``master`` to the next minor version after cutting a release branch. It
rewrites the workspace
``Cargo.toml`` version, every intra-workspace ``path`` dependency requirement
pinned to the old version (Cargo rejects a pre-release such as
``0.12.0-dev.1`` against a plain ``0.12.0`` requirement), and the version in
``finstack-quant-wasm/package.json`` and its lockfile. The Python wheel version
follows the Cargo version; maturin normalizes ``-rc.1`` to ``rc1`` and
``-dev.N`` to ``.devN``.

Usage: ``python3 scripts/stamp_version.py 0.12.0-dev.202610210600``
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import tomllib

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]

# SemVer 2.0.0 without build metadata, which PEP 440 and npm handle differently.
_SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$")
_SKIPPED_DIRS = frozenset({".git", ".venv", "node_modules", "target"})
_NPM_MANIFESTS = ("finstack-quant-wasm/package.json", "finstack-quant-wasm/package-lock.json")


def workspace_version(root: Path) -> str:
    """Return the ``[workspace.package]`` version of the workspace at ``root``."""
    with (root / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["workspace"]["package"]["version"]


def _cargo_manifests(root: Path) -> list[Path]:
    resolved_root = root.resolve()
    return sorted(
        path
        for path in root.rglob("Cargo.toml")
        if not _SKIPPED_DIRS.intersection(path.relative_to(root).parts) and path.resolve().is_relative_to(resolved_root)
    )


def _stamp_cargo(root: Path, old: str, new: str) -> None:
    workspace_line = re.compile(rf'^version = "{re.escape(old)}"$', re.MULTILINE)
    requirement = f'version = "{old}"'
    for manifest in _cargo_manifests(root):
        text = manifest.read_text(encoding="utf-8")
        if manifest == root / "Cargo.toml":
            text, count = workspace_line.subn(f'version = "{new}"', text)
            if count != 1:
                raise SystemExit(f"expected one workspace version line in {manifest}, found {count}")
        lines = [
            line.replace(requirement, f'version = "{new}"') if 'path = "' in line else line
            for line in text.splitlines(keepends=True)
        ]
        manifest.write_text("".join(lines), encoding="utf-8")


def _stamp_npm(root: Path, new: str) -> None:
    for relative in _NPM_MANIFESTS:
        path = root / relative
        if not path.exists():
            continue
        data = json.loads(path.read_text(encoding="utf-8"))
        data["version"] = new
        if "" in data.get("packages", {}):
            data["packages"][""]["version"] = new
        path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def stamp_version(root: Path, version: str) -> bool:
    """Write ``version`` into the manifests under ``root``.

    Returns ``False`` when the workspace already carries ``version`` and nothing
    was written. Raises ``SystemExit`` for a version that is not SemVer.
    """
    if not _SEMVER.match(version):
        raise SystemExit(f"not a SemVer version (no build metadata): {version!r}")
    old = workspace_version(root)
    if old == version:
        return False
    _stamp_cargo(root, old, version)
    _stamp_npm(root, version)
    return True


def main() -> None:
    """Stamp the version given on the command line into this repository."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("version", help="SemVer version, e.g. 0.12.0-rc.1 or 0.12.0-dev.202610210600")
    args = parser.parse_args()
    old = workspace_version(REPOSITORY_ROOT)
    if stamp_version(REPOSITORY_ROOT, args.version):
        print(f"Stamped {old} -> {args.version}")
    else:
        print(f"Version already {args.version}; nothing to stamp")


if __name__ == "__main__":
    main()
