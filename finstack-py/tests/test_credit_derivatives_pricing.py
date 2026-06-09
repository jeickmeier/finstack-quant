"""Behavioral smoke tests for the CDS-family instrument wrappers.

Regression for review blocker B5: ``CDSOption.price()`` used to hardcode the
decommissioned ``"black76"`` model key and raised on every call. The wrappers
now pass ``"default"``, which resolves to each instrument's registered model
(``BloombergCdso`` for CDSOption, ``HazardRate`` for the rest).
"""

from __future__ import annotations

from datetime import date
import json

from finstack.core.market_data import DiscountCurve, HazardCurve, MarketContext
from finstack.valuations.credit_derivatives import CDSOption, CreditDefaultSwap

AS_OF = date(2025, 1, 2)
AS_OF_STR = "2025-01-02"


def _market() -> MarketContext:
    ois = DiscountCurve(
        "USD-OIS",
        AS_OF,
        [(0.0, 1.0), (1.0, 0.97), (3.0, 0.90), (5.0, 0.82), (10.0, 0.65)],
        day_count="act_365f",
    )
    hazard = HazardCurve(
        "CORP-HAZARD",
        AS_OF,
        [(0.5, 0.018), (1.0, 0.020), (3.0, 0.024), (5.0, 0.028), (10.0, 0.032)],
        recovery_rate=0.40,
    )
    return MarketContext().insert(ois).insert(hazard)


def test_cds_option_example_prices() -> None:
    """B5 smoke test: CDSOption.example().price() must not raise."""
    payload = json.loads(CDSOption.example().to_json())
    node = payload.get("instrument", payload)
    spec = node.get("spec", node)
    # The example references a vol surface; supply the vol via the
    # instrument-level override so the market only needs curves.
    spec["pricing_overrides"]["implied_volatility"] = 0.30
    option = CDSOption.from_json(json.dumps(spec))

    result = option.price(_market(), AS_OF_STR)

    assert result.currency == "USD"
    assert result.price == result.price  # not NaN
    assert abs(result.price) < 10_000_000.0


def test_credit_default_swap_example_prices() -> None:
    """The CDS wrapper keeps pricing with its registered hazard-rate model."""
    cds = CreditDefaultSwap.example()

    result = cds.price(_market(), AS_OF_STR)

    assert result.currency == "USD"
    assert result.price == result.price  # not NaN
    assert abs(result.price) < 10_000_000.0
