"""Field-level parity for the typed structured-credit surface.

The published ``structured_credit.schema.json`` lists every public Rust field
of ``StructuredCredit``, ``Tranche``, ``AssetPool``, ``PoolAsset`` and
``Waterfall``. Each must be readable from the matching Python class, and each
settable field must have a builder setter or a ``with_*`` method, so a deal
can be assembled and inspected without touching JSON.
"""

from __future__ import annotations

import json
from typing import Any

import pytest

from finstack_quant import schema
from finstack_quant.valuations.instruments import (
    AdvanceRate,
    AssetPool,
    BalloonSpec,
    BorrowingBaseRules,
    ConcentrationLimit,
    EligibilityRule,
    LiquidationSpec,
    PoolAsset,
    SpecialServicingSpec,
    StructuredCredit,
    StructuredCreditBuilder,
    TermOutSpec,
    Tranche,
    TrancheBuilder,
    Waterfall,
)

# Rust-only by design (mirrors the other typed instrument builders): the three
# pricing-override bags are supplied through ``pricing_options`` on pricing.
RUST_ONLY_DEAL_FIELDS = {
    "instrument_pricing_overrides",
    "metric_pricing_overrides",
    "scenario_pricing_overrides",
}
# Assigned by ``TrancheStructure`` (payment_priority) or fixed by the engine
# (behavior_type); readable but not settable.
STRUCTURE_ASSIGNED_TRANCHE_FIELDS = {"behavior_type", "payment_priority"}
# ``AssetPool`` setters are ``with_*`` methods on the immutable pool.
POOL_SETTERS = {
    "assets": "with_assets",
    "rep_lines": "with_rep_lines",
    "instruments": "with_instruments",
    "reserve_account": "with_reserve",
    "reserve_account_rate": "with_reserve",
    "reserve_target": "with_reserve",
    "reserve_interest_destination": "with_reserve",
    "reinvestment_period": "with_reinvestment_period",
    "cumulative_defaults": "with_accounts",
    "cumulative_recoveries": "with_accounts",
    "cumulative_prepayments": "with_accounts",
    "cumulative_scheduled_amortization": "with_accounts",
    "collection_account": "with_accounts",
    "excess_spread_account": "with_accounts",
    "original_balance": "with_accounts",
}
POOL_CONSTRUCTOR_FIELDS = {"id", "deal_type", "base_currency"}


def _fields(definition: str, document_name: str = "structured_credit.schema.json") -> set[str]:
    document: dict[str, Any] = json.loads(schema.get(document_name))
    return set(document["$defs"][definition]["properties"])


def _readable(cls: type, name: str) -> bool:
    attribute = getattr(cls, name, None)
    return isinstance(attribute, property) or attribute is not None


@pytest.mark.parametrize(
    ("definition", "cls"),
    [
        ("StructuredCredit", StructuredCredit),
        ("Tranche", Tranche),
        ("AssetPool", AssetPool),
        ("PoolAsset", PoolAsset),
        ("Waterfall", Waterfall),
        ("BalloonSpec", BalloonSpec),
        ("SpecialServicingSpec", SpecialServicingSpec),
        ("LiquidationSpec", LiquidationSpec),
        ("BorrowingBaseRules", BorrowingBaseRules),
        ("AdvanceRate", AdvanceRate),
        ("EligibilityRule", EligibilityRule),
        ("ConcentrationLimit", ConcentrationLimit),
    ],
)
def test_every_public_rust_field_is_readable(definition: str, cls: type) -> None:
    """Every schema property of the Rust type is a Python getter."""
    expected = _fields(definition)
    if definition == "StructuredCredit":
        expected -= RUST_ONLY_DEAL_FIELDS
    missing = sorted(name for name in expected if not _readable(cls, name))
    assert missing == [], f"{definition} fields without a Python getter: {missing}"


def test_every_deal_field_has_a_builder_setter() -> None:
    """``StructuredCreditBuilder`` carries one setter per settable Rust field."""
    expected = _fields("StructuredCredit") - RUST_ONLY_DEAL_FIELDS
    missing = sorted(name for name in expected if not callable(getattr(StructuredCreditBuilder, name, None)))
    assert missing == [], f"StructuredCreditBuilder setters missing: {missing}"


def test_every_tranche_field_has_a_builder_setter() -> None:
    """``TrancheBuilder`` carries one setter per settable Rust field."""
    expected = _fields("Tranche") - STRUCTURE_ASSIGNED_TRANCHE_FIELDS
    # ``coupon`` is set through the two typed variants.
    expected -= {"coupon"}
    missing = sorted(name for name in expected if not callable(getattr(TrancheBuilder, name, None)))
    assert missing == [], f"TrancheBuilder setters missing: {missing}"
    assert callable(TrancheBuilder.coupon_fixed)
    assert callable(TrancheBuilder.coupon_floating)


def test_every_pool_field_has_a_constructor_argument_or_with_method() -> None:
    """``AssetPool`` fields are set by the constructor or a ``with_*`` method."""
    expected = _fields("AssetPool") - POOL_CONSTRUCTOR_FIELDS
    unmapped = sorted(expected - set(POOL_SETTERS))
    assert unmapped == [], f"AssetPool fields without a setter mapping: {unmapped}"
    missing = sorted(setter for setter in set(POOL_SETTERS.values()) if not callable(getattr(AssetPool, setter, None)))
    assert missing == [], f"AssetPool setters missing: {missing}"


def test_every_pool_asset_field_is_a_constructor_keyword() -> None:
    """``PoolAsset.__init__`` accepts every Rust field by name."""
    signature = PoolAsset.__text_signature__ or ""
    names = {part.split("=")[0].strip() for part in signature.strip("()").split(",") if part.strip() not in {"*", ""}}
    missing = sorted(_fields("PoolAsset") - names)
    assert missing == [], f"PoolAsset constructor keywords missing: {missing}"


def test_every_term_out_field_is_readable() -> None:
    """The facility's ``TermOutSpec`` fields are Python getters."""
    expected = _fields("TermOutSpec", "asset_backed_facility.schema.json")
    missing = sorted(name for name in expected if not _readable(TermOutSpec, name))
    assert missing == [], f"TermOutSpec fields without a Python getter: {missing}"
