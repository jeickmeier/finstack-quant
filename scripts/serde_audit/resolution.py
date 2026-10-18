"""Rust module, import, alias, and capability resolution."""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Sequence
from dataclasses import replace
from pathlib import Path
import re
import tomllib

from .lexer import (
    _expand_use_tree,
    _item_enabled,
    _manual_impls,
    _marker_is_contract,
    _module_at,
    _preceding_attributes,
    _SourceUnit,
)
from .models import AuditConfigurationError, PublicType

_TRAILING_GENERICS = re.compile(r"<.*>$")
_PUBLIC_USE = re.compile(r"\bpub\s+use\s+(?P<body>[^;]+);", flags=re.DOTALL)
_ANY_USE = re.compile(r"\b(?:pub\s+)?use\s+(?P<body>[^;]+);", flags=re.DOTALL)
_MODULE_DECLARATION = re.compile(r"\b(?P<vis>pub\s+)?mod\s+(?P<name>[A-Za-z_]\w*)\s*;")


def _reference_candidates(reference: str, module_path: str) -> tuple[str, ...]:
    reference = reference.strip().lstrip(":")
    if reference.startswith("crate::"):
        return (reference.removeprefix("crate::"),)
    if reference.startswith("self::"):
        return ("::".join(filter(None, (module_path, reference.removeprefix("self::")))),)
    if reference.startswith("super::"):
        parent_parts = module_path.split("::") if module_path else []
        while reference.startswith("super::"):
            reference = reference.removeprefix("super::")
            if parent_parts:
                parent_parts.pop()
        return ("::".join((*parent_parts, reference)),)
    current = "::".join(filter(None, (module_path, reference)))
    return tuple(dict.fromkeys((reference, current)))


def _resolve_symbol(
    reference: str,
    *,
    module_path: str,
    symbols: dict[str, PublicType],
    imports: dict[tuple[str, str], str],
) -> str | None:
    reference = _TRAILING_GENERICS.sub("", reference.strip())
    if "::" not in reference:
        imported = imports.get((module_path, reference))
        if imported is not None:
            reference = imported
    for candidate in _reference_candidates(reference, module_path):
        if candidate in symbols:
            return candidate
    target_name = reference.rsplit("::", 1)[-1]
    matches = [path for path in symbols if path.rsplit("::", 1)[-1] == target_name]
    if len(matches) == 1:
        return matches[0]
    return None


def _unit_uses(unit: _SourceUnit, *, public_only: bool) -> tuple[tuple[str, str, int], ...]:
    pattern = _PUBLIC_USE if public_only else _ANY_USE
    uses = []
    for match in pattern.finditer(unit.text):
        attributes = _preceding_attributes(
            unit.text,
            match.start(),
            attribute_source=unit.source,
        )
        if not _item_enabled(attributes, unit.default_features):
            continue
        module_path = _module_at(unit, match.start())
        for expanded in _expand_use_tree(match.group("body")):
            uses.append((module_path, expanded, match.start()))
    return tuple(uses)


def _module_edges(units: Sequence[_SourceUnit]) -> tuple[tuple[str, str, bool], ...]:
    edges: list[tuple[str, str, bool]] = []
    for unit in units:
        edges.extend((span.parent, span.path, span.public) for span in unit.spans)
        for match in _MODULE_DECLARATION.finditer(unit.text):
            attributes = _preceding_attributes(
                unit.text,
                match.start(),
                attribute_source=unit.source,
            )
            if not _item_enabled(attributes, unit.default_features):
                continue
            parent = _module_at(unit, match.start())
            child = "::".join(filter(None, (parent, match.group("name"))))
            edges.append((parent, child, bool(match.group("vis"))))
    return tuple(edges)


def _reachable_modules(units: Sequence[_SourceUnit]) -> set[str]:
    reachable = {""}
    edges = _module_edges(units)
    changed = True
    while changed:
        changed = False
        for parent, child, public in edges:
            if public and parent in reachable and child not in reachable:
                reachable.add(child)
                changed = True
    return reachable


def _default_enabled_modules(units: Sequence[_SourceUnit]) -> set[str]:
    enabled = {""}
    edges = _module_edges(units)
    changed = True
    while changed:
        changed = False
        for parent, child, _ in edges:
            if parent in enabled and child not in enabled:
                enabled.add(child)
                changed = True
    return enabled


def _imports(units: Sequence[_SourceUnit]) -> dict[tuple[str, str], str]:
    imports: dict[tuple[str, str], str] = {}
    for unit in units:
        for module_path, expanded, _ in _unit_uses(unit, public_only=False):
            reference, _, alias = expanded.partition("@AS@")
            local_name = alias or reference.rsplit("::", 1)[-1]
            candidates = _reference_candidates(reference, module_path)
            imports[(module_path, local_name)] = candidates[0]
    return imports


def _apply_manual_impls(
    declarations: Sequence[PublicType],
    units: Sequence[_SourceUnit],
    enabled_modules: set[str],
    default_features: frozenset[str] = frozenset(),
) -> tuple[PublicType, ...]:
    symbols = {
        "::".join(filter(None, (item.module_path, item.name))): item for item in declarations if item.kind != "alias"
    }
    imports = _imports(units)
    additions: dict[str, set[str]] = defaultdict(set)
    for unit in units:
        if unit.base_module not in enabled_modules:
            continue
        for trait, target, position in _manual_impls(
            unit.text,
            attribute_source=unit.source,
            default_features=default_features,
        ):
            module_path = _module_at(unit, position)
            resolved = _resolve_symbol(target, module_path=module_path, symbols=symbols, imports=imports)
            if resolved is not None:
                additions[resolved].add(trait)
    updated = []
    for item in declarations:
        symbol = "::".join(filter(None, (item.module_path, item.name)))
        capabilities = frozenset(set(item.capabilities) | additions.get(symbol, set()))
        with_capabilities = replace(item, capabilities=capabilities)
        updated.append(
            replace(
                with_capabilities,
                has_marker=_marker_is_contract(with_capabilities, capabilities),
            )
        )
    return tuple(updated)


def _resolve_aliases(
    declarations: Sequence[PublicType],
    units: Sequence[_SourceUnit],
) -> tuple[PublicType, ...]:
    resolved = list(declarations)
    imports = _imports(units)
    for _ in range(len(resolved) + 1):
        changed = False
        symbols = {"::".join(filter(None, (item.module_path, item.name))): item for item in resolved}
        updated = []
        for item in resolved:
            if item.kind != "alias" or not item.target_name or item.capabilities:
                updated.append(item)
                continue
            target = _resolve_symbol(
                item.target_name,
                module_path=item.module_path,
                symbols=symbols,
                imports=imports,
            )
            if target is None:
                imported = imports.get((item.module_path, item.target_name))
                updated.append(replace(item, target_name=imported) if imported is not None else item)
                continue
            target_item = symbols[target]
            updated.append(
                replace(
                    item,
                    capabilities=target_item.capabilities,
                    target_name=target,
                    has_marker=item.has_marker or target_item.has_marker,
                )
            )
            changed = True
        resolved = updated
        if not changed:
            break
    return tuple(resolved)


def _module_exports(
    declarations: Sequence[PublicType],
    units: Sequence[_SourceUnit],
) -> dict[str, dict[str, str]]:
    symbols = {"::".join(filter(None, (item.module_path, item.name))): item for item in declarations}
    imports = _imports(units)
    exports: dict[str, dict[str, str]] = defaultdict(dict)
    for symbol, item in symbols.items():
        exports[item.module_path][item.name] = symbol
    uses = [use for unit in units for use in _unit_uses(unit, public_only=True)]
    edges = _module_edges(units)
    module_paths = {
        "",
        *(item.module_path for item in declarations),
        *(parent for parent, _, _ in edges),
        *(child for _, child, _ in edges),
    }
    for _ in range(len(uses) + len(module_paths) + 1):
        changed = False
        for module_path, expanded, _ in uses:
            reference, _, alias = expanded.partition("@AS@")
            if reference.endswith("::*"):
                target_reference = reference.removesuffix("::*")
                target_module = next(
                    (
                        candidate
                        for candidate in _reference_candidates(
                            target_reference,
                            module_path,
                        )
                        if candidate in module_paths
                    ),
                    None,
                )
                if target_module is None:
                    continue
                for exported_name, target in exports[target_module].items():
                    if exports[module_path].get(exported_name) != target:
                        exports[module_path][exported_name] = target
                        changed = True
                continue
            target = _resolve_symbol(
                reference,
                module_path=module_path,
                symbols=symbols,
                imports=imports,
            )
            if target is None and "::" in reference:
                target_module_ref, target_name = reference.rsplit("::", 1)
                for candidate in _reference_candidates(
                    target_module_ref,
                    module_path,
                ):
                    if target_name in exports[candidate]:
                        target = exports[candidate][target_name]
                        break
            if target is None:
                continue
            exported_name = alias or reference.rsplit("::", 1)[-1]
            if exports[module_path].get(exported_name) != target:
                exports[module_path][exported_name] = target
                changed = True
        if not changed:
            break
    return exports


def _cargo_manifest(crate_root: Path) -> dict[str, object]:
    with (crate_root / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)


def _cargo_lib_name(manifest: dict[str, object]) -> str:
    package = manifest.get("package")
    if not isinstance(package, dict) or not isinstance(package.get("name"), str):
        raise AuditConfigurationError("Cargo manifest is missing package.name")
    library = manifest.get("lib")
    if isinstance(library, dict) and isinstance(library.get("name"), str):
        return library["name"]
    return package["name"].replace("-", "_")


def _default_cargo_features(manifest: dict[str, object]) -> frozenset[str]:
    features = manifest.get("features", {})
    if not isinstance(features, dict):
        return frozenset()
    pending = list(features.get("default", ()))
    enabled: set[str] = set()
    while pending:
        feature = pending.pop()
        if not isinstance(feature, str) or feature.startswith("dep:") or "/" in feature or feature in enabled:
            continue
        enabled.add(feature)
        pending.extend(features.get(feature, ()))
    return frozenset(enabled)
