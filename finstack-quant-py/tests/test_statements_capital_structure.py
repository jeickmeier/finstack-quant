"""Tests for the capital-structure Python bindings.

Covers the spec wrappers (EcfSweepSpec, PikToggleSpec, WaterfallSpec) and the
ModelBuilder extensions that attach debt instruments + waterfall configuration
to a FinancialModelSpec.
"""

from __future__ import annotations

from datetime import date

import pytest

from finstack_quant import statements
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import StubKind
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.valuations.instruments import Bond, TermLoan


def _debt_envelope_json(instrument_id: str) -> str:
    return Bond.fixed(
        instrument_id,
        Money(25_000_000.0, Currency("USD")),
        Rate(0.05),
        date(2025, 1, 15),
        date(2030, 1, 15),
        StubKind.SHORT_FRONT,
        "USD-OIS",
    ).to_json()


class TestEcfSweepSpec:
    def test_construct_with_minimum_args(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.5)
        assert ecf.ebitda_node == "ebitda"
        assert ecf.sweep_percentage == 0.5
        assert ecf.target_instrument_id is None

    def test_construct_with_all_args(self) -> None:
        ecf = statements.EcfSweepSpec(
            ebitda_node="ebitda",
            sweep_percentage=0.75,
            taxes_node="taxes",
            capex_node="capex",
            working_capital_node="wc",
            cash_interest_node="cs.interest_expense_cash.total",
            target_instrument_id="TL-A",
        )
        assert ecf.target_instrument_id == "TL-A"

    def test_json_roundtrip(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.5, target_instrument_id="X")
        restored = statements.EcfSweepSpec.from_json(ecf.to_json())
        assert restored.to_json() == ecf.to_json()


class TestPikToggleSpec:
    def test_defaults_min_periods_to_zero(self) -> None:
        pik = statements.PikToggleSpec(liquidity_metric="cash", threshold=1e6, target_instrument_ids=["TL"])
        assert pik.min_periods_in_pik == 0

    def test_json_roundtrip(self) -> None:
        pik = statements.PikToggleSpec(
            liquidity_metric="cash",
            threshold=1e6,
            target_instrument_ids=["A", "B"],
            min_periods_in_pik=3,
        )
        restored = statements.PikToggleSpec.from_json(pik.to_json())
        assert restored.to_json() == pik.to_json()


class TestPaymentClassSpec:
    def test_construct_and_roundtrip(self) -> None:
        cls = statements.PaymentClassSpec("1L", 0, ["TL-A", "TL-B"])
        assert cls.id == "1L"
        assert cls.rank == 0
        assert cls.instrument_ids == ["TL-A", "TL-B"]
        restored = statements.PaymentClassSpec.from_json(cls.to_json())
        assert restored.to_json() == cls.to_json()


class TestWaterfallSpec:
    def test_default_priority_order(self) -> None:
        ws = statements.WaterfallSpec()
        assert ws.priority_of_payments == [
            "fees",
            "interest",
            "amortization",
            "sweep",
            "equity",
        ]
        assert not ws.has_ecf_sweep
        assert not ws.has_pik_toggle
        assert ws.available_cash_node == "cash"
        assert ws.payment_classes == []
        assert ws.mandatory_prepay_node is None
        assert ws.voluntary_prepay_node is None

    def test_custom_priority(self) -> None:
        ws = statements.WaterfallSpec(priority_of_payments=["interest", "sweep", "equity"])
        assert ws.priority_of_payments == ["interest", "sweep", "equity"]

    def test_rejects_unknown_priority_token(self) -> None:
        with pytest.raises(ValueError, match="nonexistent"):
            statements.WaterfallSpec(priority_of_payments=["nonexistent"])

    def test_with_ecf_and_pik(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.5)
        pik = statements.PikToggleSpec(liquidity_metric="cash", threshold=1e6, target_instrument_ids=["TL"])
        ws = statements.WaterfallSpec(ecf_sweep=ecf, pik_toggle=pik)
        assert ws.has_ecf_sweep
        assert ws.has_pik_toggle

    def test_validate_accepts_standard_priority(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.5)
        ws = statements.WaterfallSpec(ecf_sweep=ecf, available_cash_node="cash")
        ws.validate()  # should not raise

    def test_validate_rejects_sweep_after_equity(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.5)
        ws = statements.WaterfallSpec(
            priority_of_payments=["fees", "interest", "amortization", "equity", "sweep"],
            ecf_sweep=ecf,
            available_cash_node="cash",
        )
        # A sweep after equity means equity is not last, which the
        # "Equity must be the last entry" rule rejects.
        with pytest.raises(ValueError, match=r"Equity.*must be the last entry"):
            ws.validate()

    def test_json_roundtrip(self) -> None:
        ecf = statements.EcfSweepSpec(ebitda_node="ebitda", sweep_percentage=0.25)
        ws = statements.WaterfallSpec(available_cash_node="free_cash_flow", ecf_sweep=ecf)
        restored = statements.WaterfallSpec.from_json(ws.to_json())
        assert restored.to_json() == ws.to_json()


class TestModelBuilderCapitalStructure:
    @pytest.fixture
    def usd(self) -> Currency:
        return Currency("USD")

    def test_add_bond_appears_in_model_json(self, usd: Currency) -> None:
        b = statements.ModelBuilder("deal")
        b.add_bond(
            id="BOND-001",
            notional=Money(10_000_000.0, usd),
            coupon_rate=0.05,
            issue_date=date(2025, 1, 15),
            maturity_date=date(2030, 1, 15),
            discount_curve_id="USD-OIS",
        )
        b.periods("2025Q1..Q4", None)
        b.value("revenue", [("2025Q1", 1.0)])
        model = b.build()
        js = model.to_json()
        assert '"capital_structure"' in js
        assert '"BOND-001"' in js

    def test_add_swap_appears_in_model_json(self, usd: Currency) -> None:
        b = statements.ModelBuilder("deal")
        b.add_swap(
            id="SWAP-001",
            notional=Money(5_000_000.0, usd),
            fixed_rate=0.04,
            start_date=date(2025, 1, 15),
            maturity_date=date(2030, 1, 15),
            discount_curve_id="USD-OIS",
            forward_curve_id="USD-SOFR-3M",
        )
        b.periods("2025Q1..Q4", None)
        b.value("x", [("2025Q1", 1.0)])
        model = b.build()
        assert '"SWAP-001"' in model.to_json()

    def test_add_debt_passes_through_canonical_envelope(self) -> None:
        b = statements.ModelBuilder("deal")
        b.add_debt("TL-A", _debt_envelope_json("TL-A"))
        b.periods("2025Q1..Q1", None)
        b.value("x", [("2025Q1", 1.0)])
        model = b.build()
        assert '"TL-A"' in model.to_json()

    def test_reporting_currency_and_fx_policy(self, usd: Currency) -> None:
        b = statements.ModelBuilder("deal")
        b.reporting_currency(usd)
        b.fx_policy("period_end")
        b.periods("2025Q1..Q1", None)
        b.value("x", [("2025Q1", 1.0)])
        b.add_debt("T", _debt_envelope_json("T"))
        js = b.build().to_json()
        assert '"reporting_currency":"USD"' in js
        assert '"fx_policy":"period_end"' in js

    def test_fx_policy_rejects_unknown_variant(self) -> None:
        b = statements.ModelBuilder("deal")
        with pytest.raises(ValueError, match="invalid fx_policy"):
            b.fx_policy("bogus_policy")

    def test_waterfall_attaches_to_model(self) -> None:
        loan = TermLoan.example()
        ecf = statements.EcfSweepSpec(
            ebitda_node="ebitda",
            sweep_percentage=0.5,
            target_instrument_id=loan.id,
        )
        first_lien = statements.PaymentClassSpec("1L", 0, [loan.id])
        ws = statements.WaterfallSpec(
            ecf_sweep=ecf,
            available_cash_node="cash",
            payment_classes=[first_lien],
        )
        b = statements.ModelBuilder("deal")
        b.add_debt(loan.id, loan.to_json())
        b.waterfall(ws)
        b.periods("2025Q1..Q1", None)
        b.value("x", [("2025Q1", 1.0)])
        js = b.build().to_json()
        assert '"waterfall"' in js
        assert '"ecf_sweep"' in js

    def test_capital_structure_methods_work_before_periods(self, usd: Currency) -> None:
        """Capital-structure methods must be state-agnostic (both NeedPeriods and Ready)."""
        b = statements.ModelBuilder("deal")
        # Attach capital structure BEFORE periods
        b.add_bond(
            id="B1",
            notional=Money(1_000_000.0, usd),
            coupon_rate=0.05,
            issue_date=date(2025, 1, 15),
            maturity_date=date(2030, 1, 15),
            discount_curve_id="USD-OIS",
        )
        b.reporting_currency(usd)
        # Then transition through periods and node addition
        b.periods("2025Q1..Q1", None)
        b.value("x", [("2025Q1", 1.0)])
        model = b.build()
        assert '"B1"' in model.to_json()


class TestModelBuilderForecasts:
    def test_forecast_spec_json_roundtrip(self) -> None:
        spec = statements.ForecastSpec.growth(0.05)
        restored = statements.ForecastSpec.from_json(spec.to_json())
        assert restored.to_json() == spec.to_json()

    def test_value_scalar_and_forecast_build_model(self) -> None:
        b = statements.ModelBuilder("forecast")
        b.periods("2025Q1..Q4", "2025Q2")
        b.value_scalar("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
        b.forecast("revenue", statements.ForecastSpec.growth(0.05))
        model = b.build()
        js = model.to_json()
        assert '"node_type":"mixed"' in js
        assert '"forecast"' in js

    def test_value_money_builds_monetary_node(self) -> None:
        usd = Currency("USD")
        b = statements.ModelBuilder("money")
        b.periods("2025Q1..Q1", None)
        b.value_money("revenue", [("2025Q1", Money(100.0, usd))])
        js = b.build().to_json()
        assert '"value_type":{"type":"monetary","currency":"USD"}' in js

    def test_mixed_builder_returns_ready_builder(self) -> None:
        b = statements.ModelBuilder("mixed")
        b.periods("2025Q1..Q4", "2025Q1")
        mixed = b.mixed("revenue")
        mixed.values([("2025Q1", 100.0)])
        mixed.forecast(statements.ForecastSpec.forward_fill())
        mixed.formula("lag(revenue, 1)")
        b = mixed.build()
        model = b.build()
        assert model.has_node("revenue")

    def test_where_meta_and_builtin_metrics(self) -> None:
        b = statements.ModelBuilder("metrics")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", 100.0)])
        b.value("cogs", [("2025Q1", 40.0)])
        b.compute("bonus", "revenue * 0.01")
        b.where_clause("revenue > 50")
        b.with_meta("source", '{"system":"unit-test"}')
        b.with_builtin_metrics()
        model = b.build()
        assert model.has_node("fin.gross_profit")
        assert '"where_text":"revenue > 50"' in model.to_json()

    def test_metric_registry_add_metric_from_registry(self) -> None:
        registry = statements.Registry.with_builtins()
        assert registry.has("fin.gross_profit")
        b = statements.ModelBuilder("metric")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", 100.0)])
        b.value("cogs", [("2025Q1", 40.0)])
        b.add_metric_from_registry("fin.gross_profit", registry)
        assert b.build().has_node("fin.gross_profit")

    def test_financial_model_from_json_runs_semantic_validation(self) -> None:
        with pytest.raises(ValueError, match="Model must have at least one period"):
            statements.FinancialModelSpec.from_json('{"schema_version":1,"id":"bad","periods":[],"nodes":{}}')


def test_pik_targets_are_required_and_nonempty() -> None:
    with pytest.raises(TypeError):
        statements.PikToggleSpec("cash", 1.0)
    with pytest.raises(TypeError):
        statements.PikToggleSpec("cash", 1.0, None)
    for payload in [
        '{"liquidity_metric":"cash","threshold":1.0}',
        '{"liquidity_metric":"cash","threshold":1.0,"target_instrument_ids":null}',
    ]:
        with pytest.raises(ValueError, match=r"field|sequence"):
            statements.PikToggleSpec.from_json(payload)
    with pytest.raises(ValueError, match="target_instrument_ids"):
        statements.PikToggleSpec("cash", 1.0, []).validate()
    explicit = statements.PikToggleSpec("cash", 1.0, ["TL"])
    explicit.validate()
    assert explicit.target_instrument_ids == ["TL"]
