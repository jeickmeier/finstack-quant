"""Native QuantLib FX touch option golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import SCHEMA, VALUATION_DATE, metadata, ql_date, tolerance
from .fx_exotics import _fx_market, _garman_kohlhagen_process

CAPTURED_ON = "2026-11-23"
SPOT = 1.10
DOMESTIC_RATE = 0.04
FOREIGN_RATE = 0.025
VOLATILITY = 0.10
PAYOUT = 100_000.0
EXPIRY = "2026-10-30"
# Both engines evaluate the same Rubinstein-Reiner closed form; the bound is the
# crate's adjudicated normal-CDF agreement with QuantLib (5e-9 relative) on the payout.
TOLERANCE = PAYOUT * 5e-9
REASON = (
    "QuantLib and Finstack evaluate the same Rubinstein-Reiner closed form; the bound is the "
    "5e-9 relative normal-CDF agreement between the two libraries applied to the payout."
)


def _touch_value(*, barrier: float, up: bool, no_touch: bool, at_expiry: bool) -> float:
    ql.Settings.instance().evaluationDate = ql_date(VALUATION_DATE)
    process = _garman_kohlhagen_process(SPOT, DOMESTIC_RATE, FOREIGN_RATE, VOLATILITY)
    option = ql.VanillaOption(
        ql.CashOrNothingPayoff(ql.Option.Call if up else ql.Option.Put, barrier, PAYOUT),
        ql.AmericanExercise(ql_date(VALUATION_DATE), ql_date(EXPIRY), at_expiry),
    )
    engine = ql.AnalyticDigitalAmericanKOEngine if no_touch else ql.AnalyticDigitalAmericanEngine
    option.setPricingEngine(engine(process))
    return option.NPV()


def _build_touch_fixture(
    *, product: str, name: str, description: str, barrier: float, up: bool, no_touch: bool, at_expiry: bool
) -> dict[str, Any]:
    engine = "AnalyticDigitalAmericanKOEngine" if no_touch else "AnalyticDigitalAmericanEngine"
    meta = metadata(
        name=name,
        domain="fx.fx_touch_option",
        description=description,
        product=product,
        source_detail=(
            f"QuantLib {ql.__version__} {engine} on a Garman-Kohlhagen process: spot {SPOT}, flat continuous "
            f"Act/365F domestic {DOMESTIC_RATE} and foreign {FOREIGN_RATE}, constant volatility {VOLATILITY}, "
            f"continuously monitored barrier {barrier}, cash payout {PAYOUT:.0f} USD."
        ),
    )
    return {
        "schema": SCHEMA,
        "metadata": {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON},
        "kind": "pricing",
        "model": "black76",
        "market": _fx_market(
            spot=SPOT,
            domestic_rate=DOMESTIC_RATE,
            foreign_rate=FOREIGN_RATE,
            vol_surface_id="EURUSD-VOL-QL",
            volatility=VOLATILITY,
            strikes=[0.9, 1.1, 1.3],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "fx_touch_option",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "base_currency": "EUR",
                    "quote_currency": "USD",
                    "barrier": barrier,
                    "barrier_direction": "up" if up else "down",
                    "touch_type": "no_touch" if no_touch else "one_touch",
                    "payout_timing": "at_expiry" if at_expiry else "at_hit",
                    "payout_amount": {"amount": f"{PAYOUT:.0f}", "currency": "USD"},
                    "monitoring_start_date": VALUATION_DATE,
                    "expiry": EXPIRY,
                    "day_count": "act_365f",
                    "domestic_discount_curve_id": "USD-OIS",
                    "foreign_discount_curve_id": "EUR-OIS",
                    "vol_surface_id": "EURUSD-VOL-QL",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": _touch_value(barrier=barrier, up=up, no_touch=no_touch, at_expiry=at_expiry)},
        "tolerances": {"npv": tolerance(TOLERANCE, REASON)},
    }


def build_fx_one_touch_at_expiry() -> dict[str, Any]:
    """Build a six-month EUR/USD down one-touch paid at expiry."""
    return _build_touch_fixture(
        product="fx_one_touch_at_expiry",
        name="eurusd_down_one_touch_at_expiry_6m_quantlib",
        description="QuantLib native six-month EUR/USD down one-touch paid at expiry.",
        barrier=1.04,
        up=False,
        no_touch=False,
        at_expiry=True,
    )


def build_fx_one_touch_at_hit() -> dict[str, Any]:
    """Build a six-month EUR/USD up one-touch paid at hit."""
    return _build_touch_fixture(
        product="fx_one_touch_at_hit",
        name="eurusd_up_one_touch_at_hit_6m_quantlib",
        description="QuantLib native six-month EUR/USD up one-touch paid at hit.",
        barrier=1.16,
        up=True,
        no_touch=False,
        at_expiry=False,
    )


def build_fx_no_touch() -> dict[str, Any]:
    """Build a six-month EUR/USD up no-touch."""
    return _build_touch_fixture(
        product="fx_no_touch",
        name="eurusd_up_no_touch_6m_quantlib",
        description="QuantLib native six-month EUR/USD up no-touch paid at expiry.",
        barrier=1.16,
        up=True,
        no_touch=True,
        at_expiry=True,
    )
