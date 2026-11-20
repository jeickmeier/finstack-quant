"""Shared pricing helpers for instrument-level golden fixtures."""

from __future__ import annotations

from datetime import date
import json

from finstack_quant.calibration import (
    CalibrationEnvelopeError,
    calibrate,
)
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import price_instrument
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


def _cdsw_as_of_display(fixture: GoldenFixture, result: ValuationResult) -> dict[str, float]:
    """Reconstruct the archived IBM screen display, not a dated settlement PV."""
    if fixture.metadata.source != "bloomberg-screen" or fixture.metadata.name != "cds_5y_par_spread":
        msg = "CDSW as-of display assertions require the archived IBM CDSW fixture"
        raise ValueError(msg)
    spec = fixture.body["instrument"]["instrument"]["spec"]
    leg = spec["premium_leg"]
    begin = date.fromisoformat(leg["start"])
    as_of = date.fromisoformat(fixture.metadata.valuation_date)
    # The first coupon (22 June) and 44-day inclusive accrual are shown on the
    # screen; this fixture is a live pay-protection ACT/360 CDS without upfront.
    if not (
        spec["side"] == "pay"
        and spec.get("upfront") is None
        and leg["day_count"] == "act_360"
        and begin < as_of < date(2026, 6, 22)
    ):
        msg = "CDSW as-of display requires the archived first coupon and no actual upfront"
        raise ValueError(msg)
    accrued = float(spec["notional"]["amount"]) * float(leg["coupon_bp"]) / 10_000 * ((as_of - begin).days + 1) / 360
    dirty = float(result.price)
    exposure = result.get_metric("default_exposure")
    if exposure is None:
        msg = "CDSW display requires the raw default exposure metric"
        raise ValueError(msg)
    return {
        "npv::cdsw_principal_as_of": dirty + accrued,
        "npv::cdsw_accrued_as_of": -accrued,
        "npv::cdsw_cash_amount_as_of": dirty,
        "default_exposure::cdsw_as_of": exposure - accrued,
    }


def run_pricing_fixture(fixture: GoldenFixture) -> dict[str, float]:
    """Run one common pricing fixture through the Python bindings."""
    body = fixture.body
    market = _resolve_market(body["market"])
    instrument_json = validated_instrument_json(body["instrument"])
    result = price_instrument(
        instrument_json,
        market,
        fixture.metadata.valuation_date,
        model=body["model"],
        metrics=requested_metrics(fixture.expected),
    )

    display = _cdsw_as_of_display(fixture, result) if any("::cdsw_" in key for key in fixture.expected) else {}
    actuals: dict[str, float] = {}
    for metric in fixture.expected:
        if metric in display:
            actuals[metric] = display[metric]
            continue
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
