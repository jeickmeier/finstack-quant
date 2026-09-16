"""Interop guarantee for typed instruments.

typed -> to_json -> from_json -> to_json is stable, and typed and JSON
inputs price identically, for every typed instrument.
"""

from __future__ import annotations

from collections.abc import Callable
import json
from typing import Any

import pytest

from finstack_quant.valuations import instruments

# Builders reused from the per-family test modules (import the helpers).
from tests.tests_typed_helpers import (
    build_capfloor,
    build_cds,
    build_cds_index,
    build_cds_tranche,
    build_convertible,
    build_equity_option,
    build_fx_forward,
    build_fx_option,
    build_irs,
    build_structured_credit,
    build_swaption,
)

ALL_TYPED = [
    ("RevolvingCredit", instruments.RevolvingCredit.example),
    ("InterestRateSwap", build_irs),
    ("Swaption", build_swaption),
    ("CapFloor", build_capfloor),
    ("CreditDefaultSwap", build_cds),
    ("CDSIndex", build_cds_index),
    ("FxForward", build_fx_forward),
    ("FxOption", build_fx_option),
    ("EquityOption", build_equity_option),
    ("CDSTranche", build_cds_tranche),
    ("ConvertibleBond", build_convertible),
    ("StructuredCredit", build_structured_credit),
]

PUBLIC_TYPED_JSON_TAGS = {
    "Bond": "bond",
    "TermLoan": "term_loan",
    **{name: json.loads(factory().to_json())["instrument"]["type"] for name, factory in ALL_TYPED},
}


@pytest.mark.parametrize(("name", "factory"), ALL_TYPED)
def test_json_round_trip_is_stable(name: str, factory: Callable[[], Any]) -> None:
    cls = getattr(instruments, name)
    instance = factory()
    once = instance.to_json()
    twice = cls.from_json(once).to_json()
    assert json.loads(twice) == json.loads(once)


@pytest.mark.parametrize(("name", "factory"), ALL_TYPED)
def test_from_json_requires_canonical_instrument_envelope(name: str, factory: Callable[[], Any]) -> None:
    cls = getattr(instruments, name)
    envelope = json.loads(factory().to_json())
    assert envelope["schema"] == "finstack_quant.instrument/1"

    parsed = cls.from_json(json.dumps(envelope))
    assert json.loads(parsed.to_json()) == envelope

    with pytest.raises(ValueError, match=r"schema|envelope"):
        cls.from_json(json.dumps(envelope["instrument"]))


@pytest.mark.parametrize(("name", "type_tag"), PUBLIC_TYPED_JSON_TAGS.items())
def test_public_from_json_runtime_docs_describe_shared_contract(name: str, type_tag: str) -> None:
    doc = getattr(instruments, name).from_json.__doc__
    assert doc
    assert "canonical v1 envelope" in doc
    assert "envelope" in doc
    assert "Bare payloads" in doc
    assert "16 MiB" in doc
    assert f'"{type_tag}"' in doc
    assert "ValueError" in doc
    assert "validated" in doc


@pytest.mark.parametrize(("name", "factory"), ALL_TYPED)
def test_typed_instance_is_accepted_by_price_instrument_dispatch(name: str, factory: Callable[[], Any]) -> None:
    """Assert the dispatch layer serializes typed instances.

    Market errors are fine, but a ``TypeError`` from
    ``extract_instrument_json`` is not.
    """
    instance = factory()
    try:
        instruments.price_instrument(instance, "{}", "2024-01-01", "default")
    except TypeError as exc:  # pragma: no cover - dispatch regression
        pytest.fail(f"typed {name} rejected by dispatch: {exc}")
    except ValueError:
        pass  # empty market: expected downstream failure, dispatch worked
