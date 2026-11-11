"""The Python extension build reuses the Rust test build's compiled crates.

`maturin develop` builds `finstack-quant-py` alone; `rust-test` and `gen-write`
build the rest of the workspace with its dev-dependencies. Cargo compiles a
crate once per distinct feature set of the crate and of everything below it, so
a single feature that only one of the two builds turns on recompiles the stack
from that crate up. The alignment entries in `finstack-quant-py/Cargo.toml` and
`finstack-quant/test-utils/Cargo.toml` keep the two graphs equal for every
package the library crates build on; this test fails when they drift.
"""

from __future__ import annotations

from pathlib import Path
import re
import shutil
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[2]
# Members whose own dependencies only one of the two builds compiles.
BINDING_ONLY = ("finstack-quant-py", "finstack-quant-wasm", "finstack-quant-wasm-macros", "finstack-quant-arrow")
EXTENSION_BUILD = ("-p", "finstack-quant-py", "--features", "extension-module", "-e", "normal,build")
TEST_BUILD = ("--workspace", "--exclude", "finstack-quant-py", "-e", "normal,build,dev")


def _features(*selection: str) -> dict[str, frozenset[str]]:
    """Package (`name version`) -> enabled features for one cargo invocation."""
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo is not on PATH")
    out = subprocess.run(  # noqa: S603
        [cargo, "tree", "--locked", *selection, "--prefix", "none", "-f", "{p}|{f}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    features: dict[str, set[str]] = {}
    for line in out.splitlines():
        package, _, enabled = line.replace(" (*)", "").partition("|")
        name = re.sub(r" \(.*\)$", "", package).strip()
        features.setdefault(name, set()).update(feature for feature in enabled.split(",") if feature)
    return {name: frozenset(enabled) for name, enabled in features.items()}


def test_extension_build_and_test_build_enable_the_same_features() -> None:
    """Every package under the library crates has one feature set in both builds."""
    excludes = [arg for member in BINDING_ONLY for arg in ("--exclude", member)]
    library = _features("--workspace", *excludes, "-e", "normal,build")
    extension = _features(*EXTENSION_BUILD)
    tests = _features(*TEST_BUILD)
    drift = {
        package: (sorted(extension[package] - tests[package]), sorted(tests[package] - extension[package]))
        for package in sorted(library)
        if package in extension and extension[package] != tests[package]
    }
    assert not drift, (
        "features differ between the extension build and the test build "
        f"(package: only in extension, only in tests): {drift}; "
        "add the missing feature to the alignment block of finstack-quant-py/Cargo.toml "
        "or finstack-quant/test-utils/Cargo.toml"
    )
