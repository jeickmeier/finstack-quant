"""Validate Rust crate publish metadata and first-crate dry-run order."""

from __future__ import annotations

import argparse
from collections import defaultdict, deque
from collections.abc import Iterable
from pathlib import Path
import subprocess
import sys
import tomllib

WORKSPACE_VERSION_KEY = ("workspace", "package", "version")
REQUIRED_FIRST_CRATE = "finstack-quant-test-utils"
REQUIRED_SECOND_CRATE = "finstack-quant-core"


def load_toml(path: Path) -> dict:
    """Load a TOML file."""
    with path.open("rb") as file:
        return tomllib.load(file)


def nested_get(data: dict, keys: tuple[str, ...]) -> object:
    """Read a nested TOML value."""
    current: object = data
    for key in keys:
        if not isinstance(current, dict):
            raise KeyError(key)
        current = current[key]
    return current


def package_name(manifest: Path) -> str | None:
    """Return the package name for a manifest if it has one."""
    package = load_toml(manifest).get("package")
    if not isinstance(package, dict):
        return None
    name = package.get("name")
    return name if isinstance(name, str) else None


def is_publishable(manifest: Path) -> bool:
    """Return whether a package manifest should be considered for crates.io publishing."""
    package = load_toml(manifest).get("package")
    if not isinstance(package, dict):
        return False
    return package.get("publish") is not False


def workspace_members(repo_root: Path) -> list[Path]:
    """Return workspace member manifests."""
    workspace = load_toml(repo_root / "Cargo.toml")["workspace"]
    members = workspace["members"]
    return [repo_root / member / "Cargo.toml" for member in members]


def finstack_crate_manifests(repo_root: Path) -> dict[str, Path]:
    """Return publishable Rust crate manifests under the Rust crate tree."""
    manifests: dict[str, Path] = {}
    rust_crate_root = repo_root / "finstack-quant"
    for manifest in workspace_members(repo_root):
        if not manifest.is_relative_to(rust_crate_root) or not is_publishable(manifest):
            continue
        name = package_name(manifest)
        if name is not None:
            manifests[name] = manifest
    return manifests


def dependency_tables(manifest_data: dict) -> Iterable[dict]:
    """Yield dependency tables that Cargo considers while packaging."""
    for key in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = manifest_data.get(key)
        if isinstance(table, dict):
            yield table

    for target in manifest_data.get("target", {}).values():
        if isinstance(target, dict):
            yield from dependency_tables(target)


def dependency_name(alias: str, spec: object) -> str | None:
    """Return the package name for a dependency spec."""
    if isinstance(spec, str):
        return alias
    if not isinstance(spec, dict):
        return None
    package = spec.get("package")
    return package if isinstance(package, str) else alias


def internal_dependencies(manifest: Path, known_crates: set[str]) -> set[str]:
    """Return internal finstack dependencies for a package manifest."""
    data = load_toml(manifest)
    dependencies: set[str] = set()
    for table in dependency_tables(data):
        for alias, spec in table.items():
            name = dependency_name(alias, spec)
            if name in known_crates:
                dependencies.add(name)
    return dependencies


def validate_internal_dependency_versions(
    repo_root: Path, manifests: dict[str, Path], workspace_version: str
) -> list[str]:
    """Return errors for internal path dependencies without explicit publish versions."""
    errors: list[str] = []
    known_crates = set(manifests)
    for _package, manifest in sorted(manifests.items()):
        data = load_toml(manifest)
        for table in dependency_tables(data):
            for alias, spec in table.items():
                name = dependency_name(alias, spec)
                if name not in known_crates or not isinstance(spec, dict) or "path" not in spec:
                    continue
                version = spec.get("version")
                if version != workspace_version:
                    rel_manifest = manifest.relative_to(repo_root)
                    errors.append(
                        f"{rel_manifest}: dependency `{alias}` ({name}) must declare "
                        f'version = "{workspace_version}" for publish packaging; found {version!r}'
                    )
    return errors


def publish_order(manifests: dict[str, Path]) -> list[str]:
    """Compute a deterministic dependency-first publish order."""
    known_crates = set(manifests)
    dependencies = {package: internal_dependencies(manifest, known_crates) for package, manifest in manifests.items()}
    dependents: dict[str, set[str]] = defaultdict(set)
    indegree = {package: len(package_deps) for package, package_deps in dependencies.items()}

    for package, package_deps in dependencies.items():
        for dep in package_deps:
            dependents[dep].add(package)

    priority = {REQUIRED_FIRST_CRATE: 0, "finstack-quant-valuations-macros": 1, REQUIRED_SECOND_CRATE: 2}
    ready = deque(
        sorted(
            (package for package, degree in indegree.items() if degree == 0),
            key=lambda item: (priority.get(item, 99), item),
        )
    )
    order: list[str] = []

    while ready:
        package = ready.popleft()
        order.append(package)
        for dependent in sorted(dependents[package], key=lambda item: (priority.get(item, 99), item)):
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                ready.append(dependent)
        ready = deque(sorted(ready, key=lambda item: (priority.get(item, 99), item)))

    if len(order) != len(manifests):
        cycle = sorted(package for package, degree in indegree.items() if degree > 0)
        raise RuntimeError(f"internal publish dependency cycle detected: {', '.join(cycle)}")
    return order


def run_publish_dry_run(repo_root: Path, manifest: Path) -> int:
    """Run cargo publish dry-run for a manifest."""
    command = ["cargo", "publish", "--dry-run", "--allow-dirty", "--manifest-path", str(manifest)]
    completed = subprocess.run(command, cwd=repo_root, check=False)  # noqa: S603
    return completed.returncode


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--skip-dry-run",
        action="store_true",
        help="Only validate manifest metadata and publish ordering.",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    """Validate publish metadata, publish order, and first-crate dry-run."""
    args = parse_args(argv)
    repo_root = Path(__file__).resolve().parents[1]
    workspace_version = str(nested_get(load_toml(repo_root / "Cargo.toml"), WORKSPACE_VERSION_KEY))
    manifests = finstack_crate_manifests(repo_root)
    required_manifests = {
        REQUIRED_FIRST_CRATE: manifests[REQUIRED_FIRST_CRATE],
        REQUIRED_SECOND_CRATE: manifests[REQUIRED_SECOND_CRATE],
    }

    errors = validate_internal_dependency_versions(repo_root, manifests, workspace_version)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1

    order = publish_order(required_manifests)
    first_index = order.index(REQUIRED_FIRST_CRATE)
    second_index = order.index(REQUIRED_SECOND_CRATE)
    if first_index >= second_index:
        print(
            f"`{REQUIRED_FIRST_CRATE}` must be published before `{REQUIRED_SECOND_CRATE}`; "
            f"computed order: {', '.join(order)}",
            file=sys.stderr,
        )
        return 1

    print("Rust crate publish order:", flush=True)
    for index, package in enumerate(order, start=1):
        print(f"{index}. {package}", flush=True)

    if args.skip_dry_run:
        return 0

    print(f"Running first-crate dry-run for `{REQUIRED_FIRST_CRATE}`...", flush=True)
    return run_publish_dry_run(repo_root, manifests[REQUIRED_FIRST_CRATE])


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
