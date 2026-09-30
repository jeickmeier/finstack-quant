"""Write the TypeScript barrels for the schema-generated contract types.

`finstack-quant-wasm/scripts/generate-contract-types.mjs` emits one module per
crate, `types/generated/<crate>/<crate>.ts`. This script writes
`<crate>/index.ts` for each crate and a root `index.ts` that re-exports every
crate as a namespace (`export * as calibration from ...`), because the same
Rust type name (`Money`, `Tenor`, ...) can appear in several crates. Import
specifiers carry `.js` extensions so the barrels resolve under NodeNext.
"""

from __future__ import annotations

import argparse
from pathlib import Path

DEFAULT_DIRECTORY = Path("finstack-quant-wasm/types/generated")


def expected_barrels(directory: Path) -> dict[Path, str]:
    """Return every barrel path with its deterministic contents."""
    crates = sorted(path.parent.name for path in directory.glob("*/*.ts") if path.stem == path.parent.name)
    barrels = {directory / crate / "index.ts": f'export * from "./{crate}.js";\n' for crate in crates}
    barrels[directory / "index.ts"] = "".join(f'export * as {crate} from "./{crate}/index.js";\n' for crate in crates)
    return barrels


def main() -> int:
    """Write or check the generated TypeScript barrels."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    barrels = expected_barrels(args.directory)
    if args.check:
        return 0 if all(path.is_file() and path.read_text() == text for path, text in barrels.items()) else 1
    for path, text in barrels.items():
        path.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
