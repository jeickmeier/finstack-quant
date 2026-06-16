"""Tests for Phase 5 envelope TypedDicts.

Verifies that envelopes constructed via the typed-dict definitions in
``finstack_quant.valuations.envelope`` round-trip through ``calibrate``,
``dry_run``, and ``validate_calibration_json`` unchanged.
"""

from __future__ import annotations

import json

from finstack_quant.valuations import (
    CalibrationEnvelope,
    CalibrationPlan,
    DiscountStep,
    ForwardStep,
    HazardStep,
    MarketQuote,
    RateDeposit,
    RateSwap,
    VolSurfaceStep,
    calibrate,
    dependency_graph_json,
    dry_run,
    validate_calibration_json,
)
from finstack_quant.valuations.envelope import MarketDatum


def _market_datum(quote: MarketQuote) -> MarketDatum:
    payload = dict(quote)
    payload.pop("class", None)
    return {"kind": "rate_quote", **payload}


def _usd_deposit_quote(quote_id: str, count: int, rate: float) -> RateDeposit:
    return {
        "class": "rates",
        "type": "deposit",
        "id": quote_id,
        "index": "USD-SOFR-OIS",
        "pillar": {"tenor": {"count": count, "unit": "months"}},
        "rate": rate,
    }


def _usd_swap_quote(quote_id: str, years: int, rate: float) -> RateSwap:
    return {
        "class": "rates",
        "type": "swap",
        "id": quote_id,
        "index": "USD-SOFR-OIS",
        "pillar": {"tenor": {"count": years, "unit": "years"}},
        "rate": rate,
    }


def _discount_step() -> DiscountStep:
    return {
        "id": "USD-OIS",
        "quote_set": "usd_quotes",
        "kind": "discount",
        "curve_id": "USD-OIS",
        "currency": "USD",
        "base_date": "2026-05-08",
    }


def _typed_envelope() -> CalibrationEnvelope:
    quotes: list[MarketQuote] = [
        _usd_deposit_quote("USD-SOFR-DEP-1M", 1, 0.0525),
        _usd_deposit_quote("USD-SOFR-DEP-3M", 3, 0.052),
        _usd_swap_quote("USD-OIS-SWAP-1Y", 1, 0.051),
        _usd_swap_quote("USD-OIS-SWAP-2Y", 2, 0.049),
        _usd_swap_quote("USD-OIS-SWAP-5Y", 5, 0.045),
    ]
    plan: CalibrationPlan = {
        "id": "usd_curves",
        "quote_sets": {"usd_quotes": [quote["id"] for quote in quotes]},
        "steps": [_discount_step()],
    }
    return {
        "schema": "finstack_quant.calibration",
        "plan": plan,
        "market_data": [_market_datum(quote) for quote in quotes],
    }


def test_typed_envelope_calibrates_successfully() -> None:
    envelope = _typed_envelope()
    result = calibrate(json.dumps(envelope))
    assert result.success
    assert "USD-OIS" in result.step_ids


def test_typed_envelope_passes_validate_calibration_json() -> None:
    envelope = _typed_envelope()
    canonical = validate_calibration_json(json.dumps(envelope))
    parsed = json.loads(canonical)
    assert parsed["plan"]["id"] == "usd_curves"
    assert parsed["plan"]["steps"][0]["kind"] == "discount"


def test_typed_envelope_passes_dry_run_with_no_errors() -> None:
    envelope = _typed_envelope()
    report = json.loads(dry_run(json.dumps(envelope)))
    assert report["errors"] == []
    nodes = report["dependency_graph"]["nodes"]
    assert len(nodes) == 1
    assert nodes[0]["kind"] == "discount"
    assert nodes[0]["writes"] == ["USD-OIS"]


def test_typed_envelope_dependency_graph_matches_steps() -> None:
    envelope = _typed_envelope()
    graph = json.loads(dependency_graph_json(json.dumps(envelope)))
    assert graph["initial_ids"] == []
    assert [n["step_id"] for n in graph["nodes"]] == ["USD-OIS"]


def test_forward_step_typeddict_round_trips() -> None:
    """Forward step needs the discount curve produced by an earlier step."""
    forward: ForwardStep = {
        "id": "USD-SOFR-3M",
        "quote_set": "sofr_3m_quotes",
        "kind": "forward",
        "curve_id": "USD-SOFR-3M",
        "currency": "USD",
        "base_date": "2026-05-08",
        "tenor_years": 0.25,
        "discount_curve_id": "USD-OIS",
    }
    discount = _discount_step()
    envelope: CalibrationEnvelope = {
        "schema": "finstack_quant.calibration",
        "plan": {
            "id": "usd_with_forward",
            "quote_sets": {
                "usd_quotes": ["USD-SOFR-DEP-1M"],
                "sofr_3m_quotes": [],
            },
            "steps": [discount, forward],
        },
        "market_data": [_market_datum(_usd_deposit_quote("USD-SOFR-DEP-1M", 1, 0.0525))],
    }
    canonical = json.loads(validate_calibration_json(json.dumps(envelope)))
    assert [s["kind"] for s in canonical["plan"]["steps"]] == ["discount", "forward"]


def test_hazard_step_typeddict_round_trips() -> None:
    hazard: HazardStep = {
        "id": "ISSUER-A-CDS",
        "quote_set": "cds_quotes",
        "kind": "hazard",
        "curve_id": "ISSUER-A-CDS",
        "entity": "Issuer-A",
        "seniority": "senior",
        "currency": "USD",
        "base_date": "2026-05-08",
        "discount_curve_id": "USD-OIS",
    }
    envelope: CalibrationEnvelope = {
        "schema": "finstack_quant.calibration",
        "plan": {
            "id": "single_name_hazard",
            "quote_sets": {
                "usd_quotes": ["USD-SOFR-DEP-1M"],
                "cds_quotes": [],
            },
            "steps": [_discount_step(), hazard],
        },
        "market_data": [_market_datum(_usd_deposit_quote("USD-SOFR-DEP-1M", 1, 0.0525))],
    }
    canonical = json.loads(validate_calibration_json(json.dumps(envelope)))
    assert canonical["plan"]["steps"][1]["kind"] == "hazard"


def test_vol_surface_step_typeddict_round_trips() -> None:
    vol: VolSurfaceStep = {
        "id": "SPX-VOL-STEP",
        "quote_set": "spx_vol_quotes",
        "kind": "vol_surface",
        "surface_id": "SPX-VOL",
        "base_date": "2026-05-08",
        "underlying_ticker": "SPX",
        "model": "SABR",
    }
    envelope: CalibrationEnvelope = {
        "schema": "finstack_quant.calibration",
        "plan": {
            "id": "spx_vol_surface",
            "quote_sets": {"spx_vol_quotes": []},
            "steps": [vol],
        },
    }
    canonical = json.loads(validate_calibration_json(json.dumps(envelope)))
    assert canonical["plan"]["steps"][0]["kind"] == "vol_surface"


def test_typeddict_envelope_uses_dollar_schema_when_provided() -> None:
    """The optional ``$schema`` key for editor JSON Schema discovery."""
    envelope: CalibrationEnvelope = {
        "$schema": "../../schemas/calibration/2/calibration.schema.json",
        "schema": "finstack_quant.calibration",
        "plan": _typed_envelope()["plan"],
    }
    canonical = json.loads(validate_calibration_json(json.dumps(envelope)))
    assert canonical["$schema"].endswith("calibration.schema.json")
