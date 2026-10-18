"""Crate scanning and workspace audit orchestration."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import replace
from pathlib import Path

from . import registries
from .lexer import (
    crate_directory,
    _crate_relative,
    _inline_module_spans,
    _mask_disabled_inline_modules,
    _mask_non_code,
    _module_for_file,
    _parse_unit_declarations,
    _SourceUnit,
)
from .models import (
    CAPABILITIES,
    NON_PRODUCT_CRATES,
    AuditConfigurationError,
    AuditReport,
    CrateSummary,
    Diagnostic,
    ExceptionEntry,
    PublicType,
    StaleException,
)
from .resolution import (
    _apply_manual_impls,
    _cargo_lib_name,
    _cargo_manifest,
    _default_cargo_features,
    _default_enabled_modules,
    _module_exports,
    _reachable_modules,
    _resolve_aliases,
)


def scan_crate(crate_root: Path, *, crate: str, source_root: Path) -> tuple[PublicType, ...]:
    """Scan one crate with conservative module visibility and symbol resolution."""
    manifest = _cargo_manifest(crate_root)
    default_features = _default_cargo_features(manifest)
    lib_name = _cargo_lib_name(manifest)
    units = []
    for path in _rust_source_files(crate_root):
        source = path.read_text(encoding="utf-8")
        text = _mask_disabled_inline_modules(
            _mask_non_code(source),
            source,
            default_features,
        )
        base_module = _module_for_file(crate_root, path)
        _, crate_path = _crate_relative(path, source_root, crate)
        units.append(
            _SourceUnit(
                path=path,
                crate_path=crate_path,
                source=source,
                text=text,
                base_module=base_module,
                spans=_inline_module_spans(text, base_module),
                default_features=default_features,
            )
        )
    enabled_modules = _default_enabled_modules(units)
    declarations = [
        item
        for unit in units
        for item in _parse_unit_declarations(
            unit,
            crate=crate,
            source_root=source_root,
            default_features=default_features,
            lib_name=lib_name,
        )
    ]
    declarations = list(
        _apply_manual_impls(
            declarations,
            units,
            enabled_modules,
            default_features,
        )
    )
    declarations = list(_resolve_aliases(declarations, units))
    symbols = {"::".join(filter(None, (item.module_path, item.name))): item for item in declarations}
    reachable_modules = _reachable_modules(units)
    exports = _module_exports(declarations, units)
    visible_by_symbol: dict[str, PublicType] = {}
    synthetic_aliases: dict[tuple[str, str], PublicType] = {}
    for module_path in reachable_modules:
        for exported_name, target in exports[module_path].items():
            target_item = symbols[target]
            export_path = "::".join(filter(None, (module_path, exported_name)))
            if exported_name == target_item.name:
                previous = visible_by_symbol.get(target)
                export_paths = {export_path}
                if previous is not None:
                    export_paths.update(previous.export_paths)
                visible_by_symbol[target] = replace(
                    target_item,
                    export_paths=frozenset(export_paths),
                )
            else:
                synthetic_aliases[(module_path, exported_name)] = PublicType(
                    crate=crate,
                    path=target_item.path,
                    crate_path=target_item.crate_path,
                    line=target_item.line,
                    name=exported_name,
                    kind="alias",
                    capabilities=target_item.capabilities,
                    has_marker=target_item.has_marker,
                    module_path=module_path,
                    target_name=target,
                    export_paths=frozenset({export_path}),
                    lib_name=target_item.lib_name,
                )
    visible = [visible_by_symbol[symbol] for symbol in sorted(visible_by_symbol)]
    visible.extend(synthetic_aliases[key] for key in sorted(synthetic_aliases))
    return tuple(visible)


def scan_rust_file(path: Path, *, crate: str, source_root: Path) -> tuple[PublicType, ...]:
    """Scan a standalone fixture file without an external module graph."""
    source = path.read_text(encoding="utf-8")
    default_features = frozenset()
    text = _mask_disabled_inline_modules(
        _mask_non_code(source),
        source,
        default_features,
    )
    _, crate_path = _crate_relative(path, source_root, crate)
    unit = _SourceUnit(
        path=path,
        crate_path=crate_path,
        source=source,
        text=text,
        base_module="",
        spans=_inline_module_spans(text, ""),
        default_features=default_features,
    )
    declarations = _parse_unit_declarations(
        unit,
        crate=crate,
        source_root=source_root,
        lib_name=crate.replace("-", "_"),
    )
    declarations = _apply_manual_impls(declarations, (unit,), {""})
    return tuple(item for item in declarations if item.module_path == "")


def _product_crates(root: Path) -> tuple[tuple[str, Path], ...]:
    crates_root = root / "finstack-quant"
    crates = []
    if not crates_root.is_dir():
        raise AuditConfigurationError(
            f"audit root {root} does not contain the expected finstack-quant/ crate directory"
        )
    for candidate in sorted(crates_root.iterdir()):
        if not candidate.is_dir():
            continue
        if (
            candidate.name not in NON_PRODUCT_CRATES
            and (candidate / "Cargo.toml").is_file()
            and (candidate / "src").is_dir()
        ):
            crates.append((candidate.name, candidate))
        # Nested crates (`finstack-quant/valuations/core`) are keyed by the
        # package-name suffix: ``valuations-core``.
        for nested in sorted(candidate.iterdir()):
            name = f"{candidate.name}-{nested.name}"
            if (
                nested.is_dir()
                and name not in NON_PRODUCT_CRATES
                and (nested / "Cargo.toml").is_file()
                and (nested / "src").is_dir()
            ):
                crates.append((name, nested))
    return tuple(sorted(crates))


def _rust_source_files(crate_root: Path) -> tuple[Path, ...]:
    files = []
    for path in (crate_root / "src").rglob("*.rs"):
        relative_parts = path.relative_to(crate_root / "src").parts
        if any(part in {"tests", "examples", "benches"} for part in relative_parts):
            continue
        if path.name in {"test.rs", "tests.rs"} or path.stem.endswith("_tests"):
            continue
        files.append(path)
    return tuple(sorted(files))


def audit_workspace(
    root: Path,
    *,
    allowlist: Sequence[ExceptionEntry] = registries.REVIEWED_EXCEPTIONS,
) -> AuditReport:
    """Audit all product Rust crate source trees below ``root``."""
    root = root.resolve()
    if not root.is_dir():
        raise AuditConfigurationError(f"audit root does not exist or is not a directory: {root}")
    crates = _product_crates(root)
    if not crates:
        raise AuditConfigurationError(f"no product Rust crates found below {root / 'finstack-quant'}")
    declarations = tuple(
        declaration
        for crate, crate_root in crates
        for declaration in scan_crate(crate_root, crate=crate, source_root=root)
    )
    external_exports = {
        "::".join(
            filter(
                None,
                (
                    declaration.lib_name,
                    export_path,
                ),
            )
        ): declaration
        for declaration in declarations
        for export_path in declaration.export_paths
    }
    globally_resolved = []
    for declaration in declarations:
        if declaration.kind != "alias" or declaration.capabilities or not declaration.target_name:
            globally_resolved.append(declaration)
            continue
        target = external_exports.get(declaration.target_name.lstrip(":"))
        if target is not None:
            globally_resolved.append(
                replace(
                    declaration,
                    capabilities=target.capabilities,
                    has_marker=declaration.has_marker or target.has_marker,
                )
            )
        else:
            globally_resolved.append(declaration)
    declarations = tuple(globally_resolved)
    by_identity = {declaration.identity: declaration for declaration in declarations}
    exception_by_identity = {entry.identity: entry for entry in allowlist}
    if len(exception_by_identity) != len(allowlist):
        raise ValueError("serde audit exception identities must be unique")

    reviewed: list[ExceptionEntry] = []

    def qualified_registry_identity(
        identity: tuple[str, str, str],
    ) -> tuple[str, str, str, str]:
        crate, path, type_name = identity
        module_path = ExceptionEntry(
            crate=crate,
            path=path,
            type_name=type_name,
            category="registry",
            rationale="registry lookup",
            allowed_missing=frozenset(),
        ).resolved_module_path
        return crate, path, module_path, type_name

    one_way_identities = {
        *(entry.identity for entry in registries.ONE_WAY_EXCEPTIONS),
        *(qualified_registry_identity(identity) for identity in registries.MAINTAINED_ONE_WAY_OUTPUTS),
    }
    required_public_identities = {
        qualified_registry_identity(identity) for identity in registries.REQUIRED_PUBLIC_TYPES
    }

    def contract_like(declaration: PublicType) -> bool:
        return (
            declaration.is_contract_like
            or (
                declaration.crate,
                declaration.crate_path,
                declaration.module_path,
                declaration.name,
            )
            in one_way_identities | required_public_identities
        )

    stale: list[StaleException] = []
    for entry in sorted(allowlist, key=lambda item: item.identity):
        declaration = by_identity.get(entry.identity)
        if declaration is None:
            stale.append(
                StaleException(
                    entry=entry,
                    reason="declaration-missing",
                    actual_missing=None,
                )
            )
            continue
        actual_missing = frozenset(CAPABILITIES) - declaration.capabilities
        if not contract_like(declaration):
            stale.append(
                StaleException(
                    entry=entry,
                    reason="not-contract-like",
                    actual_missing=actual_missing,
                )
            )
            continue
        if not entry.allowed_missing or entry.allowed_missing != actual_missing:
            stale.append(
                StaleException(
                    entry=entry,
                    reason="capability-set-changed",
                    actual_missing=actual_missing,
                )
            )
            continue
        reviewed.append(entry)

    diagnostics: list[Diagnostic] = []
    audited_crates = {crate for crate, _ in crates}
    for crate, crate_path, type_name in sorted(registries.MAINTAINED_CONTRACTS):
        registry_identity = (crate, crate_path, type_name)
        identity = qualified_registry_identity(registry_identity)
        if crate in audited_crates and identity not in by_identity:
            diagnostics.append(
                Diagnostic(
                    crate=crate,
                    path=crate_directory(root, crate) / crate_path,
                    line=0,
                    type_name=type_name,
                    missing=tuple(
                        sorted(
                            registries.MAINTAINED_REQUIRED_CAPABILITIES.get(
                                registry_identity,
                                CAPABILITIES,
                            )
                        )
                    ),
                )
            )
    for registry_identity, required in sorted(registries.REQUIRED_PUBLIC_TYPES.items()):
        crate, crate_path, type_name = registry_identity
        identity = qualified_registry_identity(registry_identity)
        if crate in audited_crates and identity not in by_identity:
            diagnostics.append(
                Diagnostic(
                    crate=crate,
                    path=crate_directory(root, crate) / crate_path,
                    line=0,
                    type_name=type_name,
                    missing=tuple(sorted(required)),
                )
            )
    stale_entries = {item.entry for item in stale}
    for declaration in declarations:
        if not contract_like(declaration):
            continue
        maintained_identity = (
            declaration.crate,
            declaration.crate_path,
            declaration.name,
        )
        required = registries.MAINTAINED_REQUIRED_CAPABILITIES.get(
            maintained_identity,
            registries.REQUIRED_PUBLIC_TYPES.get(maintained_identity, CAPABILITIES),
        )
        missing = set(required) - declaration.capabilities
        exception = exception_by_identity.get(declaration.identity)
        if exception is not None and exception not in stale_entries:
            missing -= exception.allowed_missing
        if missing:
            diagnostics.append(
                Diagnostic(
                    crate=declaration.crate,
                    path=declaration.path,
                    line=declaration.line,
                    type_name=declaration.name,
                    missing=tuple(sorted(missing)),
                )
            )
    diagnostics.sort(key=lambda item: (item.crate, item.path.as_posix(), item.line, item.type_name))

    crate_names = sorted({crate for crate, _ in crates})
    summaries = []
    for crate in crate_names:
        crate_declarations = [item for item in declarations if item.crate == crate]
        summaries.append(
            CrateSummary(
                crate=crate,
                public_types=len(crate_declarations),
                contract_types=sum(contract_like(item) for item in crate_declarations),
                reviewed_exceptions=sum(item.crate == crate for item in reviewed),
                failures=(
                    sum(item.crate == crate for item in diagnostics) + sum(item.entry.crate == crate for item in stale)
                ),
            )
        )
    return AuditReport(
        declarations=tuple(
            sorted(declarations, key=lambda item: (item.crate, item.path.as_posix(), item.line, item.name))
        ),
        diagnostics=tuple(diagnostics),
        reviewed_exceptions=tuple(reviewed),
        stale_exceptions=tuple(stale),
        summaries=tuple(summaries),
        contract_identities=frozenset(item.identity for item in declarations if contract_like(item)),
    )
