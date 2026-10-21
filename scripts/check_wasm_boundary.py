"""Reject WASM bindings that bypass the structured serializer or error path.

Two rules, both run by `mise run wasm-lint` and the `wasm-boundary` pre-commit hook:

- `serde_wasm_bindgen::to_value` emits ES `Map`s that `JSON.stringify`
  silently drops. Every binding returns through `crate::utils::to_js_value`,
  which uses the `json_compatible` serializer; `src/utils/mod.rs` is the only
  legitimate call site.
- A JS error built from a string (`JsValue::from_str`,
  `JsValue::from(format!(..))`, `js_sys::Error::new`) has no `name` or `kind`,
  unlike every error routed through `crate::utils::to_js_err`. Helpers return a
  typed Rust error, not `Result<_, String>`. Comment lines are ignored.
"""

from __future__ import annotations

from pathlib import Path
import re
import sys

WASM_SRC = Path(__file__).resolve().parents[1] / "finstack-quant-wasm" / "src"

RAW_SERIALIZER = "serde_wasm_bindgen::to_value"
BARE_ERROR = re.compile(r"JsValue::from_str|js_sys::Error::new|Err\(JsValue::from|JsValue::from\(format!")
STRING_ERROR = re.compile(r"-> Result<[^>]*, String>")
COMMENT = re.compile(r"^\s*//")


def _lines(root: Path) -> list[tuple[Path, int, str]]:
    return [
        (path, number, line)
        for path in sorted(root.rglob("*.rs"))
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1)
    ]


def serializer_offenders(src: Path) -> list[str]:
    """Return `path:line: text` for every raw serializer call outside `src/utils/mod.rs`."""
    helper = src / "utils" / "mod.rs"
    return [
        f"{path}:{number}: {line.strip()}"
        for path, number, line in _lines(src)
        if RAW_SERIALIZER in line and path != helper
    ]


def error_offenders(src: Path) -> list[str]:
    """Return `path:line: text` for every string-built error under `src/api`."""
    return [
        f"{path}:{number}: {line.strip()}"
        for path, number, line in _lines(src / "api")
        if not COMMENT.match(line) and (BARE_ERROR.search(line) or STRING_ERROR.search(line))
    ]


def main(src: Path = WASM_SRC) -> int:
    """Print every violation and return a non-zero exit status when any exist."""
    status = 0
    serializer = serializer_offenders(src)
    if serializer:
        print("Raw serde_wasm_bindgen::to_value is banned outside src/utils/mod.rs.")
        print("It emits ES Maps that JSON.stringify drops; use crate::utils::to_js_value.")
        print("\n".join(serializer))
        status = 1
    errors = error_offenders(src)
    if errors:
        print("Bindings must throw through crate::utils::to_js_err (structured FinstackError with kind).")
        print("\n".join(errors))
        status = 1
    if status == 0:
        print("WASM boundary check passed: serializer and error paths are structured.")
    return status


if __name__ == "__main__":
    sys.exit(main())
