"""Run the schema generators: verify the checked-in tree, or regenerate it.

Without ``--write`` this checks that the registries own exactly the generated
files on disk, that the schema ``$ref`` graph resolves, and that two clean-room
generations are byte-identical to each other and to the checked-in files. With
``--write`` it regenerates every registry in place.

The generators are executed from ``--bin-dir`` (default: ``target/debug`` of the
cargo target directory) instead of through ``cargo run``, which re-resolves
features and rebuilds per package. ``mise run rust-check-schemas`` and
``mise run rust-gen-schemas`` build them first.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urldefrag, urljoin

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Generator:
    """One registry-driven generator binary and its crate-relative output root."""

    crate: Path
    binary: str


# Written by `run_schema_generator` once per crate, alongside the schemas.
SCHEMA_INDEX_RELATIVE_PATH = Path("schemas/index.json")

GENERATORS = (
    Generator(Path("finstack-quant/core"), "gen_core_schemas"),
    Generator(Path("finstack-quant/analytics"), "gen_analytics_schemas"),
    Generator(Path("finstack-quant/statements-analytics"), "gen_statements_analytics_schemas"),
    Generator(Path("finstack-quant/covenants"), "gen_covenants_schemas"),
    Generator(Path("finstack-quant/features"), "gen_features_schemas"),
    Generator(Path("finstack-quant/attribution"), "gen_attribution_schemas"),
    Generator(Path("finstack-quant/calibration"), "gen_calibration_schemas"),
    Generator(Path("finstack-quant/cashflows"), "gen_cashflow_schemas"),
    Generator(Path("finstack-quant/models"), "gen_models_schemas"),
    Generator(Path("finstack-quant/margin"), "gen_margin_schema"),
    Generator(Path("finstack-quant/scenarios"), "gen_scenario_schemas"),
    Generator(Path("finstack-quant/statements"), "gen_statement_schemas"),
    Generator(Path("finstack-quant/valuations"), "gen_schemas"),
    Generator(Path("finstack-quant/portfolio"), "gen_portfolio_schemas"),
)

# Globs for generator-owned files. Every match must be a listed artifact: this
# is how an orphaned schema or instrument fixture is caught. It mirrors the
# generators' own `--check`, which reports `*.schema.json` extras below each
# schema root and `*.json` extras below the valuations fixture root.
OWNED_FILES = (
    "finstack-quant/*/schemas/**/*.schema.json",
    "finstack-quant/valuations/tests/instruments/json_examples/**/*.json",
)


def default_bin_dir() -> Path:
    """Return ``debug/`` below the cargo target directory.

    Ask cargo: ``.cargo/config.toml`` can set ``build.target-dir`` (worktrees
    here share one), which a ``CARGO_TARGET_DIR`` fallback does not see.
    """
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],  # noqa: S607
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(json.loads(metadata.stdout)["target_directory"]) / "debug"


def command(generator: Generator, bin_dir: Path, *arguments: str) -> list[str]:
    """Build one generator command without invoking a shell."""
    binary = bin_dir / generator.binary
    if not binary.is_file():
        raise FileNotFoundError(
            f"schema generator {binary} is not built; run `mise run rust-check-schemas` or `mise run rust-gen-schemas`"
        )
    return [str(binary), *arguments]


def run(generator: Generator, bin_dir: Path, *arguments: str) -> str:
    """Run one generator and return its standard output; its diagnostics go to stderr."""
    return subprocess.run(  # noqa: S603
        command(generator, bin_dir, *arguments),
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout


def run_all(bin_dir: Path, *arguments: str) -> list[str]:
    """Run every generator concurrently and return their outputs in table order.

    Generators write disjoint crate trees, so running them in parallel cannot
    change what any one of them produces.
    """
    with ThreadPoolExecutor() as pool:
        return list(pool.map(lambda generator: run(generator, bin_dir, *arguments), GENERATORS))


def parse_registry(generator: Generator, listing: str) -> tuple[Path, ...]:
    """Validate one generator's deterministic ``--list`` path inventory."""
    paths = tuple(Path(line) for line in listing.splitlines() if line)
    if not paths:
        raise ValueError(f"{generator.binary} returned an empty registry")
    if len(paths) != len(set(paths)):
        raise ValueError(f"{generator.binary} returned duplicate registry paths")
    invalid = [path for path in paths if path.is_absolute() or ".." in path.parts]
    if invalid:
        raise ValueError(f"{generator.binary} returned unsafe paths: {invalid}")
    if tuple(sorted(paths, key=lambda path: path.as_posix())) != paths:
        raise ValueError(f"{generator.binary} registry paths are not sorted")
    return paths


def generate_tree(destination: Path, bin_dir: Path) -> None:
    """Generate every registry into an isolated workspace-shaped tree."""

    def generate(generator: Generator) -> None:
        output_root = destination / generator.crate
        output_root.mkdir(parents=True, exist_ok=True)
        run(generator, bin_dir, "--write", "--output-root", str(output_root))

    with ThreadPoolExecutor() as pool:
        list(pool.map(generate, GENERATORS))


def ownership_mismatch(listed: set[Path], root: Path) -> str | None:
    """Describe listed artifacts missing below ``root`` and owned files nobody lists."""
    owned = {path.relative_to(root) for pattern in OWNED_FILES for path in root.glob(pattern) if path.is_file()}
    missing = sorted(path for path in listed if not (root / path).is_file())
    extra = sorted(owned - listed)
    if not missing and not extra:
        return None
    return f"generated artifact registry mismatch: missing={missing or 'none'} extra={extra or 'none'}"


def files_by_relative_path(directory: Path) -> dict[Path, bytes]:
    """Read every file below ``directory`` in deterministic path order."""
    return {path.relative_to(directory): path.read_bytes() for path in sorted(directory.rglob("*")) if path.is_file()}


def pointer_target(document: object, fragment: str) -> object:
    """Resolve one RFC 6901 JSON pointer fragment."""
    current = document
    if not fragment:
        return current
    if not fragment.startswith("/"):
        raise ValueError(f"unsupported non-pointer JSON Schema fragment #{fragment}")
    for encoded in fragment[1:].split("/"):
        token = encoded.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and token in current:
            current = current[token]
        elif isinstance(current, list) and token.isdigit() and int(token) < len(current):
            current = current[int(token)]
        else:
            raise ValueError(f"unresolved JSON Schema pointer #{fragment}")
    return current


def references(value: object) -> list[str]:
    """Collect every JSON Schema reference recursively."""
    found: list[str] = []
    if isinstance(value, dict):
        reference = value.get("$ref")
        if isinstance(reference, str):
            found.append(reference)
        for child in value.values():
            found.extend(references(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(references(child))
    return found


def validate_schema_graph(schema_paths: set[Path]) -> None:
    """Require unique IDs, v1 paths, and fully resolvable references."""
    documents: dict[Path, object] = {}
    ids: dict[str, tuple[Path, object]] = {}
    for path in sorted(schema_paths):
        schema_index = path.parts.index("schemas")
        if len(path.parts) <= schema_index + 2 or path.parts[schema_index + 2] != "1":
            raise ValueError(f"schema path is not v1: {path}")
        document = json.loads((ROOT / path).read_text(encoding="utf-8"))
        schema_id = document.get("$id") if isinstance(document, dict) else None
        if not isinstance(schema_id, str) or not schema_id:
            raise ValueError(f"schema has no root $id: {path}")
        if schema_id in ids:
            raise ValueError(f"duplicate schema $id {schema_id}: {ids[schema_id][0]} and {path}")
        documents[path] = document
        ids[schema_id] = (path, document)

    for path, document in documents.items():
        schema_id = document["$id"]
        for reference in references(document):
            absolute = urljoin(schema_id, reference)
            target_id, fragment = urldefrag(absolute)
            if target_id not in ids:
                raise ValueError(f"unresolved schema reference {reference!r} in {path}")
            try:
                pointer_target(ids[target_id][1], fragment)
            except ValueError as error:
                raise ValueError(f"{error} referenced by {path}") from error


def digest(files: dict[Path, bytes]) -> str:
    """Hash paths and bytes with length-delimited framing."""
    result = hashlib.sha256()
    for path, contents in sorted(files.items()):
        encoded_path = path.as_posix().encode()
        result.update(len(encoded_path).to_bytes(8, "big"))
        result.update(encoded_path)
        result.update(len(contents).to_bytes(8, "big"))
        result.update(contents)
    return result.hexdigest()


def check(bin_dir: Path) -> int:
    """Check registry ownership and two independent generated trees."""
    listed: set[Path] = set()
    for generator, listing in zip(GENERATORS, run_all(bin_dir, "--list"), strict=True):
        for relative_path in parse_registry(generator, listing):
            workspace_path = generator.crate / relative_path
            if workspace_path in listed:
                raise ValueError(f"artifact has multiple registry entries: {workspace_path}")
            listed.add(workspace_path)
        # Every generator also emits one schema index per crate. It is excluded
        # from `--list`, whose output must stay sorted and cannot place a single
        # per-crate file correctly against every crate's root directory name.
        listed.add(generator.crate / SCHEMA_INDEX_RELATIVE_PATH)

    mismatch = ownership_mismatch(listed, ROOT)
    if mismatch is not None:
        raise ValueError(mismatch)

    schemas = {path for path in listed if path.name.endswith(".schema.json")}
    validate_schema_graph(schemas)

    with (
        tempfile.TemporaryDirectory(prefix="finstack-schema-a-") as first_dir,
        tempfile.TemporaryDirectory(prefix="finstack-schema-b-") as second_dir,
    ):
        first_root = Path(first_dir)
        second_root = Path(second_dir)
        with ThreadPoolExecutor(max_workers=2) as pool:
            list(pool.map(lambda root: generate_tree(root, bin_dir), (first_root, second_root)))
        first = files_by_relative_path(first_root)
        second = files_by_relative_path(second_root)
        if first != second:
            raise ValueError("independent schema generation trees differ")
        if set(first) != listed:
            missing = sorted(listed - set(first))
            extra = sorted(set(first) - listed)
            raise ValueError(f"generated registry mismatch: missing={missing or 'none'} extra={extra or 'none'}")
        workspace = {path: (ROOT / path).read_bytes() for path in listed}
        if first != workspace:
            changed = sorted(path for path in listed if first.get(path) != workspace.get(path))
            raise ValueError(f"checked-in generated artifacts differ: {changed}")

    print(f"Schema registries are complete and byte-reproducible: {len(schemas)} schemas, sha256:{digest(first)}")
    return 0


def main(arguments: list[str] | None = None) -> int:
    """Verify the generated tree, or regenerate it with ``--write``."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin-dir", type=Path, help="directory holding the built gen_* binaries")
    parser.add_argument("--write", action="store_true", help="regenerate every registry in the workspace")
    options = parser.parse_args(arguments)
    bin_dir = options.bin_dir or default_bin_dir()
    if options.write:
        run_all(bin_dir, "--write")
        return 0
    return check(bin_dir)


if __name__ == "__main__":
    raise SystemExit(main())
