#!/usr/bin/env python3
"""Select the phase-end gates a diff needs.

Prints one line per fact: `gate <name>` for each gate to run and
`doctest_packages <pkg> ...` when the Rust doctests can be narrowed to the
changed crates. A path no rule recognises selects nothing; a change to the
build, CI or this selector itself selects everything.

Usage: gate_select.py [--base REF] [--paths FILE...]
"""

from __future__ import annotations

import argparse
import subprocess

GATES = (
    "lint",
    "rust-test",
    "rust-doctest",
    "python",
    "wasm",
    "ui-check",
    "ui-e2e",
    "ui-harness",
    "ui-install",
    "docs-build",
    "notebooks",
)

EVERYTHING = (
    "mise.toml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "pyproject.toml",
    "uv.lock",
    ".github/",
    "scripts/gate_select.py",
    "scripts/phase_gate.sh",
)

# ui-install installs every registry item into a scratch app, so it reruns only
# when an installed file, the installer or the WASM package surface changes.
UI_INSTALL = (
    "finstack-quant-ui/registry/",
    "finstack-quant-ui/public/r/",
    "finstack-quant-ui/tests/install/",
    "finstack-quant-ui/tests/browser/",
    "finstack-quant-ui/scripts/",
    "finstack-quant-wasm/index.d.ts",
    "finstack-quant-wasm/facade-surface.json",
    "finstack-quant-wasm/exports/",
    "finstack-quant-wasm/src/",
)

ARROW = "finstack-quant-arrow"


def rust_package(path: str) -> str | None:
    """Return the Cargo package owning a path under finstack-quant/, if it has one."""
    parts = path.split("/")
    if len(parts) < 3 or parts[0] != "finstack-quant":
        return None
    crate = parts[1]
    if crate == "arrow-interchange":
        return ARROW
    if crate == "valuations" and parts[2] in {
        "core",
        "rates",
        "fixed-income",
        "structured-credit",
        "credit",
        "equity",
        "fx",
        "exotics",
    }:
        return f"finstack-quant-valuations-{parts[2]}"
    if crate in {"src", "tests", "test-utils"}:
        return "finstack-quant"
    return f"finstack-quant-{crate}"


def select(paths: list[str]) -> tuple[set[str], set[str]]:
    """Map changed paths to (gates, doctest packages)."""
    gates: set[str] = set()
    packages: set[str] = set()
    for path in paths:
        if path.startswith(EVERYTHING):
            return set(GATES), set()
        if path.startswith("finstack-quant/"):
            gates |= {"lint", "rust-test", "rust-doctest", "python", "wasm", "ui-check", "ui-e2e", "notebooks"}
            package = rust_package(path)
            if package:
                packages.add(package)
        elif path.startswith("finstack-quant-py/"):
            gates |= {"lint", "python", "notebooks"}
        elif path.startswith("finstack-quant-wasm/"):
            gates |= {"lint", "wasm", "ui-check", "ui-e2e"}
        elif path.startswith("finstack-quant-ui/"):
            gates |= {"lint", "ui-check", "ui-e2e", "ui-harness"}
        elif path.startswith("docs-site/"):
            gates |= {"ui-check", "docs-build"}
        if path.startswith(UI_INSTALL):
            gates |= {"ui-install", "docs-build"}
    return gates, packages


def main() -> int:
    """Print the gates for the diff against --base."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="master", help="Diff base ref")
    parser.add_argument("--paths", nargs="*", help="Use these paths instead of git diff")
    args = parser.parse_args()
    if args.paths is not None:
        paths = args.paths
    else:
        committed = subprocess.run(  # noqa: S603
            ["/usr/bin/git", "diff", "--name-only", f"{args.base}...HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.split()
        dirty = subprocess.run(
            ["/usr/bin/git", "status", "--porcelain"], capture_output=True, text=True, check=True
        ).stdout.splitlines()
        paths = sorted({*committed, *(line[3:].split(" -> ")[-1] for line in dirty)})
    gates, packages = select(paths)
    for gate in GATES:
        if gate in gates:
            print(f"gate {gate}")
    # A full-workspace doctest run is needed when no crate was identified.
    if "rust-doctest" in gates and packages:
        print("doctest_packages " + " ".join(sorted(packages)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
