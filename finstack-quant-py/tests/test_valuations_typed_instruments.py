"""Typed Bond / TermLoan instrument classes and their pricing-union paths."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import (
    Bond,
    TermLoan,
    instrument_cashflows_json,
    price_instrument,
)


def _market_json() -> str:
    return json.dumps({
        "schema_version": 1,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "act_360",
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
        "hierarchy": None,
    })


def _fixed_bond() -> Bond:
    return Bond.fixed(
        "BOND-1",
        Money(1_000_000.0, Currency("USD")),
        Rate(0.05),
        datetime.date(2024, 1, 1),
        datetime.date(2034, 1, 1),
        StubKind.SHORT_FRONT,
        "USD-OIS",
    )


def _without_timestamp(result: ValuationResult) -> dict[str, object]:
    """Serialize a ValuationResult and drop the wall-clock stamp.

    ``meta.timestamp`` records when the pricing call ran, so two otherwise
    identical calls differ there by construction.
    """
    parsed = json.loads(result.to_json())
    parsed["meta"].pop("timestamp", None)
    return parsed


def _approx_payload(value: object) -> object:
    """Mirror a decoded payload with every float wrapped in ``pytest.approx``.

    serde_json's default (non-``float_roundtrip``) float parser is not always
    bit-exact on reparse, so a parse -> reserialize cycle can shift a value by
    1 ULP. That is a pre-existing serde_json characteristic, not a round-trip
    fidelity bug.

    The tolerance is sized for exactly that: ``rel=1e-15`` is a few ULP of an
    f64 and ``abs=0`` keeps zeros exact. ``pytest.approx``'s default
    ``rel=1e-6`` would have let a PV of 1,000,000.0 reparse as 1,000,000.9,
    which is a data-loss bug rather than a rounding artefact.
    """
    if isinstance(value, dict):
        return {key: _approx_payload(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_approx_payload(item) for item in value]
    if isinstance(value, float):
        return pytest.approx(value, rel=1e-15, abs=0)
    return value


class TestBondTyped:
    def test_fixed_constructor_and_id(self) -> None:
        bond = _fixed_bond()
        assert bond.id == "BOND-1"
        assert "BOND-1" in repr(bond)

    def test_to_json_is_canonical_envelope(self) -> None:
        payload = json.loads(_fixed_bond().to_json())
        assert payload["schema"] == "finstack_quant.instrument/1"
        assert payload["instrument"]["type"] == "bond"
        assert payload["instrument"]["spec"]["id"] == "BOND-1"

    def test_fixed_constructor_preserves_explicit_stub(self) -> None:
        bond = Bond.fixed(
            "BOND-LONG-FRONT",
            Money(1_000_000.0, Currency("USD")),
            Rate(0.05),
            datetime.date(2024, 3, 1),
            datetime.date(2034, 1, 15),
            StubKind.LONG_FRONT,
            "USD-OIS",
        )
        spec = json.loads(bond.to_json())["instrument"]["spec"]
        assert spec["cashflow_spec"]["fixed"]["stub"] == "long_front"

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
        assert payload["instrument"]["type"] == "bond"
        assert frn.id == "FRN-1"

    def test_invalid_json_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid instrument envelope JSON"):
            Bond.from_json("{not valid json")

    def test_wrong_instrument_type_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match=r"expected instrument type `bond`, got `term_loan`"):
            Bond.from_json(TermLoan.example().to_json())

    def test_invalid_dates_raise_value_error(self) -> None:
        with pytest.raises(ValueError, match=r"issue date .* must be before maturity date"):
            Bond.fixed(
                "BOND-BAD",
                Money(1_000_000.0, Currency("USD")),
                Rate(0.05),
                datetime.date(2034, 1, 1),
                datetime.date(2024, 1, 1),
                StubKind.SHORT_FRONT,
                "USD-OIS",
            )

    def test_price_instrument_typed_equals_json(self) -> None:
        bond = _fixed_bond()
        market = _market_json()
        typed = price_instrument(bond, market, "2024-06-30")
        via_json = price_instrument(bond.to_json(), market, "2024-06-30")
        assert _without_timestamp(typed) == _without_timestamp(via_json)

    def test_price_instrument_accepts_typed(self) -> None:
        bond = _fixed_bond()
        market = _market_json()
        typed = price_instrument(bond, market, "2024-06-30", "discounting", ["ytm", "dv01"])
        via_json = price_instrument(bond.to_json(), market, "2024-06-30", "discounting", ["ytm", "dv01"])
        assert _without_timestamp(typed) == _without_timestamp(via_json)

    def test_price_instrument_returns_typed_result(self) -> None:
        """``price_instrument`` hands back a ``ValuationResult``, not JSON."""
        result = price_instrument(_fixed_bond(), _market_json(), "2024-06-30")

        assert isinstance(result, ValuationResult)
        assert not isinstance(result, str)
        assert result.instrument_id == "BOND-1"
        assert result.currency == "USD"

        # Nobody loses the JSON: `to_json` still emits the wire payload, and
        # `from_json` decodes it back to an equal one.
        wire = result.to_json()
        reparsed = json.loads(ValuationResult.from_json(wire).to_json())
        assert reparsed == _approx_payload(json.loads(wire))

    def test_price_instrument_returns_typed_result_with_metrics(self) -> None:
        """``price_instrument`` hands back a ``ValuationResult`` with requested metrics."""
        result = price_instrument(_fixed_bond(), _market_json(), "2024-06-30", "discounting", ["ytm", "dv01"])

        assert isinstance(result, ValuationResult)
        assert not isinstance(result, str)
        assert result.get_metric("ytm") is not None
        assert result.get_metric("dv01") is not None

        wire = result.to_json()
        reparsed = json.loads(ValuationResult.from_json(wire).to_json())
        assert reparsed == _approx_payload(json.loads(wire))

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

    def test_to_json_is_canonical_envelope(self) -> None:
        payload = json.loads(TermLoan.example().to_json())
        assert payload["schema"] == "finstack_quant.instrument/1"
        assert payload["instrument"]["type"] == "term_loan"
        assert payload["instrument"]["spec"]["id"] == "TERM-LOAN-USD-5Y"

    def test_from_json_round_trip_preserves_fields(self) -> None:
        original = TermLoan.example().to_json()
        round_tripped = TermLoan.from_json(original).to_json()
        assert json.loads(round_tripped) == json.loads(original)

    def test_invalid_json_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid instrument envelope JSON"):
            TermLoan.from_json("[1, 2")

    def test_wrong_instrument_type_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match=r"expected instrument type `term_loan`, got `bond`"):
            TermLoan.from_json(_fixed_bond().to_json())

    def test_price_instrument_typed_equals_json(self) -> None:
        loan = TermLoan.example()
        market = _market_json()
        typed = price_instrument(loan, market, "2024-06-30")
        via_json = price_instrument(loan.to_json(), market, "2024-06-30")
        assert _without_timestamp(typed) == _without_timestamp(via_json)


class TestUnionExtraction:
    def test_non_string_non_instrument_raises_type_error(self) -> None:
        with pytest.raises(TypeError, match="typed instrument instance"):
            price_instrument(12345, _market_json(), "2024-06-30")
