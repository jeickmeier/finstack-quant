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
from tests.golden.pricing_validation import requested_metrics, validated_instrument_json
from tests.golden.schema import GoldenFixture


def _resolve_market(market: dict) -> MarketContext:
    """Return a MarketContext from a `snapshot` or `envelope` market block.

    A `snapshot` carries materialized MarketContext JSON; an `envelope`
    carries a CalibrationEnvelope routed through the calibration engine. On
    envelope failures the wrapped exception preserves the structured
    ``CalibrationEnvelopeError`` payload (``kind``, ``step_id``, ``details``).
    """
    kind = market.get("kind")
    if kind == "snapshot":
        return MarketContext.from_json(json.dumps(market["data"]))
    if kind == "envelope":
        envelope = market["envelope"]
        plan_id = envelope.get("plan", {}).get("id", "?")
        try:
            result = calibrate(json.dumps(envelope))
        except CalibrationEnvelopeError as exc:
            raise CalibrationEnvelopeError(
                f"calibrate market envelope for plan '{plan_id}' failed ({exc.kind}, step={exc.step_id}): {exc}"
            ) from exc
        return result.market
    msg = f"pricing fixture market.kind must be 'snapshot' or 'envelope', got {kind!r}"
    raise ValueError(msg)


def run_pricing_fixture(fixture: GoldenFixture) -> dict[str, float]:
    """Run one common pricing fixture through the Python bindings."""
    body = fixture.body
    market = _resolve_market(body["market"])
    instrument_json = validated_instrument_json(body["instrument"])
    result_json = price_instrument_with_metrics(
        instrument_json,
        market,
        fixture.metadata.valuation_date,
        model=body["model"],
        metrics=requested_metrics(fixture.expected),
    )
    result = ValuationResult.from_json(result_json)

    actuals: dict[str, float] = {}
    for metric in fixture.expected:
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
