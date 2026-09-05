"""A balanced borrower model in USD units, evaluated by native statements APIs.

The operating company starts with USD 110m debt before 2024Q1 and amortizes
USD 2.5m per quarter. It enters 2025 with USD 100m debt. The common analyst book
owns a USD 1m loan slice, not the borrower's entire liability. All statement
arithmetic remains inspectable as native formula nodes.

>>> borrower_model().has_node("balance_check")
True
"""

from __future__ import annotations

import json
from typing import Any

from finstack_quant.statements import FinancialModelSpec, ForecastSpec, ModelBuilder

from .synthetic import DEMO_PL_PERIODS, demo_pl_builder

PERIODS = DEMO_PL_PERIODS
ACTUAL_THROUGH = "2024Q4"


def borrower_builder() -> ModelBuilder:
    """Return a fresh builder with four actual and four forecast quarters.

    Returns:
        Native builder covering 2024Q1 through 2025Q4. Monetary nodes use USD
        currency units; rates, margins and covenant ratios are decimal values.
        Revenue and COGS grow 2% per forecast quarter and operating expense
        grows 1.5%. Debt accrues 7.5% annual simple interest on beginning debt.
        The three statements reconcile through debt, PPE and cash roll-forwards.
        EBITDA annualization is a current-quarter run rate, not trailing EBITDA.

    Raises:
        ValueError: If a native formula or forecast definition is invalid.

    >>> borrower_builder().build().has_node("net_leverage")
    True
    """
    b = demo_pl_builder(
        "analyst-borrower",
        periods=PERIODS,
        actual_through=ACTUAL_THROUGH,
        revenue=[20_000_000.0, 22_000_000.0, 23_000_000.0, 24_000_000.0],
        cogs=[12_000_000.0, 13_200_000.0, 13_800_000.0, 14_400_000.0],
        opex=[3_000_000.0, 3_200_000.0, 3_300_000.0, 3_400_000.0],
        margins=False,
        values={
            "depreciation": [1_000_000.0] * 4,
            "capex": [1_200_000.0] * 4,
            "amortization": [2_500_000.0] * 4,
            "change_nwc": [100_000.0, 200_000.0, 150_000.0, 150_000.0],
            "interest_rate": [0.075] * 4,
            "tax_rate": [0.25] * 4,
        },
    )
    for node, growth in (("revenue", 0.02), ("cogs", 0.02), ("opex", 0.015), ("change_nwc", 0.02)):
        b.forecast(node, ForecastSpec.growth(growth))
    for node in ("depreciation", "capex", "amortization", "interest_rate", "tax_rate"):
        b.forecast(node, ForecastSpec.forward_fill())
    for node, formula in {
        "ebitda_margin": "ebitda / revenue",
        "ebit": "ebitda - depreciation",
        "debt_end": "110000000 - cumsum(amortization)",
        "debt_begin": "debt_end + amortization",
        "cash_interest": "debt_begin * interest_rate / 4",
        "pretax_income": "ebit - cash_interest",
        "cash_taxes": "max(pretax_income, 0) * tax_rate",
        "net_income": "pretax_income - cash_taxes",
        "cash_from_operations": "net_income + depreciation - change_nwc",
        "free_cash_flow": "cash_from_operations - capex",
        "change_cash": "free_cash_flow - amortization",
        "cash_end": "10000000 + cumsum(change_cash)",
        "ppe_end": "100000000 + cumsum(capex) - cumsum(depreciation)",
        "nwc_end": "3000000 + cumsum(change_nwc)",
        "equity_end": "3000000 + cumsum(net_income)",
        "assets": "cash_end + ppe_end + nwc_end",
        "liabilities_and_equity": "debt_end + equity_end",
        "balance_check": "assets - liabilities_and_equity",
        "annualized_ebitda": "ebitda * 4",
        "net_debt": "debt_end - cash_end",
        "net_leverage": "net_debt / annualized_ebitda",
        "interest_coverage": "ebitda / cash_interest",
        "dscr": "(ebitda - cash_taxes - capex - change_nwc) / (cash_interest + amortization)",
        "leverage_headroom": "4.5 - net_leverage",
        "dscr_headroom": "dscr - 1.1",
        "ufcf": "ebit * (1 - tax_rate) + depreciation - capex - change_nwc",
    }.items():
        b.compute(node, formula)
    return b


def borrower_model() -> FinancialModelSpec:
    """Return a fresh native borrower statements model ready for evaluation.

    Returns:
        Built ``FinancialModelSpec`` from :func:`borrower_builder`.

    Raises:
        ValueError: If a native formula or forecast definition is invalid.

    >>> borrower_model().has_node("ufcf")
    True
    """
    return borrower_builder().build()


def borrower_vintage(vintage: str = "2024Q4") -> FinancialModelSpec:
    """Return a fresh forecast vintage with only information then available.

    Args:
        vintage: Latest actual quarter, either ``2024Q3`` or ``2024Q4``.
            The Q3 vintage forecasts revenue/COGS growth at 1% per quarter;
            the Q4 vintage incorporates the Q4 actuals and raises growth to 2%.

    Returns:
        Native model with the same node and period IDs across vintages.
        Later actual observations are removed from the earlier model, so a
        forecast-versus-actual comparison does not leak future data.

    Raises:
        ValueError: If ``vintage`` is unsupported or model validation fails.

    >>> borrower_vintage("2024Q3").has_node("revenue")
    True
    """
    if vintage not in ("2024Q3", "2024Q4"):
        raise ValueError("vintage must be '2024Q3' or '2024Q4'")
    spec = json.loads(borrower_model().to_json())
    spec["id"] = f"analyst-borrower-{vintage}"
    spec["meta"]["forecast_vintage"] = vintage
    for period in spec["periods"]:
        period["is_actual"] = period["id"] <= vintage
    for node in spec["nodes"].values():
        if "values" in node:
            node["values"] = {period: value for period, value in node["values"].items() if period <= vintage}
    if vintage == "2024Q3":
        for node in ("revenue", "cogs"):
            spec["nodes"][node]["forecast"]["params"]["rate"] = 0.01
    return FinancialModelSpec.from_json(json.dumps(spec))


def borrower_data() -> dict[str, Any]:
    """Return explicit credit assumptions, covenant rules and loan economics.

    Returns:
        Fresh JSON-compatible teaching inputs. Money is in USD units, rates
        and probabilities are decimals, spread is in bp, and coverage/leverage
        limits are multiples. Physical one-year PD and LGD are underwriting
        assumptions, independent of the quote-implied risk-neutral hazard curve.
        Scenarios change forecast assumptions only; prior actuals stay fixed.
        The model assumes flat 5% SOFR, while loan valuation uses market forwards.

    >>> borrower_data()["loan"]["holding_notional"]
    1000000.0
    """
    return {
        "borrower_id": "ACME",
        "currency": "USD",
        "money_unit": "currency units",
        "vintages": ["2024Q3", "2024Q4"],
        "forecast_periods": list(PERIODS[4:]),
        "credit": {
            "measure": "physical",
            "horizon_years": 1.0,
            "base": {"pd": 0.025, "lgd": 0.55},
            "downside": {"pd": 0.06, "lgd": 0.65},
            "severe": {"pd": 0.15, "lgd": 0.75},
        },
        "covenants": [
            {"id": "net-leverage", "node": "net_leverage", "operator": "<=", "limit": 4.5, "unit": "multiple"},
            {"id": "debt-service", "node": "dscr", "operator": ">=", "limit": 1.1, "unit": "multiple"},
        ],
        "loan": {
            "instrument_id": "BORROWER-TL",
            "facility_notional": 100_000_000.0,
            "holding_notional": 1_000_000.0,
            "holding_fraction": 0.01,
            "issue_date": "2025-01-15",
            "maturity": "2030-01-15",
            "index_id": "USD-SOFR-3M",
            "model_sofr": 0.05,
            "spread_bp": 250.0,
            "amortization_per_quarter_of_original": 0.025,
            "index_floor": 0.0,
            "day_count": "act_360",
            "coupon_frequency": "3M",
            "settlement_days": 2,
            "physical_exposure_at_default": 1_000_000.0,
        },
        "scenarios": {
            "base": {
                "revenue_growth_per_quarter": 0.02,
                "cogs_growth_per_quarter": 0.02,
                "opex_growth_per_quarter": 0.015,
                "annual_interest_rate": 0.075,
            },
            "downside": {
                "revenue_growth_per_quarter": -0.02,
                "cogs_growth_per_quarter": -0.01,
                "opex_growth_per_quarter": 0.02,
                "annual_interest_rate": 0.085,
            },
            "severe": {
                "revenue_growth_per_quarter": -0.05,
                "cogs_growth_per_quarter": -0.02,
                "opex_growth_per_quarter": 0.025,
                "annual_interest_rate": 0.095,
            },
        },
    }
