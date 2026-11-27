"""Loader route against JSON-text route, for parameters that accept JSON text.

Many parameters accept a typed object or the same object as JSON text. The
text form is being removed: a holder of JSON text then calls a named loader
first (``T.from_json``, ``Portfolio.from_spec``, or ``json.loads`` where the
parameter takes a ``dict``). While both routes exist, every case in ``CASES``
shows that the loader route gives exactly what the text route gives, on valid
text and on malformed text.

Where the two routes do differ, the case pins what each does in ``differs`` and
names the owner's ruling on it.

When the text form of a parameter is removed, set ``rejects_text`` on its case
to the exception the function then raises for a ``str``.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
import datetime as dt
import json
from typing import Any

import pytest

from finstack_quant.attribution import attribute_pnl
from finstack_quant.calibration import (
    CalibrationConfig,
    CalibrationEnvelope,
    CalibrationPlan,
    CalibrationStep,
    CdsQuote,
    CdsTrancheQuote,
    FxSpotDatum,
    InflationQuote,
    PriceDatum,
    RateQuote,
    VolQuote,
    XccyQuote,
    calibrate,
    dry_run,
    validate_calibration,
)
from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.covenants import CovenantTemplate, covenant_template, evaluate_engine, validate_covenant_engine_json
from finstack_quant.features import PanelTransformSpec, transform_panel
from finstack_quant.margin import CsaSpec, OtcMarginSpec
from finstack_quant.models import HestonParams, bs_price, heston_price
from finstack_quant.models.factor import FactorCovarianceMatrix, FactorModelConfig
from finstack_quant.models.factor.risk import decompose_factors, position_risk_report
from finstack_quant.portfolio import (
    DurationCellTable,
    FactorModel,
    MetricExpr,
    Objective,
    PerPositionMetric,
    Portfolio,
    PortfolioCashflows,
    PortfolioOptimizationSpec,
    PortfolioValuation,
    SensitivityMatrix,
    aggregate_full_cashflows,
    aggregate_metrics,
    allocate_weights,
    brinson_fachler,
    cell_returns_from_reference,
    excess_returns,
    net_in_currency_by_date,
    replay_portfolio,
    twrr_linked,
    value_portfolio,
)
from finstack_quant.scenarios import (
    HierarchyTarget,
    OperationSpec,
    ScenarioSpec,
    apply_scenario,
    validate_scenario_spec,
)
from finstack_quant.statements import (
    CheckReport,
    CheckSuiteSpec,
    Evaluator,
    FinancialModelSpec,
    ModelBuilder,
    MonteCarloConfig,
    RollupSpec,
    StatementResult,
)
from finstack_quant.statements_analytics import (
    CompanyMetrics,
    CreditMapping,
    PeerSet,
    ScenarioSet,
    ScoringDimension,
    SensitivityConfig,
    SensitivityResult,
    TerminalValueSpec,
    ThreeStatementMapping,
    VarianceConfig,
    capital_structure_summary,
    dcf_sensitivity,
    evaluate_dcf,
    evaluate_scenario_set,
    generate_tornado_entries,
    relative_value_report,
    render_check_report_text,
    run_checks,
    run_credit_underwriting_checks,
    run_sensitivity,
    run_three_statement_checks,
    run_variance,
    score_relative_value,
)
from finstack_quant.valuations.instruments import (
    AssetPool,
    Bond,
    CallPutSchedule,
    CdsIndex,
    CdsIndexConstituent,
    CdsOption,
    ConversionSpec,
    ConvertibleBond,
    HedgeSwap,
    InterestRateSwap,
    MarketHistory,
    MetricPricingOverrides,
    PremiumLegSpec,
    ProtectionLegSpec,
    RevolvingCredit,
    StructuredCredit,
    TermLoan,
    calculate_var,
    price_instrument,
    tranche_metrics,
    validate_instrument_json,
)

AS_OF = dt.date(2025, 1, 15)
QUARTERS = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"]

# Text that is not JSON, then valid JSON of four wrong shapes.
MALFORMED: Mapping[str, str] = {
    "not-json": "{",
    "empty-object": "{}",
    "empty-array": "[]",
    "number": "1",
    "unknown-field": '{"__probe__": 1}',
}


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _market() -> MarketContext:
    return MarketContext().insert(DiscountCurve.flat("USD-OIS", AS_OF, 0.04, "act_365f"))


def _bond(bond_id: str = "B1") -> Bond:
    return Bond.fixed(
        bond_id, Money(1_000_000.0, "USD"), 0.05, dt.date(2024, 1, 15), dt.date(2034, 1, 15), "none", "USD-OIS"
    )


def _built_bond(setter: str, value: Any) -> Bond:
    """A complete bond from ``Bond.builder()`` with one setter given ``value``."""
    builder = (
        Bond
        .builder()
        .id("B1")
        .notional(Money(1_000_000.0, "USD"))
        .issue_date(dt.date(2024, 1, 15))
        .maturity(dt.date(2034, 1, 15))
        .discount_curve_id("USD-OIS")
        .quote_compounding("semi_annual")
    )
    if setter != "cashflow_spec":
        builder = builder.cashflow_spec(_bond().cashflow_spec)
    return getattr(builder, setter)(value).build()


def _cashflow_spec_text() -> str:
    spec = _bond().cashflow_spec
    spec["fixed"]["rate"] = "0.0625"
    return json.dumps(spec)


def _portfolio() -> Portfolio:
    return (
        Portfolio
        .builder("book", "USD", AS_OF)
        .entity("ACME")
        .position("P1", _bond(), 1.0, entity_id="ACME", unit="face_value")
        .tag("desk", "rates")
        .build()
    )


def _portfolio_text_with_unknown_entity() -> str:
    """A spec that parses and then fails to build: the two stages map errors separately."""
    spec = json.loads(_portfolio().to_json())
    spec["positions"][0]["entity_id"] = "NOPE"
    return json.dumps(spec)


def _scenario() -> ScenarioSpec:
    return ScenarioSpec("up25", [OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)])


def _hierarchy_target() -> HierarchyTarget:
    tag_filter = {"predicates": [{"equals": {"key": "sector", "value": "financials"}}, {"exists": {"key": "region"}}]}
    return HierarchyTarget(["Credit", "US"], tag_filter)


def _model() -> FinancialModelSpec:
    builder = ModelBuilder("borrower")
    builder.periods("2025Q1..Q4", None)
    builder.value("ebitda", list(zip(QUARTERS, [100_000.0, 110_000.0, 120_000.0, 130_000.0], strict=True)))
    builder.value("cash", [(quarter, 150_000.0) for quarter in QUARTERS])
    builder.add_debt("SR", _bond("SR"))
    return builder.build()


def _results() -> StatementResult:
    return Evaluator().evaluate(_model())


def _results_text_with_schema_version(version: int) -> str:
    wire = json.loads(_results().to_json())
    wire["schema_version"] = version
    return json.dumps(wire)


def _capital_structure(results: Any, model: Any) -> Any:
    return capital_structure_summary(results, model, {"period": "2025Q4", "ebitda_node": "ebitda", "cash_node": "cash"})


def _company(name: str, **metrics: float) -> CompanyMetrics:
    return CompanyMetrics(name, metrics, meta={"gics_sector": "energy"})


def _relative_value(subject: Any) -> Any:
    universe = [
        _company("P1", leverage=1.0, oas_bp=100.0, enterprise_value=800.0, ebitda=100.0),
        _company("P2", leverage=2.0, oas_bp=200.0, enterprise_value=900.0, ebitda=100.0),
        _company("P3", leverage=3.0, oas_bp=310.0, enterprise_value=1200.0, ebitda=100.0),
        _company("P4", leverage=4.0, oas_bp=380.0, enterprise_value=500.0),
    ]
    config = {
        "filter": {
            "gics_sectors": ["energy"],
            "gics_industries": [],
            "countries": [],
            "market_cap_min": None,
            "market_cap_max": None,
            "ratings": [],
            "required_tags": [],
            "excluded_tags": [],
            "selectors": [],
        },
        "period_basis": "ltm",
        "dimensions": [
            {
                "label": "Spread vs Leverage",
                "y_extractor": {"named": "oas_bp"},
                "x_extractor": {"named": "leverage"},
                "weight": 0.7,
            },
            {
                "label": "EV/EBITDA",
                "y_extractor": {"multiple": "ev_ebitda"},
                "weight": 0.3,
                "direction": "higher_is_rich",
            },
        ],
        "multiples": ["ev_ebitda"],
    }
    return relative_value_report(subject, universe, config)


def _calibration_envelope(quote_set: str = "shared") -> str:
    """A one-deposit discount calibration; any ``quote_set`` but ``"shared"`` is undefined."""
    return json.dumps({
        "schema": "finstack_quant.calibration/1",
        "plan": {
            "id": "plan",
            "description": None,
            "quote_sets": {"shared": ["D"]},
            "steps": [
                {
                    "id": "USD-OIS",
                    "quote_set": quote_set,
                    "kind": "discount",
                    "curve_id": "USD-OIS",
                    "currency": "USD",
                    "base_date": "2026-05-08",
                }
            ],
            "settings": {},
        },
        "market_data": [
            {
                "kind": "rate_quote",
                "type": "deposit",
                "id": "D",
                "index": "USD-Deposit",
                "pillar": {"tenor": {"count": 1, "unit": "years"}},
                "rate": 0.03,
            }
        ],
    })


def _market_history_text() -> str:
    def shift(size: float) -> list[dict[str, Any]]:
        factor = {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0, "grid_years": [3.0, 5.0, 7.0]}
        return [{"factor": factor, "shift": size}]

    scenarios = [
        {"date": "2024-01-12", "shifts": shift(0.0010)},
        {"date": "2024-01-11", "shifts": shift(-0.0005)},
        {"date": "2024-01-10", "shifts": shift(0.0020)},
    ]
    return MarketHistory(AS_OF, 3, scenarios).to_json()


HESTON_CONTRACT = {
    "spot": 100.0,
    "strike": 100.0,
    "rate": 0.05,
    "dividend_yield": 0.02,
    "time_to_expiry": 1.0,
    "option_type": "call",
}
HESTON_PARAMS = {"v0": 0.04, "kappa": 2.0, "theta": 0.04, "vol_of_vol": 0.3, "rho": -0.7}
OPTION_MARKET = {
    "spot": 100.0,
    "strike": 110.0,
    "rate": 0.03,
    "dividend_yield": 0.01,
    "volatility": 0.25,
    "time_to_expiry": 0.75,
    "option_type": "call",
}
BRINSON_SECTORS = [
    {
        "sector": "TECH",
        "portfolio_weight": 0.60,
        "benchmark_weight": 0.40,
        "portfolio_return": 0.08,
        "benchmark_return": 0.06,
    },
    {
        "sector": "ENERGY",
        "portfolio_weight": 0.40,
        "benchmark_weight": 0.60,
        "portfolio_return": 0.01,
        "benchmark_return": 0.03,
    },
]
# The only tranche of `StructuredCredit.example()`.
TRANCHE_ID = "CLONOTES-A"
# The text form `net_in_currency_by_date` documents beside the full ladder.
BY_DATE_LADDER = '{"by_date":{"2025-01-15":{"USD":{"notional":{"amount":"-100","currency":"USD"}}}}}'


# ---------------------------------------------------------------------------
# Fixtures for the classes found by reading the bindings, not the stubs
# ---------------------------------------------------------------------------


# Value nodes of `_analysis_model`: the lines the built-in check suites map.
ANALYSIS_VALUES = {
    "revenue": (100.0, 110.0),
    "cost": (60.0, 65.0),
    "cash": (10.0, 15.0),
    "assets": (100.0, 105.0),
    "liabilities": (60.0, 60.0),
    "equity": (40.0, 45.0),
    "retained_earnings": (5.0, 10.0),
    "net_income": (5.0, 5.0),
    "total_debt": (60.0, 60.0),
    "ebitda": (20.0, 22.0),
    "interest_expense": (2.0, 2.0),
}


def _analysis_model(revenue_q1: float = 100.0) -> FinancialModelSpec:
    """Two quarters of `ANALYSIS_VALUES` and one computed line, ``profit``."""
    builder = ModelBuilder("analysis")
    builder.periods("2025Q1..Q2", "2025Q1")
    for node, (first, second) in ANALYSIS_VALUES.items():
        builder.value(node, [("2025Q1", revenue_q1 if node == "revenue" else first), ("2025Q2", second)])
    builder.compute("profit", "revenue - cost")
    return builder.build()


def _sensitivity_config() -> SensitivityConfig:
    return SensitivityConfig("diagonal", [("revenue", "2025Q2", 110.0, [100.0, 110.0, 120.0])], ["profit"])


def _variance(config: Any) -> Any:
    return run_variance(Evaluator().evaluate(_analysis_model()), Evaluator().evaluate(_analysis_model(105.0)), config)


CHECK_SUITE = {
    "name": "s",
    "builtin_checks": [],
    "formula_checks": [
        {
            "id": "pos",
            "name": "positive",
            "category": "internal_consistency",
            "severity": "error",
            "mode": "predicate",
            "scope": "all_periods",
            "formula": "revenue > 100",
            "message_template": "bad {period}",
        }
    ],
}
SCORING_DIMENSION = {
    "label": "Spread vs Leverage",
    "y_extractor": {"named": "oas_bp"},
    "x_extractor": {"named": "leverage"},
    "weight": 1.0,
}


def _peer_set() -> dict[str, Any]:
    peers = [("P1", 1.0, 100.0), ("P2", 2.0, 210.0), ("P3", 3.0, 300.0)]
    return {
        "subject": json.loads(_company("SUBJ", leverage=2.0, oas_bp=250.0).to_json()),
        "peers": [json.loads(_company(name, leverage=lev, oas_bp=oas).to_json()) for name, lev, oas in peers],
        "period_basis": "ltm",
    }


def _dcf_model() -> FinancialModelSpec:
    builder = ModelBuilder("dcf")
    builder.periods("2025..2026", None)
    builder.value_money("ufcf", {"2025": Money(100.0, "USD"), "2026": Money(110.0, "USD")})
    builder.with_meta("currency", "USD")
    return builder.build()


DCF_INPUTS = {
    "wacc": 0.10,
    "terminal_value": {"type": "gordon_growth", "stable_growth_rate": 0.02},
    "net_debt_override": 0.0,
}


def _calibration_plan(settings: Any = None) -> CalibrationPlan:
    step = CalibrationStep.discount("USD-OIS", {"currency": "USD", "base_date": "2026-05-08"}, quote_set="shared")
    return CalibrationPlan([step], quote_sets={"shared": ["D"]}, settings=settings)


def _envelope_with_datum(datum: Any) -> CalibrationEnvelope:
    return CalibrationEnvelope(_calibration_plan(), market_data=[datum])


def _datum_text(make: Callable[[], Any]) -> str:
    """A datum as an envelope writes it and as ``market_data`` reads it: tagged with ``kind``."""
    return json.dumps(_envelope_with_datum(make()).market_data[0])


def _prior_curve_text() -> str:
    """A calibrated discount curve as a ``prior_market`` object: tagged with ``kind``."""
    curve = json.loads(calibrate(_calibration_envelope()).market_json)["curves"][0]
    del curve["type"]
    return json.dumps({"kind": "discount_curve", **curve})


# Each calibration datum class with a valid instance.
MARKET_DATUMS: dict[str, tuple[Any, Callable[[], Any]]] = {
    "rate_quote": (RateQuote, lambda: RateQuote.deposit("D", "USD-Deposit", "1Y", 0.03)),
    "cds_quote": (CdsQuote, lambda: CdsQuote.par_spread("c5y", "ACME", "USD", "xr14", "5Y", 125.0, 0.4)),
    "vol_quote": (VolQuote, lambda: VolQuote.option_vol("v1", "SPX", "2026-01-15", 100.0, 0.2)),
    "inflation_quote": (
        InflationQuote,
        lambda: InflationQuote.inflation_swap("zc5y", "2031-05-08", 0.025, "USA-CPI-U", "USD"),
    ),
    "xccy_quote": (XccyQuote, lambda: XccyQuote("x5y", "EUR-USD", "5Y", -12.5, spot_fx=1.1)),
    "cds_tranche_quote": (
        CdsTrancheQuote,
        lambda: CdsTrancheQuote({
            "id": "IG-3-7",
            "index": "CDX.NA.IG",
            "series": 42,
            "attachment": 0.03,
            "detachment": 0.07,
            "maturity": "2031-06-20",
            "upfront_pct": 0.01,
            "coupon_bp": 100.0,
            "convention": {"currency": "USD", "doc_clause": "isda_na"},
        }),
    ),
    "fx_spot_datum": (FxSpotDatum, lambda: FxSpotDatum("EURUSD", "EUR", "USD", 1.1)),
    "price_datum": (PriceDatum, lambda: PriceDatum("AAPL", 187.5)),
}

PANEL_TRANSFORM = {
    "values": [10.0, 12.0, 20.0, 21.0],
    "time_key": ["1", "2", "1", "2"],
    "operations": [{"name": "rank", "family": "cross_sectional", "op": "rank"}],
}
POSITION_RISK_REQUEST = {
    "position_ids": ["A", "B"],
    "source": {
        "kind": "parametric",
        "weights": [100.0, 200.0],
        "covariance": [[0.04, 0.01], [0.01, 0.09]],
        "compute_incremental": True,
    },
    "confidence": 0.975,
    "budget": {"targets": [0.4, 0.6], "utilization_threshold": 1.2},
}
FACTOR_COVARIANCE = {"factor_ids": ["F", "G"], "n": 2, "data": [0.04, 0.01, 0.01, 0.09]}
FACTOR_MODEL = {
    "factors": [
        {
            "id": "usd_rates",
            "factor_type": "rates",
            "market_mapping": {"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}},
            "description": "Parallel USD rates shift",
        }
    ],
    "covariance": {"factor_ids": ["usd_rates"], "n": 1, "data": [0.0001]},
    "matching": {"mapping_table": [{"dependency_filter": {}, "attribute_filter": {}, "factor_id": "usd_rates"}]},
    "risk_measure": "variance",
}
WEIGHT_ALLOCATION = {
    "scheme": "inverse_volatility",
    "total_capital": 1_000_000.0,
    "strategies": [
        {"id": "S1", "fixed_weight": None, "returns": [0.01, -0.02, 0.015, 0.005], "risk_budget": None},
        {"id": "S2", "fixed_weight": None, "returns": [0.002, -0.001, 0.003, 0.001], "risk_budget": None},
    ],
    "covariance": None,
}
EXCESS_RETURN_POSITIONS = [
    {"id": "B1", "weight": 0.5, "duration": 0.5, "total_return": 0.03},
    {"id": "B2", "weight": 0.5, "duration": 1.5, "total_return": 0.025},
]


def _swap_with_margin(margin_spec: Any) -> InterestRateSwap:
    trade = InterestRateSwap.example()
    return (
        InterestRateSwap
        .builder()
        .id("IRS")
        .notional(trade.notional)
        .fixed_leg(trade.fixed_leg)
        .float_leg(trade.float_leg)
        .side(trade.to_dict()["side"])
        .margin_spec(margin_spec)
        .build()
    )


def _sensitivities() -> SensitivityMatrix:
    wire = {
        "base_currency": "EUR",
        "position_ids": ["A", "B"],
        "factor_ids": ["F", "G"],
        "data": [[2.0, 1.0], [3.0, -1.0]],
    }
    return SensitivityMatrix.from_json(json.dumps(wire))


def _attribution_inputs_text() -> str:
    next_day = AS_OF + dt.timedelta(days=1)
    later = MarketContext().insert(DiscountCurve.flat("USD-OIS", next_day, 0.042, "act_365f"))
    return json.dumps({
        "market_t0": json.loads(_market().to_json()),
        "market_t1": json.loads(later.to_json()),
        "as_of_t0": AS_OF.isoformat(),
        "as_of_t1": next_day.isoformat(),
        "method": "parallel",
    })


def _cell_table_text() -> str:
    reference = [
        {"duration": 0.5, "total_return": 0.01},
        {"duration": 1.5, "total_return": 0.02},
        {"duration": 2.5, "total_return": 0.03},
    ]
    return cell_returns_from_reference(reference, "UST", {"width": 1.0}).to_json()


def _factor_stress(config: Any) -> Any:
    return FactorModel.from_config(config).factor_stress(_portfolio(), _market(), AS_OF, [("usd_rates", 1.0)])


def _snapshots_text() -> str:
    next_day = AS_OF + dt.timedelta(days=1)
    later = MarketContext().insert(DiscountCurve.flat("USD-OIS", next_day, 0.041, "act_365f"))
    return json.dumps([
        {"date": AS_OF.isoformat(), "market": json.loads(_market().to_json())},
        {"date": next_day.isoformat(), "market": json.loads(later.to_json())},
    ])


def _optimization_spec(portfolio: Any) -> PortfolioOptimizationSpec:
    objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    return PortfolioOptimizationSpec.new(portfolio, objective)


def _asset_pool() -> AssetPool:
    return AssetPool("P", "clo", "USD")


def _deal_with_hedges(hedges: Any) -> StructuredCredit:
    example = StructuredCredit.example()
    return (
        StructuredCredit
        .builder()
        .id("CLO-HEDGED")
        .deal_type("clo")
        .pool(example.pool)
        .tranches(example.tranches)
        .closing_date(dt.date(2024, 1, 1))
        .first_payment_date(dt.date(2024, 4, 1))
        .maturity(dt.date(2034, 1, 1))
        .frequency(Tenor.quarterly())
        .discount_curve_id("USD-OIS")
        .hedge_swaps(hedges)
        .build()
    )


def _index_with_constituents(constituents: Any) -> CdsIndex:
    premium = PremiumLegSpec(
        start=dt.date(2024, 3, 20),
        end=dt.date(2029, 6, 20),
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        coupon_bp="100",
        discount_curve_id="USD-OIS",
        date_regime="standard",
    )
    return (
        CdsIndex
        .builder()
        .include_valuation_date_cashflows(False)
        .id("CDX-CONS")
        .index_name("CDX.NA.IG")
        .series(42)
        .version(1)
        .notional(Money(10_000_000.0, "USD"))
        .index_factor(1.0)
        .side("pay")
        .convention("isda_na")
        .premium_leg(premium)
        .protection_leg(ProtectionLegSpec("CORP-HAZARD", 0.4, 3))
        .pricing("constituents")
        .constituents(constituents)
        .build()
    )


def _constituents_text() -> str:
    row = json.loads(CdsIndexConstituent("ACME-CORP", 0.4, "CORP-HAZARD", 0.5).to_json())
    return json.dumps([row, row])


def _covenant_reports(metrics: Any) -> dict[str, str]:
    template = CovenantTemplate.lbo_standard(6.0, 2.0, 1.1, 50_000_000.0)
    engine = {
        "specs": [json.loads(covenant_template(template)[0].to_json())],
        "breach_history": [],
        "windows": [],
        "waivers": [],
    }
    reports = evaluate_engine(validate_covenant_engine_json(json.dumps(engine)), metrics, "2026-03-31")
    return {key: report.to_json() for key, report in reports.items()}


def _validation_report(scenario: Any) -> tuple[Any, ...]:
    report = validate_scenario_spec(scenario)
    return (report.valid, report.errors, report.warnings, report.operation_count)


# ---------------------------------------------------------------------------
# The table
# ---------------------------------------------------------------------------


def _to_json(result: Any) -> str:
    return result.to_json()


def _itself(result: Any) -> Any:
    return result


def _json_without_eval_time(result: Any) -> Any:
    """Parsed ``to_json()`` without ``eval_time_ms``, the wall-clock time an evaluation took."""

    def drop(node: dict[str, Any]) -> dict[str, Any]:
        return {key: value for key, value in node.items() if key != "eval_time_ms"}

    return json.loads(result.to_json(), object_hook=drop)


@dataclass(frozen=True)
class Case:
    """One parameter that reads JSON text today, and the loader that replaces the text."""

    id: str
    # The function under test, as a function of the one parameter under test.
    call: Callable[[Any], Any]
    # What a holder of JSON text calls first once the text form is gone.
    loader: Callable[[str], Any]
    # Valid JSON text for the parameter.
    text: Callable[[], str]
    # Exact comparable form of a result. The one run-time field in any result
    # compared here is `eval_time_ms`, which `_json_without_eval_time` removes.
    stable: Callable[[Any], Any] = _to_json
    malformed: Mapping[str, str] = field(default_factory=lambda: MALFORMED)
    # Labels in `malformed` that are a valid (empty) value for this parameter on
    # both routes; every other label must raise on both.
    accepted: frozenset[str] = frozenset()
    # `None` while the text route exists. Once it is removed: the exception the
    # function raises for any `str`.
    rejects_text: type[Exception] | None = None
    # Where the two routes are known to differ: a label of `malformed`, or
    # `"valid"` for the valid text, to what each route does, as
    # `(via the loader, via the text)`. Each is `"accepted"` or the name of the
    # exception type raised. The owner ruled on every entry on 2026-11-26.
    differs: Mapping[str, tuple[str, str]] = field(default_factory=dict)
    # The ruling that covers `differs`.
    ruling: str = ""


# The owner's rulings of 2026-11-26 on the differences the sweep found.
RULING_A = "A: `net_in_currency_by_date` keeps its text form; a by-date ladder has no loader and no dict form."
RULING_B = (
    "B: accepted. A serde-backed loader reads `[]` as an all-default value where the text arm required an object."
)
RULING_C = "C: accepted. After `json.loads`, JSON that is not of the expected Python type raises `TypeError`."
RULING_D = "D: to be fixed. A mapping is iterated as an empty list today; the slice that owns the site rejects it."
RULING_E = "E: `market_data` text is the `kind`-tagged datum, which the class loader does not read; `json.loads` is its loader."
RULING_G = "G: accepted. For text that is not JSON, `json.loads` raises `JSONDecodeError` where the text arm raised `PortfolioError`."
RULING_I = "I: accepted. Text deferred building the portfolio; `Portfolio.from_spec` rejects a spec that does not build at once."
RULING_J = (
    "J: `validate_scenario_spec` keeps its text form; it reports on a spec that `ScenarioSpec.from_json` rejects."
)

CASES = [
    # --- One case per typed class: each class has its own loader. ---
    Case(
        "market_context",
        lambda market: price_instrument(_bond(), market, AS_OF, metrics=["dv01", "ytm"]),
        MarketContext.from_json,
        lambda: _market().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "financial_model_spec",
        lambda model: _capital_structure(_results(), model),
        FinancialModelSpec.from_json,
        lambda: _model().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "financial_model_spec_in_scenario",
        lambda model: apply_scenario(_scenario(), _market(), AS_OF, model=model),
        FinancialModelSpec.from_json,
        lambda: _model().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "statement_result",
        lambda results: _capital_structure(results, _model()),
        StatementResult.from_json,
        lambda: _results().to_json(),
        malformed={**MALFORMED, "unsupported-schema-version": _results_text_with_schema_version(999)},
        rejects_text=TypeError,
    ),
    Case(
        "portfolio",
        lambda portfolio: value_portfolio(portfolio, _market()),
        Portfolio.from_spec,
        lambda: _portfolio().to_json(),
        malformed={**MALFORMED, "unknown-entity": _portfolio_text_with_unknown_entity()},
        rejects_text=TypeError,
    ),
    Case(
        "portfolio_valuation",
        lambda valuation: aggregate_metrics(valuation, _market()),
        PortfolioValuation.from_json,
        lambda: value_portfolio(_portfolio(), _market()).to_json(),
        rejects_text=TypeError,
    ),
    # A `str` given to `net_in_currency_by_date` goes to the Rust function
    # `net_in_currency_by_date_json`, which also reads a bare by-date ladder; it
    # does not go through the `PortfolioCashflows` loader.
    Case(
        "portfolio_cashflows",
        lambda cashflows: net_in_currency_by_date(cashflows, "USD"),
        PortfolioCashflows.from_json,
        lambda: aggregate_full_cashflows(_portfolio(), _market()).to_json(),
        stable=_itself,
        differs={
            "not-json": ("ValueError", "PortfolioError"),
            "empty-object": ("ValueError", "accepted"),
            "empty-array": ("ValueError", "PortfolioError"),
            "number": ("ValueError", "PortfolioError"),
            "unknown-field": ("ValueError", "PortfolioError"),
        },
        ruling=RULING_A,
    ),
    Case(
        "portfolio_cashflows_by_date_ladder",
        lambda cashflows: net_in_currency_by_date(cashflows, "USD"),
        PortfolioCashflows.from_json,
        lambda: BY_DATE_LADDER,
        stable=_itself,
        malformed={},
        differs={"valid": ("ValueError", "accepted")},
        ruling=RULING_A,
    ),
    Case(
        "scenario_spec",
        lambda scenario: apply_scenario(scenario, _market(), AS_OF),
        ScenarioSpec.from_json,
        lambda: _scenario().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "hierarchy_target",
        lambda target: OperationSpec.hierarchy_curve_parallel_bp("par_cds", target, 50.0),
        HierarchyTarget.from_json,
        lambda: _hierarchy_target().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "structured_credit",
        lambda deal: tranche_metrics(deal, TRANCHE_ID, _market(), AS_OF),
        StructuredCredit.from_json,
        lambda: StructuredCredit.example().to_json(),
    ),
    Case(
        "finstack_config",
        lambda config: apply_scenario(_scenario(), _market(), AS_OF, config=config),
        FinstackConfig.from_json,
        lambda: FinstackConfig().with_output_scale("USD", 4).to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "calibration_envelope_calibrate",
        calibrate,
        CalibrationEnvelope.from_json,
        _calibration_envelope,
        malformed={**MALFORMED, "undefined-quote-set": _calibration_envelope("missing")},
    ),
    Case(
        "calibration_envelope_validate_calibration",
        validate_calibration,
        CalibrationEnvelope.from_json,
        _calibration_envelope,
        malformed={**MALFORMED, "undefined-quote-set": _calibration_envelope("missing")},
    ),
    Case(
        "calibration_envelope_dry_run",
        dry_run,
        CalibrationEnvelope.from_json,
        _calibration_envelope,
        malformed={**MALFORMED, "undefined-quote-set": _calibration_envelope("missing")},
        # `dry_run` reports an undefined quote set in its result; it does not raise.
        accepted=frozenset({"undefined-quote-set"}),
    ),
    Case(
        "metric_pricing_overrides_builder",
        lambda overrides: _built_bond("metric_pricing_overrides", overrides),
        MetricPricingOverrides.from_json,
        lambda: MetricPricingOverrides(
            theta_period="1W", bond_risk_basis="callable_oas", bump_config={"rate_bump_bp": 2.0}
        ).to_json(),
        # Every field is optional, and serde reads a struct from an array too.
        accepted=frozenset({"empty-object", "empty-array"}),
    ),
    Case(
        "metric_pricing_overrides_validate",
        lambda overrides: validate_instrument_json(_bond().to_json(), metric_pricing_overrides=overrides),
        MetricPricingOverrides.from_json,
        lambda: MetricPricingOverrides(
            theta_period="1W", bond_risk_basis="callable_oas", bump_config={"rate_bump_bp": 2.0}
        ).to_json(),
        stable=_itself,
        accepted=frozenset({"empty-object"}),
        differs={"empty-array": ("accepted", "ValueError")},
        ruling=RULING_B,
    ),
    Case(
        "heston_params",
        lambda heston: heston_price(HESTON_CONTRACT, heston),
        HestonParams.from_json,
        lambda: json.dumps(HESTON_PARAMS),
        stable=_itself,
    ),
    Case(
        "market_history",
        lambda history: calculate_var([_bond()], _market(), history, AS_OF),
        MarketHistory.from_json,
        _market_history_text,
    ),
    Case(
        "company_metrics",
        _relative_value,
        CompanyMetrics.from_json,
        lambda: _company("SUBJ", leverage=2.5, oas_bp=300.0, enterprise_value=1000.0, ebitda=100.0).to_json(),
    ),
    Case(
        "typed_instrument",
        lambda bond: price_instrument(bond, _market(), AS_OF, metrics=["dv01", "ytm"]),
        Bond.from_json,
        lambda: _bond().to_json(),
    ),
    # --- Parameters with no typed class: the loader is `json.loads`. ---
    Case(
        "dict_builder_struct",
        lambda overrides: _built_bond("instrument_pricing_overrides", overrides),
        json.loads,
        lambda: json.dumps({"market_quotes": {"quoted_clean_price_pct": 99.5}}),
        # Every field is optional, and serde reads a struct from an array too.
        accepted=frozenset({"empty-object", "empty-array"}),
    ),
    Case(
        "dict_builder_enum",
        lambda spec: _built_bond("cashflow_spec", spec),
        json.loads,
        _cashflow_spec_text,
    ),
    Case(
        "dict_instrument_envelope",
        lambda envelope: price_instrument(envelope, _market(), AS_OF, metrics=["dv01", "ytm"]),
        json.loads,
        lambda: _bond().to_json(),
        differs={"empty-array": ("TypeError", "ValueError"), "number": ("TypeError", "ValueError")},
        ruling=RULING_C,
    ),
    Case(
        "dict_records_returns",
        lambda returns: twrr_linked(returns, 1.0),
        json.loads,
        lambda: json.dumps([0.01, -0.02, 0.015]),
        # No return periods is a valid input.
        accepted=frozenset({"empty-array"}),
        differs={
            "empty-object": ("accepted", "ValueError"),
            "number": ("TypeError", "ValueError"),
            "unknown-field": ("TypeError", "ValueError"),
        },
        ruling=f"{RULING_D} {RULING_C}",
    ),
    Case(
        "dict_records_sectors",
        brinson_fachler,
        json.loads,
        lambda: json.dumps(BRINSON_SECTORS),
    ),
    Case(
        "dict_params_object",
        bs_price,
        json.loads,
        lambda: json.dumps(OPTION_MARKET),
        stable=_itself,
    ),
    # --- Statements: typed classes with `from_json`. Most stubs annotate these `Any`. ---
    Case(
        "sensitivity_config",
        lambda config: run_sensitivity(_analysis_model(), config),
        SensitivityConfig.from_json,
        lambda: _sensitivity_config().to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "sensitivity_result",
        lambda result: [entry.to_json() for entry in generate_tornado_entries(result, "profit", "2025Q2")],
        SensitivityResult.from_json,
        lambda: run_sensitivity(_analysis_model(), _sensitivity_config()).to_json(),
        stable=_itself,
        rejects_text=TypeError,
    ),
    Case(
        "variance_config",
        _variance,
        VarianceConfig.from_json,
        lambda: VarianceConfig("base", "comparison", ["profit"], ["2025Q1", "2025Q2"]).to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "scenario_set",
        lambda scenarios: evaluate_scenario_set(_analysis_model(), scenarios),
        ScenarioSet.from_json,
        lambda: ScenarioSet({"base": {}, "downside": {"revenue": 90.0}}, {"downside": "base"}).to_json(),
        stable=_json_without_eval_time,
        rejects_text=TypeError,
    ),
    Case(
        "monte_carlo_config",
        lambda config: Evaluator().evaluate_monte_carlo(_analysis_model(), config),
        MonteCarloConfig.from_json,
        lambda: MonteCarloConfig(4, 123, ["revenue"], [0.5]).to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "rollup_spec",
        lambda spec: Evaluator().evaluate(_analysis_model()).rollup(_analysis_model(), spec),
        RollupSpec.from_json,
        lambda: RollupSpec("semi_annual", dict.fromkeys([*ANALYSIS_VALUES, "profit"], "sum")).to_json(),
        rejects_text=TypeError,
    ),
    Case(
        "check_suite_spec",
        lambda suite: run_checks(_analysis_model(), suite),
        CheckSuiteSpec.from_json,
        lambda: json.dumps(CHECK_SUITE),
    ),
    Case(
        "check_report",
        render_check_report_text,
        CheckReport.from_json,
        lambda: run_checks(_analysis_model(), CHECK_SUITE).to_json(),
        stable=_itself,
    ),
    Case(
        "three_statement_mapping",
        lambda mapping: run_three_statement_checks(_analysis_model(), mapping),
        ThreeStatementMapping.from_json,
        lambda: ThreeStatementMapping(
            cash_node="cash",
            retained_earnings_node="retained_earnings",
            net_income_node="net_income",
            assets_nodes=["assets"],
            liabilities_nodes=["liabilities"],
            equity_nodes=["equity"],
        ).to_json(),
    ),
    Case(
        "credit_mapping",
        lambda mapping: run_credit_underwriting_checks(_analysis_model(), mapping),
        CreditMapping.from_json,
        lambda: CreditMapping(
            debt_node="total_debt", ebitda_node="ebitda", interest_expense_node="interest_expense"
        ).to_json(),
    ),
    Case(
        "peer_set",
        lambda peer_set: score_relative_value(peer_set, [SCORING_DIMENSION]),
        PeerSet.from_json,
        lambda: json.dumps(_peer_set()),
    ),
    Case(
        "scoring_dimension",
        lambda dimension: score_relative_value(_peer_set(), [dimension]),
        ScoringDimension.from_json,
        lambda: json.dumps(SCORING_DIMENSION),
    ),
    Case(
        "terminal_value_spec",
        lambda terminal_value: dcf_sensitivity(_dcf_model(), 0.10, terminal_value, "ufcf", 0.0),
        TerminalValueSpec.from_json,
        lambda: TerminalValueSpec.gordon_growth(0.02).to_json(),
    ),
    # --- Calibration. ---
    Case(
        "calibration_config",
        _calibration_plan,
        CalibrationConfig.from_json,
        lambda: json.dumps({"solver": {"max_iterations": 64}, "use_parallel": True}),
        # No override is a valid patch.
        accepted=frozenset({"empty-object"}),
        differs={"empty-array": ("accepted", "ValueError")},
        ruling=RULING_B,
    ),
    # `market_data` reads a datum as the envelope writes it, `{"kind": ..., ...}`,
    # from a `dict` or from text. No datum class reads that document: its own
    # `to_json()` carries no `kind`, and its `from_json` rejects one.
    *[
        Case(f"dict_market_datum_{name}", _envelope_with_datum, json.loads, lambda make=make: _datum_text(make))
        for name, (_, make) in MARKET_DATUMS.items()
    ],
    *[
        Case(
            f"market_datum_{name}_class_loader",
            _envelope_with_datum,
            datum.from_json,
            lambda make=make: _datum_text(make),
            malformed={},
            differs={"valid": ("ValueError", "accepted")},
            ruling=RULING_E,
        )
        for name, (datum, make) in MARKET_DATUMS.items()
    ],
    Case(
        "dict_prior_market_object",
        lambda prior: CalibrationEnvelope(_calibration_plan(), prior_market=[prior]),
        json.loads,
        _prior_curve_text,
    ),
    # --- Features, models, portfolio, valuations: typed classes with `from_json`. ---
    Case(
        "panel_transform_spec",
        transform_panel,
        PanelTransformSpec.from_json,
        lambda: json.dumps(PANEL_TRANSFORM),
    ),
    Case(
        "factor_covariance_matrix",
        lambda covariance: decompose_factors(_sensitivities(), covariance),
        FactorCovarianceMatrix.from_json,
        lambda: json.dumps(FACTOR_COVARIANCE),
    ),
    Case(
        "duration_cell_table",
        lambda table: excess_returns(EXCESS_RETURN_POSITIONS, table),
        DurationCellTable.from_json,
        _cell_table_text,
    ),
    Case(
        "factor_model_config",
        _factor_stress,
        FactorModelConfig.from_json,
        lambda: json.dumps(FACTOR_MODEL),
    ),
    # The text arm of `PortfolioOptimizationSpec.new` keeps the parsed spec; it does
    # not build the portfolio, which `Portfolio.from_spec` does.
    Case(
        "portfolio_in_optimization_spec",
        _optimization_spec,
        Portfolio.from_spec,
        lambda: _portfolio().to_json(),
        malformed={**MALFORMED, "unknown-entity": _portfolio_text_with_unknown_entity()},
        differs={"unknown-entity": ("KeyError", "accepted")},
        ruling=RULING_I,
        rejects_text=TypeError,
    ),
    Case(
        "conversion_spec",
        lambda conversion: ConvertibleBond.example().to_builder().conversion(conversion).build(),
        ConversionSpec.from_json,
        lambda: ConversionSpec(
            ratio=20.0, policy="voluntary", anti_dilution="full_ratchet", dividend_adjustment="none"
        ).to_json(),
    ),
    Case(
        "call_put_schedule",
        lambda schedule: ConvertibleBond.example().to_builder().call_put(schedule).build(),
        CallPutSchedule.from_json,
        lambda: CallPutSchedule(
            calls=[{"start": "2026-03-15", "end": "2027-03-15", "price_pct_of_par": 101.0}]
        ).to_json(),
    ),
    # `validate_scenario_spec` reports on a spec that parses but is invalid; the
    # loader raises on the same text, so such a spec cannot be held as an object.
    Case(
        "scenario_spec_validator",
        _validation_report,
        ScenarioSpec.from_json,
        lambda: _scenario().to_json(),
        stable=_itself,
        malformed={**MALFORMED, "empty-id": '{"id": "", "operations": []}'},
        differs={"empty-id": ("ValueError", "accepted")},
        ruling=RULING_J,
    ),
    # --- Typed classes without a text loader: the parameter also takes a `dict`. ---
    Case(
        "dict_scoring_dimensions",
        lambda dimensions: score_relative_value(_peer_set(), dimensions),
        json.loads,
        lambda: json.dumps([SCORING_DIMENSION]),
    ),
    Case(
        "dict_dcf_inputs",
        lambda inputs: evaluate_dcf(_dcf_model(), inputs),
        json.loads,
        lambda: json.dumps(DCF_INPUTS),
    ),
    Case(
        "dict_dcf_options",
        lambda options: evaluate_dcf(_dcf_model(), DCF_INPUTS, options),
        json.loads,
        lambda: json.dumps({"mid_year_convention": True}),
        # Every option has a default, and serde reads a struct from an array too.
        accepted=frozenset({"empty-object", "empty-array"}),
    ),
    Case(
        "dict_otc_margin_spec",
        _swap_with_margin,
        json.loads,
        lambda: json.dumps(OtcMarginSpec.bilateral_simm(CsaSpec.usd_regulatory()).to_dict()),
    ),
    Case(
        "dict_option_contract_params",
        lambda contract: heston_price(contract, HESTON_PARAMS),
        json.loads,
        lambda: json.dumps(HESTON_CONTRACT),
        stable=_itself,
    ),
    Case(
        "dict_risk_measure",
        lambda measure: decompose_factors(_sensitivities(), FACTOR_COVARIANCE, measure),
        json.loads,
        lambda: json.dumps({"var": {"confidence": 0.99}}),
    ),
    Case(
        "dict_position_risk_request",
        position_risk_report,
        json.loads,
        lambda: json.dumps(POSITION_RISK_REQUEST),
    ),
    Case(
        "dict_attribution_inputs",
        lambda inputs: attribute_pnl(_bond(), inputs),
        json.loads,
        _attribution_inputs_text,
    ),
    Case(
        "dict_weight_allocation_spec",
        allocate_weights,
        json.loads,
        lambda: json.dumps(WEIGHT_ALLOCATION),
        differs={"not-json": ("JSONDecodeError", "PortfolioError")},
        ruling=RULING_G,
    ),
    Case(
        "dict_replay_config",
        lambda config: replay_portfolio(_portfolio(), json.loads(_snapshots_text()), config),
        json.loads,
        lambda: json.dumps({"mode": "pv_and_pnl"}),
    ),
    Case(
        "dict_replay_snapshots",
        lambda snapshots: replay_portfolio(_portfolio(), snapshots, {"mode": "pv_and_pnl"}),
        json.loads,
        _snapshots_text,
        differs={"not-json": ("JSONDecodeError", "PortfolioError"), "number": ("TypeError", "PortfolioError")},
        ruling=f"{RULING_G} {RULING_C}",
    ),
    Case(
        "dict_price_extras",
        lambda extras: price_instrument(_bond(), _market(), AS_OF, metrics=["dv01", "ytm"], extras=extras),
        json.loads,
        lambda: json.dumps({"instrument_pricing_overrides": {"market_quotes": {"quoted_clean_price_pct": 99.5}}}),
        # Every extra is optional, and serde reads a struct from an array too.
        accepted=frozenset({"empty-object", "empty-array"}),
    ),
    # Lists of rows read by `typed_list_from_py`.
    Case(
        "dict_list_pool_assets",
        lambda rows: _asset_pool().with_assets(rows),
        json.loads,
        lambda: json.dumps([asset.to_dict() for asset in StructuredCredit.example().pool.assets]),
        # No rows is a valid list.
        accepted=frozenset({"empty-array"}),
        differs={"empty-object": ("accepted", "ValueError")},
        ruling=RULING_D,
    ),
    Case(
        "dict_list_hedge_swaps",
        _deal_with_hedges,
        json.loads,
        lambda: json.dumps([HedgeSwap(InterestRateSwap.example(), notional={"tranche_par": TRANCHE_ID}).to_dict()]),
        # No rows is a valid list.
        accepted=frozenset({"empty-array"}),
        differs={"empty-object": ("accepted", "ValueError")},
        ruling=RULING_D,
    ),
    Case(
        "dict_list_index_constituents",
        _index_with_constituents,
        json.loads,
        _constituents_text,
    ),
    # Tagged enums read by `tagged_enum_from_py`: a bare label, a `dict`, or text
    # that starts with `{`.
    Case(
        "dict_tagged_enum_call_exercise",
        lambda policy: _asset_pool().with_instruments(bonds=[Bond.example()], call_exercise=policy),
        json.loads,
        lambda: json.dumps({"policy": "refinancing_incentive", "threshold_bp": 50.0}),
    ),
    Case(
        "dict_tagged_enum_reserve_destination",
        lambda destination: _asset_pool().with_reserve(
            Money(5_000_000.0, "USD"),
            reserve_account_rate=0.02,
            reserve_target=Money(6_000_000.0, "USD"),
            reserve_interest_destination=destination,
        ),
        json.loads,
        lambda: json.dumps({"kind": "tranche", "tranche_id": "EQ"}),
    ),
    Case(
        "dict_covenant_metrics",
        _covenant_reports,
        json.loads,
        lambda: json.dumps({"debt_to_ebitda": 4.0}),
        stable=_itself,
        differs={"empty-array": ("TypeError", "ValueError"), "number": ("TypeError", "ValueError")},
        ruling=RULING_C,
    ),
]


def _shape(outcome: tuple[Any, ...]) -> str:
    """``"accepted"``, or the name of the exception type an outcome raised."""
    return "accepted" if outcome[0] == "accepted" else outcome[1].__name__


def _outcome(thunk: Callable[[], Any]) -> tuple[Any, ...]:
    """``("accepted", value)``, or ``("raises", type, kind)`` where ``kind`` is the error's own code."""
    try:
        return ("accepted", thunk())
    except Exception as error:  # noqa: BLE001 - the exception type is the result under test
        return ("raises", type(error), getattr(error, "kind", None))


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("case", CASES, ids=lambda case: case.id)
def test_loader_matches_text_route(case: Case) -> None:
    text = case.text()
    if case.rejects_text is None and "valid" in case.differs:
        via_loader = _outcome(lambda: case.stable(case.call(case.loader(text))))
        via_text = _outcome(lambda: case.stable(case.call(text)))
        assert (_shape(via_loader), _shape(via_text)) == case.differs["valid"], case.ruling
        return
    via_loader = case.stable(case.call(case.loader(text)))
    if case.rejects_text is not None:
        with pytest.raises(case.rejects_text):
            case.call(text)
        return
    via_text = case.stable(case.call(text))
    # The premise of an exact comparison: one route repeats itself exactly.
    assert case.stable(case.call(text)) == via_text, "the text route does not repeat itself"
    assert via_loader == via_text


@pytest.mark.parametrize(
    ("case", "label"),
    [pytest.param(case, label, id=f"{case.id}-{label}") for case in CASES for label in case.malformed],
)
def test_malformed_text_raises_same_type(case: Case, label: str) -> None:
    text = case.malformed[label]
    if case.rejects_text is not None:
        with pytest.raises(case.rejects_text):
            case.call(text)
        return
    via_loader = _outcome(lambda: case.stable(case.call(case.loader(text))))
    via_text = _outcome(lambda: case.stable(case.call(text)))
    if label in case.differs:
        assert (_shape(via_loader), _shape(via_text)) == case.differs[label], case.ruling
        return
    if label in case.accepted:
        assert via_text[0] == "accepted", f"the text route rejects {text!r}"
        assert via_loader == via_text
        return
    assert via_text[0] == "raises", f"the text route accepts {text!r}"
    assert via_loader[0] == "raises", f"the loader route accepts {text!r}; the text route raises {via_text[1]}"
    if case.loader is json.loads and via_loader[1] is json.JSONDecodeError:
        # `json.loads` rejects text that is not JSON before the function sees it.
        # Its error is a `ValueError` subclass, so a handler written for the text
        # route's error still catches it; the two types are not equal.
        assert issubclass(via_loader[1], via_text[1])
        return
    assert via_loader == via_text


def test_every_known_difference_names_its_ruling() -> None:
    for case in CASES:
        assert bool(case.differs) == bool(case.ruling), case.id
        assert set(case.differs) <= {"valid", *case.malformed}, case.id


# Builder setters that parse a string as JSON text into an enum today. Once the
# text form is gone a string there is a label, so a string that is also valid
# JSON must not stay accepted with a different meaning. Each row: the class, the
# setter, one variant name of the enum, and the inputs accepted today (every
# other input raises `ValueError`). Only `AmortizationSpec` has a variant without
# data (`none`); for the other enums no bare string is a value.
ENUM_SETTERS: list[tuple[type, str, str, frozenset[str]]] = [
    (Bond, "cashflow_spec", "fixed", frozenset()),
    (ConvertibleBond, "cashflow_spec", "fixed", frozenset()),
    (TermLoan, "rate", "fixed", frozenset()),
    (TermLoan, "amortization", "none", frozenset({"none", '"none"'})),
    (TermLoan, "upfront_fee", "amount", frozenset()),
    (CdsOption, "strike", "spread", frozenset()),
    (RevolvingCredit, "rate", "fixed", frozenset()),
    (RevolvingCredit, "draw_repay_spec", "deterministic", frozenset()),
]


@pytest.mark.parametrize(
    ("instrument", "setter", "text", "accepted"),
    [
        pytest.param(instrument, setter, text, text in accepted, id=f"{instrument.__name__}Builder.{setter}-{text}")
        for instrument, setter, variant, accepted in ENUM_SETTERS
        for text in ("1", "true", "null", variant, f'"{variant}"')
    ],
)
def test_json_looking_strings_today(instrument: Any, setter: str, text: str, accepted: bool) -> None:
    outcome = _outcome(lambda: getattr(instrument.builder(), setter)(text))
    if accepted:
        assert outcome[0] == "accepted"
    else:
        assert outcome[:2] == ("raises", ValueError)
