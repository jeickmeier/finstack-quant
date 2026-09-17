"""Typed-only structured-credit surface.

Builders, collateral rows, deal terms, projections and analytics without any
JSON hand-editing.

The CLO regression golden is rebuilt from typed calls alone and must reproduce
both the canonical wire form and the recorded NPV.
"""

from __future__ import annotations

import datetime
import json
import math
from pathlib import Path
import pickle

import pytest

from finstack_quant.cashflows.builder import DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetPool,
    CallAssumption,
    CoverageRules,
    EquityMetrics,
    HedgeSwap,
    InterestRateSwap,
    PoolAsset,
    StructuredCredit,
    Tranche,
    TrancheCashflows,
    TrancheStructure,
    Waterfall,
    price_instrument,
    structured_credit_tranche_metrics,
)
from tests.golden.runners.pricing_common import _resolve_market
from tests.golden.schema import GoldenFixture

USD = Currency("USD")
CLOSE = datetime.date(2024, 1, 1)
MATURITY = datetime.date(2032, 1, 1)
GOLDEN = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant"
    / "valuations"
    / "tests"
    / "golden"
    / "data"
    / "pricing"
    / "regression_goldens"
    / "structured_credit"
    / "clo_mezzanine_base_case.json"
)


def usd(amount: float) -> Money:
    return Money(amount, USD)


def _tranche(id_: str, attach: float, detach: float, seniority: str, balance: float, coupon: float) -> Tranche:
    return (
        Tranche
        .builder()
        .id(id_)
        .attachment_point(attach)
        .detachment_point(detach)
        .seniority(seniority)
        .original_balance(usd(balance))
        .coupon_fixed(coupon)
        .maturity(MATURITY)
        .build()
    )


def _loans() -> list[PoolAsset]:
    return [PoolAsset.fixed_rate_bond(f"L{i}", usd(10_000_000.0), 0.08, MATURITY, DayCount.ACT_360) for i in range(10)]


def _market() -> MarketContext:
    return MarketContext().insert(DiscountCurve.flat("USD-OIS", CLOSE, 0.04))


def _clo() -> StructuredCredit:
    """A 60/30/10 CLO with an OC test, standard coverage rules and a 2028 call."""
    pool = AssetPool("P", "clo", USD).with_assets(_loans())
    tranches = TrancheStructure([
        _tranche("A", 40.0, 100.0, "senior", 60_000_000.0, 0.05),
        _tranche("B", 10.0, 40.0, "mezzanine", 30_000_000.0, 0.07),
        _tranche("E", 0.0, 10.0, "equity", 10_000_000.0, 0.0),
    ])
    return (
        StructuredCredit
        .builder()
        .id("CLO-TYPED")
        .deal_type("clo")
        .pool(pool)
        .tranches(tranches)
        .closing_date(CLOSE)
        .first_payment_date(datetime.date(2024, 4, 1))
        .maturity(MATURITY)
        .frequency(Tenor.quarterly())
        .payment_calendar_id("nyse")
        .discount_curve_id("USD-OIS")
        .prepayment_spec(PrepaymentModelSpec.constant_cpr(0.10))
        .default_spec(DefaultModelSpec.constant_cdr(0.02))
        .recovery_spec(RecoveryModelSpec(0.4, 12))
        .coverage_triggers([{"id": "OC_A", "tranche_id": "A", "kind": "oc", "trigger_level": 1.2}])
        .coverage_rules(CoverageRules.clo_standard())
        .call_assumption(CallAssumption(datetime.date(2028, 1, 1), 100.0))
        .cleanup_call_pct(0.1)
        .liquidation_price_pct(99.0)
        .loss_allocation("par_preserving")
        .principal_covers_senior_interest(True)
        .attributes({"desk": "abs"})
        .build()
    )


def test_pool_asset_rows_round_trip_through_the_typed_pool() -> None:
    loan = PoolAsset(
        "L0",
        {"type": "first_lien_loan", "industry": "Software"},
        usd(10_000_000.0),
        0.08,
        MATURITY,
        day_count=DayCount.THIRTY_360,
        credit_quality="B",
        industry="Software",
        obligor_id="OBL-1",
        purchase_price=usd(9_800_000.0),
        market_price_pct=98.0,
        balloon={"default_prob": 0.3, "extension_months": 24, "extension_rate": 0.07},
        prepayment_penalty={"kind": "fixed", "pct": 3.0, "through": "2026-01-01"},
        special_servicing={"appraisal_reduction_pct": 40.0},
        noi=usd(1_200_000.0),
        liquidation={
            "months_to_resolution": 24,
            "proceeds_pct": 55.0,
            "carry_cost_pct": 5.0,
            "reperformance_prob": 0.2,
        },
    )
    assert loan.credit_quality == "B"
    assert loan.liquidation == {
        "months_to_resolution": 24,
        "proceeds_pct": 55.0,
        "carry_cost_pct": 5.0,
        "reperformance_prob": 0.2,
    }
    assert loan.day_count == DayCount.THIRTY_360
    assert loan.balloon == {"default_prob": 0.3, "extension_months": 24, "extension_rate": 0.07}
    assert loan.prepayment_penalty["through"] == "2026-01-01"
    assert loan.special_servicing == {"appraisal_reduction_pct": 40.0}
    assert loan.noi == usd(1_200_000.0)
    assert PoolAsset.from_json(loan.to_json()).to_dict() == loan.to_dict()
    assert pickle.loads(pickle.dumps(loan)).to_json() == loan.to_json()  # noqa: S301

    pool = AssetPool("P", "cmbs", USD).with_assets([
        loan,
        PoolAsset.fixed_rate_bond("B1", usd(1.0), 0.05, MATURITY, DayCount.ACT_360).to_dict(),
    ])
    assert [row.id for row in pool.assets] == ["L0", "B1"]
    assert pool.asset_records[0]["noi"] == {"amount": "1200000", "currency": "USD"}
    with pytest.raises(ValueError, match="assets"):
        AssetPool("P", "cmbs", USD).with_assets(42)
    with pytest.raises(ValueError, match="credit_quality"):
        PoolAsset(
            "X", {"type": "high_yield_bond", "industry": None}, usd(1.0), 0.05, MATURITY, credit_quality="not-a-rating"
        )

    seasoned = pool.with_accounts(cumulative_defaults=usd(8_000_000.0), collection_account=usd(250_000.0))
    assert seasoned.cumulative_defaults == usd(8_000_000.0)
    assert seasoned.collection_account == usd(250_000.0)
    assert pool.cumulative_defaults == usd(0.0)


def test_npl_liquidation_terms_replace_the_default_flag() -> None:
    npl = PoolAsset.fixed_rate_bond("NPL-1", usd(100_000_000.0), 0.06, MATURITY, DayCount.THIRTY_360).to_dict()
    npl["liquidation"] = {
        "months_to_resolution": 24,
        "proceeds_pct": 55.0,
        "reperformance_prob": 0.2,
        "modified_rate": 0.04,
    }
    pool = AssetPool("P", "rmbs", USD).with_assets([npl])
    deal = StructuredCredit.new_rmbs(
        "NPL-1",
        pool,
        TrancheStructure.from_balances([_tranche("A", 0.0, 100.0, "senior", 100_000_000.0, 0.05)]),
        CLOSE,
        MATURITY,
        "USD-OIS",
        payment_calendar_id="nyse",
    )
    assert deal.pool.assets[0].liquidation["modified_rate"] == 0.04
    periods = deal.run_simulation_with_diagnostics(_market(), CLOSE).periods
    resolved = next(period for period in periods if period["payment_date"] >= "2026-01-01")
    assert float(resolved["defaults"]["amount"]) == pytest.approx(80_000_000.0)

    npl["is_defaulted"] = True
    npl["default_date"] = "2023-06-01"
    npl["recovery_amount"] = {"amount": "40000000", "currency": "USD"}
    with pytest.raises(ValueError, match="liquidation"):
        StructuredCredit.new_rmbs(
            "NPL-2",
            AssetPool("P", "rmbs", USD).with_assets([npl]),
            TrancheStructure.from_balances([_tranche("A", 0.0, 100.0, "senior", 100_000_000.0, 0.05)]),
            CLOSE,
            MATURITY,
            "USD-OIS",
            payment_calendar_id="nyse",
        )


def test_tranche_builder_sets_every_seasoned_field() -> None:
    tranche = (
        Tranche
        .builder()
        .id("B")
        .attachment_point(10.0)
        .detachment_point(40.0)
        .seniority("mezzanine")
        .original_balance(usd(30_000_000.0))
        .current_balance(usd(25_000_000.0))
        .deferred_interest(usd(100_000.0))
        .coupon_fixed(0.07)
        .maturity(MATURITY)
        .rating("BBB")
        .pik_enabled(True)
        .oc_trigger({"trigger_level": 1.15, "consequence": "divert_cash_flow"})
        .ic_trigger({"trigger_level": 1.05, "cure_level": 1.10, "consequence": "divert_cash_flow"})
        .attributes({"desk": "alts", "tags": ["mezz"]})
        .build()
    )
    assert tranche.current_balance == usd(25_000_000.0)
    assert tranche.deferred_interest == usd(100_000.0)
    assert tranche.rating == "BBB"
    assert tranche.pik_enabled is True
    assert tranche.oc_trigger["trigger_level"] == 1.15
    assert tranche.ic_trigger["cure_level"] == 1.10
    assert tranche.attributes.tags == ["mezz"]
    with pytest.raises(ValueError, match="rating"):
        Tranche.builder().rating("junk")


def test_deal_builder_setters_land_in_the_wire_form_and_getters() -> None:
    deal = _clo()
    spec = json.loads(deal.to_json())["instrument"]["spec"]
    assert spec["prepayment_spec"] == {"cpr": 0.1, "curve": None}
    assert spec["coverage_triggers"][0]["kind"] == "oc"
    assert spec["coverage_rules"]["ccc_bucket"]["threshold_pct"] == 7.5
    assert spec["call_assumption"] == {"date": "2028-01-01", "price_pct": 100.0, "scope": "deal"}
    assert spec["cleanup_call_pct"] == 0.1
    assert spec["liquidation_price_pct"] == 99.0
    assert spec["loss_allocation"] == "par_preserving"
    assert spec["principal_covers_senior_interest"] is True

    assert deal.prepayment_spec.cpr == 0.1
    assert deal.default_spec.cdr == 0.02
    assert deal.recovery_spec.recovery_lag == 12
    assert deal.credit_model["recovery_spec"]["rate"] == 0.4
    assert deal.stochastic_prepay_spec is None
    assert deal.delinquency is None
    assert deal.card is None
    assert deal.call_assumption.scope == "deal"
    assert deal.call_assumption.tranche_id is None
    assert deal.coverage_rules.discount_obligation["price_threshold_pct"] == 80.0
    assert deal.coverage_triggers[0]["tranche_id"] == "A"
    assert deal.cleanup_call_pct == 0.1
    assert deal.liquidation_price_pct == 99.0
    assert deal.loss_allocation == "par_preserving"
    assert deal.principal_covers_senior_interest is True
    assert deal.frequency == Tenor.quarterly()
    assert deal.payment_calendar_id == "nyse"
    assert deal.attributes.items() == [("desk", "abs")]
    assert deal.fees is None
    assert deal.waterfall is None
    assert deal.hedge_swaps == []
    assert deal.with_standard_fees().fees["special_servicer_fee_bp"] is None
    assert deal.enable_stochastic_defaults().stochastic_prepay_spec is not None
    assert StructuredCredit.from_json(deal.to_json()).to_dict() == deal.to_dict()


def test_credit_model_can_be_replaced_whole_and_then_refined() -> None:
    whole = {
        "prepayment_spec": {"cpr": 0.05, "curve": None},
        "default_spec": {"cdr": 0.01, "curve": None},
        "recovery_spec": {"rate": 0.5, "recovery_lag": 6},
    }
    pool = AssetPool("P", "abs", USD).with_assets(_loans())
    deal = (
        StructuredCredit
        .builder()
        .id("ABS-CM")
        .deal_type("abs")
        .pool(pool)
        .tranches(TrancheStructure([_tranche("A", 0.0, 100.0, "senior", 100_000_000.0, 0.05)]))
        .closing_date(CLOSE)
        .first_payment_date(datetime.date(2024, 2, 1))
        .maturity(MATURITY)
        .frequency(Tenor.monthly())
        .payment_calendar_id("nyse")
        .discount_curve_id("USD-OIS")
        .credit_model(whole)
        .default_spec(DefaultModelSpec.cumulative_loss([0.5, 1.0, 1.5], 0.6))
        .delinquency({"roll_rates": [0.5, 0.6, 1.0], "cure_rates": [0.2, 0.1, 0.0], "advancing": {"policy": "none"}})
        .build()
    )
    assert deal.prepayment_spec.cpr == 0.05
    assert deal.default_spec.curve["curve"] == "cumulative_loss"
    assert deal.recovery_spec.recovery_lag == 6
    assert deal.delinquency["roll_rates"] == [0.5, 0.6, 1.0]


def test_diagnostics_carry_the_period_record_and_coverage_tests() -> None:
    deal = _clo()
    diagnostics = deal.run_simulation_with_diagnostics(_market(), CLOSE)
    periods = diagnostics.periods
    assert periods
    assert periods[0]["coverage_tests"][0]["test_id"] == "OC_A"
    frame = diagnostics.to_dataframe()
    assert len(frame) == len(periods)
    assert frame["pool_factor"].iloc[0] == pytest.approx(periods[0]["pool_factor"])
    assert (frame["pool_factor"].diff().dropna() <= 1e-12).all()
    tests = diagnostics.coverage_tests_dataframe()
    assert list(tests.columns) == ["date", "test_id", "ratio", "trigger_level", "cushion", "passing"]
    assert len(tests) == len(periods)
    assert (tests["cushion"] == tests["ratio"] - tests["trigger_level"]).all()
    # The deal call redeems everything in 2028.
    assert frame["date"].iloc[-1] <= "2028-04-01"


def test_tranche_cashflows_and_equity_metrics_are_typed_results() -> None:
    deal = _clo()
    market = _market()
    flows = deal.tranche_cashflows("A", market, CLOSE)
    assert isinstance(flows, TrancheCashflows)
    frame = flows.to_dataframe()
    assert frame["interest"].sum() == pytest.approx(flows.total_interest.amount)
    assert frame["principal"].sum() == pytest.approx(flows.total_principal.amount)
    assert flows.total_principal.amount == pytest.approx(60_000_000.0)
    assert len(flows.accrual_periods) == len(flows.interest_flows)
    assert TrancheCashflows.from_json(flows.to_json()).to_dict() == flows.to_dict()
    with pytest.raises(KeyError, match="Z"):
        deal.tranche_cashflows("Z", market, CLOSE)

    equity = deal.equity_metrics(market, CLOSE)
    assert isinstance(equity, EquityMetrics)
    assert equity.tranche_id == "E"
    assert equity.currency == "USD"
    assert equity.invested == 10_000_000.0
    assert equity.irr is not None
    assert math.isfinite(equity.irr)
    assert equity.moic == pytest.approx(sum(value for _, value in equity.cash_on_cash))
    assert len(equity.to_dataframe()) == len(equity.cash_on_cash)
    cheaper = deal.equity_metrics(market, CLOSE, purchase_price_pct=80.0)
    assert cheaper.invested == 8_000_000.0
    assert cheaper.irr > equity.irr
    assert EquityMetrics.from_json(equity.to_json()).moic == equity.moic


def test_waterfall_introspection_and_custom_waterfall_round_trip() -> None:
    deal = _clo()
    waterfall = deal.create_waterfall()
    assert isinstance(waterfall, Waterfall)
    assert waterfall.base_currency == "USD"
    assert [test["id"] for test in waterfall.coverage_tests()] == ["OC_A"]
    assert waterfall.coverage_rules.ccc_bucket["threshold_pct"] == 7.5
    assert Waterfall.from_json(waterfall.to_json()).to_dict() == waterfall.to_dict()

    custom = StructuredCredit.from_json(
        json.dumps({
            **json.loads(deal.to_json()),
        })
    )
    rebuilt = (
        StructuredCredit
        .builder()
        .id("CLO-CUSTOM")
        .deal_type("clo")
        .pool(custom.pool)
        .tranches(custom.tranches)
        .closing_date(CLOSE)
        .first_payment_date(datetime.date(2024, 4, 1))
        .maturity(MATURITY)
        .frequency(Tenor.quarterly())
        .payment_calendar_id("nyse")
        .discount_curve_id("USD-OIS")
        .waterfall(waterfall)
        .build()
    )
    assert rebuilt.waterfall.to_dict() == waterfall.to_dict()
    assert rebuilt.create_waterfall().to_dict() == waterfall.to_dict()


def test_tranche_metrics_report_the_to_call_twins() -> None:
    deal = _clo()
    metrics = structured_credit_tranche_metrics(deal.to_json(), "A", _market(), "2024-01-01")
    assert metrics.wal_to_call is not None
    assert metrics.wal_to_call < metrics.wal
    assert metrics.z_spread_to_call_bp is not None
    assert metrics.dm_to_call_bp is None
    frame = metrics.to_dataframe()
    assert list(frame.columns)[-3:] == ["wal_to_call", "z_spread_to_call_bp", "dm_to_call_bp"]
    assert math.isnan(frame["dm_to_call_bp"].iloc[0])
    payload = json.loads(metrics.to_json())
    assert "wal_to_call" in payload
    assert "dm_to_call_bp" not in payload


def test_hedge_swaps_and_call_assumptions_are_typed_and_round_trip() -> None:
    swap = InterestRateSwap.example_standard()
    hedge = HedgeSwap(swap, notional={"tranche_par": "A"}, priority="junior_fee")
    assert hedge.swap.id == swap.id
    assert hedge.notional == {"tranche_par": "A"}
    assert hedge.priority == "junior_fee"
    assert HedgeSwap(swap).notional == "contractual"
    assert HedgeSwap(swap).priority == "senior_fee"
    assert HedgeSwap.from_json(hedge.to_json()).to_dict() == hedge.to_dict()
    with pytest.raises(ValueError, match="priority"):
        HedgeSwap(swap, priority="first")

    tranche_call = CallAssumption(datetime.date(2027, 1, 15), 101.0, tranche_id="B")
    assert tranche_call.scope == "tranche"
    assert tranche_call.tranche_id == "B"
    assert tranche_call.scope_spec == {"tranche": "B"}
    assert CallAssumption.from_json(tranche_call.to_json()).price_pct == 101.0
    rules = CoverageRules(rating_haircuts={"NR": 0.5}, defaulted_valuation={"market_value": {"pct": 60.0}})
    assert rules.defaulted_valuation == {"market_value": {"pct": 60.0}}
    with pytest.raises(ValueError, match="haircut"):
        CoverageRules(rating_haircuts={"NR": 1.5})


def test_typed_constructors_accept_the_payment_calendar() -> None:
    pool = AssetPool("P", "clo", USD).with_assets(_loans())
    deal = StructuredCredit.new_clo(
        "CLO-CAL",
        pool,
        TrancheStructure([_tranche("A", 0.0, 100.0, "senior", 100_000_000.0, 0.05)]),
        CLOSE,
        MATURITY,
        "USD-OIS",
        payment_calendar_id="nyse",
    )
    assert deal.payment_calendar_id == "nyse"
    flows = deal.tranche_cashflows("A", _market(), CLOSE)
    # Registry defaults carry defaults and recoveries, so the note is not made whole.
    assert 0.0 < flows.total_principal.amount < 100_000_000.0
    assert flows.to_dataframe()["principal"].sum() == pytest.approx(flows.total_principal.amount)


def test_attachment_points_are_derived_from_balances() -> None:
    def note(id_: str, seniority: str, balance: float, coupon: float = 0.05) -> Tranche:
        return (
            Tranche
            .builder()
            .id(id_)
            .seniority(seniority)
            .original_balance(usd(balance))
            .coupon_fixed(coupon)
            .maturity(MATURITY)
            .build()
        )

    alone = note("A", "senior", 60_000_000.0)
    assert alone.attachment_point is None
    assert alone.detachment_point is None
    with pytest.raises(ValueError, match="together"):
        Tranche.builder().id("X").attachment_point(0.0).seniority("equity").original_balance(usd(1.0)).coupon_fixed(
            0.0
        ).maturity(MATURITY).build()

    derived = TrancheStructure([
        note("A", "senior", 60_000_000.0),
        note("B", "mezzanine", 30_000_000.0),
        note("E", "equity", 10_000_000.0),
    ])
    assert [(t.id, t.attachment_point, t.detachment_point) for t in derived.tranches] == [
        ("A", 40.0, 100.0),
        ("B", 10.0, 40.0),
        ("E", 0.0, 10.0),
    ]
    # Declared points that disagree with the balances are rejected; from_balances discards them.
    declared = [
        _tranche("A", 50.0, 100.0, "senior", 60_000_000.0, 0.05),
        _tranche("E", 0.0, 50.0, "equity", 40_000_000.0, 0.0),
    ]
    with pytest.raises(ValueError, match="balance shares"):
        TrancheStructure(declared)
    rebuilt = TrancheStructure.from_balances(declared)
    assert [(t.id, t.attachment_point, t.detachment_point) for t in rebuilt.tranches] == [
        ("A", 40.0, 100.0),
        ("E", 0.0, 40.0),
    ]
    assert TrancheStructure.from_json(rebuilt.to_json()).to_json() == rebuilt.to_json()

    # A deal on the derived structure projects like the declared twin.
    explicit = _clo()
    pool = AssetPool("P", "clo", USD).with_assets(_loans())
    twin = (
        StructuredCredit
        .builder()
        .id("CLO-TYPED")
        .deal_type("clo")
        .pool(pool)
        .tranches(
            TrancheStructure([
                note("A", "senior", 60_000_000.0),
                note("B", "mezzanine", 30_000_000.0, 0.07),
                note("E", "equity", 10_000_000.0, 0.0),
            ])
        )
        .closing_date(CLOSE)
        .first_payment_date(datetime.date(2024, 4, 1))
        .maturity(MATURITY)
        .frequency(Tenor.quarterly())
        .payment_calendar_id("nyse")
        .discount_curve_id("USD-OIS")
        .prepayment_spec(PrepaymentModelSpec.constant_cpr(0.10))
        .default_spec(DefaultModelSpec.constant_cdr(0.02))
        .recovery_spec(RecoveryModelSpec(0.4, 12))
        .build()
    )
    for tranche_id in ("A", "B", "E"):
        left = explicit.tranche_cashflows(tranche_id, _market(), CLOSE).to_dataframe()
        right = twin.tranche_cashflows(tranche_id, _market(), CLOSE).to_dataframe()
        # The explicit deal also carries coverage tests and a call, so compare the
        # first payment only, where neither has acted yet.
        assert left.iloc[0]["interest"] == pytest.approx(right.iloc[0]["interest"])


def test_curve_shape_constructors_match_their_rust_shapes() -> None:
    assert PrepaymentModelSpec.abs(0.015).curve == {"curve": "abs", "speed": 0.015}
    assert PrepaymentModelSpec.vector([0.05, 0.1]).curve == {"curve": "vector", "monthly_cpr": [0.05, 0.1]}
    assert DefaultModelSpec.vector([0.01]).curve == {"curve": "vector", "monthly_cdr": [0.01]}
    timing = DefaultModelSpec.timing(0.1, [20.0, 30.0, 50.0])
    assert timing.curve["annual_pct"] == [20.0, 30.0, 50.0]
    timing.validate()
    with pytest.raises(ValueError, match="100"):
        DefaultModelSpec.timing(0.1, [20.0, 30.0]).validate()
    with pytest.raises(ValueError, match="speed"):
        PrepaymentModelSpec.abs(1.5).validate()
    recovery = RecoveryModelSpec(0.4, 12).with_severity_vector([0.5, 0.7])
    assert recovery.severity_vector == [0.5, 0.7]
    assert recovery.recovery_rate(0) == 0.5
    assert recovery.recovery_rate(5) == pytest.approx(0.3)
    assert RecoveryModelSpec(0.4, 12).recovery_rate(5) == 0.4


def _money(value: dict) -> Money:
    return Money(float(value["amount"]), Currency(value["currency"]))


def _date(value: str) -> datetime.date:
    return datetime.date.fromisoformat(value)


def _tenor(value: dict) -> Tenor:
    unit = {"days": "D", "weeks": "W", "months": "M", "years": "Y"}[value["unit"]]
    return Tenor(f"{value['count']}{unit}")


def _typed_deal_from_golden(spec: dict) -> StructuredCredit:
    """Rebuild the golden's deal from typed calls only (no ``from_json``)."""
    pool_spec = spec["pool"]
    assets = [
        (
            PoolAsset(
                row["id"],
                row["asset_type"],
                _money(row["balance"]),
                row["rate"],
                _date(row["maturity"]),
                day_count=DayCount.parse(row["day_count"]),
                spread_bp=row.get("spread_bp"),
                index_id=row.get("index_id"),
                credit_quality=row.get("credit_quality"),
                industry=row.get("industry"),
                obligor_id=row.get("obligor_id"),
                is_defaulted=row.get("is_defaulted", False),
                recovery_amount=_money(row["recovery_amount"]) if row.get("recovery_amount") else None,
                default_date=_date(row["default_date"]) if row.get("default_date") else None,
                purchase_price=_money(row["purchase_price"]) if row.get("purchase_price") else None,
                acquisition_date=_date(row["acquisition_date"]) if row.get("acquisition_date") else None,
                smm_override=row.get("smm_override"),
                mdr_override=row.get("mdr_override"),
                recovery_rate=row.get("recovery_rate"),
                commitment=_money(row["commitment"]) if row.get("commitment") else None,
                contractual_payment=_money(row["contractual_payment"]) if row.get("contractual_payment") else None,
                market_price_pct=row.get("market_price_pct"),
            )
        )
        for row in pool_spec["assets"]
    ]
    pool = (
        AssetPool(pool_spec["id"], pool_spec["deal_type"], Currency(pool_spec["base_currency"]))
        .with_assets(assets)
        .with_accounts(
            cumulative_defaults=_money(pool_spec["cumulative_defaults"]),
            cumulative_recoveries=_money(pool_spec["cumulative_recoveries"]),
            cumulative_prepayments=_money(pool_spec["cumulative_prepayments"]),
            cumulative_scheduled_amortization=_money(pool_spec["cumulative_scheduled_amortization"]),
            collection_account=_money(pool_spec["collection_account"]),
            excess_spread_account=_money(pool_spec["excess_spread_account"]),
        )
        .with_reserve(_money(pool_spec["reserve_account"]))
        .with_reinvestment_period(pool_spec["reinvestment_period"])
    )
    tranches = []
    for row in spec["tranches"]["tranches"]:
        builder = (
            Tranche
            .builder()
            .id(row["id"])
            .attachment_point(row["attachment_point"])
            .detachment_point(row["detachment_point"])
            .seniority(row["seniority"])
            .original_balance(_money(row["original_balance"]))
            .current_balance(_money(row["current_balance"]))
            .deferred_interest(_money(row["deferred_interest"]))
            .maturity(_date(row["maturity"]))
            .frequency(_tenor(row["frequency"]))
            .day_count(DayCount.parse(row["day_count"]))
            .pik_enabled(row.get("pik_enabled", False))
            .attributes(row["attributes"]["meta"] | {"tags": row["attributes"]["tags"]})
        )
        if "fixed" in row["coupon"]:
            builder = builder.coupon_fixed(row["coupon"]["fixed"]["rate"])
        else:
            builder = builder.coupon_floating(row["coupon"])
        if row.get("rating"):
            builder = builder.rating(row["rating"])
        if row.get("oc_trigger"):
            builder = builder.oc_trigger(row["oc_trigger"])
        if row.get("ic_trigger"):
            builder = builder.ic_trigger(row["ic_trigger"])
        tranches.append(builder.build())
    builder = (
        StructuredCredit
        .builder()
        .id(spec["id"])
        .deal_type(spec["deal_type"])
        .pool(pool)
        .tranches(TrancheStructure(tranches))
        .closing_date(_date(spec["closing_date"]))
        .first_payment_date(_date(spec["first_payment_date"]))
        .maturity(_date(spec["maturity"]))
        .frequency(_tenor(spec["frequency"]))
        .discount_curve_id(spec["discount_curve_id"])
        .payment_calendar_id(spec["payment_calendar_id"])
        .payment_business_day_convention(spec["payment_business_day_convention"])
        .attributes(spec["attributes"]["meta"] | {"tags": spec["attributes"]["tags"]})
        .prepayment_spec(PrepaymentModelSpec.from_json(json.dumps(spec["prepayment_spec"])))
        .default_spec(DefaultModelSpec.from_json(json.dumps(spec["default_spec"])))
        .recovery_spec(RecoveryModelSpec.from_json(json.dumps(spec["recovery_spec"])))
        .stochastic_prepay_spec(spec["stochastic_prepay_spec"])
        .stochastic_default_spec(spec["stochastic_default_spec"])
        .correlation_structure(spec["correlation_structure"])
        .market_conditions(spec["market_conditions"])
        .credit_factors(spec["credit_factors"])
        .deal_metadata(spec["deal_metadata"])
        .behavior_overrides(spec["behavior_overrides"])
        .hedge_swaps(spec["hedge_swaps"])
    )
    return builder.build()


def test_typed_only_clo_reproduces_the_regression_golden() -> None:
    fixture = GoldenFixture.from_path(GOLDEN)
    body = fixture.body
    spec = body["instrument"]["instrument"]["spec"]

    deal = _typed_deal_from_golden(spec)
    # Everything except the Rust-only pricing-override bags must match the
    # canonical deal field for field (both sides rendered by the same Rust
    # serializer, so serde defaults the fixture omits compare equal).
    overrides = {"instrument_pricing_overrides", "metric_pricing_overrides", "scenario_pricing_overrides"}
    typed_spec = {k: v for k, v in deal.to_dict().items() if k not in overrides}
    canonical = StructuredCredit.from_json(json.dumps(body["instrument"]))
    expected_spec = {k: v for k, v in canonical.to_dict().items() if k not in overrides}
    assert typed_spec == expected_spec

    market = _resolve_market(body["market"])
    result = price_instrument(deal.to_json(), market, fixture.metadata.valuation_date, model=body["model"])
    tolerance = fixture.tolerances["npv"]
    assert float(result.price) == pytest.approx(fixture.expected["npv"], abs=tolerance.abs, rel=tolerance.rel)
