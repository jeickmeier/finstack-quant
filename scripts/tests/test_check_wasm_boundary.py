"""Tests for the WASM serializer and structured-error boundary guard."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_wasm_boundary.py"
_SPEC = importlib.util.spec_from_file_location("check_wasm_boundary", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)


def _write(root: Path, relative: str, text: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def test_raw_serializer_is_allowed_only_in_the_helper(tmp_path: Path) -> None:
    """Only `src/utils/mod.rs` may call the raw serializer."""
    _write(tmp_path, "utils/mod.rs", "serde_wasm_bindgen::to_value(&v)\n")
    _write(tmp_path, "api/core/money.rs", "let out = serde_wasm_bindgen::to_value(&v);\n")

    offenders = _MODULE.serializer_offenders(tmp_path)

    assert len(offenders) == 1
    assert "api/core/money.rs:1:" in offenders[0]


def test_string_built_errors_are_rejected_but_comments_are_not(tmp_path: Path) -> None:
    """String-built errors and `Result<_, String>` helpers fail; comment lines do not."""
    _write(
        tmp_path,
        "api/core/dates.rs",
        '// JsValue::from_str is banned\nreturn Err(JsValue::from_str("bad"));\nfn helper() -> Result<f64, String> {\n',
    )

    offenders = _MODULE.error_offenders(tmp_path)

    assert [line.split("dates.rs:", 1)[1].split(":", 1)[0] for line in offenders] == ["2", "3"]


def test_clean_tree_passes(tmp_path: Path) -> None:
    """A tree with no violations exits zero."""
    _write(tmp_path, "utils/mod.rs", "serde_wasm_bindgen::to_value(&v)\n")
    _write(tmp_path, "api/core/money.rs", "self.inner.checked_add(o).map_err(to_js_err)\n")

    assert _MODULE.main(tmp_path) == 0
