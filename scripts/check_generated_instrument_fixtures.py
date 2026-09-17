"""Validate canonical generated instrument fixtures against the registry manifest."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import tomllib

MANIFEST = Path("finstack-quant/valuations/tests/instruments/coverage_manifest.toml")
INSTRUMENTS_DIR = MANIFEST.parent
EXPECTED_CANONICAL_FIXTURES = 78


def canonical_fixture_entries(root: Path) -> list[tuple[str, Path]]:
    """Return manifest tags and canonical generated fixture paths."""
    manifest_path = root / MANIFEST
    with manifest_path.open("rb") as manifest_file:
        manifest = tomllib.load(manifest_file)

    entries = [(instrument["tag"], INSTRUMENTS_DIR / instrument["fixture"]) for instrument in manifest["instrument"]]
    paths = [path for _, path in entries]
    unique_paths = set(paths)
    if len(paths) != EXPECTED_CANONICAL_FIXTURES or len(unique_paths) != len(paths):
        raise ValueError(
            f"coverage manifest must declare exactly {EXPECTED_CANONICAL_FIXTURES} unique canonical fixtures"
        )
    return sorted(entries, key=lambda entry: entry[1].as_posix())


def generated_fixtures_are_valid(root: Path) -> bool:
    """Return whether the fixture inventory exactly matches the registry."""
    entries = canonical_fixture_entries(root)
    expected_paths = {relative_path for _, relative_path in entries}
    fixture_root = root / INSTRUMENTS_DIR / "json_examples"
    actual_paths = {path.relative_to(root) for path in fixture_root.glob("*.json") if path.is_file()}
    if actual_paths != expected_paths:
        return False

    for tag, relative_path in entries:
        path = root / relative_path
        if not path.is_file():
            return False
        try:
            document = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            return False
        if document.get("schema") != "finstack_quant.instrument/1":
            return False
        instrument = document.get("instrument")
        if not isinstance(instrument, dict) or instrument.get("type") != tag:
            return False
    return True


def main() -> int:
    """Check canonical generated fixtures for the selected repository root."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    return 0 if generated_fixtures_are_valid(args.root.resolve()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
