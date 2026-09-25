"""Typed structured-credit spec models and StructuredCredit deal."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    MarketContext,
    ScalarTimeSeries,
)
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetPool,
    StructuredCredit,
    Tranche,
)
from tests.tests_typed_helpers import (
    canonical_structured_credit_json,
    structured_credit_pool as _pool,
    structured_credit_tranches as _tranches,
)


def _valid_deal_json() -> str:
    return canonical_structured_credit_json()


def _metrics_tranche_id(deal_json: str) -> str:
    """Return the first non-equity tranche from the canonical fixture."""
    tranches = json.loads(deal_json)["instrument"]["spec"]["tranches"]["tranches"]
    for tranche in tranches:
        if tranche.get("seniority") != "equity":
            return tranche["id"]
    return tranches[0]["id"]


def _market() -> MarketContext:
    as_of = datetime.date(2024, 1, 1)
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
        .insert(
            ForwardCurve(
                "SOFR-3M",
                0.25,
                as_of,
                [(0.0, 0.04), (10.0, 0.04)],
                day_count="act_360",
            )
        )
    )
    market.insert_series(ScalarTimeSeries("FIXING:SOFR-3M", [(datetime.date(2023, 12, 28), 0.04)]))
    return market


class TestStructuredCreditTyped:
    def test_new_abs_round_trips(self) -> None:
        deal = StructuredCredit.new_abs(
            "ABS-1",
            _pool(),
            _tranches(),
            datetime.date(2024, 1, 15),
            datetime.date(2031, 1, 15),
            "USD-SOFR-DISC",
        )
        payload = json.loads(deal.to_json())
        assert payload["instrument"]["type"] == "structured_credit"
        # The typed constructor and serde wire both use canonical snake_case.
        assert payload["instrument"]["spec"]["deal_type"] == "abs"
        assert StructuredCredit.from_json(deal.to_json()).id == "ABS-1"

    def test_tranche_builder_validates_attach_detach(self) -> None:
        with pytest.raises(ValueError, match="Invalid"):
            (
                Tranche
                .builder()
                .id("BAD")
                .attach_pct(50.0)
                .detach_pct(10.0)
                .seniority("senior")
                .original_balance(Money(1.0, Currency("USD")))
                .coupon_fixed(0.05)
                .maturity(datetime.date(2031, 1, 15))
                .build()
            )

    def test_deal_type_string_is_validated(self) -> None:
        with pytest.raises(ValueError, match="invalid deal_type"):
            AssetPool("POOL-X", "cdo_squared", Currency("USD"))

    def test_wire_casing_round_trips_without_translation(self) -> None:
        """The typed surface must accept exactly the strings `to_json()` emits.

        `DealType` and `TrancheSeniority` use canonical snake_case at their
        serde source. This asserts the round trip end to end: build a deal,
        serialize it, pull the `deal_type` and a tranche `seniority` straight
        out of that JSON, and feed those exact strings back into the typed
        constructors without a translation shim.
        """
        deal = StructuredCredit.new_abs(
            "ABS-1",
            _pool(),
            _tranches(),
            datetime.date(2024, 1, 15),
            datetime.date(2031, 1, 15),
            "USD-SOFR-DISC",
        )
        payload = json.loads(deal.to_json())
        deal_type_wire = payload["instrument"]["spec"]["deal_type"]
        tranche_seniority_wire = payload["instrument"]["spec"]["tranches"]["tranches"][0]["seniority"]

        # Feed the exact wire strings straight back into the typed
        # constructors: this must succeed without any casing translation.
        AssetPool("POOL-RT", deal_type_wire, Currency("USD"))
        Tranche.builder().seniority(tranche_seniority_wire)
        StructuredCredit.builder().deal_type(deal_type_wire)

    def test_all_deal_type_literal_values_accepted(self) -> None:
        for value in ("clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"):
            AssetPool("POOL-DT", value, Currency("USD"))
            StructuredCredit.builder().deal_type(value)

    def test_all_seniority_literal_values_accepted(self) -> None:
        for value in ("senior", "mezzanine", "subordinated", "equity"):
            Tranche.builder().seniority(value)

    def test_uppercase_deal_type_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid deal_type"):
            AssetPool("POOL-LC", "ABS", Currency("USD"))  # schema-rejection-test
        with pytest.raises(ValueError, match="invalid deal_type"):
            StructuredCredit.builder().deal_type("ABS")  # schema-rejection-test

    def test_pascal_case_seniority_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid seniority"):
            Tranche.builder().seniority("Senior")  # schema-rejection-test

    def test_from_json_accepts_repo_fixture(self) -> None:
        from pathlib import Path

        fixture = (
            Path(__file__).resolve().parents[2]
            / "finstack-quant"
            / "valuations"
            / "tests"
            / "instruments"
            / "json_examples"
            / "structured_credit.json"
        )
        envelope = json.loads(fixture.read_text())
        deal = StructuredCredit.from_json(json.dumps(envelope))
        assert json.loads(deal.to_json())["instrument"]["type"] == "structured_credit"


class TestStructuredCreditTypedAnalytics:
    def test_typed_metrics_equal_json_metrics_on_fixture(self) -> None:
        """Golden: typed deal in == JSON deal in, identical TrancheMetrics."""
        from finstack_quant.valuations.instruments import (
            StructuredCredit,
            TrancheMetrics,
            structured_credit_tranche_metrics,
        )

        deal_json = _valid_deal_json()
        tranche_id = _metrics_tranche_id(deal_json)
        market = _market()

        via_json = structured_credit_tranche_metrics(deal_json, tranche_id, market, "2024-01-01")
        typed_deal = StructuredCredit.from_json(deal_json)
        via_typed = structured_credit_tranche_metrics(typed_deal, tranche_id, market, "2024-01-01")
        # Both sides serialize the same Rust value directly (no parse cycle),
        # so the wire payloads must match exactly.
        assert json.loads(via_typed.to_json()) == json.loads(via_json.to_json())

        assert via_typed.tranche_id == tranche_id

        # Field-by-field with pytest.approx rather than exact string/dict
        # equality: serde_json's default (non-`float_roundtrip`) float parser
        # is not always bit-exact on reparse, so a few of this fixture's
        # values shift by 1 ULP across a parse -> reserialize cycle. That is
        # an unrelated, pre-existing serde_json characteristic (present for
        # every f64-bearing struct in this workspace) rather than anything
        # about typed-vs-JSON parity, which the assert above already proved.
        decoded = json.loads(via_typed.to_json())
        reencoded = json.loads(TrancheMetrics.from_json(via_typed.to_json()).to_json())
        assert reencoded.keys() == decoded.keys()
        for key, expected in decoded.items():
            if isinstance(expected, float):
                assert reencoded[key] == pytest.approx(expected)
            else:
                assert reencoded[key] == expected

    def test_oas_result_wrapper_round_trips(self) -> None:
        from finstack_quant.valuations.instruments import OasResult

        payload = json.dumps({
            "oas": 0.0125,
            "model_price": 99.5,
            "market_price": 98.75,
            "num_paths": 256,
            "price_std_error": 0.05,
        })
        result = OasResult.from_json(payload)
        assert result.oas == pytest.approx(0.0125)
        assert json.loads(result.to_json()) == json.loads(payload)

    def test_scenario_table_wrapper_exposes_cells(self) -> None:
        from finstack_quant.valuations.instruments import ScenarioTable

        payload = json.dumps({
            "tranche_id": "A",
            "cells": [
                {
                    "cpr": 0.06,
                    "cdr": 0.02,
                    "severity": 0.6,
                    "price": 98.2,
                    "wal": 4.1,
                    "writedown": 0.0,
                }
            ],
        })
        table = ScenarioTable.from_json(payload)
        assert table.tranche_id == "A"
        assert table.cells()[0]["price"] == pytest.approx(98.2)
