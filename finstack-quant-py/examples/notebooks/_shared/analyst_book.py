"""Fresh, cumulative teaching books using canonical public instrument payloads.

Amounts are currency units, rates are decimals, and quoted spreads are basis
points. ``USD-CASH`` and ``EUR-CASH`` are dated cash deposits, not perpetual
bank balances. Every factory returns independently owned mutable objects; a
later stage never mutates an earlier snapshot. Pricing remains in the public
``finstack_quant`` APIs, visible in the lessons.

>>> spec = book_spec("foundations")
>>> len(spec["positions"])
4
>>> "BORROWER-TL" in instruments("base")
False
"""

from __future__ import annotations

from datetime import date, timedelta
import json
from pathlib import Path
from typing import Any

from finstack_quant.calibration import calibrate
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    FxDeltaVolSurface,
    FxMatrix,
    MarketContext,
    ScalarTimeSeries,
    VolSurface,
)
from finstack_quant.portfolio import Portfolio

from .instrument_fixtures import cds, fixed_bond, instrument_envelope, irs, term_loan
from .market import usd_sofr_curve, usd_sofr_fixings

AS_OF = date(2025, 1, 15)
STAGES = ("foundations", "rates", "options", "base", "common")
RISK_METRICS = (
    "dv01",
    "bucketed_dv01",
    "cs01",
    "bucketed_cs01",
    "delta",
    "gamma",
    "vega",
    "rho",
    "theta",
    "pv01",
)
_CALIBRATION_EXAMPLE = (
    Path(__file__).resolve().parents[4]
    / "finstack-quant/calibration/examples/market_bootstrap/03_single_name_hazard.json"
)


def _stage_index(stage: str) -> int:
    if stage not in STAGES:
        raise ValueError(f"stage must be one of {STAGES}; got {stage!r}")
    return STAGES.index(stage)


def instruments(stage: str = "base") -> dict[str, dict[str, Any]]:
    """Return fresh canonical instrument envelopes through a curriculum stage.

    Args:
        stage: One of ``STAGES``. The loan first appears in ``common`` (4.3).

    Returns:
        Instrument ID to independently owned envelope; every position is one
        unit of its full contractual notional.

    Raises:
        ValueError: If ``stage`` is unknown.

    >>> list(instruments("rates"))[-1]
    'USD-PAYER-IRS'
    """
    index = _stage_index(stage)
    raw: dict[str, dict[str, Any]] = {}
    for iid, currency, amount, coupon in (
        ("USD-CORP", "USD", "2000000", "0.055"),
        ("EUR-GOVT", "EUR", "1500000", "0.0325"),
    ):
        _, bond = fixed_bond(0)
        bond["spec"].update({
            "id": iid,
            "notional": {"amount": amount, "currency": currency},
            "discount_curve_id": f"{currency}-OIS",
            "maturity": "2030-01-15",
        })
        bond["spec"]["cashflow_spec"]["fixed"]["rate"] = coupon
        bond["spec"]["attributes"] = {
            "tags": ["fixed-income"],
            "meta": {"sector": "corporate" if currency == "USD" else "government"},
        }
        raw[iid] = bond
    for currency, amount, rate in (("USD", "500000", "0.045"), ("EUR", "250000", "0.03")):
        iid = f"{currency}-CASH"
        raw[iid] = {
            "type": "deposit",
            "spec": {
                "id": iid,
                "notional": {"amount": amount, "currency": currency},
                "start_date": "2025-01-14",
                "maturity": "2025-07-15",
                "day_count": "act_360",
                "quote_rate": rate,
                "discount_curve_id": f"{currency}-OIS",
                "attributes": {"tags": ["cash-deposit"], "meta": {"sector": "cash"}},
            },
        }
    if index >= 1:
        _, swap = irs(0)
        swap["spec"].update({"id": "USD-PAYER-IRS", "notional": {"amount": "2000000", "currency": "USD"}})
        swap["spec"]["fixed"]["rate"] = "0.045"
        raw["USD-PAYER-IRS"] = swap
    if index >= 2:
        raw["SPX-CALL"] = {
            "type": "equity_option",
            "spec": {
                "id": "SPX-CALL",
                "underlying_ticker": "SPX",
                "strike": 5200.0,
                "option_type": "call",
                "exercise_style": "european",
                "expiry": "2025-12-19",
                "notional": {"amount": "100", "currency": "USD"},
                "day_count": "act_365f",
                "settlement": "cash",
                "discount_curve_id": "USD-OIS",
                "spot_id": "SPX-SPOT",
                "vol_surface_id": "SPX-VOL",
                "div_yield_id": "SPX-DIV",
                "attributes": {"tags": ["equity-option"], "meta": {"sector": "equity"}},
            },
        }
    if index >= 3:
        _, credit = cds(0)
        credit["spec"].update({"id": "ACME-CDS", "notional": {"amount": "1000000", "currency": "USD"}})
        credit["spec"]["premium"]["end"] = "2030-03-20"
        credit["spec"]["premium"]["spread_bp"] = "100"
        credit["spec"]["protection"]["credit_curve_id"] = "ACME-HZD"
        credit["spec"]["attributes"] = {"tags": ["credit-hedge"], "meta": {"sector": "corporate"}}
        raw["ACME-CDS"] = credit
    if index >= 4:
        _, loan = term_loan(0)
        loan["spec"].update({
            "id": "BORROWER-TL",
            "notional_limit": {"amount": "1000000", "currency": "USD"},
            "issue_date": "2025-01-15",
            "maturity": "2030-01-15",
            "attributes": {"tags": ["leveraged-loan"], "meta": {"sector": "corporate", "borrower": "ACME"}},
        })
        raw["BORROWER-TL"] = loan
    return {iid: instrument_envelope(instrument) for iid, instrument in raw.items()}


def book_spec(stage: str = "base") -> dict[str, Any]:
    """Return a fresh USD-reporting ``PortfolioSpec`` for ``stage``.

    Args:
        stage: Curriculum stage from ``STAGES``.

    Returns:
        Native portfolio JSON payload with one entity and unit quantities;
        contract notional, not quantity, sets exposure.

    Raises:
        ValueError: If the stage is unknown.

    >>> book_spec("common")["positions"][-1]["instrument_id"]
    'BORROWER-TL'
    """
    return {
        "id": f"ANALYST-{stage.upper()}",
        "as_of": AS_OF.isoformat(),
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": iid,
                "entity_id": "FUND",
                "instrument_id": iid,
                "instrument_spec": envelope["instrument"],
                "quantity": 1.0,
                "unit": "units",
            }
            for iid, envelope in instruments(stage).items()
        ],
    }


def build_book(stage: str = "base") -> Portfolio:
    """Build the native portfolio for a curriculum stage.

    Args:
        stage: A value in ``STAGES``.

    Returns:
        Independently owned native ``Portfolio`` at ``AS_OF``.

    Raises:
        ValueError: If the stage or a canonical instrument definition is invalid.

    >>> len(build_book("foundations"))
    4
    """
    return Portfolio.from_spec(json.dumps(book_spec(stage)))


def calibration_envelope(stage: str = "base", as_of: date = AS_OF) -> dict[str, Any]:
    """Return the quote-backed USD calibration input used by a market stage.

    Args:
        stage: Curriculum stage; ``base`` and ``common`` add ACME CDS quotes.
        as_of: Base date for every calibrated curve. Quote tenors stay fixed.

    Returns:
        Fresh canonical calibration envelope. It reuses the repository's
        deposit/IRS/CDS example and retains the full hazard replay recipe.

    Raises:
        ValueError: If the stage is unknown.
        OSError: If the repository example cannot be read.

    >>> calibration_envelope("base")["plan"]["steps"][-1]["curve_id"]
    'ACME-HZD'
    """
    index = _stage_index(stage)
    envelope = json.loads(_CALIBRATION_EXAMPLE.read_text())
    envelope.pop("$schema", None)
    envelope["plan"]["id"] = "analyst-market"
    envelope["plan"]["description"] = "Deterministic teaching quotes; not observed market data."
    for step in envelope["plan"]["steps"]:
        step["base_date"] = as_of.isoformat()
        if step["kind"] == "hazard":
            step.update({"id": "ACME-HZD", "curve_id": "ACME-HZD", "entity": "ACME"})
    for quote in envelope["market_data"]:
        if quote["kind"] == "cds_quote":
            quote["entity"] = "ACME"
    if index < 3:
        envelope["plan"]["steps"] = envelope["plan"]["steps"][:1]
        envelope["plan"]["quote_sets"].pop("issuer_a_cds_quotes")
        envelope["market_data"] = [quote for quote in envelope["market_data"] if quote["kind"] != "cds_quote"]
    return envelope


def single_name_calibration_envelope(as_of: date = AS_OF) -> dict[str, Any]:
    """Return the quote-backed single-name hazard inputs introduced in 1.5.

    Args:
        as_of: Base date for the discount and hazard calibration curves.

    Returns:
        Fresh OIS and ACME CDS quote inputs with the lossless hazard replay
        recipe. This market-data fixture does not add a CDS holding to a book.

    Raises:
        OSError: If the repository calibration example cannot be read.

    >>> single_name_calibration_envelope()["plan"]["steps"][-1]["curve_id"]
    'ACME-HZD'
    """
    return calibration_envelope("base", as_of)


def build_market(stage: str = "base", as_of: date = AS_OF) -> MarketContext:
    """Build a fresh market containing all inputs for ``stage``.

    Args:
        stage: Curriculum stage from ``STAGES``.
        as_of: Curve base date; changing it rolls constant tenor quotes.

    Returns:
        Market with discount curves, EUR/USD=1.08, and only the additional
        forward, option and calibrated hazard inputs required by the stage.

    Raises:
        ValueError: If the stage is unknown or market input validation fails.
        RuntimeError: If native calibration cannot solve the teaching quotes.
        OSError: If the canonical calibration example cannot be read.

    >>> build_market("foundations").get_discount("USD-OIS").id
    'USD-OIS'
    """
    index = _stage_index(stage)
    market = calibrate(json.dumps(calibration_envelope(stage, as_of))).market
    market.insert(
        DiscountCurve(
            "EUR-OIS",
            as_of,
            [
                (0.0, 1.0),
                (1.0, 0.97),
                (3.0, 0.91),
                (5.0, 0.85),
                (10.0, 0.72),
            ],
            day_count="act_365f",
        )
    )
    fx = FxMatrix()
    fx.set_quote(Currency("EUR"), Currency("USD"), 1.08)
    market.insert_fx(fx)
    # Overnight observations enter with the first compounding lesson, before
    # the later term-SOFR forward curve and payer swap are introduced.
    market.insert(ForwardCurve.flat("USD-SOFR", 1 / 360, as_of, 0.05))
    first_fixing = date(2024, 1, 1)
    market.insert_series(
        ScalarTimeSeries(
            "FIXING:USD-SOFR",
            [(first_fixing + timedelta(days=offset), 0.05) for offset in range((as_of - first_fixing).days + 1)],
        )
    )
    if index >= 1:
        market.insert(usd_sofr_curve(as_of))
        market.insert_series(usd_sofr_fixings(as_of))
        for surface_id in ("USD-CAP-VOL", "USD-SWPNVOL"):
            market.insert(VolSurface(surface_id, [0.25, 0.5, 1.0, 3.0, 5.0], [0.01, 0.045, 0.08], [0.25] * 15))
    if index >= 2:
        market.insert_price("SPX-SPOT", 5200.0, "USD")
        market.insert_price("SPX-DIV", 0.015)
        market.insert(
            FxDeltaVolSurface(
                "EURUSD-VOL", [0.5, 1.0, 2.0], [0.10, 0.11, 0.12], [0.01, 0.012, 0.012], [0.003, 0.004, 0.004]
            )
        )
        market.insert(
            VolSurface(
                "SPX-VOL",
                [0.25, 0.5, 1.0, 2.0],
                [4000.0, 5200.0, 6400.0],
                [
                    [0.25, 0.20, 0.19],
                    [0.25, 0.20, 0.19],
                    [0.25, 0.20, 0.19],
                    [0.25, 0.20, 0.19],
                ],
            )
        )
    return market


def expected_metrics(stage: str = "base") -> dict[str, tuple[str, ...]]:
    """Declare the requested metric families applicable to each fixture position.

    Args:
        stage: A value in ``STAGES``.

    Returns:
        Position IDs mapped to required scalar or qualified-series families.
        Inapplicable families are omitted, never represented as invented zeros.
        Straight-bond vega is applicable and correctly evaluates to zero.

    Raises:
        ValueError: If the stage is unknown.

    >>> "cs01" in expected_metrics("base")["USD-CASH"]
    False
    """
    families = {
        "bond": ("dv01", "bucketed_dv01", "cs01", "bucketed_cs01", "vega", "theta"),
        "deposit": ("dv01", "bucketed_dv01", "theta"),
        "interest_rate_swap": ("dv01", "bucketed_dv01", "pv01", "theta"),
        "equity_option": ("dv01", "bucketed_dv01", "delta", "gamma", "vega", "rho", "theta"),
        "credit_default_swap": ("dv01", "bucketed_dv01", "cs01", "bucketed_cs01", "theta"),
        "term_loan": ("dv01", "bucketed_dv01", "cs01", "bucketed_cs01", "theta"),
    }
    return {iid: families[envelope["instrument"]["type"]] for iid, envelope in instruments(stage).items()}
