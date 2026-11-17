"""Regression coverage for statements builder recovery and public contracts."""

import json
import math

import pytest

from finstack_quant import statements as s
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import PeriodPlan
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.statements_analytics import run_checks
from finstack_quant.valuations.instruments import InterestRateSwap
from tests.tests_typed_helpers import projection_curve


def test_canonical_forecast_and_registry_names() -> None:
    assert not hasattr(s.ForecastSpec, "override")
    assert s.ForecastSpec.overrides({"2025Q2": 120.0}).method.kind == "override"
    registry = s.Registry.with_builtins()
    assert not hasattr(registry, "dependencies")
    assert "fin.gross_profit" in registry.get_dependencies("fin.gross_margin")
    assert s.parse_formula("ltm(revenue)") == "ltm(revenue)"
    with pytest.raises(ValueError, match="Function 'ltm' is not supported"):
        s.parse_and_compile("ltm(revenue)")
    assert s.parse_and_compile("ttm(revenue)") is None


@pytest.mark.parametrize("field", ["default_tolerance", "default_relative_tolerance", "materiality_threshold"])
@pytest.mark.parametrize("value", [-1.0, math.nan, math.inf])
def test_check_config_rejects_invalid_thresholds(field: str, value: float) -> None:
    with pytest.raises(ValueError, match=field):
        s.CheckConfig(**{field: value})


def test_check_config_json_validates_thresholds() -> None:
    with pytest.raises(ValueError, match="default_tolerance"):
        s.CheckConfig.from_json('{"default_tolerance":-0.01}')
    assert s.CheckConfig().validate() is None


@pytest.mark.parametrize("field", ["default_tolerance", "default_relative_tolerance", "materiality_threshold"])
def test_check_suite_json_validates_config_thresholds(field: str) -> None:
    payload = json.dumps({"name": "invalid", "config": {field: -0.01}})
    with pytest.raises(ValueError, match=field):
        s.CheckSuiteSpec.from_json(payload)
    assert s.CheckSuiteSpec("valid", config=s.CheckConfig()).config.validate() is None


def test_forecast_spec_uses_canonical_log_normal_name() -> None:
    assert s.ForecastSpec.log_normal(0.01, 0.1, 42).method.kind == "log_normal"
    assert not hasattr(s.ForecastSpec, "lognormal")


def test_precomputed_checks_cannot_relabel_explicit_money_as_scalar() -> None:
    builder = s.ModelBuilder("mixed-currency")
    builder.periods("2025Q1..Q1", None)
    for node, amount, currency in [("assets", 100.0, "USD"), ("liabilities", 60.0, "EUR"), ("equity", 40.0, "EUR")]:
        builder.value_money(node, [("2025Q1", Money(amount, Currency(currency)))])
    model_payload = json.loads(builder.build().to_json())
    for node in model_payload["nodes"].values():
        node.pop("value_type", None)
    model = s.FinancialModelSpec.from_json(json.dumps(model_payload))
    results = s.Evaluator().evaluate(model)
    suite = s.CheckSuiteSpec(
        "accounting",
        builtin_checks=[
            {
                "type": "balance_sheet_articulation",
                "assets_nodes": ["assets"],
                "liabilities_nodes": ["liabilities"],
                "equity_nodes": ["equity"],
            }
        ],
    )
    with pytest.raises(ValueError, match="incompatible units"):
        run_checks(model, suite, results)
    result_payload = json.loads(results.to_json())
    assert result_payload["node_value_types"]["assets"] == {"type": "monetary", "currency": "USD"}
    result_payload["node_value_types"] = {node: {"type": "scalar"} for node in model_payload["nodes"]}
    with pytest.raises(ValueError, match="result units"):
        run_checks(model, suite, json.dumps(result_payload))


@pytest.mark.parametrize(("side", "fixed_rate", "forward_rate"), [("receive", 0.01, -0.01), ("pay", -0.02, 0.01)])
def test_swap_with_two_receiving_legs_reports_interest_income(
    side: str, fixed_rate: float, forward_rate: float
) -> None:
    builder = s.ModelBuilder("negative-rate-receiver")
    # The swap starts after the valuation date so its first reset is not a past fixing.
    builder.add_debt(
        "IRS",
        InterestRateSwap.from_conventions({
            "id": "IRS",
            "notional": Money(100.0, Currency("USD")),
            "side": "pay",
            "fixed_rate": fixed_rate,
            "start_date": "2025-01-06",
            "maturity": "2026-01-06",
            "index_id": "USD-SOFR-3M",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
        }),
    )
    builder.periods("2025..2025", None).value_scalar("cash", {"2025": 10.0})
    builder.compute("income", "cs.interest_income.IRS")
    model_payload = json.loads(builder.build().to_json())
    model_payload["capital_structure"]["debt_instruments"][0]["spec"]["spec"]["side"] = side
    model = s.FinancialModelSpec.from_json(json.dumps(model_payload))
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.02, "act_365f"))
        .insert(projection_curve("USD-SOFR-3M", "2025-01-01", [(10.0, forward_rate)]))
    )
    results = s.Evaluator().evaluate_with_market(model, market, "2025-01-01", "full_snapshot")
    assert results.cs_cashflows is not None
    cashflows = json.loads(results.cs_cashflows.to_json())
    flow = cashflows["by_instrument"]["IRS"]["2025"]
    assert float(flow["interest_expense_cash"]["amount"]) == 0.0
    income = float(flow["interest_income_cash"]["amount"])
    assert income > 0.0
    assert results.cs_cashflows.get_component("interest_expense_cash", "IRS", "2025") == 0.0
    assert results.cs_cashflows.get_component("debt_balance", "IRS", "2025") == 0.0
    assert results.cs_cashflows.get_component("accrued_interest", "IRS", "2025") == 0.0
    assert float(flow["principal_payment"]["amount"]) == 0.0
    assert results.get("income", "2025") == pytest.approx(income)
    # STMT-007: interest-income and per-currency totals are reachable typed.
    cs = results.cs_cashflows
    assert cs.get_component("interest_income", "IRS", "2025") == pytest.approx(income)
    usd_totals = cs.totals_by_currency["USD"]["2025"]
    assert usd_totals["interest_income_cash"].amount == pytest.approx(income)
    assert usd_totals["interest_expense_cash"].amount == 0.0
    if cs.reporting_currency is not None:
        assert cs.get_component("interest_income", "total", "2025") == pytest.approx(income)
        assert cs.get_component("interest_expense_cash", "total", "2025") == 0.0
        assert cs.get_component("interest_expense_pik", "total", "2025") == 0.0
        assert cs.get_component("accrued_interest", "total", "2025") == 0.0


def test_merge_into_results_writes_normalized_node() -> None:
    """STMT-007: normalized values go back into a StatementResult via Rust."""
    builder = s.ModelBuilder("normalize-merge")
    builder.periods("2025Q1..Q2", None)
    builder.value_money(
        "ebitda", [("2025Q1", Money(100.0, Currency("USD"))), ("2025Q2", Money(110.0, Currency("USD")))]
    )
    results = s.Evaluator().evaluate(builder.build())
    config = s.NormalizationConfig("ebitda").add_adjustment(
        s.Adjustment.fixed("addback", "Addback", {"2025Q1": 5.0, "2025Q2": 5.0})
    )
    normalized = s.normalize(results, config)

    merged = s.merge_into_results(results, normalized, "adjusted_ebitda")
    assert merged.get("adjusted_ebitda", "2025Q1") == pytest.approx(normalized[0].final_value)
    assert merged.get("adjusted_ebitda", "2025Q2") == pytest.approx(normalized[1].final_value)
    assert "adjusted_ebitda" not in results.node_ids()
    assert s.merge_into_results(results, [], "adjusted_ebitda").node_ids() == results.node_ids()
    with pytest.raises(ValueError, match="Duplicate normalization period"):
        s.merge_into_results(results, [normalized[0], normalized[0]], "adjusted_ebitda")


def test_model_rejects_rate_hedge_as_pik_target() -> None:
    builder = s.ModelBuilder("hedge-pik")
    builder.add_debt(
        "IRS",
        InterestRateSwap.from_conventions({
            "id": "IRS",
            "notional": Money(100.0, Currency("USD")),
            "side": "pay",
            "fixed_rate": 0.01,
            "start_date": "2025-01-01",
            "maturity": "2026-01-01",
            "index_id": "USD-SOFR-3M",
            "discount_curve_id": "USD-OIS",
            "forward_curve_id": "USD-SOFR-3M",
        }),
    )
    builder.periods("2025..2025", None).value_scalar("cash", {"2025": 10.0})
    builder.waterfall(s.WaterfallSpec(pik_toggle=s.PikToggleSpec("cash", 20.0, ["IRS"])))
    with pytest.raises(ValueError, match="only borrowing debt coupons can capitalize into principal"):
        builder.build()


def test_adjustment_cap_requires_an_explicit_supported_base_mode() -> None:
    adjustment = s.Adjustment.percentage("synergies", "Synergies", "revenue", 0.05)
    for mode in ("reported", "progressive"):
        capped = adjustment.with_cap("ebitda", 0.20, mode)
        assert capped.cap_base_mode == mode
    assert json.loads(adjustment.to_json()).get("cap") is None
    assert not hasattr(adjustment, "with_cap_mode")
    with pytest.raises(TypeError):
        adjustment.with_cap("ebitda", 0.20)  # type: ignore[call-arg]
    with pytest.raises(ValueError, match="reported or progressive"):
        adjustment.with_cap("ebitda", 0.20, "same_period")


def test_empty_explicit_periods_preserve_model_builder_for_retry() -> None:
    builder = s.ModelBuilder("recoverable")
    builder.value_scalar("revenue", {"2025Q1": 100.0})
    builder.with_meta("marker", {"retained": True})
    with pytest.raises(ValueError, match="at least one period"):
        builder.periods_explicit(PeriodPlan.from_json('{"calendar": "gregorian", "periods": []}'))
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.forecast("revenue", s.ForecastSpec.forward_fill())
    model = json.loads(builder.build().to_json())
    assert model["nodes"]["revenue"]["values"]["2025Q1"] == 100.0
    assert model["meta"]["marker"] == {"retained": True}


def test_failed_mixed_formula_preserves_configuration_and_parent() -> None:
    parent = s.ModelBuilder("mixed-recovery").periods("2025Q1..Q2", "2025Q1")
    parent.value_scalar("base", {"2025Q1": 100.0})
    parent.with_meta("marker", "retained")
    mixed = parent.mixed("mixed")
    assert mixed.values([("2025Q1", 40.0)]) is mixed
    assert mixed.forecast(s.ForecastSpec.forward_fill()) is mixed
    assert mixed.name("Retained name") is mixed
    with pytest.raises(ValueError, match="parse"):
        mixed.formula("(")
    mixed.formula("base * 0.5")
    model = json.loads(mixed.build().build().to_json())
    assert model["nodes"]["mixed"]["values"]["2025Q1"] == 40.0
    assert model["nodes"]["mixed"]["name"] == "Retained name"
    assert model["nodes"]["mixed"]["formula_text"] == "base * 0.5"
    assert model["meta"]["marker"] == "retained"
    assert "base" in model["nodes"]


def test_accounting_overflow_is_an_error_without_invalid_numbers() -> None:
    builder = s.ModelBuilder("overflow")
    builder.periods("2025Q1..Q2", None)
    for name, bad, good in [("a1", 1e308, 40.0), ("a2", 1e308, 60.0), ("l", 1e308, 70.0), ("e", 1e308, 30.0)]:
        builder.value_scalar(name, {"2025Q1": bad, "2025Q2": good})
    model = builder.build()
    suite = s.CheckSuiteSpec(
        "overflow",
        builtin_checks=[
            {
                "type": "balance_sheet_articulation",
                "assets_nodes": ["a1", "a2"],
                "liabilities_nodes": ["l"],
                "equity_nodes": ["e"],
            }
        ],
        config=s.CheckConfig(materiality_threshold=1e300),
    )
    # An unrepresentable sum is an execution error: no verdict and no
    # non-finite number is published.
    with pytest.raises(ValueError, match="cannot be represented"):
        run_checks(model, suite)


def test_bundled_earnings_use_reported_costs_and_add_da_back_once() -> None:
    """Hand-worked statement: COGS 400 and OPEX 300 include 80 of D&A between them."""
    registry = s.Registry.with_builtins()
    builder = s.ModelBuilder("income")
    builder.periods("2025Q1..Q1")
    for node, value in [
        ("revenue", 1000.0),
        ("cogs", 400.0),
        ("opex", 300.0),
        ("depreciation", 50.0),
        ("amortization", 30.0),
        ("interest_expense", 40.0),
        ("tax_expense", 65.0),
    ]:
        builder.value_scalar(node, [("2025Q1", value)])
    for metric in ("fin.ebitda_margin", "fin.net_margin"):
        builder.add_metric_from_registry(metric, registry)
    result = s.Evaluator().evaluate(builder.build())

    expected = {
        "fin.operating_income": 300.0,
        "fin.ebit": 300.0,
        "fin.ebitda": 380.0,
        "fin.net_income": 195.0,
        "fin.ebitda_margin": 0.38,
        "fin.net_margin": 0.195,
    }
    assert {node: result.get(node, "2025Q1") for node in expected} == expected


def test_bundled_margin_is_unavailable_without_positive_revenue() -> None:
    registry = s.Registry.with_builtins()
    builder = s.ModelBuilder("no-revenue")
    builder.periods("2025Q1..Q1")
    builder.value_scalar("revenue", [("2025Q1", 0.0)])
    builder.value_scalar("cogs", [("2025Q1", 40.0)])
    builder.add_metric_from_registry("fin.gross_margin", registry)
    result = s.Evaluator().evaluate(builder.build())

    assert result.get("fin.gross_profit", "2025Q1") == -40.0
    assert result.get_cell("fin.gross_margin", "2025Q1") == {
        "status": "unavailable",
        "reason": "undefined_base",
        "source": "formula",
    }


def test_forecast_seeds_are_exact_and_choices_are_explicit() -> None:
    """Seed 9007199254740993 is not representable as a float; it round-trips exactly."""
    seed = 9007199254740993
    spec = s.ForecastSpec.normal(0.0, 1.0, seed)
    assert spec.params["seed"] == seed
    document = json.loads(spec.to_json())
    assert document["params"]["seed"] == str(seed)
    assert s.ForecastSpec.from_json(spec.to_json()).params["seed"] == seed
    assert s.ForecastSpec(s.ForecastMethod.normal(), {"mean": 0.0, "std_dev": 1.0, "seed": seed}).params == spec.params

    with pytest.raises(ValueError, match="seed"):
        s.ForecastSpec.from_json(json.dumps({"method": "normal", "params": {"mean": 0.0, "std_dev": 1.0, "seed": 42}}))
    with pytest.raises(ValueError, match="shape"):
        s.ForecastSpec(s.ForecastMethod.fade_to_target(), {"target": 1.0})
    with pytest.raises(ValueError, match="half_life"):
        s.ForecastSpec(s.ForecastMethod.fade_to_target(), {"target": 1.0, "shape": "linear", "half_life": 2.0})
    with pytest.raises(ValueError, match="more than once"):
        s.ForecastSpec.from_json('{"method": "growth_pct", "params": {"rate": 0.1, "rate": 0.2}}')
    assert s.ForecastSpec.fade_to_target(1.0, "geometric").params["shape"] == "geometric"
