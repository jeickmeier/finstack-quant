"""Typed asset-backed facility (warehouse line) surface.

The facility is a synthetic two-class structured-credit deal; the typed class
must expose the borrowing base, the projection and the registry metrics with
the same numbers the generic pricing entry point produces.
"""

from __future__ import annotations

import datetime
import json
import pickle

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetBackedFacility,
    AssetBackedFacilityBuilder,
    Bond,
    FacilityProjection,
    StructuredCredit,
    list_standard_metrics,
    price_instrument,
)

CLOSE = datetime.date(2024, 1, 15)
USD = Currency("USD")


def _market() -> MarketContext:
    return MarketContext().insert(DiscountCurve.flat("USD-OIS", CLOSE, 0.04))


def _facility(drawn: float = 60_000_000.0) -> AssetBackedFacility:
    return (
        AssetBackedFacility
        .builder()
        .id("WH-1")
        .collateral(AssetBackedFacility.example().collateral)
        .borrowing_base_rules({
            "advance_rates": [{"asset_class": "*", "rate": 0.8}],
            "concentration_limits": [{"scope": "obligor", "max_pct": 20.0}],
        })
        .commitment(Money(80_000_000.0, USD))
        .drawn(drawn, currency="USD")
        .margin_bp(600.0)
        .unused_fee_bp(50.0)
        .closing_date(CLOSE)
        .revolving_end(datetime.date(2026, 1, 15))
        .maturity(datetime.date(2030, 1, 15))
        .frequency("3M")
        .day_count("act_360")
        .payment_calendar_id("nyse")
        .term_out(24)
        .discount_curve_id("USD-OIS")
        .build()
    )


def test_example_round_trips_through_json_dict_and_pickle() -> None:
    facility = AssetBackedFacility.example()
    assert facility.id == "ABF-EXAMPLE"
    again = AssetBackedFacility.from_json(facility.to_json())
    assert again.to_dict() == facility.to_dict()
    assert pickle.loads(pickle.dumps(facility)).to_json() == facility.to_json()  # noqa: S301
    assert facility.to_dict()["commitment"]["currency"] == "USD"
    assert facility.default_model == "discounting"
    assert facility.market_dependencies()["curves"]["discount_curves"] == ["USD-OIS"]
    with pytest.raises(ValueError, match="asset_backed_facility"):
        AssetBackedFacility.from_json(Bond.example().to_json())


def test_builder_exposes_every_field_and_rejects_an_overdrawn_line() -> None:
    facility = _facility()
    assert isinstance(AssetBackedFacility.builder(), AssetBackedFacilityBuilder)
    assert facility.commitment.amount == 80_000_000.0
    assert facility.drawn.amount == 60_000_000.0
    assert facility.undrawn.amount == 20_000_000.0
    assert facility.index_id is None
    assert facility.margin_bp == 600.0
    assert facility.unused_fee_bp == 50.0
    assert facility.closing_date == CLOSE
    assert facility.revolving_end == datetime.date(2026, 1, 15)
    assert facility.effective_revolving_end == datetime.date(2026, 1, 15)
    assert facility.repayment_date == datetime.date(2028, 1, 15)
    assert facility.maturity == datetime.date(2030, 1, 15)
    assert str(facility.frequency) == "3M"
    assert str(facility.day_count) == "act_360"
    assert facility.payment_calendar_id == "nyse"
    assert facility.amortization_events == []
    assert facility.term_out == {"months": 24}
    assert facility.liquidation_price_pct is None
    assert facility.discount_curve_id == "USD-OIS"
    assert facility.borrowing_base_rules["advance_rates"][0]["rate"] == 0.8
    assert facility.credit_model["prepayment_spec"] == json.loads(facility.prepayment_spec.to_json())
    assert facility.collateral.id == AssetBackedFacility.example().collateral.id

    with pytest.raises(ValueError, match="drawn"):
        _facility(drawn=90_000_000.0)

    builder = AssetBackedFacility.builder().id("WH-2")
    with pytest.raises(ValueError, match="required"):
        builder.build()
    with pytest.raises(ValueError, match="consumed"):
        builder.id("WH-3")


def test_amortization_events_pull_the_revolving_end_forward() -> None:
    facility = (
        AssetBackedFacility
        .builder()
        .id("WH-EV")
        .collateral(AssetBackedFacility.example().collateral)
        .borrowing_base_rules({"advance_rates": [{"asset_class": "*", "rate": 0.8}]})
        .commitment(80_000_000.0, currency="USD")
        .drawn(60_000_000.0, currency="USD")
        .margin_bp(600.0)
        .closing_date(CLOSE)
        .revolving_end(datetime.date(2026, 1, 15))
        .maturity(datetime.date(2030, 1, 15))
        .frequency("3M")
        .payment_calendar_id("nyse")
        .amortization_events([
            {"kind": "date", "date": "2025-01-15"},
            {"kind": "cumulative_loss", "max_pct": 5.0},
        ])
        .discount_curve_id("USD-OIS")
        .build()
    )
    assert facility.effective_revolving_end == datetime.date(2025, 1, 15)
    assert facility.repayment_date == datetime.date(2030, 1, 15)
    assert [event["kind"] for event in facility.amortization_events] == ["date", "cumulative_loss"]


def test_borrowing_base_report_matches_the_registry_metric() -> None:
    facility = _facility()
    report = facility.borrowing_base()
    base = float(report["borrowing_base"]["amount"])
    assert base > 0.0
    assert float(report["eligible_collateral"]["amount"]) >= base
    assert facility.metric(_market(), CLOSE, "abf_borrowing_base") == pytest.approx(base)
    assert facility.metric(_market(), CLOSE, "abf_advance_rate_utilization") == pytest.approx(60_000_000.0 / base)
    assert facility.metric(_market(), CLOSE, "abf_borrowing_base_cushion") == pytest.approx(
        (base - 60_000_000.0) / base * 100.0
    )


def test_projection_reconciles_with_the_lender_cashflows_and_the_price() -> None:
    facility = _facility()
    projection = facility.project(_market(), CLOSE)
    assert isinstance(projection, FacilityProjection)
    frame = projection.to_dataframe()
    assert list(frame.columns) == ["date", "interest", "principal", "unused_fee", "draw", "lender_total", "residual"]
    lender = projection.lender_cashflows
    assert frame["lender_total"].sum() == pytest.approx(sum(amount.amount for _, amount in lender))
    assert frame["principal"].sum() == pytest.approx(60_000_000.0, rel=1e-9)
    assert frame["unused_fee"].sum() == pytest.approx(sum(amount.amount for _, amount in projection.unused_fees))
    assert projection.unused_fees[0][1].amount > 0.0
    assert projection.facility.tranche_id == "FACILITY"
    assert projection.residual.tranche_id == "RESIDUAL"
    assert projection.diagnostics.periods
    assert FacilityProjection.from_json(projection.to_json()).to_dict() == projection.to_dict()
    assert pickle.loads(pickle.dumps(projection)).to_json() == projection.to_json()  # noqa: S301

    irr = facility.facility_irr(_market(), CLOSE)
    assert 0.055 < irr < 0.075
    assert facility.metric(_market(), CLOSE, "abf_facility_irr") == pytest.approx(irr)

    typed = facility.price(_market(), CLOSE, metrics=["dv01"])
    generic = price_instrument(facility.to_json(), _market(), CLOSE, metrics=["dv01"])
    assert float(typed.price) == pytest.approx(float(generic.price))
    assert typed.metrics["dv01"] == pytest.approx(generic.metrics["dv01"])
    assert float(typed.price) > 0.0


def test_synthesized_deal_is_a_two_class_structure_with_a_borrowing_base_test() -> None:
    deal = _facility().synthesized_deal()
    assert isinstance(deal, StructuredCredit)
    ids = [tranche.id for tranche in deal.tranches.tranches]
    assert ids == ["RESIDUAL", "FACILITY"]
    kinds = {test["kind"] for test in deal.create_waterfall().coverage_tests()}
    assert "borrowing_base" in kinds


def test_facility_metrics_are_registered() -> None:
    metrics = set(list_standard_metrics())
    assert {
        "abf_borrowing_base",
        "abf_borrowing_base_cushion",
        "abf_advance_rate_utilization",
        "abf_facility_irr",
        "abf_residual_irr",
    } <= metrics


def test_every_amortization_event_kind_projects_with_documented_units() -> None:
    """Percent loss thresholds, an excess-spread floor and a dated event all project."""
    base = _facility()
    for events in (
        [{"kind": "cumulative_loss", "max_pct": 4.0}],
        [{"kind": "excess_spread", "min_3m": 0.01}],
        [{"kind": "date", "date": "2025-01-15"}],
    ):
        facility = (
            AssetBackedFacility
            .builder()
            .id("WH-EVT")
            .collateral(base.collateral)
            .borrowing_base_rules(base.borrowing_base_rules)
            .commitment(base.commitment)
            .drawn(base.drawn)
            .margin_bp(600.0)
            .unused_fee_bp(50.0)
            .closing_date(CLOSE)
            .revolving_end(datetime.date(2026, 1, 15))
            .maturity(datetime.date(2030, 1, 15))
            .frequency("3M")
            .payment_calendar_id("nyse")
            .term_out(24)
            .amortization_events(events)
            .discount_curve_id("USD-OIS")
            .build()
        )
        projection = facility.project(_market(), CLOSE)
        assert projection.facility.total_principal.amount > 0.0
    dated = facility
    assert dated.effective_revolving_end == datetime.date(2025, 1, 15)
    assert all(date <= datetime.date(2025, 4, 30) for date, _ in projection.unused_fees)


def test_draws_fees_and_readvance_round_trip_and_project() -> None:
    """Scheduled draws lift the facility balance; the projection reports them."""
    base = _facility()
    facility = (
        AssetBackedFacility
        .builder()
        .id("WH-DRAW")
        .collateral(base.collateral)
        .borrowing_base_rules(base.borrowing_base_rules)
        .commitment(base.commitment)
        .drawn(base.drawn)
        .margin_bp(600.0)
        .closing_date(CLOSE)
        .revolving_end(datetime.date(2026, 1, 15))
        .maturity(datetime.date(2030, 1, 15))
        .frequency("3M")
        .payment_calendar_id("nyse")
        .term_out(24)
        .draw_schedule([{"date": "2025-01-15", "amount": {"amount": "5000000", "currency": "USD"}}])
        .readvance_to_borrowing_base(False)
        .discount_curve_id("USD-OIS")
        .build()
    )
    assert facility.draw_schedule[0]["amount"] == {"amount": "5000000", "currency": "USD"}
    assert facility.readvance_to_borrowing_base is False
    assert facility.fees is None
    again = AssetBackedFacility.from_json(facility.to_json())
    assert again.to_dict() == facility.to_dict()
    projection = facility.project(_market(), CLOSE)
    assert len(projection.draws) == 1
    assert projection.draws[0][1].amount == 5_000_000.0
    frame = projection.to_dataframe()
    assert "draw" in frame.columns
    assert projection.diagnostics.early_amortization_date is None
    assert [amount.amount for _, _, amount in projection.diagnostics.tranche_draws] == [5_000_000.0]
