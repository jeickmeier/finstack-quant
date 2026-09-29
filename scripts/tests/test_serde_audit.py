"""Tests for the Rust serde contract audit."""

from __future__ import annotations

from collections import Counter
from pathlib import Path
import re
import sys

import pytest

from scripts import serde_audit
from scripts.serde_audit import registries

_MODULE = serde_audit
_REGISTRIES = registries

_FIXTURES = Path(__file__).parent / "fixtures" / "serde_audit"
_REPOSITORY_ROOT = Path(__file__).parents[2]


def _write_crate(tmp_path: Path, source: str) -> Path:
    crate = tmp_path / "finstack-quant" / "sample"
    (crate / "src").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "finstack-quant-sample"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    (crate / "src" / "lib.rs").write_text(source, encoding="utf-8")
    return crate


def test_parser_handles_multiline_qualified_and_manual_traits() -> None:
    """Derives and manual impls provide effective capabilities."""
    declarations = _MODULE.scan_rust_file(
        _FIXTURES / "parser_cases.rs",
        crate="sample",
        source_root=_FIXTURES,
    )
    by_name = {declaration.name: declaration for declaration in declarations}

    assert by_name["MultilineSpec"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert by_name["QualifiedEnvelope"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert by_name["ManualResult"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})


def test_crate_scan_resolves_modules_reexports_aliases_and_cross_file_impls() -> None:
    """Only externally reachable declarations are gated and aliases inherit support."""
    declarations = _MODULE.scan_crate(
        _FIXTURES / "module_graph",
        crate="sample",
        source_root=_FIXTURES,
    )
    by_name = {declaration.name: declaration for declaration in declarations}

    assert "HiddenSpec" not in by_name
    assert "NeverExportedSpec" not in by_name
    assert "PhantomSpec" not in by_name
    assert "ReexportedResult" in by_name
    assert "CashflowResult" in by_name
    assert by_name["RenamedResult"].kind == "alias"
    assert "AliasOnlyResult" not in by_name
    assert by_name["PublicAliasResult"].kind == "alias"
    assert by_name["GlobEnvelope"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert by_name["RootAliasSpec"].kind == "alias"
    assert by_name["LocalAliasEnvelope"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert by_name["CrossFileResult"].capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert "DefaultFeatureEnvelope" in by_name
    assert "EnabledPublicResult" in by_name
    assert "EnabledInlineResult" in by_name
    assert "OptionalFeatureEnvelope" not in by_name
    assert "DisabledPublicResult" not in by_name
    assert "OptionalInlineResult" not in by_name
    assert "OptionalAliasResult" not in by_name
    assert "DisabledAliasResult" not in by_name


def test_qualified_impls_do_not_cross_contaminate_duplicate_names() -> None:
    """Qualified manual impls resolve to one nested declaration."""
    declarations = _MODULE.scan_crate(
        _FIXTURES / "module_graph",
        crate="sample",
        source_root=_FIXTURES,
    )
    duplicates = sorted(
        (declaration.module_path, declaration.capabilities)
        for declaration in declarations
        if declaration.name == "DuplicateResult"
    )

    assert duplicates == [
        ("public_api::duplicate_a", frozenset({"Serialize", "Deserialize", "JsonSchema"})),
        ("public_api::duplicate_b", frozenset({"Serialize", "Deserialize", "JsonSchema"})),
    ]
    inline_duplicates = sorted(
        (declaration.module_path, declaration.capabilities)
        for declaration in declarations
        if declaration.name == "InlineDuplicateResult"
    )
    assert inline_duplicates == [
        ("inline_a", frozenset({"Serialize"})),
        ("inline_b", frozenset({"Serialize", "Deserialize", "JsonSchema"})),
    ]
    assert len({item.identity for item in declarations if item.name == "InlineDuplicateResult"}) == 2


def test_exception_identity_includes_inline_module_path(tmp_path: Path) -> None:
    """Same-file duplicate names require a module-qualified classification."""
    _write_crate(
        tmp_path,
        """
pub mod first {
    #[derive(serde::Serialize)]
    pub struct DuplicateResult {
        pub value: String,
    }
}

pub mod second {
    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    pub struct DuplicateResult {
        pub value: String,
    }
}
""",
    )
    exception = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        module_path="first",
        type_name="DuplicateResult",
        category="runtime-result",
        rationale="First-module runtime output only.",
        allowed_missing=frozenset({"Deserialize", "JsonSchema"}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=(exception,))

    assert not report.failed
    assert report.reviewed_exceptions == (exception,)


def test_default_build_cfg_directionality() -> None:
    """Only capabilities enabled in the effective default build count."""
    declarations = _MODULE.scan_crate(
        _FIXTURES / "module_graph",
        crate="sample",
        source_root=_FIXTURES,
    )
    by_name = {declaration.name: declaration for declaration in declarations}

    assert by_name["FeatureOnlyResult"].capabilities == frozenset()
    assert by_name["DefaultDerivedResult"].capabilities == frozenset({"Serialize"})
    assert by_name["EnabledByDefaultResult"].capabilities == frozenset({"Deserialize"})
    assert by_name["NestedPredicateResult"].capabilities == frozenset({"Serialize"})
    assert by_name["CfgManualResult"].capabilities == frozenset()
    assert by_name["CfgExternalImplResult"].capabilities == frozenset()


def test_split_top_level_tracks_all_rust_delimiters() -> None:
    """Nested cfg predicates and derive arguments remain intact."""
    assert _MODULE._split_top_level(
        'all(feature = "enabled", any(unix, not(target_arch = "wasm32"))), '
        "derive(serde::Serialize, helper::Macro<[u8; 4], { 1 + 2 }>)"
    ) == [
        'all(feature = "enabled", any(unix, not(target_arch = "wasm32")))',
        "derive(serde::Serialize, helper::Macro<[u8; 4], { 1 + 2 }>)",
    ]


def test_target_predicates_follow_current_host_policy() -> None:
    """Unix, target_arch, and target_os are evaluated against the Python host."""
    assert _MODULE._cfg_condition_enabled("unix") is _MODULE.CURRENT_TARGET_IS_UNIX
    assert _MODULE._cfg_condition_enabled(f'target_arch = "{_MODULE.CURRENT_TARGET_ARCH}"')
    assert not _MODULE._cfg_condition_enabled('target_arch = "definitely_other"')
    assert _MODULE._cfg_condition_enabled(f'target_os = "{_MODULE.CURRENT_TARGET_OS}"')
    assert not _MODULE._cfg_condition_enabled('target_os = "definitely_other"')


def test_portfolio_margin_manual_impl_resolves_across_files() -> None:
    """The production cross-file wire impls provide effective capabilities."""
    declarations = _MODULE.scan_crate(
        _REPOSITORY_ROOT / "finstack-quant" / "portfolio",
        crate="portfolio",
        source_root=_REPOSITORY_ROOT,
    )
    declaration = next(item for item in declarations if item.name == "PortfolioMarginResult")

    assert declaration.capabilities == frozenset({"Serialize", "Deserialize"})


def test_production_recursive_glob_discovers_cashflow_envelope() -> None:
    """The valuations glob bridge exposes the registered binding output."""
    declarations = _MODULE.scan_crate(
        _REPOSITORY_ROOT / "finstack-quant" / "valuations",
        crate="valuations",
        source_root=_REPOSITORY_ROOT,
    )
    declaration = next(item for item in declarations if item.name == "InstrumentCashflowEnvelope")

    assert declaration.capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert "instruments::cashflow_export::InstrumentCashflowEnvelope" in (declaration.export_paths)
    identity = (
        "valuations",
        "src/instruments/common_impl/cashflow_export.rs",
        "InstrumentCashflowEnvelope",
    )
    assert identity in _MODULE.REQUIRED_PUBLIC_TYPES
    assert identity not in _MODULE.MAINTAINED_CONTRACTS


def test_parser_respects_serde_directionality_and_test_exclusions() -> None:
    """Serialize-only declarations stay one-way and cfg(test) modules are skipped."""
    declarations = _MODULE.scan_rust_file(
        _FIXTURES / "parser_cases.rs",
        crate="sample",
        source_root=_FIXTURES,
    )
    by_name = {declaration.name: declaration for declaration in declarations}

    assert by_name["SerializeOnlyResult"].capabilities == frozenset({"Serialize"})
    assert "HiddenTestSpec" not in by_name


def test_marker_detection_gates_type_without_contract_suffix() -> None:
    """An explicit schema marker makes an otherwise ordinary name contract-like."""
    declarations = _MODULE.scan_rust_file(
        _FIXTURES / "parser_cases.rs",
        crate="sample",
        source_root=_FIXTURES,
    )
    marker_type = next(declaration for declaration in declarations if declaration.name == "MarkerOnlyType")

    assert marker_type.has_marker
    assert marker_type.is_contract_like


def test_marker_detection_ignores_domain_versions_and_handles_raw_strings() -> None:
    """Only contract-semantic marker fields gate ordinary type names."""
    declarations = _MODULE.scan_crate(
        _FIXTURES / "module_graph",
        crate="sample",
        source_root=_FIXTURES,
    )
    by_name = {declaration.name: declaration for declaration in declarations}

    assert not by_name["DomainVersion"].has_marker
    assert by_name["VersionedDocument"].has_marker
    assert by_name["SerdeVersionedDocument"].has_marker
    assert by_name["ManualMarker"].has_marker
    assert not by_name["DomainDefaultVersion"].has_marker
    assert "PhantomSpec" not in by_name


def test_new_unapproved_spec_reports_missing_capabilities(tmp_path: Path) -> None:
    """A new public Spec fails closed until all contract traits are supported."""
    _write_crate(
        tmp_path,
        """
#[derive(Debug)]
pub struct UnapprovedSpec {
    pub value: String,
}
""",
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=())

    assert report.failed
    assert len(report.diagnostics) == 1
    diagnostic = report.diagnostics[0]
    assert diagnostic.type_name == "UnapprovedSpec"
    assert diagnostic.missing == ("Deserialize", "JsonSchema", "Serialize")
    assert "finstack-quant/sample/src/lib.rs" in diagnostic.path.as_posix()


def test_reviewed_one_way_result_uses_narrow_allowlist(tmp_path: Path) -> None:
    """A reviewed output Result may omit only explicitly allowed capabilities."""
    _write_crate(
        tmp_path,
        """
#[derive(serde::Serialize)]
pub struct OutputResult {
    pub value: String,
}
""",
    )
    exception = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        type_name="OutputResult",
        category="output-view",
        rationale="Emitted report DTO; no supported inbound wire format.",
        allowed_missing=frozenset({"Deserialize", "JsonSchema"}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=(exception,))

    assert not report.failed
    assert report.reviewed_exceptions == (exception,)


def test_stale_allowlist_entry_fails_check(tmp_path: Path) -> None:
    """Removed or fully round-trippable exceptions cannot linger silently."""
    _write_crate(
        tmp_path,
        """
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct OutputResult {
    pub value: String,
}
""",
    )
    exception = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        type_name="OutputResult",
        category="output-view",
        rationale="Emitted report DTO; no supported inbound wire format.",
        allowed_missing=frozenset({"Deserialize", "JsonSchema"}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=(exception,))

    assert report.failed
    stale = report.stale_exceptions[0]
    assert stale.entry == exception
    assert stale.actual_missing == frozenset()
    assert stale.allowed_missing == exception.allowed_missing


def test_exception_is_stale_when_missing_set_changes_or_type_is_not_contract_like(
    tmp_path: Path,
) -> None:
    """Classifications describe an exact capability gap on a gated declaration."""
    _write_crate(
        tmp_path,
        """
#[derive(serde::Serialize)]
pub struct OutputResult {
    pub value: String,
}

pub struct Ordinary {
    pub value: String,
}
""",
    )
    mismatched = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        type_name="OutputResult",
        category="output-view",
        rationale="Serialize-only emitted output.",
        allowed_missing=frozenset({"Deserialize"}),
    )
    not_contract = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        type_name="Ordinary",
        category="runtime-dto",
        rationale="In-process value.",
        allowed_missing=frozenset({"Serialize", "Deserialize", "JsonSchema"}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=(mismatched, not_contract))

    assert {item.reason for item in report.stale_exceptions} == {
        "capability-set-changed",
        "not-contract-like",
    }
    assert report.summaries[0].failures == len(report.diagnostics) + 2


def test_invalid_root_fails_clearly(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Missing repository structure is a clear CLI error, not an empty pass."""
    with pytest.raises(_MODULE.AuditConfigurationError, match="finstack-quant"):
        _MODULE.audit_workspace(tmp_path)
    monkeypatch.setattr(
        sys,
        "argv",
        ["python -m scripts.serde_audit", "--check", "--root", str(tmp_path)],
    )
    with pytest.raises(SystemExit, match="2"):
        _MODULE.main()
    assert "expected finstack-quant/ crate directory" in capsys.readouterr().err


def test_cross_crate_alias_uses_manifest_lib_name(tmp_path: Path) -> None:
    """Cross-crate aliases resolve by Cargo lib name, not directory spelling."""
    crates = tmp_path / "finstack-quant"
    provider = crates / "provider-dir"
    consumer = crates / "consumer"
    for crate in (provider, consumer):
        (crate / "src").mkdir(parents=True)
    (provider / "Cargo.toml").write_text(
        """
[package]
name = "hyphenated-package"
version = "0.1.0"

[lib]
name = "custom_api"
""",
        encoding="utf-8",
    )
    (provider / "src" / "lib.rs").write_text(
        """
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Canonical {
    pub value: String,
}
""",
        encoding="utf-8",
    )
    (consumer / "Cargo.toml").write_text(
        '[package]\nname = "consumer-package"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    (consumer / "src" / "lib.rs").write_text(
        """
use custom_api::Canonical;
pub type ImportedSpec = Canonical;
""",
        encoding="utf-8",
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=())
    alias = next(item for item in report.declarations if item.name == "ImportedSpec")

    assert alias.capabilities == frozenset({"Serialize", "Deserialize", "JsonSchema"})
    assert not report.failed


def test_missing_maintained_registry_type_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A parser/reachability miss cannot silently drop a maintained contract."""
    _write_crate(tmp_path, "pub struct Ordinary { pub value: String }\n")
    monkeypatch.setattr(
        _REGISTRIES,
        "MAINTAINED_CONTRACTS",
        frozenset({("sample", "src/lib.rs", "MissingEnvelope")}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=())

    assert report.failed
    assert report.diagnostics[0].type_name == "MissingEnvelope"
    assert report.diagnostics[0].line == 0


def test_missing_required_binding_output_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A resolver miss cannot silently drop a required binding output."""
    _write_crate(tmp_path, "pub struct Ordinary { pub value: String }\n")
    monkeypatch.setattr(
        _REGISTRIES,
        "REQUIRED_PUBLIC_TYPES",
        {
            (
                "sample",
                "src/lib.rs",
                "MissingBindingEnvelope",
            ): frozenset({"Serialize", "JsonSchema"})
        },
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=())

    assert report.failed
    diagnostic = report.diagnostics[0]
    assert diagnostic.type_name == "MissingBindingEnvelope"
    assert diagnostic.missing == ("JsonSchema", "Serialize")


def test_missing_maintained_one_way_reports_policy_capabilities(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Missing maintained outputs report only their policy requirements."""
    _write_crate(tmp_path, "pub struct Ordinary { pub value: String }\n")
    identity = ("sample", "src/lib.rs", "MissingOneWayResult")
    monkeypatch.setattr(_REGISTRIES, "MAINTAINED_CONTRACTS", frozenset({identity}))
    monkeypatch.setattr(
        _REGISTRIES,
        "MAINTAINED_REQUIRED_CAPABILITIES",
        {identity: frozenset({"Serialize", "JsonSchema"})},
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=())

    assert report.diagnostics[0].missing == ("JsonSchema", "Serialize")


def test_ci_path_runs_audit_tests_and_check_mode() -> None:
    """The build workflow reaches pytest and check mode through gen-check."""
    mise = (_REPOSITORY_ROOT / "mise.toml").read_text(encoding="utf-8")
    build = (_REPOSITORY_ROOT / ".github" / "workflows" / "build.yml").read_text(encoding="utf-8")

    audit_task = mise.split("[tasks.rust-serde-audit]", 1)[1].split("[tasks.", 1)[0]
    check_schemas = mise.split("[tasks.rust-check-schemas]", 1)[1].split("[tasks.", 1)[0]
    gen_check = mise.split("[tasks.gen-check]", 1)[1].split("[tasks.", 1)[0]
    assert "pytest scripts/tests/test_serde_audit.py" in audit_task
    assert "python -m scripts.serde_audit --check" in audit_task
    assert "mise run rust-serde-audit" in check_schemas
    assert "mise run rust-check-schemas" in gen_check
    assert "mise run gen-check" in build


def test_documented_one_way_inventory_is_exact() -> None:
    """The policy's named serialize-only inventory matches the audit registry."""
    policy = (_REPOSITORY_ROOT / "docs" / "SERDE_STABILITY.md").read_text(encoding="utf-8")
    inventory = policy.split("## Serialize-only public output views", 1)[1].split(
        "This list is based on effective trait support",
        1,
    )[0]
    inventory_listing = inventory.split(
        "The current public inventory contains 24 one-way types:",
        1,
    )[1]
    documented_names = set(re.findall(r"`([A-Z][A-Za-z0-9]+)`", inventory_listing))
    audited_names = {identity[2] for identity in _MODULE.ONE_WAY_OUTPUT_IDENTITIES}

    assert "current public inventory contains 24 one-way types" in inventory
    assert documented_names == audited_names
    assert len(_MODULE.ONE_WAY_OUTPUT_IDENTITIES) == 24
    assert len(_MODULE.ONE_WAY_EXCEPTIONS) == 23
    assert all(entry.rationale and entry.category for entry in _MODULE.ONE_WAY_EXCEPTIONS)
    assert all(
        entry.category != "serde-without-schema"
        and "predates" not in entry.rationale
        and "baseline debt" not in entry.rationale
        for entry in _MODULE.REVIEWED_EXCEPTIONS
    )
    reviewed_identities = {(entry.crate, entry.path, entry.type_name) for entry in _MODULE.REVIEWED_EXCEPTIONS}
    assert reviewed_identities.isdisjoint(_MODULE.MAINTAINED_CONTRACTS)
    assert Counter(entry.category for entry in _MODULE.REVIEWED_EXCEPTIONS) == {
        "allocation-output": 3,
        "analysis-report": 2,
        "attribution-report": 5,
        "binding-view": 8,
        "generic-error-result-alias": 7,
        "in-process-execution-envelope": 1,
        "in-process-serde-spec": 16,
        "internal-registry-document": 3,
        "non-maintained-serde-output": 35,
        "runtime-result": 16,
        "runtime-spec": 3,
        "scenario-view": 2,
        "validation-report": 3,
    }
    assert len(_MODULE.REVIEWED_EXCEPTIONS) == 104


def test_maintained_contract_capability_matrix_is_complete() -> None:
    """Every maintained contract has an explicit, bounded capability policy."""
    assert set(_MODULE.MAINTAINED_REQUIRED_CAPABILITIES) == set(_MODULE.MAINTAINED_CONTRACTS)
    assert {
        identity
        for identity, required in _MODULE.MAINTAINED_REQUIRED_CAPABILITIES.items()
        if required != _MODULE.CAPABILITIES
    } == _MODULE.MAINTAINED_ONE_WAY_OUTPUTS
    assert set(_MODULE.MAINTAINED_REQUIRED_CAPABILITIES.values()) <= {
        _MODULE.CAPABILITIES,
        frozenset({"Serialize", "JsonSchema"}),
    }


def test_non_persisted_runtime_result_requires_exact_exception(tmp_path: Path) -> None:
    """Runtime Result naming is exempted per type rather than per crate."""
    _write_crate(
        tmp_path,
        """
pub struct SolverResult {
    pub iterations: usize,
}

pub struct UnexpectedResult {
    pub iterations: usize,
}
""",
    )
    exception = _MODULE.ExceptionEntry(
        crate="sample",
        path="src/lib.rs",
        type_name="SolverResult",
        category="runtime-result",
        rationale="In-memory solver state, never serialized or persisted.",
        allowed_missing=frozenset({"Serialize", "Deserialize", "JsonSchema"}),
    )

    report = _MODULE.audit_workspace(tmp_path, allowlist=(exception,))

    assert report.failed
    assert [diagnostic.type_name for diagnostic in report.diagnostics] == ["UnexpectedResult"]


@pytest.mark.parametrize("mode", ["report", "check"])
def test_cli_modes_are_available(mode: str) -> None:
    """The command-line parser exposes explicit report and check modes."""
    parser = _MODULE.build_parser()
    arguments = parser.parse_args([f"--{mode}"])

    assert getattr(arguments, mode)
