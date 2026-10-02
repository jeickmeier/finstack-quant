"""Cross-host goldens for the valuations surface bound to WASM by slice P7.

Every case is computed here by the Python binding and compared with
``finstack-quant-wasm/tests/facade/valuations_parity.golden.json``. The facade
test ``finstack-quant-wasm/tests/facade/valuations_parity.test.mjs`` computes
the same cases through the WASM binding from the inputs stored in that file
and asserts the same values, so both hosts are pinned to one Rust result.

Regenerate the golden after an intended change with::

    UPDATE_VALUATIONS_PARITY_GOLDEN=1 uv run --no-sync pytest \
        finstack-quant-py/tests/test_valuations_wasm_parity.py
    (cd finstack-quant-wasm && npx --no-install prettier --write \
        tests/facade/valuations_parity.golden.json)
"""

from __future__ import annotations

import datetime
import json
import math
import os
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import HazardCurve, VolSurface
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate
from finstack_quant.models.credit import MertonModel
from finstack_quant.valuations import ValuationResult, composite, instruments as ins, market as conventions, schema
from tests import (
    test_typed_rates_instruments as rates_cases,
    test_typed_structured_credit_surface as sc_cases,
    test_valuations_credit_equity_fx as cef_cases,
)
from tests.tests_typed_helpers import build_irs, structured_credit_pool

GOLDEN = Path(__file__).parents[2] / "finstack-quant-wasm/tests/facade/valuations_parity.golden.json"
USD = Currency("USD")
AS_OF = "2024-06-20"

EXAMPLE_CLASSES = [
    "Bond",
    "TermLoan",
    "RevolvingCredit",
    "AssetBackedFacility",
    "FxForward",
    "FxOption",
    "InterestRateSwap",
    "Swaption",
    "CapFloor",
    "CreditDefaultSwap",
    "CdsIndex",
    "CdsTranche",
    "ConvertibleBond",
    "EquityOption",
]


def _json(value: Any) -> Any:
    """A ``to_json()`` payload as parsed JSON."""
    return json.loads(value.to_json())


def _pinned_timestamp(result_json: str) -> str:
    """A valuation result with its wall-clock stamp replaced by a fixed one."""
    document = json.loads(result_json)
    document["meta"]["timestamp"] = "2024-01-15T00:00:00.000000000Z"
    return ValuationResult.from_json(json.dumps(document)).to_json()


def _money(value: Money) -> dict[str, Any]:
    return {"amount": value.amount, "currency": value.currency.code}


def _schema_summary(text: str) -> dict[str, Any]:
    """Identity of a schema document: enough to tell both hosts read the same one."""
    document = json.loads(text)
    return {
        "id": document.get("$id"),
        "title": document.get("title"),
        "keys": sorted(document),
        "definitions": sorted(document.get("$defs", {})),
    }


def _merton_bond() -> ins.Bond:
    bond = ins.Bond.fixed(
        "PIK-1",
        Money(100.0, USD),
        Rate(0.08),
        datetime.date(2024, 1, 15),
        datetime.date(2029, 1, 15),
        StubKind.SHORT_FRONT,
        "USD-OIS",
    )
    envelope = json.loads(bond.to_json())
    envelope["instrument"]["spec"].setdefault("instrument_pricing_overrides", {}).setdefault("model_config", {})[
        "mc_paths"
    ] = 64
    return ins.Bond.from_json(json.dumps(envelope))


def build_cases() -> dict[str, Any]:
    cases: dict[str, Any] = {}

    # -- typed instrument examples ---------------------------------------
    examples = {name: getattr(ins, name).example() for name in EXAMPLE_CLASSES}
    cases["examples"] = {name: instrument.to_json() for name, instrument in examples.items()}
    cases["to_dict"] = {name: instrument.to_dict() for name, instrument in examples.items()}
    cases["market_dependencies"] = {name: instrument.market_dependencies() for name, instrument in examples.items()}
    cases["default_model"] = {name: instrument.default_model for name, instrument in examples.items()}
    cases["expiry"] = {
        name: (instrument.expiry.isoformat() if instrument.expiry else None)
        for name, instrument in examples.items()
        if hasattr(instrument, "expiry")
    }

    # -- rates -------------------------------------------------------------
    swap = build_irs()
    rates_market = rates_cases._market_json()
    swap_result = swap.price(rates_market, "2024-01-15", metrics=["dv01"])
    conventions_swap = ins.InterestRateSwap.from_conventions(
        "IRS-CONV",
        Money(10_000_000.0, USD),
        "pay",
        0.035,
        "2025-01-15",
        "2030-01-15",
        "USD-SOFR",
        "USD-OIS",
        "USD-SOFR",
    )
    swaption = ins.Swaption.example()
    cases["rates"] = {
        "market": rates_market,
        "swap": swap.to_json(),
        "price": swap_result.price,
        "dv01": swap.metric(rates_market, "2024-01-15", "dv01"),
        "result_json": _pinned_timestamp(swap_result.to_json()),
        "price_decimal": swap_result.price_decimal(),
        "get_metric": swap_result.get_metric("dv01"),
        "metric_keys": swap_result.metric_keys(),
        "metric_count": swap_result.metric_count(),
        "metric_units": swap_result.metric_units(),
        "all_covenants_passed": swap_result.all_covenants_passed(),
        "failed_covenants": swap_result.failed_covenants(),
        "from_conventions": conventions_swap.to_json(),
        "swaption_forward_swap_rate": swaption.forward_swap_rate(rates_market, "2024-01-15"),
        "swaption_strike": swaption.get_strike(),
        "swaption_underlying_start": swaption.get_underlying_start_date().isoformat(),
        "swaption_underlying_maturity": swaption.get_underlying_maturity().isoformat(),
    }

    # -- credit ------------------------------------------------------------
    credit_market = cef_cases._credit_market()
    cds = ins.CreditDefaultSwap.example()
    index = ins.CdsIndex.from_preset(
        ins.CdsIndexParams.cdx_na_ig(42, 1, Bps(100.0)),
        "CDX-IG-42",
        Money(10_000_000.0, USD),
        "pay",
        "2024-03-20",
        "2029-06-20",
        0.4,
        "USD-OIS",
        "CDX.NA.IG.HAZARD",
    )
    tranche_params = ins.CdsTrancheParams.mezzanine_tranche(
        "CDX.NA.IG", 42, Money(10_000_000.0, USD), "2029-12-20", Bps(100.0)
    )
    tranche = ins.CdsTranche.standard("CDX-42-3X7", tranche_params, "USD-OIS", "CDX.NA.IG.HAZARD", "pay")
    cases["credit"] = {
        "market": credit_market.to_json(),
        "cds_price": cds.price(credit_market, AS_OF, "hazard_rate").price,
        "cds_par_spread": cds.par_spread(credit_market, AS_OF),
        "index": index.to_json(),
        "index_par_spread": index.par_spread(credit_market, AS_OF),
        "index_risky_pv01": index.risky_pv01(credit_market, AS_OF),
        "index_cs01": index.cs01(credit_market, AS_OF),
        "tranche": tranche.to_json(),
    }

    # -- equity ------------------------------------------------------------
    equity_market = cef_cases._equity_market()
    option = ins.EquityOption.example()
    option_price = option.price(equity_market, "2024-01-15").price
    european = ins.EquityOption.european("SPX-CALL", "SPX", 4500.0, "2024-06-21", 100.0, USD, "call")
    convertible = ins.ConvertibleBond.example()
    cases["equity"] = {
        "market": equity_market.to_json(),
        "price": option_price,
        "greeks": option.greeks(equity_market, "2024-01-15"),
        "delta": option.delta(equity_market, "2024-01-15"),
        "implied_vol": option.implied_vol(equity_market, "2024-01-15", option_price),
        "european": european.to_json(),
    }
    convertible_market = cef_cases._equity_market()
    convertible_market.insert_price("TECH", 45.0, currency="USD")
    convertible_market.insert(cef_cases._discount("USD-IG", 0.04))
    convertible_market.insert(VolSurface("TECH-VOL", [0.25, 1.0, 5.0], [20.0, 40.0, 60.0], [[0.3, 0.3, 0.3]] * 3))
    convertible_market.insert(HazardCurve.flat("USD-CREDIT-BBB", cef_cases.BASE, 0.02, 0.4))
    cases["convertible"] = {
        "market": convertible_market.to_json(),
        "parity": convertible.parity(convertible_market),
        "conversion_premium": convertible.conversion_premium(convertible_market, 1_100_000.0),
        "conversion_ratio": convertible.conversion_ratio,
        "greeks": convertible.greeks(convertible_market, AS_OF),
    }

    # -- structured credit -------------------------------------------------
    deal = sc_cases._clo()
    sc_market = sc_cases._market()
    close = sc_cases.CLOSE.isoformat()
    stochastic = deal.enable_stochastic()
    # The stochastic pricer reads the capital structure junior-first.
    junior_first = json.loads(stochastic.to_json())
    junior_first["instrument"]["spec"]["tranches"]["tranches"].reverse()
    stochastic_deal = ins.StructuredCredit.from_json(json.dumps(junior_first))
    stochastic_result = stochastic_deal.price_stochastic(sc_market, close, num_paths=8)
    equity = deal.equity_metrics(sc_market, close)
    cases["structured"] = {
        "market": sc_market.to_json(),
        "as_of": close,
        "deal": deal.to_json(),
        "to_dict": deal.to_dict(),
        "tranche_cashflows": _json(deal.tranche_cashflows("A", sc_market, close)),
        "equity_metrics": _json(equity),
        "diagnostics": _json(deal.run_simulation_with_diagnostics(sc_market, close)),
        "create_waterfall": _json(deal.create_waterfall()),
        "with_standard_fees": deal.with_standard_fees().to_json(),
        "enable_stochastic": stochastic.to_json(),
        "stochastic_deal": stochastic_deal.to_json(),
        "stochastic_npv": stochastic_result.npv.amount,
        "stochastic_paths": stochastic_result.num_paths,
        "coverage_tests": deal.create_waterfall().coverage_tests(),
    }

    # -- Merton Monte Carlo ------------------------------------------------
    merton = MertonModel(100.0, 0.25, 60.0, 0.04)
    config = (
        ins
        .MertonMcConfig(merton, 0.40)
        .pik_schedule(ins.PikSchedule.uniform(ins.PikMode.pik()))
        .barrier_crossing(ins.BarrierCrossing.discrete())
        .steps_per_year(12)
    )
    bond = _merton_bond()
    merton_result = bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15))
    cases["merton"] = {
        "model": json.loads(merton.to_json()),
        "bond": bond.to_json(),
        "config": _json(config),
        "clean_price_pct": merton_result.clean_price_pct,
        "expected_loss": merton_result.expected_loss,
        "num_paths": merton_result.num_paths,
        "default_rate": merton_result.path_statistics.default_rate,
    }

    # -- data-type constructors ---------------------------------------------
    stepped = ins.PikSchedule.stepped([(0.0, ins.PikMode.pik()), (2.0, ins.PikMode.cash())])
    senior = (
        ins.Tranche
        .builder()
        .id("A")
        .seniority("senior")
        .original_balance(Money(72_000_000.0, USD))
        .coupon_fixed(0.05)
        .maturity(datetime.date(2031, 1, 15))
        .build()
    )
    junior = (
        ins.Tranche
        .builder()
        .id("E")
        .seniority("equity")
        .original_balance(Money(8_000_000.0, USD))
        .coupon_fixed(0.0)
        .maturity(datetime.date(2031, 1, 15))
        .build()
    )
    bond_asset = ins.PoolAsset.fixed_rate_bond("B1", Money(1_000_000.0, USD), 0.07, "2030-01-15", DayCount.THIRTY_360)
    loan_asset = ins.PoolAsset.floating_rate_loan(
        "L1", Money(2_000_000.0, USD), "USD-SOFR-3M", 350.0, "2030-01-15", DayCount.ACT_360
    )
    pool = ins.AssetPool("POOL", "clo", USD)
    reinvestment_period = {
        "end": "2028-01-15",
        "is_active": True,
        "criteria": {"max_price_pct": 100.0, "min_yield": 0.0},
        "amortizing_tranches": ["A"],
        "assumptions": None,
    }
    scenario_table = {
        "tranche_id": "A",
        "cells": [{"cpr": 0.1, "cdr": 0.02, "severity": 0.4, "price": 99.5, "wal": 4.2, "writedown": 0.0}],
    }
    index_params = ins.CdsIndexParams.cdx_na_hy(42, 1, Bps(500.0))
    cases["data"] = {
        "barrier_crossing": [_json(ins.BarrierCrossing.discrete()), _json(ins.BarrierCrossing.brownian_bridge())],
        "pik_mode": [
            _json(ins.PikMode.cash()),
            _json(ins.PikMode.pik()),
            _json(ins.PikMode.split(0.6, 0.4)),
            _json(ins.PikMode.toggle()),
        ],
        "pik_schedule_uniform": _json(ins.PikSchedule.uniform(ins.PikMode.pik())),
        "pik_schedule_stepped": _json(stepped),
        "pik_schedule_mode_at": [_json(stepped.mode_at(1.0)), _json(stepped.mode_at(3.0))],
        "tranches": [_json(senior), _json(junior)],
        "tranche_structure": _json(ins.TrancheStructure.from_balances([senior, junior])),
        "pool_assets": [_json(bond_asset), _json(loan_asset)],
        "pool": _json(pool),
        "pool_with_assets": _json(pool.with_assets([bond_asset, loan_asset])),
        "pool_with_rep_lines": _json(pool.with_rep_lines(structured_credit_pool().rep_lines)),
        "pool_with_instruments": _json(pool.with_instruments(bonds=[ins.Bond.example()])),
        "reinvestment_period": reinvestment_period,
        "pool_with_reinvestment_period": _json(pool.with_reinvestment_period(reinvestment_period)),
        "scenario_table": scenario_table,
        "scenario_table_cells": ins.ScenarioTable.from_json(json.dumps(scenario_table)).cells(),
        "pool_with_reserve": _json(
            pool.with_reserve(Money(500_000.0, USD), 0.02, reserve_target=Money(750_000.0, USD))
        ),
        "pool_with_accounts": _json(
            pool.with_accounts(collection_account=Money(125_000.0, USD), original_balance=Money(3_000_000.0, USD))
        ),
        "prepayment_penalty": [
            _json(ins.PrepaymentPenalty.lockout("2026-01-15")),
            _json(ins.PrepaymentPenalty.fixed(3.0)),
            _json(ins.PrepaymentPenalty.step_down([("2026-01-15", 3.0), ("2027-01-15", 1.0)])),
            _json(ins.PrepaymentPenalty.yield_maintenance(reinvestment_rate=0.03, floor_pct=1.0)),
        ],
        "coverage_rules": _json(ins.CoverageRules.clo_standard()),
        "amortization_event": [
            _json(ins.AmortizationEvent.date("2026-01-15")),
            _json(ins.AmortizationEvent.cumulative_loss(0.05)),
            _json(ins.AmortizationEvent.excess_spread(0.01)),
        ],
        "equity_tranche_attachment": [
            ins.CdsTrancheParams.equity_tranche(
                "CDX.NA.IG", 42, Money(10_000_000.0, USD), "2029-12-20", Bps(500.0)
            ).attach_pct,
            ins.CdsTrancheParams.equity_tranche(
                "CDX.NA.IG", 42, Money(10_000_000.0, USD), "2029-12-20", Bps(500.0)
            ).detach_pct,
        ],
        "itraxx_index_name": ins.CdsIndexParams.itraxx_europe(40, 1, Bps(100.0)).index_name,
        "cds_index_params": {
            "index_name": index_params.index_name,
            "series": index_params.series,
            "version": index_params.version,
            "coupon_bp": index_params.coupon_bp,
            "convention": index_params.convention,
            "num_constituents": index_params.num_constituents,
        },
        "cds_tranche_params": {
            "index_name": tranche_params.index_name,
            "series": tranche_params.series,
            "attach_pct": tranche_params.attach_pct,
            "detach_pct": tranche_params.detach_pct,
            "coupon_bp": tranche_params.coupon_bp,
            "realized_loss": tranche_params.realized_loss,
            "maturity": tranche_params.maturity.isoformat(),
            "notional": _money(tranche_params.notional),
        },
    }

    # -- composite -----------------------------------------------------------
    leg = composite.CompositeLegSpec("LEG-A", ins.Bond.example(), 1.0)
    cases["composite"] = {
        "leg": _json(leg),
        "leg_instrument_dict": leg.instrument_dict(),
        "weighting": [
            _json(composite.WeightingMethod.fixed_quantity()),
            _json(composite.WeightingMethod.notional_weighted(Money(1_000_000.0, USD))),
            _json(composite.WeightingMethod.metric_weighted("dv01", "LEG-A", 1.0, neutralize=True)),
            _json(composite.WeightingMethod.dv01_neutral("LEG-A", 1.0)),
            _json(composite.WeightingMethod.delta_neutral("LEG-A", 2.0)),
            _json(composite.WeightingMethod.duration_weighted("LEG-A", 3.0)),
            _json(composite.WeightingMethod.volatility_weighted("LEG-A", 1.0, 60, 20, 252.0)),
        ],
        "rebalance": [
            _json(composite.RebalanceRule.manual()),
            _json(composite.RebalanceRule.dates([datetime.date(2024, 3, 29), datetime.date(2024, 6, 28)])),
            _json(
                composite.RebalanceRule.calendar(
                    datetime.date(2024, 1, 31), Tenor.monthly(), "nyse", "modified_following", "2025-01-31"
                )
            ),
        ],
    }

    # -- conventions and schemas ----------------------------------------------
    registry = conventions.ConventionRegistry()
    cases["conventions"] = {
        "rate_index": registry.require_rate_index("USD-SOFR-OIS").to_dict(),
        "cds": registry.resolve_cds("USD", "isda_na").to_dict(),
        "primary_cds_family": registry.primary_cds_family("EUR"),
        "swaption": registry.require_swaption("USD").to_dict(),
        "inflation_swap": registry.require_inflation_swap("USD-CPI").to_dict(),
        "ir_future": registry.require_ir_future("CME:SR3").to_dict(),
        "xccy": registry.require_xccy("EUR/USD-XCCY").to_dict(),
    }
    cases["schema"] = {
        "instrument_types": schema.instrument_types(),
        "instrument_envelope": _schema_summary(schema.instrument_envelope_schema()),
        "bond": _schema_summary(schema.instrument_schema("bond")),
        "valuation_result": _schema_summary(schema.valuation_result_schema()),
    }
    return json.loads(json.dumps(cases))


def _assert_close(actual: Any, expected: Any, path: str) -> None:
    if isinstance(expected, dict):
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key, value in expected.items():
            _assert_close(actual[key], value, f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, list), path
        assert len(actual) == len(expected), path
        for index, value in enumerate(expected):
            _assert_close(actual[index], value, f"{path}[{index}]")
    elif isinstance(expected, float) and isinstance(actual, (int, float)) and not isinstance(actual, bool):
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), f"{path}: {actual} != {expected}"
    else:
        assert actual == expected, path


def test_python_matches_the_cross_host_golden() -> None:
    cases = build_cases()
    if os.environ.get("UPDATE_VALUATIONS_PARITY_GOLDEN") == "1":
        GOLDEN.write_text(json.dumps(cases, indent=2) + "\n")
    _assert_close(cases, json.loads(GOLDEN.read_text()), "golden")


def test_valuation_result_methods_match_the_typed_wrapper() -> None:
    rates = json.loads(GOLDEN.read_text())["rates"]
    result = ValuationResult.from_json(rates["result_json"])
    assert result.price_decimal() == rates["price_decimal"]
    assert result.metric_keys() == rates["metric_keys"]
    assert result.get_metric("not_a_metric") is None


def test_unknown_tranche_is_a_lookup_miss() -> None:
    deal = sc_cases._clo()
    with pytest.raises(KeyError, match="tranche"):
        deal.tranche_cashflows("Z", sc_cases._market(), sc_cases.CLOSE)


def test_tranche_builder_requires_both_points() -> None:
    builder = ins.Tranche.builder().id("A").seniority("senior").attach_pct(10.0)
    builder = builder.original_balance(Money(1.0, USD)).coupon_fixed(0.05).maturity(datetime.date(2031, 1, 15))
    with pytest.raises(ValueError, match="attach_pct and detach_pct must be set together"):
        builder.build()


def test_tranche_loss_methods_report_missing_index_data_as_a_lookup_miss() -> None:
    tranche = ins.CdsTranche.example()
    market = cef_cases._equity_market()
    with pytest.raises(KeyError):
        tranche.expected_loss(market)
    with pytest.raises(KeyError):
        tranche.jump_to_default(market, AS_OF)
