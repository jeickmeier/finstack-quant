"""Verify schema registry completeness and clean-room reproducibility."""

from __future__ import annotations

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
    """One registry-driven generator and its crate-relative output root."""

    crate: Path
    package: str
    binary: str


# Written by `run_schema_index_generator` once per crate, alongside the schemas.
SCHEMA_INDEX_RELATIVE_PATH = Path("schemas/index.json")

GENERATORS = (
    Generator(Path("finstack-quant/core"), "finstack-quant-core", "gen_core_schemas"),
    Generator(Path("finstack-quant/analytics"), "finstack-quant-analytics", "gen_analytics_schemas"),
    Generator(
        Path("finstack-quant/statements-analytics"),
        "finstack-quant-statements-analytics",
        "gen_statements_analytics_schemas",
    ),
    Generator(Path("finstack-quant/covenants"), "finstack-quant-covenants", "gen_covenants_schemas"),
    Generator(Path("finstack-quant/features"), "finstack-quant-features", "gen_features_schemas"),
    Generator(Path("finstack-quant/attribution"), "finstack-quant-attribution", "gen_attribution_schemas"),
    Generator(
        Path("finstack-quant/calibration"),
        "finstack-quant-calibration",
        "gen_calibration_schemas",
    ),
    Generator(Path("finstack-quant/cashflows"), "finstack-quant-cashflows", "gen_cashflow_schemas"),
    Generator(Path("finstack-quant/models"), "finstack-quant-models", "gen_factor_model_schemas"),
    Generator(Path("finstack-quant/margin"), "finstack-quant-margin", "gen_margin_schema"),
    Generator(Path("finstack-quant/scenarios"), "finstack-quant-scenarios", "gen_scenario_schemas"),
    Generator(Path("finstack-quant/statements"), "finstack-quant-statements", "gen_statement_schemas"),
    Generator(Path("finstack-quant/valuations"), "finstack-quant-valuations", "gen_schemas"),
    Generator(Path("finstack-quant/portfolio"), "finstack-quant-portfolio", "gen_materialization_schemas"),
)


def command(generator: Generator, *arguments: str) -> list[str]:
    """Build one generator command without invoking a shell."""
    return [
        "cargo",
        "run",
        "--quiet",
        "-p",
        generator.package,
        "--bin",
        generator.binary,
        "--",
        *arguments,
    ]


def registry_paths(generator: Generator) -> tuple[Path, ...]:
    """Read and validate the generator's deterministic path inventory."""
    result = subprocess.run(  # noqa: S603
        command(generator, "--list"),
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    paths = tuple(Path(line) for line in result.stdout.splitlines() if line)
    if not paths:
        raise ValueError(f"{generator.package} returned an empty registry")
    if len(paths) != len(set(paths)):
        raise ValueError(f"{generator.package} returned duplicate registry paths")
    invalid = [path for path in paths if path.is_absolute() or ".." in path.parts]
    if invalid:
        raise ValueError(f"{generator.package} returned unsafe paths: {invalid}")
    if tuple(sorted(paths, key=lambda path: path.as_posix())) != paths:
        raise ValueError(f"{generator.package} registry paths are not sorted")
    return paths


def generate_tree(destination: Path) -> None:
    """Generate every registry into an isolated workspace-shaped tree."""
    for generator in GENERATORS:
        output_root = destination / generator.crate
        output_root.mkdir(parents=True, exist_ok=True)
        subprocess.run(  # noqa: S603
            command(generator, "--write", "--output-root", str(output_root)),
            cwd=ROOT,
            check=True,
            stdout=subprocess.DEVNULL,
        )


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


def main() -> int:
    """Check registry ownership and two independent generated trees."""
    listed: set[Path] = set()
    for generator in GENERATORS:
        for relative_path in registry_paths(generator):
            workspace_path = generator.crate / relative_path
            if workspace_path in listed:
                raise ValueError(f"artifact has multiple registry entries: {workspace_path}")
            listed.add(workspace_path)
        # Every generator also emits one schema index per crate. It is excluded
        # from `--list`, whose output must stay sorted and cannot place a single
        # per-crate file correctly against every crate's root directory name.
        listed.add(generator.crate / SCHEMA_INDEX_RELATIVE_PATH)

    actual_schemas = {path.relative_to(ROOT) for path in ROOT.glob("finstack-quant/*/schemas/**/*.schema.json")}
    registered_schemas = {path for path in listed if path.name.endswith(".schema.json")}
    if actual_schemas != registered_schemas:
        missing = sorted(registered_schemas - actual_schemas)
        extra = sorted(actual_schemas - registered_schemas)
        raise ValueError(f"schema registry mismatch: missing={missing or 'none'} extra={extra or 'none'}")

    validate_schema_graph(actual_schemas)

    with (
        tempfile.TemporaryDirectory(prefix="finstack-schema-a-") as first_dir,
        tempfile.TemporaryDirectory(prefix="finstack-schema-b-") as second_dir,
    ):
        first_root = Path(first_dir)
        second_root = Path(second_dir)
        generate_tree(first_root)
        generate_tree(second_root)
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

    print(
        f"Schema registries are complete and byte-reproducible: {len(actual_schemas)} schemas, sha256:{digest(first)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
