"""Run cargo-semver-checks against the renamed v0.5.0 workspace layout."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys

BASELINE_TAG = "v0.5.0"
DEFAULT_MANIFEST = "finstack-quant/Cargo.toml"

PACKAGE_RENAMES = {
    "finstack": "finstack-quant",
    "finstack-analytics": "finstack-quant-analytics",
    "finstack-attribution": "finstack-quant-attribution",
    "finstack-cashflows": "finstack-quant-cashflows",
    "finstack-core": "finstack-quant-core",
    "finstack-covenants": "finstack-quant-covenants",
    "finstack-factor-model": "finstack-quant-factor-model",
    "finstack-margin": "finstack-quant-margin",
    "finstack-monte-carlo": "finstack-quant-monte-carlo",
    "finstack-portfolio": "finstack-quant-portfolio",
    "finstack-py": "finstack-quant-py",
    "finstack-scenarios": "finstack-quant-scenarios",
    "finstack-statements": "finstack-quant-statements",
    "finstack-statements-analytics": "finstack-quant-statements-analytics",
    "finstack-statements-fuzz": "finstack-quant-statements-fuzz",
    "finstack-test-utils": "finstack-quant-test-utils",
    "finstack-valuations": "finstack-quant-valuations",
    "finstack-valuations-macros": "finstack-quant-valuations-macros",
    "finstack-wasm": "finstack-quant-wasm",
}

TOP_LEVEL_RENAMES = (
    ("finstack-py", "finstack-quant-py"),
    ("finstack-wasm", "finstack-quant-wasm"),
    ("finstack", "finstack-quant"),
)

PACKAGE_NAME_RE = re.compile(r'^(name\s*=\s*")([^"]+)(".*)$')
INLINE_TABLE_DEP_RE = re.compile(r"^(\s*)([A-Za-z0-9_-]+)(\s*=\s*\{)(.*)$")


def run(command: list[str], *, cwd: Path, input_bytes: bytes | None = None) -> subprocess.CompletedProcess[bytes]:
    """Run a subprocess and raise on failure."""
    return subprocess.run(command, cwd=cwd, input=input_bytes, check=True)  # noqa: S603


def capture(command: list[str], *, cwd: Path) -> bytes:
    """Run a subprocess and capture stdout."""
    return subprocess.run(command, cwd=cwd, check=True, stdout=subprocess.PIPE).stdout  # noqa: S603


def normalized_baseline_root(repo_root: Path, tag: str, *, refresh: bool) -> Path:
    """Create a normalized baseline checkout under target/semver-checks."""
    baseline_dir_name = re.sub(r"[^A-Za-z0-9_.-]+", "_", tag)
    baseline_root = repo_root / "target" / "semver-checks" / "baselines" / f"{baseline_dir_name}-normalized"
    if refresh and baseline_root.exists():
        shutil.rmtree(baseline_root)
    if baseline_root.exists():
        return baseline_root

    baseline_root.mkdir(parents=True, exist_ok=True)
    archive = capture(["git", "archive", tag], cwd=repo_root)
    run(["tar", "-x", "-C", str(baseline_root)], cwd=repo_root, input_bytes=archive)

    for old, new in TOP_LEVEL_RENAMES:
        old_path = baseline_root / old
        if old_path.exists():
            old_path.rename(baseline_root / new)

    for manifest in baseline_root.rglob("Cargo.toml"):
        normalize_manifest(manifest, baseline_root)

    return baseline_root


def normalize_manifest(manifest: Path, baseline_root: Path) -> None:
    """Normalize package identities while preserving old dependency crate aliases."""
    text = manifest.read_text()

    if manifest == baseline_root / "Cargo.toml":
        text = normalize_workspace_member_paths(text)
    else:
        text = normalize_relative_paths(text)

    for old_name, new_name in sorted(PACKAGE_RENAMES.items(), key=lambda item: len(item[0]), reverse=True):
        text = text.replace(f'package = "{old_name}"', f'package = "{new_name}"')

    lines: list[str] = []
    section: str | None = None
    for line in text.splitlines():
        normalized_line = line
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped

        name_match = PACKAGE_NAME_RE.match(normalized_line)
        if section == "[package]" and name_match and name_match.group(2) in PACKAGE_RENAMES:
            normalized_line = f"{name_match.group(1)}{PACKAGE_RENAMES[name_match.group(2)]}{name_match.group(3)}"

        dep_match = INLINE_TABLE_DEP_RE.match(normalized_line)
        if dep_match and dep_match.group(2) in PACKAGE_RENAMES and "package =" not in dep_match.group(4):
            normalized_line = (
                f"{dep_match.group(1)}{dep_match.group(2)}{dep_match.group(3)} "
                f'package = "{PACKAGE_RENAMES[dep_match.group(2)]}",{dep_match.group(4)}'
            )

        lines.append(normalized_line)

    manifest.write_text("\n".join(lines) + "\n")


def normalize_workspace_member_paths(text: str) -> str:
    """Rewrite root workspace members to the current top-level directory names."""
    for old, new in sorted(PACKAGE_RENAMES.items(), key=lambda item: len(item[0]), reverse=True):
        text = text.replace(f'"{old}/', f'"{new}/')
        text = text.replace(f'"{old}"', f'"{new}"')
    return text


def normalize_relative_paths(text: str) -> str:
    """Rewrite relative paths that point at renamed top-level directories."""
    for old, new in TOP_LEVEL_RENAMES:
        text = text.replace(f"../{old}/", f"../{new}/")
    return text


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse wrapper-specific args and leave cargo-semver-checks args untouched."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--baseline-rev", "--baseline-tag", default=BASELINE_TAG, help="Git tag/rev to use as the baseline"
    )
    parser.add_argument("--refresh-baseline", action="store_true", help="Recreate the normalized baseline checkout")
    parser.add_argument(
        "--manifest-path",
        default=DEFAULT_MANIFEST,
        help=f"Current package manifest to check; defaults to {DEFAULT_MANIFEST}",
    )
    args, semver_args = parser.parse_known_args(argv)
    args.semver_args = semver_args
    return args


def main(argv: list[str]) -> int:
    """Normalize the baseline and delegate to cargo-semver-checks."""
    args = parse_args(argv)
    repo_root = Path(__file__).resolve().parents[1]
    baseline_root = normalized_baseline_root(repo_root, args.baseline_rev, refresh=args.refresh_baseline)
    baseline_manifest_root = baseline_root / Path(args.manifest_path).parent

    command = [
        "cargo-semver-checks",
        "semver-checks",
        "--manifest-path",
        args.manifest_path,
        "--baseline-root",
        str(baseline_manifest_root),
        *args.semver_args,
    ]
    completed = subprocess.run(command, cwd=repo_root, check=False)  # noqa: S603
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
