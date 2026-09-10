"""Typed CreditDefaultSwap / CDSIndex bindings + CDS typed-vs-JSON golden."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import (
    CDSIndex,
    CreditDefaultSwap,
    PremiumLegSpec,
    ProtectionLegSpec,
    TermLoan,
    price_instrument,
)
from tests.tests_typed_helpers import build_cds as _cds, cds_legs as _legs

# Every canonical snake_case serde wire value of the Rust `StubKind` enum.
# Kept in sync manually with
# `finstack-quant/core/src/dates/schedule_iter.rs`; a stub `Literal` that
# drifts from this set is exactly the bug this test guards against.
_VALID_STUB_VALUES = (
    "none",
    "short_front",
    "short_back",
    "long_front",
    "long_back",
)


def _market_json() -> str:
    return json.dumps({
        "schema_version": 1,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "act_360",
                "knot_points": [[0.0, 1.0], [5.0, 0.90], [10.0, 0.80]],
                "interp_style": "monotone_convex",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            },
            {
                "type": "hazard",
                "id": "ACME-CDS",
                "base": "2024-01-01",
                "knot_points": [[0.0, 0.02], [5.0, 0.02]],
                "recovery_rate": 0.4,
                "issuer": None,
                "seniority": None,
                "currency": None,
                "day_count": "act_365f",
                "par_points": [],
            },
        ],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
        "hierarchy": None,
    })


def _cds_hand_written_json() -> str:
    """Hand-authored instrument envelope for the same economic CDS as `_cds`.

    Deliberately NOT derived from `CreditDefaultSwap.to_json()` — this is the
    independent side of the typed-vs-JSON golden. If Rust's envelope
    serializer ever mis-maps a field, this literal (built directly from the
    canonical `PremiumLegSpec`/`ProtectionLegSpec`/`CreditDefaultSwap` struct
    definitions and their canonical serde wire names) will diverge from the
    typed path instead of silently matching it.
    """
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "credit_default_swap",
            "spec": {
                "id": "CDS-1",
                "notional": {"amount": "10000000", "currency": "USD"},
                "side": "pay",
                "convention": "isda_na",
                "premium": {
                    "standard_imm_dates": True,
                    "start": "2024-03-20",
                    "end": "2029-06-20",
                    "frequency": {"count": 3, "unit": "months"},
                    "stub": "short_front",
                    "business_day_convention": "modified_following",
                    "calendar_id": None,
                    "day_count": "act_360",
                    "spread_bp": "100",
                    "discount_curve_id": "USD-OIS",
                },
                "protection": {
                    "credit_curve_id": "ACME-CDS",
                    "recovery_rate": 0.4,
                    "settlement_delay": 3,
                },
                "valuation_convention": "bloomberg_cdsw_clean",
                "attributes": {},
            },
        },
    })


def _without_timestamp(result: ValuationResult) -> dict[str, object]:
    parsed = json.loads(result.to_json())
    parsed["meta"].pop("timestamp", None)
    return parsed


class TestCreditDefaultSwapTyped:
    def test_envelope_json_and_round_trip(self) -> None:
        payload = json.loads(_cds().to_json())
        assert payload["instrument"]["type"] == "credit_default_swap"
        assert CreditDefaultSwap.from_json(_cds().to_json()).id == "CDS-1"

    def test_invalid_convention_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid convention"):
            CreditDefaultSwap.builder().convention("bespoke")

    def test_protection_leg_recovery_validated(self) -> None:
        with pytest.raises(ValueError, match="Recovery rate"):
            ProtectionLegSpec("ACME-CDS", 1.5, 3)

    def test_from_json_rejects_wrong_type(self) -> None:
        with pytest.raises(ValueError, match="credit_default_swap"):
            CreditDefaultSwap.from_json(TermLoan.example().to_json())

    def test_builder_missing_required_field_raises(self) -> None:
        with pytest.raises(ValueError, match="missing required field"):
            CreditDefaultSwap.builder().id("CDS-BAD").build()

    @pytest.mark.parametrize("value", ["pay", "receive"])
    def test_every_side_literal_value_accepted(self, value: str) -> None:
        premium, protection = _legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-SIDE")
            .notional(Money(1_000_000.0, Currency("USD")))
            .side(value)
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .build()
        )
        assert cds.id == "CDS-SIDE"

    @pytest.mark.parametrize("value", ["isda_na", "isda_eu", "isda_as", "custom"])
    def test_every_convention_literal_value_accepted(self, value: str) -> None:
        premium, protection = _legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-CONV")
            .notional(Money(1_000_000.0, Currency("USD")))
            .side("pay")
            .convention(value)
            .premium(premium)
            .protection(protection)
            .build()
        )
        assert cds.id == "CDS-CONV"

    @pytest.mark.parametrize(
        "value",
        ["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"],
    )
    def test_every_doc_clause_literal_value_accepted(self, value: str) -> None:
        premium, protection = _legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-DOC")
            .notional(Money(1_000_000.0, Currency("USD")))
            .side("pay")
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .doc_clause(value)
            .build()
        )
        payload = json.loads(cds.to_json())
        assert payload["instrument"]["spec"]["doc_clause"] == value

    def test_invalid_doc_clause_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid doc_clause"):
            CreditDefaultSwap.builder().doc_clause("bespoke")

    def test_doc_clause_wrong_case_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid doc_clause"):
            CreditDefaultSwap.builder().doc_clause("CR14")

    def test_convention_wrong_case_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid convention"):
            CreditDefaultSwap.builder().convention("ISDA_NA")

    def test_protection_effective_date_setter(self) -> None:
        premium, protection = _legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-FWD")
            .notional(Money(1_000_000.0, Currency("USD")))
            .side("pay")
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .protection_effective_date(datetime.date(2024, 6, 20))
            .build()
        )
        payload = json.loads(cds.to_json())
        assert payload["instrument"]["spec"]["protection_effective_date"] == "2024-06-20"

    def test_builder_setters_accept_keyword_value(self) -> None:
        """Every builder setter's `value` parameter name must match its text_signature."""
        premium, protection = _legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id(value="CDS-KW")
            .notional(value=Money(1_000_000.0, Currency("USD")))
            .side(value="pay")
            .convention(value="isda_na")
            .premium(value=premium)
            .protection(value=protection)
            .build()
        )
        assert cds.id == "CDS-KW"

    def test_golden_typed_pv_equals_json_pv(self) -> None:
        """Payer CDS: typed path and an independently hand-written JSON payload produce identical ValuationResult.

        The JSON side is NOT derived from ``cds.to_json()`` (that would make
        both sides call the same Rust serializer, so a bug inside
        the envelope serializer's own field mapping would be identically wrong on
        both sides and pass). See `_cds_hand_written_json`.
        """
        cds = _cds()
        typed = price_instrument(cds, _market_json(), "2024-01-01", "hazard_rate")
        via_json = price_instrument(_cds_hand_written_json(), _market_json(), "2024-01-01", "hazard_rate")
        assert _without_timestamp(typed) == _without_timestamp(via_json)


class TestPremiumLegSpecTyped:
    @pytest.mark.parametrize("stub", _VALID_STUB_VALUES)
    def test_every_stub_literal_value_accepted(self, stub: str) -> None:
        """Every value in the `stub` Literal (including `"none"`) must be a real accepted wire value."""
        leg = PremiumLegSpec(
            datetime.date(2024, 3, 20),
            datetime.date(2029, 6, 20),
            Tenor.quarterly(),
            DayCount.ACT_360,
            100.0,
            "USD-OIS",
            stub=stub,
            standard_imm_dates=False,
        )
        assert "spread_bp=100" in repr(leg)


class TestCDSIndexTyped:
    def test_builder_round_trip(self) -> None:
        premium, protection = _legs()
        index = (
            CDSIndex
            .builder()
            .id("CDX-IG-42")
            .index_name("CDX.NA.IG")
            .series(42)
            .version(1)
            .notional(Money(10_000_000.0, Currency("USD")))
            .index_factor(1.0)
            .side("pay")
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .pricing("single_curve")
            .num_constituents(125)
            .build()
        )
        payload = json.loads(index.to_json())
        assert payload["instrument"]["type"] == "cds_index"
        assert CDSIndex.from_json(index.to_json()).id == "CDX-IG-42"

    @pytest.mark.parametrize("value", ["isda_na", "isda_eu", "isda_as", "custom"])
    def test_every_convention_literal_value_accepted(self, value: str) -> None:
        premium, protection = _legs()
        index = (
            CDSIndex
            .builder()
            .id("CDX-CONV")
            .index_name("CDX.NA.IG")
            .series(42)
            .version(1)
            .notional(Money(10_000_000.0, Currency("USD")))
            .index_factor(1.0)
            .side("pay")
            .convention(value)
            .premium(premium)
            .protection(protection)
            .pricing("single_curve")
            .num_constituents(125)
            .build()
        )
        assert index.id == "CDX-CONV"

    @pytest.mark.parametrize("value", ["single_curve", "constituents"])
    def test_every_pricing_literal_value_accepted(self, value: str) -> None:
        premium, protection = _legs()
        builder = (
            CDSIndex
            .builder()
            .id("CDX-PRICING")
            .index_name("CDX.NA.IG")
            .series(42)
            .version(1)
            .notional(Money(10_000_000.0, Currency("USD")))
            .index_factor(1.0)
            .side("pay")
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .pricing(value)
        )
        if value == "constituents":
            builder = builder.constituents(
                json.dumps([
                    {
                        "credit": {
                            "reference_entity": "ACME-CORP",
                            "recovery_rate": 0.4,
                            "credit_curve_id": "ACME-HZD",
                        },
                        "weight": 1.0,
                        "defaulted": False,
                    }
                ])
            )
        index = builder.build()
        payload = json.loads(index.to_json())
        assert payload["instrument"]["spec"]["pricing"] == value

    def test_invalid_pricing_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid pricing"):
            CDSIndex.builder().pricing("SingleCurve")  # schema-rejection-test

    def test_invalid_convention_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid convention"):
            CDSIndex.builder().convention("bespoke")

    def test_from_json_rejects_wrong_type(self) -> None:
        with pytest.raises(ValueError, match="cds_index"):
            CDSIndex.from_json(TermLoan.example().to_json())

    def test_builder_missing_required_field_raises(self) -> None:
        with pytest.raises(ValueError, match="missing required field"):
            CDSIndex.builder().id("CDX-BAD").build()

    def test_builder_setters_accept_keyword_value(self) -> None:
        """Every builder setter's `value` parameter name must match its text_signature."""
        premium, protection = _legs()
        index = (
            CDSIndex
            .builder()
            .id(value="CDX-KW")
            .index_name(value="CDX.NA.IG")
            .series(value=42)
            .version(value=1)
            .notional(value=Money(10_000_000.0, Currency("USD")))
            .index_factor(value=1.0)
            .side(value="pay")
            .convention(value="isda_na")
            .premium(value=premium)
            .protection(value=protection)
            .pricing(value="single_curve")
            .num_constituents(value=125)
            .build()
        )
        assert index.id == "CDX-KW"
