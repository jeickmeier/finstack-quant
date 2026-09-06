"""ValuationResult ergonomics, metric-key rendering, pricing error kinds and typed pricing options."""

from __future__ import annotations

import datetime
import json
import pickle

import pandas as pd
import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.valuations import ValuationResult, instrument_cashflows
from finstack_quant.valuations.instruments import (
    Bond,
    FixedLegSpec,
    FloatLegSpec,
    InterestRateSwap,
    MarketHistory,
    MetricPricingOverrides,
    price_instrument,
)

AS_OF = datetime.date(2024, 1, 15)


def _market() -> MarketContext:
    return (
        MarketContext()
        .insert(
            DiscountCurve(
                "USD-OIS",
                AS_OF,
                [(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)],
            )
        )
        .insert(
            ForwardCurve(
                "USD-SOFR-3M",
                0.25,
                AS_OF,
                [(0.0, 0.04), (5.0, 0.042), (10.0, 0.045)],
                day_count="act_360",
            )
        )
    )


def _bond() -> Bond:
    return Bond.fixed(
        "B1",
        Money(1_000_000.0, Currency("USD")),
        Rate(0.05),
        AS_OF,
        datetime.date(2029, 1, 15),
        StubKind.NONE,
        "USD-OIS",
    )


def _swap() -> InterestRateSwap:
    # Spot-starting 5y pay-fixed USD swap; the forward leg starts at as_of so
    # no historical fixing is required.
    start = datetime.date(2024, 1, 17)
    end = datetime.date(2029, 1, 17)
    fixed = FixedLegSpec(
        "USD-OIS",
        0.04,
        Tenor.semi_annual(),
        DayCount.THIRTY_360,
        start,
        end,
        compounding_simple=False,
    )
    floating = FloatLegSpec(
        "USD-OIS",
        "USD-SOFR-3M",
        0.0,
        Tenor.quarterly(),
        DayCount.ACT_360,
        start,
        end,
    )
    return (
        InterestRateSwap
        .builder()
        .id("SWP-5Y")
        .notional(Money(10_000_000.0, Currency("USD")))
        .side("pay")
        .fixed(fixed)
        .float(floating)
        .build()
    )


# Metric keys are literal (no Rust identifier escaping)


def test_metric_keys_have_no_rust_escaping() -> None:
    result = price_instrument(_swap(), _market(), AS_OF, metrics=["pv01", "bucketed_dv01"])
    keys = result.metric_keys()

    assert not any("_x2d" in key or "_x5f" in key for key in keys), keys
    assert "pv01::USD-OIS" in keys
    assert any(key.startswith("bucketed_dv01::USD-OIS::") and key.endswith("y") for key in keys), keys

    columns = list(result.to_dataframe().columns)
    assert not any("_x2d" in column or "_x5f" in column for column in columns), columns
    assert "pv01::USD-OIS" in columns

    payload = json.loads(result.to_json())
    assert "pv01::USD-OIS" in payload["measures"]
    assert not any("_x2d" in key for key in payload["measures"])

    round_trip = ValuationResult.from_json(result.to_json())
    assert round_trip.metric_keys() == keys
    assert round_trip == result

    assert result.get_metric("pv01::USD_x2dOIS") is None
    assert "pv01::USD_x2dOIS" not in result
    with pytest.raises(KeyError):
        result["pv01::USD_x2dOIS"]


@pytest.mark.parametrize("key", ["pv01::USD_x2dOIS", "pv01::curve_xray", "bucketed_dv01::"])
def test_noncanonical_keys_from_json_are_rejected(key: str) -> None:
    result = price_instrument(_bond(), _market(), AS_OF)
    payload = json.loads(result.to_json())
    payload["measures"] = {key: 12.5}
    with pytest.raises(ValueError, match="noncanonical"):
        ValuationResult.from_json(json.dumps(payload))


def test_canonical_keys_preserve_distinct_escape_labels() -> None:
    result = price_instrument(_bond(), _market(), AS_OF)
    payload = json.loads(result.to_json())
    payload["measures"] = {"pv01::USD-OIS": 12.5, "pv01::USD_x5fx2dOIS": 3.0}
    restored = ValuationResult.from_json(json.dumps(payload))
    assert restored.metric_series("pv01") == [(["USD-OIS"], 12.5), (["USD_x2dOIS"], 3.0)]
    assert restored.get_metric("pv01::USD-OIS") == 12.5
    assert restored.get_metric("pv01::USD_x5fx2dOIS") == 3.0


# ValuationResult ergonomics


def test_valuation_result_dict_access_and_money_value() -> None:
    result = price_instrument(_bond(), _market(), AS_OF, metrics=["ytm", "dv01", "duration_mod"])

    assert isinstance(result.value, Money)
    assert result.value.currency.code == "USD"
    assert result.value.amount == pytest.approx(result.price)

    metrics = result.metrics
    assert list(metrics) == result.metric_keys()
    assert metrics["dv01"] == result["dv01"] == result.get_metric("dv01")
    assert "ytm" in result
    assert "vega" not in result

    with pytest.raises(KeyError) as excinfo:
        result["DV01"]
    assert "dv01" in str(excinfo.value)

    assert result.covenants is None
    assert result.explanation is None
    assert result.all_covenants_passed()

    units = result.metric_units()
    assert units["ytm"] == "decimal"
    assert units["dv01"] == "currency"
    assert units["duration_mod"] == "years"

    html = result._repr_html_()
    assert isinstance(html, str)
    assert "<table" in html
    assert ValuationResult.__doc__
    assert "Valuation envelope" in ValuationResult.__doc__


def test_valuation_result_structural_equality_and_pickle() -> None:
    result = price_instrument(_bond(), _market(), AS_OF, metrics=["ytm"])
    clone = pickle.loads(pickle.dumps(result))  # noqa: S301
    assert clone == result
    assert result != "not a result"

    other = price_instrument(_bond(), _market(), AS_OF, metrics=["dv01"])
    assert other != result


def test_long_and_series_dataframes() -> None:
    result = price_instrument(_swap(), _market(), AS_OF, metrics=["dv01", "bucketed_dv01"])

    long = result.to_long_dataframe()
    assert list(long.columns) == ["metric", "curve", "bucket", "value"]
    scalar = long[long["metric"] == "dv01"]
    assert len(scalar) == 1
    assert scalar["curve"].isna().all()
    buckets = long[(long["metric"] == "bucketed_dv01") & long["bucket"].notna()]
    assert set(buckets["curve"]) == {"USD-OIS", "USD-SOFR-3M"}

    series = result.metric_series_dataframe("bucketed_dv01")
    assert list(series.columns) == ["metric", "curve", "bucket", "value"]
    assert len(series) == len(result.metric_series("bucketed_dv01"))
    assert series["value"].sum() == pytest.approx(sum(value for _, value in result.metric_series("bucketed_dv01")))


# Error kinds survive the pricer boundary


def test_missing_curve_raises_key_error() -> None:
    with pytest.raises(KeyError, match="USD-OIS"):
        price_instrument(_bond(), MarketContext(), AS_OF)


def test_validation_failure_raises_value_error() -> None:
    # A seasoned floating leg without a fixing series is a validation failure.
    start = datetime.date(2023, 7, 17)
    end = datetime.date(2028, 7, 17)
    seasoned = (
        InterestRateSwap
        .builder()
        .id("SWP-SEASONED")
        .notional(Money(1_000_000.0, Currency("USD")))
        .side("pay")
        .fixed(
            FixedLegSpec(
                "USD-OIS",
                0.04,
                Tenor.semi_annual(),
                DayCount.THIRTY_360,
                start,
                end,
                compounding_simple=False,
            )
        )
        .float(
            FloatLegSpec(
                "USD-OIS",
                "USD-SOFR-3M",
                0.0,
                Tenor.quarterly(),
                DayCount.ACT_360,
                start,
                end,
            )
        )
        .build()
    )
    with pytest.raises(ValueError, match="fixings"):
        price_instrument(seasoned, _market(), AS_OF)


def test_unknown_metric_error_is_short_and_case_folded() -> None:
    with pytest.raises(ValueError, match=r"Unknown metric") as excinfo:
        price_instrument(_bond(), _market(), AS_OF, metrics=["DV01"])
    message = str(excinfo.value)
    assert "dv01" in message
    suggestions = message.rsplit("Did you mean", maxsplit=1)[-1]
    assert suggestions.count(",") <= 4, message


# price_instrument signature: `instrument` keyword, typed/dict options


def test_price_instrument_keyword_and_typed_options() -> None:
    market = _market()
    by_keyword = price_instrument(instrument=_bond(), market=market, as_of=AS_OF, metrics=["theta"])
    assert "theta" in by_keyword

    opts = MetricPricingOverrides(theta_period="1W")
    assert opts.theta_period == "1W"
    assert opts == MetricPricingOverrides.from_json(opts.to_json())
    assert pickle.loads(pickle.dumps(opts)) == opts  # noqa: S301
    assert "theta_period='1W'" in repr(opts)

    typed = price_instrument(_bond(), market, AS_OF, metrics=["theta"], pricing_options=opts)
    as_dict = price_instrument(_bond(), market, AS_OF, metrics=["theta"], pricing_options={"theta_period": "1W"})
    as_str = price_instrument(
        _bond(), market, AS_OF, metrics=["theta"], pricing_options=json.dumps({"theta_period": "1W"})
    )
    assert typed["theta"] == as_dict["theta"] == as_str["theta"]
    assert typed["theta"] != by_keyword["theta"]

    with pytest.raises(ValueError, match=r"Invalid input data"):
        MetricPricingOverrides(theta_period="soon")
    with pytest.raises(ValueError, match=r"bond_risk_basis: expected"):
        MetricPricingOverrides(bond_risk_basis="nope")


def test_market_history_typed_twin() -> None:
    scenarios = [
        {
            "date": "2024-01-12",
            "shifts": [
                {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": 0.001}
            ],
        },
        {
            "date": "2024-01-11",
            "shifts": [
                {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": -0.0005}
            ],
        },
    ]
    history = MarketHistory(AS_OF, 2, scenarios)
    assert len(history) == 2
    assert history.base_date == AS_OF
    assert history.window_days == 2
    assert history.scenarios[0]["date"] == "2024-01-12"
    assert MarketHistory.from_dict(json.loads(history.to_json())).to_json() == history.to_json()
    assert pickle.loads(pickle.dumps(history)).to_json() == history.to_json()  # noqa: S301

    frame = history.to_dataframe()
    assert list(frame.columns)[:3] == ["date", "type", "curve_id"]
    assert frame["shift"].tolist() == [0.001, -0.0005]
    assert (frame["type"] == "discount_rate").all()

    typed = price_instrument(_bond(), _market(), AS_OF, metrics=["hvar"], market_history=history)
    as_dict = price_instrument(
        _bond(), _market(), AS_OF, metrics=["hvar"], market_history=json.loads(history.to_json())
    )
    assert typed["hvar"] == as_dict["hvar"]


def test_instrument_cashflows_accepts_typed_instrument_and_date() -> None:
    envelope, frame = instrument_cashflows(_bond(), _market(), AS_OF, model="discounting")
    assert envelope["instrument_id"] == "B1"
    assert isinstance(frame, pd.DataFrame)
    assert len(frame) > 0
    assert envelope["total_pv"] == pytest.approx(price_instrument(_bond(), _market(), AS_OF).price, abs=0.01)
