"""Enforce substantive input documentation on exported Rust callables.

The Rust compiler's ``missing_docs`` lint establishes that public items have a
doc comment, but it cannot prove that callers know how to supply every input.
This checker builds rustdoc JSON through ``cargo public-api`` and checks every
publicly reachable function, associated function, trait method, and constructor
with non-``self`` inputs. Each must provide a ``# Arguments`` section containing
a non-trivial Markdown-list entry for every input.

Run ``python3 scripts/check_public_api_input_docs.py --help`` for focused
package checks. ``mise run rust-doc`` runs the full workspace gate.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
TARGET_DOC_DIR = REPO_ROOT / "target" / "doc"
ARGUMENTS_HEADING_RE = re.compile(r"(?mi)^# Arguments\s*$")
GENERIC_DESCRIPTION_RE = re.compile(
    r"^(?:the )?(?:input|parameter|value)(?: (?:value|parameter|to use|for the operation))?[.!]?$",
    re.IGNORECASE,
)


class DocumentationError:
    """A missing or insufficient input-documentation entry."""

    def __init__(self, package: str, path: str, line: int, symbol: str, message: str) -> None:
        """Create a diagnostic tied to one exported callable."""
        self.package = package
        self.path = path
        self.line = line
        self.symbol = symbol
        self.message = message

    def format(self) -> str:
        """Render the diagnostic in a compiler-style format."""
        return f"{self.path}:{self.line}: {self.package}::{self.symbol}: {self.message}"


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse package-selection and build-control options."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-p",
        "--package",
        action="append",
        dest="packages",
        help="Check one Cargo package; repeat to check several. Defaults to every workspace library package.",
    )
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Reuse existing rustdoc JSON in target/doc instead of running cargo public-api.",
    )
    parser.add_argument(
        "--max-errors",
        type=int,
        default=200,
        help="Maximum diagnostics to print before showing a summary (default: 200).",
    )
    return parser.parse_args(argv)


def cargo_metadata() -> dict[str, Any]:
    """Load workspace package metadata from Cargo."""
    completed = subprocess.run(  # noqa: S603 -- fixed Cargo subcommand; no shell is used.
        [cargo_executable(), "metadata", "--no-deps", "--format-version", "1"],
        cwd=REPO_ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return json.loads(completed.stdout)


def cargo_executable() -> str:
    """Resolve Cargo to an absolute executable path for controlled subprocess calls."""
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo is required to check public API documentation")
    return cargo


def workspace_libraries(metadata: dict[str, Any], requested: list[str] | None) -> list[tuple[str, str]]:
    """Return package names and rustdoc JSON stems for selected workspace libraries."""
    workspace_members = set(metadata["workspace_members"])
    libraries: list[tuple[str, str]] = []

    for package in metadata["packages"]:
        if package["id"] not in workspace_members:
            continue
        library_targets = [target for target in package["targets"] if "lib" in target["kind"]]
        if not library_targets:
            continue
        if requested is not None and package["name"] not in requested:
            continue
        libraries.append((package["name"], library_targets[0]["name"]))

    missing = set(requested or ()) - {package for package, _ in libraries}
    if missing:
        names = ", ".join(sorted(missing))
        raise ValueError(f"requested package is not a workspace library: {names}")
    return sorted(libraries)


def build_rustdoc_json(package: str) -> None:
    """Generate current public rustdoc metadata for one package."""
    subprocess.run(  # noqa: S603 -- package is constrained to Cargo workspace metadata.
        [cargo_executable(), "public-api", "-p", package, "-sss", "--color", "never"],
        cwd=REPO_ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def argument_names(item: dict[str, Any]) -> list[str]:
    """Return caller-supplied argument names from one rustdoc function item."""
    inputs = item["inner"]["function"]["sig"]["inputs"]
    return [name for name, _ in inputs if name and name != "self"]


def argument_section(docs: str) -> str | None:
    """Extract the Arguments section up to the next same-level heading."""
    match = ARGUMENTS_HEADING_RE.search(docs)
    if match is None:
        return None
    remainder = docs[match.end() :]
    next_heading = re.search(r"(?m)^# (?!Arguments\s*$)", remainder)
    return remainder if next_heading is None else remainder[: next_heading.start()]


def parameter_description(section: str, parameter: str) -> str | None:
    """Return the list-item text for one exact parameter name, if present."""
    escaped = re.escape(parameter)
    pattern = re.compile(rf"(?ms)^\s*[-*]\s+`{escaped}`\s*(?:[-—:]\s+)(.+?)(?=^\s*[-*]\s+`|^#|\Z)")
    match = pattern.search(section)
    if match is None:
        return None
    return " ".join(match.group(1).split())


def is_substantive(description: str) -> bool:
    """Reject empty, tautological, and implausibly short parameter descriptions."""
    plain_text = re.sub(r"[`*_\[\]()]", "", description).strip()
    return len(plain_text) >= 16 and GENERIC_DESCRIPTION_RE.fullmatch(plain_text) is None


def public_callable_errors(package: str, rustdoc_path: Path) -> list[DocumentationError]:
    """Find missing input documentation in one package's generated rustdoc JSON."""
    document = json.loads(rustdoc_path.read_text())
    public_paths = set(document["paths"])
    errors: list[DocumentationError] = []

    for item_id, item in document["index"].items():
        if item_id not in public_paths or item.get("crate_id") != 0 or item.get("visibility") != "public":
            continue
        if "function" not in item.get("inner", {}):
            continue
        parameters = argument_names(item)
        if not parameters:
            continue

        span = item.get("span") or {}
        path = span.get("filename", "<generated>")
        line = span.get("begin", [0])[0]
        symbol = "::".join(document["paths"][item_id]["path"])
        docs = item.get("docs") or ""
        section = argument_section(docs)
        if section is None:
            errors.append(DocumentationError(package, path, line, symbol, "missing # Arguments section"))
            continue

        for parameter in parameters:
            description = parameter_description(section, parameter)
            if description is None:
                errors.append(
                    DocumentationError(package, path, line, symbol, f"missing documentation for `{parameter}`")
                )
            elif not is_substantive(description):
                errors.append(
                    DocumentationError(
                        package,
                        path,
                        line,
                        symbol,
                        f"documentation for `{parameter}` is not substantive",
                    )
                )
    return errors


def main(argv: list[str]) -> int:
    """Build selected rustdoc metadata, then report every input-doc contract violation."""
    args = parse_args(argv)
    try:
        libraries = workspace_libraries(cargo_metadata(), args.packages)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    errors: list[DocumentationError] = []
    for package, rustdoc_stem in libraries:
        if not args.skip_build:
            print(f"checking public input documentation: {package}", file=sys.stderr)
            build_rustdoc_json(package)
        rustdoc_path = TARGET_DOC_DIR / f"{rustdoc_stem}.json"
        if not rustdoc_path.exists():
            print(f"error: missing rustdoc JSON for {package}: {rustdoc_path}", file=sys.stderr)
            return 2
        errors.extend(public_callable_errors(package, rustdoc_path))

    if not errors:
        print(f"public input documentation: clean ({len(libraries)} packages)")
        return 0

    for error in errors[: args.max_errors]:
        print(error.format(), file=sys.stderr)
    if len(errors) > args.max_errors:
        print(f"... {len(errors) - args.max_errors} additional documentation errors omitted", file=sys.stderr)
    print(f"public input documentation: {len(errors)} error(s) in {len(libraries)} packages", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
