"""Reject obsolete or hand-maintained JSON/Serde contract machinery."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOTS = (
    ROOT / "finstack-quant",
    ROOT / "finstack-quant-py/src",
    ROOT / "finstack-quant-py/finstack_quant",
    ROOT / "finstack-quant-wasm/src",
    ROOT / "scripts",
)
SOURCE_SUFFIXES = {".rs", ".py", ".pyi", ".js", ".mjs", ".ts"}
CONTRACT_ROOTS = (
    ROOT / "finstack-quant",
    ROOT / "finstack-quant-py",
    ROOT / "finstack-quant-wasm",
    ROOT / "scripts",
    ROOT / "docs",
)
CONTRACT_SUFFIXES = SOURCE_SUFFIXES | {".ipynb", ".json", ".md", ".toml"}
EXCLUDED_PARTS = {
    "tests",
    "benches",
    "examples",
    "target",
    "node_modules",
    "__pycache__",
}
CONTRACT_EXCLUDED_PARTS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "node_modules",
    "target",
    "__pycache__",
}
EXCLUDED_PREFIXES = (
    Path("docs/superpowers/plans"),
    Path("scripts/tests/fixtures"),
    Path("scripts/check_schema_residue.py"),
)
SCHEMA_REJECTION_MARKER = "schema-rejection-test"

# These enums intentionally use exact values that cannot be represented by a
# container-level snake_case rule. Every other persisted serde enum must state
# its naming policy mechanically; stale entries fail the check.
ENUM_NAMING_EXCEPTIONS = {
    ("finstack-quant/attribution/src/spec.rs", "AttributionSchema"): "exact namespaced schema marker",
    ("finstack-quant/core/src/dates/daycount/mod.rs", "DayCount"): "contractually fixed day-count spellings",
    ("finstack-quant/core/src/types/ratings.rs", "CreditRating"): "external agency rating codes",
    (
        "finstack-quant/models/src/factor/credit/hierarchy.rs",
        "CreditFactorModelSchema",
    ): "exact namespaced schema marker",
    ("finstack-quant/models/src/factor/envelope.rs", "FactorModelConfigSchema"): "exact namespaced schema marker",
    ("finstack-quant/margin/src/schema.rs", "MarginSchema"): "exact namespaced schema marker",
    (
        "finstack-quant/portfolio/src/materialization/envelope.rs",
        "PortfolioMaterializationSchema",
    ): "exact namespaced schema marker",
    ("finstack-quant/scenarios/src/envelope.rs", "ScenarioSchema"): "exact namespaced schema marker",
    ("finstack-quant/scenarios/src/templates/json.rs", "ScenarioTemplateSchema"): "exact namespaced schema marker",
    ("finstack-quant/test-utils/src/golden/types.rs", "Tolerance"): "contractually fixed tolerance unit tags",
    (
        "finstack-quant/calibration/src/api/schema.rs",
        "CalibrationSchema",
    ): "exact namespaced schema marker",
    (
        "finstack-quant/valuations/src/instruments/fixed_income/mbs_passthrough/types.rs",
        "AgencyProgram",
    ): "external agency program codes",
    (
        "finstack-quant/valuations/src/instruments/fx/ndf/types.rs",
        "NdfFixingSource",
    ): "external fixing-source codes",
    (
        "finstack-quant/valuations/src/instruments/json_loader.rs",
        "InstrumentSchema",
    ): "exact namespaced schema marker",
}


@dataclass(frozen=True)
class ForbiddenPattern:
    """One forbidden production pattern and its diagnostic label."""

    label: str
    expression: re.Pattern[str]


FORBIDDEN = (
    ForbiddenPattern(
        "manual JsonSchema implementation",
        re.compile(r"\bimpl\s+(?:schemars::)?JsonSchema\s+for\b"),
    ),
    ForbiddenPattern(
        "handwritten json_schema macro",
        re.compile(r"\b(?:schemars::)?json_schema!\s*\("),
    ),
    ForbiddenPattern("schema_with override", re.compile(r"\bschema_with\b")),
    ForbiddenPattern(
        "serde compatibility alias",
        re.compile(r"#\s*\[\s*serde\s*\([^\]]*\balias\s*="),
    ),
    ForbiddenPattern(
        "serde_json::Value schema projection",
        re.compile(r"#\s*\[\s*schemars\s*\(\s*with\s*=\s*\"serde_json::Value\""),
    ),
    ForbiddenPattern(
        "schema-only single-instrument shadow enum",
        re.compile(r"\benum\s+SingleInstrumentJson\b"),
    ),
    ForbiddenPattern(
        "obsolete schema assembly helper",
        re.compile(
            r"\b(?:postprocess_schema|assemble_schema|fallback_instrument_schema|"
            r"PricingOverridesWire|rust_sync_schemas|sync_instrument_schema_overrides|"
            r"NormalizedEnum|parse_normalized_enum|parse_day_count_override|"
            r"value_portfolio_typed|parametric_var_decomposition_typed|"
            r"historical_var_decomposition_typed|evaluate_risk_budget_typed|"
            r"optimize_portfolio_typed)\b"
        ),
    ),
    ForbiddenPattern(
        "retired skipped-path field",
        re.compile(r"\bnum_skipped\b"),
    ),
    ForbiddenPattern(
        "retired scalar arbitrage-forward field",
        re.compile(r"\bforward\s*:\s*Option\s*<\s*f64\s*>"),
    ),
    ForbiddenPattern(
        "retired schema contract symbol",
        re.compile(
            r"\b(?:LegacyPanelTransformResult|spot_ids|add_spot_id|equity_instruments|"
            r"dual_values|cache_budget_mb|CacheStrategy|deprecated_ids|sp_empirical|"
            r"moodys_empirical|sp_assumptions_v1|moodys_assumptions_v1|bilateral_cva)\b"
        ),
    ),
    ForbiddenPattern(
        "retired abbreviated contract identifier",
        re.compile(
            r"\b(?:freq|dc|bdc|asof_spreads|surface_ids|"
            r"[A-Za-z][A-Za-z0-9_]*_(?:ccy|bps))\b"
        ),
    ),
)

DIRECT_CONSUMER_FORBIDDEN = (
    ForbiddenPattern(
        "bare instrument JSON serialization",
        re.compile(
            r'json\.dumps\s*\(\s*\{\s*(?:"|\\")type(?:"|\\")\s*:\s*'
            r'(?:"|\\")[^"\\]+(?:"|\\")\s*,\s*(?:"|\\")spec(?:"|\\")\s*:'
        ),
    ),
    ForbiddenPattern(
        "bare InstrumentJson serialization",
        re.compile(r"serde_json::to_string(?:_pretty)?\s*\(\s*&\s*InstrumentJson::"),
    ),
    ForbiddenPattern(
        "retired JSON contract key",
        re.compile(
            r'(?:"|\\")'
            r"(?:freq|dc|bdc|asof_spreads|surface_ids|initial_market|rate_calibration_recipe|num_skipped|"
            r"spot_ids|equity_instruments|dual_values|cache_budget_mb|deprecated_ids|bilateral_cva|"
            r"[A-Za-z][A-Za-z0-9_]*_(?:ccy|bps))"
            r'(?:"|\\")\s*:'
        ),
    ),
    ForbiddenPattern(
        "retired schema contract symbol",
        re.compile(
            r"\b(?:LegacyPanelTransformResult|spot_ids|add_spot_id|equity_instruments|"
            r"dual_values|cache_budget_mb|CacheStrategy|deprecated_ids|sp_empirical|"
            r"moodys_empirical|sp_assumptions_v1|moodys_assumptions_v1|bilateral_cva)\b"
        ),
    ),
    ForbiddenPattern(
        "retired margin registry identifier",
        re.compile(r"\bvaluations\.margin_registry\.v1\b"),
    ),
    ForbiddenPattern(
        "retired persisted day-count spelling",
        re.compile(
            r'(?:"|\\")'
            r"(?:Act360|Actual360|Act365F|Act365L|Nl365|Thirty360|Thirty360::BondBasis|ThirtyE360|"
            r"ThirtyE360Isda|ActAct|ActActIsma|Bus252|Actual365Fixed|"
            r"actual_360|actual_365_fixed|thirty_360|30_360::BondBasis)"
            r'(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "non-canonical acronym splitting",
        re.compile(
            r'(?:"|\\")'
            r"(?:yo_y_inflation_swap|hull_white1_f|na_n_propagated)"
            r'(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "retired PascalCase convertible enum value",
        re.compile(
            r'(?:(?:"|\\")(?:policy|anti_dilution|dividend_adjustment)(?:"|\\")\s*:\s*)'
            r'(?:"|\\")(?:Voluntary|FullRatchet|WeightedAverage|AdjustPrice|AdjustRatio)(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "retired PascalCase type discriminator",
        re.compile(
            r'(?:(?:"|\\")type(?:"|\\")\s*:\s*)'
            r'(?:"|\\")[A-Z][A-Za-z0-9_]*(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "retired tranche behavior_type field",
        re.compile(r'(?:"|\\")behavior_type(?:"|\\")\s*:'),
    ),
    ForbiddenPattern(
        "retired PascalCase tranche seniority",
        re.compile(
            r'(?:(?:"|\\")seniority(?:"|\\")\s*:\s*)'
            r'(?:"|\\")(?:Senior|Mezzanine|Subordinated|Equity)(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "retired PascalCase typed result value",
        re.compile(
            r'(?:(?:"|\\")'
            r"(?:instrument_type|model_key|pricing_mode|account_type)"
            r'(?:"|\\")\s*:\s*)'
            r'(?:"|\\")[A-Z][A-Za-z0-9_() ,-]*(?:"|\\")'
        ),
    ),
    ForbiddenPattern(
        "retired calibration marker",
        re.compile(r"(?:finstack_quant\.calibration|schemas/calibration)/(?:0|[2-9][0-9]*)\b"),
    ),
    ForbiddenPattern(
        "retired numeric schema version",
        re.compile(r'(?:"|\\")schema_version(?:"|\\")\s*:\s*(?:0|[2-9][0-9]*)\b'),
    ),
    ForbiddenPattern(
        "malformed repeated canonical name",
        re.compile(
            r"\b[A-Za-z][A-Za-z0-9_]*(?:"
            r"frequencyuenc|frequencys|currencyrenc|currencys|"
            r"conventionention|business_day_conventionf|countount|day_countf|"
            r"versionersion|surfaceface|spreads_preads|as_of_of|"
            r"credit_curve_curve|hazard_curve_curve|vol_surface_surface|bp_bp|"
            r"frequency_frequency|currency_currency|day_count_day_count|"
            r"business_day_convention_business_day_convention"
            r")[A-Za-z0-9_]*\b"
        ),
    ),
)

BINDING_FORBIDDEN = (
    ForbiddenPattern(
        "binding bypasses canonical instrument envelope",
        re.compile(r"serde_json::from_(?:str|value)\s*::\s*<\s*InstrumentJson\s*>"),
    ),
    ForbiddenPattern(
        "binding bypasses canonical instrument envelope",
        re.compile(
            r"(?:let|const)\s+[A-Za-z_]\w*\s*:\s*InstrumentJson\s*=\s*"
            r"serde_json::from_(?:str|value)\s*\("
        ),
    ),
)


RETIRED_PATHS = (
    ROOT / "scripts/sync_instrument_schema_overrides.py",
    ROOT / "scripts/golden/migrate_v1_to_v2.py",
    ROOT / "finstack-quant/calibration/schemas/calibration/2",
    ROOT / "finstack-quant/calibration/schemas/calibration/3",
    ROOT / "finstack-quant/calibration/schemas/calibration/4",
)


def source_files() -> list[Path]:
    """Return deterministic production and direct-consumer source files."""
    files: set[Path] = set()
    for source_root in SOURCE_ROOTS:
        candidates = [source_root] if source_root.is_file() else source_root.rglob("*")
        for path in candidates:
            if not path.is_file() or path.suffix not in SOURCE_SUFFIXES:
                continue
            relative = path.relative_to(ROOT)
            if any(part in EXCLUDED_PARTS for part in relative.parts):
                continue
            if any(relative.is_relative_to(prefix) for prefix in EXCLUDED_PREFIXES):
                continue
            files.add(path)
    return sorted(files)


def contract_files() -> list[Path]:
    """Return every text artifact that can carry or document a JSON contract."""
    files: set[Path] = set()
    for contract_root in CONTRACT_ROOTS:
        candidates = [contract_root] if contract_root.is_file() else contract_root.rglob("*")
        for path in candidates:
            if not path.is_file() or path.suffix not in CONTRACT_SUFFIXES:
                continue
            relative = path.relative_to(ROOT)
            if any(part in CONTRACT_EXCLUDED_PARTS for part in relative.parts):
                continue
            if any(relative.is_relative_to(prefix) for prefix in EXCLUDED_PREFIXES):
                continue
            files.add(path)
    return sorted(files)


def line_number(contents: str, offset: int) -> int:
    """Convert a character offset to a one-based line number."""
    return contents.count("\n", 0, offset) + 1


def matching_attribute_open(contents: str, close_index: int) -> int | None:
    """Return the opening bracket matching one attribute's closing bracket."""
    depth = 0
    for index in range(close_index, -1, -1):
        if contents[index] == "]":
            depth += 1
        elif contents[index] == "[":
            depth -= 1
            if depth == 0:
                return index
    return None


def preceding_attributes(contents: str, declaration_start: int) -> tuple[str, ...]:
    """Collect contiguous Rust attributes immediately before a declaration."""
    ranges: list[tuple[int, int]] = []
    cursor = declaration_start - 1
    while cursor >= 0:
        while cursor >= 0 and contents[cursor].isspace():
            cursor -= 1
        if cursor < 0 or contents[cursor] != "]":
            break
        open_index = matching_attribute_open(contents, cursor)
        if open_index is None:
            break
        hash_index = open_index - 1
        while hash_index >= 0 and contents[hash_index].isspace():
            hash_index -= 1
        if hash_index < 0 or contents[hash_index] != "#":
            break
        ranges.append((hash_index, cursor + 1))
        cursor = hash_index - 1
    ranges.reverse()
    return tuple(contents[start:end] for start, end in ranges)


def serde_enum_naming_findings(files: list[Path]) -> list[str]:
    """Reject persisted enums without an explicit, reviewed wire naming rule."""
    findings: list[str] = []
    reviewed: set[tuple[str, str]] = set()
    enum_pattern = re.compile(r"\b(?:pub(?:\([^)]*\))?\s+)?enum\s+(?P<name>[A-Za-z_]\w*)")

    for path in files:
        if path.suffix != ".rs":
            continue
        contents = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT).as_posix()
        for match in enum_pattern.finditer(contents):
            attributes = " ".join(preceding_attributes(contents, match.start()))
            if "Serialize" not in attributes or "Deserialize" not in attributes:
                continue

            identity = (relative, match.group("name"))
            has_snake_case = bool(re.search(r'rename_all\s*=\s*"snake_case"', attributes))
            is_untagged = bool(re.search(r"\buntagged\b", attributes))
            uses_wire_type = bool(re.search(r"\btry_from\s*=", attributes) and re.search(r"\binto\s*=", attributes))
            canonical = has_snake_case or is_untagged or uses_wire_type

            if identity in ENUM_NAMING_EXCEPTIONS:
                if canonical:
                    findings.append(
                        f"{relative}:{line_number(contents, match.start())}: "
                        f"stale serde enum naming exception for {identity[1]}"
                    )
                else:
                    reviewed.add(identity)
                continue
            if canonical:
                continue

            findings.append(
                f"{relative}:{line_number(contents, match.start())}: "
                f"serde enum {identity[1]} lacks canonical snake_case naming"
            )

    for relative, name in sorted(ENUM_NAMING_EXCEPTIONS.keys() - reviewed):
        findings.append(f"stale serde enum naming exception: {relative}:{name}")
    return findings


def optional_wire_adapter_findings(files: list[Path]) -> list[str]:
    """Reject optional serde adapters that accidentally require omitted fields."""
    findings: list[str] = []
    field_pattern = re.compile(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>[A-Za-z_]\w*)\s*:")

    for path in files:
        if path.suffix != ".rs":
            continue
        contents = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT).as_posix()
        for match in field_pattern.finditer(contents):
            attributes = " ".join(preceding_attributes(contents, match.start()))
            if not re.search(r'wire::optional_[A-Za-z0-9_]+(?:"|\b)', attributes):
                continue
            if re.search(r"serde\s*\([^\]]*\bdefault\b", attributes):
                continue
            findings.append(
                f"{relative}:{line_number(contents, match.start())}: optional wire adapter "
                f"for {match.group('name')} lacks a serde default"
            )
    return findings


def asymmetric_bounded_adapter_findings(files: list[Path]) -> list[str]:
    """Reject bounded numeric fields that validate only while deserializing."""
    findings: list[str] = []
    field_pattern = re.compile(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?P<name>[A-Za-z_]\w*)\s*:")
    adapter_pairs = {
        "deserialize_positive_f64": "serialize_positive_f64",
        "deserialize_non_negative_f64": "serialize_non_negative_f64",
        "deserialize_closed_unit_interval_f64": "serialize_closed_unit_interval_f64",
        "deserialize_open_unit_interval_f64": "serialize_open_unit_interval_f64",
        "deserialize_correlation": "serialize_correlation",
        "deserialize_optional_positive_f64": "serialize_optional_positive_f64",
    }

    for path in files:
        if path.suffix != ".rs":
            continue
        contents = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT).as_posix()
        for match in field_pattern.finditer(contents):
            attributes = " ".join(preceding_attributes(contents, match.start()))
            for deserialize_name, serialize_name in adapter_pairs.items():
                if deserialize_name not in attributes or serialize_name in attributes:
                    continue
                findings.append(
                    f"{relative}:{line_number(contents, match.start())}: bounded numeric adapter "
                    f"for {match.group('name')} uses {deserialize_name} without {serialize_name}"
                )
    return findings


def schema_artifacts() -> list[Path]:
    """Return every checked-in generated JSON Schema artifact, sorted."""
    return sorted(ROOT.glob("finstack-quant/*/schemas/**/*.schema.json"))


def positional_schema_name_findings(artifacts: list[tuple[Path, dict]]) -> list[str]:
    """Reject `$defs` names that schemars disambiguated with a positional digit.

    Two Rust types sharing a name make schemars emit `Foo` and `Foo2`. Which
    one gets the suffix depends on visit order, so the name is unstable across
    unrelated edits and a cached tool schema silently starts describing the
    other type. Give one of the two an explicit `#[schemars(rename = "...")]`.

    Only a trailing digit whose stem is *also* a definition in the same
    artifact is a collision, so genuine names such as `Iso4217` or `Sha256`
    stay legal.
    """
    findings: list[str] = []
    for path, document in artifacts:
        names = set(document.get("$defs", {}))
        for name in sorted(names):
            stem = name.rstrip("0123456789")
            if stem != name and stem in names:
                findings.append(
                    f"{path.relative_to(ROOT)}: positional `$defs` name {name!r} collides "
                    f"with {stem!r}; rename one with #[schemars(rename = ...)]"
                )
    return findings


def _accepted_string_values(document: object) -> set[str]:
    """Collect every string an artifact accepts via `const` or `enum`."""
    values: set[str] = set()
    if isinstance(document, dict):
        constant = document.get("const")
        if isinstance(constant, str):
            values.add(constant)
        enumeration = document.get("enum")
        if isinstance(enumeration, list):
            values.update(entry for entry in enumeration if isinstance(entry, str))
        for value in document.values():
            values |= _accepted_string_values(value)
    elif isinstance(document, list):
        for value in document:
            values |= _accepted_string_values(value)
    return values


# A backticked literal in a description that matches an accepted value once
# separators are removed, but is not itself accepted. Case is significant so
# that prose citing a Rust variant name (`ZSpread` for the wire's `z_spread`)
# stays legal; only separator spellings are treated as typos.
_DOC_LITERAL = re.compile(r"`([A-Za-z0-9][A-Za-z0-9_/.\-]{1,40})`")


def _fold_separators(value: str) -> str:
    return re.sub(r"[/_.\-]", "", value)


def _misspelled_literals(
    node: object, accepted: set[str], by_folded: dict[str, str], reported: set[tuple[str, str]]
) -> None:
    """Collect backticked literals that differ from an accepted value by separators alone."""
    if isinstance(node, dict):
        description = node.get("description")
        if isinstance(description, str):
            for literal in _DOC_LITERAL.findall(description):
                if literal in accepted:
                    continue
                match = by_folded.get(_fold_separators(literal))
                if match is not None and match != literal:
                    reported.add((literal, match))
        for value in node.values():
            _misspelled_literals(value, accepted, by_folded, reported)
    elif isinstance(node, list):
        for value in node:
            _misspelled_literals(value, accepted, by_folded, reported)


def undeclared_enum_spelling_findings(artifacts: list[tuple[Path, dict]]) -> list[str]:
    """Reject descriptions that advertise a value the same artifact rejects.

    A schema whose prose promises `act/360` while its `const` set only holds
    `act_360` is self-contradicting: a reader — human or model — copies the
    documented spelling and is rejected at the door.
    """
    findings: list[str] = []
    for path, document in artifacts:
        accepted = _accepted_string_values(document)
        by_folded: dict[str, str] = {}
        for value in sorted(accepted):
            by_folded.setdefault(_fold_separators(value), value)

        reported: set[tuple[str, str]] = set()
        _misspelled_literals(document, accepted, by_folded, reported)
        for literal, match in sorted(reported):
            findings.append(
                f"{path.relative_to(ROOT)}: description advertises {literal!r} but the artifact only accepts {match!r}"
            )
    return findings


def _reference_blind(node: object) -> object:
    """Structural signature that ignores every `$ref` target.

    Artifacts differ in whether shared definitions are externalized, so the same
    definition legitimately appears with local and published references. Blanking
    the target compares meaning rather than reference style.
    """
    if isinstance(node, dict):
        return {key: ("<ref>" if key == "$ref" else _reference_blind(value)) for key, value in node.items()}
    if isinstance(node, list):
        return [_reference_blind(value) for value in node]
    return node


def conflicting_schema_definition_findings(artifacts: list[tuple[Path, dict]]) -> list[str]:
    """Reject one `$defs` name standing for two different types across artifacts.

    A consumer that caches `Severity` from one artifact and applies it to
    another gets a silently wrong contract.
    """
    bodies: dict[str, dict[str, list[str]]] = {}
    for path, document in artifacts:
        for name, body in document.get("$defs", {}).items():
            signature = json.dumps(_reference_blind(body), sort_keys=True, separators=(",", ":"))
            bodies.setdefault(name, {}).setdefault(signature, []).append(path.relative_to(ROOT).as_posix())

    findings: list[str] = []
    for name, signatures in sorted(bodies.items()):
        if len(signatures) < 2:
            continue
        owners = "; ".join(sorted(files[0] for files in signatures.values()))
        findings.append(
            f"`$defs` name {name!r} describes {len(signatures)} different types "
            f"({owners}): rename all but one with #[schemars(rename = ...)]"
        )
    return findings


def is_explicit_rejection_test(contents: str, match: re.Match[str]) -> bool:
    """Return whether a retired spelling is intentionally tested for rejection."""
    line_start = contents.rfind("\n", 0, match.start()) + 1
    line_end = contents.find("\n", match.end())
    if line_end == -1:
        line_end = len(contents)
    if SCHEMA_REJECTION_MARKER in contents[line_start:line_end]:
        return True

    rust_test_start = contents.rfind("#[test]", 0, match.start())
    if rust_test_start == -1:
        return False
    return SCHEMA_REJECTION_MARKER in contents[rust_test_start : match.start()]


def main() -> int:
    """Report every forbidden residue and fail if any remains."""
    findings: list[str] = []
    sources = source_files()
    for path in sources:
        contents = path.read_text(encoding="utf-8")
        for forbidden in FORBIDDEN:
            for match in forbidden.expression.finditer(contents):
                line_start = contents.rfind("\n", 0, match.start()) + 1
                line_end = contents.find("\n", match.end())
                if line_end == -1:
                    line_end = len(contents)
                contents[line_start:line_end]
                if is_explicit_rejection_test(contents, match):
                    continue
                relative = path.relative_to(ROOT)
                findings.append(
                    f"{relative}:{line_number(contents, match.start())}: {forbidden.label}: {match.group(0)!r}"
                )

        if path.is_relative_to(ROOT / "finstack-quant-py/src") or path.is_relative_to(ROOT / "finstack-quant-wasm/src"):
            for forbidden in BINDING_FORBIDDEN:
                for match in forbidden.expression.finditer(contents):
                    relative = path.relative_to(ROOT)
                    findings.append(
                        f"{relative}:{line_number(contents, match.start())}: {forbidden.label}: {match.group(0)!r}"
                    )

    findings.extend(serde_enum_naming_findings(sources))
    findings.extend(optional_wire_adapter_findings(sources))
    findings.extend(asymmetric_bounded_adapter_findings(sources))

    for path in contract_files():
        contents = path.read_text(encoding="utf-8")
        for forbidden in DIRECT_CONSUMER_FORBIDDEN:
            for match in forbidden.expression.finditer(contents):
                if is_explicit_rejection_test(contents, match):
                    continue
                relative = path.relative_to(ROOT)
                findings.append(
                    f"{relative}:{line_number(contents, match.start())}: {forbidden.label}: {match.group(0)!r}"
                )

    artifacts = [(path, json.loads(path.read_text(encoding="utf-8"))) for path in schema_artifacts()]
    findings.extend(positional_schema_name_findings(artifacts))
    findings.extend(undeclared_enum_spelling_findings(artifacts))
    findings.extend(conflicting_schema_definition_findings(artifacts))

    findings.extend(f"retired path still exists: {path.relative_to(ROOT)}" for path in RETIRED_PATHS if path.exists())
    manifest = (ROOT / "scripts/generated-artifacts.txt").read_text(encoding="utf-8")
    if "/schemas/" in manifest:
        findings.append("scripts/generated-artifacts.txt duplicates schema registry ownership")

    if findings:
        raise SystemExit("JSON/Serde residue check failed:\n" + "\n".join(findings))
    print("JSON/Serde production and direct-consumer residue check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
