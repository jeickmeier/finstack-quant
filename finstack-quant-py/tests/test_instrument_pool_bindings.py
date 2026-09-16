"""Pools of real instruments through the typed structured-credit bindings.

Covers the ``AssetPool`` instrument-collateral and reserve accessors, the
deterministic simulation diagnostics and the stochastic pricing result with
its draw option cost distribution.
"""

from __future__ import annotations

import datetime
import json
import pickle

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetPool,
    Bond,
    RevolvingCredit,
    SimulationDiagnostics,
    StochasticPricingResult,
    StructuredCredit,
    TermLoan,
    Tranche,
    TrancheStructure,
    price_instrument,
)
from tests.test_typed_revolving_credit import (
    AS_OF,
    builder,
    market,
    market_anchored,
    stochastic_spec,
)

MATURITY = datetime.date(2034, 1, 15)


def tranches(senior: float, equity: float) -> TrancheStructure:
    equity_tranche = (
        Tranche
        .builder()
        .id("EQ")
        .attachment_point(0.0)
        .detachment_point(10.0)
        .seniority("equity")
        .original_balance(Money(equity, Currency("USD")))
        .coupon_fixed(0.0)
        .maturity(MATURITY)
        .build()
    )
    senior_tranche = (
        Tranche
        .builder()
        .id("A")
        .attachment_point(10.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(Money(senior, Currency("USD")))
        .coupon_fixed(0.05)
        .maturity(MATURITY)
        .build()
    )
    return TrancheStructure([equity_tranche, senior_tranche])


def pool_deal(
    revolver: RevolvingCredit, *, reserve: float, destination: str | dict[str, object] = "waterfall"
) -> StructuredCredit:
    pool = (
        AssetPool("POOL-PY", "clo", Currency("USD"))
        .with_instruments(
            bonds=[Bond.example(), Bond.example_floating()],
            term_loans=[TermLoan.example_floating_with_ddtl()],
            revolvers=[revolver],
            call_exercise="first_call",
        )
        .with_reserve(
            Money(reserve, Currency("USD")),
            reserve_account_rate=0.03,
            reserve_target=Money(reserve, Currency("USD")),
            reserve_interest_destination=destination,
        )
    )
    deal = StructuredCredit.new_clo("POOL-DEAL", pool, tranches(48_600_000.0, 5_400_000.0), AS_OF, MATURITY, "USD-OIS")
    return StructuredCredit.from_json(json.dumps(_with_calendar(json.loads(deal.to_json()))))


def _with_calendar(envelope: dict[str, object]) -> dict[str, object]:
    spec = envelope["instrument"]["spec"]
    spec["payment_calendar_id"] = "nyse"
    spec["prepayment_spec"] = {"cpr": 0.0, "curve": None}
    spec["default_spec"] = {"cdr": 0.0, "curve": None}
    return envelope


def test_pool_accessors_report_the_collateral_and_reserve() -> None:
    pool = (
        AssetPool("POOL-PY", "clo", Currency("USD"))
        .with_instruments(
            bonds=[Bond.example()],
            revolvers=[RevolvingCredit.example()],
            call_exercise={"policy": "refinancing_incentive", "threshold_bp": 50.0},
            put_exercise="first_put",
            overrides=[{"id": "US912828XG33", "call": {"policy": "first_call"}}],
        )
        .with_reserve(
            5_000_000.0,
            reserve_account_rate=0.02,
            reserve_target=6_000_000.0,
            reserve_interest_destination={"kind": "tranche", "tranche_id": "EQ"},
            currency="USD",
        )
    )
    collateral = pool.instruments
    assert collateral is not None
    assert [b["id"] for b in collateral["bonds"]] == ["US912828XG33"]
    assert [r["id"] for r in collateral["revolvers"]] == ["RCF-USD-3Y"]
    assert collateral["call_exercise"] == {"policy": "refinancing_incentive", "threshold_bp": 50.0}
    assert collateral["put_exercise"] == {"policy": "first_put"}
    assert collateral["overrides"][0]["call"] == {"policy": "first_call"}
    assert pool.reserve_account == Money(5_000_000.0, Currency("USD"))
    assert pool.reserve_account_rate == 0.02
    assert pool.reserve_target == Money(6_000_000.0, Currency("USD"))
    assert pool.reserve_interest_destination == {"kind": "tranche", "tranche_id": "EQ"}
    assert "instruments=2" in repr(pool)
    # The typed pool round-trips with the collateral intact.
    again = AssetPool.from_json(pool.to_json())
    assert again.instruments == collateral
    assert AssetPool("P", "clo", Currency("USD")).instruments is None
    with pytest.raises(ValueError, match="call_exercise"):
        AssetPool("P", "clo", Currency("USD")).with_instruments(call_exercise="not_a_policy")


def test_deterministic_pool_prices_and_reports_the_reserve_path() -> None:
    deal = pool_deal(
        RevolvingCredit.example(), reserve=40_000_000.0, destination={"kind": "tranche", "tranche_id": "EQ"}
    )
    ctx = market()
    valuation = price_instrument(deal, ctx, AS_OF, "default")
    assert valuation.value.amount > 0.0

    diagnostics = deal.run_simulation_with_diagnostics(ctx, AS_OF)
    assert isinstance(diagnostics, SimulationDiagnostics)
    # The revolver draws 5M and the delayed-draw loan 15M, all from the reserve.
    assert diagnostics.draws_from_reserve == Money(20_000_000.0, Currency("USD"))
    assert diagnostics.unfunded_draws == Money(0.0, Currency("USD"))
    assert diagnostics.reserve_replenished.amount > 0.0
    path = diagnostics.reserve_balance_path
    assert path
    assert all(isinstance(d, datetime.date) for d, _ in path)
    interest = sum(m.amount for _, m in diagnostics.reserve_interest_paid)
    assert interest > 0.0
    frame = diagnostics.to_dataframe()
    assert list(frame.columns) == ["date", "reserve_balance", "reserve_interest"]
    assert frame["reserve_interest"].sum() == pytest.approx(interest)
    again = SimulationDiagnostics.from_json(diagnostics.to_json())
    assert again.draws_from_reserve == diagnostics.draws_from_reserve
    restored = pickle.loads(pickle.dumps(diagnostics))  # noqa: S301 - trusted in-process round trip
    assert restored.to_dict() == diagnostics.to_dict()
    assert "SimulationDiagnostics" in repr(diagnostics)


def test_unfundable_draw_calendar_is_reported_as_a_value_error() -> None:
    deal = pool_deal(RevolvingCredit.example(), reserve=1_000_000.0)
    with pytest.raises(ValueError, match="exceed the reserve"):
        deal.run_simulation_with_diagnostics(market(), AS_OF)


def test_stochastic_pool_reports_draws_and_the_option_cost_by_tranche() -> None:
    revolver = builder(stochastic_spec(0.25, market_anchored(), credit=True), credit=True).build()
    deal = pool_deal(revolver, reserve=40_000_000.0)
    ctx = market()
    result = deal.price_stochastic(ctx, AS_OF, num_paths=32)
    assert isinstance(result, StochasticPricingResult)
    assert result.num_paths == 32
    assert result.pricing_mode == {"monte_carlo": {"num_paths": 32, "antithetic": True}}
    assert result.npv.currency == Currency("USD")
    assert result.expected_collateral_draws.amount > 15_000_000.0
    assert 0.0 <= result.unfunded_draw_path_fraction <= 1.0
    assert result.draw_option_cost.amount < 0.0
    assert len(result.draw_option_cost_paths) == 32
    assert result.pv_confidence_interval[0] <= result.npv.amount <= result.pv_confidence_interval[1]
    assert 0.0 < result.es_confidence < 1.0
    assert result.dirty_price > 0.0
    assert result.expected_loss.amount >= 0.0
    assert result.unexpected_loss.amount >= 0.0
    assert result.expected_shortfall.amount >= 0.0

    tranche_costs = {t["tranche_id"]: t["draw_option_cost"] for t in result.tranche_results}
    total = sum(float(c["amount"]) for c in tranche_costs.values())
    assert total == pytest.approx(result.draw_option_cost.amount, rel=1e-6)

    frame = result.to_dataframe()
    assert list(frame["tranche_id"]) == ["EQ", "A"]
    assert frame["draw_option_cost"].sum() == pytest.approx(result.draw_option_cost.amount, rel=1e-6)
    distribution = result.draw_option_cost_dataframe()
    assert list(distribution.columns) == ["path", "draw_option_cost"]
    assert distribution["draw_option_cost"].mean() == pytest.approx(result.draw_option_cost.amount)

    again = StochasticPricingResult.from_json(result.to_json())
    assert again.draw_option_cost == result.draw_option_cost
    restored = pickle.loads(pickle.dumps(result))  # noqa: S301 - trusted in-process round trip
    assert restored.npv == result.npv
    assert "StochasticPricingResult" in repr(result)
    with pytest.raises(ValueError, match="at least one"):
        deal.price_stochastic(ctx, AS_OF, num_paths=0)
