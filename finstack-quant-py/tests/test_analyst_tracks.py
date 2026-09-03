"""Native economic checks for the two independent analyst track fixtures."""

from __future__ import annotations

from copy import deepcopy
from datetime import date
import importlib
import json
import math
from pathlib import Path
from types import ModuleType

import numpy as np
import pytest

from finstack_quant.core.market_data import DiscountCurve
from finstack_quant.models import bs_price
from finstack_quant.models.correlation import CorrelatedBernoulli, LatentMultiFactor
from finstack_quant.portfolio import value_portfolio
from finstack_quant.valuations.instruments import price_instrument, structured_credit_tranche_metrics


@pytest.fixture(scope="module")
def tracks() -> ModuleType:
    with pytest.MonkeyPatch.context() as patch:
        patch.syspath_prepend(str(Path(__file__).parents[1] / "examples" / "notebooks"))
        return importlib.import_module("_shared.analyst_tracks")


def test_tracks_are_independent_fresh_native_books(tracks: ModuleType) -> None:
    common = tracks.common.book_spec("common")
    before = deepcopy(common)
    credit, vol = tracks.book_spec("credit"), tracks.book_spec("vol")
    assert credit["positions"][:8] == vol["positions"][:8] == common["positions"]
    assert {p["instrument_id"] for p in credit["positions"][8:]} == set(tracks.credit_extension())
    assert {p["instrument_id"] for p in vol["positions"][8:]} == set(tracks.vol_extension())
    assert all(p["instrument_spec"]["type"] != "structured_credit" for p in credit["positions"])
    credit["positions"][0]["instrument_spec"]["spec"]["notional"]["amount"] = "1"
    assert tracks.common.book_spec("common") == before
    assert tracks.book_spec("credit")["positions"][0] == before["positions"][0]
    for stage in tracks.TRACKS:
        book, market = tracks.build_book(stage), tracks.build_market(stage)
        value = value_portfolio(book, market, metrics=[])
        assert len(book.position_ids) == (12 if stage == "credit" else 11)
        assert math.isfinite(value.total_value)
        assert value.total_value > 0
        rows = json.loads(value.to_json())["position_values"].values()
        assert sum(float(row["value_base"]["amount"]) for row in rows) == pytest.approx(value.total_value, abs=0.01)


def test_real_clo_one_payment_cash_conservation_and_separate_bbb(tracks: ModuleType) -> None:
    deal = tracks.clo_deal(one_period=True)
    assets = deal["instrument"]["spec"]["pool"]["assets"]
    assert len(assets) == 5
    assert all(float(asset["balance"]["amount"]) > 0 for asset in assets)
    principal = sum(float(asset["balance"]["amount"]) for asset in assets)
    accrual = (date(2025, 4, 15) - tracks.AS_OF).days / 360.0
    interest = principal * 0.08 * accrual
    receipts = {"AAA": 80e6 * (1 + 0.04 * accrual), "BBB": 15e6 * (1 + 0.06 * accrual)}
    receipts["EQUITY"] = principal + interest - sum(receipts.values())
    market = tracks.build_market("credit")
    df = market.get_discount("USD-OIS").df((date(2025, 4, 15) - tracks.AS_OF).days / 365.0)
    native = {
        iid: structured_credit_tranche_metrics(json.dumps(deal), iid, market, tracks.AS_OF).pv for iid in receipts
    }
    assert sum(native.values()) / df == pytest.approx(principal + interest, abs=0.01)
    assert all(native[iid] == pytest.approx(receipts[iid] * df, abs=0.01) for iid in receipts)
    holding = tracks.clo_bbb_holding()
    assert holding["tranche_id"] == "BBB"
    assert holding["quantity"] == 1


def test_oc_breach_changes_priority_and_does_not_invent_a_bbb_instrument(tracks: ModuleType) -> None:
    market = tracks.build_market("credit")
    base, breach = tracks.clo_deal(), tracks.clo_deal(oc_trigger=1.30)
    assert 100e6 / 80e6 < 1.30
    metrics = [
        {
            iid: structured_credit_tranche_metrics(json.dumps(payload), iid, market, tracks.AS_OF)
            for iid in ("AAA", "BBB", "EQUITY")
        }
        for payload in (base, breach)
    ]
    assert metrics[1]["AAA"].wal < metrics[0]["AAA"].wal
    assert metrics[1]["BBB"].pv < metrics[0]["BBB"].pv
    assert all(result.pv > 0 for group in metrics for result in group.values())


def test_index_payer_front_end_protection_and_convertible_limit(tracks: ModuleType) -> None:
    market = tracks.build_market("credit")
    payer = tracks.credit_index_inputs()["CDX-PAYER"]
    knockout = deepcopy(payer)
    knockout["instrument"]["spec"]["knockout"] = True
    nonko = price_instrument(json.dumps(payer), market, tracks.AS_OF, model="bloomberg_cdso").value.amount
    ko = price_instrument(json.dumps(knockout), market, tracks.AS_OF, model="bloomberg_cdso").value.amount
    assert nonko > ko > 0
    payload = tracks.convertible_limit()
    tree = price_instrument(json.dumps(payload), market, tracks.AS_OF, model="tree").value.amount
    analytical = 1000 * math.exp(-0.04) + 10 * bs_price(100, 100, 0.04, 0.0, 0.25, 1.0, True)
    assert tree >= 1000 * math.exp(-0.04)
    assert tree == pytest.approx(analytical, abs=0.20)


def test_wrong_way_risk_keeps_marginal_utilization_fixed(tracks: ModuleType) -> None:
    # High utilization and default retain the same marginals; only dependence changes.
    independent, dependent = (CorrelatedBernoulli(0.25, 0.10, rho) for rho in (0.0, 0.3))
    marginal = 0.4 + 0.6 * 0.25
    conditional = [0.4 + 0.6 * distribution.joint_p11 / 0.10 for distribution in (independent, dependent)]
    assert conditional[0] == pytest.approx(marginal)
    assert conditional[1] > conditional[0]
    payload = tracks.stochastic_revolver(num_paths=512)
    market = tracks.build_market("credit")
    first = price_instrument(json.dumps(payload), market, tracks.AS_OF, model="monte_carlo_gbm").value.amount
    second = price_instrument(json.dumps(payload), market, tracks.AS_OF, model="monte_carlo_gbm").value.amount
    assert first == second
    assert first > 0


def test_future_delivery_choice_switches_and_american_value_is_not_lower(tracks: ModuleType) -> None:
    inputs = tracks.futures_inputs()
    cheapest = []
    for rate in (0.04, 0.055):
        market = tracks.build_market("vol")
        market.insert(
            DiscountCurve(
                "USD-TREASURY",
                tracks.AS_OF,
                [(t, math.exp(-rate * t)) for t in (0.0, 1.0, 5.0, 10.0, 20.0)],
                day_count="act_365f",
            )
        )
        prices = {}
        for iid in ("UST-DELIVERABLE-A", "UST-DELIVERABLE-B"):
            future = deepcopy(inputs["UST-FUTURE"])
            future["instrument"]["spec"].update({"ctd_bond_id": iid, "ctd_bond": inputs[iid]["instrument"]["spec"]})
            result = price_instrument(
                json.dumps(future),
                market,
                tracks.AS_OF,
                model="bond_future_clean_price_proxy",
                metrics=["futures_price"],
            )
            prices[iid] = result.metrics["futures_price"]
        cheapest.append(min(prices, key=prices.get))
    assert cheapest == ["UST-DELIVERABLE-A", "UST-DELIVERABLE-B"]
    option = inputs["SPX-FUTURE-CALL"]
    market = tracks.build_market("vol")
    european = price_instrument(json.dumps(option), market, tracks.AS_OF).value.amount
    option["instrument"]["spec"]["terms"]["exercise_style"] = "american"
    american = price_instrument(json.dumps(option), market, tracks.AS_OF).value.amount
    assert american >= european > 0


def test_variance_units_ohlc_and_kirk_independent_mc(tracks: ModuleType) -> None:
    market = tracks.build_market("vol")
    variance = price_instrument(
        json.dumps(tracks.variance_inputs()["SPX-VARIANCE"]),
        market,
        tracks.AS_OF,
        metrics=["variance_expected", "variance_realized"],
    )
    df = market.get_discount("USD-OIS").df(1.0)
    assert variance.value.amount == pytest.approx(1e6 * df * (variance.metrics["variance_expected"] - 0.04), abs=0.01)
    assert variance.metrics["variance_expected"] == pytest.approx(0.04, abs=0.0005)
    observations = tracks.ohlc_observations()
    assert all(
        [day for day, _ in rows] == [day for day, _ in observations["SPX-CLOSE"]] for rows in observations.values()
    )
    spread = tracks.commodity_inputs()["WTI-BRENT-SPREAD"]
    kirk = price_instrument(json.dumps(spread), market, tracks.AS_OF, model="black76").value.amount
    time = (date(2025, 9, 15) - tracks.AS_OF).days / 365.0
    forwards = np.array([market.get_price_curve(f"{name}-FORWARD").price(time) for name in ("WTI", "BRENT")])
    sigma = np.array([0.30, 0.28])
    factors = LatentMultiFactor(2, sigma.tolist(), [1.0, 0.85, 0.85, 1.0])
    z = np.random.default_rng(20250115).standard_normal((30_000, 2))
    shocks = np.array([factors.generate_correlated_factors(row.tolist()) for row in z])
    up = forwards * np.exp(-0.5 * sigma**2 * time + math.sqrt(time) * shocks)
    down = forwards * np.exp(-0.5 * sigma**2 * time - math.sqrt(time) * shocks)
    samples = 0.5 * (np.maximum(up[:, 0] - up[:, 1] + 3, 0) + np.maximum(down[:, 0] - down[:, 1] + 3, 0))
    samples *= 10_000 * market.get_discount("USD-OIS").df(time)
    standard_error = samples.std(ddof=1) / math.sqrt(len(samples))
    assert abs(kirk - samples.mean()) < 4 * standard_error + 0.01 * abs(kirk)
