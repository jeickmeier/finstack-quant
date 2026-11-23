"""Fail on logic-bearing `mod.rs` files or mixed `foo.rs` + `foo/` module roots.

Every Rust crate under `finstack-quant/` and `finstack-quant-wasm/` keeps
`mod.rs` as pure module wiring (attributes, `mod <name>;` declarations, and
`use`/`pub use` re-exports) and avoids mixed module roots where `foo.rs` sits
beside a `foo/` directory of submodules or sibling `tests.rs`.

`finstack-quant-py/` follows the same rule while allowing PyO3 submodule
registration wiring (`fn register*`, `fn add_*`, `const EXPORTS`, and the
`use` imports they reference) inside `mod.rs`.

Allowed inside domain and WASM `mod.rs`:
- doc comments, inner/outer attributes (`#![...]`, `#[...]`),
- out-of-line module declarations (`mod foo;`, including `#[cfg(test)] mod tests;`),
- `pub(...) use` re-exports and at most 3 private non-glob `use` statements
  (excluding `#[cfg(doc)]` doc-link imports),
- `#[cfg(doctest)] struct ReadmeDoctests;`,
- pure inline re-export umbrella modules (`pub mod foo { pub use ...; }`).

Forbidden inside `mod.rs`:
- any `fn`, `struct`, `enum`, `union`, `trait`, `impl`, `const`, `static`,
  `type`, or `macro_rules!` item (other than `ReadmeDoctests`, or PyO3
  registration wiring in `finstack-quant-py/`),
- any inline `mod <name> { ... }` containing logic or inline tests (extract to
  a sibling `.rs` file or `tests.rs`),
- any private glob import (`use ...::*;`) or >3 private `use` statements in
  domain/WASM `mod.rs` (implicit-prelude `mod.rs` pattern).

`--report` lists findings without failing.
"""

from __future__ import annotations

import argparse
from collections.abc import Iterator
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from rust_source import SKIPPED_DIRS

STRICT_ROOTS = ("finstack-quant", "finstack-quant-wasm")
PY_BINDING_ROOT = "finstack-quant-py"
ALL_ROOTS = (*STRICT_ROOTS, PY_BINDING_ROOT)
MAX_PRIVATE_USES_IN_MOD_RS = 3
PY_ALLOWED_FN_PREFIXES = ("register", "add_")
PY_ALLOWED_CONST_NAMES = frozenset({"EXPORTS"})
FORBIDDEN_ITEMS = frozenset({
    "const",
    "enum",
    "fn",
    "impl",
    "macro_rules",
    "static",
    "struct",
    "trait",
    "type",
    "union",
})
VISIBILITY_MODIFIERS = frozenset({"async", "default", "pub", "unsafe"})
IDENT_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
NON_ROOT_SPECIAL_NAMES = frozenset({"lib.rs", "main.rs", "mod.rs"})


def _tokenize_rust(source: str) -> Iterator[tuple[int, str, str]]:
    """Yield `(line, kind, text)` tokens for Rust code, skipping comments and strings.

    `kind` is one of `"ident"`, `"punct"`, or `"attr"`. Attributes (`#[...]` and
    `#![...]`) are collapsed into single `"attr"` tokens so keywords inside
    attribute arguments do not look like items.
    """
    n = len(source)
    i = 0
    line = 1

    def advance(count: int) -> None:
        nonlocal i, line
        line += source.count("\n", i, i + count)
        i += count

    while i < n:
        ch = source[i]
        if ch in " \t\r\n":
            advance(1)
            continue
        # Line comment (including /// and //!)
        if ch == "/" and i + 1 < n and source[i + 1] == "/":
            nl = source.find("\n", i)
            advance(n - i if nl == -1 else nl - i + 1)
            continue
        # Nested block comment
        if ch == "/" and i + 1 < n and source[i + 1] == "*":
            depth = 1
            advance(2)
            while i < n and depth > 0:
                if source[i : i + 2] == "/*":
                    depth += 1
                    advance(2)
                elif source[i : i + 2] == "*/":
                    depth -= 1
                    advance(2)
                else:
                    advance(1)
            continue
        # Raw string literal r#"..."# / br#"..."# / cr#"..."#
        raw_match = re.match(r'(?:[bc])?r(#*)"', source[i:])
        if raw_match:
            hashes = raw_match.group(1)
            advance(len(raw_match.group(0)))
            end_seq = '"' + hashes
            end_idx = source.find(end_seq, i)
            advance(n - i if end_idx == -1 else end_idx - i + len(end_seq))
            continue
        # Regular or byte/c string literal
        str_match = re.match(r'(?:[bc])?"', source[i:])
        if str_match:
            advance(len(str_match.group(0)))
            while i < n:
                if source[i] == "\\":
                    advance(2 if i + 1 < n else 1)
                elif source[i] == '"':
                    advance(1)
                    break
                else:
                    advance(1)
            continue
        # Char literal vs lifetime ('a vs 'x' or '\n')
        if ch == "'":
            char_match = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^'\\\n])'", source[i:])
            if char_match:
                advance(len(char_match.group(0)))
                continue
            advance(1)
            continue
        # Inner or outer Rust attribute.
        if ch == "#" and (source[i : i + 2] == "#[" or source[i : i + 3] == "#!["):
            start_line = line
            start = i
            advance(3 if source[i : i + 3] == "#![" else 2)
            depth = 1
            while i < n and depth > 0:
                c = source[i]
                if c == '"':
                    advance(1)
                    while i < n:
                        if source[i] == "\\":
                            advance(2 if i + 1 < n else 1)
                        elif source[i] == '"':
                            advance(1)
                            break
                        else:
                            advance(1)
                elif c == "/" and i + 1 < n and source[i + 1] == "/":
                    nl = source.find("\n", i)
                    advance(n - i if nl == -1 else nl - i + 1)
                elif c == "[":
                    depth += 1
                    advance(1)
                elif c == "]":
                    depth -= 1
                    advance(1)
                else:
                    advance(1)
            yield start_line, "attr", source[start:i]
            continue
        ident = IDENT_RE.match(source, i)
        if ident:
            text = ident.group(0)
            tok_line = line
            advance(len(text))
            yield tok_line, "ident", text
            continue
        tok_line = line
        advance(1)
        yield tok_line, "punct", ch


def _skip_parens(tokens: list[tuple[int, str, str]], idx: int) -> int:
    """Advance past a balanced `(...)` group starting at `tokens[idx]`."""
    depth = 0
    while idx < len(tokens):
        _, kind, text = tokens[idx]
        if kind == "punct" and text == "(":
            depth += 1
        elif kind == "punct" and text == ")":
            depth -= 1
            if depth <= 0:
                return idx + 1
        idx += 1
    return idx


def _skip_statement(tokens: list[tuple[int, str, str]], idx: int) -> int:
    """Advance past an item or statement ending at `;` or a balanced `{ ... }`."""
    brace_depth = 0
    while idx < len(tokens):
        _, kind, text = tokens[idx]
        if kind == "punct":
            if text == "{":
                brace_depth += 1
            elif text == "}":
                brace_depth -= 1
                if brace_depth <= 0:
                    return idx + 1
            elif text == ";" and brace_depth == 0:
                return idx + 1
        idx += 1
    return idx


def _check_mod_block(
    tokens: list[tuple[int, str, str]],
    start: int,
    end: int,
    rel: str,
    findings: list[str],
    *,
    allow_py_registration: bool = False,
) -> None:
    """Inspect a module token slice `[start, end)` for non-wiring items."""
    idx = start
    pending_attrs: list[str] = []
    private_use_count = 0
    first_private_use_line: int | None = None

    while idx < end:
        tok_line, kind, text = tokens[idx]
        if kind == "attr":
            pending_attrs.append(text)
            idx += 1
            continue
        if kind == "punct" and text == ";":
            pending_attrs.clear()
            idx += 1
            continue

        # Consume leading visibility and modifiers (`pub`, `pub(crate)`, `unsafe`, `async`, `default`).
        item_line = tok_line
        is_pub = False
        while idx < end and tokens[idx][1] == "ident" and tokens[idx][2] in VISIBILITY_MODIFIERS:
            mod_kw = tokens[idx][2]
            idx += 1
            if mod_kw == "pub":
                is_pub = True
                if idx < end and tokens[idx][1] == "punct" and tokens[idx][2] == "(":
                    idx = _skip_parens(tokens, idx)

        if idx >= end:
            break

        kw_line, kw_kind, kw = tokens[idx]
        if kw_kind == "ident" and kw == "use":
            is_cfg_doc = any(re.search(r"#\[\s*cfg\s*\(\s*doc\s*\)\s*\]", attr) for attr in pending_attrs)
            pending_attrs.clear()
            stmt_end = _skip_statement(tokens, idx + 1)
            if not is_pub and not is_cfg_doc:
                use_tokens = [tok[2] for tok in tokens[idx + 1 : stmt_end] if tok[1] in ("ident", "punct")]
                has_glob = "*" in use_tokens
                is_pyo3_prelude = allow_py_registration and "".join(use_tokens).rstrip(";") == "pyo3::prelude::*"
                if has_glob and not is_pyo3_prelude:
                    findings.append(
                        f"{rel}:{kw_line}: private glob `use ...::*;` in mod.rs; import explicitly in child modules"
                    )
                private_use_count += 1
                if first_private_use_line is None:
                    first_private_use_line = kw_line
            idx = stmt_end
            continue

        if kw_kind == "ident" and kw == "mod":
            # `mod name;` is allowed; `mod name { ... }` is allowed only when its body is pure
            # re-exports/declarations and it is not a `#[cfg(test)]` inline test module.
            is_cfg_test = any(re.search(r"#\[\s*cfg\s*\([^)]*\btest\b", attr) for attr in pending_attrs)
            pending_attrs.clear()
            name_tok = tokens[idx + 1] if idx + 1 < end else (kw_line, "ident", "<unknown>")
            after = idx + 2
            if after < end and tokens[after][1] == "punct" and tokens[after][2] == ";":
                idx = after + 1
                continue
            if after < end and tokens[after][1] == "punct" and tokens[after][2] == "{":
                body_start = after + 1
                depth = 1
                cursor = body_start
                while cursor < end and depth > 0:
                    if tokens[cursor][1] == "punct" and tokens[cursor][2] == "{":
                        depth += 1
                    elif tokens[cursor][1] == "punct" and tokens[cursor][2] == "}":
                        depth -= 1
                    cursor += 1
                body_end = cursor - 1
                if is_cfg_test:
                    findings.append(
                        f"{rel}:{item_line}: inline #[cfg(test)] mod `{name_tok[2]}` in mod.rs; move to sibling tests.rs"
                    )
                else:
                    sub_findings: list[str] = []
                    _check_mod_block(
                        tokens,
                        body_start,
                        body_end,
                        rel,
                        sub_findings,
                        allow_py_registration=allow_py_registration,
                    )
                    findings.extend(sub_findings)
                idx = cursor
                continue
            idx = _skip_statement(tokens, idx + 1)
            continue

        if kw_kind == "ident" and kw in FORBIDDEN_ITEMS:
            next_name = tokens[idx + 1][2] if idx + 1 < end and tokens[idx + 1][1] == "ident" else ""
            # Permit the README doctest marker: `#[cfg(doctest)] struct ReadmeDoctests;`
            is_readme_doctest = (
                kw == "struct" and next_name == "ReadmeDoctests" and any("doctest" in attr for attr in pending_attrs)
            )
            # In finstack-quant-py, permit PyO3 module registration wiring (`fn register*`, `fn add_*`, `const EXPORTS`).
            is_py_wiring = allow_py_registration and (
                (kw == "fn" and next_name.startswith(PY_ALLOWED_FN_PREFIXES))
                or (kw == "const" and (next_name in PY_ALLOWED_CONST_NAMES or next_name.endswith("_EXPORTS")))
            )
            pending_attrs.clear()
            if not is_readme_doctest and not is_py_wiring:
                label = "macro_rules!" if kw == "macro_rules" else kw
                findings.append(
                    f"{rel}:{kw_line}: `{label}` item in mod.rs; move logic/definitions to a sibling module"
                )
            idx = _skip_statement(tokens, idx + 1)
            continue

        # Macro invocation or other statement at module scope.
        pending_attrs.clear()
        if kw_kind == "ident" and idx + 1 < end and tokens[idx + 1][1] == "punct" and tokens[idx + 1][2] == "!":
            findings.append(f"{rel}:{kw_line}: macro invocation `{kw}!` in mod.rs; move logic to a sibling module")
        idx = _skip_statement(tokens, idx + 1)

    if (
        not allow_py_registration
        and private_use_count > MAX_PRIVATE_USES_IN_MOD_RS
        and first_private_use_line is not None
    ):
        findings.append(
            f"{rel}:{first_private_use_line}: {private_use_count} private `use` statements in mod.rs "
            f"(max {MAX_PRIVATE_USES_IN_MOD_RS}); import directly in child modules instead of using mod.rs as an implicit prelude"
        )


def _iter_src_files(root: Path, roots: tuple[str, ...] = ALL_ROOTS) -> Iterator[Path]:
    """Yield every `.rs` file under `<root_dir>/**/src/` for the configured workspace roots."""
    for sub in roots:
        base = root / sub
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.rs")):
            rel_parts = path.relative_to(root).parts
            if "src" not in rel_parts:
                continue
            if any(part in SKIPPED_DIRS or part.startswith(".") for part in rel_parts):
                continue
            yield path


def mod_rs_findings(root: Path) -> list[str]:
    """Every logic-bearing `mod.rs` item across domain, WASM, and Python binding crates."""
    found: list[str] = []
    for path in _iter_src_files(root):
        if path.name != "mod.rs":
            continue
        rel = path.relative_to(root).as_posix()
        allow_py = rel.startswith(PY_BINDING_ROOT + "/")
        tokens = list(_tokenize_rust(path.read_text(encoding="utf-8", errors="ignore")))
        _check_mod_block(tokens, 0, len(tokens), rel, found, allow_py_registration=allow_py)
    return found


def mixed_root_findings(root: Path) -> list[str]:
    """Every `foo.rs` paired with a sibling `foo/` directory containing `.rs` files."""
    found: list[str] = []
    for path in _iter_src_files(root):
        if path.name in NON_ROOT_SPECIAL_NAMES:
            continue
        sibling_dir = path.with_suffix("")
        if sibling_dir.is_dir() and any(sibling_dir.rglob("*.rs")):
            rel = path.relative_to(root).as_posix()
            stem = path.stem
            found.append(f"{rel}: mixed module root alongside `{stem}/`; convert to `{stem}/mod.rs`")
    return found


def main(argv: list[str] | None = None) -> int:
    """Print findings; exit 1 on any unless `--report`."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--report", action="store_true", help="list findings without failing")
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parent.parent
    found = mod_rs_findings(root) + mixed_root_findings(root)
    for finding in found:
        print(finding)
    if found:
        print(f"{len(found)} mod.rs purity or mixed-module-root violation(s) in workspace Rust crates.")
        return 0 if args.report else 1
    print("All workspace mod.rs files are pure and no mixed module roots exist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
