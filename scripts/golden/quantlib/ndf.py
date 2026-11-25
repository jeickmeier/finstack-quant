"""Non-deliverable forward golden builder on QuantLib discount curves.

QuantLib has no non-deliverable forward instrument, so this fixture takes its
discount factors from QuantLib flat curves and evaluates the covered-interest-
parity forward and the cash settlement here, independently of Finstack.
"""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
# Both sides discount the same dated cash flow on the same flat curves; only
# double-precision evaluation order differs.
ARITHMETIC_REL = 1e-9


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _discount(rate: float, date: str) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    curve = ql.FlatForward(evaluation_date, rate, ql.Actual365Fixed(), ql.Continuous)
    return curve.discount(ql_date(date))


def build_ndf() -> dict[str, Any]:
    """Build a six-month off-market USD/CNY non-deliverable forward fixture."""
    spot, contract_rate = 7.20, 7.00
    fixing, maturity = "2026-10-28", "2026-10-30"
    notional = 10_000_000.0
    usd_rate, cny_rate = 0.04, 0.02

    def npv(usd_per_cny_shift: float = 0.0, rate_shift: float = 0.0) -> float:
        df_usd = _discount(usd_rate + rate_shift, maturity)
        df_cny = _discount(cny_rate + rate_shift, maturity)
        # A relative move of the CNY/USD rate (USD per CNY) divides the CNY-per-USD spot.
        forward = spot / (1.0 + usd_per_cny_shift) * df_usd / df_cny
        # Long CNY against USD, cash-settled in USD on the maturity date.
        return notional * (1.0 / forward - 1.0 / contract_rate) * df_usd

    expected = {
        "dv01": central_difference(lambda shift: npv(rate_shift=shift), 0.0),
        "fx01": central_difference(lambda shift: npv(usd_per_cny_shift=shift), 0.0, 0.01),
        "npv": npv(),
    }
    reason = (
        "One USD settlement N*(1/F - 1/K) with the covered-interest-parity forward F = S*DF_USD/DF_CNY, "
        "discounted on QuantLib flat curves, against the same valuation in Finstack; only floating-point "
        "evaluation order differs."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name="usdcny_ndf_6m_quantlib",
                domain="fx.ndf",
                description="Six-month off-market USD/CNY non-deliverable forward valued on QuantLib discount curves.",
                product="ndf",
                source_detail=(
                    f"QuantLib {ql.__version__} FlatForward discount factors (continuous Act/365F, USD {usd_rate}, "
                    f"CNY {cny_rate}) to maturity {maturity}; forward F = S*DF_USD/DF_CNY with spot S = {spot} CNY "
                    f"per USD; USD settlement N*(1/F - 1/K)*DF_USD, N = {notional:.0f} CNY, K = {contract_rate}. "
                    "DV01 is the central 1bp parallel move of both curves; FX01 the central 1% relative move of "
                    "the CNY/USD rate."
                ),
            )
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", usd_rate), flat_discount_curve("CNY-OIS", cny_rate)],
            fx={
                "config": {"pivot_currency": "USD", "cache_capacity": 256},
                "quotes": [["USD", "CNY", spot]],
                "pinned_quotes": [],
                "provider_quotes": [],
                "provider_pinned_quotes": [],
            },
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "ndf",
                "spec": {
                    "id": "USDCNY-NDF-6M-QUANTLIB",
                    "base_currency": "CNY",
                    "settlement_currency": "USD",
                    "fixing_date": fixing,
                    "maturity": maturity,
                    "notional": {"amount": f"{notional:.0f}", "currency": "CNY"},
                    "contract_rate": contract_rate,
                    "quote_convention": "base_per_settlement",
                    "domestic_discount_curve_id": "USD-OIS",
                    "foreign_discount_curve_id": "CNY-OIS",
                    "fixing_source": "pboc",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {metric: {"rel": ARITHMETIC_REL, "tolerance_reason": reason} for metric in expected},
    }
