"""Conservative Rust lexical analysis and declaration parsing."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass, replace
from pathlib import Path
import re

from . import registries
from .models import (
    CAPABILITIES,
    CURRENT_TARGET_ARCH,
    CURRENT_TARGET_IS_UNIX,
    CURRENT_TARGET_OS,
    PublicType,
)


@dataclass(frozen=True)
class _ModuleSpan:
    path: str
    parent: str
    public: bool
    start: int
    end: int


@dataclass(frozen=True)
class _SourceUnit:
    path: Path
    crate_path: str
    source: str
    text: str
    base_module: str
    spans: tuple[_ModuleSpan, ...]
    default_features: frozenset[str]


def _blank(chars: list[str], start: int, end: int) -> None:
    for index in range(start, min(end, len(chars))):
        if chars[index] != "\n":
            chars[index] = " "


def _mask_non_code(source: str) -> str:
    """Mask comments and Rust literals while preserving byte offsets.

    This is intentionally a conservative lexer, not a Rust parser. It handles
    nested block comments, byte/ordinary strings, arbitrary-hash raw strings,
    character literals, and lifetimes. Macro-expanded declarations are outside
    scope; known derives that generate serde/schema impls are listed explicitly.
    """
    chars = list(source)
    index = 0
    block_depth = 0
    state = "code"
    delimiter = ""
    while index < len(chars):
        current = chars[index]
        following = chars[index + 1] if index + 1 < len(chars) else ""
        if state == "code":
            raw_match = re.match(r'(?:br|r)(?P<hashes>#{0,255})"', source[index:])
            if raw_match and (index == 0 or not (source[index - 1].isalnum() or source[index - 1] == "_")):
                hashes = raw_match.group("hashes")
                close = f'"{hashes}'
                end = source.find(close, index + raw_match.end())
                end = len(source) if end < 0 else end + len(close)
                _blank(chars, index, end)
                index = end
                continue
            if current == "/" and following == "/":
                chars[index] = chars[index + 1] = " "
                index += 2
                state = "line-comment"
                continue
            if current == "/" and following == "*":
                chars[index] = chars[index + 1] = " "
                index += 2
                block_depth = 1
                state = "block-comment"
                continue
            if current == '"' or (current == "b" and following == '"'):
                if current == "b":
                    chars[index] = " "
                    index += 1
                delimiter = '"'
                index += 1
                state = "literal"
                continue
            if current == "'" and re.match(r"'(?:\\.|[^\\'\n])'", source[index : index + 4]):
                delimiter = "'"
                index += 1
                state = "literal"
                continue
        elif state == "line-comment":
            if current == "\n":
                state = "code"
            else:
                chars[index] = " "
        elif state == "block-comment":
            if current == "/" and following == "*":
                chars[index] = chars[index + 1] = " "
                index += 2
                block_depth += 1
                continue
            if current == "*" and following == "/":
                chars[index] = chars[index + 1] = " "
                index += 2
                block_depth -= 1
                if block_depth == 0:
                    state = "code"
                continue
            if current != "\n":
                chars[index] = " "
        elif state == "literal":
            if current == "\\" and following:
                _blank(chars, index, index + 2)
                index += 2
                continue
            if current == delimiter:
                state = "code"
            elif current != "\n":
                chars[index] = " "
        index += 1
    return "".join(chars)


def _matching_open(text: str, close_index: int, opener: str, closer: str) -> int | None:
    depth = 0
    for index in range(close_index, -1, -1):
        if text[index] == closer:
            depth += 1
        elif text[index] == opener:
            depth -= 1
            if depth == 0:
                return index
    return None


def _matching_close(text: str, open_index: int, opener: str = "{", closer: str = "}") -> int | None:
    depth = 0
    for index in range(open_index, len(text)):
        if text[index] == opener:
            depth += 1
        elif text[index] == closer:
            depth -= 1
            if depth == 0:
                return index
    return None


def _preceding_attributes(
    text: str,
    declaration_start: int,
    *,
    attribute_source: str | None = None,
) -> tuple[str, ...]:
    ranges: list[tuple[int, int]] = []
    cursor = declaration_start - 1
    while cursor >= 0:
        while cursor >= 0 and text[cursor].isspace():
            cursor -= 1
        if cursor < 0 or text[cursor] != "]":
            break
        open_index = _matching_open(text, cursor, "[", "]")
        if open_index is None:
            break
        hash_index = open_index - 1
        while hash_index >= 0 and text[hash_index].isspace():
            hash_index -= 1
        if hash_index < 0 or text[hash_index] != "#":
            break
        ranges.append((hash_index, cursor + 1))
        cursor = hash_index - 1
    ranges.reverse()
    source = text if attribute_source is None else attribute_source
    return tuple(source[start:end] for start, end in ranges)


def _derived_capabilities(
    attributes: Iterable[str],
    default_features: frozenset[str] = frozenset(),
) -> set[str]:
    capabilities: set[str] = set()
    for attribute in attributes:
        derive_source = attribute
        cfg_attr = re.search(r"\bcfg_attr\s*\((.*)\)\s*\]", attribute, flags=re.DOTALL)
        if cfg_attr is not None:
            parts = _split_top_level(cfg_attr.group(1))
            if len(parts) < 2 or not _cfg_condition_enabled(
                parts[0],
                default_features,
            ):
                continue
            derive_source = ",".join(parts[1:])
        for derive in re.findall(r"\bderive\s*\((.*?)\)", derive_source, flags=re.DOTALL):
            for item in derive.split(","):
                trait = item.strip().split("::")[-1]
                if trait in CAPABILITIES:
                    capabilities.add(trait)
    return capabilities


def _manual_impls(
    text: str,
    *,
    attribute_source: str | None = None,
    default_features: frozenset[str] = frozenset(),
) -> tuple[tuple[str, str, int], ...]:
    pattern = re.compile(
        r"\bimpl(?:\s*<[^{};]*>)?\s+"
        r"(?P<trait>(?:(?:::)?[A-Za-z_]\w*::)*(?:Serialize|Deserialize|JsonSchema))"
        r"(?:\s*<[^{};]*>)?\s+for\s+"
        r"(?P<target>(?:(?:::)?[A-Za-z_]\w*::)*[A-Za-z_]\w*)",
        flags=re.DOTALL,
    )
    implementations = []
    for match in pattern.finditer(text):
        attributes = _preceding_attributes(
            text,
            match.start(),
            attribute_source=attribute_source,
        )
        if _item_enabled(attributes, default_features):
            implementations.append((match.group("trait").split("::")[-1], match.group("target"), match.start()))
    return tuple(implementations)


def _cfg_condition_enabled(
    condition: str,
    default_features: frozenset[str] = frozenset(),
) -> bool:
    condition = re.sub(r"\s+", "", condition)
    host_predicates = {
        "test": False,
        "unix": CURRENT_TARGET_IS_UNIX,
        "windows": not CURRENT_TARGET_IS_UNIX,
    }
    if condition in host_predicates:
        return host_predicates[condition]
    selector = re.fullmatch(r'(feature|target_arch|target_os)="([^"]+)"', condition)
    if selector is not None:
        name, value = selector.groups()
        selectors = {
            "feature": default_features,
            "target_arch": frozenset({CURRENT_TARGET_ARCH}),
            "target_os": frozenset({CURRENT_TARGET_OS}),
        }
        return value in selectors[name]
    for operator, reducer in (
        ("not", lambda values: not values[0]),
        ("all", all),
        ("any", any),
    ):
        prefix = f"{operator}("
        if condition.startswith(prefix) and condition.endswith(")"):
            values = [
                _cfg_condition_enabled(part, default_features) for part in _split_top_level(condition[len(prefix) : -1])
            ]
            return bool(values) and reducer(values)
    return False


def _item_enabled(
    attributes: Iterable[str],
    default_features: frozenset[str] = frozenset(),
) -> bool:
    for attribute in attributes:
        cfg = re.search(r"\bcfg\s*\((.*)\)\s*\]", attribute, flags=re.DOTALL)
        if cfg is not None and not _cfg_condition_enabled(
            cfg.group(1),
            default_features,
        ):
            return False
    return True


def _mask_disabled_inline_modules(
    text: str,
    source: str,
    default_features: frozenset[str],
) -> str:
    chars = list(text)
    pattern = re.compile(r"\b(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{")
    for match in tuple(pattern.finditer(text)):
        attributes = _preceding_attributes(
            text,
            match.start(),
            attribute_source=source,
        )
        if _item_enabled(attributes, default_features):
            continue
        open_index = text.find("{", match.start(), match.end())
        close_index = _matching_close(text, open_index)
        if close_index is None:
            continue
        for index in range(match.start(), close_index + 1):
            if chars[index] != "\n":
                chars[index] = " "
    return "".join(chars)


def _module_for_file(crate_root: Path, path: Path) -> str:
    relative = path.relative_to(crate_root / "src")
    if relative.name == "lib.rs":
        return ""
    parts = relative.parent.parts if relative.name == "mod.rs" else (*relative.parent.parts, relative.stem)
    return "::".join(parts)


def _module_at(unit: _SourceUnit, position: int) -> str:
    containing = [span for span in unit.spans if span.start < position < span.end]
    return max(containing, key=lambda span: span.start).path if containing else unit.base_module


def _inline_module_spans(text: str, base_module: str) -> tuple[_ModuleSpan, ...]:
    spans: list[_ModuleSpan] = []
    pattern = re.compile(r"\b(?P<vis>pub\s+)?mod\s+(?P<name>[A-Za-z_]\w*)\s*\{")
    for match in pattern.finditer(text):
        parent = base_module
        containing = [span for span in spans if span.start < match.start() < span.end]
        if containing:
            parent = max(containing, key=lambda span: span.start).path
        open_index = text.find("{", match.start(), match.end())
        close_index = _matching_close(text, open_index)
        if close_index is None:
            continue
        path = "::".join(filter(None, (parent, match.group("name"))))
        spans.append(
            _ModuleSpan(
                path=path,
                parent=parent,
                public=bool(match.group("vis")),
                start=open_index,
                end=close_index,
            )
        )
    return tuple(spans)


def crate_directory(source_root: Path, crate: str) -> Path:
    """Return the crate directory (relative to ``source_root``) for an audit crate key.

    A nested crate such as ``valuations-core`` lives at
    ``finstack-quant/valuations/core``; every other key is a direct child of
    ``finstack-quant/``.
    """
    direct = Path("finstack-quant") / crate
    if (source_root / direct).is_dir():
        return direct
    parent, _, child = crate.rpartition("-")
    return Path("finstack-quant") / parent / child


def _crate_relative(path: Path, source_root: Path, crate: str) -> tuple[Path, str]:
    relative_path = path.relative_to(source_root)
    crate_prefix = crate_directory(source_root, crate)
    try:
        crate_path = relative_path.relative_to(crate_prefix).as_posix()
    except ValueError:
        crate_path = path.relative_to(path.parents[1]).as_posix()
    return relative_path, crate_path


def _marker_metadata(body: str) -> tuple[frozenset[str], frozenset[str]]:
    names: set[str] = set()
    defaults: set[str] = set()
    field_pattern = re.compile(
        r"(?:(?:pub(?:\([^)]*\))?)\s+)?(?P<name>schema_version|schema|version)\s*:",
        flags=re.IGNORECASE,
    )
    for field in field_pattern.finditer(body):
        marker_name = field.group("name").lower()
        names.add(marker_name)
        field_attributes = _preceding_attributes(body, field.start())
        has_default = any("serde" in item and re.search(r"\bdefault\b", item) for item in field_attributes)
        if has_default:
            defaults.add(marker_name)
    return frozenset(names), frozenset(defaults)


def _marker_is_contract(
    declaration: PublicType,
    capabilities: frozenset[str] | None = None,
) -> bool:
    if (declaration.crate, declaration.crate_path, declaration.name) in registries.MAINTAINED_CONTRACTS:
        return True
    effective = declaration.capabilities if capabilities is None else capabilities
    recognized = declaration.marker_names & {"schema", "schema_version"}
    return bool(recognized and effective & {"Serialize", "Deserialize"})


def _parse_unit_declarations(
    unit: _SourceUnit,
    *,
    crate: str,
    source_root: Path,
    default_features: frozenset[str] = frozenset(),
    lib_name: str = "",
) -> list[PublicType]:
    declarations: list[PublicType] = []
    pattern = re.compile(r"\bpub\s+(?P<kind>struct|enum|type)\s+(?P<name>[A-Za-z_]\w*)")
    relative_path, crate_path = _crate_relative(unit.path, source_root, crate)
    for match in pattern.finditer(unit.text):
        attributes = _preceding_attributes(
            unit.text,
            match.start(),
            attribute_source=unit.source,
        )
        if not _item_enabled(attributes, default_features):
            continue
        kind = match.group("kind")
        name = match.group("name")
        body = ""
        target_name = None
        if kind == "type":
            semicolon = unit.text.find(";", match.end())
            equals = unit.text.find("=", match.end(), semicolon)
            if equals >= 0 and semicolon >= 0:
                target_name = unit.text[equals + 1 : semicolon].strip()
        else:
            boundary_match = re.search(r"[{;]", unit.text[match.end() :])
            if boundary_match is not None:
                boundary = match.end() + boundary_match.start()
                if unit.text[boundary] == "{":
                    close_index = _matching_close(unit.text, boundary)
                    if close_index is not None:
                        body = unit.text[boundary + 1 : close_index]
        marker_names, marker_defaults = _marker_metadata(body)
        capabilities = frozenset(_derived_capabilities(attributes, default_features))
        declarations.append(
            PublicType(
                crate=crate,
                path=relative_path,
                crate_path=crate_path,
                line=unit.source.count("\n", 0, match.start()) + 1,
                name=name,
                kind="alias" if kind == "type" else kind,
                capabilities=capabilities,
                has_marker=False,
                module_path=_module_at(unit, match.start()),
                target_name=target_name,
                marker_names=marker_names,
                marker_defaults=marker_defaults,
                lib_name=lib_name,
            )
        )
        declarations[-1] = replace(
            declarations[-1],
            has_marker=_marker_is_contract(declarations[-1], capabilities),
        )
    return declarations


def _split_top_level(value: str) -> list[str]:
    parts: list[str] = []
    stack: list[str] = []
    closers = {"(": ")", "[": "]", "{": "}"}
    start = 0
    for index, character in enumerate(value):
        if character in closers:
            stack.append(closers[character])
        elif stack and character == stack[-1]:
            stack.pop()
        elif character == "," and not stack:
            parts.append(value[start:index].strip())
            start = index + 1
    parts.append(value[start:].strip())
    return [part for part in parts if part]


def _expand_use_tree(value: str) -> tuple[str, ...]:
    value = re.sub(r"(?<!\s)\s+as\s+", "@AS@", value)
    value = re.sub(r"\s+", "", value)
    open_index = value.find("{")
    if open_index < 0:
        return (value,)
    close_index = _matching_close(value, open_index, "{", "}")
    if close_index is None:
        return (value,)
    prefix = value[:open_index]
    suffix = value[close_index + 1 :]
    expanded = []
    for part in _split_top_level(value[open_index + 1 : close_index]):
        expanded.extend(_expand_use_tree(f"{prefix}{part}{suffix}"))
    return tuple(expanded)
