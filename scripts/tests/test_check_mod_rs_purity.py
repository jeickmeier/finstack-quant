"""Tests for the `mod.rs` purity and mixed-module-root checker."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_mod_rs_purity.py"
_SPEC = importlib.util.spec_from_file_location("check_mod_rs_purity", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def test_accepts_pure_mod_rs_and_umbrella_reexport_mods(tmp_path: Path) -> None:
    """Pure declarations, re-exports, doc comments, and umbrella re-export blocks pass."""
    _write(
        tmp_path / "finstack-quant/core/src/math/mod.rs",
        "//! Math module containing `fn`, `struct`, and `enum` in docs.\n"
        "mod engine;\n"
        "#[cfg(test)]\n"
        "mod tests;\n"
        "pub use engine::{compute, enumerate_paths};\n"
        "#[cfg(doctest)]\n"
        '#[doc = include_str!("../../README.md")]\n'
        "struct ReadmeDoctests;\n"
        "pub mod umbrella {\n"
        "    pub use super::engine::compute;\n"
        "}\n",
    )
    assert _MODULE.mod_rs_findings(tmp_path) == []
    assert _MODULE.mixed_root_findings(tmp_path) == []


def test_flags_logic_and_inline_tests_in_mod_rs(tmp_path: Path) -> None:
    """Items and inline `#[cfg(test)] mod tests { ... }` in `mod.rs` are rejected."""
    _write(
        tmp_path / "finstack-quant/core/src/foo/mod.rs",
        "mod sub;\n"
        "pub const LIMIT: usize = 10;\n"
        "pub struct Thing;\n"
        "pub fn helper() {}\n"
        "#[cfg(test)]\n"
        "mod tests {\n"
        "    #[test]\n"
        "    fn it_works() {}\n"
        "}\n",
    )
    found = _MODULE.mod_rs_findings(tmp_path)
    assert found == [
        "finstack-quant/core/src/foo/mod.rs:2: `const` item in mod.rs; move logic/definitions to a sibling module",
        "finstack-quant/core/src/foo/mod.rs:3: `struct` item in mod.rs; move logic/definitions to a sibling module",
        "finstack-quant/core/src/foo/mod.rs:4: `fn` item in mod.rs; move logic/definitions to a sibling module",
        "finstack-quant/core/src/foo/mod.rs:6: inline #[cfg(test)] mod `tests` in mod.rs; move to sibling tests.rs",
    ]


def test_flags_mixed_module_roots_but_ignores_tests_rs(tmp_path: Path) -> None:
    """`foo.rs` paired with `foo/` fails; `tests.rs` paired with `tests/` is allowed."""
    _write(tmp_path / "finstack-quant/core/src/foo.rs", "mod bar;\n")
    _write(tmp_path / "finstack-quant/core/src/foo/bar.rs", "pub fn x() {}\n")
    _write(tmp_path / "finstack-quant/core/src/baz/tests.rs", "fn t() {}\n")
    _write(tmp_path / "finstack-quant/core/src/baz/tests/extra.rs", "fn t2() {}\n")
    assert _MODULE.mixed_root_findings(tmp_path) == [
        "finstack-quant/core/src/foo.rs: mixed module root alongside `foo/`; convert to `foo/mod.rs`",
    ]


def test_ignores_binding_crates(tmp_path: Path) -> None:
    """Python and WASM binding crates are outside domain `finstack-quant/` scope."""
    _write(tmp_path / "finstack-quant-py/src/bindings/core/mod.rs", "pub fn register() {}\n")
    _write(tmp_path / "finstack-quant-wasm/src/api/core/mod.rs", "pub fn init() {}\n")
    assert _MODULE.mod_rs_findings(tmp_path) == []
    assert _MODULE.mixed_root_findings(tmp_path) == []


def test_report_mode_exits_zero(tmp_path: Path, monkeypatch, capsys) -> None:
    """`--report` prints findings and exits 0; default mode exits 1."""
    _write(tmp_path / "finstack-quant/core/src/a/mod.rs", "pub fn leak() {}\n")
    monkeypatch.setattr(_MODULE, "Path", lambda *_a, **_kw: tmp_path / "scripts" / "check_mod_rs_purity.py")
    assert _MODULE.main(["--report"]) == 0
    out = capsys.readouterr().out
    assert "finstack-quant/core/src/a/mod.rs:1" in out
    assert _MODULE.main([]) == 1
