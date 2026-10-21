"""Host names in ``finstack_quant.models`` are Rust names.

AGENTS.md makes Rust the canonical API: a bound function, class or class member
carries the name of the Rust item it binds. The Python <-> WASM ledger in
``test_parity_ledger.py`` cannot see a name both hosts spell the same way but
Rust spells differently, so this test compares the Python ``models`` surface
(and, through the ledger, its WASM twins) against the Rust sources of the crates
it binds: ``models``, ``factor`` and ``core``.

The comparison is lexical. Rust identifiers are read from non-test code with
comments and string literals blanked:

* a free function matches any ``pub fn`` of that name;
* a class matches a ``pub struct`` / ``enum`` / ``trait`` / ``type`` of that name;
* a class member matches a ``fn`` in an ``impl`` block or trait body of the
  same-named Rust type, one of its ``pub`` fields, or the snake_case of one of its enum variants
  (an enum-variant constructor or constant). A trailing ``_`` (Python keyword
  escape such as ``lambda_``) is dropped first.

Host-standard members are not Rust items: the wire exits ``to_json`` /
``from_json`` / ``to_dict`` / ``from_dict`` (the result-return contract) and the
ledger's rule-excluded pandas/arrow exits. Every other mismatch is recorded in
``parity_contract.toml`` under ``[parity.rust_names]`` with a reason, and an entry
that no longer mismatches must be removed.
"""

from __future__ import annotations

import inspect
from pathlib import Path
import re
import tomllib
from typing import Any

import finstack_quant.models

ROOT = Path(__file__).parents[3]
LEDGER: dict[str, Any] = tomllib.loads((Path(__file__).parents[2] / "parity_contract.toml").read_text())["parity"]
RECORDED: dict[str, str] = LEDGER.get("rust_names", {})
PY_RULE_MEMBERS = set(LEDGER["python_rule_excluded_members"])
WIRE_MEMBERS = {"to_json", "from_json", "to_dict", "from_dict"}
RUST_ROOTS = [ROOT / "finstack-quant" / crate / "src" for crate in ("models", "factor", "core")]


def _code_only(text: str) -> str:
    """Blank comments, string literals and char literals, keeping offsets and newlines."""
    out = list(text)
    i, n = 0, len(text)

    def blank(start: int, end: int) -> None:
        for k in range(start, min(end, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end == -1 else end
            blank(i, end)
            i = end
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif (raw := re.match(r'(?:b|c)?r(#*)"', text[i : i + 260])) and (i == 0 or not text[i - 1].isalnum()):
            closing = '"' + raw.group(1)
            end = text.find(closing, i + raw.end())
            end = n if end == -1 else end + len(closing)
            blank(i, end)
            i = end
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(i, j + 1)
            i = j + 1
        elif c == "'" and (char := re.match(r"'(?:\\.[^']*|[^\\'])'", text[i : i + 12])):
            blank(i, i + char.end())
            i += char.end()
        else:
            i += 1
    return "".join(out)


def _block_end(code: str, open_brace: int) -> int:
    """Offset just past the brace that closes the one at ``open_brace``."""
    depth = 0
    for k in range(open_brace, len(code)):
        if code[k] == "{":
            depth += 1
        elif code[k] == "}":
            depth -= 1
            if depth == 0:
                return k + 1
    return len(code)


def _without_test_items(code: str) -> str:
    """Drop every ``#[cfg(test)]`` item (test modules and test-only functions)."""
    while (match := re.search(r"#\[cfg\(test\)\]", code)) is not None:
        brace = code.find("{", match.end())
        semi = code.find(";", match.end())
        end = semi + 1 if semi != -1 and (brace == -1 or semi < brace) else _block_end(code, brace)
        code = code[: match.start()] + code[end:]
    return code


def _snake(name: str) -> str:
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name).lower()


def _rust_index() -> tuple[set[str], set[str], dict[str, set[str]]]:
    """Public fn names, public type names and member names per Rust type name."""
    functions: set[str] = set()
    types: set[str] = set()
    members: dict[str, set[str]] = {}
    for root in RUST_ROOTS:
        for path in sorted(root.rglob("*.rs")):
            if path.name == "tests.rs" or "tests" in path.relative_to(root).parts[:-1]:
                continue
            code = _without_test_items(_code_only(path.read_text()))
            functions |= set(re.findall(r"\bpub(?:\([^)]*\))?\s+(?:const\s+)?(?:unsafe\s+)?fn\s+(\w+)", code))
            types |= set(re.findall(r"\bpub(?:\([^)]*\))?\s+(?:struct|enum|trait|type)\s+(\w+)", code))
            for impl in re.finditer(
                r"\bimpl\b(?:\s*<[^{;]*?>)?\s+(?:[\w:]+(?:<[^{;]*?>)?\s+for\s+)?(?:[\w]+::)*(\w+)[^{;]*\{", code
            ):
                body = code[impl.end() - 1 : _block_end(code, impl.end() - 1)]
                members.setdefault(impl.group(1), set()).update(re.findall(r"\bfn\s+(\w+)", body))
            for item in re.finditer(r"\bpub\s+(struct|enum|trait)\s+(\w+)[^{;]*\{", code):
                body = code[item.end() : _block_end(code, item.end() - 1) - 1]
                if item.group(1) == "trait":
                    found = re.findall(r"\bfn\s+(\w+)", body)
                elif item.group(1) == "struct":
                    found = re.findall(r"\bpub(?:\([^)]*\))?\s+(\w+)\s*:", body)
                else:
                    found = [
                        _snake(variant) for variant in re.findall(r"(?:^|,)\s*(?:#\[[^\]]*\]\s*)*([A-Z]\w*)", body)
                    ]
                members.setdefault(item.group(2), set()).update(found)
    return functions, types, members


def _python_models_surface() -> dict[str, Any]:
    """Every public function and class under ``finstack_quant.models``, by qualified name."""
    found: dict[str, Any] = {}
    seen: set[str] = set()

    def walk(module: Any, path: str) -> None:
        if path in seen:
            return
        seen.add(path)
        for name in getattr(module, "__all__", None) or []:
            value = getattr(module, name)
            if inspect.ismodule(value):
                walk(value, f"{path}.{name}")
            elif inspect.isclass(value) or inspect.isroutine(value):
                found.setdefault(f"{path}.{name}", value)

    walk(finstack_quant.models, "finstack_quant.models")
    return found


def _mismatches() -> set[str]:
    functions, types, members = _rust_index()
    missing: set[str] = set()
    for qualified, value in _python_models_surface().items():
        name = qualified.rsplit(".", 1)[1]
        if not inspect.isclass(value):
            if name not in functions:
                missing.add(qualified)
            continue
        if name not in types:
            missing.add(qualified)
        rust_members = members.get(name, set())
        for member in vars(value):
            if member.startswith("_") or member in WIRE_MEMBERS or member in PY_RULE_MEMBERS:
                continue
            if member.startswith("to_") and member.endswith(("_dataframe", "_series")):
                continue
            rust = member.rstrip("_")
            if rust not in rust_members and rust.lower() not in rust_members:
                missing.add(f"{qualified}.{member}")
    return missing


def test_models_host_names_are_rust_names() -> None:
    """Every models host name is a Rust name or is recorded with a reason."""
    missing = _mismatches()
    unrecorded = sorted(missing - set(RECORDED))
    stale = sorted(set(RECORDED) - missing)
    reasonless = sorted(name for name, reason in RECORDED.items() if not reason.strip())
    assert (unrecorded, stale, reasonless) == ([], [], []), (
        "Host names in finstack_quant.models diverged from Rust.\n"
        f"  not a Rust name and not in [parity.rust_names]: {unrecorded}\n"
        f"  recorded in [parity.rust_names] but now a Rust name (remove): {stale}\n"
        f"  recorded without a reason: {reasonless}"
    )
