"""Registry-backed schema discovery, retrieval and validation.

These three functions are the surface an agent uses to author a payload without
being handed a 158k-token union: list what exists, fetch the one schema that
matters, then get pointer-precise feedback on a near miss.
"""

from __future__ import annotations

import json
from types import ModuleType

import pytest

from finstack_quant.attribution import schema as attribution_schema
from finstack_quant.calibration import schema as calibration_schema
from finstack_quant.cashflows import schema as cashflows_schema
from finstack_quant.core import schema as core_schema
from finstack_quant.margin import schema as margin_schema
from finstack_quant.models.factor import schema as factor_model_schema
from finstack_quant.portfolio import schema as portfolio_schema
from finstack_quant.scenarios import schema as scenarios_schema
from finstack_quant.statements import schema as statements_schema
from finstack_quant.valuations import schema as valuations_schema

# Every crate that owns a schema registry exposes the same three functions.
NAMESPACES = [
    attribution_schema,
    calibration_schema,
    cashflows_schema,
    core_schema,
    factor_model_schema,
    margin_schema,
    portfolio_schema,
    scenarios_schema,
    statements_schema,
    valuations_schema,
]


@pytest.mark.parametrize("namespace", NAMESPACES)
def test_index_describes_every_artifact(namespace: ModuleType) -> None:
    index = json.loads(namespace.index())

    assert index["schema_index_version"] == 1
    assert index["artifacts"], "every schema namespace publishes at least one artifact"
    for row in index["artifacts"]:
        assert set(row) == {"$id", "bytes", "kind", "path", "summary", "title"}
        assert row["kind"] in {"input", "output", "component"}
        assert row["bytes"] > 0
        assert row["summary"]


@pytest.mark.parametrize("namespace", NAMESPACES)
def test_index_is_sorted_by_path(namespace: ModuleType) -> None:
    paths = [row["path"] for row in json.loads(namespace.index())["artifacts"]]
    assert paths == sorted(paths)


@pytest.mark.parametrize("namespace", NAMESPACES)
def test_get_accepts_path_id_and_filename(namespace: ModuleType) -> None:
    row = json.loads(namespace.index())["artifacts"][0]
    by_path = namespace.get(row["path"])

    assert namespace.get(row["$id"]) == by_path
    assert namespace.get(row["path"].rsplit("/", 1)[-1]) == by_path
    assert json.loads(by_path)["$id"] == row["$id"]


def test_get_rejects_an_unknown_selector() -> None:
    with pytest.raises(KeyError):
        valuations_schema.get("no_such_instrument.schema.json")


def test_validate_accepts_a_published_example() -> None:
    example = json.loads(valuations_schema.get("bond.schema.json"))["examples"][0]

    assert json.loads(valuations_schema.validate("bond.schema.json", json.dumps(example))) == []


def test_validate_names_the_field_a_near_miss_is_missing() -> None:
    # Single-instrument schemas used to wrap their payload in a one-branch
    # `oneOf`, which made a validator report against the whole subtree. This is
    # the diagnostic the repair loop depends on.
    payload = json.loads(valuations_schema.get("bond.schema.json"))["examples"][0]
    del payload["instrument"]["spec"]["maturity"]

    failures = json.loads(valuations_schema.validate("bond.schema.json", json.dumps(payload)))

    assert len(failures) == 1
    assert failures[0]["pointer"] == "/instrument/spec"
    assert "maturity" in failures[0]["message"]


def test_validate_resolves_cross_document_references() -> None:
    # Published schemas `$ref` each other by absolute `$id` on a host that does
    # not resolve; validation only works because the whole corpus is registered
    # in process.
    schema = json.loads(valuations_schema.get("bond.schema.json"))
    external = [value for value in json.dumps(schema).split('"') if value.startswith("https://finstack_quant.dev/")]
    assert external, "bond references shared definitions by absolute id"

    example = json.loads(valuations_schema.get("bond.schema.json"))["examples"][0]
    assert json.loads(valuations_schema.validate("bond.schema.json", json.dumps(example))) == []


def test_validate_rejects_a_non_json_payload() -> None:
    with pytest.raises(ValueError, match="payload is not JSON"):
        valuations_schema.validate("bond.schema.json", "{not json")


def test_instrument_catalogue_is_cheap_to_list() -> None:
    # The point of the index: choosing an instrument must not require loading
    # the umbrella union, which is two orders of magnitude larger.
    index = valuations_schema.index()
    umbrella = valuations_schema.get("instruments/1/instrument.schema.json")

    assert len(index) * 20 < len(umbrella)


def test_every_registry_crate_publishes_the_same_surface() -> None:
    # Ten crates own a registry; an eleventh added later must show up here rather
    # than silently shipping without the discovery surface.
    assert len(NAMESPACES) == 10
    for namespace in NAMESPACES:
        # Namespaces that predate the registry also keep their named accessors,
        # so the trio is a floor rather than the whole surface.
        assert {"get", "index", "validate"} <= set(namespace.__all__)


def test_indexes_together_cover_the_whole_published_corpus() -> None:
    paths = {row["path"] for namespace in NAMESPACES for row in json.loads(namespace.index())["artifacts"]}
    assert len(paths) == 116, "the ten indexes must account for every checked-in artifact"


@pytest.mark.parametrize("namespace", NAMESPACES)
def test_llm_profile_is_self_contained(namespace: ModuleType) -> None:
    # Published schemas reference an unresolvable host. The projection exists so
    # a caller can hand one document to a model without anything being fetched.
    for row in json.loads(namespace.index())["artifacts"]:
        projected = namespace.get(row["path"], profile="llm")
        assert '"$ref": "https://' not in projected, row["path"]


def test_llm_profile_flattens_unit_enums() -> None:
    projected = json.loads(valuations_schema.get("money.schema.json", profile="llm"))
    currency = projected["$defs"]["Currency"]

    assert "oneOf" not in currency, "a 159-branch union is not something to decode against"
    assert len(currency["enum"]) > 100
    assert "USD" in currency["enum"]


def test_llm_profile_strips_rust_prose_but_keeps_the_values() -> None:
    canonical = valuations_schema.get("day_count.schema.json")
    projected = valuations_schema.get("day_count.schema.json", profile="llm")

    # The canonical artifact carries rustdoc verbatim; that is the cost being cut.
    assert "```" in canonical
    assert "# Examples" in canonical
    assert "```" not in projected, "code fences do not help a payload author"
    assert "# Examples" not in projected

    # ISDA grounding and every accepted spelling must survive the trim.
    assert "ISDA" in projected, "domain references are the part worth keeping"
    accepted = json.loads(projected)["enum"]
    assert set(accepted) >= {"act_360", "act_365f", "30_360", "act_act", "bus_252"}

    # Each spelling keeps its own citation, so the caller can tell `act_360`
    # from `act_365f` by the section each implements rather than by guessing.
    # That grounding is not free: it is why the projection lands around 6x
    # smaller rather than the 10x achievable by dropping citations with the
    # rest of the prose.
    assert "4.16(d)" in projected, "per-variant section numbers survive"
    assert len(projected) * 5 < len(canonical), f"{len(canonical)} -> {len(projected)}"


def test_get_rejects_an_unknown_profile() -> None:
    with pytest.raises(ValueError, match="unknown schema profile"):
        valuations_schema.get("bond.schema.json", profile="strict")


def test_validate_always_uses_the_canonical_contract() -> None:
    # The projection is both stricter and looser than the runtime contract, so
    # validating against it would silently mis-report.
    example = json.loads(valuations_schema.get("bond.schema.json"))["examples"][0]

    assert json.loads(valuations_schema.validate("bond.schema.json", json.dumps(example))) == []


@pytest.mark.parametrize("namespace", NAMESPACES)
@pytest.mark.parametrize("selector", ["", "json", ".schema.json"])
def test_registry_rejects_partial_selectors(namespace: ModuleType, selector: str) -> None:
    with pytest.raises(KeyError):
        namespace.get(selector)
    with pytest.raises(KeyError):
        namespace.validate(selector, "{}")


@pytest.mark.parametrize("namespace", NAMESPACES)
def test_registry_byte_count_matches_canonical_schema_file(namespace: ModuleType) -> None:
    row = json.loads(namespace.index())["artifacts"][0]
    assert row["bytes"] == len(namespace.get(row["$id"]).encode("utf-8")) + 1
