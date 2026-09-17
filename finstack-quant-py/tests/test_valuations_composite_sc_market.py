"""Composite typed twins, structured-credit value objects, and market conventions.

Covers the binding-parity fixes for the composite instrument surface
(typed ``execution_trades`` + JSON twin, date-like rebalance rules, dict
observations), the structured-credit value objects (``to_json`` /
``from_json`` / pickle / getters, dict inputs), ``KeyError`` for unknown
tranches, and the read-only ``ConventionRegistry``.
"""

from __future__ import annotations

import datetime as dt
import json
import pickle

import pandas as pd
import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext, ScalarTimeSeries
from finstack_quant.core.money import Money
from finstack_quant.valuations.composite import (
    CompositeHistoryEngine,
    CompositeInstrument,
    CompositeLegSpec,
    CompositeSpec,
    RebalanceRule,
    WeightingMethod,
)
from finstack_quant.valuations.instruments import (
    AssetPool,
    RepLine,
    StructuredCredit,
    Tranche,
    TrancheStructure,
    structured_credit_tranche_metrics,
    structured_credit_tranche_scenario_table,
)
from finstack_quant.valuations.market import (
    CdsConventionSpec,
    ConventionRegistry,
    RateIndexConventions,
)
from tests.tests_typed_helpers import canonical_structured_credit_json


def _equity_envelope(instrument_id: str, price: float) -> str:
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "equity",
            "spec": {
                "id": instrument_id,
                "ticker": instrument_id,
                "currency": "USD",
                "shares": 1.0,
                "price_quote": price,
                "price_id": None,
                "div_yield_id": None,
                "discrete_dividends": [],
                "discount_curve_id": "USD",
                "attributes": {},
            },
        },
    })


def _fixed_spec() -> CompositeSpec:
    return CompositeSpec(
        "A-B",
        Currency("USD"),
        Money(100.0, Currency("USD")),
        [
            CompositeLegSpec("A", _equity_envelope("A", 100.0), 1.0),
            CompositeLegSpec("B", _equity_envelope("B", 90.0), -1.0),
        ],
        WeightingMethod.fixed_quantity(),
        RebalanceRule.manual(),
    )


# --------------------------------------------------------------------------- composite


def test_execution_trades_typed_twin_matches_json_twin() -> None:
    resolved = _fixed_spec().initialize(MarketContext(), dt.date(2025, 1, 1)).instrument
    typed = resolved.execution_trades()
    assert typed == json.loads(resolved.execution_trades_json())
    assert [t["quantity_delta"] for t in typed] == [1.0, -1.0]
    frame = resolved.execution_trades_dataframe()
    assert isinstance(frame, pd.DataFrame)
    assert list(frame.columns) == ["instrument_id", "instrument_type", "quantity_delta"]
    assert list(frame["quantity_delta"]) == [1.0, -1.0]


def test_rebalance_result_exposes_typed_trades() -> None:
    result = _fixed_spec().initialize(MarketContext(), dt.date(2025, 1, 1))
    assert result.trades == json.loads(result.trades_json)
    assert list(result.to_dataframe()["instrument_id"]) == ["A", "B"]


def test_rebalance_rule_accepts_date_objects_and_strings() -> None:
    via_dates = RebalanceRule.dates([dt.date(2025, 1, 31), pd.Timestamp("2025-02-28"), "2025-03-31"])
    via_strings = RebalanceRule.dates(["2025-01-31", "2025-02-28", "2025-03-31"])
    assert via_dates.to_json() == via_strings.to_json()
    calendar = RebalanceRule.calendar(dt.date(2025, 1, 1), "monthly", "weekends_only", "following", "2026-01-01")
    assert (
        calendar.to_json()
        == RebalanceRule.calendar("2025-01-01", "monthly", "weekends_only", "following", dt.date(2026, 1, 1)).to_json()
    )
    assert repr(RebalanceRule.manual()) == "RebalanceRule(kind='manual')"
    with pytest.raises(ValueError, match=r"strictly increasing"):
        RebalanceRule.dates(["2025-02-28", "2025-01-31"])


def test_history_engine_accepts_observation_lists() -> None:
    state = json.loads(MarketContext().to_json())
    observations = [{"date": "2025-01-01", "state": state}, {"date": "2025-01-02", "state": state}]
    via_list = CompositeHistoryEngine.run_from_spec(_fixed_spec(), observations)
    via_json = CompositeHistoryEngine.run_from_spec(_fixed_spec(), json.dumps(observations))
    assert via_list.to_json() == via_json.to_json()
    assert via_list.dates == ["2025-01-01", "2025-01-02"]
    assert list(via_list.to_dataframe()["return_index"]) == [100.0, 100.0]
    resolved = _fixed_spec().initialize(MarketContext(), dt.date(2025, 1, 1)).instrument
    assert len(CompositeHistoryEngine.run(resolved, observations)) == 2


def test_composite_reprs_and_spec_getters() -> None:
    spec = _fixed_spec()
    assert repr(spec).startswith("CompositeSpec(id='A-B', reporting_currency='USD', capital=100")
    assert spec.capital == Money(100.0, Currency("USD"))
    assert [leg.instrument_id for leg in spec.legs] == ["A", "B"]
    assert repr(spec.weighting_method) == "WeightingMethod(kind='fixed_quantity')"
    assert repr(spec.rebalance_rule) == "RebalanceRule(kind='manual')"
    leg = spec.legs[0]
    assert repr(leg) == "CompositeLegSpec(instrument_id='A', instrument_type='equity', weight=1)"
    assert leg.instrument_dict() == json.loads(leg.instrument_json)
    resolved = spec.initialize(MarketContext(), dt.date(2025, 1, 1)).instrument
    assert repr(resolved) == "CompositeInstrument(id='A-B', effective_date='2025-01-01', legs=2)"
    assert repr(resolved.state) == "CompositeState(effective_date='2025-01-01', legs=2)"
    assert isinstance(CompositeInstrument.from_json(resolved.to_json()), CompositeInstrument)


# --------------------------------------------------------------------------- structured credit


def _tranche(id_: str, attach: float, detach: float, seniority: str, balance: float) -> Tranche:
    return (
        Tranche
        .builder()
        .id(id_)
        .attachment_point(attach)
        .detachment_point(detach)
        .seniority(seniority)
        .original_balance(Money(balance, Currency("USD")))
        .coupon_fixed(0.05)
        .maturity("2031-01-15")
        .build()
    )


def test_tranche_value_object_contract() -> None:
    tranche = _tranche("A", 10.0, 100.0, "senior", 90.0)
    assert tranche.id == "A"
    assert tranche.attachment_point == 10.0
    assert tranche.maturity == dt.date(2031, 1, 15)
    assert tranche.original_balance == Money(90.0, Currency("USD"))
    assert isinstance(tranche.coupon, dict)
    assert tranche.pik_enabled is False
    restored = Tranche.from_json(tranche.to_json())
    assert restored.to_json() == tranche.to_json()
    assert pickle.loads(pickle.dumps(tranche)).to_json() == tranche.to_json()  # noqa: S301
    assert tranche.to_dict() == json.loads(tranche.to_json())
    assert repr(tranche).startswith("Tranche(id='A', seniority=")
    assert "pik_enabled=False" in repr(tranche)
    builder = Tranche.builder().attachment_point(0.0)
    assert repr(builder) == "TrancheBuilder(attachment_point=0, detachment_point=None, consumed=False)"


def test_tranche_builder_accepts_rate_objects_and_dict_coupon() -> None:
    from finstack_quant.core.types import Rate

    via_rate = (
        Tranche
        .builder()
        .id("A")
        .attachment_point(0.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(Money(100.0, Currency("USD")))
        .coupon_fixed(Rate(0.05))
        .maturity(dt.date(2031, 1, 15))
        .build()
    )
    via_float = _tranche("A", 0.0, 100.0, "senior", 100.0)
    assert via_rate.to_json() == via_float.to_json()
    floating = (
        Tranche
        .builder()
        .id("F")
        .attachment_point(0.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(Money(100.0, Currency("USD")))
        .coupon_floating(json.loads(json.dumps(via_float.coupon)))
        .maturity(dt.date(2031, 1, 15))
        .build()
    )
    assert floating.coupon == via_float.coupon


def test_tranche_structure_rep_line_and_asset_pool_contracts() -> None:
    structure = TrancheStructure([_tranche("A", 10.0, 100.0, "senior", 90.0), _tranche("E", 0.0, 10.0, "equity", 10.0)])
    assert [t.id for t in structure.tranches] == ["A", "E"]
    assert structure.total_size == Money(100.0, Currency("USD"))
    assert TrancheStructure.from_json(structure.to_json()).to_json() == structure.to_json()
    assert pickle.loads(pickle.dumps(structure)).to_json() == structure.to_json()  # noqa: S301

    line = RepLine(
        "LINE-1",
        Money(80.0, Currency("USD")),
        0.07,
        "2031-01-15",
        12,
        DayCount.ACT_360,
        asset_type={"type": "first_lien_loan", "industry": None},
        spread_bp=150.0,
    )
    assert line.maturity == dt.date(2031, 1, 15)
    assert line.spread_bp == 150.0
    assert line.balance == Money(80.0, Currency("USD"))
    assert RepLine.from_json(line.to_json()).to_json() == line.to_json()
    assert pickle.loads(pickle.dumps(line)).to_json() == line.to_json()  # noqa: S301
    assert repr(line).startswith("RepLine(id='LINE-1', balance=80")

    pool = AssetPool("POOL-1", "abs", "USD").with_rep_lines([line])
    assert pool.base_currency == "USD"
    assert pool.deal_type == "abs"
    assert [rl.id for rl in pool.rep_lines] == ["LINE-1"]
    assert pool.asset_records == []
    assert pool.cumulative_defaults == Money(0.0, Currency("USD"))
    assert AssetPool.from_json(pool.to_json()).to_json() == pool.to_json()
    assert pickle.loads(pickle.dumps(pool)).to_json() == pool.to_json()  # noqa: S301
    assert pool.with_assets([]).asset_records == []
    assert pool.assets == []
    assert pool.with_assets(()).to_json() == pool.with_assets([]).to_json()


def _market() -> MarketContext:
    as_of = dt.date(2024, 1, 1)
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
        .insert(ForwardCurve("SOFR-3M", 0.25, as_of, [(0.0, 0.04), (10.0, 0.04)], day_count="act_360"))
    )
    market.insert_series(ScalarTimeSeries("FIXING:SOFR-3M", [(dt.date(2023, 12, 28), 0.04)]))
    return market


def test_structured_credit_getters_and_dict_inputs() -> None:
    deal = StructuredCredit.from_json(canonical_structured_credit_json())
    assert deal.deal_type in {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
    assert isinstance(deal.pool, AssetPool)
    assert isinstance(deal.tranches, TrancheStructure)
    assert isinstance(deal.closing_date, dt.date)
    assert isinstance(deal.maturity, dt.date)
    assert deal.to_dict()["id"] == deal.id
    rebuilt = (
        StructuredCredit
        .builder()
        .id(deal.id)
        .deal_type(deal.deal_type)
        .pool(deal.pool)
        .tranches(deal.tranches)
        .closing_date(deal.closing_date.isoformat())
        .first_payment_date(deal.first_payment_date)
        .maturity(deal.maturity)
        .frequency(Tenor.quarterly())
        .discount_curve_id(deal.discount_curve_id)
        .market_conditions(deal.to_dict()["market_conditions"])
        .credit_factors(deal.to_dict()["credit_factors"])
        .build()
    )
    assert rebuilt.id == deal.id
    assert "consumed=" in repr(StructuredCredit.builder())


def test_unknown_tranche_raises_key_error_and_grid_accepts_dict() -> None:
    deal = StructuredCredit.from_json(canonical_structured_credit_json())
    with pytest.raises(KeyError):
        structured_credit_tranche_metrics(deal, "NOPE", _market(), "2024-01-01")
    tranche_id = deal.tranches.tranches[0].id
    grid = {"cprs": [0.10], "cdrs": [0.02], "severities": [0.40]}
    via_dict = structured_credit_tranche_scenario_table(deal, tranche_id, _market(), "2024-01-01", grid)
    via_json = structured_credit_tranche_scenario_table(
        instrument=deal, tranche_id=tranche_id, market=_market(), as_of="2024-01-01", grid=json.dumps(grid)
    )
    assert via_dict.to_json() == via_json.to_json()
    assert via_dict._repr_html_() == via_dict.to_dataframe()._repr_html_()


# --------------------------------------------------------------------------- market conventions


def test_convention_registry_lookups_return_typed_records() -> None:
    registry = ConventionRegistry()
    sofr = registry.require_rate_index("USD-SOFR")
    assert isinstance(sofr, RateIndexConventions)
    assert sofr.currency == "USD"
    assert sofr.default_reset_lag_days >= 0
    assert sofr.day_count == "act_360"
    assert RateIndexConventions.from_json(sofr.to_json()) == sofr
    assert pickle.loads(pickle.dumps(sofr)) == sofr  # noqa: S301
    assert sofr.to_dict() == json.loads(sofr.to_json())
    assert repr(sofr).startswith("RateIndexConventions(currency='USD'")

    cds = registry.resolve_cds("USD", "cr14")
    assert isinstance(cds, CdsConventionSpec)
    assert cds.family == "isda_na"
    assert cds.frequency == "3M"
    assert registry.primary_cds_family("USD") == "isda_na"
    assert registry.primary_cds_family("EUR") == "isda_eu"

    assert registry.require_swaption("USD").float_leg_index
    assert registry.require_inflation_swap("USD-CPI").inflation_lag
    assert registry.require_ir_future("CME:SR3").face_value > 0
    xccy = registry.require_xccy("EUR/USD-XCCY")
    assert {xccy.base_currency, xccy.quote_currency} == {"EUR", "USD"}


def test_convention_registry_errors() -> None:
    registry = ConventionRegistry()
    with pytest.raises(KeyError):
        registry.require_rate_index("NOPE")
    with pytest.raises(KeyError):
        registry.require_xccy("NOPE")
    with pytest.raises(ValueError, match=r"unknown CDS doc clause"):
        registry.resolve_cds("USD", "snac")
    with pytest.raises(ValueError, match=r"Matching variant not found"):
        registry.primary_cds_family("XXX")
