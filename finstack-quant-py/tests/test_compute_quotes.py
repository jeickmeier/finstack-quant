"""The bond quote set against its metrics, through the Python surface."""

from __future__ import annotations

import datetime
import json
import math

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations.instruments import Bond, BondQuoteSet, compute_quotes, price_instrument

AS_OF = datetime.date(2025, 1, 15)
MARKET = (
    MarketContext()
    .insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.04))
    .insert(DiscountCurve.flat("USD-TREASURY", AS_OF, 0.04))
)
QUOTE_METRICS = ("ytm", "ytw", "z_spread", "oas", "asw_par", "asw_market", "i_spread")


def _quoted_json(bond: Bond, clean_price_pct: float) -> str:
    spec = json.loads(bond.to_json())
    spec["instrument"]["spec"].setdefault("instrument_pricing_overrides", {}).setdefault("market_quotes", {})[
        "quoted_clean_price_pct"
    ] = clean_price_pct
    return json.dumps(spec)


def _callable() -> Bond:
    spec = json.loads(Bond.example_callable().to_json())
    spec["instrument"]["spec"]["instrument_pricing_overrides"] = {
        "model_config": {"hw1f_mean_reversion": 0.03, "hw1f_sigma": 0.01}
    }
    return Bond.from_json(json.dumps(spec))


@pytest.mark.parametrize(("bond", "clean"), [(Bond.example(), 98.5), (_callable(), 108.0)])
def test_each_field_is_the_priced_metric(bond: Bond, clean: float) -> None:
    quotes = compute_quotes(bond, MARKET, AS_OF, {"clean_price_pct": clean})
    assert quotes.clean_price_pct == clean
    result = price_instrument(_quoted_json(bond, clean), MARKET, AS_OF, metrics=list(QUOTE_METRICS))
    for name in QUOTE_METRICS:
        assert getattr(quotes, name) == result.get_metric(name), name


def test_a_quote_as_text_equals_the_quote_as_a_dict() -> None:
    bond = Bond.example()
    assert (
        compute_quotes(bond, MARKET, AS_OF, '{"ytm": 0.05}').to_dict()
        == compute_quotes(bond, MARKET, AS_OF, {"ytm": 0.05}).to_dict()
    )


def test_a_price_returns_from_its_yield() -> None:
    bond = Bond.example()
    quotes = compute_quotes(bond, MARKET, AS_OF, {"clean_price_pct": 98.5})
    back = compute_quotes(bond, MARKET, AS_OF, {"ytm": quotes.ytm})
    # YTM_TOLERANCE is 1e-12 on the yield axis: about duration times that, in percent.
    assert back.clean_price_pct == pytest.approx(98.5, abs=1e-9)


def test_what_does_not_apply_is_none_and_round_trips() -> None:
    quotes = compute_quotes(Bond.example(), MARKET, AS_OF, {"clean_price_pct": 98.5})
    assert quotes.discount_margin is None
    assert json.loads(quotes.to_json())["discount_margin"] is None
    assert BondQuoteSet.from_json(quotes.to_json()).to_dict() == quotes.to_dict()
    frame = quotes.to_dataframe()
    assert len(frame) == 1
    assert frame["ytm"].iloc[0] == quotes.ytm
    assert math.isnan(frame["discount_margin"].astype(float).iloc[0])


@pytest.mark.parametrize(
    ("bond", "quote", "message"),
    [
        (Bond.example(), {"discount_margin": 0.01}, "requires a floating-rate bond"),
        (_callable(), {"z_spread": 0.01}, "requires an explicit quoted clean price"),
        (Bond.example(), {"yield": 0.05}, "unknown variant"),
        (Bond.example(), {"ytm": 0.05, "ytw": 0.05}, "invalid BondQuoteInput"),
        (Bond.example(), {"ytm": math.nan}, "invalid BondQuoteInput"),
        (Bond.example(), 5, "invalid BondQuoteInput"),
    ],
)
def test_a_quote_the_bond_does_not_have_or_a_malformed_one_is_a_value_error(
    bond: Bond, quote: object, message: str
) -> None:
    with pytest.raises(ValueError, match=message):
        compute_quotes(bond, MARKET, AS_OF, quote)  # type: ignore[arg-type]


def test_a_missing_curve_is_a_key_error() -> None:
    with pytest.raises(KeyError):
        compute_quotes(Bond.example(), MarketContext(), AS_OF, {"ytm": 0.05})


def test_a_failed_metric_fails_the_whole_set() -> None:
    bond = Bond.example_callable()
    with pytest.raises(ValueError, match="bond quote metric 'oas'"):
        compute_quotes(bond, MARKET, AS_OF, {"clean_price_pct": 100.0})
