"""Tests for the Rust source-file size check."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_loc.py"
_SPEC = importlib.util.spec_from_file_location("check_loc", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _measured(root: Path) -> dict[str, int]:
    return dict(_MODULE.measure(root))


def test_counts_only_rust_under_crate_src(tmp_path: Path) -> None:
    """Only `.rs` files under a crate's `src/` count; other languages are ignored."""
    _write(tmp_path / "finstack-quant/core/src/lib.rs", "fn a() {}\n")
    _write(tmp_path / "finstack-quant/core/build/gen.rs", "fn b() {}\n")
    _write(tmp_path / "finstack-quant-wasm/index.d.ts", "x\n" * 5000)
    _write(tmp_path / "finstack-quant-ui/src/generated/types.ts", "x\n" * 5000)

    assert _measured(tmp_path) == {"finstack-quant/core/src/lib.rs": 1}


def test_excludes_test_files_and_generated_sources(tmp_path: Path) -> None:
    """Sibling test files, `tests/` directories and generated code are not source."""
    for rel in (
        "finstack-quant/core/src/money/tests.rs",
        "finstack-quant/core/src/tests/helpers.rs",
        "finstack-quant/core/src/generated/iso.rs",
        "finstack-quant/core/tests/integration.rs",
    ):
        _write(tmp_path / rel, "fn f() {}\n")

    assert _measured(tmp_path) == {}


def test_strips_test_only_items_but_not_cfg_not_test(tmp_path: Path) -> None:
    """`#[cfg(test)]` items are excluded; `#[cfg(not(test))]` code still counts."""
    source = "\n".join([
        "fn production() {}",  # 1
        "#[cfg(not(test))]",  # 2
        "fn only_outside_tests() {}",  # 3
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn t() {}",
        "}",
        "",
    ])
    _write(tmp_path / "finstack-quant/core/src/lib.rs", source)

    assert _measured(tmp_path) == {"finstack-quant/core/src/lib.rs": 3}


def test_ci_fails_only_above_the_hard_limit(tmp_path: Path, monkeypatch, capsys) -> None:
    """`--ci` exits 1 for a file over the hard limit and 0 for one between the limits."""
    monkeypatch.setattr(_MODULE, "__file__", str(tmp_path / "scripts" / "check_loc.py"))
    _write(tmp_path / "finstack-quant/core/src/big.rs", "fn f() {}\n" * 1500)
    assert _MODULE.main(["--ci"]) == 0
    assert "1 Rust source file(s) between 1000 and 2000 lines" in capsys.readouterr().out

    _write(tmp_path / "finstack-quant/core/src/big.rs", "fn f() {}\n" * 2001)
    assert _MODULE.main(["--ci"]) == 1
