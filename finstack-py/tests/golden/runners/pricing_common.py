"""Shared pricing helpers for instrument-level golden fixtures."""

from __future__ import annotations

import json

from finstack.core.market_data import MarketContext
from finstack.valuations.instruments import price_instrument_with_metrics

from finstack.valuations import (
    CalibrationEnvelopeError,
    ValuationResult,
    calibrate,
)
from tests.golden.pricing_validation import validated_instrument_json
from tests.golden.runners import validate_source_validation_fixture
from tests.golden.schema import GoldenFixture


def _resolve_market(inputs: dict) -> MarketContext:
    """Return a MarketContext from either the 'market' or 'market_envelope' key.

    Mutually exclusive: the fixture must provide exactly one. 'market' is the
    materialized MarketContext JSON (snapshot); 'market_envelope' is a
    CalibrationEnvelope routed through the calibration engine.

    On envelope-driven failures the wrapped exception preserves the
    structured ``CalibrationEnvelopeError`` payload (``kind``, ``step_id``,
    ``details``) for downstream debugging — the legacy ``except ValueError``
    pattern would have missed Phase 4's ``CalibrationEnvelopeError``
    (RuntimeError subclass) entirely.
    """
    has_market = "market" in inputs
    has_envelope = "market_envelope" in inputs
    if has_market and has_envelope:
        raise ValueError("pricing fixture supplied both 'market' and 'market_envelope'; specify exactly one")
    if has_market:
        return MarketContext.from_json(json.dumps(inputs["market"]))
    if has_envelope:
        envelope = inputs["market_envelope"]
        plan_id = envelope.get("plan", {}).get("id", "?")
        try:
            result = calibrate(json.dumps(envelope))
        except CalibrationEnvelopeError as exc:
            raise CalibrationEnvelopeError(
                f"calibrate market_envelope for plan '{plan_id}' failed ({exc.kind}, step={exc.step_id}): {exc}"
            ) from exc
        return result.market
    raise ValueError("pricing fixture must supply either 'market' or 'market_envelope'")


def run_pricing_fixture(fixture: GoldenFixture) -> dict[str, float]:
    """Run one common pricing fixture through the Python bindings."""
    validate_source_validation_fixture("pricing runner", fixture)

    inputs = fixture.inputs
    market = _resolve_market(inputs)
    instrument_json = validated_instrument_json(inputs["instrument_json"])
    result_json = price_instrument_with_metrics(
        instrument_json,
        market,
        inputs["valuation_date"],
        model=inputs["model"],
        metrics=list(inputs["metrics"]),
    )
    result = ValuationResult.from_json(result_json)

    actuals: dict[str, float] = {}
    for metric in fixture.expected_outputs:
        if metric == "npv":
            actuals[metric] = float(result.price)
            continue
        value = result.get_metric(metric)
        if value is None:
            raise ValueError(f"result missing metric {metric!r}")
        actuals[metric] = float(value)
    return actuals


def run(fixture: GoldenFixture) -> dict[str, float]:
    """Run a fixture that follows the shared pricing input contract."""
    return run_pricing_fixture(fixture)
