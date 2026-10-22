"""Runtime checks for the public JSON Schema namespaces.

These tests pin module topology, the ``__all__`` surface, returned schema text,
and the ``KeyError``/``ValueError`` mapping. Statements use the canonical Rust
schema registry. Returned documents must match the checked-in artifacts, so
stale generated files or extension builds are detected at the host boundary.
"""

from __future__ import annotations

import importlib
import json
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

from finstack_quant.cashflows import schema as cashflows_schema
from finstack_quant.models.factor import schema as factor_model_schema
from finstack_quant.statements import schema as statements_schema
from finstack_quant.valuations import schema as valuations_schema

REPO_ROOT = Path(__file__).resolve().parents[2]
CASHFLOW_SCHEMA_BASE = "https://finstack_quant.dev/schemas/cashflow/1/"
INSTRUMENT_SCHEMA_BASE = "https://finstack_quant.dev/schemas/instrument/1/"
INSTRUMENT_SCHEMA_DIR = REPO_ROOT / "finstack-quant" / "valuations" / "schemas" / "instruments" / "1"

SCHEMA_MODULES: dict[str, ModuleType] = {
    "finstack_quant.cashflows.schema": cashflows_schema,
    "finstack_quant.models.factor.schema": factor_model_schema,
    "finstack_quant.statements.schema": statements_schema,
    "finstack_quant.valuations.schema": valuations_schema,
}

# Zero-argument accessors that return a single JSON Schema document as text.
DOCUMENT_ACCESSORS: list[tuple[str, str]] = [
    ("finstack_quant.valuations.schema", "instrument_envelope_schema"),
    ("finstack_quant.valuations.schema", "valuation_result_schema"),
]

# (module, accessor, repo-relative path of the generated schema artifact).
COMPILED_VS_CHECKED_IN: list[tuple[str, str, str]] = [
    (
        "finstack_quant.statements.schema",
        "financial_model_spec.schema.json",
        "finstack-quant/statements/schemas/statements/1/financial_model_spec.schema.json",
    ),
    (
        "finstack_quant.statements.schema",
        "normalization_config.schema.json",
        "finstack-quant/statements/schemas/statements/1/normalization_config.schema.json",
    ),
    (
        "finstack_quant.statements.schema",
        "statement_result.schema.json",
        "finstack-quant/statements/schemas/statements/1/statement_result.schema.json",
    ),
    (
        "finstack_quant.valuations.schema",
        "instrument_envelope_schema",
        "finstack-quant/valuations/schemas/instruments/1/instrument.schema.json",
    ),
    (
        "finstack_quant.valuations.schema",
        "valuation_result_schema",
        "finstack-quant/valuations/schemas/results/1/valuation_result.schema.json",
    ),
]

# Every schema namespace carries the three registry functions (`get`, `index`,
# `validate`) alongside its own accessors, and lists them in sorted order.
EXPECTED_EXPORTS: dict[str, list[str]] = {
    "finstack_quant.cashflows.schema": [
        "get",
        "index",
        "resources",
        "validate",
    ],
    "finstack_quant.models.factor.schema": [
        "get",
        "index",
        "validate",
    ],
    "finstack_quant.statements.schema": [
        "get",
        "index",
        "validate",
    ],
    "finstack_quant.valuations.schema": [
        "get",
        "index",
        "instrument_envelope_schema",
        "instrument_schema",
        "instrument_types",
        "validate",
        "validate_instrument_envelope_json",
        "validate_instrument_type_json",
        "valuation_result_schema",
    ],
}


def _parse_schema_document(text: str) -> dict[str, Any]:
    """Parse accessor output and assert it is a JSON Schema object."""
    assert isinstance(text, str), f"accessor returned {type(text)!r}, expected str"
    document = json.loads(text)
    assert isinstance(document, dict), "schema text must decode to a JSON object"
    assert "$schema" in document or "title" in document, (
        f"schema object carries neither `$schema` nor `title`: {sorted(document)[:8]}"
    )
    return document


@pytest.mark.parametrize("module_path", sorted(SCHEMA_MODULES))
def test_schema_namespace_is_importable(module_path: str) -> None:
    """Each ``<domain>.schema`` namespace resolves on its public dotted path."""
    imported = importlib.import_module(module_path)
    assert imported is SCHEMA_MODULES[module_path]
    assert imported.__doc__, f"{module_path} must document itself"


@pytest.mark.parametrize("module_path", sorted(SCHEMA_MODULES))
def test_schema_namespace_all_is_exhaustive_and_sorted(module_path: str) -> None:
    """``__all__`` lists every public callable, in sorted order."""
    module = SCHEMA_MODULES[module_path]
    exported = list(module.__all__)
    assert exported == sorted(exported), f"{module_path}.__all__ is not sorted"
    assert exported == EXPECTED_EXPORTS[module_path]

    public = {name for name in dir(module) if not name.startswith("_")}
    assert public == set(exported), (
        f"{module_path}.__all__ diverged from the live surface.\n"
        f"  missing from __all__: {sorted(public - set(exported))}\n"
        f"  listed but absent: {sorted(set(exported) - public)}"
    )
    for name in exported:
        assert callable(getattr(module, name)), f"{module_path}.{name} is not callable"


@pytest.mark.parametrize(("module_path", "accessor"), DOCUMENT_ACCESSORS)
def test_document_accessor_returns_parsable_schema_text(module_path: str, accessor: str) -> None:
    """Every single-document accessor returns JSON text for a schema object."""
    _parse_schema_document(getattr(SCHEMA_MODULES[module_path], accessor)())


def test_instrument_types_are_non_empty_and_all_resolve() -> None:
    """Every registry discriminator resolves through ``instrument_schema``."""
    types = valuations_schema.instrument_types()
    assert isinstance(types, list)
    assert types, "instrument registry must not be empty"
    assert "bond" in types
    assert len(set(types)) == len(types), "instrument discriminators must be unique"

    for instrument_type in types:
        document = _parse_schema_document(valuations_schema.instrument_schema(instrument_type))
        assert document.get("title") == instrument_type, (
            f"schema for {instrument_type!r} is titled {document.get('title')!r}"
        )


def test_unknown_instrument_type_raises_key_error() -> None:
    """A missing discriminator is a lookup miss, not a value error."""
    with pytest.raises(KeyError) as excinfo:
        valuations_schema.instrument_schema("not_a_supported_instrument_type")
    assert "instrument_types()" in str(excinfo.value)

    with pytest.raises(KeyError):
        valuations_schema.validate_instrument_type_json(
            "not_a_supported_instrument_type",
            json.dumps({}),
        )


def _bond_example_payload() -> str:
    """Return the worked bond envelope shipped inside the bond schema."""
    document = _parse_schema_document(valuations_schema.instrument_schema("bond"))
    examples = document["examples"]
    assert examples, "bond schema must ship at least one worked example"
    return json.dumps(examples[0])


def test_validators_accept_a_known_good_envelope() -> None:
    """Both validators accept the schema's own worked bond example."""
    payload = _bond_example_payload()
    # Both validators echo the canonical JSON of the accepted payload.
    envelope_echo = valuations_schema.validate_instrument_envelope_json(payload)
    assert json.loads(envelope_echo)["schema"] == "finstack_quant.instrument/1"
    type_echo = valuations_schema.validate_instrument_type_json("bond", payload)
    assert json.loads(type_echo)["instrument"]["type"] == "bond"


def test_validators_reject_a_malformed_envelope() -> None:
    """A structurally valid envelope with an empty spec fails typed validation."""
    malformed = json.dumps({"schema": "finstack_quant.instrument/1", "instrument": {"type": "bond", "spec": {}}})
    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_envelope_json(malformed)
    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_type_json("bond", malformed)


def test_validators_reject_non_json_input() -> None:
    """Undecodable text is reported as bad input, not silently accepted."""
    with pytest.raises(ValueError, match="invalid instrument JSON"):
        valuations_schema.validate_instrument_envelope_json("{not json")
    with pytest.raises(ValueError, match="invalid instrument JSON"):
        valuations_schema.validate_instrument_type_json("bond", "{not json")


def test_cashflow_resources_expose_every_component_schema() -> None:
    """``resources()`` maps canonical ``$id`` URIs to parsable schema text."""
    resources = cashflows_schema.resources()
    assert isinstance(resources, dict)
    assert resources, "cashflow component schemas must not be empty"

    for uri, text in resources.items():
        assert isinstance(uri, str)
        assert uri.startswith(CASHFLOW_SCHEMA_BASE), f"unexpected resource URI: {uri!r}"
        document = _parse_schema_document(text)
        assert document.get("$id") == uri, f"resource key {uri!r} disagrees with its own $id {document.get('$id')!r}"

    assert f"{CASHFLOW_SCHEMA_BASE}schedule_params.schema.json" in resources


@pytest.mark.parametrize(
    ("module_path", "accessor", "relative_path"),
    COMPILED_VS_CHECKED_IN,
    ids=[f"{accessor}" for _, accessor, _ in COMPILED_VS_CHECKED_IN],
)
def test_compiled_schema_matches_checked_in_artifact(
    module_path: str,
    accessor: str,
    relative_path: str,
) -> None:
    """The compiled-in copy equals the generated schema file in the repo.

    This is the drift guard behind the "schemas ship with the wheel" promise:
    if a schema is regenerated on disk but the extension is not rebuilt, the
    two documents diverge and this fails.
    """
    artifact = REPO_ROOT / relative_path
    assert artifact.is_file(), f"missing generated schema artifact: {relative_path}"
    module = SCHEMA_MODULES[module_path]
    text = module.get(accessor) if accessor.endswith(".schema.json") else getattr(module, accessor)()
    compiled = json.loads(text)
    assert compiled == json.loads(artifact.read_text(encoding="utf-8"))


def test_compiled_cashflow_resources_match_checked_in_artifacts() -> None:
    """Every cashflow component schema matches its checked-in artifact."""
    schema_dir = REPO_ROOT / "finstack-quant" / "cashflows" / "schemas" / "cashflow" / "1"
    assert schema_dir.is_dir(), f"missing cashflow schema directory: {schema_dir}"

    resources = cashflows_schema.resources()
    for uri, text in resources.items():
        artifact = schema_dir / uri.removeprefix(CASHFLOW_SCHEMA_BASE)
        assert artifact.is_file(), f"missing generated schema artifact: {artifact}"
        assert json.loads(text) == json.loads(artifact.read_text(encoding="utf-8"))

    on_disk = {path.name for path in schema_dir.glob("*.schema.json")}
    compiled_in = {uri.removeprefix(CASHFLOW_SCHEMA_BASE) for uri in resources}
    assert compiled_in == on_disk, (
        "cashflow resources diverged from the checked-in schema directory.\n"
        f"  not compiled in: {sorted(on_disk - compiled_in)}\n"
        f"  not on disk: {sorted(compiled_in - on_disk)}"
    )


@pytest.mark.parametrize("instrument_type", sorted(valuations_schema.instrument_types()))
def test_compiled_instrument_type_schema_matches_checked_in_artifact(instrument_type: str) -> None:
    """Every per-type instrument schema matches its checked-in artifact.

    Checking only ``bond`` left the other sixty-nine unguarded: a schema
    regenerated on disk without rebuilding the extension would have gone
    unnoticed for every type but one. The artifact path is derived from the
    schema's own ``$id`` rather than hard-coded, so adding an instrument needs
    no change here.
    """
    compiled = _parse_schema_document(valuations_schema.instrument_schema(instrument_type))

    schema_id = compiled.get("$id")
    assert isinstance(schema_id, str), f"{instrument_type} carries no string $id: {schema_id!r}"
    assert schema_id.startswith(INSTRUMENT_SCHEMA_BASE), f"{instrument_type} carries an off-base $id: {schema_id!r}"
    artifact = INSTRUMENT_SCHEMA_DIR / schema_id.removeprefix(INSTRUMENT_SCHEMA_BASE)
    assert artifact.is_file(), f"missing generated schema artifact: {artifact}"
    assert compiled == json.loads(artifact.read_text(encoding="utf-8"))


def test_compiled_instrument_types_cover_the_whole_schema_directory() -> None:
    """The registry and the checked-in schema tree are a bijection.

    This is what stops the parametrized sweep above from silently shrinking: a
    per-type schema that exists on disk but is no longer reachable through
    ``instrument_types()`` (or the reverse) fails here. ``instrument.schema.json``
    is the envelope, served by ``instrument_envelope_schema`` instead.
    """
    ids = {
        instrument_type: json.loads(valuations_schema.instrument_schema(instrument_type))["$id"]
        for instrument_type in valuations_schema.instrument_types()
    }
    assert len(set(ids.values())) == len(ids), "two instrument types share one schema $id"

    compiled_in = {schema_id.removeprefix(INSTRUMENT_SCHEMA_BASE) for schema_id in ids.values()}
    on_disk = {
        str(path.relative_to(INSTRUMENT_SCHEMA_DIR)) for path in INSTRUMENT_SCHEMA_DIR.rglob("*.schema.json")
    } - {"instrument.schema.json"}
    assert compiled_in == on_disk, (
        "instrument registry diverged from the checked-in schema directory.\n"
        f"  not compiled in: {sorted(on_disk - compiled_in)}\n"
        f"  not on disk: {sorted(compiled_in - on_disk)}"
    )


# Validator discrimination: payloads that must be REJECTED


def test_validate_instrument_type_json_rejects_another_types_payload() -> None:
    """A valid bond envelope must not validate against a different type's schema.

    Without this, ``validate_instrument_type_json`` could ignore the
    discriminator entirely and every "accepts a good payload" test above would
    still pass.
    """
    payload = _bond_example_payload()
    echo = valuations_schema.validate_instrument_type_json("bond", payload)
    assert json.loads(echo)["instrument"]["type"] == "bond"
    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_type_json("interest_rate_swap", payload)


def test_validators_reject_an_unknown_field() -> None:
    """Instrument specs deny unknown fields, so a typo cannot be silently dropped."""
    payload = json.loads(_bond_example_payload())
    payload["instrument"]["spec"]["not_a_real_field"] = 1
    text = json.dumps(payload)

    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_envelope_json(text)
    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_type_json("bond", text)


def test_validators_reject_a_bad_scalar() -> None:
    """A wrongly typed field is rejected even though the envelope shape is fine."""
    payload = json.loads(_bond_example_payload())
    payload["instrument"]["spec"]["notional"] = "not-money"
    text = json.dumps(payload)

    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_envelope_json(text)
    with pytest.raises(ValueError, match="validation failed"):
        valuations_schema.validate_instrument_type_json("bond", text)
