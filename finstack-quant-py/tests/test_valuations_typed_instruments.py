"""Typed Bond / TermLoan instrument classes and their pricing-union paths."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate
from finstack_quant.valuations.instruments import (
    Bond,
    TermLoan,
    instrument_cashflows_json,
    price_instrument,
    price_instrument_with_metrics,
)


def _market_json() -> str:
    return json.dumps({
        "version": 2,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "Act360",
                "knot_points": [[0.0, 1.0], [5.0, 0.90], [10.0, 0.80]],
                "interp_style": "monotone_convex",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            }
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
    })


def _fixed_bond() -> Bond:
    return Bond.fixed(
        "BOND-1",
        Money(1_000_000.0, Currency("USD")),
        Rate(0.05),
        datetime.date(2024, 1, 1),
        datetime.date(2034, 1, 1),
        "USD-OIS",
    )


def _without_timestamp(result_json: str) -> dict[str, object]:
    """Parse a ValuationResult and drop the wall-clock stamp.

    ``meta.timestamp`` records when the pricing call ran, so two otherwise
    identical calls differ there by construction.
    """
    parsed = json.loads(result_json)
    parsed["meta"].pop("timestamp", None)
    return parsed


class TestBondTyped:
    def test_fixed_constructor_and_id(self) -> None:
        bond = _fixed_bond()
        assert bond.id == "BOND-1"
        assert "BOND-1" in repr(bond)

    def test_to_json_is_tagged(self) -> None:
        payload = json.loads(_fixed_bond().to_json())
        assert payload["type"] == "bond"
        assert payload["spec"]["id"] == "BOND-1"

    def test_from_json_round_trip_preserves_fields(self) -> None:
        original = _fixed_bond().to_json()
        round_tripped = Bond.from_json(original).to_json()
        assert json.loads(round_tripped) == json.loads(original)

    def test_floating_constructor(self) -> None:
        frn = Bond.floating(
            "FRN-1",
            Money(1_000_000.0, Currency("USD")),
            "USD-SOFR-3M",
            Bps(200),
            datetime.date(2024, 1, 1),
            datetime.date(2030, 1, 1),
            Tenor.quarterly(),
            DayCount.ACT_360,
            "USD-OIS",
        )
        payload = json.loads(frn.to_json())
        assert payload["type"] == "bond"
        assert frn.id == "FRN-1"

    def test_invalid_json_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid instrument JSON"):
            Bond.from_json("{not valid json")

    def test_wrong_instrument_type_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match='expected instrument type "bond"'):
            Bond.from_json(TermLoan.example().to_json())

    def test_invalid_dates_raise_value_error(self) -> None:
        with pytest.raises(ValueError, match="start must be before end"):
            Bond.fixed(
                "BOND-BAD",
                Money(1_000_000.0, Currency("USD")),
                Rate(0.05),
                datetime.date(2034, 1, 1),
                datetime.date(2024, 1, 1),
                "USD-OIS",
            )

    def test_price_instrument_typed_equals_json(self) -> None:
        bond = _fixed_bond()
        market = _market_json()
        typed = price_instrument(bond, market, "2024-06-30")
        via_json = price_instrument(bond.to_json(), market, "2024-06-30")
        assert _without_timestamp(typed) == _without_timestamp(via_json)

    def test_price_instrument_with_metrics_accepts_typed(self) -> None:
        bond = _fixed_bond()
        market = _market_json()
        typed = price_instrument_with_metrics(bond, market, "2024-06-30", "discounting", ["ytm", "dv01"])
        via_json = price_instrument_with_metrics(bond.to_json(), market, "2024-06-30", "discounting", ["ytm", "dv01"])
        assert _without_timestamp(typed) == _without_timestamp(via_json)

    def test_instrument_cashflows_json_accepts_typed(self) -> None:
        bond = _fixed_bond()
        market = _market_json()
        typed = instrument_cashflows_json(bond, market, "2024-06-30", "discounting")
        via_json = instrument_cashflows_json(bond.to_json(), market, "2024-06-30", "discounting")
        assert json.loads(typed) == json.loads(via_json)


class TestTermLoanTyped:
    def test_example_and_id(self) -> None:
        loan = TermLoan.example()
        assert loan.id == "TERM-LOAN-USD-5Y"
        assert "TERM-LOAN-USD-5Y" in repr(loan)

    def test_to_json_is_tagged(self) -> None:
        payload = json.loads(TermLoan.example().to_json())
        assert payload["type"] == "term_loan"
        assert payload["spec"]["id"] == "TERM-LOAN-USD-5Y"

    def test_from_json_round_trip_preserves_fields(self) -> None:
        original = TermLoan.example().to_json()
        round_tripped = TermLoan.from_json(original).to_json()
        assert json.loads(round_tripped) == json.loads(original)

    def test_invalid_json_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid instrument JSON"):
            TermLoan.from_json("[1, 2")

    def test_wrong_instrument_type_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match='expected instrument type "term_loan"'):
            TermLoan.from_json(_fixed_bond().to_json())

    def test_price_instrument_typed_equals_json(self) -> None:
        loan = TermLoan.example()
        market = _market_json()
        typed = price_instrument(loan, market, "2024-06-30")
        via_json = price_instrument(loan.to_json(), market, "2024-06-30")
        assert _without_timestamp(typed) == _without_timestamp(via_json)


class TestUnionExtraction:
    def test_non_string_non_instrument_raises_type_error(self) -> None:
        with pytest.raises(TypeError, match="Bond / TermLoan"):
            price_instrument(12345, _market_json(), "2024-06-30")
