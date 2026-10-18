"""Data models and shared constants for the serde audit."""

from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
import platform

CAPABILITIES = ("Serialize", "Deserialize", "JsonSchema")
CONTRACT_SUFFIXES = ("Spec", "Envelope", "Result")
NON_PRODUCT_CRATES = frozenset({"test-utils", "valuations-macros"})
CURRENT_TARGET_IS_UNIX = os.name == "posix"
CURRENT_TARGET_ARCH = {
    "arm64": "aarch64",
    "amd64": "x86_64",
}.get(platform.machine().lower(), platform.machine().lower())
CURRENT_TARGET_OS = {
    "darwin": "macos",
}.get(platform.system().lower(), platform.system().lower())


@dataclass(frozen=True)
class PublicType:
    """A mechanically discovered public Rust struct or enum."""

    crate: str
    path: Path
    crate_path: str
    line: int
    name: str
    kind: str
    capabilities: frozenset[str]
    has_marker: bool
    module_path: str = ""
    target_name: str | None = None
    marker_names: frozenset[str] = frozenset()
    marker_defaults: frozenset[str] = frozenset()
    export_paths: frozenset[str] = frozenset()
    lib_name: str = ""

    @property
    def identity(self) -> tuple[str, str, str, str]:
        """Return the module-qualified declaration identity."""
        return self.crate, self.crate_path, self.module_path, self.name

    @property
    def is_contract_like(self) -> bool:
        """Return whether naming or an explicit marker puts the type in scope."""
        return self.has_marker or self.name.endswith(CONTRACT_SUFFIXES)


@dataclass(frozen=True)
class ExceptionEntry:
    """A narrowly reviewed exception to contract capability requirements."""

    crate: str
    path: str
    type_name: str
    category: str
    rationale: str
    allowed_missing: frozenset[str]
    module_path: str | None = None

    @property
    def resolved_module_path(self) -> str:
        """Return explicit or file-derived module path."""
        if self.module_path is not None:
            return self.module_path
        path = Path(self.path)
        relative = path.relative_to("src")
        if relative.name == "lib.rs":
            return ""
        parts = relative.parent.parts if relative.name == "mod.rs" else (*relative.parent.parts, relative.stem)
        return "::".join(parts)

    @property
    def identity(self) -> tuple[str, str, str, str]:
        """Return the stable declaration identity."""
        return self.crate, self.path, self.resolved_module_path, self.type_name


@dataclass(frozen=True)
class Diagnostic:
    """A contract declaration missing one or more required capabilities."""

    crate: str
    path: Path
    line: int
    type_name: str
    missing: tuple[str, ...]


@dataclass(frozen=True)
class StaleException:
    """A reviewed classification that no longer exactly matches source."""

    entry: ExceptionEntry
    reason: str
    actual_missing: frozenset[str] | None

    @property
    def allowed_missing(self) -> frozenset[str]:
        """Return the capability gap recorded by the classification."""
        return self.entry.allowed_missing


@dataclass(frozen=True)
class CrateSummary:
    """Deterministic audit counts for one product crate."""

    crate: str
    public_types: int
    contract_types: int
    reviewed_exceptions: int
    failures: int


@dataclass(frozen=True)
class AuditReport:
    """Complete result of a workspace audit."""

    declarations: tuple[PublicType, ...]
    diagnostics: tuple[Diagnostic, ...]
    reviewed_exceptions: tuple[ExceptionEntry, ...]
    stale_exceptions: tuple[StaleException, ...]
    summaries: tuple[CrateSummary, ...]
    contract_identities: frozenset[tuple[str, str, str, str]]

    @property
    def failed(self) -> bool:
        """Return whether check mode must fail."""
        return bool(self.diagnostics or self.stale_exceptions)


class AuditConfigurationError(ValueError):
    """Raised when the requested repository root cannot be audited."""
