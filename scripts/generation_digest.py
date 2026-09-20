"""Compute a stable digest for checked-in generated artifacts."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

DEFAULT_PATHS = (
    Path("finstack-quant/core/src/generated"),
    Path("finstack-quant/core/schemas"),
    Path("finstack-quant/attribution/schemas"),
    Path("finstack-quant/cashflows/schemas"),
    Path("finstack-quant/models/schemas"),
    Path("finstack-quant/margin/schemas"),
    Path("finstack-quant/scenarios/schemas"),
    Path("finstack-quant/statements/schemas"),
    Path("finstack-quant/valuations/schemas"),
    Path("finstack-quant/valuations/tests/instruments/json_examples"),
    Path("finstack-quant/portfolio/schemas"),
    Path("finstack-quant-wasm/types/generated"),
    Path("finstack-quant-ui/src/generated"),
    Path("finstack-quant-ui/src/contract-provenance.json"),
)
DEFAULT_MANIFEST_PATHS = (
    Path("finstack-quant/valuations/tests/instruments/json_examples"),
    Path("finstack-quant-wasm/types/generated"),
    Path("finstack-quant-ui/src/generated"),
    Path("finstack-quant-ui/src/contract-provenance.json"),
)
DEFAULT_MANIFEST = Path("scripts/generated-artifacts.txt")


def generated_files(root: Path, paths: list[Path]) -> list[Path]:
    """Return generated files in stable repository-relative order."""
    files: list[Path] = []
    for relative_path in paths:
        path = root / relative_path
        if path.is_file():
            files.append(path)
        elif path.is_dir():
            files.extend(candidate for candidate in path.rglob("*") if candidate.is_file())
    return sorted(files, key=lambda path: path.relative_to(root).as_posix())


def generated_relative_paths(root: Path, paths: list[Path]) -> list[str]:
    """Return the complete generated-file inventory as repository-relative paths."""
    return [path.relative_to(root).as_posix() for path in generated_files(root, paths)]


def write_manifest(root: Path, paths: list[Path], manifest_path: Path) -> None:
    """Explicitly replace the expected generated-artifact path inventory."""
    inventory = generated_relative_paths(root, paths)
    destination = manifest_path if manifest_path.is_absolute() else root / manifest_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("".join(f"{path}\n" for path in inventory), encoding="utf-8")


def check_manifest(root: Path, paths: list[Path], manifest_path: Path) -> None:
    """Reject missing and obsolete generated files without mutating the manifest."""
    source = manifest_path if manifest_path.is_absolute() else root / manifest_path
    expected = {
        line.strip()
        for line in source.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }
    actual = set(generated_relative_paths(root, paths))
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        raise ValueError(f"generated artifact manifest mismatch: missing={missing or 'none'} extra={extra or 'none'}")


def generation_digest(root: Path, paths: list[Path]) -> tuple[str, int]:
    """Hash generated paths and contents with unambiguous framing."""
    digest = hashlib.sha256()
    files = generated_files(root, paths)
    for path in files:
        relative = path.relative_to(root).as_posix().encode()
        contents = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(contents).to_bytes(8, "big"))
        digest.update(contents)
    return digest.hexdigest(), len(files)


def main() -> int:
    """Print the aggregate digest for generated artifacts."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check-manifest", action="store_true")
    mode.add_argument("--write-manifest", action="store_true")
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()

    root = args.root.resolve()
    paths = args.paths or list(DEFAULT_PATHS)
    manifest_paths = args.paths or list(DEFAULT_MANIFEST_PATHS)
    if args.write_manifest:
        write_manifest(root, manifest_paths, args.manifest)
    elif args.check_manifest:
        try:
            check_manifest(root, manifest_paths, args.manifest)
        except (OSError, ValueError) as error:
            parser.error(str(error))
    digest, file_count = generation_digest(root, paths)
    if args.verbose:
        print(f"sha256:{digest} ({file_count} files)")
    else:
        print(digest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
