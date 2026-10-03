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
from finstack_quant.valuations import ValuationResult, schema
from finstack_quant.valuations.instruments import (
    Bond,
    CdsOption,
    CreditDefaultSwap,
    FixedLegSpec,
    FloatLegSpec,
    InterestRateSwap,
    MarketHistory,
    MetricPricingOverrides,
    TermLoan,
    VarResult,
    calculate_var_with_pricing,
    instrument_cashflows,
    instrument_envelope_from_spec,
    metric_metadata,
    pretty_instrument_json,
    price_instrument,
    validate_instrument_json,
    validate_typed_instrument_json,
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
        .fixed_leg(fixed)
        .float_leg(floating)
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


@pytest.mark.parametrize("typed", [False, True])
def test_cds_missing_curve_raises_key_error(typed: bool) -> None:
    cds = CreditDefaultSwap.example()
    if typed:
        with pytest.raises(KeyError, match="USD-OIS"):
            cds.price(MarketContext(), AS_OF)
    else:
        with pytest.raises(KeyError, match="USD-OIS"):
            price_instrument(cds, MarketContext(), AS_OF)


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
        .fixed_leg(
            FixedLegSpec(
                "USD-OIS",
                0.04,
                Tenor.semi_annual(),
                DayCount.THIRTY_360,
                start,
                end,
            )
        )
        .float_leg(
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

    opts = MetricPricingOverrides(theta_period="1W", theta_day_basis="trading_252")
    assert str(opts.theta_period) == "1W"
    assert opts.theta_day_basis == "trading_252"
    assert opts == MetricPricingOverrides.from_json(opts.to_json())
    assert pickle.loads(pickle.dumps(opts)) == opts  # noqa: S301
    assert "theta_period='1W'" in repr(opts)

    typed = price_instrument(_bond(), market, AS_OF, metrics=["theta"], metric_pricing_overrides=opts)
    as_dict = price_instrument(
        _bond(),
        market,
        AS_OF,
        metrics=["theta"],
        metric_pricing_overrides={"theta_period": {"count": 1, "unit": "weeks"}, "theta_day_basis": "trading_252"},
    )
    as_str = price_instrument(
        _bond(),
        market,
        AS_OF,
        metrics=["theta"],
        metric_pricing_overrides=json.dumps({
            "theta_period": {"count": 1, "unit": "weeks"},
            "theta_day_basis": "trading_252",
        }),
    )
    assert typed["theta"] == as_dict["theta"] == as_str["theta"]
    assert typed["theta"] != by_keyword["theta"]

    with pytest.raises(ValueError, match=r"Invalid tenor"):
        MetricPricingOverrides(theta_period="soon")
    with pytest.raises(ValueError, match=r"theta_day_basis: expected"):
        MetricPricingOverrides(theta_day_basis="nope")
    with pytest.raises(ValueError, match=r"MetricPricingOverrides"):
        MetricPricingOverrides.from_json('{"theta_period": "1W"}')  # schema-rejection-test: retired string form
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
    envelope = instrument_cashflows(_bond(), _market(), AS_OF, "discounting")
    frame = envelope.to_dataframe()
    assert envelope.instrument_id == "B1"
    assert isinstance(frame, pd.DataFrame)
    assert len(frame) > 0
    assert pickle.loads(pickle.dumps(envelope)).to_json() == envelope.to_json()  # noqa: S301
    assert envelope.total_pv == pytest.approx(price_instrument(_bond(), _market(), AS_OF).price, abs=0.01)


def test_metric_metadata_describes_canonical_keys() -> None:
    keys = [
        "ytm",
        "bucketed_dv01::A_x3a_x3aB::5y",
        "custom_metric",
        "pv01::USD-OIS",
        "ytm",
    ]
    metadata = metric_metadata(keys)
    assert [entry["key"] for entry in metadata] == keys
    assert metadata[0] == {
        "key": "ytm",
        "metric": "ytm",
        "components": [],
        "unit": "decimal",
        "group": "Pricing",
        "bucketed": False,
    }
    assert metadata[1] == {
        "key": "bucketed_dv01::A_x3a_x3aB::5y",
        "metric": "bucketed_dv01",
        "components": ["A::B", "5y"],
        "unit": "currency",
        "group": "Sensitivity",
        "bucketed": True,
    }
    assert metadata[2] == {
        "key": "custom_metric",
        "metric": "custom_metric",
        "components": [],
        "unit": "unknown",
        "group": None,
        "bucketed": False,
    }
    assert metadata[3]["components"] == ["USD-OIS"]
    assert metadata[3]["bucketed"] is False
    assert metric_metadata([]) == []
    with pytest.raises(ValueError, match="noncanonical composite metric key"):
        metric_metadata(["pv01::USD_x2dOIS"])


def test_validate_instrument_json_merges_overrides_before_validation() -> None:
    document = json.loads(_bond().to_json())
    document["instrument"]["spec"]["metric_pricing_overrides"] = {"theta_period": "invalid"}
    raw = json.dumps(document)

    with pytest.raises(ValueError, match="invalid instrument envelope JSON"):
        validate_instrument_json(raw)
    prepared = validate_instrument_json(raw, metric_pricing_overrides='{"theta_period":{"count":1,"unit":"weeks"}}')
    assert json.loads(prepared)["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"] == {
        "count": 1,
        "unit": "weeks",
    }
    assert validate_instrument_json(prepared) == prepared
    with pytest.raises(ValueError, match="invalid metric_pricing_overrides JSON"):
        validate_instrument_json(raw, metric_pricing_overrides="{")


@pytest.mark.parametrize("patch", [{}, {"theta_day_basis": "trading_252"}])
def test_partial_metric_override_patch_retains_existing_theta_period(patch: dict) -> None:
    document = json.loads(_bond().to_json())
    period = {"count": 1, "unit": "weeks"}
    document["instrument"]["spec"]["metric_pricing_overrides"] = {"theta_period": period}
    raw = json.dumps(document)
    baseline = price_instrument(raw, _market(), AS_OF, metrics=["theta"])
    prepared = json.loads(validate_instrument_json(raw, metric_pricing_overrides=patch))
    assert prepared["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"] == period
    patched = price_instrument(raw, _market(), AS_OF, metrics=["theta"], metric_pricing_overrides=patch)
    assert patched["theta"] == pytest.approx(baseline["theta"])


def test_explicit_null_metric_override_patch_clears_theta_period() -> None:
    document = json.loads(_bond().to_json())
    document["instrument"]["spec"]["metric_pricing_overrides"] = {
        "theta_period": {"count": 1, "unit": "weeks"},
    }
    raw = json.dumps(document)
    patched = price_instrument(
        raw, _market(), AS_OF, metrics=["theta"], metric_pricing_overrides={"theta_period": None}
    )
    baseline = price_instrument(_bond(), _market(), AS_OF, metrics=["theta"])
    assert patched["theta"] == pytest.approx(baseline["theta"])


@pytest.mark.parametrize(
    ("quote_field", "low_quote", "high_quote"),
    [("quoted_z_spread", 0.0, 0.05), ("quoted_clean_price_pct", 80.0, 110.0)],
)
def test_term_loan_cs01_uses_the_quote_anchor(quote_field: str, low_quote: float, high_quote: float) -> None:
    results = []
    for quote in (low_quote, high_quote):
        document = json.loads(TermLoan.example().to_json())
        document["instrument"]["spec"]["instrument_pricing_overrides"] = {
            "market_quotes": {quote_field: quote},
        }
        result = price_instrument(
            json.dumps(document), _market(), AS_OF, model="discounting", metrics=["cs01", "bucketed_cs01"]
        )
        assert result["bucketed_cs01"] == pytest.approx(result["cs01"])
        results.append(result["cs01"])
    assert results[0] != pytest.approx(results[1])


def test_every_pricing_entry_point_accepts_the_same_metric_pricing_overrides_types() -> None:
    """``price_instrument``, typed ``.price()`` and ``validate_instrument_json`` share one coercion."""
    market = _market()
    bond = _bond()
    typed = MetricPricingOverrides(theta_period="1W")
    as_dict = {"theta_period": {"count": 1, "unit": "weeks"}}
    expected = price_instrument(bond, market, AS_OF, metrics=["theta"], metric_pricing_overrides=typed)["theta"]
    for value in (typed, as_dict, json.dumps(as_dict)):
        assert bond.price(market, AS_OF, metrics=["theta"], metric_pricing_overrides=value)["theta"] == expected
        prepared = json.loads(validate_instrument_json(bond.to_json(), metric_pricing_overrides=value))
        assert prepared["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"] == as_dict["theta_period"]


def test_pricing_entry_points_reject_retired_pricing_options_keyword() -> None:
    market = _market()
    bond = _bond()
    with pytest.raises(TypeError):
        bond.price(market, AS_OF, pricing_options={})  # schema-rejection-test: retired kwarg pricing_options
    with pytest.raises(TypeError):
        price_instrument(
            bond, market, AS_OF, pricing_options={}
        )  # schema-rejection-test: retired kwarg pricing_options
    with pytest.raises(TypeError):
        validate_instrument_json(
            bond.to_json(), pricing_options="{}"
        )  # schema-rejection-test: retired kwarg pricing_options


# VALA-006 — multi-instrument historical VaR (calculate_var_with_pricing)


def _rate_history() -> MarketHistory:
    shift = lambda s: [  # noqa: E731
        {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": s}
    ]
    return MarketHistory(
        AS_OF,
        3,
        [
            {"date": "2024-01-12", "shifts": shift(0.0010)},
            {"date": "2024-01-11", "shifts": shift(-0.0005)},
            {"date": "2024-01-10", "shifts": shift(0.0020)},
        ],
    )


def test_calculate_var_single_instrument_matches_hvar_metric() -> None:
    history = _rate_history()
    result = calculate_var_with_pricing([_bond()], _market(), history, AS_OF)
    metric = price_instrument(_bond(), _market(), AS_OF, metrics=["hvar", "expected_shortfall"], market_history=history)

    assert isinstance(result, VarResult)
    assert result.num_scenarios == 3
    assert result.confidence_level == 0.95
    assert result.var < 0.0
    assert result.var == pytest.approx(metric["hvar"], rel=1e-12, abs=1e-9)
    assert result.expected_shortfall == pytest.approx(metric["expected_shortfall"], rel=1e-12, abs=1e-9)
    assert result.pnl_distribution == sorted(result.pnl_distribution)


def test_calculate_var_diversifies_offsetting_positions() -> None:
    shift = lambda s: [  # noqa: E731
        {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": s},
        {"factor": {"type": "forward_rate", "curve_id": "USD-SOFR-3M", "tenor_years": 5.0}, "shift": s},
    ]
    history = MarketHistory(
        AS_OF,
        3,
        [
            {"date": "2024-01-12", "shifts": shift(0.0010)},
            {"date": "2024-01-11", "shifts": shift(-0.0005)},
            {"date": "2024-01-10", "shifts": shift(0.0020)},
        ],
    )
    bond_var = calculate_var_with_pricing([_bond()], _market(), history, AS_OF).var
    swap_var = calculate_var_with_pricing([_swap()], _market(), history, AS_OF).var
    # Long bond (loses when rates rise) against a pay-fixed swap (gains).
    hedged = calculate_var_with_pricing([_bond(), _swap().to_json()], _market(), history, AS_OF)

    assert bond_var < -1.0
    assert swap_var < -1.0
    assert hedged.num_scenarios == 3
    assert abs(hedged.var) < abs(bond_var) + abs(swap_var) - 1.0
    doubled = calculate_var_with_pricing([_bond(), _bond()], _market(), history, AS_OF)
    assert doubled.var == pytest.approx(2.0 * bond_var, rel=1e-12)


def test_calculate_var_config_model_and_result_round_trip() -> None:
    history = _rate_history()
    result = calculate_var_with_pricing(
        [_bond()],
        _market(),
        json.loads(history.to_json()),
        AS_OF,
        config={"confidence_level": 0.99, "method": "full_revaluation"},
        model="discounting",
    )
    assert result.confidence_level == 0.99
    assert VarResult.from_json(result.to_json()) == result
    assert pickle.loads(pickle.dumps(result)) == result  # noqa: S301
    frame = result.to_dataframe()
    assert list(frame.columns) == ["pnl"]
    assert frame["pnl"].tolist() == result.pnl_distribution

    empty = calculate_var_with_pricing([], _market(), history, AS_OF)
    assert (empty.var, empty.num_scenarios) == (0.0, 0)
    with pytest.raises(ValueError, match="strictly between"):
        calculate_var_with_pricing([_bond()], _market(), history, AS_OF, config={"confidence_level": 1.5})
    with pytest.raises(ValueError, match="unknown field"):
        calculate_var_with_pricing([_bond()], _market(), history, AS_OF, config={"confidence": 0.99})
    with pytest.raises(ValueError, match="no_such_model"):
        calculate_var_with_pricing([_bond()], _market(), history, AS_OF, model="no_such_model")


# VALA-007 — instrument_envelope_from_spec for JSON-only instrument types


def test_instrument_envelope_from_spec_wraps_and_validates_bare_spec() -> None:
    spec = {
        "id": "EURUSD-SPOT",
        "base_currency": "EUR",
        "quote_currency": "USD",
        "settlement_date": "2025-01-17",
        "quoted_spot": 1.2,
        "notional": {"amount": "1000000", "currency": "EUR"},
        "attributes": {},
    }
    envelope = instrument_envelope_from_spec("fx_spot", spec)
    # FUP-003: the envelope is a value (dict), not JSON text.
    assert isinstance(envelope, dict)
    assert envelope["schema"] == "finstack_quant.instrument/1"
    assert envelope["instrument"]["type"] == "fx_spot"
    assert json.loads(validate_instrument_json(envelope)) == envelope
    assert instrument_envelope_from_spec("fx_spot", json.dumps(spec)) == envelope

    with pytest.raises(ValueError, match="bare spec"):
        instrument_envelope_from_spec("fx_spot", {"type": "fx_spot", "spec": spec})
    with pytest.raises(ValueError, match="unknown_field"):
        instrument_envelope_from_spec("fx_spot", {**spec, "unknown_field": 1})


def test_instrument_envelope_dict_is_accepted_by_instrument_entry_points() -> None:
    """FUP-003: the returned dict prices and validates without ``json.dumps``."""
    bond = _bond()
    envelope = instrument_envelope_from_spec("bond", bond.to_dict())
    assert isinstance(envelope, dict)

    priced = price_instrument(envelope, _market(), AS_OF)
    assert priced.price == price_instrument(bond, _market(), AS_OF).price
    assert priced.price == price_instrument(json.dumps(envelope), _market(), AS_OF).price

    canonical = validate_instrument_json(envelope)
    assert json.loads(canonical) == envelope
    assert validate_typed_instrument_json("bond", envelope) == canonical
    assert json.loads(pretty_instrument_json(envelope)) == envelope
    assert json.loads(schema.validate_instrument_envelope_json(envelope)) == envelope
    assert json.loads(schema.validate_instrument_type_json("bond", envelope)) == envelope
    assert instrument_cashflows(envelope, _market(), AS_OF, "discounting").to_json() == (
        instrument_cashflows(bond, _market(), AS_OF, "discounting").to_json()
    )

    var = calculate_var_with_pricing([envelope], _market(), _rate_history(), AS_OF)
    assert var.var == calculate_var_with_pricing([bond], _market(), _rate_history(), AS_OF).var

    with pytest.raises(TypeError, match="dict or JSON string"):
        price_instrument(1.5, _market(), AS_OF)
    with pytest.raises(TypeError, match="dict or JSON string"):
        validate_instrument_json(1.5)


# VALB-007 — typed CdsOption


def test_cds_option_typed_wrapper_round_trips_and_matches_example_json() -> None:
    from finstack_quant.valuations.credit_derivatives import cds_option_example_json

    option = CdsOption.example()
    assert option.to_json() == cds_option_example_json()
    assert CdsOption.from_json(option.to_json()).to_json() == option.to_json()
    assert pickle.loads(pickle.dumps(option)).to_json() == option.to_json()  # noqa: S301
    assert option.strike == {"spread": "0.01"}
    assert (option.option_type, option.settlement, option.exercise_style) == ("call", "cash", "european")
    assert option.expiry == datetime.date(2025, 6, 20)
    assert option.notional.amount == 10_000_000.0
    assert option.coupon_bp is None
    assert option.underlying_convention == "isda_na"
    assert option.to_dict()["vol_surface_id"] == "CDSOPT-VOL"
    assert validate_instrument_json(option.to_json()) == option.to_json()


def test_cds_option_builder_sets_every_field_and_validates() -> None:
    import decimal

    option = (
        CdsOption
        .builder()
        .id("CDXO-1")
        .strike({"clean_price_pct": "107.0"})
        .option_type("put")
        .exercise_style("european")
        .expiry("2025-06-20")
        .underlying_maturity(datetime.date(2030, 6, 20))
        .notional(Money(25_000_000.0, Currency("USD")))
        .settlement("physical")
        .recovery_rate(0.3)
        .discount_curve_id("USD-OIS")
        .credit_curve_id("CDX-HY-HAZARD")
        .vol_surface_id("CDX-HY-VOL")
        .underlying_is_index(True)
        .index_factor(0.98)
        .strike_index_factor(1.0)
        .coupon_bp(decimal.Decimal("500"))
        .underlying_convention("isda_na")
        .protection_start_convention("forward")
        .build()
    )
    assert option.strike == {"clean_price_pct": "107.0"}
    assert option.coupon_bp == decimal.Decimal("500")
    assert (option.underlying_is_index, option.index_factor, option.strike_index_factor) == (True, 0.98, 1.0)
    assert option.protection_start_convention == "forward"
    assert CdsOption.from_json(option.to_json()).to_json() == option.to_json()

    with pytest.raises(ValueError, match="missing required field"):
        CdsOption.builder().id("X").build()
    with pytest.raises(ValueError, match="strike"):
        CdsOption.builder().strike({"strike": "0.01"})
