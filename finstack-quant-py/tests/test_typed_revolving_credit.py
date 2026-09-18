"""Typed ``RevolvingCredit`` class, builder and per-path Monte Carlo result.

Mirrors the Rust ``revolving_credit`` surface: envelope round trips, the
builder one setter per Rust setter, deterministic and stochastic pricing, the
path-retaining ``price_with_paths`` and the draw option cost metric.
"""

from __future__ import annotations

import datetime
import json
import pickle

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    HazardCurve,
    MarketContext,
    ScalarTimeSeries,
)
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    EnhancedMonteCarloResult,
    RevolvingCredit,
    RevolvingCreditBuilder,
    instrument_cashflows_json,
    price_instrument,
)

AS_OF = datetime.date(2024, 1, 15)
MATURITY = datetime.date(2027, 1, 15)
HAZARD_ID = "BORROWER-HZ"


def market() -> MarketContext:
    """Flat 3% OIS discounting, a flat 4% SOFR-3M index with its fixings and a rising hazard curve."""
    fixings = [(AS_OF - datetime.timedelta(days=days), 0.04) for days in range(25)]
    ctx = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.03))
        .insert(ForwardCurve("USD-SOFR-3M", 0.25, AS_OF, [(0.0, 0.04), (10.0, 0.04)], day_count="act_360"))
        .insert(HazardCurve(HAZARD_ID, AS_OF, [(1.0, 0.04), (3.0, 0.10), (5.0, 0.14)], recovery_rate=0.4))
    )
    ctx.insert_series(ScalarTimeSeries("FIXING:USD-SOFR-3M", fixings))
    return ctx


def stochastic_spec(volatility: float, spread: dict[str, object], *, credit: bool) -> dict[str, object]:
    return {
        "stochastic": {
            "utilization_process": {
                "mean_reverting": {
                    "target_rate": 0.6,
                    "speed": 1.0,
                    "volatility": volatility,
                    "spread_sensitivity": 0.0,
                }
            },
            "num_paths": 16,
            "seed": 42,
            "antithetic": True,
            "use_sobol_qmc": False,
            "mc_config": {
                "correlation_matrix": None,
                "credit_spread_process": spread,
                "interest_rate_process": None,
                "util_credit_corr": 0.5 if credit else None,
            },
        }
    }


def builder(draw_repay_spec: dict[str, object] | None = None, *, credit: bool = False) -> RevolvingCreditBuilder:
    """Floating SOFR + 250bp facility, USD 50M committed, USD 10M drawn."""
    b = (
        RevolvingCredit
        .builder()
        .id("RCF-PY")
        .commitment_amount(Money(50_000_000.0, Currency("USD")))
        .drawn_amount(10_000_000.0, currency="USD")
        .commitment_date(AS_OF)
        .maturity(MATURITY)
        .base_rate_spec(RevolvingCredit.example().base_rate_spec)
        .day_count("act_360")
        .frequency("3M")
        .fees_flat(25.0, 10.0, 5.0)
        .draw_repay_spec(draw_repay_spec if draw_repay_spec is not None else {"deterministic": []})
        .discount_curve_id("USD-OIS")
        .recovery_rate(0.4)
        .leq(0.5 if credit else 0.0)
    )
    if credit:
        b = b.credit_curve_id(HAZARD_ID)
    return b


def market_anchored() -> dict[str, object]:
    return {
        "market_anchored": {
            "credit_curve_id": HAZARD_ID,
            "kappa": 0.5,
            "implied_vol": 0.4,
            "tenor_years": None,
        }
    }


def test_example_round_trips_through_the_envelope_and_pickle() -> None:
    facility = RevolvingCredit.example()
    assert facility.id == "RCF-USD-3Y"
    assert not facility.is_stochastic
    envelope = json.loads(facility.to_json())
    assert envelope["instrument"]["type"] == "revolving_credit"
    again = RevolvingCredit.from_json(facility.to_json())
    assert json.loads(again.to_json()) == envelope
    restored = pickle.loads(pickle.dumps(facility))  # noqa: S301 - trusted in-process round trip
    assert json.loads(restored.to_json()) == envelope
    assert facility.to_dict()["id"] == "RCF-USD-3Y"
    assert "revolving_credit" not in facility.to_dict()


def test_from_json_rejects_other_instrument_types() -> None:
    from finstack_quant.valuations.instruments import TermLoan

    with pytest.raises(ValueError, match="revolving_credit"):
        RevolvingCredit.from_json(TermLoan.example().to_json())
    with pytest.raises(ValueError, match=r"(?i)json"):
        RevolvingCredit.from_json("{not json")


def test_builder_sets_every_field_and_getters_read_them_back() -> None:
    facility = builder(credit=True).build()
    assert facility.id == "RCF-PY"
    assert facility.commitment_amount == Money(50_000_000.0, Currency("USD"))
    assert facility.drawn_amount == Money(10_000_000.0, Currency("USD"))
    assert facility.commitment_date == AS_OF
    assert facility.maturity == MATURITY
    assert "floating" in facility.base_rate_spec
    assert str(facility.day_count) == "act_360"
    assert str(facility.frequency) == "3M"
    assert facility.fees["facility_fee_bp"] == 5.0
    assert facility.draw_repay_spec == {"deterministic": []}
    assert facility.discount_curve_id == "USD-OIS"
    assert facility.credit_curve_id == HAZARD_ID
    assert facility.recovery_rate == 0.4
    assert facility.leq == 0.5
    assert str(facility.stub) == "short_front"
    assert facility.expiry in (None, MATURITY)
    assert isinstance(facility.default_model, str)
    deps = facility.market_dependencies()
    assert "USD-OIS" in json.dumps(deps)
    assert "RevolvingCreditBuilder" in repr(RevolvingCredit.builder().id("X"))
    assert "RCF-PY" in repr(facility)


def test_builder_accepts_a_bare_fixed_rate_and_is_consumed_by_build() -> None:
    b = builder().base_rate_spec(0.06)
    facility = b.build()
    assert facility.base_rate_spec == {"fixed": {"rate": 0.06}}
    with pytest.raises(ValueError, match="consumed"):
        b.build()


def test_builder_reports_missing_required_fields() -> None:
    with pytest.raises(ValueError, match=r"(?i)missing|required"):
        RevolvingCredit.builder().id("RCF-MISSING").build()


def test_deterministic_facility_prices_like_price_instrument() -> None:
    facility = RevolvingCredit.example()
    ctx = market()
    typed = facility.price(ctx, AS_OF, metrics=["dv01"])
    via_json = price_instrument(facility.to_json(), ctx, AS_OF, "default", metrics=["dv01"])
    assert typed.value.amount == pytest.approx(via_json.value.amount, rel=1e-12)
    assert facility.metric(ctx, AS_OF, "dv01") == pytest.approx(via_json.metrics["dv01"], rel=1e-9)
    # A deterministic schedule has no simulated draws, hence no draw option cost.
    assert facility.metric(ctx, AS_OF, "draw_option_cost") == 0.0
    with pytest.raises(ValueError, match="stochastic"):
        facility.price_with_paths(ctx, AS_OF)


def test_expected_cashflows_matches_the_json_schedule() -> None:
    facility = RevolvingCredit.example()
    ctx = market()
    schedule = facility.expected_cashflows(ctx, AS_OF)
    frame = schedule.to_dataframe()
    assert not frame.empty
    via_json = json.loads(instrument_cashflows_json(facility.to_json(), ctx, AS_OF, "discounting"))
    assert len(json.loads(schedule.to_json())["flows"]) == len(via_json["flows"])


def test_price_with_paths_returns_every_path_and_the_option_cost() -> None:
    facility = builder(stochastic_spec(0.25, market_anchored(), credit=True), credit=True).build()
    assert facility.is_stochastic
    ctx = market()
    result = facility.price_with_paths(ctx, AS_OF)
    assert isinstance(result, EnhancedMonteCarloResult)
    assert result.num_simulated_paths == 16
    assert result.num_paths == 8  # antithetic pairs
    assert result.pv.currency == Currency("USD")
    assert len(result.path_pvs) == 16
    assert len(result.path_draw_option_costs) == 16
    assert len(result.utilization_paths) == 16
    assert len(result.credit_spread_paths) == 16
    assert len(result.observation_dates) == len(result.utilization_paths[0])
    # Draws at a margin below a widening fair spread cost the lender.
    assert result.draw_option_cost.amount < 0.0
    assert result.draw_option_cost_ci_95[1].amount <= result.draw_option_cost_ci_95[1].amount
    assert result.pv_ci_95[0].amount <= result.pv.amount <= result.pv_ci_95[1].amount

    frame = result.to_dataframe()
    assert list(frame.columns) == ["path", "pv", "draw_option_cost"]
    assert len(frame) == 16
    assert frame["draw_option_cost"].mean() == pytest.approx(sum(result.path_draw_option_costs) / 16)

    # JSON, dict and pickle round trips.
    again = EnhancedMonteCarloResult.from_json(result.to_json())
    assert again.draw_option_cost == result.draw_option_cost
    restored = pickle.loads(pickle.dumps(result))  # noqa: S301 - trusted in-process round trip
    assert restored.pv == result.pv
    assert set(result.to_dict()) == {"mc_result", "path_results", "draw_option_cost"}
    assert "EnhancedMonteCarloResult" in repr(result)

    # The metric reports the same Monte Carlo mean.
    metric = facility.metric(ctx, AS_OF, "draw_option_cost")
    assert metric == pytest.approx(result.draw_option_cost.amount, rel=1e-9)


def test_constant_spread_at_the_margin_has_zero_option_cost() -> None:
    facility = builder(stochastic_spec(0.25, {"constant": 0.025}, credit=False)).build()
    result = facility.price_with_paths(market(), AS_OF)
    assert all(cost == 0.0 for cost in result.path_draw_option_costs)
    assert result.draw_option_cost.amount == 0.0


def test_typed_instance_is_accepted_by_price_instrument() -> None:
    facility = RevolvingCredit.example()
    result = price_instrument(facility, market(), AS_OF, "default")
    assert result.value.amount == pytest.approx(facility.price(market(), AS_OF).value.amount)


def test_builder_accepts_dated_commitment_margin_and_fee_steps() -> None:
    fees = {
        "upfront_fee": None,
        "commitment_fee_tiers": [{"threshold": "0", "bp": "50"}],
        "usage_fee_tiers": [],
        "facility_fee_bp": 0.0,
        "steps": [{"date": "2026-01-15", "commitment_delta_bp": 25.0, "usage_delta_bp": 0.0, "facility_delta_bp": 0.0}],
    }
    facility = (
        builder()
        .fees(fees)
        .commitment_schedule([
            {"date": "2026-07-15", "amount": {"amount": "30000000", "currency": "USD"}, "fee_bp": 25.0}
        ])
        .margin_steps([{"date": "2026-01-15", "delta_bp": 100}])
        .build()
    )
    assert facility.commitment_schedule[0]["amount"]["amount"] == "30000000"
    assert facility.margin_steps == [{"date": "2026-01-15", "delta_bp": 100}]
    assert facility.fees["steps"][0]["commitment_delta_bp"] == 25.0
    round_trip = RevolvingCredit.from_json(facility.to_json())
    assert round_trip.commitment_schedule == facility.commitment_schedule
    # A step below the drawn balance is a build-time error naming the step.
    with pytest.raises(ValueError, match="below the drawn balance"):
        builder().commitment_schedule([
            {"date": "2026-07-15", "amount": {"amount": "5000000", "currency": "USD"}, "fee_bp": 0.0}
        ]).build()


def test_builder_accepts_a_letter_of_credit_sublimit() -> None:
    lc = {
        "sublimit": {"amount": "4000000", "currency": "USD"},
        "outstanding": {"amount": "2000000", "currency": "USD"},
        "events": [{"date": "2026-01-15", "amount": {"amount": "1000000", "currency": "USD"}, "is_issue": True}],
        "fee_bp": None,
        "fronting_fee_bp": 12.5,
        "leq": 0.5,
    }
    facility = builder().lc(lc).build()
    assert facility.lc["fronting_fee_bp"] == 12.5
    assert facility.lc["events"][0]["is_issue"] is True
    assert builder().lc(None).build().lc is None
    with pytest.raises(ValueError, match="sublimit"):
        builder().lc({**lc, "outstanding": {"amount": "5000000", "currency": "USD"}}).build()


def test_builder_accepts_percentage_upfront_scheduled_fees_and_oid_switch() -> None:
    fees = {
        "upfront_fee": {"pct_of_commitment": 0.02},
        "commitment_fee_tiers": [{"threshold": "0", "bp": "50"}],
        "usage_fee_tiers": [],
        "facility_fee_bp": 0.0,
        "steps": [],
    }
    facility = (
        builder()
        .fees(fees)
        .scheduled_fees([{"date": "2026-03-01", "amount": {"amount": "100000", "currency": "USD"}}])
        .oid_eir({"include_fees": False})
        .build()
    )
    assert facility.fees["upfront_fee"] == {"pct_of_commitment": 0.02}
    assert facility.scheduled_fees[0]["amount"]["amount"] == "100000"
    assert facility.oid_eir == {"include_fees": False}
    assert builder().oid_eir(None).build().oid_eir is None
    without_fees = facility.price(market(), AS_OF, metrics=["oid_eir_amortization"]).metrics["oid_eir_rate"]
    with_fees = (
        builder()
        .fees(fees)
        .oid_eir({"include_fees": True})
        .build()
        .price(market(), AS_OF, metrics=["oid_eir_amortization"])
        .metrics["oid_eir_rate"]
    )
    # The 2% upfront fee and the running fees lift the origination effective rate.
    assert with_fees > without_fees
