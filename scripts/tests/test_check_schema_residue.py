"""Tests for the generated-schema residue lints.

These lints exist to stop three defects that shipped undetected once already, so
each one is tested both ways: it must fire on the shape it targets and stay
silent on the legitimate shapes that resemble it.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_schema_residue.py"
_SPEC = importlib.util.spec_from_file_location("check_schema_residue", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)

_ARTIFACT = _MODULE.ROOT / "finstack-quant/example/schemas/example/1/example.schema.json"
_OTHER = _MODULE.ROOT / "finstack-quant/other/schemas/other/1/other.schema.json"


def test_positional_name_fires_on_schemars_collision_suffix() -> None:
    """A `Foo`/`Foo2` pair in one artifact is a schemars collision."""
    findings = _MODULE.positional_schema_name_findings([(_ARTIFACT, {"$defs": {"AssetType": {}, "AssetType2": {}}})])
    assert len(findings) == 1
    assert "'AssetType2'" in findings[0]


def test_positional_name_allows_names_that_genuinely_end_in_digits() -> None:
    """Names whose stem is not a sibling definition are legitimate."""
    # `Iso4217`/`Sha256`/`Target2` are real names, not collision suffixes: their
    # stems are not themselves definitions.
    findings = _MODULE.positional_schema_name_findings([
        (_ARTIFACT, {"$defs": {"Iso4217": {}, "Sha256": {}, "Target2": {}}})
    ])
    assert findings == []


def test_enum_spelling_fires_on_separator_typo_in_a_description() -> None:
    """Prose promising `act/360` where only `act_360` is accepted is a defect."""
    document = {
        "description": "Supported values include `act/360` and `30/360`.",
        "$defs": {"DayCount": {"oneOf": [{"const": "act_360"}, {"const": "30_360"}]}},
    }
    findings = _MODULE.undeclared_enum_spelling_findings([(_ARTIFACT, document)])
    assert len(findings) == 2
    assert any("'act/360'" in finding and "'act_360'" in finding for finding in findings)


def test_enum_spelling_allows_prose_citing_a_rust_variant_name() -> None:
    """Case differences name the Rust variant and stay legal."""
    # Doc comments legitimately name the Rust variant (`ZSpread`) alongside the
    # wire spelling (`z_spread`); case is what distinguishes the two.
    document = {
        "description": "The `ZSpread` variant serializes as `z_spread`.",
        "$defs": {"Measure": {"enum": ["z_spread"]}},
    }
    assert _MODULE.undeclared_enum_spelling_findings([(_ARTIFACT, document)]) == []


def test_conflicting_definitions_fire_when_one_name_means_two_things() -> None:
    """One `$defs` name with two meanings misleads a caching consumer."""
    findings = _MODULE.conflicting_schema_definition_findings([
        (_ARTIFACT, {"$defs": {"Severity": {"enum": ["info", "warning", "error"]}}}),
        (_OTHER, {"$defs": {"Severity": {"enum": ["error", "warning"]}}}),
    ])
    assert len(findings) == 1
    assert "'Severity'" in findings[0]


def test_conflicting_definitions_tolerate_externalized_versus_inlined_refs() -> None:
    """Reference style is not a difference in meaning."""
    # The same definition legitimately appears with a local `$ref` in artifacts
    # that do not externalize and a published `$ref` in those that do.
    inlined = {"$defs": {"Deposit": {"properties": {"id": {"$ref": "#/$defs/Id"}}}}}
    externalized = {
        "$defs": {
            "Deposit": {"properties": {"id": {"$ref": "https://finstack_quant.dev/schemas/common/1/id.schema.json"}}}
        }
    }
    findings = _MODULE.conflicting_schema_definition_findings([(_ARTIFACT, inlined), (_OTHER, externalized)])
    assert findings == []


def test_checked_in_artifacts_are_clean() -> None:
    """The shipped corpus must satisfy every lint above."""
    artifacts = [(path, _MODULE.json.loads(path.read_text(encoding="utf-8"))) for path in _MODULE.schema_artifacts()]
    assert artifacts, "expected checked-in schema artifacts to lint against"
    assert _MODULE.positional_schema_name_findings(artifacts) == []
    assert _MODULE.undeclared_enum_spelling_findings(artifacts) == []
    assert _MODULE.conflicting_schema_definition_findings(artifacts) == []


# One text per prefiltered pattern that the full expression must flag, kept in
# a fixture so the residue check does not flag this test. Each prefilter must
# match inside every such match, or it would skip a file that holds real residue.
_PREFILTER_POSITIVES = _MODULE.json.loads(
    (Path(__file__).parent / "fixtures/residue_prefilter_positives.json").read_text(encoding="utf-8")
)


def test_every_prefilter_is_contained_in_every_match() -> None:
    """A prefilter that misses a real match would silently disable its check."""
    prefiltered = [
        forbidden
        for group in (_MODULE.FORBIDDEN, _MODULE.DIRECT_CONSUMER_FORBIDDEN, _MODULE.BINDING_FORBIDDEN)
        for forbidden in group
        if forbidden.prefilter is not None
    ]
    assert {forbidden.label for forbidden in prefiltered} == set(_PREFILTER_POSITIVES)
    for forbidden in prefiltered:
        text = _PREFILTER_POSITIVES[forbidden.label]
        matches = list(forbidden.expression.finditer(text))
        assert matches, f"{forbidden.label}: positive example does not match"
        for match in matches:
            assert forbidden.prefilter.search(match.group(0)), f"{forbidden.label}: prefilter misses {match.group(0)!r}"
        assert [match.span() for match in forbidden.finditer(text)] == [match.span() for match in matches]


def test_prefilter_skips_files_it_rules_out() -> None:
    """A file without the prefilter's piece is not scanned at all."""
    forbidden = next(item for item in _MODULE.FORBIDDEN if item.label == "retired skipped-path field")
    assert list(forbidden.finditer("fn unrelated() {}")) == []
